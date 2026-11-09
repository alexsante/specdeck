# Contributing to Spec Deck

Thanks for helping. Spec Deck is a desktop command center for [Spec Kit](https://github.com/github/spec-kit) repositories: open a repo, see where every spec stands, then open one spec for its stage, tasks, user stories, review health, decisions, and documents.

## License of your contribution

Spec Deck is licensed under the MIT license or the Apache License 2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.

By opening a pull request you agree that your contribution is available under those same terms. The Apache License describes how to mark something you submit as not a contribution.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By taking part you agree to it.

## Before you change behavior

- Product behavior lives in [`specs/001-command-center/spec.md`](specs/001-command-center/spec.md). When a change alters that behavior, update the spec and record the decision in its Clarifications section.
- The app is read-only toward the repositories it opens. The one exception is Pull now: a click runs `git pull --ff-only`, and the app refuses while anything is uncommitted. Review comments go to GitHub through the `gh` CLI, never into local files. A new write needs a decision in the spec first.
- `prototype/extract.py` is the reference parser. `crates/speckit-core` must produce the same JSON for the same repository. After a parser change, compare the two with `scripts/parity.py` on a Spec Kit repo you are allowed to use.
- The interface source is `prototype/template.html`. `app/ui/index.html` is generated from it. Edit the template, then regenerate. CI rejects a generated file that has drifted.
- The review interface source is `prototype/review-layer.js` and `prototype/review-layer.css`. The build scripts inline those files. Edit the source, then regenerate.
- Leave `prototype/data.json` and the built mockup pages untracked. They are git-ignored because they can contain another project's content.

`HANDOFF.md` is the design notebook from the original brainstorm: platform order, why stage is derived from files, and how the Tauri app is laid out. Read it when a change touches those decisions.

## Setup

You need Rust (stable, via [rustup](https://rustup.rs)), Python 3, and Git. `rust-toolchain.toml` selects stable.

On Linux, install the Tauri 2 system packages before the first build. CI uses this list on Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

On macOS, install the Xcode command line tools, then use the same Cargo commands. Windows is later in the platform order. CI runs the tests on Linux and only compiles on macOS and Windows.

The GitHub CLI (`gh`) is needed only when you try review comments or the pull badge against GitHub. The app never reads a token itself; `gh` does.

The first release build of the dependencies takes several minutes. Later builds take seconds.

## Everyday commands

Run the app against a Spec Kit repository, or with no path and use Open project:

```sh
cargo run -p spec-deck -- /path/to/spec-kit-repo
```

Test everything that does not need a private checkout:

```sh
cargo test --workspace
```

CI runs `cargo test --workspace --locked` on Ubuntu 24.04. One test, `proj_nava`, reads `~/Projects/proj-nava` when that checkout exists and returns immediately otherwise. You do not need that repository.

Parser parity, after a change to `prototype/extract.py` or `crates/speckit-core`:

```sh
python3 -I prototype/extract.py /path/to/spec-kit-repo /tmp/py.json
cargo run -q -p speckit-core --bin speckit-dump /path/to/spec-kit-repo /tmp/rs.json
python3 -I scripts/parity.py /tmp/py.json /tmp/rs.json
```

Regenerate the app interface after editing the template or the review layer:

```sh
python3 -I scripts/make_ui.py prototype/template.html app/ui/index.html
```

Preview the mockup in a browser. `prototype/data.json` and `prototype/mockup.html` stay untracked:

```sh
python3 -I prototype/extract.py /path/to/spec-kit-repo prototype/data.json
python3 -I prototype/build.py
```

## Style

Match the lines around your edit, and leave unrelated code as it is. The Rust sources keep long lines, and CI does not run rustfmt.

## Pull requests

Branch from `main`. In the pull request, say why the change is needed and how you checked it. `cargo test --workspace` should pass.

A parser fix and a visual redesign are easier to review as separate pull requests.

## Issues

Use the issue templates. For a parser bug, include a small excerpt you are allowed to share, with private names removed. Report security problems through [SECURITY.md](SECURITY.md), in private.
