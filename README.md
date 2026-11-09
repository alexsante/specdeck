# Spec Deck (working name)

A read-only macOS command center for projects that use [Spec Kit](https://github.com/github/spec-kit): see where every spec stands, then open one to see its stage, tasks, user stories, review health, decisions, and documents in one place.

Status: specified; a Rust + Tauri proof of concept runs on Linux (branch `tauri-poc`). See `HANDOFF.md` and `specs/001-command-center/spec.md`.

## Download

Get the latest installer for your platform from **[Releases → Latest](https://github.com/alexsante/speckit-mac/releases/latest)** (the repo is private: you need access to it).

| Platform | File |
|---|---|
| macOS (Apple Silicon and Intel) | `…_universal.dmg` |
| Windows 10 and 11 (x64) | `…_x64-setup.exe`, or `…_x64_en-US.msi` |
| Linux x64 | `…_amd64.AppImage` (any distribution), `…_amd64.deb` (Debian, Ubuntu), `….x86_64.rpm` (Fedora, openSUSE) |

The builds are not code-signed yet. On macOS, right-click the app and choose **Open** the first time (or run `xattr -dr com.apple.quarantine "/Applications/Spec Deck.app"`). On Windows, choose **More info → Run anyway** if SmartScreen appears. Spec Deck needs `git`, and `gh` signed in for review comments.

## Release a new version

```sh
make version V=0.2.0          # sets the version in Cargo.toml and Cargo.lock
git commit -am 'Release v0.2.0' && git tag v0.2.0 && git push origin HEAD v0.2.0
```

The tag starts `.github/workflows/release.yml`, which builds every installer into a **draft** release. Check it on GitHub and click **Publish**; it then becomes the latest release. To try the builds without releasing, run the Release workflow by hand from the Actions tab: the installers are attached to the run instead. `.github/workflows/ci.yml` tests every push and pull request.

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
