# Handoff: Spec Deck (working name)

A read-only macOS app that opens a Spec Kit repo and shows a command center: where every spec stands, and for one spec its stage, tasks, user stories, review health, decisions, and documents.

This file carries the context of the brainstorm session held on 2026-11-08 on the Linux laptop. Read it before doing anything else.

## Where things stand

| Item | State |
|---|---|
| Spec | `specs/001-command-center/spec.md`. Status: **Clarified**, no open questions. Ready for `/speckit-plan`. |
| Parser prototype | `prototype/extract.py`. Python reference implementation of every parsing rule. The Swift parser should port it and match its output. |
| Mockup | Clickable, built from real proj-nava data: https://claude.ai/artifact/Mg1G4gMX7QJQszRVJohze9 (private, in Alex's claude.ai account). Source: `prototype/template.html` + `prototype/build.py`. |
| Swift code | None yet. |

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
| Mac only, native | It is a tool kept open next to the agent. |
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

Dark-first with a light variant. Low-contrast layered surfaces, one violet accent, a single gradient reserved for the feature being worked on now, a small stage-colour stripe on rows and bars, hatching for idle or off time, a white primary button, native macOS chrome and system type. Inspired by a dark project-timeline dashboard shot Alex shared (not included here; describe-only).

## Suggested plan shape (input for `/speckit-plan`, not decided)

- `SpecKitCore`: a Swift package with no UI. Parsers (spec, tasks, review, checklists), the stage and attention rules, git reads (shell out to `git`, off the main thread), and an FSEvents watcher with debounce. Fixture tests built from trimmed proj-nava folders.
- `SpecDeck.app`: SwiftUI `NavigationSplitView` (projects, then views or specs, then detail). Markdown and Mermaid documents render in a `WKWebView`.
- Keep the last good parse when a file is mid-write (User Story 3).
