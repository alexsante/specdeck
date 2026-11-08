// Sample backend for the mockups: same status shape as the app's review commands,
// answered from memory. Writes change the sample threads; nothing leaves the page.
function createMockBackend(o) {
  const me = o.me, pr = o.pr;
  let threads = structuredClone(o.threads), pendingReview = threads.some(t => t.state === "pending") ? "R_sample" : null, n = 100;
  const html = s => "<p>" + o.esc(s).replace(/\n\n+/g, "</p><p>").replace(/\n/g, "<br>") + "</p>";
  const pass = id => ({ id, ok: true, message: "" });
  const fail = (id, message) => ({ id, ok: false, message });
  const wait = v => new Promise(r => setTimeout(() => r(v), 280));
  const fix = c => ({ ...c, html: html(c.body), text: c.body });

  function status(fid) {
    const sc = o.scenario();
    const base = { feature: fid, checks: [], read: false, write: false, message: null, login: me, branch: o.branch, cr: null, files: [], threads: [] };
    const stop = (id, msg, okBefore) => ({ ...base, checks: [...okBefore.map(pass), fail(id, msg)], message: msg });
    if (fid !== o.feature) return stop("pr", `This spec's branch has no pull request in this mockup. Open one with \`gh pr create --draft\`, then click Check again.`, ["cli", "auth", "remote"]);
    if (sc === "nogh") return stop("cli", "Comments need the GitHub CLI. Install it with your package manager (for example `sudo pacman -S github-cli` or `sudo apt install gh`), then click Check again.", []);
    if (sc === "noauth") return stop("auth", "You're not signed in to GitHub. Run `gh auth login` in a terminal, then click Check again.", ["cli"]);
    if (sc === "offline") return stop("remote", `Can't reach \`github.com/${o.repo}\`. Check your connection or VPN, then click Check again.`, ["cli", "auth"]);
    if (sc === "nopr") return stop("pr", `\`${o.branch}\` has no pull request. Open one with \`gh pr create --draft\`, then click Check again.`, ["cli", "auth", "remote"]);
    const merged = sc === "merged";
    const checks = ["cli", "auth", "remote", "pr"].map(pass);
    checks.push(merged ? fail("open", `PR #${pr.number} is merged. Its threads are read-only.`) : pass("open"));
    const reason = p => sc === "dirty" && p === "spec.md" ? "`spec.md` has uncommitted changes, so its lines may not match GitHub. Commit and push them to comment on this file."
      : sc === "ahead" ? `Your branch isn't in sync with PR #${pr.number}. Push or pull, then click Check again.`
      : sc === "otherbranch" ? `You're on \`main\`. Check out \`${o.branch}\` to comment on this spec.` : null;
    const files = o.files.map(p => {
      const r = reason(p), inSync = !r;
      return { path: p, inPr: true, inSync, canComment: inSync && !merged, reason: r || (merged ? checks[4].message : null), allLines: true, ranges: [] };
    });
    return {
      ...base, checks, read: true, write: !merged, message: merged ? checks[4].message : null,
      cr: { forge: "github", repo: o.repo, number: pr.number, url: `https://github.com/${o.repo}/pull/${pr.number}`, state: merged ? "merged" : "open", draft: false,
        headRef: o.branch, base: "b", head: pr.head, nodeId: "PR_x", pendingReview, changedFiles: pr.changedFiles, specFiles: pr.specFiles },
      files, threads: threads.map(t => ({ ...t, comments: t.comments.map(fix) })),
    };
  }
  const find = id => threads.find(t => t.id === id);
  const now = () => new Date().toISOString();
  function settle() {
    threads = threads.filter(t => t.comments.length);
    if (!threads.some(t => t.comments.some(c => c.pending))) pendingReview = null;
  }
  return {
    status: (fid) => wait(status(fid)),
    counts: () => wait({ [o.feature]: threads.filter(t => t.state === "open").length }),
    comment(fid, path, a, b, body, pending) {
      if (pending) pendingReview ??= "R_sample";
      threads.push({ id: "T" + n++, path, start: a, end: b, fileLevel: false, state: pending ? "pending" : "open", resolvedBy: null, was: null,
        comments: [{ id: "C" + n++, author: me, createdAt: now(), body, pending, url: "" }] });
      return wait(status(fid));
    },
    reply(fid, tid, body) { find(tid).comments.push({ id: "C" + n++, author: me, createdAt: now(), body, pending: !!pendingReview, url: "" }); return wait(status(fid)); },
    resolve(fid, tid, res) { const t = find(tid); t.state = res ? "resolved" : "open"; t.resolvedBy = res ? me : null; return wait(status(fid)); },
    edit(fid, cid, body) { threads.forEach(t => t.comments.forEach(c => { if (c.id === cid) c.body = body; })); return wait(status(fid)); },
    del(fid, cid) { threads.forEach(t => t.comments = t.comments.filter(c => c.id !== cid)); settle(); return wait(status(fid)); },
    submit(fid) {
      threads.forEach(t => { t.comments.forEach(c => { if (c.pending) { c.pending = false; c.createdAt = now(); } }); if (t.state === "pending") t.state = "open"; });
      pendingReview = null; return wait(status(fid));
    },
    discard(fid) { threads.forEach(t => t.comments = t.comments.filter(c => !c.pending)); settle(); return wait(status(fid)); },
  };
}

// Sample threads anchored to real lines of the campaign budgeting spec.md (PR #217 has none yet).
function sampleThreads(me) {
  const c = (author, at, body, pending = false) => ({ id: "c" + Math.random().toString(36).slice(2, 8), author, createdAt: at, body, pending, url: "https://github.com/" });
  return [
    { id: "T1", path: "spec.md", start: 46, end: 46, fileLevel: false, state: "open", comments: [
      c("dana-k", "2026-11-08T09:14:00Z", "This rule is still a delivery assumption, not a product decision. Can we get it confirmed before schema lock? It decides the shape of the policy table."),
      c(me, "2026-11-08T09:40:00Z", "Agreed. I'll ask in WB-27394 and record the answer as a Clarifications session.")] },
    { id: "T2", path: "spec.md", start: 146, end: 146, fileLevel: false, state: "open", comments: [
      c("dana-k", "2026-11-08T09:22:00Z", "Should \"at most one live version\" be a database constraint, or is the use-case check enough? Two concurrent activations could both pass the check.")] },
    { id: "T3", path: "spec.md", start: 39, end: 39, fileLevel: false, state: "resolved", resolvedBy: me, comments: [
      c("dana-k", "2026-11-07T16:05:00Z", "If an org admin is also the planner, can they approve their own proposal?"),
      c(me, "2026-11-07T16:30:00Z", "No. Same user is same user, whatever their role.")] },
    { id: "T4", path: "spec.md", start: null, end: null, fileLevel: false, state: "outdated", was: "- Q: Is the budget monthly or lifetime? → A: Monthly periods within the flight.", comments: [
      c("dana-k", "2026-11-07T11:48:00Z", "Monthly periods seem wrong for campaigns that run six weeks.")] },
    { id: "T5", path: "spec.md", start: 210, end: 210, fileLevel: false, state: "pending", comments: [
      c(me, "", "This restates the delivery assumption from Clarifications. Link to it instead, so the two can't drift apart.", true)] },
  ];
}
