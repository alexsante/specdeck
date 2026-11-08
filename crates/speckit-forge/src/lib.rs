//! Review threads for Spec Kit files on a forge, behind one adapter interface (FR-067).
//!
//! The only adapter today is GitHub, which talks to GitHub through the `gh` CLI so
//! the app never sees a token (FR-061). Nothing here writes to the repository on disk.

pub mod diff;
pub mod github;
pub mod local;
pub mod run;

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// One readiness check, in the order FR-060 lists them.
#[derive(Serialize, Clone, Debug)]
pub struct Check {
    pub id: String,
    pub ok: bool,
    pub message: String,
}

/// A pull request (GitHub) or merge request (GitLab) for a feature's branch.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequest {
    pub forge: String,
    pub repo: String,
    pub number: u64,
    pub url: String,
    pub state: String, // open, merged, closed
    pub draft: bool,
    pub head_ref: String,
    pub base: String,
    pub head: String,
    pub node_id: String,
    pub pending_review: Option<String>,
    /// Files the PR changes, and how many of them are under `specs/`.
    pub changed_files: usize,
    pub spec_files: usize,
}

/// What can be done with one file of the feature folder.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileState {
    /// Relative to the feature folder (`spec.md`, `contracts/http.md`).
    pub path: String,
    pub in_pr: bool,
    /// Byte-identical to the file at the PR head.
    pub in_sync: bool,
    pub can_comment: bool,
    /// Why commenting is off for this file, as one line.
    pub reason: Option<String>,
    /// Added file: every line takes a line comment.
    pub all_lines: bool,
    /// Modified file: the right-side lines inside the diff.
    pub ranges: Vec<[u32; 2]>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub created_at: String,
    pub body: String,
    pub pending: bool,
    pub url: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    /// Relative to the feature folder.
    pub path: String,
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub file_level: bool,
    pub state: String, // open, resolved, outdated, pending
    pub resolved_by: Option<String>,
    /// For outdated threads: the text of the line the comment was made on.
    pub was: Option<String>,
    pub comments: Vec<Comment>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub feature: String,
    pub checks: Vec<Check>,
    /// Threads could be loaded.
    pub read: bool,
    /// The change request takes new comments (open). Each file adds its own condition.
    pub write: bool,
    /// First failing check, as the one-line message the interface shows.
    pub message: Option<String>,
    pub login: Option<String>,
    pub branch: Option<String>,
    pub cr: Option<ChangeRequest>,
    pub files: Vec<FileState>,
    pub threads: Vec<Thread>,
}

impl Status {
    fn pass(&mut self, id: &str) {
        self.checks.push(Check { id: id.into(), ok: true, message: String::new() });
    }
    fn fail(&mut self, id: &str, message: String) {
        if self.message.is_none() {
            self.message = Some(message.clone());
        }
        self.checks.push(Check { id: id.into(), ok: false, message });
    }
    pub fn file(&self, path: &str) -> Option<&FileState> {
        self.files.iter().find(|f| f.path == path)
    }
}

/// Where a comment goes. Carries all three commits so a GitLab adapter, which needs
/// base, start and head, fits without changing callers.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Relative to the repository root.
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub file_level: bool,
    pub base: String,
    pub start_commit: String,
    pub head: String,
}

/// Turn "lines `start`–`end` of `file`" into a position, or say why it can't be commented on.
pub fn position(st: &Status, file: &str, start: u32, end: u32) -> Result<Position, String> {
    if let Some(m) = st.checks.iter().find(|c| !c.ok) {
        return Err(m.message.clone());
    }
    let cr = st.cr.as_ref().ok_or("No pull request.")?;
    let f = st.file(file).ok_or_else(|| format!("{file} is not in this spec's folder."))?;
    if !f.can_comment {
        return Err(f.reason.clone().unwrap_or_else(|| format!("{file} can't take comments.")));
    }
    let (start, end) = (start.min(end).max(1), start.max(end).max(1));
    Ok(Position {
        path: format!("specs/{}/{}", st.feature, file),
        start,
        end,
        file_level: !f.all_lines && !diff::covers(&f.ranges, start, end),
        base: cr.base.clone(),
        start_commit: cr.base.clone(),
        head: cr.head.clone(),
    })
}

/// A forge adapter. `feature` is a folder name under `specs/`; `branch_hint` is the
/// spec's `**Feature Branch**:` header.
pub trait Forge: Send + Sync {
    fn status(&self, repo: &Path, feature: &str, branch_hint: Option<&str>) -> Status;
    fn add_comment(&self, cr: &ChangeRequest, pos: &Position, body: &str, pending: bool) -> Result<(), String>;
    /// Always submitted as a plain comment review: no approve, no request changes (FR-063).
    fn submit_review(&self, cr: &ChangeRequest, body: &str) -> Result<(), String>;
    fn discard_review(&self, cr: &ChangeRequest) -> Result<(), String>;
    fn reply(&self, cr: &ChangeRequest, thread: &str, body: &str) -> Result<(), String>;
    fn set_resolved(&self, thread: &str, resolved: bool) -> Result<(), String>;
    fn edit_comment(&self, comment: &str, body: &str) -> Result<(), String>;
    fn delete_comment(&self, comment: &str) -> Result<(), String>;
    /// Unresolved threads on spec files across open change requests, by feature folder.
    fn open_threads_by_feature(&self, repo: &Path) -> Result<BTreeMap<String, usize>, String>;
}
