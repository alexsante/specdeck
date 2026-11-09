# Spec Deck

[![CI](https://github.com/alexsante/speckit-mac/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsante/speckit-mac/actions/workflows/ci.yml)

A desktop command center for [Spec Kit](https://github.com/github/spec-kit) repositories. Open a repo and see where every spec stands. Open one spec to see its stage, tasks, user stories, review health, decisions, and documents.

Linux first, then macOS, then Windows. Built with Rust and Tauri 2. The app is read-only toward the repository it opens, except **Pull now**, which runs `git pull --ff-only` when you click it and refuses while anything is uncommitted.

Status: specified, with a working proof of concept. No signed releases yet. Behavior is defined in [`specs/001-command-center/spec.md`](specs/001-command-center/spec.md).

## Run

Install Rust (stable) and the [Tauri prerequisites](CONTRIBUTING.md#setup) for your platform, then:

```sh
cargo run -p spec-deck -- /path/to/spec-kit-repo
```

Leave the path off to start from the welcome view and use Open project.

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
