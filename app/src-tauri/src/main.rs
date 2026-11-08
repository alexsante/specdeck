// Spec Deck: a read-only command center for Spec Kit projects.
//
// The app never writes to the project it opens. It reads the project with
// speckit-core, watches specs/ and .specify/ for changes, and pushes a fresh
// model to the interface as a "project-updated" event.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod review;
mod view;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct AppState {
    repo: Mutex<Option<PathBuf>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    /// GitHub through the gh CLI (User Story 10).
    forge: speckit_forge::github::GitHub,
    /// Review status per feature; cleared when files change on disk.
    review: Mutex<std::collections::HashMap<String, review::Cached>>,
}

fn load(repo: &Path) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let p = speckit_core::read_project(repo).map_err(|e| e.to_string())?;
    eprintln!("[spec-deck] read {} features from {} in {:?}", p.features.len(), repo.display(), started.elapsed());
    Ok(view::project(&p, repo))
}

fn last_project_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("last-project.txt"))
}

/// Make `repo` the open project: read it, remember it, and watch it.
fn open_repo(app: &AppHandle, repo: PathBuf) -> Result<Value, String> {
    let repo = std::fs::canonicalize(&repo).map_err(|e| format!("{}: {e}", repo.display()))?;
    let data = load(&repo)?;
    let state = app.state::<AppState>();
    *state.repo.lock().unwrap() = Some(repo.clone());
    state.review.lock().unwrap().clear();
    if let Some(f) = last_project_file(app) {
        let _ = std::fs::create_dir_all(f.parent().unwrap());
        let _ = std::fs::write(f, repo.display().to_string());
    }

    // Watch for changes. Reads are ignored: on Linux the watcher also reports
    // file opens, and every reload opens the spec files, which would loop.
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if !matches!(ev.kind, EventKind::Access(_)) {
                let _ = tx.send(());
            }
        }
    })
    .map_err(|e| e.to_string())?;
    for dir in ["specs", ".specify"] {
        let p = repo.join(dir);
        if p.exists() {
            watcher.watch(&p, RecursiveMode::Recursive).map_err(|e| e.to_string())?;
        }
    }
    // Commits and branch switches append to the HEAD reflog (HEAD itself is
    // replaced by rename, which ends a file watch on Linux).
    let head = repo.join(".git").join("logs").join("HEAD");
    if head.exists() {
        let _ = watcher.watch(&head, RecursiveMode::NonRecursive);
    }

    // Debounce: after the first change, wait for 300 ms of quiet, then reload
    // once and push the new model. The thread ends when the watcher is dropped.
    let handle = app.clone();
    let watched = repo.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(Duration::from_millis(300)) {
                    Ok(()) => continue,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            let open = handle.state::<AppState>().repo.lock().unwrap().clone();
            if open.as_deref() != Some(watched.as_path()) {
                return; // another project was opened
            }
            // Line numbers may have moved: the next review call re-checks the files.
            handle.state::<AppState>().review.lock().unwrap().clear();
            if let Ok(data) = load(&watched) {
                let _ = handle.emit("project-updated", data);
                eprintln!("[spec-deck] change on disk: pushed project-updated");
            }
        }
    });
    *state.watcher.lock().unwrap() = Some(watcher);
    Ok(data)
}

fn current_repo(state: &AppState) -> Result<PathBuf, String> {
    state.repo.lock().unwrap().clone().ok_or_else(|| "No project is open.".into())
}

/// The project to show at launch: a path given on the command line, else the last one opened.
#[tauri::command]
fn current_project(app: AppHandle, state: State<AppState>) -> Result<Option<Value>, String> {
    eprintln!("[spec-deck] interface asked for the current project");
    if let Some(repo) = state.repo.lock().unwrap().clone() {
        return load(&repo).map(Some);
    }
    let arg = std::env::args().nth(1).map(PathBuf::from);
    let last = last_project_file(&app).and_then(|f| std::fs::read_to_string(f).ok()).map(|s| PathBuf::from(s.trim()));
    match arg.or(last) {
        Some(p) if p.join("specs").is_dir() => open_repo(&app, p).map(Some),
        _ => Ok(None),
    }
}

#[tauri::command]
async fn pick_project(app: AppHandle) -> Result<Option<Value>, String> {
    let Some(folder) = app.dialog().file().set_title("Open a Spec Kit project").blocking_pick_folder() else {
        return Ok(None); // cancelled
    };
    let path = folder.into_path().map_err(|e| e.to_string())?;
    if !path.join("specs").is_dir() {
        return Err(format!("{} has no specs/ folder. Pick the root of a Spec Kit project.", path.display()));
    }
    open_repo(&app, path).map(Some)
}

/// Resolve `specs/<feature>/<file>` and refuse anything outside specs/.
fn feature_path(repo: &Path, feature: &str, file: Option<&str>) -> Result<PathBuf, String> {
    let specs = std::fs::canonicalize(repo.join("specs")).map_err(|e| e.to_string())?;
    let mut p = specs.join(feature);
    if let Some(f) = file {
        p = p.join(f);
    }
    let p = std::fs::canonicalize(&p).map_err(|_| format!("{feature}/{} not found", file.unwrap_or("")))?;
    if !p.starts_with(&specs) {
        return Err("Path is outside specs/.".into());
    }
    Ok(p)
}

fn md_options() -> comrak::Options<'static> {
    let mut o = comrak::Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.autolink = true;
    o.extension.footnotes = true;
    // Raw HTML in the markdown stays escaped.
    o
}

/// Parse markdown, pulling each block's end back to its last non-blank line
/// (comrak lets a list's last item run over the blank line after it).
pub fn parse_md<'a>(arena: &'a comrak::Arena<'a>, md: &str, o: &comrak::Options) -> &'a comrak::nodes::AstNode<'a> {
    let root = comrak::parse_document(arena, md, o);
    let lines: Vec<&str> = md.lines().collect();
    let blank = |n: usize| lines.get(n.wrapping_sub(1)).map_or(true, |l| l.trim().is_empty());
    for n in root.descendants() {
        let mut d = n.data.borrow_mut();
        if d.value.block() {
            while d.sourcepos.end.line > d.sourcepos.start.line && blank(d.sourcepos.end.line) {
                d.sourcepos.end.line -= 1;
                d.sourcepos.end.column = lines.get(d.sourcepos.end.line - 1).map_or(1, |l| l.len().max(1));
            }
        }
    }
    root
}

/// Render markdown. With `sourcepos`, block elements carry `data-sourcepos="l:c-l:c"`,
/// which the interface uses to put comments on the right source lines (FR-062).
pub fn markdown_html(md: &str, sourcepos: bool) -> String {
    let mut o = md_options();
    o.render.sourcepos = sourcepos;
    let arena = comrak::Arena::new();
    let root = parse_md(&arena, md, &o);
    let mut out = String::new();
    let _ = comrak::format_html(root, &o, &mut out);
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn file_html(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return format!("<p class=\"dim\">{} is not a text file.</p>", esc(&name));
    };
    if name.ends_with(".md") {
        markdown_html(&text, true)
    } else {
        let lang = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        format!("<pre><code class=\"language-{}\">{}</code></pre>", esc(&lang), esc(&text))
    }
}

/// Render one artifact as HTML. A folder (contracts/, checklists/) renders each file in turn.
#[tauri::command]
fn read_doc(state: State<AppState>, feature: String, file: String) -> Result<String, String> {
    eprintln!("[spec-deck] read_doc {feature}/{file}");
    render_doc(&current_repo(&state)?, &feature, &file)
}

fn render_doc(repo: &Path, feature: &str, file: &str) -> Result<String, String> {
    let p = feature_path(repo, feature, Some(file))?;
    if !p.is_dir() {
        return Ok(file_html(&p));
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&p).map_err(|e| e.to_string())?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    let mut html = String::new();
    for e in entries.iter().filter(|e| e.is_file()) {
        let name = e.file_name().unwrap().to_string_lossy();
        html.push_str(&format!("<section class=\"docfile\" data-path=\"{0}/{1}\"><div class=\"docfile-name\">{0}/{1}</div>{2}</section>", esc(&file), esc(&name), file_html(e)));
    }
    Ok(if html.is_empty() { "<p class=\"dim\">This folder is empty.</p>".into() } else { html })
}

fn system_open(target: &str, reveal: bool) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        if reveal {
            c.arg("-R");
        }
        c
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer");
        if reveal {
            c.arg("/select,");
        }
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(target).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open a feature folder in the file manager (`reveal`) or with the default app.
#[tauri::command]
fn open_feature(state: State<AppState>, feature: String, reveal: bool) -> Result<(), String> {
    let repo = current_repo(&state)?;
    let p = feature_path(&repo, &feature, None)?;
    system_open(&p.display().to_string(), reveal)
}

/// Open a repo-relative path (a task's file, a spec's tasks.md) with the default app.
/// Paths outside the open repo are refused; missing files get a clear message.
#[tauri::command]
fn open_path(state: State<AppState>, path: String) -> Result<(), String> {
    let repo = std::fs::canonicalize(current_repo(&state)?).map_err(|e| e.to_string())?;
    let p = std::fs::canonicalize(repo.join(&path)).map_err(|_| format!("{path} is not in this checkout."))?;
    if !p.starts_with(&repo) {
        return Err("That path is outside the project.".into());
    }
    system_open(&p.display().to_string(), false)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened.".into());
    }
    system_open(&url, false)
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

fn main() {
    // `spec-deck --dump-view <repo>` prints the JSON the interface receives, without opening a window.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--dump-view") {
        let repo = std::fs::canonicalize(args.get(2).map(String::as_str).unwrap_or(".")).expect("repo path");
        match load(&repo) {
            Ok(v) => println!("{v}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // `--dump-doc <repo> <feature> <file>` and `--dump-review <repo> <feature> [branch]`: debugging aids.
    if args.get(1).map(String::as_str) == Some("--dump-doc") && args.len() >= 5 {
        let repo = std::fs::canonicalize(&args[2]).expect("repo path");
        match render_doc(&repo, &args[3], &args[4]) {
            Ok(h) => println!("{h}"),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--dump-review") && args.len() >= 4 {
        let repo = std::fs::canonicalize(&args[2]).expect("repo path");
        let st = speckit_forge::Forge::status(&speckit_forge::github::GitHub::default(), &repo, &args[3], args.get(4).map(String::as_str));
        println!("{}", review::to_view(&st));
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            current_project, pick_project, read_doc, open_feature, open_path, open_url, copy_text,
            review::review_status, review::review_counts, review::review_comment, review::review_submit,
            review::review_discard, review::review_reply, review::review_resolve, review::review_edit, review::review_delete
        ])
        .run(tauri::generate_context!())
        .expect("error while running Spec Deck");
}

#[cfg(test)]
mod sourcepos {
    //! SC-007: every commentable block maps to exactly its source lines.
    use comrak::nodes::NodeValue;

    /// The blocks the interface makes commentable: top-level blocks, and each item of a top-level list.
    fn blocks(md: &str) -> Vec<(usize, usize)> {
        let arena = comrak::Arena::new();
        let root = super::parse_md(&arena, md, &super::md_options());
        let mut out = Vec::new();
        for n in root.children() {
            let d = n.data.borrow();
            if matches!(d.value, NodeValue::List(_)) {
                for it in n.children() {
                    let p = it.data.borrow().sourcepos;
                    out.push((p.start.line, p.end.line));
                }
            } else {
                out.push((d.sourcepos.start.line, d.sourcepos.end.line));
            }
        }
        out
    }

    fn check(md: &str) -> Result<(), String> {
        let lines: Vec<&str> = md.lines().collect();
        let blank = |n: usize| lines.get(n - 1).map_or(true, |l| l.trim().is_empty());
        let bs = blocks(md);
        let mut covered = vec![false; lines.len() + 1];
        let mut prev_end = 0;
        for &(a, b) in &bs {
            if a > b || a <= prev_end {
                return Err(format!("block {a}-{b} overlaps or is reversed (previous ended at {prev_end})"));
            }
            if blank(a) || blank(b) {
                return Err(format!("block {a}-{b} starts or ends on a blank line"));
            }
            for c in covered.iter_mut().take(b.min(lines.len()) + 1).skip(a) {
                *c = true;
            }
            prev_end = b;
        }
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim();
            // Link reference definitions produce no node.
            let refdef = t.starts_with('[') && t.contains("]:");
            if !t.is_empty() && !covered[i + 1] && !refdef {
                return Err(format!("line {} is in no block: {t:.60}", i + 1));
            }
        }
        Ok(())
    }

    #[test]
    fn small_cases() {
        check("# T\n\npara\nwraps\n\n- a\n- b\n\n  more b\n\n| x |\n|---|\n| 1 |\n\n```\ncode\n```\n").unwrap();
        assert_eq!(blocks("# T\n\n- a\n- b\n"), vec![(1, 1), (3, 3), (4, 4)]);
    }

    #[test]
    fn proj_nava() {
        let Some(home) = std::env::var_os("HOME") else { return };
        let specs = std::path::Path::new(&home).join("Projects/proj-nava/specs");
        let Ok(dirs) = std::fs::read_dir(&specs) else {
            eprintln!("proj-nava not found; skipped");
            return;
        };
        let (mut files, mut nblocks, mut bad) = (0, 0, Vec::new());
        for d in dirs.flatten() {
            for name in ["spec.md", "plan.md", "tasks.md"] {
                let Ok(md) = std::fs::read_to_string(d.path().join(name)) else { continue };
                files += 1;
                nblocks += blocks(&md).len();
                if let Err(e) = check(&md) {
                    bad.push(format!("{}/{name}: {e}", d.file_name().to_string_lossy()));
                }
            }
        }
        eprintln!("SC-007: {files} files, {nblocks} blocks, {} problems", bad.len());
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
