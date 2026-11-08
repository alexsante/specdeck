//! The typed model. Field names serialize to the same JSON keys as
//! `prototype/extract.py`, so the two implementations can be diffed.

use serde::Serialize;

/// Pipeline order. "Fragment" (a folder without spec.md) sits outside it.
pub const STAGES: [&str; 7] = ["Specified", "Clarified", "Planned", "Tasked", "Analyzed", "Building", "Done"];

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub project: String,
    pub generated: String,
    pub head: String,
    pub current: Option<serde_json::Value>,
    pub stages: Vec<&'static str>,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub id: String,
    pub ticket: String,
    pub slug: String,
    pub files: Vec<String>,
    pub contracts: Vec<String>,
    pub spec: Option<Spec>,
    pub tasks: Option<Vec<Phase>>,
    pub review: Option<Review>,
    pub checklists: Vec<Checklist>,
    pub last_commit: Option<String>,
    pub first_commit: Option<String>,
    pub stage: String,
    /// "artifacts" or "status" (stage taken from the Status line).
    pub stage_source: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Spec {
    pub title: String,
    pub status: String,
    pub branch: Option<String>,
    pub created: Option<String>,
    pub input: String,
    pub clickup: Option<String>,
    pub stories: Vec<Story>,
    pub fr: usize,
    pub nfr: usize,
    pub sc: usize,
    pub markers: Vec<String>,
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Story {
    pub n: u32,
    pub title: String,
    pub priority: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub date: Option<String>,
    pub label: String,
    /// "human", "agent", "flagged" or "unlabeled".
    pub kind: String,
    pub qa: Vec<Qa>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Qa {
    pub q: String,
    pub a: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Phase {
    pub title: String,
    pub tasks: Vec<Task>,
    pub total: usize,
    pub done: usize,
    pub deferred: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub done: bool,
    pub parallel: bool,
    pub story: Option<String>,
    pub tags: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Review {
    pub title: String,
    pub verdict: Option<String>,
    pub passes: Vec<Pass>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pass {
    pub label: String,
    pub gate: Option<String>,
    pub verdict: Option<String>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub resolved: bool,
    pub text: String,
    pub fix: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Checklist {
    pub name: String,
    pub total: usize,
    pub done: usize,
}
