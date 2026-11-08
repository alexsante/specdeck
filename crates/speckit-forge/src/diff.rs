//! Which right-side (new file) lines of a unified diff patch can take a line comment.

/// Inclusive line ranges on the new side that appear in the patch (context or added lines).
pub fn commentable(patch: &str) -> Vec<[u32; 2]> {
    let mut out: Vec<[u32; 2]> = Vec::new();
    let mut right: u32 = 0;
    let mut in_hunk = false;
    for l in patch.lines() {
        if let Some(rest) = l.strip_prefix("@@ ") {
            // @@ -a,b +c,d @@ heading
            let plus = rest.split_whitespace().find(|t| t.starts_with('+')).unwrap_or("+0");
            right = plus[1..].split(',').next().and_then(|n| n.parse().ok()).unwrap_or(0);
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        match l.chars().next() {
            Some('-') => {}
            Some('\\') => {} // "\ No newline at end of file"
            _ => {
                // ' ' context, '+' added, or an empty context line
                match out.last_mut() {
                    Some(r) if r[1] + 1 == right => r[1] = right,
                    _ => out.push([right, right]),
                }
                right += 1;
            }
        }
    }
    out
}

/// True when every line from `start` to `end` lies inside one of `ranges`.
pub fn covers(ranges: &[[u32; 2]], start: u32, end: u32) -> bool {
    (start..=end).all(|n| ranges.iter().any(|r| r[0] <= n && n <= r[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks() {
        let p = "@@ -1,3 +1,4 @@\n line1\n-old\n+new\n+added\n line3\n@@ -20,2 +21,2 @@ heading\n ctx\n-gone\n+here\n\\ No newline at end of file";
        assert_eq!(commentable(p), vec![[1, 4], [21, 22]]);
        assert!(covers(&commentable(p), 2, 4));
        assert!(!covers(&commentable(p), 4, 6));
    }

    #[test]
    fn single_line_hunk() {
        assert_eq!(commentable("@@ -1 +1 @@\n-a\n+b"), vec![[1, 1]]);
    }
}
