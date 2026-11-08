# Handoff: Spec Deck (working name)

A desktop app (Linux first, then macOS, then Windows) that opens a Spec Kit repo and shows a command center: where every spec stands, and for one spec its stage, tasks, user stories, review health, decisions, and documents.

This file carries the context of the brainstorm session held on 2026-11-08 on the Linux laptop. Read it before doing anything else.

## Where things stand

| Item | State |
|---|---|
| Spec | `specs/001-command-center/spec.md`. Status: **Clarified**, no open questions. Ready for `/speckit-plan`. |
| Parser prototype | `prototype/extract.py`. Python reference implementation of every parsing rule. |
| Tauri proof of concept | Branch `tauri-poc`: `crates/speckit-core` (Rust parser, identical output to the Python reference on all 104 proj-nava specs) and `app/` (Tauri 2 app). Built and tested on Linux (Omarchy) on 2026-11-08. See "Tauri proof of concept" below. |
| Mockup | Clickable, built from real proj-nava data: https://claude.ai/artifact/Mg1G4gMX7QJQszRVJohze9 (private, in Alex's claude.ai account). Source: `prototype/template.html` + `prototype/build.py`. |
| Review mockup (User Story 10) | https://claude.ai/artifact/SXCybz6EV8dutmHEuKLq18 (private). Source: `prototype/review.html` + `prototype/build_review.py`, filled with PR #217's `spec.md`; threads are sample data. Not yet in the app. |

## Resume on the Mac

1. Initialise Spec Kit in this repo (`specify init --here`, pick the agent you use). Keep the existing `specs/` folder.
2. Create the feature branch the spec names: `git checkout -b 001-command-center`. Spec Kit's scripts expect the branch to match the feature folder.
3. Optional: regenerate the mockup against your local proj-nava checkout:
   ```sh
   python3 -I prototype/extract.py ~/path/to/proj-nava prototype/data.json
   python3 -I prototype/build.py   # writes prototype/mockup.html (open in a browser)
   ```
   `data.json` and the built pages are git-ignored because they contain proj-nava content.
4. Run `/speckit-plan` on the spec.

## Decisions (and why)

| Decision | Why |
|---|---|
| Personal tool first; may be shared with the team later | Keeps v1 free of signing, notarization, and settings. |
| Read-only | The app and the coding agent must never write the same files. "Copy next command" bridges to the workflow. |
| Linux first, then macOS, then Windows; built with Rust + Tauri (see Stack decision) | Human decision, 2026-11-08. The app is built and tested on the Linux laptop today, and Tauri covers all three. |
| Review comments go to the branch's GitHub PR (User Story 10) | No backend: GitHub holds identity, threads, permissions. All access through `gh`; GitLab later as a second adapter. Comment only, because spec and code share one PR. |
| Stage is derived from artifacts, not the `**Status**:` line | The Status line goes stale (33 proj-nava specs say "Draft", most are finished). |
| Exception: no `tasks.md` → trust the Status line | Human decision. Older specs have no tasks file but say "Implemented". The UI marks these stages with an asterisk. |
| Review shows the latest pass by default; earlier passes selectable | `review.md` holds several gates in sequence (pre-implementation ANALYZE passes, then a post-implementation REVIEW). Old findings must not raise alarms. |
| Unlabeled clarification sessions are "Unattributed" | Human decision. |
| "Needs attention" is split into **Waiting on me** and **Unfinished** | Human-confirmed. Waiting on me = only a human can unblock (open markers, latest-pass FAIL or open HIGH/CRITICAL, flagged conflicts); includes Done specs. Unfinished = not fully planned or partially built, with "Stalled" (14 days) and "Rest deferred". |
| Timeline bars run from `**Created**:` to merge (or to today) | proj-nava squash-merges, so per-day commit activity on main is useless. |
| Links between specs come from ticket numbers in `spec.md`, drawn only for the selected bar | 383 references across 76 specs; drawing all is unreadable. |
| Specs on unchecked-out branches are v2 | Read them later with `git ls-tree` / `git show branch:path`. |

## The target format is not stock Spec Kit

proj-nava (the reference repo) extends Spec Kit heavily. The spec's Edge Cases and FRs list every rule; the main traps found so far:

- Extra artifacts: `review.md` (verdicts and findings tables), `deployment.md`, `.specify/feature.json` (current feature, branch, ClickUp link), `.specify/memory/codebase-map.md`. No `.specify/templates/`.
- Folder names start with ClickUp ticket numbers, shared by several folders (`26683-*` has nine). Branch names (`feature/WB-27450-Title-Case`) do not match folder names; read the `**Feature Branch**:` header.
- Task lines: `[x]` or `[X]`, bold IDs (`**T012**`), custom tags (`[DG-1]`), non-phase `##` sections, phases titled `Deferred — …`.
- Clarifications: `Q:` and numbered `Q1:`, `→` and `->`, session labels after `:` / `—` or in parentheses.
- Session classification must match whole words in a fixed order ("lock" inside "blocking" once misfiled dozens of sessions). See FR-031.
- `[NEEDS CLARIFICATION: …]` only counts with a colon and text, and never inside code spans (specs mention the syntax).
- Review verdicts: "Verdict (first pass)" is a pass name, not a PASS. Severity words vary (`major`/`minor` → HIGH/LOW). A finding's Status column decides resolved/open.

`extract.py` handles all of these. When porting, use proj-nava folders as test fixtures and compare against the Python output.

## Visual direction

Dark-first with a light variant. Low-contrast layered surfaces, one violet accent, a single gradient reserved for the feature being worked on now, a small stage-colour stripe on rows and bars, hatching for idle or off time, a white primary button, native window chrome and system type on each platform. Inspired by a dark project-timeline dashboard shot Alex shared (not included here; describe-only).

## Stack decision (2026-11-08): Rust + Tauri, not SwiftUI

Chosen after weighing SwiftUI, Tauri, and GPUI (Zed's framework):

- Tauri runs on macOS and Linux, so it can be built and tested on the Linux laptop too.
- The mockup's HTML/CSS/JS became the app's interface with little change.
- Markdown and Mermaid render natively in the web view (Mermaid was the gap for both SwiftUI and GPUI).
- The Rust core crate can later back a CLI or terminal UI.

Platform order (decided 2026-11-08): Linux first, then macOS, then Windows.

## Tauri proof of concept

Layout:

```
Cargo.toml                  workspace
crates/speckit-core/        parser + model (port of prototype/extract.py), `speckit-dump` CLI
crates/speckit-forge/       review threads on GitHub through `gh` (User Story 10), `forge-status` CLI
app/src-tauri/              Tauri 2 app: commands, file watcher, markdown rendering (comrak)
app/ui/index.html           interface, generated from prototype/template.html by scripts/make_ui.py
app/ui/vendor/              Mermaid 12.2.0, bundled for offline use
scripts/parity.py           diff the Rust and Python parser output
scripts/make_ui.py          turn the mockup template into the app interface
prototype/review-layer.js   review UI (chip, status line, gutters, threads, Conversations, submit); one source for both mockups and the app
prototype/review-layer.css  its styles
prototype/review-mock.js    sample backend for the mockups (never reaches the app)
prototype/review.html       PR-review mockup page; build with prototype/build_review.py
```

Design workflow: change `prototype/template.html`, review it as the mockup (`python3 -I prototype/build.py`, open `prototype/mockup.html`), then regenerate the app interface with `python3 -I scripts/make_ui.py prototype/template.html app/ui/index.html`. Don't edit `app/ui/index.html` by hand; the next regeneration overwrites it. The script stops if a template change breaks one of its anchors.

Run it:

```sh
cargo run -p spec-deck -- ~/path/to/proj-nava          # or launch without a path and use "Open project…"
cargo run -p spec-deck -- --dump-view ~/path/to/repo    # print the JSON the interface receives
cargo run -p spec-deck -- --dump-doc REPO FEATURE spec.md     # rendered document with data-sourcepos
cargo run -p spec-deck -- --dump-review REPO FEATURE [BRANCH] # review status (calls GitHub, read-only)
cargo test                                                # all crates; SC-007 runs over ~/Projects/proj-nava when present
cargo test -p speckit-core
python3 -I prototype/extract.py REPO /tmp/py.json \
  && cargo run -q -p speckit-core --bin speckit-dump REPO /tmp/rs.json \
  && python3 -I scripts/parity.py /tmp/py.json /tmp/rs.json
```

The first build compiles all dependencies (about 10 minutes); later builds take seconds. On macOS: install Rust with rustup, then the same commands. To make a `.app`/`.dmg`: `cargo install tauri-cli`, `cargo tauri icon app/src-tauri/icons/icon.png`, `cargo tauri build`.

What the proof of concept proves:

- Parser parity: Rust output identical to the Python reference for all 104 proj-nava features; 1.4 s for the whole repo (mostly git).
- Interface to Rust calls, folder picker, remembered last project, links opening in the browser, clipboard.
- Live reload: editing a `tasks.md` pushes a new model about 120 ms later. On Linux, file *reads* also raise watch events; they are filtered out, or every reload would trigger another.
- Documents rendered by comrak, Mermaid drawn in the web view.
- Timeline v2 (approved 2026-11-08): fixed label column, grouping by ticket or stage, true-length bars with tasks-done fill and idle hatching, lifecycle milestones (created, clarify sessions by who decided, analyze and review passes, merge), dependency arrows for the selected spec, lead-time summary.
- Review on GitHub (User Story 10, plan and tasks in `specs/001-command-center/`): readiness checks, threads in place, comment on block line ranges, pending review submitted as Comment only, reply, resolve, Copy for agent, "Open review threads" view. Checked read-only against proj-nava PRs #217, #203, #171 and in headless Chromium; writes are unit-tested but not yet run against a real PR.
- Task and user-story detail modals: full task text, files (open with the default app), linked story, review findings that mention the task or story; story description, why-this-priority, independent test, Given/When/Then scenarios, tasks. Arrow keys step through, Esc closes.

Known gaps: no recent-projects list yet (only the last one); no specs from other branches (v2); the window title does not follow the view; git dates are re-read on every reload (could be cached until `.git/logs/HEAD` changes); unsigned and unbundled. The interface was checked in headless Chromium with the real data and on the live app through its logs; it has not yet been looked at on screen in WebKitGTK.
