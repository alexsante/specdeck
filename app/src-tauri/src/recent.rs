//! Recently opened projects (FR-053), kept in the app's config folder. Never inside a project.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

const MAX: usize = 8;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Recent {
    pub path: String,
    pub name: String,
    pub specs: usize,
    pub building: usize,
    pub branch: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub opened: u64,
    /// Filled in when listed: the folder still exists and has `specs/`.
    #[serde(default, skip_deserializing)]
    pub ok: bool,
    /// Filled in when listed: the path with the home folder shown as `~`.
    #[serde(default, skip_deserializing)]
    pub display: String,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

pub fn display(path: &str) -> String {
    match home() {
        Some(h) if Path::new(path).starts_with(&h) => {
            let rest = Path::new(path).strip_prefix(&h).unwrap_or(Path::new(""));
            format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display())
        }
        _ => path.to_string(),
    }
}

pub fn read(file: &Path) -> Vec<Recent> {
    std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write(file: &Path, list: &[Recent]) {
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(s) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(file, s);
    }
}

/// The list as the interface shows it: newest first, with `ok` and `display` filled in.
pub fn list(file: &Path) -> Vec<Recent> {
    let mut l = read(file);
    l.sort_by(|a, b| b.opened.cmp(&a.opened));
    for r in &mut l {
        r.ok = Path::new(&r.path).join("specs").is_dir();
        r.display = display(&r.path);
    }
    l
}

/// Record that `repo` was just opened, with its summary from the view JSON.
pub fn touch(file: &Path, repo: &Path, view: &Value) {
    let path = repo.display().to_string();
    let features = view["features"].as_array();
    let entry = Recent {
        path: path.clone(),
        name: view["project"].as_str().unwrap_or_default().to_string(),
        specs: features.map_or(0, |f| f.len()),
        building: features.map_or(0, |f| f.iter().filter(|x| x["stage"] == "Building").count()),
        branch: view["head"].as_str().filter(|h| !h.is_empty()).map(String::from),
        opened: now_ms(),
        ..Default::default()
    };
    let mut l: Vec<Recent> = read(file).into_iter().filter(|r| r.path != path).collect();
    l.insert(0, entry);
    l.sort_by(|a, b| b.opened.cmp(&a.opened));
    l.truncate(MAX);
    write(file, &l);
}

pub fn forget(file: &Path, path: &str) {
    let l: Vec<Recent> = read(file).into_iter().filter(|r| r.path != path).collect();
    write(file, &l);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn touch_dedupes_orders_and_forgets() {
        let dir = std::env::temp_dir().join(format!("recent-test-{}", std::process::id()));
        let file = dir.join("recent-projects.json");
        let _ = std::fs::remove_file(&file);
        let view = |n: &str| json!({"project": n, "head": "main", "features": [{"stage": "Building"}, {"stage": "Done"}]});
        touch(&file, Path::new("/x/a"), &view("a"));
        std::thread::sleep(std::time::Duration::from_millis(3));
        touch(&file, Path::new("/x/b"), &view("b"));
        std::thread::sleep(std::time::Duration::from_millis(3));
        touch(&file, Path::new("/x/a"), &view("a"));
        let l = list(&file);
        assert_eq!(l.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!((l[0].specs, l[0].building, l[0].ok), (2, 1, false));
        forget(&file, "/x/a");
        assert_eq!(list(&file).len(), 1);
    }
}
