# Spec Deck

[![CI](https://github.com/alexsante/speckit-mac/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsante/speckit-mac/actions/workflows/ci.yml)

A desktop command center for [Spec Kit](https://github.com/github/spec-kit) repositories. Open a repo and see where every spec stands. Open one spec to see its stage, tasks, user stories, review health, decisions, and documents.

Linux first, then macOS, then Windows. Built with Rust and Tauri 2. The app is read-only toward the repository it opens, except **Pull now**, which runs `git pull --ff-only` when you click it and refuses while anything is uncommitted.

Status: specified, with a working proof of concept. Releases are not code-signed yet. Behavior is defined in [`specs/001-command-center/spec.md`](specs/001-command-center/spec.md).

## Download

Get the latest installer for your platform from **[Releases → Latest](https://github.com/alexsante/speckit-mac/releases/latest)** (the repo is private: you need access to it).

| Platform | File |
|---|---|
| macOS (Apple Silicon and Intel) | `…_universal.dmg` |
| Windows 10 and 11 (x64) | `…_x64-setup.exe`, or `…_x64_en-US.msi` |
| Linux x64 | `…_amd64.AppImage` (any distribution), `…_amd64.deb` (Debian, Ubuntu), `….x86_64.rpm` (Fedora, openSUSE) |

The builds are not code-signed yet. On macOS, right-click the app and choose **Open** the first time (or run `xattr -dr com.apple.quarantine "/Applications/Spec Deck.app"`). On Windows, choose **More info → Run anyway** if SmartScreen appears. Spec Deck needs `git`, and `gh` signed in for review comments.

## Build and run

Install Rust (stable) and the [Tauri prerequisites](CONTRIBUTING.md#setup) for your platform. The Makefile works on Linux, macOS, and Windows (from Git Bash or MSYS2):

```sh
make doctor                               # check the toolchain and system libraries
make run REPO=/path/to/spec-kit-repo      # build and open a project (REPO optional)
make test                                 # all crates' tests
make parity REPO=/path/to/spec-kit-repo   # Rust parser vs prototype/extract.py
make bundle                               # installable package (needs: make tauri-cli)
```

Leave `REPO` off to start from the welcome view and use Open project. `make` alone lists every target. On Linux, `make run SAFE_GL=1` works around a blank window on some GPU and Wayland setups.

## Release a new version

```sh
make version V=0.2.0          # sets the version in Cargo.toml and Cargo.lock
git commit -am 'Release v0.2.0' && git tag v0.2.0 && git push origin HEAD v0.2.0
```

The tag starts `.github/workflows/release.yml`, which builds every installer into a **draft** release. Check it on GitHub and click **Publish**; it then becomes the latest release. To try the builds without releasing, run the Release workflow by hand from the Actions tab: the installers are attached to the run instead. `.github/workflows/ci.yml` tests every push and pull request.

## Layout

```
crates/speckit-core/     parser and model. Must match prototype/extract.py
crates/speckit-forge/    GitHub review threads and pull status, through the gh CLI
app/                     Tauri 2 app. app/ui/index.html is generated
prototype/               Python reference parser and the HTML mockup
scripts/parity.py        diff the Rust and Python parser JSON
scripts/make_ui.py       regenerate app/ui/index.html from prototype/template.html
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Participation is covered by the [code of conduct](CODE_OF_CONDUCT.md).

## License

Spec Deck is licensed under either of:

- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)

at your option.
