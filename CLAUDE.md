# speckit-mac

Read `HANDOFF.md` first. It holds the decisions and context from the brainstorm that produced this repo.

- The spec is `specs/001-command-center/spec.md`. It is the source of truth; record new decisions in its Clarifications section.
- `prototype/extract.py` is the reference parser. `crates/speckit-core` must produce identical JSON for the same repo; check with `scripts/parity.py` after any parser change.
- The app is Rust + Tauri 2 (`app/`). The interface is `app/ui/index.html` (plain HTML/JS, no bundler; Tauri API via `window.__TAURI__`).
- Never commit proj-nava content (`prototype/data.json`, built mockup pages). They are git-ignored.
- The app is read-only toward the repos it opens.
