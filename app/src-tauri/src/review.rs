//! Review threads (User Story 10): Tauri commands over the forge adapter.
//!
//! Nothing here touches the project on disk except to read it; writes go to GitHub.

use crate::{current_repo, markdown_html, AppState};
use serde_json::{json, Value};
use speckit_forge::{position, Forge, Status};
use std::time::{Duration, Instant};
use tauri::State;

const FRESH: Duration = Duration::from_secs(60);

pub struct Cached {
    at: Instant,
    hint: Option<String>,
    status: Status,
}

/// Remove `<!-- … -->` (bots put run ids there) before rendering a comment.
fn strip_html_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("<!--") {
        out.push_str(&rest[..i]);
        match rest[i..].find("-->") {
            Some(j) => rest = &rest[i + j + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// The status as the interface consumes it: comment bodies also rendered as HTML.
pub fn to_view(st: &Status) -> Value {
    let mut v = serde_json::to_value(st).unwrap_or(Value::Null);
    if let Some(ts) = v["threads"].as_array_mut() {
        for t in ts {
            if let Some(cs) = t["comments"].as_array_mut() {
                for c in cs {
                    let body = strip_html_comments(c["body"].as_str().unwrap_or_default());
                    c["html"] = json!(markdown_html(&body, false));
                    c["text"] = json!(body);
                }
            }
        }
    }
    v
}

fn compute(state: &AppState, feature: &str, hint: Option<String>) -> Result<Status, String> {
    let repo = current_repo(state)?;
    let t = Instant::now();
    let st = state.forge.status(&repo, feature, hint.as_deref());
    eprintln!("[spec-deck] review status {feature} in {:?}: {:?}", t.elapsed(), st.message);
    state.review.lock().unwrap().insert(feature.to_string(), Cached { at: Instant::now(), hint, status: st.clone() });
    Ok(st)
}

/// Cached status when fresh; the watcher clears the cache when files change.
fn cached(state: &AppState, feature: &str) -> Result<Status, String> {
    let hit = state.review.lock().unwrap().get(feature).map(|c| (c.at.elapsed() < FRESH, c.hint.clone(), c.status.clone()));
    match hit {
        Some((true, _, st)) => Ok(st),
        Some((false, hint, _)) => compute(state, feature, hint),
        None => compute(state, feature, None),
    }
}

fn hint_of(state: &AppState, feature: &str) -> Option<String> {
    state.review.lock().unwrap().get(feature).and_then(|c| c.hint.clone())
}

/// Run a write after checking it is still allowed, then return the fresh status.
fn write(state: &AppState, feature: &str, f: impl FnOnce(&Status) -> Result<(), String>) -> Result<Value, String> {
    let st = cached(state, feature)?;
    if let Some(m) = st.checks.iter().find(|c| !c.ok) {
        return Err(m.message.clone());
    }
    f(&st)?;
    let hint = hint_of(state, feature);
    compute(state, feature, hint).map(|s| to_view(&s))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

macro_rules! with_state {
    ($app:expr, |$s:ident| $body:expr) => {{
        let app = $app.clone();
        blocking(move || {
            let st: State<AppState> = tauri::Manager::state(&app);
            let $s: &AppState = &st;
            $body
        })
        .await
    }};
}

#[tauri::command]
pub async fn review_status(app: tauri::AppHandle, feature: String, branch: Option<String>, force: bool) -> Result<Value, String> {
    with_state!(app, |s| {
        if !force {
            let hit = s.review.lock().unwrap().get(&feature).filter(|c| c.at.elapsed() < FRESH).map(|c| c.status.clone());
            if let Some(st) = hit {
                return Ok(to_view(&st));
            }
        }
        compute(s, &feature, branch).map(|st| to_view(&st))
    })
}

#[tauri::command]
pub async fn review_counts(app: tauri::AppHandle) -> Result<Value, String> {
    with_state!(app, |s| {
        let repo = current_repo(s)?;
        s.forge.open_threads_by_feature(&repo).map(|m| json!(m))
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn review_comment(app: tauri::AppHandle, feature: String, file: String, start: u32, end: u32, body: String, pending: bool) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |st| {
        let pos = position(st, &file, start, end)?;
        let cr = st.cr.as_ref().ok_or("No pull request.")?;
        if !pending && cr.pending_review.is_some() {
            return Err("You have a pending review. Add this comment to it, or submit the review first.".into());
        }
        s.forge.add_comment(cr, &pos, &body, pending)
    }))
}

#[tauri::command]
pub async fn review_submit(app: tauri::AppHandle, feature: String, body: String) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |st| s.forge.submit_review(st.cr.as_ref().ok_or("No pull request.")?, &body)))
}

#[tauri::command]
pub async fn review_discard(app: tauri::AppHandle, feature: String) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |st| s.forge.discard_review(st.cr.as_ref().ok_or("No pull request.")?)))
}

#[tauri::command]
pub async fn review_reply(app: tauri::AppHandle, feature: String, thread: String, body: String) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |st| s.forge.reply(st.cr.as_ref().ok_or("No pull request.")?, &thread, &body)))
}

#[tauri::command]
pub async fn review_resolve(app: tauri::AppHandle, feature: String, thread: String, resolved: bool) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |_| s.forge.set_resolved(&thread, resolved)))
}

#[tauri::command]
pub async fn review_edit(app: tauri::AppHandle, feature: String, comment: String, body: String) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |_| s.forge.edit_comment(&comment, &body)))
}

#[tauri::command]
pub async fn review_delete(app: tauri::AppHandle, feature: String, comment: String) -> Result<Value, String> {
    with_state!(app, |s| write(s, &feature, |_| s.forge.delete_comment(&comment)))
}

#[cfg(test)]
mod tests {
    use super::strip_html_comments;
    #[test]
    fn strips() {
        assert_eq!(strip_html_comments("<!-- id: 1 -->\n## Bug\ntext <!-- x --> end"), "## Bug\ntext  end");
        assert_eq!(strip_html_comments("a <!-- open"), "a");
    }
}
