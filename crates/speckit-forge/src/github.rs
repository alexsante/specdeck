//! GitHub adapter. Every call goes through `gh api`; strings reach GitHub only as JSON on
//! stdin (`--input -`), never as `-F` fields, which `gh` would expand (`@file`).

use crate::diff;
use crate::local::{self, git};
use crate::run::{Output, Process, Runner};
use crate::{ChangeRequest, Comment, FileState, Forge, Position, Status, Thread};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

pub struct GitHub {
    runner: Box<dyn Runner>,
    /// The `gh` program once found: "gh" on PATH, or a path from the login shell.
    program: Mutex<Option<String>>,
}

impl Default for GitHub {
    fn default() -> Self {
        GitHub::new(Box::new(Process::default()))
    }
}

pub fn install_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "`brew install gh`"
    } else if cfg!(target_os = "windows") {
        "`winget install GitHub.cli`"
    } else {
        "your package manager (for example `sudo pacman -S github-cli` or `sudo apt install gh`)"
    }
}

const PR_FIELDS: &str = "nodes{id number state isDraft url headRefName headRefOid baseRefOid reviews(states:PENDING,first:1){nodes{id}}}";

impl GitHub {
    pub fn new(runner: Box<dyn Runner>) -> Self {
        GitHub { runner, program: Mutex::new(None) }
    }

    /// Find `gh`. Desktop launchers often lack the shell's PATH (mise, Homebrew), so
    /// fall back to asking the login shell where it is.
    fn program(&self) -> Option<String> {
        if let Some(p) = self.program.lock().unwrap().clone() {
            return Some(p);
        }
        let works = |p: &str| matches!(self.runner.run(p, &["--version"], None, None), Ok(o) if o.ok);
        let mut found = works("gh").then(|| "gh".to_string());
        if found.is_none() && !cfg!(target_os = "windows") {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            for flag in ["-lc", "-ic"] {
                if let Ok(o) = self.runner.run(&shell, &[flag, "command -v gh"], None, None) {
                    if let Some(p) = o.stdout.lines().map(str::trim).filter(|l| l.starts_with('/')).last() {
                        if works(p) {
                            found = Some(p.to_string());
                            break;
                        }
                    }
                }
            }
        }
        *self.program.lock().unwrap() = found.clone();
        found
    }

    fn gh(&self, args: &[&str], stdin: Option<&str>) -> Result<Output, String> {
        let p = self.program().ok_or("The GitHub CLI (gh) was not found.")?;
        self.runner.run(&p, args, stdin, None).map_err(|e| e.to_string())
    }

    /// Run a GraphQL query; return `data`, or the first error message.
    fn graphql(&self, query: &str, vars: Value) -> Result<Value, String> {
        let input = json!({ "query": query, "variables": vars }).to_string();
        let o = self.gh(&["api", "graphql", "--input", "-"], Some(&input))?;
        let v: Value = serde_json::from_str(&o.stdout).unwrap_or(Value::Null);
        if let Some(msg) = v["errors"][0]["message"].as_str() {
            return Err(msg.to_string());
        }
        if !o.ok {
            return Err(first_line(&o.stderr));
        }
        Ok(v["data"].clone())
    }

    fn rest_post(&self, path: &str, body: Value) -> Result<(), String> {
        let o = self.gh(&["api", "-X", "POST", path, "--input", "-"], Some(&body.to_string()))?;
        if o.ok {
            return Ok(());
        }
        let v: Value = serde_json::from_str(&o.stdout).unwrap_or(Value::Null);
        let detail = v["errors"][0]["message"].as_str().or(v["errors"][0].as_str()).or(v["message"].as_str()).map(String::from);
        Err(detail.unwrap_or_else(|| first_line(&o.stderr)))
    }

    /// The PR's files: (path, status, blob sha, patch). `gh pr view` stops at 100; this pages.
    fn pr_files(&self, repo: &str, n: u64) -> Result<Vec<(String, String, String, String)>, String> {
        let url = format!("repos/{repo}/pulls/{n}/files?per_page=100");
        let o = self.gh(&["api", "--paginate", &url, "--jq", ".[] | [.filename, .status, .sha, (.patch // \"\")]"], None)?;
        if !o.ok {
            return Err(first_line(&o.stderr));
        }
        Ok(o.stdout
            .lines()
            .filter_map(|l| serde_json::from_str::<(String, String, String, String)>(l).ok())
            .collect())
    }

    fn threads(&self, cr: &ChangeRequest, prefix: &str) -> Result<Vec<Thread>, String> {
        let (o, r) = cr.repo.split_once('/').unwrap_or_default();
        let q = "query($o:String!,$r:String!,$n:Int!,$after:String){repository(owner:$o,name:$r){pullRequest(number:$n){\
            reviewThreads(first:100,after:$after){pageInfo{hasNextPage endCursor} nodes{id isResolved isOutdated path line startLine subjectType resolvedBy{login} \
            comments(first:100){nodes{id author{login} body createdAt state url diffHunk}}}}}}}";
        let mut out = Vec::new();
        let mut after = Value::Null;
        loop {
            let d = self.graphql(q, json!({"o": o, "r": r, "n": cr.number, "after": after}))?;
            let rt = &d["repository"]["pullRequest"]["reviewThreads"];
            for t in rt["nodes"].as_array().into_iter().flatten() {
                let path = t["path"].as_str().unwrap_or_default();
                let Some(rel) = path.strip_prefix(prefix) else { continue };
                out.push(thread(t, rel));
            }
            if rt["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
            after = rt["pageInfo"]["endCursor"].clone();
        }
        Ok(out)
    }
}

fn first_line(s: &str) -> String {
    s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("gh failed without a message").to_string()
}

fn thread(t: &Value, rel: &str) -> Thread {
    let comments: Vec<Comment> = t["comments"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| Comment {
            id: c["id"].as_str().unwrap_or_default().into(),
            author: c["author"]["login"].as_str().unwrap_or("ghost").into(),
            created_at: c["createdAt"].as_str().unwrap_or_default().into(),
            body: c["body"].as_str().unwrap_or_default().into(),
            pending: c["state"] == "PENDING",
            url: c["url"].as_str().unwrap_or_default().into(),
        })
        .collect();
    let file_level = t["subjectType"] == "FILE";
    let end = t["line"].as_u64().map(|n| n as u32);
    let start = t["startLine"].as_u64().map(|n| n as u32).or(end);
    let state = if !comments.is_empty() && comments.iter().all(|c| c.pending) {
        "pending"
    } else if t["isResolved"] == true {
        "resolved"
    } else if t["isOutdated"] == true || (!file_level && end.is_none()) {
        "outdated"
    } else {
        "open"
    };
    let was = (state == "outdated")
        .then(|| t["comments"]["nodes"][0]["diffHunk"].as_str().and_then(|h| h.lines().last()).map(|l| l.get(1..).unwrap_or("").to_string()))
        .flatten();
    Thread {
        id: t["id"].as_str().unwrap_or_default().into(),
        path: rel.into(),
        start,
        end,
        file_level,
        state: state.into(),
        resolved_by: t["resolvedBy"]["login"].as_str().map(String::from),
        was,
        comments,
    }
}

/// Choose among a branch's PRs: newest open, else newest merged, else newest.
fn pick(nodes: &Value) -> Option<&Value> {
    let ns = nodes.as_array()?;
    ns.iter().find(|p| p["state"] == "OPEN").or_else(|| ns.iter().find(|p| p["state"] == "MERGED")).or(ns.first())
}

fn change_request(p: &Value, repo: &str) -> ChangeRequest {
    ChangeRequest {
        forge: "github".into(),
        repo: repo.into(),
        number: p["number"].as_u64().unwrap_or(0),
        url: p["url"].as_str().unwrap_or_default().into(),
        state: p["state"].as_str().unwrap_or("OPEN").to_lowercase(),
        draft: p["isDraft"] == true,
        head_ref: p["headRefName"].as_str().unwrap_or_default().into(),
        base: p["baseRefOid"].as_str().unwrap_or_default().into(),
        head: p["headRefOid"].as_str().unwrap_or_default().into(),
        node_id: p["id"].as_str().unwrap_or_default().into(),
        pending_review: p["reviews"]["nodes"][0]["id"].as_str().map(String::from),
        changed_files: 0,
        spec_files: 0,
    }
}

/// One line for a failed call to GitHub during the readiness checks.
fn unreachable(err: &str, repo: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("401") || e.contains("bad credentials") {
        "Your GitHub sign-in has expired. Run `gh auth login`, then click Check again.".into()
    } else if e.contains("could not resolve to a repository") || e.contains("404") {
        format!("GitHub doesn't show you `{repo}`. Check that your account has access to it.")
    } else {
        format!("Can't reach `github.com/{repo}`. Check your connection or VPN, then click Check again.")
    }
}

impl Forge for GitHub {
    fn status(&self, repo: &Path, feature: &str, branch_hint: Option<&str>) -> Status {
        let mut s = Status { feature: feature.into(), ..Default::default() };
        let r = self.runner.as_ref();

        // 1. The CLI is installed.
        if self.program().is_none() {
            s.fail("cli", format!("Comments need the GitHub CLI. Install it with {}, then click Check again.", install_hint()));
            return s;
        }
        s.pass("cli");

        // 2. Signed in (local: does gh have a token for github.com?).
        match self.gh(&["auth", "token", "--hostname", "github.com"], None) {
            Ok(o) if o.ok && !o.stdout.trim().is_empty() => s.pass("auth"),
            _ => {
                s.fail("auth", "You're not signed in to GitHub. Run `gh auth login` in a terminal, then click Check again.".into());
                return s;
            }
        }

        // 3. The remote responds, and the token works (one query also finds the PR).
        let Some(slug) = git(r, repo, &["remote", "get-url", "origin"]).as_deref().and_then(local::github_slug) else {
            s.fail("remote", "This repo's `origin` isn't on GitHub. GitLab support comes later.".into());
            return s;
        };
        let branch = git(r, repo, &["branch", "--show-current"]).filter(|b| !b.is_empty());
        s.branch = branch.clone();
        let mut cands: Vec<String> = branch.iter().cloned().collect();
        if let Some(h) = branch_hint.filter(|h| !h.is_empty() && !cands.iter().any(|c| c == h)) {
            cands.push(h.to_string());
        }
        let (o, n) = slug.split_once('/').unwrap();
        let q = format!(
            "query($o:String!,$r:String!,$b0:String!,$b1:String!){{viewer{{login}} repository(owner:$o,name:$r){{\
             c0:pullRequests(headRefName:$b0,first:5,orderBy:{{field:CREATED_AT,direction:DESC}}){{{PR_FIELDS}}} \
             c1:pullRequests(headRefName:$b1,first:5,orderBy:{{field:CREATED_AT,direction:DESC}}){{{PR_FIELDS}}}}}}}"
        );
        let b0 = cands.first().cloned().unwrap_or_default();
        let b1 = cands.get(1).cloned().unwrap_or_else(|| b0.clone());
        let d = match self.graphql(&q, json!({"o": o, "r": n, "b0": b0, "b1": b1})) {
            Ok(d) => d,
            Err(e) => {
                s.fail("remote", unreachable(&e, &slug));
                return s;
            }
        };
        s.login = d["viewer"]["login"].as_str().map(String::from);
        s.pass("remote");

        // 4. The spec has a PR: the current branch's PR if it touches the feature folder, else
        // the PR for the spec's Feature Branch header.
        let prefix = format!("specs/{feature}/");
        // Files and threads are fetched together (one round trip instead of two).
        type Files = Vec<(String, String, String, String)>;
        let mut chosen: Option<(ChangeRequest, Files, Result<Vec<Thread>, String>)> = None;
        for (i, key) in ["c0", "c1"].iter().enumerate().take(cands.len()) {
            let Some(p) = pick(&d["repository"][key]["nodes"]) else { continue };
            let cr = change_request(p, &slug);
            let (files, threads) = std::thread::scope(|sc| {
                let t = sc.spawn(|| self.threads(&cr, &prefix));
                let f = self.pr_files(&slug, cr.number);
                (f, t.join().unwrap_or_else(|_| Err("thread fetch panicked".into())))
            });
            let files = match files {
                Ok(f) => f,
                Err(e) => {
                    s.fail("pr", unreachable(&e, &slug));
                    return s;
                }
            };
            let touches = files.iter().any(|f| f.0.starts_with(&prefix));
            let is_current = i == 0 && branch.is_some();
            if touches || !is_current {
                chosen = Some((cr, files, threads));
                break;
            }
        }
        let Some((mut cr, pr_files, threads)) = chosen else {
            let b = branch_hint.or(branch.as_deref()).unwrap_or("This branch");
            s.fail("pr", format!("`{b}` has no pull request. Open one with `gh pr create --draft`, then click Check again."));
            return s;
        };
        s.pass("pr");
        cr.changed_files = pr_files.len();
        cr.spec_files = pr_files.iter().filter(|f| f.0.starts_with("specs/")).count();

        // 5. The PR is open.
        s.write = cr.state == "open";
        if s.write {
            s.pass("open");
        } else {
            s.fail("open", format!("PR #{} is {}. Its threads are read-only.", cr.number, cr.state));
        }

        // 6. Per file: on disk, byte-identical to the PR head?
        let local_files = local::files_under(&repo.join("specs").join(feature));
        let repo_paths: Vec<String> = local_files.iter().map(|f| format!("{prefix}{f}")).collect();
        let mut hash_args = vec!["hash-object", "--"];
        hash_args.extend(repo_paths.iter().map(String::as_str));
        let hashes: Vec<String> = git(r, repo, &hash_args).unwrap_or_default().lines().map(String::from).collect();
        let dirty = local::dirty(r, repo, &prefix);
        let head_local = git(r, repo, &["rev-parse", "HEAD"]).unwrap_or_default();
        let by_path: BTreeMap<&str, &(String, String, String, String)> = pr_files.iter().map(|f| (f.0.as_str(), f)).collect();
        for (i, rel) in local_files.iter().enumerate() {
            let full = &repo_paths[i];
            let mut fs = FileState { path: rel.clone(), ..Default::default() };
            match by_path.get(full.as_str()) {
                None => fs.reason = Some(format!("`{rel}` isn't changed in PR #{}, so GitHub can't take comments on it.", cr.number)),
                Some((_, status, sha, patch)) => {
                    fs.in_pr = true;
                    fs.in_sync = hashes.get(i).is_some_and(|h| h == sha);
                    fs.all_lines = status == "added";
                    fs.ranges = if fs.all_lines { vec![] } else { diff::commentable(patch) };
                    if !fs.in_sync {
                        fs.reason = Some(if dirty.contains(full) {
                            format!("`{rel}` has uncommitted changes, so its lines may not match GitHub. Commit and push them to comment on this file.")
                        } else if branch.as_deref() != Some(cr.head_ref.as_str()) {
                            format!("You're on `{}`. Check out `{}` to comment on this spec.", branch.as_deref().unwrap_or("a detached HEAD"), cr.head_ref)
                        } else if head_local != cr.head {
                            format!("Your branch isn't in sync with PR #{}. Push or pull, then click Check again.", cr.number)
                        } else {
                            format!("`{rel}` differs from PR #{}.", cr.number)
                        });
                    }
                }
            }
            fs.can_comment = s.write && fs.in_sync;
            if !s.write {
                fs.reason = s.message.clone(); // merged or closed: that is the reason, whatever the checkout
            }
            s.files.push(fs);
        }

        match threads {
            Ok(t) => {
                s.threads = t;
                s.read = true;
            }
            Err(e) => {
                s.fail("threads", unreachable(&e, &slug));
            }
        }
        s.cr = Some(cr);
        s
    }

    fn add_comment(&self, cr: &ChangeRequest, pos: &Position, body: &str, pending: bool) -> Result<(), String> {
        let body = if pos.file_level {
            let at = if pos.start == pos.end { format!("Line {}", pos.start) } else { format!("Lines {}–{}", pos.start, pos.end) };
            format!("{at}:\n\n{body}")
        } else {
            body.to_string()
        };
        if !pending {
            let mut b = json!({"body": body, "commit_id": pos.head, "path": pos.path});
            if pos.file_level {
                b["subject_type"] = json!("file");
            } else {
                b["line"] = json!(pos.end);
                b["side"] = json!("RIGHT");
                if pos.start < pos.end {
                    b["start_line"] = json!(pos.start);
                    b["start_side"] = json!("RIGHT");
                }
            }
            return self.rest_post(&format!("repos/{}/pulls/{}/comments", cr.repo, cr.number), b);
        }
        let review = match &cr.pending_review {
            Some(id) => id.clone(),
            None => {
                let d = self.graphql(
                    "mutation($pr:ID!,$c:GitObjectID!){addPullRequestReview(input:{pullRequestId:$pr,commitOID:$c}){pullRequestReview{id}}}",
                    json!({"pr": cr.node_id, "c": pos.head}),
                )?;
                d["addPullRequestReview"]["pullRequestReview"]["id"].as_str().ok_or("GitHub did not return a review id.")?.to_string()
            }
        };
        let mut input = json!({"pullRequestReviewId": review, "path": pos.path, "body": body});
        if pos.file_level {
            input["subjectType"] = json!("FILE");
        } else {
            input["subjectType"] = json!("LINE");
            input["line"] = json!(pos.end);
            input["side"] = json!("RIGHT");
            if pos.start < pos.end {
                input["startLine"] = json!(pos.start);
                input["startSide"] = json!("RIGHT");
            }
        }
        self.graphql("mutation($i:AddPullRequestReviewThreadInput!){addPullRequestReviewThread(input:$i){thread{id}}}", json!({"i": input}))
            .map(|_| ())
    }

    fn submit_review(&self, cr: &ChangeRequest, body: &str) -> Result<(), String> {
        let id = cr.pending_review.as_ref().ok_or("You have no pending review on this pull request.")?;
        self.graphql(
            "mutation($id:ID!,$b:String){submitPullRequestReview(input:{pullRequestReviewId:$id,event:COMMENT,body:$b}){pullRequestReview{id}}}",
            json!({"id": id, "b": body}),
        )
        .map(|_| ())
    }

    fn discard_review(&self, cr: &ChangeRequest) -> Result<(), String> {
        let id = cr.pending_review.as_ref().ok_or("You have no pending review on this pull request.")?;
        self.graphql("mutation($id:ID!){deletePullRequestReview(input:{pullRequestReviewId:$id}){pullRequestReview{id}}}", json!({"id": id}))
            .map(|_| ())
    }

    fn reply(&self, cr: &ChangeRequest, thread: &str, body: &str) -> Result<(), String> {
        let mut input = json!({"pullRequestReviewThreadId": thread, "body": body});
        if let Some(r) = &cr.pending_review {
            input["pullRequestReviewId"] = json!(r);
        }
        self.graphql("mutation($i:AddPullRequestReviewThreadReplyInput!){addPullRequestReviewThreadReply(input:$i){comment{id}}}", json!({"i": input}))
            .map(|_| ())
    }

    fn set_resolved(&self, thread: &str, resolved: bool) -> Result<(), String> {
        let m = if resolved { "resolveReviewThread" } else { "unresolveReviewThread" };
        self.graphql(&format!("mutation($t:ID!){{{m}(input:{{threadId:$t}}){{thread{{id}}}}}}"), json!({"t": thread})).map(|_| ())
    }

    fn edit_comment(&self, comment: &str, body: &str) -> Result<(), String> {
        self.graphql(
            "mutation($c:ID!,$b:String!){updatePullRequestReviewComment(input:{pullRequestReviewCommentId:$c,body:$b}){pullRequestReviewComment{id}}}",
            json!({"c": comment, "b": body}),
        )
        .map(|_| ())
    }

    fn delete_comment(&self, comment: &str) -> Result<(), String> {
        self.graphql("mutation($c:ID!){deletePullRequestReviewComment(input:{id:$c}){pullRequestReview{id}}}", json!({"c": comment})).map(|_| ())
    }

    fn open_threads_by_feature(&self, repo: &Path) -> Result<BTreeMap<String, usize>, String> {
        let slug = git(self.runner.as_ref(), repo, &["remote", "get-url", "origin"])
            .as_deref()
            .and_then(local::github_slug)
            .ok_or("origin isn't on GitHub")?;
        let (o, n) = slug.split_once('/').unwrap();
        let d = self.graphql(
            "query($o:String!,$r:String!){repository(owner:$o,name:$r){pullRequests(states:OPEN,first:50,orderBy:{field:UPDATED_AT,direction:DESC}){nodes{reviewThreads(first:100){nodes{isResolved path}}}}}}",
            json!({"o": o, "r": n}),
        )?;
        let mut out = BTreeMap::new();
        for p in d["repository"]["pullRequests"]["nodes"].as_array().into_iter().flatten() {
            for t in p["reviewThreads"]["nodes"].as_array().into_iter().flatten() {
                if t["isResolved"] == true {
                    continue;
                }
                if let Some(f) = t["path"].as_str().and_then(|p| p.strip_prefix("specs/")).and_then(|p| p.split('/').next()) {
                    *out.entry(f.to_string()).or_insert(0) += 1;
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Fake;
    use std::fs;

    const PRS: &str = r#"{"data":{"viewer":{"login":"me"},"repository":{
      "c0":{"nodes":[{"id":"PR_1","number":217,"state":"OPEN","isDraft":false,"url":"u","headRefName":"feature/x","headRefOid":"H","baseRefOid":"B","reviews":{"nodes":[]}}]},
      "c1":{"nodes":[{"id":"PR_1","number":217,"state":"OPEN","isDraft":false,"url":"u","headRefName":"feature/x","headRefOid":"H","baseRefOid":"B","reviews":{"nodes":[]}}]}}}}"#;
    const THREADS: &str = r#"{"data":{"repository":{"pullRequest":{"reviewThreads":{"pageInfo":{"hasNextPage":false},"nodes":[
      {"id":"T1","isResolved":false,"isOutdated":false,"path":"specs/f/spec.md","line":46,"startLine":null,"subjectType":"LINE","resolvedBy":null,
       "comments":{"nodes":[{"id":"C1","author":{"login":"dana"},"body":"hi","createdAt":"2026-11-08","state":"SUBMITTED","url":"x","diffHunk":"@@\n+text"}]}},
      {"id":"T2","isResolved":false,"isOutdated":true,"path":"specs/f/spec.md","line":null,"startLine":null,"subjectType":"LINE","resolvedBy":null,
       "comments":{"nodes":[{"id":"C2","author":{"login":"dana"},"body":"old","createdAt":"2026-11-07","state":"SUBMITTED","url":"x","diffHunk":"@@\n+was this"}]}},
      {"id":"T3","isResolved":false,"isOutdated":false,"path":"apps/x.go","line":1,"subjectType":"LINE","comments":{"nodes":[]}}]}}}}}"#;

    fn repo() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("forge-test-{}", std::process::id()));
        fs::create_dir_all(d.join("specs/f/contracts")).unwrap();
        fs::write(d.join("specs/f/spec.md"), "x").unwrap();
        fs::write(d.join("specs/f/plan.md"), "x").unwrap();
        fs::write(d.join("specs/f/contracts/a.md"), "x").unwrap();
        d
    }

    fn base() -> Fake {
        Fake::default()
            .on(&["gh --version"], true, "gh 2")
            .on(&["auth token"], true, "gho_x")
            .on(&["remote get-url"], true, "https://github.com/o/r.git")
            .on(&["branch --show-current"], true, "feature/x")
            .on(&["rev-parse HEAD"], true, "H")
            .on(&["status --porcelain"], true, "")
            // contracts/a.md, plan.md, spec.md (sorted)
            .on(&["hash-object"], true, "SA\nSP\nSS")
    }

    #[test]
    fn missing_cli() {
        let f = Fake::default().missing("gh").on(&["-lc"], true, "").on(&["-ic"], true, "");
        let s = GitHub::new(Box::new(f)).status(&repo(), "f", None);
        assert!(!s.read && !s.write);
        assert!(s.message.unwrap().contains("Install it"));
    }

    #[test]
    fn not_signed_in() {
        let f = Fake::default().on(&["gh --version"], true, "gh").on(&["auth token"], false, "no token");
        let s = GitHub::new(Box::new(f)).status(&repo(), "f", None);
        assert_eq!(s.checks.last().unwrap().id, "auth");
        assert!(s.message.unwrap().contains("gh auth login"));
    }

    #[test]
    fn offline_and_expired() {
        for (err, want) in [("dial tcp: lookup api.github.com: no such host", "Can't reach"), ("HTTP 401: Bad credentials", "expired")] {
            let f = base().on(&["api graphql"], false, err);
            let s = GitHub::new(Box::new(f)).status(&repo(), "f", None);
            assert!(s.message.as_deref().unwrap().contains(want), "{err} -> {:?}", s.message);
            assert!(!s.read);
        }
    }

    #[test]
    fn no_pr() {
        let none = r#"{"data":{"viewer":{"login":"me"},"repository":{"c0":{"nodes":[]},"c1":{"nodes":[]}}}}"#;
        let f = base().on(&["api graphql", "c0:"], true, none);
        let s = GitHub::new(Box::new(f)).status(&repo(), "f", Some("feature/x"));
        assert!(s.message.unwrap().contains("gh pr create --draft"));
    }

    #[test]
    fn ready_with_files_and_threads() {
        let files = "[\"specs/f/spec.md\",\"added\",\"SS\",\"\"]\n[\"specs/f/plan.md\",\"modified\",\"OTHER\",\"@@ -1 +1,2 @@\\n a\\n+b\"]\n[\"apps/x.go\",\"added\",\"Z\",\"\"]";
        let f = base().on(&["api graphql", "c0:"], true, PRS).on(&["--paginate"], true, files).on(&["api graphql", "reviewThreads"], true, THREADS);
        let s = GitHub::new(Box::new(f)).status(&repo(), "f", None);
        assert!(s.read && s.write, "{:?}", s.checks);
        assert_eq!(s.login.as_deref(), Some("me"));
        let spec = s.file("spec.md").unwrap();
        assert!(spec.can_comment && spec.all_lines);
        let plan = s.file("plan.md").unwrap();
        assert!(!plan.can_comment && plan.reason.as_deref().unwrap().contains("differs"));
        assert!(s.file("contracts/a.md").unwrap().reason.as_deref().unwrap().contains("isn't changed"));
        assert_eq!(s.threads.len(), 2, "code-file threads are dropped");
        assert_eq!(s.threads[0].state, "open");
        assert_eq!(s.threads[1].state, "outdated");
        assert_eq!(s.threads[1].was.as_deref(), Some("was this"));
        let p = crate::position(&s, "spec.md", 46, 44).unwrap();
        assert_eq!((p.start, p.end, p.file_level, p.head.as_str()), (44, 46, false, "H"));
        assert!(crate::position(&s, "plan.md", 1, 1).is_err());
    }

    #[test]
    fn other_branch_checked_out_is_read_only_per_file() {
        let files = "[\"specs/f/spec.md\",\"added\",\"REMOTE\",\"\"]";
        let f = Fake::default()
            .on(&["gh --version"], true, "gh 2")
            .on(&["auth token"], true, "gho_x")
            .on(&["remote get-url"], true, "git@github.com:o/r.git")
            .on(&["branch --show-current"], true, "main")
            .on(&["rev-parse HEAD"], true, "M")
            .on(&["status --porcelain"], true, "")
            .on(&["hash-object"], true, "SA\nSP\nLOCAL")
            .on(&["api graphql", "c0:"], true, &PRS.replace("\"c0\":{\"nodes\":[{", "\"c0\":{\"nodes\":[]},\"x\":{\"nodes\":[{")) // main has no PR
            .on(&["--paginate"], true, files)
            .on(&["api graphql", "reviewThreads"], true, THREADS);
        let s = GitHub::new(Box::new(f)).status(&repo(), "f", Some("feature/x"));
        assert!(s.read && s.write);
        let spec = s.file("spec.md").unwrap();
        assert!(!spec.can_comment);
        assert!(spec.reason.as_deref().unwrap().contains("Check out `feature/x`"), "{:?}", spec.reason);
    }

    #[test]
    fn writes_send_strings_as_json_and_submit_only_comment() {
        let f = std::sync::Arc::new(Fake::default().on(&["gh --version"], true, "gh 2").on(&["api"], true, r#"{"data":{"addPullRequestReview":{"pullRequestReview":{"id":"R1"}}}}"#));
        let gh = GitHub::new(Box::new(f.clone()));
        let cr = ChangeRequest { repo: "o/r".into(), number: 7, node_id: "PR_1".into(), head: "H".into(), ..Default::default() };
        let pos = Position { path: "specs/f/spec.md".into(), start: 3, end: 5, file_level: false, base: "B".into(), start_commit: "B".into(), head: "H".into() };
        gh.add_comment(&cr, &pos, "@/etc/passwd", true).unwrap();
        gh.add_comment(&cr, &Position { file_level: true, ..pos.clone() }, "note", false).unwrap();
        gh.submit_review(&ChangeRequest { pending_review: Some("R1".into()), ..cr.clone() }, "sum").unwrap();
        let calls = f.calls();
        let joined = calls.join("\n");
        assert!(calls.iter().all(|c| !c.contains(" -F ") && !c.contains(" -f ")), "{joined}");
        assert!(joined.contains("\"startLine\":3") && joined.contains("\"line\":5") && joined.contains("@/etc/passwd"));
        assert!(joined.contains("\"subject_type\":\"file\"") && joined.contains("Lines 3–5:"));
        assert!(joined.contains("event:COMMENT") && !joined.contains("APPROVE") && !joined.contains("REQUEST_CHANGES"));
    }
}
