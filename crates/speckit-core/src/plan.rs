//! plan.md: the Summary section and the Technical Context fields.

use crate::model::Plan;
use crate::text::{cut, re, section_md, sections, strip_comments, strip_md};

re!(FIELD, r"^\s*(?:[-*]\s+)?\*\*([^*]+?)\*\*:\s*(.+)$");
re!(BULLET, r"^\s*[-*]\s+([A-Za-z][\w /&.+-]{0,39}):\s+(.+)$");
re!(ROW, r"^\s*\|(.+)\|\s*$");
re!(SEP_CELL, r"^\s*:?-+:?\s*$");

fn cells(line: &str) -> Option<Vec<&str>> {
    ROW.captures(line).map(|c| c.get(1).unwrap().as_str().split('|').map(str::trim).collect())
}

fn is_sep(line: &str) -> bool {
    cells(line).is_some_and(|c| c.iter().all(|x| SEP_CELL.is_match(x)))
}

/// Technical Context as [key, value] pairs, from `**Key**: value` lines
/// (stock Spec Kit), `- Key: value` bullets, or the rows of a table.
pub fn parse_context(body: &str) -> Vec<(String, String)> {
    let body = strip_comments(body);
    let lines: Vec<&str> = body.split('\n').collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(c) = FIELD.captures(line).or_else(|| BULLET.captures(line)) {
            out.push((strip_md(&c[1]), cut(c[2].trim(), 300)));
            continue;
        }
        let Some(cs) = cells(line) else { continue };
        if cs.len() < 2 || is_sep(line) {
            continue;
        }
        if lines.get(i + 1).is_some_and(|n| is_sep(n)) {
            continue; // header row
        }
        let rest: Vec<&str> = cs[1..].iter().copied().filter(|c| !c.is_empty()).collect();
        out.push((strip_md(cs[0]), cut(&rest.join(" · "), 300)));
    }
    out
}

pub fn parse_plan(text: &str) -> Plan {
    let ctx = sections(text, 2)
        .into_iter()
        .find(|(h, _)| h.to_lowercase().starts_with("technical context"))
        .map(|(_, b)| b)
        .unwrap_or_default();
    Plan { summary: section_md(text, &["summary"], 2000), context: parse_context(&ctx) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_stock_fields_and_tables() {
        let p = parse_plan("# P\n\n## Summary\n\nDo it.\n\n## Technical Context\n\n**Language/Version**: Rust 1.85\n- Storage: none\n\n| Area | Stack |\n| --- | --- |\n| `API` | Go |\n\n## Next\n");
        assert_eq!(p.summary.as_deref(), Some("Do it."));
        assert_eq!(p.context, vec![
            ("Language/Version".into(), "Rust 1.85".into()),
            ("Storage".into(), "none".into()),
            ("API".into(), "Go".into()),
        ]);
    }
}
