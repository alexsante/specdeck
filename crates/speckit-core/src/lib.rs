//! Reads a Spec Kit repository into a typed [`Project`].
//!
//! This is a port of `prototype/extract.py`; both must produce the same JSON
//! for the same repository (see `scripts/parity.py`). Parsing never fails as a
//! whole: a file that is missing or unreadable simply yields no data for it.

pub mod model;
pub mod plan;
pub mod review;
pub mod spec;
pub mod tasks;
pub mod worktree;
mod text;

pub use model::*;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use text::{re, read};

re!(FOLDER, r"^(\d+)-(.+)$");
re!(STATUS_LEADING, r"^[*\s]+");
re!(STATUS_DONE, r"^(implemented|delivered|shipped|done|complete)");
re!(STATUS_BUILDING, r"^in progress");

/// Run `git -C <repo> <args>` and return trimmed stdout ("" on any failure).
pub fn git(repo: &Path, args: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub(crate) fn list_dir(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

fn parse_checklists(dir: &Path) -> Vec<Checklist> {
    static ITEM: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?m)^\s*-\s*\[([ xX])\]").unwrap());
    let cdir = dir.join("checklists");
    if !cdir.is_dir() {
        return vec![];
    }
    list_dir(&cdir)
        .into_iter()
        .map(|name| {
            let text = read(&cdir.join(&name)).unwrap_or_default();
            let items: Vec<_> = ITEM.captures_iter(&text).map(|c| c[1].to_string()).collect();
            Checklist { total: items.len(), done: items.iter().filter(|c| *c != " ").count(), name }
        })
        .collect()
}

/// Stage from the artifacts present. When there is no tasks.md, a Status
/// line that says the work is implemented (or in progress) is trusted.
pub fn derive_stage(spec: Option<&Spec>, files: &[String], tasks: Option<&[Phase]>) -> (String, String) {
    let tasks = tasks.unwrap_or(&[]);
    let total: usize = tasks.iter().map(|p| p.total).sum();
    let done: usize = tasks.iter().map(|p| p.done).sum();
    let has = |f: &str| files.iter().any(|x| x == f);
    let r = |s: &str, src: &str| (s.to_string(), src.to_string());
    let Some(s) = spec else { return r("Fragment", "artifacts") };
    if total > 0 && done == total {
        return r("Done", "artifacts");
    }
    if total > 0 && done > 0 {
        return r("Building", "artifacts");
    }
    if total == 0 {
        let st = STATUS_LEADING.replace(&s.status, "").to_lowercase();
        if STATUS_DONE.is_match(&st) {
            return r("Done", "status");
        }
        if STATUS_BUILDING.is_match(&st) {
            return r("Building", "status");
        }
    }
    if has("review.md") && total > 0 {
        return r("Analyzed", "artifacts");
    }
    if total > 0 {
        return r("Tasked", "artifacts");
    }
    if has("plan.md") {
        return r("Planned", "artifacts");
    }
    if !s.sessions.is_empty() {
        return r("Clarified", "artifacts");
    }
    r("Specified", "artifacts")
}

/// Last commit touching the folder (ISO time) and the oldest commit that added a file to it (date).
fn git_dates(repo: &Path, id: &str) -> (Option<String>, Option<String>) {
    let path = format!("specs/{id}");
    let last = git(repo, &["log", "-1", "--format=%cI", "--", &path]);
    let first = git(repo, &["log", "--diff-filter=A", "--format=%cs", "--", &path]);
    let first = first.lines().last().map(str::to_string);
    ((!last.is_empty()).then_some(last), first)
}

/// Parse one feature folder (without git dates or cross-references).
pub fn read_feature(specs_dir: &Path, id: &str) -> Feature {
    let d = specs_dir.join(id);
    let files = list_dir(&d);
    let (ticket, slug) = match FOLDER.captures(id) {
        Some(c) => (c[1].to_string(), c[2].to_string()),
        None => (id.to_string(), id.to_string()),
    };
    let contracts = if files.iter().any(|f| f == "contracts") { list_dir(&d.join("contracts")) } else { vec![] };
    let spec = read(&d.join("spec.md")).map(|t| spec::parse_spec(&t));
    let plan = read(&d.join("plan.md")).map(|t| plan::parse_plan(&t));
    let tasks = read(&d.join("tasks.md")).map(|t| tasks::parse_tasks(&t));
    let review = read(&d.join("review.md")).map(|t| review::parse_review(&t));
    let (stage, stage_source) = derive_stage(spec.as_ref(), &files, tasks.as_deref());
    Feature {
        id: id.to_string(),
        ticket,
        slug,
        contracts,
        checklists: parse_checklists(&d),
        files,
        spec,
        plan,
        tasks,
        review,
        last_commit: None,
        first_commit: None,
        stage,
        stage_source,
        refs: vec![],
    }
}

/// Read the whole project. Git lookups run in parallel.
pub fn read_project(repo: &Path) -> std::io::Result<Project> {
    let specs_dir = repo.join("specs");
    if !specs_dir.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("no specs/ folder in {}", repo.display())));
    }
    let ids: Vec<String> = list_dir(&specs_dir).into_iter().filter(|n| specs_dir.join(n).is_dir()).collect();

    let mut features: Vec<Feature> = ids.iter().map(|id| read_feature(&specs_dir, id)).collect();

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = features.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for part in features.chunks_mut(chunk) {
            s.spawn(move || {
                for f in part {
                    (f.last_commit, f.first_commit) = git_dates(repo, &f.id);
                }
            });
        }
    });

    // Cross-spec references: ticket numbers that belong to another feature folder.
    let tickets: BTreeSet<String> = features.iter().map(|f| f.ticket.clone()).collect();
    for f in &mut features {
        let text = read(&specs_dir.join(&f.id).join("spec.md")).unwrap_or_default();
        let found: BTreeSet<String> = spec::ticket_mentions(&text).into_iter().collect();
        f.refs = found.into_iter().filter(|t| tickets.contains(t) && *t != f.ticket).collect();
    }

    let current = read(&repo.join(".specify").join("feature.json")).and_then(|t| serde_json::from_str(&t).ok());
    let abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    Ok(Project {
        project: abs.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        generated: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        head: git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        current,
        stages: STAGES.to_vec(),
        features,
    })
}
