# Spec Deck: build, run, and test on Linux, macOS, and Windows.
#
# Needs GNU make and a POSIX shell. On Windows, run it from Git Bash or MSYS2
# (for example `winget install ezwinports.make`, then `make` in Git Bash).
#
#   make            list the targets
#   make doctor     check the tools and system libraries this platform needs
#   make run REPO=~/Projects/proj-nava
#
# The interface app/ui/index.html is generated from prototype/template.html; every
# build target regenerates it first when the template or the review layer changed.

.DEFAULT_GOAL := help
.PHONY: help doctor ui build release run run-release dev bundle tauri-cli icons \
        test test-core parity dump mockup fmt lint clean

# ---------------------------------------------------------------- platform
ifeq ($(OS),Windows_NT)
  PLATFORM := windows
  EXE      := .exe
else
  UNAME := $(shell uname -s)
  ifeq ($(UNAME),Darwin)
    PLATFORM := macos
  else
    PLATFORM := linux
  endif
  EXE :=
endif

CARGO   ?= cargo
PYTHON  ?= $(shell command -v python3 2>/dev/null || command -v python 2>/dev/null)
APP     := spec-deck
BIN     := target/debug/$(APP)$(EXE)
BIN_REL := target/release/$(APP)$(EXE)
SCRATCH ?= target/scratch

# A Spec Kit project to open, dump, or compare. Optional for `run` (empty opens the most recent one).
REPO ?=

# Linux only: some GPU and Wayland setups draw a blank WebKitGTK window. `make run SAFE_GL=1`
# turns off the renderers that cause it.
ifeq ($(PLATFORM),linux)
  ifeq ($(SAFE_GL),1)
    RUN_ENV := WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1
  endif
endif

# Linux only: a shell started outside the desktop session (an editor's or agent's terminal,
# a service) may lack WAYLAND_DISPLAY and DISPLAY, and GTK then fails to start. Use the
# session's Wayland socket, else X display :0, when this shell names neither.
ifeq ($(PLATFORM),linux)
define find_display
if [ -z "$$WAYLAND_DISPLAY$$DISPLAY" ]; then \
  rt="$${XDG_RUNTIME_DIR:-/run/user/$$(id -u)}"; \
  w=$$(ls -1 "$$rt" 2>/dev/null | grep -E '^wayland-[0-9]+$$' | head -n 1); \
  if [ -n "$$w" ]; then export XDG_RUNTIME_DIR="$$rt" WAYLAND_DISPLAY="$$w"; echo "This shell has no display set; using the Wayland session ($$w)."; \
  elif [ -S /tmp/.X11-unix/X0 ]; then export DISPLAY=:0; echo "This shell has no display set; using X display :0."; \
  else echo "No graphical session found: DISPLAY and WAYLAND_DISPLAY are unset and no session socket exists. Run this from a terminal in your desktop session."; exit 1; fi; \
fi;
endef
endif

UI_SOURCES := prototype/template.html prototype/review-layer.js prototype/review-layer.css scripts/make_ui.py

define need_repo
	@if [ -z "$(REPO)" ]; then echo "Set REPO to a Spec Kit project, for example: make $@ REPO=~/Projects/proj-nava"; exit 2; fi
endef

# ---------------------------------------------------------------- help
help: ## List the targets
	@echo "Spec Deck on $(PLATFORM). Targets:"
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -e 's/:.*## /\t/' | awk -F'\t' '{ printf "  %-12s %s\n", $$1, $$2 }'
	@echo "Variables: REPO=<project>  SAFE_GL=1 (Linux)  PYTHON=<interpreter>  CARGO=<cargo>"

# ---------------------------------------------------------------- environment
doctor: ## Check the toolchain and this platform's system libraries
	@ok=1; \
	for t in "$(CARGO)" git; do command -v $$t >/dev/null 2>&1 && echo "ok   $$t" || { echo "MISSING $$t"; ok=0; }; done; \
	if [ -n "$(PYTHON)" ]; then echo "ok   python ($(PYTHON))"; else echo "MISSING python3 (needed for make ui, parity, mockup)"; ok=0; fi; \
	command -v gh >/dev/null 2>&1 && echo "ok   gh (review comments)" || echo "note gh not found: review comments stay off until it is installed"; \
	$(CARGO) tauri --version >/dev/null 2>&1 && echo "ok   tauri-cli (make bundle)" || echo "note tauri-cli not installed: only needed for make bundle and make icons (make tauri-cli)"; \
	case "$(PLATFORM)" in \
	  linux) \
	    if command -v pkg-config >/dev/null 2>&1; then \
	      for l in webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0 gtk+-3.0; do \
	        pkg-config --exists $$l && echo "ok   $$l" || { echo "MISSING $$l"; ok=0; }; done; \
	    else echo "MISSING pkg-config"; ok=0; fi; \
	    [ $$ok = 1 ] || echo "Install on Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libayatana-appindicator3-dev librsvg2-dev build-essential pkg-config; Fedora: sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel librsvg2-devel; Arch: sudo pacman -S webkit2gtk-4.1 gtk3 libsoup3 librsvg base-devel" ;; \
	  macos) \
	    xcode-select -p >/dev/null 2>&1 && echo "ok   Xcode command line tools" || { echo "MISSING Xcode command line tools: xcode-select --install"; ok=0; } ;; \
	  windows) \
	    echo "note Windows needs the MSVC build tools (Visual Studio Installer: Desktop development with C++) and WebView2 (preinstalled on Windows 10 and 11)" ;; \
	esac; \
	[ $$ok = 1 ] && echo "Ready to build." || { echo "Fix the MISSING items above."; exit 1; }

tauri-cli: ## Install the Tauri CLI (for bundle and icons)
	$(CARGO) install tauri-cli --version "^2" --locked

# ---------------------------------------------------------------- interface
app/ui/index.html: $(UI_SOURCES)
	@[ -n "$(PYTHON)" ] || { echo "python3 is needed to generate the interface"; exit 1; }
	$(PYTHON) -I scripts/make_ui.py prototype/template.html app/ui/index.html

ui: app/ui/index.html ## Regenerate the app interface from prototype/template.html

# ---------------------------------------------------------------- build and run
build: ui ## Debug build of the app
	$(CARGO) build -p $(APP)

release: ui ## Optimized build of the app
	$(CARGO) build -p $(APP) --release

run: build ## Run the debug app (REPO= optional)
	@$(find_display) echo "$(strip $(RUN_ENV) $(BIN) $(REPO))"; $(RUN_ENV) $(BIN) $(REPO)

run-release: release ## Run the optimized app (REPO= optional)
	@$(find_display) echo "$(strip $(RUN_ENV) $(BIN_REL) $(REPO))"; $(RUN_ENV) $(BIN_REL) $(REPO)

dev: ## Rebuild the interface and restart the app whenever a source file changes (needs cargo-watch)
	@$(CARGO) watch --version >/dev/null 2>&1 || { echo "Install cargo-watch first: cargo install cargo-watch"; exit 1; }
	$(CARGO) watch -w crates -w app/src-tauri/src -w prototype -w scripts \
	  -s '$(MAKE) --no-print-directory run'

bundle: ui ## Installable package for this platform (.deb/.rpm/.AppImage, .app/.dmg, or .msi/.exe)
	@$(CARGO) tauri --version >/dev/null 2>&1 || { echo "Install the Tauri CLI first: make tauri-cli"; exit 1; }
	cd app/src-tauri && $(CARGO) tauri build
	@echo "Packages are in target/release/bundle/"

icons: ## Regenerate every icon size from app/src-tauri/icons/icon.png
	@$(CARGO) tauri --version >/dev/null 2>&1 || { echo "Install the Tauri CLI first: make tauri-cli"; exit 1; }
	cd app/src-tauri && $(CARGO) tauri icon icons/icon.png

# ---------------------------------------------------------------- checks
test: ui ## Run every crate's tests
	$(CARGO) test

test-core: ## Run the parser's tests only
	$(CARGO) test -p speckit-core

parity: ## Compare the Rust parser with prototype/extract.py on REPO
	$(need_repo)
	@mkdir -p $(SCRATCH)
	$(PYTHON) -I prototype/extract.py $(REPO) $(SCRATCH)/py.json
	$(CARGO) run -q -p speckit-core --bin speckit-dump $(REPO) $(SCRATCH)/rs.json
	$(PYTHON) -I scripts/parity.py $(SCRATCH)/py.json $(SCRATCH)/rs.json

dump: ui ## Print the JSON the interface receives for REPO (stdout is only the JSON)
	$(need_repo)
	@$(CARGO) build -q -p $(APP) >&2
	@$(BIN) --dump-view $(REPO)

mockup: ## Build prototype/mockup.html from REPO (git-ignored: it holds the project's content)
	$(need_repo)
	$(PYTHON) -I prototype/extract.py $(REPO) prototype/data.json
	$(PYTHON) -I prototype/build.py

fmt: ## Format the Rust code
	$(CARGO) fmt --all

lint: ## Check formatting and run clippy (each skipped when not installed)
	@if $(CARGO) fmt --version >/dev/null 2>&1; then $(CARGO) fmt --all -- --check; else echo "rustfmt not installed (rustup component add rustfmt); skipped"; fi
	@if $(CARGO) clippy --version >/dev/null 2>&1; then $(CARGO) clippy --all-targets; else echo "clippy not installed (rustup component add clippy); skipped"; fi

clean: ## Remove build output (keeps the generated interface)
	$(CARGO) clean
