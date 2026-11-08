# speckit-mac

Read `HANDOFF.md` first. It holds the decisions and context from the brainstorm that produced this repo.

- The spec is `specs/001-command-center/spec.md`. It is the source of truth; record new decisions in its Clarifications section.
- `prototype/extract.py` is the reference parser. A Swift parser must match its behaviour on the same input.
- Never commit proj-nava content (`prototype/data.json`, built mockup pages). They are git-ignored.
- The app is read-only toward the repos it opens.
