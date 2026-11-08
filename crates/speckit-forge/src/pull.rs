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
}
