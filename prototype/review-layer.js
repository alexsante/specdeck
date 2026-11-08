// Review layer (User Story 10): PR review threads on the rendered Spec Kit documents.
// One source for the mockups and the app. The page supplies a backend: sample data in
// the mockups, Tauri commands in the app. Every backend call returns a fresh status:
// { feature, checks, read, write, message, login, branch, cr, files, threads }.
function createReview(cfg) {
  const { backend, esc, toast, rerender, openTab } = cfg;
  const S = new Map(), loading = new Set(), fetchedAt = new Map();
  let counts = {}, cur = { f: null, tab: null }, refetchTimer = null;
  const st = { sel: null, draft: "", summary: "", filter: "open", collapsed: new Set(), dialog: false, confirmDiscard: false, busy: false, jump: null };

  const plural = (n, w) => `${n} ${w}${n === 1 ? "" : "s"}`;
  const lines = (a, b) => a === b ? `line ${a}` : `lines ${a}–${b}`;
  const tabOf = p => p.includes("/") ? p.split("/")[0] : p;
  const md1 = s => esc(s || "").replace(/`([^`]+)`/g, "<code>$1</code>");
  const when = iso => { const d = new Date(iso); return isNaN(d) ? esc(iso || "") : d.toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }); };
  const ini = u => esc((u || "?").slice(0, 2).toUpperCase());
  const fileOf = (s, p) => s?.files?.find(x => x.path === p);
  const openN = s => (s?.threads || []).filter(t => t.state === "open").length;
  const pendingN = s => (s?.threads || []).flatMap(t => t.comments).filter(c => c.pending).length;
  const thread = id => (S.get(cur.f?.id)?.threads || []).find(t => t.id === id);
  const range = t => [t.start ?? t.end, t.end ?? t.start];

  async function load(f, force) {
    if (!f || loading.has(f.id)) return;
    loading.add(f.id);
    try {
      const s = await backend.status(f.id, f.branch || null, !!force);
      S.set(f.id, s);
      if (s.read) counts[f.id] = openN(s);
      if (force) toast(s.message ? "Still not ready: " + md1(s.message) : "GitHub checks passed");
    } catch (e) {
      S.set(f.id, { feature: f.id, checks: [], read: false, write: false, message: String(e), files: [], threads: [] });
    } finally {
      fetchedAt.set(f.id, Date.now());
      loading.delete(f.id);
      rerender();
    }
  }

  async function loadCounts() {
    try { counts = (await backend.counts()) || {}; } catch { counts = {}; }
    rerender();
  }

  // Files changed on disk: drop stale statuses. Re-check the open spec, at most every 20 s.
  function invalidate() {
    for (const k of [...S.keys()]) if (k !== cur.f?.id) S.delete(k);
    const f = cur.f;
    if (!f) return;
    const wait = Math.max(0, 20000 - (Date.now() - (fetchedAt.get(f.id) || 0)));
    clearTimeout(refetchTimer);
    refetchTimer = setTimeout(() => { if (cur.f?.id === f.id) load(f); }, wait);
  }

  function reset() { S.clear(); counts = {}; st.sel = null; st.draft = ""; st.dialog = false; }

  // ---------- pieces the page places ----------
  function chip(f) {
    const s = S.get(f.id);
    if (s?.cr) {
      const c = s.cr, n = s.read ? openN(s) : null;
      return `<a class="rv-chip" href="${esc(c.url)}" data-rv-url="${esc(c.url)}"><span class="rv-dot ${esc(c.state)}"></span>PR #${c.number} · ${c.draft ? "Draft" : c.state[0].toUpperCase() + c.state.slice(1)}${n != null ? ` · ${plural(n, "open thread")}` : ""}</a>`;
    }
    return counts[f.id] ? `<span class="rv-chip"><span class="rv-dot open"></span>${plural(counts[f.id], "open thread")}</span>` : "";
  }

  function line(f, tab) {
    const s = S.get(f.id);
    if (!s) return `<div class="rv-line wait"><span class="rv-lamp"></span><span class="msg">Checking GitHub…</span></div>`;
    let ok = !s.message, msg;
    if (ok) {
      const files = s.files.filter(x => tabOf(x.path) === tab), off = files.filter(x => !x.canComment);
      msg = `Commenting as <b>${esc(s.login)}</b> on PR #${s.cr.number}.`;
      if (files.length && off.length === files.length) { ok = false; msg = md1(off[0].reason); }
      else if (off.length) msg += ` ${plural(off.length, "file")} here can't take comments: ${md1(off[0].reason)}`;
    } else msg = md1(s.message);
    return `<div class="rv-line ${ok ? "" : "off"}"><span class="rv-lamp"></span><span class="msg">${msg}</span><button class="btn sm" data-rv-check ${loading.has(f.id) ? "disabled" : ""}>${loading.has(f.id) ? "Checking…" : "Check again"}</button></div>`;
  }

  function tabCount(f, tab) {
    const s = S.get(f.id);
    const n = (s?.threads || []).filter(t => t.state === "open" && tabOf(t.path) === tab).length;
    return n ? `<span class="rv-cnt">${n}</span>` : "";
  }

  // The documents card next to the Conversations list (when threads could be read).
  function grid(f, docsHtml) {
    const s = S.get(f.id);
    if (!s?.read) return docsHtml;
    const by = { open: [], pending: [], resolved: [], outdated: [] };
    s.threads.forEach(t => by[t.state]?.push(t));
    const lbl = { open: "Open", pending: "Pending", resolved: "Resolved", outdated: "Outdated" };
    const list = by[st.filter].sort((a, b) => a.path.localeCompare(b.path) || (a.end || 0) - (b.end || 0));
    const item = t => {
      const c = t.comments[0] || {}, [a, b] = range(t);
      const placed = fileOf(s, t.path)?.inSync && t.state !== "outdated";
      return `<button class="rv-ci" data-rv-jump="${esc(t.id)}"><div class="top"><span class="rv-av ${c.author === s.login ? "me" : ""}">${ini(c.author)}</span><b>${esc(c.author)}</b><span class="mono dim">${esc(t.path)}${a ? ":" + a : ""}</span>${t.comments.length > 1 ? `<span class="dim r">${t.comments.length - 1} repl${t.comments.length === 2 ? "y" : "ies"}</span>` : ""}</div>
        <div class="ex">${esc(c.text ?? c.body)}</div>${t.was ? `<div class="was">The line this was on has changed. It said: ${esc(t.was)}</div>` : !placed ? `<div class="was">Not shown in the document: the file on disk differs from PR #${s.cr.number}.</div>` : ""}</button>`;
    };
    return `<div class="rv-grid">${docsHtml}<div class="card panel rv-convo"><header>Conversations<span class="r"><a href="${esc(s.cr.url)}" data-rv-url="${esc(s.cr.url)}">PR #${s.cr.number} ↗</a></span></header>
      <div class="rv-filters">${Object.keys(lbl).map(k => `<button class="${st.filter === k ? "on" : ""}" data-rv-filter="${k}">${lbl[k]} · ${by[k].length}</button>`).join("")}</div>
      ${list.length ? list.map(item).join("") : `<div class="none-msg">No ${lbl[st.filter].toLowerCase()} threads on this spec's files.</div>`}</div></div>`;
  }

  // ---------- threads and composer inside the document ----------
  function commentHtml(c, s, t) {
    const mine = c.pending && c.author === s.login;
    return `<div class="rv-cm"><span class="rv-av ${c.author === s.login ? "me" : ""}">${ini(c.author)}</span><div class="b"><div class="who">${esc(c.author)}<span class="dim">${c.pending ? "Pending, only you can see this" : when(c.createdAt)}</span>
      ${mine ? `<span class="acts"><button class="rv-link" data-rv-edit="${esc(c.id)}" data-rv-thread="${esc(t.id)}">Edit</button><button class="rv-link" data-rv-del="${esc(c.id)}">Delete</button></span>` : ""}</div>
      <div class="tx">${c.html ?? esc(c.body)}</div></div></div>`;
  }
  function threadHtml(t, s) {
    const col = st.collapsed.has(t.id) || (t.state === "resolved" && !st.collapsed.has("!" + t.id));
    const [a, b] = range(t);
    const chipH = t.state === "resolved" ? `<span class="chip rv-ok">Resolved${t.resolvedBy ? " by " + esc(t.resolvedBy) : ""}</span>` : t.state === "pending" ? `<span class="chip rv-pend">Pending review</span>` : `<span class="chip calm">Open</span>`;
    const where = t.fileLevel ? "whole file" : lines(a, b);
    const head = `<div class="th" ${col ? `data-rv-expand="${esc(t.id)}"` : ""}>${chipH}<span class="mono dim">${where}</span><span class="dim r">${col ? `${plural(t.comments.length, "comment")} · show` : plural(t.comments.length, "comment")}</span></div>`;
    if (col) return `<div class="rv-thread collapsed" id="rvt-${esc(t.id)}">${head}</div>`;
    const off = !s.write || st.busy, url = t.comments.find(c => c.url)?.url;
    const replying = st.sel?.reply === t.id || (st.sel?.edit && st.sel.thread === t.id);
    const acts = t.state === "pending" ? "" : `<div class="tacts"><button class="btn sm" data-rv-reply="${esc(t.id)}" ${off ? "disabled" : ""}>Reply</button><button class="btn sm" data-rv-resolve="${esc(t.id)}" ${off ? "disabled" : ""}>${t.state === "resolved" ? "Unresolve" : "Resolve"}</button><button class="btn sm" data-rv-agent="${esc(t.id)}" title="Copies a prompt for your coding agent">Copy for agent</button>${url ? `<a class="btn sm" href="${esc(url)}" data-rv-url="${esc(url)}">View on GitHub ↗</a>` : ""}${t.state === "resolved" ? `<button class="btn sm" data-rv-collapse="${esc(t.id)}">Collapse</button>` : ""}</div>`;
    return `<div class="rv-thread ${t.state}" id="rvt-${esc(t.id)}">${head}${t.comments.map(c => commentHtml(c, s, t)).join("")}${replying ? composerHtml(s) : acts}</div>`;
  }
  function composerHtml(s) {
    const x = st.sel, hasPending = !!s.cr?.pendingReview;
    const label = x.edit ? "Edit your pending comment" : x.reply ? "Reply" : `Comment on <span class="mono">${esc(x.path)}</span> ${lines(x.a, x.b)} · shift-click another <b>+</b> to extend`;
    const fs = fileOf(s, x.path);
    const fileLevel = !x.reply && !x.edit && fs && !fs.allLines && !(fs.ranges || []).some(r => r[0] <= x.a && x.b <= r[1]);
    const btns = x.edit ? `<button class="btn primary" data-rv-send="edit">${st.busy ? "Saving…" : "Save"}</button>`
      : x.reply ? `<button class="btn primary" data-rv-send="reply">${st.busy ? "Posting…" : hasPending ? "Add reply to review" : "Reply"}</button>`
      : `${hasPending ? "" : `<button class="btn" data-rv-send="now">Comment now</button>`}<button class="btn primary" data-rv-send="review">${st.busy ? "Saving…" : hasPending ? "Add to review" : "Start a review"}</button>`;
    return `<div class="rv-composer"><div class="lbl">${label}</div>${fileLevel ? `<div class="lbl warn">These lines aren't in the PR's diff, so this goes on the whole file, starting "${lines(x.a, x.b).replace(/^l/, "L")}:".</div>` : ""}
      <textarea id="rv-draft" placeholder="${x.reply ? "Write a reply" : "Leave a comment. Markdown works. Ctrl+Enter to save."}">${esc(st.draft)}</textarea>
      <div class="row2"><button class="btn" data-rv-cancel>Cancel</button><span class="sp"></span>${btns}</div></div>`;
  }

  function blocksOf(container) {
    const out = [];
    for (const el of container.children) {
      if (!el.dataset?.sourcepos) continue;
      if (el.tagName === "UL" || el.tagName === "OL") {
        for (const li of el.children) if (li.dataset?.sourcepos) out.push(li);
      } else out.push(el);
    }
    return out.map(el => { const m = el.dataset.sourcepos.match(/^(\d+):\d+-(\d+):\d+/); return m && { el, a: +m[1], b: +m[2] }; }).filter(Boolean);
  }
  const anchor = el => /^(TABLE|PRE|HR)$/.test(el.tagName) ? (el.parentElement.classList.contains("rv-wrap") ? el.parentElement : wrap(el)) : el;
  function wrap(el) { const w = document.createElement("div"); w.className = "rv-wrap"; el.replaceWith(w); w.appendChild(el); return w; }
  const after = (el, html) => el.tagName === "LI" ? el.insertAdjacentHTML("beforeend", html) : el.insertAdjacentHTML("afterend", html);

  function decorate(doc, f, s) {
    doc.querySelectorAll(".rv-gut,.rv-thread,.rv-composer,.rv-filethreads").forEach(e => e.remove());
    doc.querySelectorAll(".rv-blk").forEach(e => e.classList.remove("rv-blk", "rv-sel", "rv-flash"));
    doc.classList.add("rv-on");
    const parts = [...doc.querySelectorAll("section.docfile[data-path]")];
    const units = parts.length ? parts.map(p => [p, p.dataset.path]) : [[doc, doc.dataset.file || cur.tab]];
    for (const [box, path] of units) {
      const fs = fileOf(s, path), can = !!fs?.canComment && s.write;
      const why = can ? "" : (fs?.reason || s.message || "Commenting is off for this file.").replace(/`/g, "");
      const bs = blocksOf(box);
      for (const k of bs) {
        const a = anchor(k.el);
        a.classList.add("rv-blk");
        const sel = st.sel && !st.sel.reply && !st.sel.edit && st.sel.path === path && k.a <= st.sel.b && k.b >= st.sel.a;
        if (sel) a.classList.add("rv-sel");
        a.insertAdjacentHTML("afterbegin", `<span class="rv-gut"><span class="ln">${k.a}</span><button class="rv-add ${can ? "" : "off"}" data-rv-add="${esc(path)}" data-a="${k.a}" data-b="${k.b}" title="${esc(can ? "Comment on " + lines(k.a, k.b) : why)}" aria-label="${esc(can ? "Comment on " + lines(k.a, k.b) : why)}">+</button></span>`);
      }
      if (!s.read) continue;
      const ts = s.threads.filter(t => t.path === path && t.state !== "outdated");
      const fileLevel = ts.filter(t => t.fileLevel || t.end == null);
      if (fileLevel.length) {
        const top = box.querySelector(":scope > .docfile-name");
        const html = `<div class="rv-filethreads">${fileLevel.map(t => threadHtml(t, s)).join("")}</div>`;
        top ? top.insertAdjacentHTML("afterend", html) : box.insertAdjacentHTML("afterbegin", html);
      }
      if (fs?.inSync) {
        for (const t of ts.filter(t => !t.fileLevel && t.end != null)) {
          const k = bs.filter(k => k.a <= t.end).at(-1) || bs[0];
          if (k) after(anchor(k.el), threadHtml(t, s));
        }
      }
      if (st.sel && !st.sel.reply && !st.sel.edit && st.sel.path === path) {
        const k = bs.filter(k => k.a <= st.sel.b).at(-1);
        if (k) after(anchor(k.el), composerHtml(s));
      }
    }
  }

  function barHtml(f, s) {
    const n = pendingN(s);
    if (!s?.read || !n) return "";
    if (st.confirmDiscard) return `<div class="rv-bar"><span>Discard ${plural(n, "pending comment")}? They have not been posted.</span><span class="sp"></span><button class="btn" data-rv-keep>Keep</button><button class="btn primary" data-rv-discard-yes>Discard</button></div>`;
    return `<div class="rv-bar"><span class="chip rv-pend">Pending review</span><span>${plural(n, "comment")} only you can see. Nothing is posted until you submit.</span><span class="sp"></span><button class="btn" data-rv-discard ${st.busy ? "disabled" : ""}>Discard</button><button class="btn primary" data-rv-submit ${s.write && !st.busy ? "" : "disabled"}>Submit review</button></div>`;
  }
  function dialogHtml(f, s) {
    const pend = s.threads.flatMap(t => t.comments.filter(c => c.pending).map(c => [t, c]));
    const code = (s.cr.changedFiles || 0) - (s.cr.specFiles || 0);
    const why = code > 0 ? `This PR also changes ${plural(code, "file")} outside <span class="mono">specs/</span>, so a verdict here would cover code you haven't reviewed.` : "Verdicts stay on GitHub.";
    return `<div class="rv-scrim" data-rv-scrim><div class="rv-dialog" role="dialog" aria-modal="true" aria-labelledby="rv-dh">
      <h2 id="rv-dh">Submit review on PR #${s.cr.number}</h2>
      <ul>${pend.map(([t, c]) => `<li><span class="mono">${esc(t.path)}${t.end ? ":" + t.end : ""}</span> ${esc((c.text ?? c.body).slice(0, 80))}${(c.text ?? c.body).length > 80 ? "…" : ""}</li>`).join("")}</ul>
      <label for="rv-summary" class="dim">Summary (optional)</label>
      <textarea id="rv-summary" placeholder="Overall thoughts on the spec">${esc(st.summary)}</textarea>
      <label class="opt"><input type="radio" name="rv-ev" checked> <b>Comment</b><span class="d">Post the comments without a verdict.</span></label>
      <label class="opt dis"><input type="radio" name="rv-ev" disabled> <b>Approve</b><span class="d">Not offered in Spec Deck. ${why} Approve on GitHub.</span></label>
      <label class="opt dis"><input type="radio" name="rv-ev" disabled> <b>Request changes</b><span class="d">Not offered in Spec Deck, for the same reason. Use GitHub.</span></label>
      <div class="acts"><button class="btn" data-rv-close>Cancel</button><button class="btn primary" data-rv-post ${st.busy ? "disabled" : ""}>${st.busy ? "Posting…" : `Post ${plural(pend.length, "comment")}`}</button></div></div></div>`;
  }

  // Called after every page render (f = null when no spec is shown).
  function afterRender(f, tab) {
    cur = { f, tab };
    const bar = document.getElementById("rvbar"), dlg = document.getElementById("rvdialog");
    if (!f) { if (bar) bar.innerHTML = ""; if (dlg) dlg.innerHTML = ""; return; }
    const s = S.get(f.id);
    if (!s && !loading.has(f.id)) load(f);
    const doc = document.getElementById("doc");
    if (doc && s) decorate(doc, f, s);
    if (bar) bar.innerHTML = barHtml(f, s);
    if (dlg) dlg.innerHTML = st.dialog && s?.cr ? dialogHtml(f, s) : "";
    const ta = document.getElementById("rv-draft");
    if (ta) { ta.oninput = e => st.draft = e.target.value; if (!st.dialog) ta.focus({ preventScroll: true }); }
    const sum = document.getElementById("rv-summary");
    if (sum) { sum.oninput = e => st.summary = e.target.value; sum.focus(); }
    if (st.jump) {
      const el = document.getElementById("rvt-" + st.jump);
      if (el) {
        st.jump = null;
        el.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
        el.classList.add("rv-flash");
      }
    }
  }

  async function run(op, done) {
    const f = cur.f;
    if (st.busy || !f) return;
    st.busy = true; rerender();
    try {
      const s = await op(f);
      S.set(f.id, s); counts[f.id] = openN(s);
      st.sel = null; st.draft = "";
      toast(done);
      return true;
    } catch (e) { toast(md1(String(e))); return false; }
    finally { st.busy = false; rerender(); }
  }

  function agentPrompt(t) {
    const s = S.get(cur.f.id), [a, b] = range(t);
    return `Review thread on specs/${cur.f.id}/${t.path}${a ? " " + lines(a, b) : ""} (PR #${s.cr.number}, ${s.cr.url}):\n`
      + t.comments.map(c => `- ${c.author}: ${(c.text ?? c.body).replace(/\s+/g, " ").trim()}`).join("\n")
      + `\nUpdate the spec to address it. Don't resolve the thread; a human does that.`;
  }

  // Returns true when the click was the review layer's.
  function click(e) {
    const el = e.target.closest("[data-rv-add],[data-rv-send],[data-rv-cancel],[data-rv-reply],[data-rv-resolve],[data-rv-agent],[data-rv-url],[data-rv-edit],[data-rv-del],[data-rv-expand],[data-rv-collapse],[data-rv-filter],[data-rv-jump],[data-rv-check],[data-rv-submit],[data-rv-discard],[data-rv-discard-yes],[data-rv-keep],[data-rv-close],[data-rv-post],[data-rv-scrim]");
    if (!el) return false;
    const d = el.dataset, f = cur.f, s = f && S.get(f.id);
    if ("rvScrim" in d) { if (e.target === el) { st.dialog = false; rerender(); } return true; }
    e.preventDefault();
    if (d.rvUrl) { cfg.openUrl(d.rvUrl); return true; }
    if ("rvCheck" in d) { load(f, true); rerender(); return true; }
    if (!s) return true;
    if (d.rvAdd != null) {
      if (el.classList.contains("off")) { toast(md1(fileOf(s, d.rvAdd)?.reason || s.message || "Commenting is off for this file.")); return true; }
      const a = +d.a, b = +d.b;
      if (e.shiftKey && st.sel && !st.sel.reply && !st.sel.edit && st.sel.path === d.rvAdd) st.sel = { ...st.sel, a: Math.min(st.sel.a, a), b: Math.max(st.sel.b, b) };
      else { st.sel = { path: d.rvAdd, a, b }; st.draft = ""; }
      rerender(); return true;
    }
    if ("rvCancel" in d) { st.sel = null; st.draft = ""; rerender(); return true; }
    if (d.rvSend) {
      const body = st.draft.trim();
      if (!body) { document.getElementById("rv-draft")?.focus(); return true; }
      const x = st.sel;
      if (d.rvSend === "edit") run(f => backend.edit(f.id, x.edit, body), "Pending comment updated");
      else if (d.rvSend === "reply") run(f => backend.reply(f.id, x.reply, body), s.cr.pendingReview ? "Reply added to your pending review" : `Reply posted to PR #${s.cr.number}`);
      else {
        const pending = d.rvSend === "review";
        run(f => backend.comment(f.id, x.path, x.a, x.b, body, pending), pending ? "Added to your pending review" : `Comment posted to PR #${s.cr.number}`);
      }
      return true;
    }
    if (d.rvReply) { st.sel = { reply: d.rvReply }; st.draft = ""; rerender(); return true; }
    if (d.rvEdit) { const c = thread(d.rvThread)?.comments.find(c => c.id === d.rvEdit); st.sel = { edit: d.rvEdit, thread: d.rvThread }; st.draft = c?.text ?? c?.body ?? ""; rerender(); return true; }
    if (d.rvDel) { run(f => backend.del(f.id, d.rvDel), "Pending comment deleted"); return true; }
    if (d.rvResolve) { const t = thread(d.rvResolve), res = t.state !== "resolved"; run(f => backend.resolve(f.id, t.id, res), res ? "Thread resolved" : "Thread reopened"); return true; }
    if (d.rvAgent) { const t = thread(d.rvAgent); cfg.copy(agentPrompt(t)).then(() => toast("Copied a prompt for your coding agent"), err => toast(md1(String(err)))); return true; }
    if (d.rvExpand) { st.collapsed.delete(d.rvExpand); st.collapsed.add("!" + d.rvExpand); rerender(); return true; }
    if (d.rvCollapse) { st.collapsed.delete("!" + d.rvCollapse); st.collapsed.add(d.rvCollapse); rerender(); return true; }
    if (d.rvFilter) { st.filter = d.rvFilter; rerender(); return true; }
    if (d.rvJump) {
      const t = thread(d.rvJump); if (!t) return true;
      st.collapsed.delete(t.id); st.collapsed.add("!" + t.id); st.jump = t.id;
      const tab = tabOf(t.path);
      if (tab !== cur.tab) openTab(tab); else rerender();
      if (!fileOf(s, t.path)?.inSync || t.state === "outdated") { st.jump = null; toast(t.state === "outdated" ? "That line has changed since the comment. Open it on GitHub to see the old version." : `This file on disk differs from PR #${s.cr.number}, so the thread isn't shown in place.`); }
      return true;
    }
    if ("rvSubmit" in d) { st.dialog = true; st.summary = ""; rerender(); return true; }
    if ("rvClose" in d) { st.dialog = false; rerender(); return true; }
    if ("rvPost" in d) { const n = pendingN(s); run(f => backend.submit(f.id, st.summary.trim()), `Review posted to PR #${s.cr.number}: ${plural(n, "comment")}`).then(ok => { if (ok) { st.dialog = false; st.filter = "open"; } rerender(); }); return true; }
    if ("rvDiscard" in d) { st.confirmDiscard = true; rerender(); return true; }
    if ("rvKeep" in d) { st.confirmDiscard = false; rerender(); return true; }
    if ("rvDiscardYes" in d) { st.confirmDiscard = false; run(f => backend.discard(f.id), "Pending review discarded"); return true; }
    return true;
  }

  function key(e) {
    if (e.key === "Escape" && (st.dialog || st.sel || st.confirmDiscard)) { st.dialog = false; st.sel = null; st.confirmDiscard = false; rerender(); return true; }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && e.target.id === "rv-draft") {
      const primary = e.target.closest(".rv-composer")?.querySelector(".btn.primary");
      primary?.click(); return true;
    }
    return false;
  }

  return { load, loadCounts, invalidate, reset, chip, line, tabCount, grid, afterRender, click, key, counts: () => counts };
}
