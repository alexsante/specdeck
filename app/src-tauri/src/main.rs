// Spec Deck: a command center for Spec Kit projects.
//
// The app never writes to the project it opens, with one exception you trigger yourself:
// "Pull now" runs `git pull --ff-only` (see `git_pull`). It reads the project with
// speckit-core, watches specs/ and .specify/ in every worktree for changes, and pushes a
// fresh model to the interface as a "project-updated" event (the viewed worktree changed)
// or "worktrees-updated" (only another worktree changed).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod recent;
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
    /// The worktree being viewed: documents, pull, and review act here.
    repo: Mutex<Option<PathBuf>>,
    /// The folder the user opened (FR-071); the view can switch to its other worktrees (FR-072).
    opened: Mutex<Option<PathBuf>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    /// GitHub through the gh CLI (User Story 10).
    forge: speckit_forge::github::GitHub,
    /// Review status per feature; cleared when files change on disk.
    review: Mutex<std::collections::HashMap<String, review::Cached>>,
    /// A `git pull` is running (one at a time).
    pulling: std::sync::atomic::AtomicBool,
}

/// The worktrees of the project, compared against the viewed one (FR-073 to FR-075).
fn worktrees_view(viewed: &Path, opened: &Path) -> Value {
    let started = std::time::Instant::now();
    let snaps = speckit_core::worktree::snapshots(viewed);
    eprintln!("[spec-deck] read {} worktrees in {:?}", snaps.len(), started.elapsed());
    view::worktrees(&snaps, viewed, opened)
}

/// Read the viewed worktree. The project keeps the name of the folder the user opened.
fn load(repo: &Path, opened: &Path) -> Result<Value, String> {
    let started = std::time::Instant::now();
    let p = speckit_core::read_project(repo).map_err(|e| e.to_string())?;
    eprintln!("[spec-deck] read {} features from {} in {:?}", p.features.len(), repo.display(), started.elapsed());
    let mut v = view::project(&p, repo);
    if let Some(n) = opened.file_name() {
        v["project"] = n.to_string_lossy().into_owned().into();
    }
    v["opened"] = opened.display().to_string().into();
    v["worktrees"] = worktrees_view(repo, opened);
    // The GitHub repo, for branch links (none for other hosts).
    let origin = std::process::Command::new("git").args(["remote", "get-url", "origin"]).current_dir(repo).output().ok();
    let slug = origin.filter(|o| o.status.success()).and_then(|o| speckit_forge::local::github_slug(&String::from_utf8_lossy(&o.stdout)));
    v["remote"] = match slug {
        Some(s) => serde_json::json!({ "slug": s, "web": format!("https://github.com/{s}") }),
        None => Value::Null,
    };
    Ok(v)
}

fn last_project_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("last-project.txt"))
}

fn recents_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("recent-projects.json"))
}

/// Recent projects, newest first. The old single last-project file seeds the list once.
fn recents(app: &AppHandle) -> Vec<recent::Recent> {
    let Some(file) = recents_file(app) else { return vec![] };
    if !file.exists() {
        if let Some(p) = last_project_file(app).and_then(|f| std::fs::read_to_string(f).ok()) {
            let p = PathBuf::from(p.trim());
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            recent::touch(&file, &p, &serde_json::json!({ "project": name }));
        }
    }
    recent::list(&file)
}

/// The HEAD reflog of a worktree: commits and branch switches append to it (HEAD itself
/// is replaced by rename, which ends a file watch on Linux). Linked worktrees keep it
/// under the main repo's `.git/worktrees/<name>/`.
fn head_reflog(worktree: &Path) -> Option<PathBuf> {
    let dir = speckit_core::git(worktree, &["rev-parse", "--absolute-git-dir"]);
    let p = PathBuf::from(dir).join("logs").join("HEAD");
    (p.exists()).then_some(p)
}

/// Show `repo`: read it, watch it and its sibling worktrees. `opened` is the folder the
/// user opened; None means `repo` itself is being opened, which also records it as recent.
fn open_repo(app: &AppHandle, repo: PathBuf, opened: Option<PathBuf>) -> Result<Value, String> {
    let repo = std::fs::canonicalize(&repo).map_err(|e| format!("{}: {e}", repo.display()))?;
    let remember = opened.is_none();
    let opened = opened.unwrap_or_else(|| repo.clone());
    let data = load(&repo, &opened)?;
    let state = app.state::<AppState>();
    *state.repo.lock().unwrap() = Some(repo.clone());
    *state.opened.lock().unwrap() = Some(opened.clone());
    state.review.lock().unwrap().clear();
    if remember {
        if let Some(f) = recents_file(app) {
            recent::touch(&f, &repo, &data);
        }
    }

    // Watch for changes. Reads are ignored: on Linux the watcher also reports
    // file opens, and every reload opens the spec files, which would loop.
    // Each event says whether it touched the viewed worktree (full reload) or
    // only another worktree (the worktree summary is enough).
    let roots: Vec<PathBuf> = data["worktrees"]["list"]
        .as_array()
        .map(|l| l.iter().filter(|w| w["prunable"] != true).filter_map(|w| w["path"].as_str()).map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p))).collect())
        .unwrap_or_default();
    let viewed_log = head_reflog(&repo);
    let (tx, rx) = mpsc::channel::<bool>();
    let (cls_roots, cls_repo, cls_log) = (roots.clone(), repo.clone(), viewed_log.clone());
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if !matches!(ev.kind, EventKind::Access(_)) {
                // The deepest worktree root containing the path owns it (worktrees may nest).
                let viewed = ev.paths.iter().any(|p| {
                    cls_log.as_deref() == Some(p.as_path())
                        || cls_roots.iter().filter(|r| p.starts_with(r)).max_by_key(|r| r.as_os_str().len()) == Some(&cls_repo)
                });
                let _ = tx.send(viewed);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    for root in roots.iter().chain(std::iter::once(&repo)) {
        for dir in ["specs", ".specify"] {
            let p = root.join(dir);
            if p.exists() {
                let r = watcher.watch(&p, RecursiveMode::Recursive);
                if root == &repo {
                    r.map_err(|e| e.to_string())?;
                }
            }
        }
    }
    if let Some(log) = &viewed_log {
        let _ = watcher.watch(log, RecursiveMode::NonRecursive);
    }

    // Debounce: after the first change, wait for 300 ms of quiet, then reload
    // once and push the new model. The thread ends when the watcher is dropped.
    let handle = app.clone();
    let watched = repo.clone();
    std::thread::spawn(move || {
        while let Ok(first) = rx.recv() {
            let mut full = first;
            loop {
                match rx.recv_timeout(Duration::from_millis(300)) {
                    Ok(v) => full |= v,
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            let st = handle.state::<AppState>();
            let open = st.repo.lock().unwrap().clone();
            if open.as_deref() != Some(watched.as_path()) {
                return; // another project or worktree is shown now
            }
            if !full {
                let _ = handle.emit("worktrees-updated", worktrees_view(&watched, &opened));
                eprintln!("[spec-deck] change in another worktree: pushed worktrees-updated");
                continue;
            }
            // Line numbers may have moved: the next review call re-checks the files.
            st.review.lock().unwrap().clear();
            if let Ok(data) = load(&watched, &opened) {
                let _ = handle.emit("project-updated", data);
                eprintln!("[spec-deck] change on disk: pushed project-updated");
            }
        }
    });
    *state.watcher.lock().unwrap() = Some(watcher);
    Ok(data)
}

fn opened_repo(state: &AppState) -> Result<PathBuf, String> {
    state.opened.lock().unwrap().clone().or_else(|| state.repo.lock().unwrap().clone()).ok_or_else(|| "No project is open.".into())
}

/// A worktree of the open project, checked against `git worktree list` (never an arbitrary path).
fn project_worktree(state: &AppState, path: &str) -> Result<speckit_core::worktree::Worktree, String> {
    let opened = opened_repo(state)?;
    let want = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    speckit_core::worktree::list_worktrees(&opened)
        .into_iter()
        .find(|w| std::fs::canonicalize(&w.path).unwrap_or_else(|_| PathBuf::from(&w.path)) == want)
        .ok_or_else(|| format!("{} is not a worktree of this project.", recent::display(path)))
}

/// Switch the view to another worktree of the open project (FR-072).
#[tauri::command]
async fn view_worktree(app: AppHandle, path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let w = project_worktree(&state, &path)?;
        if w.prunable {
            return Err(format!("{} no longer exists. `git worktree prune` removes it from git's list.", recent::display(&w.path)));
        }
        if !Path::new(&w.path).join("specs").is_dir() {
            return Err(format!("{} has no specs/ folder.", recent::display(&w.path)));
        }
        let opened = opened_repo(&state)?;
        open_repo(&app, PathBuf::from(&w.path), Some(opened))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The worktree summary alone, for refreshing on window focus (new or removed worktrees).
#[tauri::command]
async fn worktree_status(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        Ok(worktrees_view(&current_repo(&state)?, &opened_repo(&state)?))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Show a worktree's folder in the file manager.
#[tauri::command]
fn reveal_worktree(state: State<AppState>, path: String) -> Result<(), String> {
    let w = project_worktree(&state, &path)?;
    system_open(&w.path, true)
}

fn current_repo(state: &AppState) -> Result<PathBuf, String> {
    state.repo.lock().unwrap().clone().ok_or_else(|| "No project is open.".into())
}

/// The project to open at launch: a path given on the command line, else the most recent one.
fn launch_path(app: &AppHandle) -> Option<PathBuf> {
    std::env::args().nth(1).filter(|a| !a.starts_with("--")).map(PathBuf::from).or_else(|| recents(app).into_iter().find(|r| r.ok).map(|r| PathBuf::from(r.path)))
}

/// What the splash should name while the launch project loads, without reading it yet.
#[tauri::command]
fn launch_target(app: AppHandle, state: State<AppState>) -> Option<recent::Recent> {
    let p = state.opened.lock().unwrap().clone().or_else(|| state.repo.lock().unwrap().clone()).or_else(|| launch_path(&app))?;
    let path = std::fs::canonicalize(&p).unwrap_or(p).display().to_string();
    let known = recents(&app).into_iter().find(|r| r.path == path);
    Some(known.unwrap_or_else(|| recent::Recent {
        name: Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        display: recent::display(&path),
        path,
        ok: true,
        ..Default::default()
    }))
}

/// The project to show at launch (see `launch_path`); None shows the welcome view.
#[tauri::command]
async fn current_project(app: AppHandle) -> Result<Option<Value>, String> {
    eprintln!("[spec-deck] interface asked for the current project");
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(repo) = app.state::<AppState>().repo.lock().unwrap().clone() {
            let opened = opened_repo(&app.state::<AppState>())?;
            return load(&repo, &opened).map(Some);
        }
        match launch_path(&app) {
            Some(p) if p.join("specs").is_dir() => open_repo(&app, p, None).map(Some),
            _ => Ok(None),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    let folder = app.dialog().file().set_title("Open a Spec Kit project").blocking_pick_folder()?;
    folder.into_path().ok().map(|p| p.display().to_string())
}

/// Open a folder picked, dropped, or chosen from the recent list.
#[tauri::command]
async fn open_project_path(app: AppHandle, path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(&path);
        if !p.join("specs").is_dir() {
            return Err(format!("{} has no specs/ folder. Pick the root of a Spec Kit project.", recent::display(&path)));
        }
        open_repo(&app, p, None)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn recent_projects(app: AppHandle) -> Vec<recent::Recent> {
    recents(&app)
}

#[tauri::command]
fn forget_recent(app: AppHandle, path: String) -> Vec<recent::Recent> {
    if let Some(f) = recents_file(&app) {
        recent::forget(&f, &path);
    }
    recents(&app)
}

/// The app's one write to a repo, and only on a click: `git pull --ff-only` (FR-070).
/// The file watcher then reloads the project as usual.
#[tauri::command]
async fn git_pull(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let repo = current_repo(&state)?;
        if state.pulling.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("A pull is already running.".into());
        }
        let runner = speckit_forge::run::Process { timeout: std::time::Duration::from_secs(120) };
        let r = speckit_forge::pull::pull_ff(&runner, &repo);
        state.pulling.store(false, std::sync::atomic::Ordering::SeqCst);
        eprintln!("[spec-deck] git pull in {}: {r:?}", repo.display());
        state.review.lock().unwrap().clear();
        r
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Text size and title bar theme for the window (the page sets its own colours and fonts).
#[tauri::command]
fn set_appearance(app: AppHandle, zoom: f64, theme: String) -> Result<(), String> {
    let w = app.get_webview_window("main").ok_or("No main window.")?;
    w.set_zoom(zoom.clamp(0.8, 1.4)).map_err(|e| e.to_string())?;
    let t = match theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None, // follow the system
    };
    w.set_theme(t).map_err(|e| e.to_string())
}

/// Anything to pull for the open project's branch? Read-only (`git ls-remote`, no fetch).
#[tauri::command]
async fn pull_status(app: AppHandle) -> Result<speckit_forge::pull::PullStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let repo = current_repo(&state)?;
        let t = std::time::Instant::now();
        let s = state.forge.pull_status(&repo);
        eprintln!("[spec-deck] pull status {} in {:?}", s.state, t.elapsed());
        Ok(s)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Is the GitHub CLI installed and signed in? For the welcome view.
#[tauri::command]
async fn gh_ready(app: AppHandle) -> Result<speckit_forge::Ready, String> {
    tauri::async_runtime::spawn_blocking(move || speckit_forge::Forge::ready(&app.state::<AppState>().forge)).await.map_err(|e| e.to_string())
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
        match load(&repo, &repo) {
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
    if args.get(1).map(String::as_str) == Some("--dump-pull") && args.len() >= 3 {
        let repo = std::fs::canonicalize(&args[2]).expect("repo path");
        println!("{}", serde_json::to_string(&speckit_forge::github::GitHub::default().pull_status(&repo)).unwrap());
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
            current_project, launch_target, pick_folder, open_project_path, recent_projects, forget_recent, gh_ready, pull_status, git_pull, set_appearance, read_doc, open_feature, open_path, open_url, copy_text,
            view_worktree, worktree_status, reveal_worktree,
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
