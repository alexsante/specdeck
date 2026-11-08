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

/* ---------- empty state ---------- */
.empty-state { max-width: 460px; margin: 14vh auto 0; text-align: center; display: flex; flex-direction: column; gap: 12px; align-items: center; }
.empty-state h1 { margin: 6px 0 0; font-size: 22px; }
.empty-state p { margin: 0; color: var(--dim); }
.empty-state .err { color: var(--bad); }
.empty-state .logo { width: 56px; height: 56px; border-radius: 14px; background: var(--grad); display: grid; place-items: center; }
.empty-state .logo svg { width: 28px; height: 28px; stroke: #fff; stroke-width: 2.2; fill: none; stroke-linecap: round; }
</style>""")

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
    '${docCache.get(docKey) ?? `<div class="dim">Loading ${esc(state.docTab)}…</div>`}</div>`;'
) + s[end:]
rep('function openPath(p) { toast(`Opens <code>${esc(p)}</code> in your editor (mockup)`); }',
    'function openPath(p) { invoke("open_path", { path: p }).catch(err => toast(esc(err))); }')

# ---------------------------------------------------------------- render, documents, data, events
rep('''function render() {
  document.getElementById("wtitle").textContent = state.view === "spec" ? `${DATA.project} — ${byId[state.spec].id}` : `${DATA.project} — Spec Deck`;
  renderSide();
  state.view === "spec" ? renderSpec() : renderProject();''', '''function render() {
  if (!DATA) return renderEmpty();
  document.title = state.view === "spec" ? `${byId[state.spec].id} — ${DATA.project}` : `${DATA.project} — Spec Deck`;
  renderSide();
  state.view === "spec" ? renderSpec() : renderProject();
  if (state.view === "spec") loadDoc();''')
rep("// ---------- detail modals ----------", '''function renderEmpty(message) {
  document.title = "Spec Deck";
  document.getElementById("side").innerHTML = `<div><h6>Projects</h6><button class="open" data-pick>+ Open project…</button></div>`;
  document.getElementById("main").innerHTML = `<div class="empty-state">
    <div class="logo"><svg viewBox="0 0 24 24"><path d="M5 7h9M8 12h11M6 17h7"/></svg></div>
    <h1>Open a Spec Kit project</h1>
    <p>Pick the folder that contains <span class="mono">specs/</span>. Spec Deck reads it and never writes to it.</p>
    ${message ? `<p class="err">${esc(message)}</p>` : ""}
    <button class="btn primary" data-pick>Open project…</button></div>`;
}

// ---------- documents (rendered by Rust; Mermaid drawn here) ----------
const docCache = new Map();
let mermaidReady = null;
function loadMermaid() {
  mermaidReady ??= new Promise((ok, fail) => {
    const sc = document.createElement("script");
    sc.src = "vendor/mermaid-12.2.0.min.js";
    sc.onload = () => {
      const dark = matchMedia("(prefers-color-scheme: dark)").matches;
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

async function pickProject() {
  try {
    const d = await invoke("pick_project");
    if (!d) return; // cancelled
    setData(d);
    RV.reset();
    RV.loadCounts();
    Object.assign(state, { stage: null, nav: "all", q: "", tlSel: null, tlCenter: true, modal: null });
    if (location.hash) location.hash = ""; else route();
  } catch (e) {
    DATA ? toast(esc(e)) : renderEmpty(String(e));
  }
}

// ---------- detail modals ----------''')

rep('const el = e.target.closest("', 'const el = e.target.closest("[data-pick],[data-folder],')
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
  try {
    const d = await invoke("current_project");
    if (d) { setData(d); RV.loadCounts(); }
  } catch (e) {
    return renderEmpty(String(e));
  }
  route();
})();
</script>""")

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
assert 'data-toast="' not in s, "a mockup-only toast button would reach the app"
open(out, "w", encoding="utf-8").write(s)
print("wrote", out, len(s), "bytes")
