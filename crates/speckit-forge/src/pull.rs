//! Is there anything to pull? Read-only: `git ls-remote` asks the remote for its branch tip
//! without fetching, so nothing in `.git` changes (FR-002). The forge can add counts.

use crate::local::git;
use crate::run::Runner;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullStatus {
    /// current, behind (pull needed), ahead (push needed, nothing to pull), unpublished, unknown
    pub state: String,
    /// The upstream checked, for example `origin/main`.
    pub upstream: Option<String>,
    /// New commits on the remote, when known.
    pub behind: Option<usize>,
    /// How many files under `specs/` those commits change, when known.
    pub spec_files: Option<usize>,
    pub message: Option<String>,
}

/// `compare(local_head, remote_tip)` returns (commits behind, spec files changed) from the forge,
/// for when the remote commits aren't in the local repo yet.
pub fn pull_status(r: &dyn Runner, repo: &Path, compare: impl Fn(&str, &str) -> Option<(usize, usize)>) -> PullStatus {
    let unknown = |m: &str| PullStatus { state: "unknown".into(), message: Some(m.into()), ..Default::default() };
    let Some(branch) = git(r, repo, &["branch", "--show-current"]).filter(|b| !b.is_empty()) else {
        return unknown("Detached HEAD: no branch to compare.");
    };
    // The upstream if one is set, else the branch of the same name on origin.
    let up = git(r, repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).filter(|u| u.contains('/'));
    let (remote, name) = match &up {
        Some(u) => {
            let (a, b) = u.split_once('/').unwrap();
            (a.to_string(), b.to_string())
        }
        None => ("origin".to_string(), branch.clone()),
    };
    let upstream = Some(format!("{remote}/{name}"));
    let Ok(o) = r.run("git", &["ls-remote", "--heads", &remote, &format!("refs/heads/{name}")], None, Some(repo)) else {
        return PullStatus { upstream, ..unknown("Couldn't reach the remote.") };
    };
    if !o.ok {
        return PullStatus { upstream, ..unknown("Couldn't reach the remote.") };
    }
    let Some(tip) = o.stdout.split_whitespace().next().map(String::from) else {
        return PullStatus { state: "unpublished".into(), upstream, ..Default::default() };
    };
    let head = git(r, repo, &["rev-parse", "HEAD"]).unwrap_or_default();
    if tip == head {
        return PullStatus { state: "current".into(), upstream, ..Default::default() };
    }
    let known = git(r, repo, &["cat-file", "-e", &format!("{tip}^{{commit}}")]).is_some();
    if known {
        if git(r, repo, &["merge-base", "--is-ancestor", &tip, "HEAD"]).is_some() {
            return PullStatus { state: "ahead".into(), upstream, ..Default::default() };
        }
        let behind = git(r, repo, &["rev-list", "--count", &format!("HEAD..{tip}")]).and_then(|n| n.trim().parse().ok());
        let spec_files = git(r, repo, &["diff", "--name-only", &format!("HEAD...{tip}"), "--", "specs/"]).map(|s| s.lines().filter(|l| !l.is_empty()).count());
        return PullStatus { state: "behind".into(), upstream, behind, spec_files, message: None };
    }
    // The remote has commits this repo has never seen: a pull is needed. Ask the forge for counts.
    let (behind, spec_files) = compare(&head, &tip).map_or((None, None), |(b, s)| (Some(b), Some(s)));
    PullStatus { state: "behind".into(), upstream, behind, spec_files, message: None }
}

/// The one write the app makes, and only when you click Pull: `git pull --ff-only` from the
/// branch's upstream. Refuses when anything is uncommitted or a merge or rebase is in progress,
/// so an agent's work in progress is never touched. Returns a one-line summary.
pub fn pull_ff(r: &dyn Runner, repo: &Path) -> Result<String, String> {
    let branch = git(r, repo, &["branch", "--show-current"]).filter(|b| !b.is_empty()).ok_or("You're on a detached HEAD, so there's no branch to pull.")?;
    let gitdir = git(r, repo, &["rev-parse", "--absolute-git-dir"]).map(std::path::PathBuf::from).unwrap_or_else(|| repo.join(".git"));
    if ["MERGE_HEAD", "rebase-merge", "rebase-apply", "CHERRY_PICK_HEAD"].iter().any(|f| gitdir.join(f).exists()) {
        return Err("A merge or rebase is in progress. Finish it in a terminal first.".into());
    }
    let dirty = git(r, repo, &["status", "--porcelain", "--untracked-files=no"]).unwrap_or_default();
    let n = dirty.lines().filter(|l| !l.trim().is_empty()).count();
    if n > 0 {
        return Err(format!("{n} uncommitted {} in this checkout. Commit or stash {} first, so nothing in progress is overwritten.", if n == 1 { "change" } else { "changes" }, if n == 1 { "it" } else { "them" }));
    }
    let up = git(r, repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).filter(|u| u.contains('/'));
    let (remote, name) = match &up {
        Some(u) => {
            let (a, b) = u.split_once('/').unwrap();
            (a.to_string(), b.to_string())
        }
        None => ("origin".to_string(), branch.clone()),
    };
    let before = git(r, repo, &["rev-parse", "HEAD"]).unwrap_or_default();
    let o = r.run("git", &["pull", "--ff-only", "--no-rebase", &remote, &name], None, Some(repo)).map_err(|e| format!("git pull didn't finish: {e}"))?;
    if !o.ok {
        let e = format!("{}\n{}", o.stderr, o.stdout).to_lowercase();
        return Err(if e.contains("not possible to fast-forward") || e.contains("diverg") {
            format!("Your branch and {remote}/{name} have diverged. Pull in a terminal to merge or rebase.")
        } else if e.contains("would be overwritten") {
            "Untracked files would be overwritten by the pull. Move them aside, then try again.".into()
        } else if e.contains("terminal prompts disabled") || e.contains("could not read username") || e.contains("authentication failed") || e.contains("permission denied") {
            "Git needs your credentials. Pull once in a terminal so your credential helper remembers them.".into()
        } else if e.contains("could not resolve host") || e.contains("unable to access") || e.contains("timed out") {
            format!("Couldn't reach {remote}. Check your connection, then try again.")
        } else {
            e.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git pull failed.").to_string()
        });
    }
    let after = git(r, repo, &["rev-parse", "HEAD"]).unwrap_or_default();
    if after == before {
        return Ok("Already up to date.".into());
    }
    let commits: usize = git(r, repo, &["rev-list", "--count", &format!("{before}..{after}")]).and_then(|n| n.trim().parse().ok()).unwrap_or(0);
    let specs = git(r, repo, &["diff", "--name-only", &before, &after, "--", "specs/"]).map_or(0, |s| s.lines().filter(|l| !l.is_empty()).count());
    let c = if commits == 1 { "1 commit".to_string() } else { format!("{commits} commits") };
    Ok(match specs {
        0 => format!("Pulled {c}. No spec files changed."),
        1 => format!("Pulled {c}. 1 spec file updated."),
        n => format!("Pulled {c}. {n} spec files updated."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Fake;

    fn base() -> Fake {
        Fake::default().on(&["branch --show-current"], true, "feature/x").on(&["@{u}"], true, "origin/feature/x").on(&["rev-parse HEAD"], true, "LOCAL")
    }
    const NO: fn(&str, &str) -> Option<(usize, usize)> = |_, _| None;

    #[test]
    fn current() {
        let f = base().on(&["ls-remote"], true, "LOCAL\trefs/heads/feature/x");
        assert_eq!(pull_status(&f, Path::new("."), NO).state, "current");
    }

    #[test]
    fn behind_with_unknown_commits_asks_the_forge() {
        let f = base().on(&["ls-remote"], true, "REMOTE\trefs/heads/feature/x").on(&["cat-file"], false, "");
        let s = pull_status(&f, Path::new("."), |a, b| (a == "LOCAL" && b == "REMOTE").then_some((3, 2)));
        assert_eq!((s.state.as_str(), s.behind, s.spec_files, s.upstream.as_deref()), ("behind", Some(3), Some(2), Some("origin/feature/x")));
    }

    #[test]
    fn behind_with_fetched_commits_counts_locally() {
        let f = base()
            .on(&["ls-remote"], true, "REMOTE\trefs/heads/feature/x")
            .on(&["cat-file"], true, "")
            .on(&["merge-base"], false, "")
            .on(&["rev-list --count"], true, "4")
            .on(&["diff --name-only"], true, "specs/a/spec.md\nspecs/a/tasks.md\nspecs/b/plan.md");
        let s = pull_status(&f, Path::new("."), NO);
        assert_eq!((s.behind, s.spec_files), (Some(4), Some(3)));
    }

    #[test]
    fn ahead_and_unpublished_and_offline() {
        let f = base().on(&["ls-remote"], true, "OLD\trefs/heads/feature/x").on(&["cat-file"], true, "").on(&["merge-base"], true, "");
        assert_eq!(pull_status(&f, Path::new("."), NO).state, "ahead");
        let f = base().on(&["ls-remote"], true, "");
        assert_eq!(pull_status(&f, Path::new("."), NO).state, "unpublished");
        let f = base().on(&["ls-remote"], false, "fatal: unable to access");
        assert_eq!(pull_status(&f, Path::new("."), NO).state, "unknown");
    }

    #[test]
    fn no_upstream_uses_origin_same_name() {
        let f = Fake::default().on(&["branch --show-current"], true, "main").on(&["@{u}"], false, "").on(&["rev-parse HEAD"], true, "L").on(&["ls-remote --heads origin refs/heads/main"], true, "L\trefs/heads/main");
        let s = pull_status(&f, Path::new("."), NO);
        assert_eq!((s.state.as_str(), s.upstream.as_deref()), ("current", Some("origin/main")));
    }

    #[test]
    fn pull_refuses_dirty_and_reports_divergence() {
        let f = base().on(&["--absolute-git-dir"], true, "/nonexistent/.git").on(&["status --porcelain"], true, " M specs/a/tasks.md\n M x.go");
        assert!(pull_ff(&f, Path::new(".")).unwrap_err().contains("2 uncommitted changes"));
        let f = base().on(&["--absolute-git-dir"], true, "/nonexistent/.git").on(&["status --porcelain"], true, "").on(&["pull --ff-only"], false, "fatal: Not possible to fast-forward, aborting.");
        assert!(pull_ff(&f, Path::new(".")).unwrap_err().contains("diverged"));
        let f = base().on(&["--absolute-git-dir"], true, "/nonexistent/.git").on(&["status --porcelain"], true, "").on(&["pull --ff-only"], false, "fatal: could not read Username for 'https://github.com': terminal prompts disabled");
        assert!(pull_ff(&f, Path::new(".")).unwrap_err().contains("credentials"));
    }

    #[test]
    fn pull_summary() {
        // HEAD changes between the two rev-parse calls: first LOCAL, then NEW.
        let f = std::sync::Arc::new(base().on(&["--absolute-git-dir"], true, "/nonexistent/.git").on(&["status --porcelain"], true, "").on(&["pull --ff-only --no-rebase origin feature/x"], true, ""));
        struct Seq(std::sync::Arc<Fake>, std::sync::Mutex<u32>);
        impl Runner for Seq {
            fn run(&self, p: &str, a: &[&str], i: Option<&str>, c: Option<&Path>) -> std::io::Result<crate::run::Output> {
                if a == ["rev-parse", "HEAD"] {
                    let mut n = self.1.lock().unwrap();
                    *n += 1;
                    return Ok(crate::run::Output { ok: true, stdout: if *n == 1 { "LOCAL" } else { "NEW" }.into(), stderr: String::new() });
                }
                if a.first() == Some(&"rev-list") { return Ok(crate::run::Output { ok: true, stdout: "1".into(), stderr: String::new() }); }
                if a.first() == Some(&"diff") { return Ok(crate::run::Output { ok: true, stdout: "specs/a/spec.md\nspecs/a/tasks.md\n".into(), stderr: String::new() }); }
                self.0.run(p, a, i, c)
            }
        }
        let s = Seq(f.clone(), std::sync::Mutex::new(0));
        assert_eq!(pull_ff(&s, Path::new(".")).unwrap(), "Pulled 1 commit. 2 spec files updated.");
        assert!(f.calls().iter().any(|c| c.contains("pull --ff-only --no-rebase origin feature/x")));
    }
}
