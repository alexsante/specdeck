//! Git worktrees of a project (FR-071 to FR-075).
//!
//! Everything here is read-only: `git worktree list`, `git status`, `git rev-list`,
//! and reading the spec folders from disk. Nothing in this module is part of the
//! `extract.py` parity check; [`crate::read_project`] is unchanged.

use crate::{git, list_dir, read_feature};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One entry of `git worktree list --porcelain`.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    /// Commit checked out (full hash; empty for a fresh repo).
    pub head: String,
    /// Short branch name; None for a detached HEAD.
    pub branch: Option<String>,
    /// The main worktree (git lists it first).
    pub main: bool,
    /// Git still lists it, but its folder is gone.
    pub prunable: bool,
}

/// Parse `git worktree list --porcelain`. Bare entries are skipped.
pub fn parse_worktree_list(text: &str) -> Vec<Worktree> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let mut w = Worktree::default();
        let mut bare = false;
        for line in block.lines() {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            match k {
                "worktree" => w.path = v.to_string(),
                "HEAD" => w.head = v.to_string(),
                "branch" => w.branch = Some(v.strip_prefix("refs/heads/").unwrap_or(v).to_string()),
                "bare" => bare = true,
                "prunable" => w.prunable = true,
                _ => {}
            }
        }
        if w.path.is_empty() || bare {
            continue;
        }
        w.main = out.is_empty();
        if !Path::new(&w.path).is_dir() {
            w.prunable = true;
        }
        out.push(w);
    }
    out
}

/// The worktrees of the repo that contains `repo`, main first. A folder that is not
/// a git repo yields itself as the only (main) worktree.
pub fn list_worktrees(repo: &Path) -> Vec<Worktree> {
    let list = parse_worktree_list(&git(repo, &["worktree", "list", "--porcelain"]));
    if !list.is_empty() {
        return list;
    }
    let path = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    vec![Worktree { path: path.display().to_string(), main: true, ..Default::default() }]
}

/// One spec folder as a worktree has it: enough to compare copies and list them.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpecCopy {
    pub id: String,
    pub ticket: String,
    pub title: String,
    pub stage: String,
    pub done: usize,
    pub total: usize,
    pub branch: Option<String>,
    /// Hash of every file's path and bytes in the folder; equal hashes mean identical copies.
    pub fingerprint: String,
    /// Files in the folder with uncommitted changes (relative to the folder).
    pub uncommitted: Vec<String>,
}

/// What one worktree holds, read from its folder and git state.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pub worktree: Worktree,
    pub specs: Vec<SpecCopy>,
    /// `.specify/feature.json`, as written.
    pub current: Option<serde_json::Value>,
    /// Newest change time under `specs/` and `.specify/` (RFC 3339).
    pub activity: Option<String>,
    /// Uncommitted files in the whole worktree.
    pub dirty: usize,
    /// Commits on this branch that the main worktree's branch lacks, and the reverse.
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
}

/// FNV-1a, stable across runs and platforms.
struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    for name in list_dir(dir) {
        let p = dir.join(&name);
        if p.is_dir() {
            walk(base, &p, out);
        } else {
            out.push(p.strip_prefix(base).unwrap_or(&p).to_path_buf());
        }
    }
}

/// Hash a folder's files (relative paths and contents, in sorted order).
pub fn fingerprint(dir: &Path) -> String {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    let mut h = Fnv::new();
    for f in files {
        h.write(f.to_string_lossy().replace('\\', "/").as_bytes());
        h.write(&[0]);
        h.write(&std::fs::read(dir.join(&f)).unwrap_or_default());
        h.write(&[0]);
    }
    format!("{:016x}", h.0)
}

fn newest_mtime(dir: &Path, best: &mut Option<std::time::SystemTime>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            newest_mtime(&p, best);
        } else if let Ok(t) = e.metadata().and_then(|m| m.modified()) {
            if best.map_or(true, |b| t > b) {
                *best = Some(t);
            }
        }
    }
}

/// Uncommitted paths from `git status --porcelain -z` (renames give the new path).
pub fn parse_status_z(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts = text.split('\0');
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let code = &entry[..2];
        out.push(entry[3..].to_string());
        if code.starts_with('R') || code.starts_with('C') {
            parts.next(); // the original path
        }
    }
    out
}

/// Read one worktree. `main_branch` is the main worktree's branch, for ahead/behind counts.
pub fn snapshot(w: &Worktree, main_branch: Option<&str>) -> Snapshot {
    let mut s = Snapshot { worktree: w.clone(), ..Default::default() };
    if w.prunable {
        return s;
    }
    let root = Path::new(&w.path);
    // Not `git()`: it trims, and a status line can start with a space.
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let changed = parse_status_z(&status);
    s.dirty = changed.len();
    let specs_dir = root.join("specs");
    let ids: Vec<String> = list_dir(&specs_dir).into_iter().filter(|n| specs_dir.join(n).is_dir()).collect();
    s.specs = ids
        .iter()
        .map(|id| {
            let f = read_feature(&specs_dir, id);
            let (done, total) = f.tasks.as_deref().unwrap_or(&[]).iter().fold((0, 0), |a, p| (a.0 + p.done, a.1 + p.total));
            let prefix = format!("specs/{id}/");
            SpecCopy {
                title: f.spec.as_ref().map(|s| s.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| f.slug.replace('-', " ")),
                branch: f.spec.as_ref().and_then(|s| s.branch.clone()),
                id: f.id,
                ticket: f.ticket,
                stage: f.stage,
                done,
                total,
                fingerprint: fingerprint(&specs_dir.join(id)),
                uncommitted: changed.iter().filter_map(|p| p.strip_prefix(&prefix).map(str::to_string)).collect(),
            }
        })
        .collect();
    s.current = crate::text::read(&root.join(".specify").join("feature.json")).and_then(|t| serde_json::from_str(&t).ok());
    let mut t = None;
    newest_mtime(&specs_dir, &mut t);
    newest_mtime(&root.join(".specify"), &mut t);
    s.activity = t.map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    if let (Some(mb), false) = (main_branch, w.main) {
        let counts = git(root, &["rev-list", "--left-right", "--count", &format!("{mb}...HEAD")]);
        let mut it = counts.split_whitespace().filter_map(|n| n.parse().ok());
        s.behind = it.next();
        s.ahead = it.next();
    }
    s
}

/// Read every worktree of the repo that contains `repo`, in parallel, main first.
pub fn snapshots(repo: &Path) -> Vec<Snapshot> {
    let list = list_worktrees(repo);
    let main_branch = list.first().and_then(|w| w.branch.clone());
    std::thread::scope(|sc| {
        let handles: Vec<_> = list.iter().map(|w| sc.spawn(|| snapshot(w, main_branch.as_deref()))).collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain() {
        let t = "worktree /tmp\nHEAD abc\nbranch refs/heads/main\n\nworktree /nonexistent/x\nHEAD def\ndetached\nprunable gitdir file points to non-existent location\n\nworktree /srv/bare\nbare\n";
        let l = parse_worktree_list(t);
        assert_eq!(l.len(), 2);
        assert!(l[0].main && !l[0].prunable);
        assert_eq!(l[0].branch.as_deref(), Some("main"));
        assert!(!l[1].main && l[1].prunable && l[1].branch.is_none());
        assert_eq!(l[1].head, "def");
    }

    #[test]
    fn status_z() {
        let t = " M specs/1-a/spec.md\0?? specs/2-b/tasks.md\0R  specs/1-a/new.md\0specs/1-a/old.md\0";
        assert_eq!(parse_status_z(t), vec!["specs/1-a/spec.md", "specs/2-b/tasks.md", "specs/1-a/new.md"]);
    }

    /// A real repo with a linked worktree: copies, uncommitted files, ahead/behind.
    #[test]
    fn real_worktrees() {
        let base = std::env::temp_dir().join(format!("speckit-wt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let main = base.join("main");
        let run = |dir: &Path, args: &[&str]| {
            let o = std::process::Command::new("git").arg("-C").arg(dir).args(args)
                .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t")
                .output().unwrap();
            assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        };
        std::fs::create_dir_all(main.join("specs/001-a")).unwrap();
        std::fs::write(main.join("specs/001-a/spec.md"), "# Feature Specification: A\n").unwrap();
        run(&main, &["init", "-q", "-b", "main"]);
        run(&main, &["add", "."]);
        run(&main, &["commit", "-q", "-m", "a"]);
        let linked = base.join("wt");
        run(&main, &["worktree", "add", "-q", "-b", "feature/b", linked.to_str().unwrap()]);
        std::fs::create_dir_all(linked.join("specs/002-b")).unwrap();
        std::fs::write(linked.join("specs/002-b/spec.md"), "# Feature Specification: B\n").unwrap();
        run(&linked, &["add", "."]);
        run(&linked, &["commit", "-q", "-m", "b"]);
        std::fs::write(linked.join("specs/001-a/spec.md"), "# Feature Specification: A, edited\n").unwrap();

        let snaps = snapshots(&linked);
        assert_eq!(snaps.len(), 2);
        let (m, w) = (&snaps[0], &snaps[1]);
        assert!(m.worktree.main && !w.worktree.main);
        assert_eq!(w.worktree.branch.as_deref(), Some("feature/b"));
        assert_eq!((w.ahead, w.behind), (Some(1), Some(0)));
        assert_eq!(w.dirty, 1);
        let a = |s: &Snapshot| s.specs.iter().find(|c| c.id == "001-a").unwrap().clone();
        assert_ne!(a(m).fingerprint, a(w).fingerprint);
        assert_eq!(a(w).uncommitted, vec!["spec.md"]);
        assert!(m.specs.iter().all(|c| c.id != "002-b"));
        assert!(w.specs.iter().any(|c| c.id == "002-b" && c.title == "B"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
