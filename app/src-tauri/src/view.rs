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
                "tasks": p.tasks.iter().map(|t| json!([t.id, t.done as u8, t.parallel as u8, t.story, t.text])).collect::<Vec<_>>(),
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
        "stories": spec.map(|s| s.stories.iter().map(|st| json!([st.n, st.title, st.priority])).collect::<Vec<_>>()).unwrap_or_default(),
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
