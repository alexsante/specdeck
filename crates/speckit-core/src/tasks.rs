//! tasks.md: checkbox tasks grouped under `## Phase N: …` headings.

use crate::model::{Phase, Task};
use crate::text::{clip, re};

re!(HEADING, r"^(#{2,3}) (.+)$");
re!(PHASE_WORD, r"(?i)^Phase\b");
re!(TASK, r"^\s*-\s*\[([ xX])\]\s*(?:\*\*)?(T\d+)(?:\*\*)?\s*(.*)$");
re!(TAG, r"^\[([^\]]+)\]\s*");
re!(STORY_TAG, r"^US\d+$");
re!(PHASE_PREFIX, r"^Phase\s*\d+\s*[:—–-]\s*");
re!(DEFERRED, r"(?i)^deferred\b");
re!(FILE_SPAN, r"`([^`\s]+)`");
re!(FILE_EXT, r"\.[A-Za-z]{1,5}$");

/// Code spans that look like paths: contain a slash or end in a file extension.
pub fn task_files(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in FILE_SPAN.captures_iter(text) {
        let span = c[1].trim_end_matches(['.', ',', ':', ';']).to_string();
        if (span.contains('/') || FILE_EXT.is_match(&span)) && !span.starts_with("http") && !out.contains(&span) {
            out.push(span);
        }
    }
    out.truncate(10);
    out
}

pub fn parse_tasks(text: &str) -> Vec<Phase> {
    let mut phases: Vec<Phase> = Vec::new();
    let mut cur: Option<usize> = None;
    let new_phase = |title: &str| Phase { title: title.to_string(), tasks: vec![], total: 0, done: 0, deferred: false };

    for line in text.lines() {
        if let Some(h) = HEADING.captures(line) {
            if h[1].len() == 2 {
                let title = h[2].trim();
                cur = if PHASE_WORD.is_match(title) {
                    phases.push(new_phase(title));
                    Some(phases.len() - 1)
                } else {
                    None // a non-phase section such as "## Dependencies"
                };
            }
            continue;
        }
        let Some(t) = TASK.captures(line) else { continue };
        let idx = *cur.get_or_insert_with(|| {
            phases.push(new_phase("Other")); // task outside any phase heading
            phases.len() - 1
        });
        let mut rest = t.get(3).map_or("", |m| m.as_str());
        let mut tags = Vec::new();
        while let Some(tag) = TAG.captures(rest) {
            tags.push(tag[1].to_string());
            rest = &rest[tag.get(0).unwrap().end()..];
        }
        phases[idx].tasks.push(Task {
            id: t[2].to_string(),
            done: &t[1] != " ",
            parallel: tags.iter().any(|t| t == "P"),
            story: tags.iter().find(|t| STORY_TAG.is_match(t)).cloned(),
            tags: tags.iter().filter(|t| *t != "P" && !STORY_TAG.is_match(t)).cloned().collect(),
            text: clip(rest, 160),
            raw: {
                let r = rest.trim();
                if r.chars().count() <= 1200 { r.to_string() } else { format!("{}…", r.chars().take(1199).collect::<String>()) }
            },
            files: task_files(rest),
        });
    }

    phases.retain(|p| !p.tasks.is_empty());
    for p in &mut phases {
        p.title = PHASE_PREFIX.replace(&p.title, "").into_owned();
        p.total = p.tasks.len();
        p.done = p.tasks.iter().filter(|t| t.done).count();
        p.deferred = DEFERRED.is_match(&p.title);
    }
    phases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_file_paths() {
        let f = task_files("Edit `apps/web/src/main.tsx`, `README.md` and `make test`; see `https://x.io/a`.");
        assert_eq!(f, vec!["apps/web/src/main.tsx", "README.md"]);
    }

    #[test]
    fn handles_variants() {
        let md = "# Tasks\n\n## Format\n\n## Phase 1: Setup\n\n- [x] T001 [P] [US1] Do a\n- [X] **T002** [DG-1] Do b\n\n## Dependencies\n\n## Phase 2: Deferred — later\n\n- [ ] T003 Do c\n";
        let p = parse_tasks(md);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].title.as_str(), p[0].done, p[0].total), ("Setup", 2, 2));
        assert!(p[0].tasks[0].parallel);
        assert_eq!(p[0].tasks[0].story.as_deref(), Some("US1"));
        assert_eq!(p[0].tasks[1].tags, vec!["DG-1"]);
        assert!(p[1].deferred);
    }
}
