//! Read-only git queries about the checkout.

use crate::run::Runner;
use std::collections::BTreeSet;
use std::path::Path;

pub fn git(r: &dyn Runner, repo: &Path, args: &[&str]) -> Option<String> {
    match r.run("git", args, None, Some(repo)) {
        Ok(o) if o.ok => Some(o.stdout.trim_end().to_string()),
        _ => None,
    }
}

/// `owner/name` from a GitHub remote URL (https or ssh); None for other hosts.
pub fn github_slug(url: &str) -> Option<String> {
    let rest = url.trim().split_once("github.com").map(|(_, r)| r)?;
    let s = rest.trim_start_matches([':', '/']).trim_end_matches('/');
    let s = s.strip_suffix(".git").unwrap_or(s);
    let mut parts = s.split('/');
    let (o, n) = (parts.next()?, parts.next()?);
    (!o.is_empty() && !n.is_empty()).then(|| format!("{o}/{n}"))
}

/// Paths (repo-relative) with uncommitted changes under `prefix`, including untracked files.
pub fn dirty(r: &dyn Runner, repo: &Path, prefix: &str) -> BTreeSet<String> {
    git(r, repo, &["status", "--porcelain", "--untracked-files=all", "--", prefix])
        .unwrap_or_default()
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| {
            let p = &l[3..];
            p.rsplit(" -> ").next().unwrap_or(p).trim_matches('"').to_string()
        })
        .collect()
}

/// Every file under `dir`, as paths relative to it, sorted.
pub fn files_under(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slugs() {
        assert_eq!(github_slug("https://github.com/Wrecking/proj-nava.git").as_deref(), Some("Wrecking/proj-nava"));
        assert_eq!(github_slug("git@github.com:alexsante/speckit-mac.git").as_deref(), Some("alexsante/speckit-mac"));
        assert_eq!(github_slug("ssh://git@github.com/a/b").as_deref(), Some("a/b"));
        assert_eq!(github_slug("https://gitlab.com/a/b.git"), None);
    }
}
