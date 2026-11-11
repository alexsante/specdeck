//! Small markdown helpers shared by the parsers.

use regex::Regex;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        pub(crate) static $name: ::std::sync::LazyLock<::regex::Regex> =
            ::std::sync::LazyLock::new(|| ::regex::Regex::new($pat).unwrap());
    };
}
pub(crate) use re;

re!(BOLD, r"\*\*(.+?)\*\*");
re!(CODE, r"`([^`]+)`");
re!(LINK, r"\[([^\]]+)\]\([^)]+\)");

/// Remove bold, inline code and link markup, then trim.
pub fn strip_md(s: &str) -> String {
    let s = BOLD.replace_all(s, "$1");
    let s = CODE.replace_all(&s, "$1");
    let s = LINK.replace_all(&s, "$1");
    s.trim().to_string()
}

/// `strip_md`, then cut to `n` characters with an ellipsis.
pub fn clip(s: &str, n: usize) -> String {
    let s = strip_md(s);
    if s.chars().count() <= n {
        return s;
    }
    let cut: String = s.chars().take(n - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Split markdown into (heading, body) pairs at one heading level. A body
/// stops at the next heading of the same level or any higher level.
pub fn sections(text: &str, level: usize) -> Vec<(String, String)> {
    let pat = Regex::new(&format!(r"(?m)^{} (.+)$", "#".repeat(level))).unwrap();
    let higher = (level > 1).then(|| Regex::new(&format!(r"(?m)^#{{1,{}}} ", level - 1)).unwrap());
    let matches: Vec<_> = pat.captures_iter(text).collect();
    let mut out = Vec::new();
    for (i, c) in matches.iter().enumerate() {
        let m = c.get(0).unwrap();
        let end = matches.get(i + 1).map_or(text.len(), |n| n.get(0).unwrap().start());
        let mut body = &text[m.end()..end];
        if let Some(h) = higher.as_ref().and_then(|h| h.find(body)) {
            body = &body[..h.start()];
        }
        out.push((c[1].trim().to_string(), body.to_string()));
    }
    out
}

/// Cut to `n` characters with an ellipsis, without touching markup.
pub fn cut(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let c: String = s.chars().take(n - 1).collect();
    format!("{}…", c.trim_end())
}

re!(COMMENT, r"(?s)<!--.*?-->");

/// Remove HTML comments.
pub fn strip_comments(s: &str) -> String {
    COMMENT.replace_all(s, "").into_owned()
}

/// Over `n` characters, cut at the last blank line before `n` so no block is split.
pub fn cut_blocks(body: &str, n: usize) -> String {
    if body.chars().count() <= n {
        return body.to_string();
    }
    let head: String = body.chars().take(n).collect();
    let head = match head.rfind("\n\n") {
        Some(at) if at > 0 => &head[..at],
        _ => &head[..],
    };
    format!("{}\n\n…", head.trim_end())
}

/// Markdown body of the first `## ` section whose heading starts with one of
/// `names` (lowercase), comments removed, cut by [`cut_blocks`].
pub fn section_md(text: &str, names: &[&str], n: usize) -> Option<String> {
    let (_, body) = sections(text, 2).into_iter().find(|(h, _)| {
        let h = h.to_lowercase();
        names.iter().any(|x| h.starts_with(x))
    })?;
    Some(cut_blocks(strip_comments(&body).trim(), n))
}

/// Read a UTF-8 file, normalizing line endings. Missing, unreadable, or
/// non-UTF-8 files (and directories) read as `None`.
pub fn read(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.replace('\r', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markup() {
        assert_eq!(strip_md("**Bold** and `code` and [link](http://x)"), "Bold and code and link");
    }

    #[test]
    fn cuts_at_blank_lines() {
        assert_eq!(cut_blocks("aaa\n\nbbb", 20), "aaa\n\nbbb");
        assert_eq!(cut_blocks("aaa\n\nbbbbbb", 7), "aaa\n\n…");
        assert_eq!(section_md("# T\n\n## Summary\n\n<!-- x -->Hi.\n\n## Next\n", &["summary"], 50).as_deref(), Some("Hi."));
    }

    #[test]
    fn clips_by_characters() {
        assert_eq!(clip("ééééé", 5), "ééééé");
        assert_eq!(clip("abc defgh", 5), "abc…");
    }
}
