// Diagram viewer: Mermaid diagrams in documents open in a viewer that zooms, pans and goes full screen.
// Inlined into the app by scripts/make_ui.py. Edit this file, not the copy in app/ui/index.html.
//
// In a document, each diagram gets an "Expand" button; clicking the diagram does the same.
// In the viewer: wheel or pinch zooms at the pointer (trackpad scrolling pans), drag moves,
// double-click zooms in, + − 0 (fit) 1 (100%) and arrow keys work, F toggles full screen, Esc closes.
const DG = (() => {
  const MIN = 0.05, MAX = 10;
  const esc = s => String(s ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const call = (cmd, args) => window.__TAURI__ ? window.__TAURI__.core.invoke(cmd, args) : Promise.reject(new Error("no app"));
  let v = null; // { list, i, k, x, y, w, h, fs }
  let host, stage, canvas;

  // Called after Mermaid has drawn a document's diagrams (before the document is cached).
  function decorate(root, file) {
    root.querySelectorAll(".mermaid").forEach(d => {
      if (d.querySelector(".dg-open") || !d.querySelector("svg")) return;
      d.classList.add("dg");
      d.title = "Click to open the diagram viewer";
      d.insertAdjacentHTML("beforeend", `<button class="dg-open" type="button" aria-label="Expand diagram">⤢ Expand</button>`);
      if (file) d.dataset.file = file;
    });
  }

  function ensureHost() {
    if (host) return;
    host = document.createElement("div");
    host.id = "dgview";
    host.hidden = true;
    host.setAttribute("role", "dialog");
    host.setAttribute("aria-modal", "true");
    host.setAttribute("aria-label", "Diagram viewer");
    document.body.appendChild(host);
    host.addEventListener("click", onBarClick);
    host.addEventListener("wheel", onWheel, { passive: false });
    host.addEventListener("pointerdown", onDown);
    host.addEventListener("dblclick", e => { if (e.target.closest(".dg-stage")) zoomAt(e.clientX, e.clientY, 2); });
    // Safari / WKWebView trackpad pinch
    let g0 = 1;
    host.addEventListener("gesturestart", e => { e.preventDefault(); g0 = v?.k || 1; });
    host.addEventListener("gesturechange", e => { e.preventDefault(); if (v) zoomAt(e.clientX, e.clientY, (g0 * e.scale) / v.k); });
  }

  function open(diagram) {
    const doc = diagram.closest(".doc") || document;
    const list = [...doc.querySelectorAll(".mermaid.dg")];
    ensureHost();
    v = { list, i: Math.max(0, list.indexOf(diagram)), k: 1, x: 0, y: 0, fs: false };
    host.hidden = false;
    document.documentElement.classList.add("dg-lock");
    show();
    host.querySelector(".dg-close").focus();
  }

  function show() {
    const d = v.list[v.i], svg = d.querySelector("svg");
    const vb = svg.viewBox?.baseVal;
    const r = svg.getBoundingClientRect();
    v.w = vb?.width || r.width || 800;
    v.h = vb?.height || r.height || 600;
    const n = v.list.length, file = d.dataset.file;
    host.innerHTML = `<div class="dg-bar">
        <div class="dg-title">${n > 1 ? `<button class="dg-btn" data-dg="prev" ${v.i ? "" : "disabled"} aria-label="Previous diagram">‹</button><span>Diagram ${v.i + 1} of ${n}</span><button class="dg-btn" data-dg="next" ${v.i < n - 1 ? "" : "disabled"} aria-label="Next diagram">›</button>` : `<span>Diagram</span>`}${file ? `<span class="dg-file">${esc(file)}</span>` : ""}</div>
        <div class="dg-tools">
          <button class="dg-btn" data-dg="out" aria-label="Zoom out" title="Zoom out (−)">−</button>
          <button class="dg-btn dg-pct" data-dg="one" title="Actual size (1)">100%</button>
          <button class="dg-btn" data-dg="in" aria-label="Zoom in" title="Zoom in (+)">+</button>
          <button class="dg-btn" data-dg="fit" title="Fit to window (0)">Fit</button>
          <span class="dg-sep"></span>
          <button class="dg-btn" data-dg="fs" title="Full screen (F)">${v.fs ? "Exit full screen" : "Full screen"}</button>
          <button class="dg-btn" data-dg="copy" title="Copy the Mermaid source">Copy source</button>
          <button class="dg-btn dg-close" data-dg="close" aria-label="Close" title="Close (Esc)">✕</button>
        </div></div>
      <div class="dg-stage"><div class="dg-canvas"></div></div>
      <div class="dg-hint">Wheel or pinch to zoom · drag or two-finger scroll to move · double-click to zoom in · <kbd>0</kbd> fit · <kbd>Esc</kbd> close</div>`;
    stage = host.querySelector(".dg-stage");
    canvas = host.querySelector(".dg-canvas");
    const copy = svg.cloneNode(true);
    copy.removeAttribute("style");
    copy.setAttribute("width", v.w);
    copy.setAttribute("height", v.h);
    canvas.style.width = v.w + "px";
    canvas.style.height = v.h + "px";
    canvas.appendChild(copy);
    fit();
  }

  function apply() {
    const r = stage.getBoundingClientRect();
    v.sw = r.width; v.sh = r.height; // remembered so a resize can keep the view centred
    canvas.style.transform = `translate(${v.x}px, ${v.y}px) scale(${v.k})`;
    const pct = host.querySelector(".dg-pct");
    if (pct) pct.textContent = Math.round(v.k * 100) + "%";
  }
  function fit() {
    const r = stage.getBoundingClientRect(), pad = 32;
    v.k = Math.min(MAX, Math.max(MIN, Math.min((r.width - pad * 2) / v.w, (r.height - pad * 2) / v.h)));
    v.x = (r.width - v.w * v.k) / 2;
    v.y = (r.height - v.h * v.k) / 2;
    apply();
  }
  function zoomAt(cx, cy, f) {
    const r = stage.getBoundingClientRect(), px = cx - r.left, py = cy - r.top;
    const k = Math.min(MAX, Math.max(MIN, v.k * f));
    v.x = px - (px - v.x) * (k / v.k);
    v.y = py - (py - v.y) * (k / v.k);
    v.k = k;
    apply();
  }
  const zoomCenter = f => { const r = stage.getBoundingClientRect(); zoomAt(r.left + r.width / 2, r.top + r.height / 2, f); };
  function actual() { const r = stage.getBoundingClientRect(); zoomAt(r.left + r.width / 2, r.top + r.height / 2, 1 / v.k); }
  function pan(dx, dy) { v.x += dx; v.y += dy; apply(); }

  function onWheel(e) {
    if (!v || !e.target.closest(".dg-stage")) return;
    e.preventDefault();
    // A mouse wheel (whole lines, or large integer pixel steps without sideways movement) zooms;
    // pinch arrives as ctrl+wheel; any other trackpad scroll pans.
    const wheel = e.deltaMode === 1 || (e.deltaX === 0 && Number.isInteger(e.deltaY) && Math.abs(e.deltaY) >= 50);
    if (e.ctrlKey || e.metaKey || wheel) {
      const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
      zoomAt(e.clientX, e.clientY, Math.exp(-dy * (e.ctrlKey && !wheel ? 0.01 : 0.0015)));
    } else pan(-e.deltaX, -e.deltaY);
  }

  function onDown(e) {
    if (!v || e.button !== 0 || !e.target.closest(".dg-stage")) return;
    e.preventDefault();
    const sx = e.clientX - v.x, sy = e.clientY - v.y;
    stage.classList.add("dragging");
    stage.setPointerCapture(e.pointerId);
    const move = ev => { v.x = ev.clientX - sx; v.y = ev.clientY - sy; apply(); };
    const up = () => { stage.classList.remove("dragging"); stage.removeEventListener("pointermove", move); stage.removeEventListener("pointerup", up); stage.removeEventListener("pointercancel", up); };
    stage.addEventListener("pointermove", move);
    stage.addEventListener("pointerup", up);
    stage.addEventListener("pointercancel", up);
  }

  async function setFullscreen(on) {
    try { await call("set_fullscreen", { on }); }
    catch {
      // Outside the app (or if the window refuses), fall back to the page's own full screen.
      try { on ? await host.requestFullscreen?.() : document.fullscreenElement && await document.exitFullscreen(); } catch {}
    }
    v.fs = on;
    const b = host.querySelector('[data-dg="fs"]');
    if (b) b.textContent = on ? "Exit full screen" : "Full screen";
  }

  function close() {
    if (!v) return;
    if (v.fs) setFullscreen(false);
    v = null;
    host.hidden = true;
    host.innerHTML = "";
    document.documentElement.classList.remove("dg-lock");
  }

  function onBarClick(e) {
    const b = e.target.closest("[data-dg]");
    if (!b || !v) return;
    const a = b.dataset.dg;
    if (a === "in") zoomCenter(1.25);
    else if (a === "out") zoomCenter(0.8);
    else if (a === "one") actual();
    else if (a === "fit") fit();
    else if (a === "fs") setFullscreen(!v.fs);
    else if (a === "close") close();
    else if (a === "prev" || a === "next") { v.i += a === "next" ? 1 : -1; show(); }
    else if (a === "copy") {
      const src = v.list[v.i].dataset.src || "";
      const done = () => window.toast?.("Copied the Mermaid source");
      call("copy_text", { text: src }).then(done, () => navigator.clipboard?.writeText(src).then(done, () => {}));
    }
  }

  // Opening: the Expand button or a click on the diagram. Runs before the page's own click handler.
  document.addEventListener("click", e => {
    const d = e.target.closest(".doc .mermaid.dg");
    if (!d || e.target.closest(".rv-gut, a, button:not(.dg-open)")) return;
    e.stopPropagation();
    open(d);
  }, true);

  document.addEventListener("keydown", e => {
    if (!v) return;
    const k = e.key;
    const map = { "+": () => zoomCenter(1.25), "=": () => zoomCenter(1.25), "-": () => zoomCenter(0.8), "0": fit, "1": actual,
      f: () => setFullscreen(!v.fs), F: () => setFullscreen(!v.fs), Escape: close,
      ArrowLeft: () => pan(60, 0), ArrowRight: () => pan(-60, 0), ArrowUp: () => pan(0, 60), ArrowDown: () => pan(0, -60) };
    if (e.metaKey || e.ctrlKey || e.altKey || !map[k]) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    map[k]();
  }, true);
  // A resize (or full screen) keeps the zoom and moves the view by half the change, so what was centred stays centred.
  window.addEventListener("resize", () => {
    if (!v || !stage) return;
    const r = stage.getBoundingClientRect();
    pan((r.width - v.sw) / 2, (r.height - v.sh) / 2);
  });

  return { decorate, open, close };
})();
