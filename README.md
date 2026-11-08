# Spec Deck (working name)

A read-only macOS command center for projects that use [Spec Kit](https://github.com/github/spec-kit): see where every spec stands, then open one to see its stage, tasks, user stories, review health, decisions, and documents in one place.

Status: specified, not yet built. See `HANDOFF.md` and `specs/001-command-center/spec.md`.

## Prototype

`prototype/` holds a Python parser that reads a Spec Kit repo and an HTML mockup built from its output:

```sh
python3 -I prototype/extract.py /path/to/spec-kit-repo prototype/data.json
python3 -I prototype/build.py   # writes prototype/mockup.html
```
