//! End to end with real git: a bare "remote", two clones, a commit pushed from one,
//! then the other sees it (read-only) and pulls it (fast-forward only).

use speckit_forge::pull::{pull_ff, pull_status};
use speckit_forge::run::Process;
use std::path::Path;
use std::process::Command;

fn sh(dir: &Path, args: &[&str]) {
    let o = Command::new("git").args(args).current_dir(dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn behind_then_pull_then_current() {
    let root = std::env::temp_dir().join(format!("forge-real-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    sh(&root, &["init", "-q", "--bare", "-b", "main", "remote.git"]);
    sh(&root, &["clone", "-q", "remote.git", "a"]);
    let a = root.join("a");
    std::fs::create_dir_all(a.join("specs/001-x")).unwrap();
    std::fs::write(a.join("specs/001-x/spec.md"), "# X\n").unwrap();
    sh(&a, &["add", "."]);
    sh(&a, &["commit", "-qm", "first"]);
    sh(&a, &["push", "-q", "origin", "HEAD:main"]);
    sh(&root, &["clone", "-q", "remote.git", "b"]);
    let b = root.join("b");

    let r = Process::default();
    let none = |_: &str, _: &str| None;
    assert_eq!(pull_status(&r, &b, none).state, "current");

    std::fs::write(a.join("specs/001-x/tasks.md"), "- [ ] T001 do it\n").unwrap();
    sh(&a, &["add", "."]);
    sh(&a, &["commit", "-qm", "tasks"]);
    sh(&a, &["push", "-q", "origin", "HEAD:main"]);

    let fetch_head = b.join(".git/FETCH_HEAD");
    let before = std::fs::metadata(&fetch_head).ok().and_then(|m| m.modified().ok());
    let s = pull_status(&r, &b, none);
    assert_eq!(s.state, "behind", "{s:?}");
    assert_eq!(std::fs::metadata(&fetch_head).ok().and_then(|m| m.modified().ok()), before, "status must not fetch");

    // Uncommitted work blocks the pull and is left alone.
    std::fs::write(b.join("specs/001-x/spec.md"), "# X, edited by an agent\n").unwrap();
    assert!(pull_ff(&r, &b).unwrap_err().contains("uncommitted"));
    assert_eq!(std::fs::read_to_string(b.join("specs/001-x/spec.md")).unwrap(), "# X, edited by an agent\n");
    sh(&b, &["checkout", "--", "specs/001-x/spec.md"]);

    assert_eq!(pull_ff(&r, &b).unwrap(), "Pulled 1 commit. 1 spec file updated.");
    assert!(b.join("specs/001-x/tasks.md").exists());
    assert_eq!(pull_status(&r, &b, none).state, "current");
    assert_eq!(pull_ff(&r, &b).unwrap(), "Already up to date.");

    // Diverged: a local commit and a new remote commit. Fast-forward only refuses.
    std::fs::write(a.join("specs/001-x/plan.md"), "plan\n").unwrap();
    sh(&a, &["add", "."]);
    sh(&a, &["commit", "-qm", "plan"]);
    sh(&a, &["push", "-q", "origin", "HEAD:main"]);
    std::fs::write(b.join("local.txt"), "mine\n").unwrap();
    sh(&b, &["add", "."]);
    sh(&b, &["commit", "-qm", "local"]);
    assert!(pull_ff(&r, &b).unwrap_err().contains("diverged"));
    let _ = std::fs::remove_dir_all(&root);
}
