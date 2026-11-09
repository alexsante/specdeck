# speckit-mac

Read `HANDOFF.md` first. It holds the decisions and context from the brainstorm that produced this repo.

- The spec is `specs/001-command-center/spec.md`. It is the source of truth; record new decisions in its Clarifications section.
- `prototype/extract.py` is the reference parser. `crates/speckit-core` must produce identical JSON for the same repo; check with `scripts/parity.py` after any parser change.
- The app is Rust + Tauri 2 (`app/`). Its interface `app/ui/index.html` is generated from the mockup `prototype/template.html` by `scripts/make_ui.py`: edit the template, never the generated file.
- The review UI is `prototype/review-layer.js` + `.css`, inlined into both mockups and the app by the build scripts. Edit those files, not the copies.
- Never commit proj-nava content (`prototype/data.json`, built mockup pages). They are git-ignored.
- The app is read-only toward the repos it opens, except "Pull now" (`git pull --ff-only` on a click, refused while anything is uncommitted; FR-070). Review comments (User Story 10) go to GitHub through `gh`, never to local files. Don't add other writes without the human's go-ahead.
- The source is open under MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). Contributor-facing docs are `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and `SECURITY.md`. Leave the license as `MIT OR Apache-2.0` in the workspace `Cargo.toml`.
