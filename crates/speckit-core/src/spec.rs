//! spec.md: header fields, user stories, requirement counts, open-question
//! markers and clarification sessions.

use crate::model::{Qa, Session, Spec, Story, StoryBody};
use crate::text::{clip, cut, re, sections, strip_md};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

re!(H1, r"(?m)^# (.+)$");
re!(H2_START, r"(?m)^## ");
re!(FEATURE_PREFIX, r"^Feature Specification:\s*");
re!(HEADER, r"(?m)^\*\*([A-Za-z ][A-Za-z ]*?)\*\*:\s*(.+)$");
re!(CLICKUP, r"https://app\.clickup\.com/t/\w+");
re!(STORY, r"(?m)^###\s+(?:User Story|US)\s*(\d+)\s*[-–—:]?\s*(.*?)\s*\((?:Priority:\s*)?(P\d)\)\s*$");
re!(MARKER, r"\[NEEDS CLARIFICATION:\s*([^\]]+)\]");
re!(FENCED, r"(?ms)^```.*?^```");
re!(INLINE_CODE, r"`[^`\n]*`");
re!(QA, r"(?m)^\s*-\s*(?:\*\*)?Q\d*(?:\*\*)?:\s*(.+?)\s*(?:→|->)\s*(?:\*\*)?A\d*(?:\*\*)?:\s*(.+)$");
re!(DATE, r"\d{4}-\d{2}-\d{2}");
re!(SESSION_PREFIX, r"^Session\s*\d{4}-\d{2}-\d{2}\s*[:—–-]?\s*");
re!(PARENS, r"^\((.*)\)$");
re!(FR, r"\*\*FR-\d+");
re!(NFR, r"\*\*NFR-\d+");
re!(SC, r"\*\*SC-\d+");
re!(STORY_STOP, r"(?m)^(#{1,3} |---\s*$)");
re!(STORY_FIELD, r"(?m)^\*\*(Why this priority|Independent Test|Acceptance Scenarios)\*\*:?");
re!(STORY_WHY, r"(?m)^\*\*Why this priority\*\*:\s*(.+)$");
re!(STORY_TEST, r"(?m)^\*\*Independent Test\*\*:\s*(.+)$");
re!(STORY_ACC, r"(?m)^\*\*Acceptance Scenarios\*\*:?");
re!(SCENARIO, r"(?m)^\d+\.\s+(.+)$");
re!(HUMAN_WORDS, r"\b(human|locks?|locked|product decisions?|user-approved|accepted|binding)\b");

/// Classify a `### Session …` heading by who made the decisions.
/// Order matters, and words must match whole ("lock" is not "blocking").
pub fn session_kind(heading: &str) -> &'static str {
    let h = heading.to_lowercase();
    if h.contains("flagged") || h.contains("conflict") {
        return "flagged";
    }
    if h.contains("no human") || h.contains("without a human") {
        return "agent";
    }
    // A human confirming or deciding wins over an agent having proposed.
    if HUMAN_WORDS.is_match(&h) {
        return "human";
    }
    if h.contains("analyst default") || h.contains("agent") || h.contains("clarify pass") {
        return "agent";
    }
    "unlabeled"
}

/// Description, why-this-priority, independent test and numbered acceptance scenarios.
pub fn parse_story_body(body: &str) -> StoryBody {
    let desc = &body[..STORY_FIELD.find(body).map_or(body.len(), |m| m.start())];
    let field = |re: &regex::Regex, n: usize| re.captures(body).map(|c| cut(c[1].trim(), n));
    let scenarios = STORY_ACC
        .find(body)
        .map(|m| SCENARIO.captures_iter(&body[m.end()..]).map(|c| cut(c[1].trim(), 600)).collect())
        .unwrap_or_default();
    StoryBody { description: cut(desc.trim(), 900), why: field(&STORY_WHY, 500), test: field(&STORY_TEST, 600), scenarios }
}

pub fn parse_spec(text: &str) -> Spec {
    let title = H1.captures(text).map(|c| c[1].to_string()).unwrap_or_default();
    let title = FEATURE_PREFIX.replace(&title, "").trim().to_string();

    let head = &text[..H2_START.find(text).map_or(text.len(), |m| m.start())];
    let mut header: HashMap<String, String> = HashMap::new();
    for c in HEADER.captures_iter(head) {
        header.insert(c[1].trim().to_lowercase(), c[2].trim().to_string());
    }
    let get = |k: &str| header.get(k).map(String::as_str).unwrap_or("");
    let clickup = CLICKUP.find(&format!("{}{}", get("clickup"), get("input"))).map(|m| m.as_str().to_string());

    let stories = STORY
        .captures_iter(text)
        .map(|c| {
            let rest = &text[c.get(0).unwrap().end()..];
            let body = &rest[..STORY_STOP.find(rest).map_or(rest.len(), |m| m.start())];
            Story { n: c[1].parse().unwrap_or(0), title: clip(&c[2], 90), priority: c[3].to_string(), body: parse_story_body(body) }
        })
        .collect();

    let clar = sections(text, 2)
        .into_iter()
        .find(|(h, _)| h.to_lowercase().starts_with("clarifications"))
        .map(|(_, b)| b)
        .unwrap_or_default();
    let sessions = sections(&format!("\n{clar}"), 3)
        .into_iter()
        .filter(|(h, _)| h.to_lowercase().starts_with("session"))
        .map(|(h, b)| {
            let label = SESSION_PREFIX.replace(&h, "").trim().to_string();
            let label = PARENS.replace(&label, "$1");
            let label = strip_md(&label);
            Session {
                date: DATE.find(&h).map(|m| m.as_str().to_string()),
                label: if label.is_empty() { "Session".into() } else { label },
                kind: session_kind(&h).into(),
                qa: QA.captures_iter(&b).map(|c| Qa { q: clip(&c[1], 220), a: clip(&c[2], 260) }).collect(),
            }
        })
        .collect();

    // Markers inside code (fenced blocks or `…` spans) only mention the syntax.
    let no_code = FENCED.replace_all(text, "");
    let no_code = INLINE_CODE.replace_all(&no_code, "");
    let markers = MARKER.captures_iter(&no_code).map(|c| clip(&c[1], 200)).collect();

    let branch = strip_md(get("feature branch"));
    Spec {
        title,
        status: clip(get("status"), 160),
        branch: (!branch.is_empty()).then_some(branch),
        created: header.get("created").cloned(),
        input: clip(get("input"), 400),
        clickup,
        stories,
        fr: FR.find_iter(text).count(),
        nfr: NFR.find_iter(text).count(),
        sc: SC.find_iter(text).count(),
        markers,
        sessions,
    }
}

/// Five-digit ticket numbers (`WB-12345` or `12345`) mentioned in spec.md.
pub fn ticket_mentions(text: &str) -> Vec<String> {
    static TICKET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:WB-)?(\d{5})\b").unwrap());
    TICKET.captures_iter(text).map(|c| c[1].to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_sessions() {
        assert_eq!(session_kind("Session 2026-11-05: Clarify pass (no human input)"), "agent");
        assert_eq!(session_kind("Session 2026-11-05 (human decisions)"), "human");
        assert_eq!(session_kind("Session 2026-09-01: Analyst defaults (ambiguities resolved without blocking)"), "agent");
        assert_eq!(session_kind("Session 2026-11-08: Attention model (proposed by agent, confirmed by human)"), "human");
        assert_eq!(session_kind("Session 2026-09-03: Brownfield and prior-spec conflicts (**FLAGGED**)"), "flagged");
        assert_eq!(session_kind("Session 2026-09-03"), "unlabeled");
    }

    #[test]
    fn parses_story_bodies() {
        let md = "# T\n\n### User Story 1 - Sign in (Priority: P1)\n\nA user signs in.\n\n**Why this priority**: Everything needs it.\n\n**Independent Test**: Sign in once.\n\n**Acceptance Scenarios**:\n\n1. **Given** a user, **When** they sign in, **Then** they see home.\n2. **Given** a bad password, **When** they sign in, **Then** they see an error.\n\n---\n\n### Edge Cases\n";
        let s = parse_spec(md);
        let b = &s.stories[0].body;
        assert_eq!(b.description, "A user signs in.");
        assert_eq!(b.why.as_deref(), Some("Everything needs it."));
        assert_eq!(b.test.as_deref(), Some("Sign in once."));
        assert_eq!(b.scenarios.len(), 2);
    }

    #[test]
    fn ignores_markers_in_code() {
        let s = parse_spec("# Feature Specification: X\n\nSee `[NEEDS CLARIFICATION: no]`.\n\n[NEEDS CLARIFICATION: yes?]\n");
        assert_eq!(s.markers, vec!["yes?"]);
    }

    #[test]
    fn parses_numbered_qa_and_paren_labels() {
        let s = parse_spec("# T\n\n## Clarifications\n\n### Session 2026-11-05 (human decisions)\n\n- Q1: Slug? → A: Option A.\n- Q: Old? -> A: Yes.\n\n## Requirements\n");
        assert_eq!(s.sessions.len(), 1);
        assert_eq!(s.sessions[0].label, "human decisions");
        assert_eq!(s.sessions[0].kind, "human");
        assert_eq!(s.sessions[0].qa.len(), 2);
    }
}
