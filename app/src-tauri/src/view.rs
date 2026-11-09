//! Shape the core model into the compact JSON the interface consumes
//! (the same shape `prototype/build.py` produced for the mockup).

use serde_json::{json, Value};
use speckit_core::{Feature, Project};
use std::collections::BTreeMap;

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let cut: String = s.chars().take(n - 1).collect();
    format!("{}…", cut.trim_end())
}

fn feature(f: &Feature) -> Value {
    let spec = f.spec.as_ref();
    let phases: Vec<Value> = f
        .tasks
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|p| {
            let mut by_story: BTreeMap<&str, [usize; 2]> = BTreeMap::new();
            for t in &p.tasks {
                if let Some(s) = t.story.as_deref() {
                    let e = by_story.entry(s).or_default();
                    e[1] += 1;
                    e[0] += t.done as usize;
                }
            }
            json!({
                "title": p.title, "total": p.total, "done": p.done, "deferred": p.deferred,
                "stories": by_story.keys().collect::<Vec<_>>(),
                "byStory": by_story,
                "tasks": p.tasks.iter().map(|t| json!([t.id, t.done as u8, t.parallel as u8, t.story, t.text, t.raw, t.files, t.tags])).collect::<Vec<_>>(),
            })
        })
        .collect();
    let next = f.tasks.as_deref().unwrap_or(&[]).iter().flat_map(|p| &p.tasks).find(|t| !t.done);
    let checklists = [f.checklists.iter().map(|c| c.done).sum::<usize>(), f.checklists.iter().map(|c| c.total).sum::<usize>()];

    json!({
        "id": f.id, "ticket": f.ticket, "slug": f.slug, "stage": f.stage, "stageSource": f.stage_source,
        "files": f.files, "lastCommit": f.last_commit, "firstCommit": f.first_commit, "refs": f.refs,
        "contracts": f.contracts.len(),
        "title": spec.map(|s| s.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| f.slug.replace('-', " ")),
        "status": spec.map(|s| s.status.clone()),
        "branch": spec.and_then(|s| s.branch.clone()),
        "created": spec.and_then(|s| s.created.clone()),
        "clickup": spec.and_then(|s| s.clickup.clone()),
        "input": spec.map(|s| clip(&s.input, 260)),
        "fr": spec.map_or(0, |s| s.fr), "nfr": spec.map_or(0, |s| s.nfr), "sc": spec.map_or(0, |s| s.sc),
        "markers": spec.map(|s| s.markers.clone()).unwrap_or_default(),
        "stories": spec.map(|s| s.stories.iter().map(|st| json!([st.n, st.title, st.priority, st.body])).collect::<Vec<_>>()).unwrap_or_default(),
        "sessions": spec.map(|s| s.sessions.iter().map(|se| json!({
            "date": se.date, "label": clip(&se.label, 90), "kind": se.kind, "n": se.qa.len(),
            "qa": se.qa.iter().map(|q| json!([q.q, q.a])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "phases": phases,
        "next": next.map(|t| json!([t.id, t.text])),
        "review": f.review.as_ref().map(|r| json!({
            "verdict": r.verdict, "title": clip(&r.title, 100), "passes": r.passes,
        })),
        "checklists": checklists,
    })
}

pub fn project(p: &Project, path: &std::path::Path) -> Value {
    json!({
        "project": p.project, "path": path.display().to_string(), "generated": p.generated, "head": p.head,
        "current": p.current, "stages": p.stages,
        "features": p.features.iter().map(feature).collect::<Vec<_>>(),
    })
}

/// Worktrees of the project (FR-071 to FR-075), shaped for the interface:
/// - `list`: every worktree, main first, with its specs that are new or changed against the main worktree;
/// - `copies`: per spec in the viewed worktree, the other worktrees whose copy differs from it;
/// - `elsewhere`: specs the viewed worktree doesn't have, newest worktree activity first;
/// - `uncommitted`: per spec, the viewed worktree's uncommitted files;
/// - `doneEdited`: per spec, worktrees whose copy is Done and has uncommitted edits (FR-074).
pub fn worktrees(snaps: &[speckit_core::worktree::Snapshot], viewed: &std::path::Path, opened: &std::path::Path) -> Value {
    use speckit_core::worktree::{Snapshot, SpecCopy};
    use std::collections::HashMap;
    fn by_id(s: &Snapshot) -> HashMap<&str, &SpecCopy> {
        s.specs.iter().map(|c| (c.id.as_str(), c)).collect()
    }
    let canon = |p: &std::path::Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (viewed, opened) = (canon(viewed), canon(opened));
    let is = |s: &Snapshot, p: &std::path::Path| canon(std::path::Path::new(&s.worktree.path)) == p;
    let name = |s: &Snapshot| std::path::Path::new(&s.worktree.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let copy = |c: &SpecCopy| json!({ "id": c.id, "ticket": c.ticket, "title": c.title, "stage": c.stage, "done": c.done, "total": c.total, "uncommitted": c.uncommitted });

    let main = snaps.iter().find(|s| s.worktree.main).map(by_id).unwrap_or_default();
    let view = snaps.iter().find(|s| is(s, &viewed));
    let view_ids = view.map(by_id).unwrap_or_default();

    let list: Vec<Value> = snaps
        .iter()
        .map(|s| {
            let own = by_id(s);
            let changes: Vec<Value> = if s.worktree.main {
                vec![]
            } else {
                s.specs
                    .iter()
                    .filter_map(|c| {
                        let kind = match main.get(c.id.as_str()) {
                            None => "new",
                            Some(m) if m.fingerprint != c.fingerprint => "changed",
                            _ => return None,
                        };
                        let mut v = copy(c);
                        v["kind"] = kind.into();
                        Some(v)
                    })
                    .collect()
            };
            let current = s.current.as_ref().and_then(|cur| {
                let id = cur.get("feature")?.as_str()?;
                let c = own.get(id)?;
                Some(json!({ "feature": id, "title": c.title, "stage": c.stage, "done": c.done, "total": c.total, "branch": cur.get("branch") }))
            });
            json!({
                "path": s.worktree.path, "display": crate::recent::display(&s.worktree.path), "name": name(s),
                "branch": s.worktree.branch, "head": s.worktree.head.get(..7).unwrap_or(&s.worktree.head),
                "main": s.worktree.main, "prunable": s.worktree.prunable,
                "opened": is(s, &opened), "viewing": is(s, &viewed),
                "dirty": s.dirty, "ahead": s.ahead, "behind": s.behind, "activity": s.activity,
                "specs": s.specs.len(), "current": current, "changes": changes,
            })
        })
        .collect();

    let mut copies: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    let mut elsewhere: Vec<(Option<&String>, Value)> = vec![];
    let mut done_edited: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut order: Vec<&Snapshot> = snaps.iter().collect();
    order.sort_by(|a, b| b.activity.cmp(&a.activity));
    for s in order {
        for c in &s.specs {
            if c.stage == "Done" && !c.uncommitted.is_empty() {
                done_edited.entry(&c.id).or_default().push(name(s));
            }
            if is(s, &viewed) {
                continue;
            }
            match view_ids.get(c.id.as_str()) {
                Some(v) if v.fingerprint != c.fingerprint => {
                    // Identical copies in several worktrees make one entry, shown as the main worktree's when it has one.
                    let list = copies.entry(&c.id).or_default();
                    let entry = json!({ "path": s.worktree.path, "name": name(s), "main": s.worktree.main, "fingerprint": c.fingerprint,
                        "stage": c.stage, "done": c.done, "total": c.total, "uncommitted": !c.uncommitted.is_empty(), "alsoIn": [] });
                    match list.iter_mut().find(|e| e["fingerprint"] == c.fingerprint.as_str()) {
                        Some(e) if s.worktree.main => {
                            let mut also = e["alsoIn"].as_array().cloned().unwrap_or_default();
                            also.push(e["name"].clone());
                            *e = entry;
                            e["alsoIn"] = also.into();
                        }
                        Some(e) => e["alsoIn"].as_array_mut().unwrap().push(name(s).into()),
                        None => list.push(entry),
                    }
                }
                None if seen.insert(c.id.as_str()) => {
                    let mut v = copy(c);
                    v["path"] = s.worktree.path.clone().into();
                    v["name"] = name(s).into();
                    elsewhere.push((s.activity.as_ref(), v));
                }
                _ => {}
            }
        }
    }
    let uncommitted: BTreeMap<&str, &Vec<String>> = view.map(|v| v.specs.iter().filter(|c| !c.uncommitted.is_empty()).map(|c| (c.id.as_str(), &c.uncommitted)).collect()).unwrap_or_default();
    json!({
        "list": list, "copies": copies, "elsewhere": elsewhere.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        "uncommitted": uncommitted, "doneEdited": done_edited,
    })
}
