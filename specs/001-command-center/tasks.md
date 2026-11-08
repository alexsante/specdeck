# Tasks: User Story 10 · Discuss a spec on its pull request

**Input**: [plan.md](./plan.md), [spec.md](./spec.md) (FR-060 to FR-067, SC-007)
User Stories 1 to 9 were built as the Tauri proof of concept before this file existed; their tasks are not listed here.

## Phase 1: Setup

- [x] T001 Create `crates/speckit-forge` and add it to the workspace (`Cargo.toml`)
- [x] T002 [P] Process runner with timeout and no prompts, plus a scripted fake for tests (`crates/speckit-forge/src/run.rs`)

## Phase 2: Foundational

- [x] T003 Forge trait, Status, ChangeRequest, FileState, Thread, Comment, Position; `position()` picks line vs file-level (`crates/speckit-forge/src/lib.rs`)
- [x] T004 [P] Diff hunks to commentable right-side lines (`crates/speckit-forge/src/diff.rs`)
- [x] T005 [P] Read-only git queries: remote slug, dirty paths, files under a folder (`crates/speckit-forge/src/local.rs`)
- [x] T006 comrak source positions, with block ends pulled back off trailing blank lines (`app/src-tauri/src/main.rs`)
- [x] T007 SC-007 test over every proj-nava `spec.md`, `plan.md`, `tasks.md`: 271 files, 24,117 blocks, 0 problems

## Phase 3: User Story 10 (P2)

**Goal**: comment on Spec Kit files from the rendered documents; see, reply to, and resolve PR review threads.
**Independent Test**: spec.md User Story 10.

- [x] T008 [US10] Readiness checks in FR-060 order with one-line messages; `gh` found on PATH or through the login shell (`github.rs`)
- [x] T009 [US10] PR lookup: current branch's PR when it touches the feature folder, else the Feature Branch header's PR; file list paged past 100
- [x] T010 [US10] Per-file sync check: local blob equals the PR file's blob; reason names the cause
- [x] T011 [US10] Threads with pagination, filtered to the feature folder; open, resolved, outdated, pending; text of the original line for outdated threads
- [x] T012 [US10] Writes: pending review (create, add thread), comment now (REST), submit as COMMENT only, discard, reply, resolve and unresolve, edit and delete pending comments; strings only as JSON on stdin
- [x] T013 [US10] Open threads by feature across open PRs (one query)
- [x] T014 [US10] Unit tests with the fake runner: each readiness failure, ready state, other branch checked out, write request bodies (10 tests)
- [x] T015 [US10] Tauri commands `review_*` with a 60 s cache cleared by the file watcher; writes re-check before posting (`app/src-tauri/src/review.rs`)
- [x] T016 [US10] Review layer shared by the mockups and the app: PR chip, status line, gutters with **+**, inline threads, composer, Conversations, pending-review bar, submit dialog (`prototype/review-layer.js`, `prototype/review-layer.css`)
- [x] T017 [P] [US10] Sample backend for the mockups (`prototype/review-mock.js`); `review.html`, `template.html`, `build.py`, `build_review.py`, `make_ui.py` inline the layer
- [x] T018 [US10] "Open review threads" view in the sidebar
- [x] T019 [US10] Live read-only checks against proj-nava: PR #217 (12 spec files commentable), PR #203 (4 outdated bot threads), PR #171 (merged, read-only, resolved threads)
- [x] T020 [US10] Interface run in headless Chromium with real Rust output: gutters, placement, folder sections, comment arguments, pending bar
- [ ] T021 [US10] Post, reply, resolve and submit against a real pull request (waiting for a human go-ahead on which PR)
- [ ] T022 [US10] Look at it in the real WebKitGTK window on Linux

## Phase 4: Polish

- [x] T023 Update `HANDOFF.md` and `CLAUDE.md` (forge crate, review layer files, build steps)
- [ ] T024 GitLab adapter (`glab`) behind the same trait. Later, per spec.
