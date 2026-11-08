# Implementation Plan: User Story 10 · Discuss a spec on its pull request

**Branch**: `tauri-poc` | **Date**: 2026-11-08 | **Spec**: [spec.md](./spec.md) (User Story 10, FR-060 to FR-067, SC-007)

User Stories 1 to 9 are built as the Tauri proof of concept (see `HANDOFF.md`). This plan covers the increment that adds GitHub review threads.

## Summary

Spec Deck shows a spec's GitHub pull request review threads in place in the rendered documents, and lets me comment, reply, resolve, and submit a Comment review without leaving the app. All GitHub access goes through the `gh` CLI behind a forge adapter, so the app holds no token and has no backend, and a GitLab adapter can be added later. The repo on disk is never written.

## Technical Context

**Language/Version**: Rust 2021 (workspace), JavaScript in the Tauri web view (no bundler)
**Primary Dependencies**: Tauri 2, comrak (markdown, now with source positions), serde_json, the `gh` CLI at runtime (not linked)
**Storage**: None. GitHub holds threads; the app keeps a short in-memory cache per pull request.
**Testing**: `cargo test` (adapter tested against a fake `gh` runner; diff-hunk and source-position tests), parity script for the parser, headless Chromium for the interface
**Target Platform**: Linux first, then macOS, then Windows
**Project Type**: desktop app (Rust core crates + Tauri shell + HTML interface)
**Performance Goals**: readiness and threads for one spec in under 1.5 s on a normal connection (measured 0.66 s for the combined query on PR #217); the documents never wait for GitHub
**Constraints**: read-only toward the repo (FR-002); no token handling (FR-061); review event is always COMMENT (FR-063)
**Scale/Scope**: proj-nava: 104 features, PRs of 100+ files (PR #217: 121), up to ~10 spec threads per PR seen

## Constitution Check

This repo has no `.specify/memory/constitution.md` yet. The project rules that act as one (CLAUDE.md, HANDOFF.md decisions) are checked instead:

| Rule | Status |
|---|---|
| Read-only toward the repo | Pass. Writes go to GitHub through `gh`; no file under the project is created or changed. "Copy for agent" goes to the clipboard. |
| Parser parity with the Python reference | Unaffected. The forge code is a new crate; speckit-core's output does not change. |
| Interface generated from the mockup | Pass. The review layer is one shared source (`prototype/review-layer.js`) inlined into both mockups and into the app by the build scripts. |
| No proj-nava content committed | Pass. Tests that read proj-nava skip when it is absent and commit no fixtures from it. |

## Research decisions

| Decision | Chosen | Rejected and why |
|---|---|---|
| How to call GitHub | `gh api` and `gh api graphql --input -` (JSON on stdin), run as a child process with a timeout | octocrab/reqwest: would need the token (FR-061). `gh pr` subcommands: their JSON omits thread positions and they cap file lists at 100. |
| Finding the PR for a spec | The current branch's PR when it touches `specs/<feature>/`; else the PR for the spec's `**Feature Branch**:` header; newest open first, then newest merged | Searching all PRs per spec: slow, and PR #202 shows threads can sit in unrelated PRs, which is out of scope. |
| When a comment may be written and a thread placed in a document | When the file on disk is byte-identical to the file at the PR head: local `git hash-object` equals the blob `sha` in the PR file list | Local `HEAD` equals PR head: too strict (unrelated unpushed commits block it) and too loose (uncommitted edits pass). This replaces two FR-060 checks with one exact check; the message still names the cause (uncommitted edits, other branch checked out, unpushed or unpulled commits). |
| Which lines can take a line comment | Added files: every line. Modified files: right-side lines inside the patch hunks. Outside those, or no patch (very large file): a file-level comment whose body starts with "Lines a–b:" | Refusing the comment: loses the remark. |
| Pending review vs comment now | A pending review is created on the first "Add to review" (`addPullRequestReview` with the PR head commit), threads added with `addPullRequestReviewThread`. "Comment now" (REST `pulls/{n}/comments`) is offered only while I have no pending review, since GitHub allows one pending review per person | Offering both always: GitHub would reject one of them. |
| Block-to-line mapping | comrak `render.sourcepos` writes `data-sourcepos="start:col-end:col"` on block elements; the interface makes each top-level block (each list item for lists) commentable | Re-parsing markdown in JavaScript: a second parser that can disagree with the renderer. |
| Project-wide counts (FR-065) | One GraphQL query for open PRs (50 newest) with their unresolved thread paths, mapped to feature folders by path prefix | Per-spec queries: 104 calls. |
| Finding `gh` when launched from a desktop launcher | PATH first, then the path printed by the login shell (`$SHELL -lc 'command -v gh'`) | PATH only: launchers often lack the shell's PATH (mise, Homebrew). |

## Design

### Crate `crates/speckit-forge`

```
lib.rs        Forge trait, shared types
github.rs     GitHub adapter (gh)
run.rs        Runner trait: real child process with timeout, and a fake for tests
diff.rs       patch hunks -> commentable right-side lines
local.rs      git checks: current branch, HEAD, remote slug, blob hash, dirty paths
```

```rust
pub trait Forge {
    fn status(&self, repo: &Path, feature: &str, branch_hint: Option<&str>) -> Status;
    fn add_comment(&self, cr: &ChangeRequest, pos: &Position, body: &str, pending: bool) -> Result<(), String>;
    fn submit_review(&self, cr: &ChangeRequest, body: &str) -> Result<(), String>; // always COMMENT
    fn discard_review(&self, cr: &ChangeRequest) -> Result<(), String>;
    fn reply(&self, cr: &ChangeRequest, thread: &str, body: &str) -> Result<(), String>;
    fn set_resolved(&self, thread: &str, resolved: bool) -> Result<(), String>;
    fn edit_comment(&self, comment: &str, body: &str) -> Result<(), String>;
    fn delete_comment(&self, comment: &str) -> Result<(), String>;
    fn open_threads_by_feature(&self, repo: &Path) -> Result<BTreeMap<String, usize>, String>;
}
```

Data model (serialised to the interface):

- **Status**: `checks` (ordered list of `{id, ok, message}`), `read`, `write`, `login`, `cr`, `files` (per feature file: `inPr`, `inSync`, `reason`, `lines` = all | ranges | none), `threads`.
- **ChangeRequest**: forge, number, url, state (open, merged, closed), draft, `base`, `head` commits, node id, my pending review id.
- **Position**: path, `start`, `end` lines, `fileLevel`, and the three commits `base`, `start_commit`, `head` (GitHub uses head; GitLab needs all three).
- **Thread**: id, path, `start`, `end`, state (open, resolved, outdated, pending), resolvedBy, `was` (original line text for outdated threads), comments (`id`, author, createdAt, body, pending, url).

Readiness order and what each failure disables (FR-060):

| # | Check | Failure message (one line) | Disables |
|---|---|---|---|
| 1 | `gh` found | "Comments need the GitHub CLI. Install it (`sudo pacman -S github-cli`, `brew install gh` or `winget install GitHub.cli`), then click Check again." | read + write |
| 2 | signed in (`gh auth token`) | "You're not signed in to GitHub. Run `gh auth login`, then click Check again." | read + write |
| 3 | remote responds and token valid | "Can't reach github.com/owner/repo …" / "Your GitHub sign-in has expired. Run `gh auth login` …" / "origin isn't a GitHub remote." | read + write |
| 4 | PR found | "`branch` has no pull request. Open one with `gh pr create --draft`, then click Check again." | read + write |
| 5 | PR open | "PR #n is merged. Its threads are read-only." | write |
| 6 | file matches PR head (per file) | "spec.md has uncommitted changes …" / "You're on X. Check out Y to comment on this spec." / "Your branch is not in sync with PR #n. Push or pull, then click Check again." / "plan.md isn't changed in PR #n, so GitHub can't take comments on it." | write (and inline placement) for that file |

### Tauri commands (contract)

| Command | Args | Returns |
|---|---|---|
| `review_status` | `feature`, `force` | Status (cached 60 s per feature unless `force`) |
| `review_counts` | – | `{feature: openThreads}` |
| `review_comment` | `feature`, `file`, `start`, `end`, `body`, `pending` | Status |
| `review_submit` | `feature`, `body` | Status |
| `review_discard` | `feature` | Status |
| `review_reply` | `feature`, `thread`, `body` | Status |
| `review_resolve` | `feature`, `thread`, `resolved` | Status |
| `review_edit` / `review_delete` | `feature`, `comment`, (`body`) | Status |

Writes re-check readiness for the file before posting and refuse if it no longer passes.

### Interface

`prototype/review-layer.js` + `prototype/review-layer.css` are the single source for the review UI. They draw the PR chip, the status line, block gutters with **+**, inline threads, the composer, the Conversations panel, the pending-review bar and the submit dialog, and take a backend object: sample data in the mockups, `invoke` calls in the app. `build.py`, `build_review.py` and `make_ui.py` inline them.

## Verification

- Unit: diff hunks; status decisions for each readiness failure with a fake runner; request bodies for every write (event is COMMENT; strings passed only as JSON on stdin).
- SC-007: every top-level block's `data-sourcepos` in every proj-nava `spec.md`, `plan.md`, `tasks.md` starts and ends on a non-blank line, blocks don't overlap, and every non-blank line belongs to a block.
- Live, read-only: status and threads for proj-nava PR #217 (no threads, write allowed) and PR #203 (4 spec threads, other branch checked out, read-only).
- Writes against a real PR are not run without a human go-ahead.
