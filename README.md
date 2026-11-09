# Spec Deck (working name)

A read-only macOS command center for projects that use [Spec Kit](https://github.com/github/spec-kit): see where every spec stands, then open one to see its stage, tasks, user stories, review health, decisions, and documents in one place.

Status: specified; a Rust + Tauri proof of concept runs on Linux (branch `tauri-poc`). See `HANDOFF.md` and `specs/001-command-center/spec.md`.

## Build and run

The Makefile works on Linux, macOS, and Windows (from Git Bash or MSYS2):

```sh
make doctor                               # check the toolchain and system libraries
make run REPO=/path/to/spec-kit-repo      # build and open a project (REPO optional)
make test                                 # all crates' tests
make parity REPO=/path/to/spec-kit-repo   # Rust parser vs prototype/extract.py
make bundle                               # installable package (needs: make tauri-cli)
```

`make` alone lists every target. On Linux, `make run SAFE_GL=1` works around a blank window on some GPU and Wayland setups.

## Prototype

`prototype/` holds a Python parser that reads a Spec Kit repo and an HTML mockup built from its output:

```sh
python3 -I prototype/extract.py /path/to/spec-kit-repo prototype/data.json
python3 -I prototype/build.py   # writes prototype/mockup.html
```
