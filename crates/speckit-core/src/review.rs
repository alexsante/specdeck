//! review.md: a sequence of passes (pre-implementation ANALYZE, post-
//! implementation REVIEW, re-runs), each with a verdict and findings tables.

use crate::model::{Finding, Pass, Review};
use crate::text::{clip, re, strip_md};
use std::collections::HashMap;

pub const SEVERITIES: [&str; 5] = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "INFO"];
const RESOLVED: [&str; 8] = ["resolved", "closed", "fixed", "n/a", "accepted", "addressed", "done", "verified"];

re!(H1, r"(?m)^# (.+)$");
re!(H2_START, r"(?m)^## ");
// "first pass", "second pass", "detection passes" are pass names, not verdicts.
re!(PASS_NAME, r"(?i)\b(first|second|third|fourth|final|next|later|prior|detection|\d+(?:st|nd|rd|th)?)\s+pass(?:es)?\b");
re!(VERDICT_INLINE, r"(?i)verdict[^\n]*?\b(pass|fail)\b");
re!(VERDICT_NEXT_LINE, r"(?i)verdict[^\n]*\n+\s*\**\s*(pass|fail)\b");
re!(CURRENT_VERDICT, r"(?i)current verdict[^:]*:\s*\**\s*(pass|fail)");
re!(GATE_REVIEW, r"\bREVIEW\b|post-?implementation");
re!(GATE_ANALYZE, r"(?i)\bANALYZE\b|pre-?implementation");
re!(SEVERITY_HEADER, r"(?i)\|\s*Severity\s*\|");

fn severity_alias(s: &str) -> &str {
    match s {
        "BLOCKER" => "CRITICAL",
        "MAJOR" => "HIGH",
        "MODERATE" => "MEDIUM",
        "MINOR" => "LOW",
        "TRIVIAL" | "NIT" | "NOTE" => "INFO",
        other => other,
    }
}

pub fn find_verdict(text: &str) -> Option<String> {
    let t = PASS_NAME.replace_all(text, "");
    VERDICT_INLINE
        .captures(&t)
        .or_else(|| VERDICT_NEXT_LINE.captures(&t))
        .map(|c| c[1].to_uppercase())
}

fn gate_of(heading: &str) -> Option<String> {
    if GATE_REVIEW.is_match(heading) {
        Some("Review".into())
    } else if GATE_ANALYZE.is_match(heading) {
        Some("Analyze".into())
    } else {
        None
    }
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// Rows of every markdown table with a Severity column, mapped by header name.
pub fn parse_findings(text: &str) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !(line.starts_with('|') && SEVERITY_HEADER.is_match(line)) {
            i += 1;
            continue;
        }
        let cols: Vec<String> = cells(line).into_iter().map(|c| c.to_lowercase()).collect();
        i += 2; // skip the separator row
        while i < lines.len() && lines[i].starts_with('|') {
            let row: HashMap<&str, String> = cols.iter().map(String::as_str).zip(cells(lines[i])).collect();
            let get = |k: &str| row.get(k).map(String::as_str).unwrap_or("");
            let first = |keys: &[&str]| keys.iter().map(|k| get(k)).find(|v| !v.is_empty()).unwrap_or("").to_string();
            let sev_raw = get("severity");
            let sev = if sev_raw.is_empty() {
                String::new()
            } else {
                strip_md(sev_raw).to_uppercase().split(' ').next().unwrap_or("").to_string()
            };
            let sev = severity_alias(&sev).to_string();
            if SEVERITIES.contains(&sev.as_str()) {
                let status = strip_md(get("status"));
                let resolved = status
                    .split_whitespace()
                    .next()
                    .map(|w| RESOLVED.contains(&w.to_lowercase().trim_matches(|c| c == '.' || c == ',' || c == ':')))
                    .unwrap_or(false);
                let owner = strip_md(get("owner"));
                out.push(Finding {
                    id: strip_md(get("id")),
                    severity: sev,
                    owner: (!owner.is_empty()).then_some(owner),
                    status: (!status.is_empty()).then_some(status),
                    resolved,
                    text: clip(&first(&["finding", "summary", "description", "issue", "evidence"]), 300),
                    fix: clip(&first(&["recommended fix", "recommendation", "suggestion", "resolution"]), 240),
                });
            }
            i += 1;
        }
    }
    out
}

/// A pass starts at a `##` section that states a verdict. Sections without
/// one (such as `## Findings`) belong to the pass before them. The last pass
/// is the current one.
pub fn parse_review(text: &str) -> Review {
    let title = H1.captures(text).map(|c| strip_md(&c[1])).unwrap_or_else(|| "Review".into());
    let starts: Vec<usize> = H2_START.find_iter(text).map(|m| m.start()).collect();
    let preamble = &text[..starts.first().copied().unwrap_or(text.len())];

    let mut passes: Vec<Pass> = Vec::new();
    for (i, &s) in starts.iter().enumerate() {
        let sec = &text[s..starts.get(i + 1).copied().unwrap_or(text.len())];
        let first_line = sec.lines().next().unwrap_or("");
        let heading = first_line.chars().skip(3).collect::<String>().trim().to_string();
        let v = find_verdict(sec);
        let f = parse_findings(sec);
        if v.is_some() || passes.is_empty() {
            passes.push(Pass {
                label: strip_md(&heading),
                gate: gate_of(&heading).or_else(|| gate_of(&title)),
                verdict: v,
                findings: f,
            });
        } else {
            let last = passes.last_mut().unwrap();
            last.findings.extend(f);
            if last.verdict.is_none() {
                last.verdict = v;
            }
        }
    }
    if passes.is_empty() {
        passes.push(Pass { label: title.clone(), gate: gate_of(&title), verdict: find_verdict(preamble), findings: vec![] });
    }

    // A preamble "Current verdict" line is the fallback for the overall verdict.
    let cur = CURRENT_VERDICT.captures(&PASS_NAME.replace_all(preamble, "")).map(|c| c[1].to_uppercase());
    let verdict = passes.last().unwrap().verdict.clone().or(cur).or_else(|| find_verdict(preamble));

    // Within a pass, a repeated finding ID keeps its first position and its last content.
    for p in &mut passes {
        let mut order: Vec<Finding> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for f in p.findings.drain(..) {
            let key = if f.id.is_empty() { f.text.clone() } else { f.id.clone() };
            match index.get(&key) {
                Some(&i) => order[i] = f,
                None => {
                    index.insert(key, order.len());
                    order.push(f);
                }
            }
        }
        p.findings = order;
    }
    Review { title, verdict, passes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_names_are_not_verdicts() {
        assert_eq!(find_verdict("## Verdict (first pass)\n\n**FAIL**: 1 HIGH"), Some("FAIL".into()));
    }

    #[test]
    fn splits_passes_and_normalizes_severity() {
        let md = "# Review: X (pre-implementation ANALYZE)\n\n## Verdict (first pass)\n\n**FAIL**\n\n## Findings\n\n| ID | Severity | Status |\n|---|---|---|\n| A-01 | major | **Resolved** |\n\n## REVIEW (post-implementation)\n\n### Verdict: PASS\n\n| ID | Severity | Finding |\n|---|---|---|\n| R-01 | MEDIUM | Thing |\n";
        let r = parse_review(md);
        assert_eq!(r.passes.len(), 2);
        assert_eq!(r.passes[0].verdict.as_deref(), Some("FAIL"));
        assert_eq!(r.passes[0].findings[0].severity, "HIGH");
        assert!(r.passes[0].findings[0].resolved);
        assert_eq!(r.passes[1].gate.as_deref(), Some("Review"));
        assert_eq!(r.verdict.as_deref(), Some("PASS"));
    }
}
