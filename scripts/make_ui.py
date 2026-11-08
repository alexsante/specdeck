"""Derive the app interface (app/ui/index.html) from the mockup (prototype/template.html).

The mockup stays the design source: change it, review it, then regenerate.
Usage: python3 -I scripts/make_ui.py prototype/template.html app/ui/index.html

Each replacement must match exactly once; a mismatch stops the script so a
changed template is noticed instead of producing a half-converted file.
"""
import sys

src, out = sys.argv[1], sys.argv[2]
s = open(src, encoding="utf-8").read()


def rep(a, b):
    global s
    n = s.count(a)
    assert n == 1, (n, a[:80])
    s = s.replace(a, b)


def rep_line(prefix, new):
    """Replace the single line that starts with prefix."""
    global s
    lines = s.split("\n")
    hits = [i for i, l in enumerate(lines) if l.startswith(prefix)]
    assert len(hits) == 1, (len(hits), prefix)
    lines[hits[0]] = new
    s = "\n".join(lines)


# ---------------------------------------------------------------- CSS
rep("<title>Spec Deck mockup</title>", "<title>Spec Deck</title>")
rep('<meta name="claude:viewport-fit-sides" content="cover">\n', "")
rep("</style>", """
/* ---------- app shell (replaces the mockup's fake window) ---------- */
html, body { height: 100%; }
body { padding: 0 !important; overflow: hidden; }
.win { border: 0; border-radius: 0; box-shadow: none; max-width: none; height: 100%; display: flex; flex-direction: column; }
.titlebar { display: none; }
.shell { flex: 1; min-height: 0; }
.side, .main { overflow-y: auto; }
.side .open { cursor: pointer; }

/* ---------- rendered documents ---------- */
.doc { font-size: 13.5px; line-height: 1.6; max-width: 900px; min-width: 0; }
.doc > :first-child { margin-top: 0; }
.doc h1 { font-size: 20px; margin: 4px 0 10px; }
.doc h2 { font-size: 16px; margin: 24px 0 8px; padding-top: 12px; border-top: 1px solid var(--line); }
.doc h3 { font-size: 14px; margin: 18px 0 6px; }
.doc h4 { font-size: 13px; margin: 14px 0 4px; color: var(--dim); }
.doc p, .doc ul, .doc ol { margin: 0 0 10px; }
.doc ul, .doc ol { padding-left: 22px; }
.doc li > p { margin: 0; }
.doc code { font-family: var(--mono); font-size: 12px; background: var(--surface-2); border: 1px solid var(--line); padding: 0 4px; border-radius: 4px; }
.doc pre { background: var(--surface-2); border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; overflow-x: auto; }
.doc pre code { background: none; border: 0; padding: 0; }
.doc table { border-collapse: collapse; display: block; overflow-x: auto; font-size: 12.5px; margin: 0 0 12px; }
.doc th, .doc td { border: 1px solid var(--line); padding: 5px 8px; text-align: left; vertical-align: top; }
.doc th { background: var(--surface-2); }
.doc blockquote { margin: 0 0 10px; padding: 4px 12px; border-left: 3px solid var(--accent); color: var(--dim); }
.doc a { color: var(--accent); }
.doc .mermaid { background: var(--surface-2); border: 1px solid var(--line); border-radius: 8px; padding: 12px; overflow-x: auto; text-align: center; margin: 0 0 12px; }
.doc input[type="checkbox"] { margin-right: 6px; }
.docfile + .docfile { margin-top: 18px; padding-top: 14px; border-top: 1px dashed var(--line); }
.docfile-name { font-family: var(--mono); font-size: 11.5px; color: var(--dim); margin-bottom: 6px; }
.doc h2.rv-blk > .rv-gut { top: 14px; }

/* ---------- start views fill the window ---------- */
.shell.bare > .main > .start { min-height: 0; }
</style>""")

rep('''  document.getElementById("wtitle").textContent = "Spec Deck";
  document.getElementById("main").innerHTML = state.view === "splash"''', '''  document.title = "Spec Deck";
  document.getElementById("main").innerHTML = state.view === "splash"''')

# ---------------------------------------------------------------- data becomes runtime state
rep("const DATA = /*__DATA__*/null;\nconst STAGES = DATA.stages; // Specified … Done",
    'let DATA = null;\nconst STAGES = ["Specified", "Clarified", "Planned", "Tasked", "Analyzed", "Building", "Done"];\n'
    "const T = window.__TAURI__;\nconst invoke = (cmd, args) => T.core.invoke(cmd, args);")
rep("const F = DATA.features;\nconst byId = Object.fromEntries(F.map(f => [f.id, f]));\nconst NOW = new Date(DATA.generated);",
    "let F = [], byId = {}, NOW = new Date();")
rep_line("const waitingList = ", "let waitingList = [], unfinishedList = [], agentHeavy = [];")
rep_line("const unfinishedList = ", "")
rep_line("const agentHeavy = ", "")
rep_line("const LISTS = ", "let LISTS = { all: [], timeline: [], waiting: [], unfinished: [], agent: [], review: [] };")
rep_line("const cur = DATA.current", "let cur = null;")
rep_line("const TODAY = ", "let TODAY = new Date();")

# ---------------------------------------------------------------- sidebar project block
rep('<span class="proj"><span class="ic">N</span>${esc(DATA.project)}</span>',
    '<span class="proj" title="${esc(DATA.path)}"><span class="ic">${esc((DATA.project[0] || "?").toUpperCase())}</span>${esc(DATA.project)}</span>')
rep('<button class="open">+ Open project…</button>', '<button class="open" data-pick>+ Open project…</button>')

# ---------------------------------------------------------------- spec view: real actions and documents
rep('<button class="btn" data-toast="Opened folder in editor">Open in editor</button><button class="btn" data-toast="Revealed in Finder">Reveal</button>',
    '<button class="btn" data-folder="${esc(f.id)}">Open folder</button>')
start = s.index("  const docBody = ")
end = s.index("\n", s.index("contract files", start))
s = s[:start] + (
    '  const docKey = `${f.id}/${state.docTab}`;\n'
    '  const docBody = `<div class="doc" id="doc" data-feature="${esc(f.id)}" data-file="${esc(state.docTab)}">'
    '${docCache.get(docKey) ?? `<div class="dim doc-loading">Loading ${esc(state.docTab)}…</div>`}</div>`;'
) + s[end:]
rep('function openPath(p) { toast(`Opens <code>${esc(p)}</code> in your editor (mockup)`); }',
    'function openPath(p) { invoke("open_path", { path: p }).catch(err => toast(esc(err))); }')

# ---------------------------------------------------------------- render, documents, data, events
rep('''function render() {
  if (startView()) return;
  document.getElementById("wtitle").textContent = state.view === "spec" ? `${DATA.project} — ${byId[state.spec].id}` : `${DATA.project} — Spec Deck`;
  renderSide();
  state.view === "spec" ? renderSpec() : renderProject();''', '''function render() {
  if (!DATA && state.view !== "splash" && state.view !== "boot") state.view = "welcome";
  if (startView()) return;
  document.title = state.view === "spec" ? `${byId[state.spec].id} — ${DATA.project}` : `${DATA.project} — Spec Deck`;
  renderSide();
  state.view === "spec" ? renderSpec() : renderProject();
  if (state.view === "spec") loadDoc();''')
rep("// ---------- detail modals ----------", '''// ---------- documents (rendered by Rust; Mermaid drawn here) ----------
const docCache = new Map();
let mermaidReady = null;
function loadMermaid() {
  mermaidReady ??= new Promise((ok, fail) => {
    const sc = document.createElement("script");
    sc.src = "vendor/mermaid-12.2.0.min.js";
    sc.onload = () => {
      const dark = isDark();
      mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: dark ? "dark" : "default" });
      ok(window.mermaid);
    };
    sc.onerror = fail;
    document.head.appendChild(sc);
  });
  return mermaidReady;
}
async function loadDoc() {
  const el = document.getElementById("doc");
  if (!el) return;
  const { feature, file } = el.dataset, key = `${feature}/${file}`;
  if (docCache.has(key)) return;
  let html;
  try { html = await invoke("read_doc", { feature, file }); }
  catch (e) { html = `<p class="dim">${esc(e)}</p>`; }
  const now = document.getElementById("doc");
  if (!now || now.dataset.feature !== feature || now.dataset.file !== file) return; // the view moved on
  now.innerHTML = html;
  now.style.minHeight = ""; // held at the previous document's height while loading
  const blocks = [...now.querySelectorAll("pre > code.language-mermaid")];
  if (blocks.length) {
    try {
      const m = await loadMermaid();
      const nodes = blocks.map(code => {
        const div = document.createElement("div");
        div.className = "mermaid";
        div.dataset.sourcepos = code.parentElement.dataset.sourcepos || "";
        div.textContent = code.textContent;
        code.parentElement.replaceWith(div);
        return div;
      });
      await m.run({ nodes });
    } catch (e) { console.warn("mermaid", e); }
  }
  docCache.set(key, now.innerHTML); // cached before the review layer decorates it
  RV.afterRender(byId[feature], file);
}

function setData(d) {
  DATA = d; F = d.features; byId = Object.fromEntries(F.map(f => [f.id, f]));
  NOW = new Date(d.generated);
  TODAY = new Date(Date.UTC(NOW.getUTCFullYear(), NOW.getUTCMonth(), NOW.getUTCDate()));
  waitingList = F.filter(f => waiting(f).length);
  unfinishedList = F.filter(f => unfinished(f).length || f.stage === "Building");
  agentHeavy = F.filter(f => f.stage !== "Done" && dec(f).agent + dec(f).flagged > 0);
  LISTS = { all: F, timeline: F.filter(inFlight), waiting: waitingList, unfinished: unfinishedList, agent: agentHeavy, review: [] };
  cur = d.current && byId[d.current.feature];
  docCache.clear();
}

// Open a project folder: the splash shows if reading takes more than a moment.
async function openProjectPath(path, target) {
  const t = setTimeout(() => { splashFor = target || recents.find(r => r.path === path) || { name: path.split(/[\\/]/).pop(), path }; state.view = "splash"; render(); }, 150);
  try {
    const d = await invoke("open_project_path", { path });
    clearTimeout(t);
    showProject(d);
  } catch (e) {
    clearTimeout(t);
    if (DATA) { if (state.view === "splash") state.view = "project"; render(); toast(esc(e)); }
    else { welcomeError = String(e); state.view = "welcome"; render(); }
  }
}
function showProject(d) {
  setData(d);
  RV.reset();
  RV.loadCounts();
  refreshRecents();
  welcomeError = "";
  Object.assign(state, { view: "project", stage: null, nav: "all", q: "", tlSel: null, tlCenter: true, modal: null });
  if (location.hash) location.hash = ""; else route();
}
async function pickProject() {
  let path;
  try { path = await invoke("pick_folder"); } catch (e) { return toast(esc(e)); }
  if (path) openProjectPath(path);
}
async function refreshRecents() {
  try { recents = await invoke("recent_projects"); } catch { recents = []; }
  render();
}
async function checkGitHub() {
  try { ghReady = await invoke("gh_ready"); } catch (e) { ghReady = { ok: false, message: String(e) }; }
  if (state.view === "welcome") render();
}

// ---------- detail modals ----------''')

rep('const el = e.target.closest("[data-openpath]', 'const el = e.target.closest("[data-pick],[data-folder],[data-openpath]')
rep('''  const ds = el.dataset;
  if (ds.open) { location.hash = ds.open; return; }''', '''  const ds = el.dataset;
  if ("pick" in ds) return pickProject();
  if (ds.folder) { invoke("open_feature", { feature: ds.folder, reveal: false }).catch(err => toast(esc(err))); return; }
  if (ds.open) { location.hash = ds.open; return; }''')
rep('''  else if (ds.copy) { const ok = () => toast(`Copied <code>${esc(ds.copy)}</code>`), no = () => toast(`Copy blocked here. Command: <code>${esc(ds.copy)}</code>`);
    try { navigator.clipboard.writeText(ds.copy).then(ok, no); } catch { no(); } return; }''', '''  else if (ds.copy) {
    invoke("copy_text", { text: ds.copy }).then(() => toast(`Copied <code>${esc(ds.copy)}</code>`), err => toast(esc(err)));
    return;
  }''')
rep('document.getElementById("win").scrollIntoView({ block: "start" });', 'document.getElementById("main").scrollTop = 0;')
rep("RV.loadCounts();\nroute();\n</script>", "route();\n</script>")  # counts load once a project is open
rep("route();\n</script>", """// Web links open in the browser, not inside the app.
document.addEventListener("click", e => {
  const a = e.target.closest("a[href]");
  if (!a || !/^https?:/.test(a.href)) return;
  e.preventDefault();
  invoke("open_url", { url: a.href }).catch(err => toast(esc(err)));
}, true);

// The watcher pushes a fresh model whenever specs/ or .specify/ change on disk.
T.event.listen("project-updated", e => {
  const main = document.getElementById("main"), top = main.scrollTop;
  setData(e.payload);
  RV.invalidate();
  render();
  main.scrollTop = top;
  toast("Updated from disk");
});
(async () => {
  state.view = "boot";
  refreshRecents();
  checkGitHub();
  let target = null;
  try { target = await invoke("launch_target"); } catch {}
  if (!target) { state.view = "welcome"; return render(); }
  const t = setTimeout(() => { splashFor = target; state.view = "splash"; render(); }, 150);
  try {
    const d = await invoke("current_project");
    clearTimeout(t);
    if (d) return showProject(d);
  } catch (e) {
    clearTimeout(t);
    welcomeError = String(e);
  }
  state.view = "welcome";
  render();
})();
</script>""")

# ---------------------------------------------------------------- start views: real recents, gh check, handlers
start = s.index("// Recent projects: in the mockup")
end = s.index("let splashFor = null")
s = s[:start] + """// Recent projects come from the app's config folder (recent_projects); gh is checked once at launch.
let recents = [];
let ghReady = { ok: false, login: "", message: "" };
""" + s[end:]
start, end = s.index("// @start-handlers-start"), s.index("// @start-handlers-end") + len("// @start-handlers-end")
s = s[:start] + """document.addEventListener("click", e => {
  const el = e.target.closest("[data-recent],[data-forget]");
  if (!el) return;
  if (el.dataset.forget) { invoke("forget_recent", { path: el.dataset.forget }).then(r => { recents = r; render(); }); return; }
  openProjectPath(el.dataset.recent);
});
document.addEventListener("keydown", e => {
  if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "o") { e.preventDefault(); pickProject(); }
});
// Folders dropped on the window (Tauri gives real paths; the web view's own drop events don't).
T.event.listen("tauri://drag-enter", () => document.getElementById("drop")?.classList.add("over"));
T.event.listen("tauri://drag-leave", () => document.getElementById("drop")?.classList.remove("over"));
T.event.listen("tauri://drag-drop", e => {
  document.getElementById("drop")?.classList.remove("over");
  const p = e.payload?.paths?.[0];
  if (p) openProjectPath(p);
});""" + s[end:]

# ---------------------------------------------------------------- appearance: the window's own zoom and title bar
start, end = s.index("// @zoom-start"), s.index("// @zoom-end") + len("// @zoom-end")
s = s[:start] + """function setZoom(z, theme) { invoke("set_appearance", { zoom: z, theme }).catch(e => console.warn("set_appearance", e)); }""" + s[end:]
rep("async function loadDoc() {", """// A theme change re-renders documents so Mermaid diagrams pick up the new colours.
appearanceHooks.push(() => {
  docCache.clear();
  if (window.mermaid) mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: isDark() ? "dark" : "default" });
  if (DATA && state.view === "spec") render();
});
async function loadDoc() {""")

# ---------------------------------------------------------------- pull available: real checks
start, end = s.index("// @pull-start"), s.index("// @pull-end") + len("// @pull-end")
s = s[:start] + """let pullState = null, pullAt = 0, pullTimer = null;
// Checked when a project opens, every 5 minutes, on window focus, and after changes on disk
// (a pull rewrites HEAD), at most every 20 s.
async function checkPull(force) {
  if (!DATA) return;
  const wait = force ? 0 : Math.max(0, 20000 - (Date.now() - pullAt));
  clearTimeout(pullTimer);
  pullTimer = setTimeout(async () => {
    pullAt = Date.now();
    const path = DATA.path;
    try { const p = await invoke("pull_status"); if (DATA?.path === path) { pullState = p; render(); } } catch { /* offline: keep the last state */ }
  }, wait);
}
setInterval(() => checkPull(false), 5 * 60 * 1000);
addEventListener("focus", () => { if (Date.now() - pullAt > 60000) checkPull(false); });""" + s[end:]
rep('''  RV.reset();
  RV.loadCounts();
  refreshRecents();''', '''  RV.reset();
  RV.loadCounts();
  refreshRecents();
  pullState = null;
  checkPull(true);''')
rep('''  RV.invalidate();
  render();''', '''  RV.invalidate();
  checkPull(false);
  render();''')

# ---------------------------------------------------------------- review layer: real backend, shared code inlined
start, end = s.index("// @rv-backend-start"), s.index("// @rv-backend-end") + len("// @rv-backend-end")
s = s[:start] + """const RV = createReview({
  backend: {
    status: (feature, branch, force) => invoke("review_status", { feature, branch, force }),
    counts: () => invoke("review_counts"),
    comment: (feature, file, start, end, body, pending) => invoke("review_comment", { feature, file, start, end, body, pending }),
    submit: (feature, body) => invoke("review_submit", { feature, body }),
    discard: feature => invoke("review_discard", { feature }),
    reply: (feature, thread, body) => invoke("review_reply", { feature, thread, body }),
    resolve: (feature, thread, resolved) => invoke("review_resolve", { feature, thread, resolved }),
    edit: (feature, comment, body) => invoke("review_edit", { feature, comment, body }),
    del: (feature, comment) => invoke("review_delete", { feature, comment }),
  },
  esc, toast, rerender: () => render(), openTab: tab => { state.docTab = tab; render(); },
  copy: text => invoke("copy_text", { text }),
  openUrl: url => invoke("open_url", { url }),
});""" + s[end:]
rep("<script>/*__RVMOCK__*/</script>\n", "")
import os
proto = os.path.dirname(os.path.abspath(src))
read = lambda n: open(os.path.join(proto, n), encoding="utf-8").read()
rep("/*__RVCSS__*/", read("review-layer.css"))
rep("/*__RVJS__*/", read("review-layer.js"))

assert "/*__DATA__*/" not in s
assert "createMockBackend" not in s, "mockup sample data would reach the app"
assert "archived-demo" not in s and "Opens the dropped folder" not in s, "mockup start-view samples would reach the app"
assert 'data-toast="' not in s, "a mockup-only toast button would reach the app"
open(out, "w", encoding="utf-8").write(s)
print("wrote", out, len(s), "bytes")
