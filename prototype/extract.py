#!/usr/bin/env python3
"""Prototype Spec Kit parser.

Reads a Spec Kit repo and emits JSON describing every feature folder under
specs/. Doubles as executable parser rules for the future Swift SpecKitCore
package: every format variation found in the reference repo is handled here.

Usage: python3 -I extract.py <repo> <out.json>
"""
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone

# ---------------------------------------------------------------- helpers

def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except (FileNotFoundError, IsADirectoryError, UnicodeDecodeError):
        return None


def strip_md(s):
    s = re.sub(r"\*\*(.+?)\*\*", r"\1", s)
    s = re.sub(r"`([^`]+)`", r"\1", s)
    s = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", s)
    return s.strip()


def clip(s, n):
    s = strip_md(s)
    return s if len(s) <= n else s[: n - 1].rstrip() + "…"


def sections(text, level):
    """Split markdown into (heading, body) at the given heading level."""
    pat = re.compile(r"^" + "#" * level + r" (.+)$", re.M)
    out, matches = [], list(pat.finditer(text))
    for i, m in enumerate(matches):
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        # stop at a heading of a higher level
        body = text[m.end():end]
        hi = re.search(r"^#{1," + str(level - 1) + r"} ", body, re.M)
        if hi:
            body = body[: hi.start()]
        out.append((m.group(1).strip(), body))
    return out


def git(repo, *args):
    try:
        return subprocess.run(["git", "-C", repo, *args], capture_output=True,
                              text=True, timeout=20).stdout.strip()
    except Exception:
        return ""

# ---------------------------------------------------------------- spec.md

HEADER_RE = re.compile(r"^\*\*([A-Za-z ][A-Za-z ]*?)\*\*:\s*(.+)$", re.M)
STORY_RE = re.compile(
    r"^###\s+(?:User Story|US)\s*(\d+)\s*[-–—:]?\s*(.*?)\s*\((?:Priority:\s*)?(P\d)\)\s*$", re.M)
MARKER_RE = re.compile(r"\[NEEDS CLARIFICATION:\s*([^\]]+)\]")
QA_RE = re.compile(r"^\s*-\s*(?:\*\*)?Q\d*(?:\*\*)?:\s*(.+?)\s*(?:→|->)\s*(?:\*\*)?A\d*(?:\*\*)?:\s*(.+)$", re.M)


def session_kind(heading):
    h = heading.lower()
    if "flagged" in h or "conflict" in h:
        return "flagged"
    if "no human" in h or "without a human" in h:
        return "agent"
    # A human confirming or deciding wins over an agent having proposed.
    # Whole words only: "lock" must not match "blocking".
    if re.search(r"\b(human|locks?|locked|product decisions?|user-approved|accepted|binding)\b", h):
        return "human"
    if "analyst default" in h or "agent" in h or "clarify pass" in h:
        return "agent"
    return "unlabeled"


STORY_FIELD_RE = re.compile(r"^\*\*(Why this priority|Independent Test|Acceptance Scenarios)\*\*:?", re.M)


def parse_story_body(body):
    """Description, why-this-priority, independent test and numbered
    acceptance scenarios. Markdown (bold, code) is kept for display."""
    first = STORY_FIELD_RE.search(body)
    desc = (body[: first.start()] if first else body).strip()
    why = re.search(r"^\*\*Why this priority\*\*:\s*(.+)$", body, re.M)
    test = re.search(r"^\*\*Independent Test\*\*:\s*(.+)$", body, re.M)
    acc = re.search(r"^\*\*Acceptance Scenarios\*\*:?", body, re.M)
    scenarios = re.findall(r"^\d+\.\s+(.+)$", body[acc.end():], re.M) if acc else []
    cut = lambda x, n: x if len(x) <= n else x[: n - 1].rstrip() + "…"
    return {"description": cut(desc, 900), "why": cut(why.group(1).strip(), 500) if why else None,
            "test": cut(test.group(1).strip(), 600) if test else None,
            "scenarios": [cut(x.strip(), 600) for x in scenarios]}


def parse_spec(text):
    if text is None:
        return None
    h1 = re.search(r"^# (.+)$", text, re.M)
    title = h1.group(1) if h1 else ""
    title = re.sub(r"^Feature Specification:\s*", "", title).strip()

    head = text[: (re.search(r"^## ", text, re.M) or re.search(r"\Z", text)).start()]
    header = {k.strip().lower(): v.strip() for k, v in HEADER_RE.findall(head)}
    clickup = None
    m = re.search(r"https://app\.clickup\.com/t/\w+", header.get("clickup", "") + header.get("input", ""))
    if m:
        clickup = m.group(0)

    stories = []
    for m in STORY_RE.finditer(text):
        rest = text[m.end():]
        stop = re.search(r"^(#{1,3} |---\s*$)", rest, re.M)
        stories.append({"n": int(m.group(1)), "title": clip(m.group(2), 90), "priority": m.group(3),
                        "body": parse_story_body(rest[: stop.start()] if stop else rest)})

    sessions = []
    clar = next((b for h, b in sections(text, 2) if h.lower().startswith("clarifications")), "")
    for h, b in sections("\n" + clar, 3):
        if not h.lower().startswith("session"):
            continue
        date = re.search(r"\d{4}-\d{2}-\d{2}", h)
        label = re.sub(r"^Session\s*\d{4}-\d{2}-\d{2}\s*[:—–-]?\s*", "", h).strip()
        label = re.sub(r"^\((.*)\)$", r"\1", label)
        qa = [{"q": clip(q, 220), "a": clip(a, 260)} for q, a in QA_RE.findall(b)]
        sessions.append({"date": date.group(0) if date else None,
                         "label": strip_md(label) or "Session",
                         "kind": session_kind(h), "qa": qa})

    return {
        "title": title,
        "status": clip(header.get("status", ""), 160),
        "branch": strip_md(header.get("feature branch", "")) or None,
        "created": header.get("created"),
        "input": clip(header.get("input", ""), 400),
        "clickup": clickup,
        "stories": stories,
        "fr": len(re.findall(r"\*\*FR-\d+", text)),
        "nfr": len(re.findall(r"\*\*NFR-\d+", text)),
        "sc": len(re.findall(r"\*\*SC-\d+", text)),
        # Markers inside code (`…` or fenced blocks) only mention the syntax.
        "markers": [clip(m, 200) for m in MARKER_RE.findall(
            re.sub(r"`[^`\n]*`", "", re.sub(r"(?ms)^```.*?^```", "", text)))],
        "sessions": sessions,
    }

# ---------------------------------------------------------------- tasks.md

TASK_RE = re.compile(r"^\s*-\s*\[([ xX])\]\s*(?:\*\*)?(T\d+)(?:\*\*)?\s*(.*)$")
TAG_RE = re.compile(r"^\[([^\]]+)\]\s*")


FILE_SPAN_RE = re.compile(r"`([^`\s]+)`")


def task_files(text):
    """Code spans that look like paths: contain a slash or end in a file extension."""
    out = []
    for span in FILE_SPAN_RE.findall(text):
        span = span.rstrip(".,:;")
        if ("/" in span or re.search(r"\.[A-Za-z]{1,5}$", span)) and not span.startswith("http") and span not in out:
            out.append(span)
    return out[:10]


def parse_tasks(text):
    if text is None:
        return None
    phases, cur = [], None
    for line in text.splitlines():
        hm = re.match(r"^(#{2,3}) (.+)$", line)
        if hm:
            h = hm.group(2).strip()
            if len(hm.group(1)) == 2:
                cur = {"title": h, "tasks": []} if re.match(r"Phase\b", h, re.I) else None
                if cur:
                    phases.append(cur)
            continue
        tm = TASK_RE.match(line)
        if not tm:
            continue
        if cur is None:  # task outside any Phase heading
            cur = {"title": "Other", "tasks": []}
            phases.append(cur)
        rest, tags = tm.group(3), []
        while True:
            t = TAG_RE.match(rest)
            if not t:
                break
            tags.append(t.group(1))
            rest = rest[t.end():]
        cur["tasks"].append({
            "id": tm.group(2), "done": tm.group(1) != " ",
            "parallel": "P" in tags,
            "story": next((t for t in tags if re.fullmatch(r"US\d+", t)), None),
            "tags": [t for t in tags if t != "P" and not re.fullmatch(r"US\d+", t)],
            "text": clip(rest, 160),
            "raw": rest.strip() if len(rest.strip()) <= 1200 else rest.strip()[:1199] + "…",
            "files": task_files(rest),
        })
    phases = [p for p in phases if p["tasks"]]
    for p in phases:
        p["title"] = re.sub(r"^Phase\s*\d+\s*[:—–-]\s*", "", p["title"])
        p["total"] = len(p["tasks"])
        p["done"] = sum(t["done"] for t in p["tasks"])
        p["deferred"] = bool(re.match(r"deferred\b", p["title"], re.I))
    return phases

# ---------------------------------------------------------------- review.md

SEVERITIES = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "INFO"]
SEV_ALIASES = {"BLOCKER": "CRITICAL", "MAJOR": "HIGH", "MODERATE": "MEDIUM", "MINOR": "LOW",
               "TRIVIAL": "INFO", "NIT": "INFO", "NOTE": "INFO"}
RESOLVED = {"resolved", "closed", "fixed", "n/a", "accepted", "addressed", "done", "verified"}
# "first pass", "second pass", "detection passes" are pass *names*, not verdicts.
PASS_NAME_RE = re.compile(r"\b(first|second|third|fourth|final|next|later|prior|detection|\d+(?:st|nd|rd|th)?)\s+pass(?:es)?\b", re.I)


def find_verdict(text):
    t = PASS_NAME_RE.sub("", text)
    m = (re.search(r"verdict[^\n]*?\b(pass|fail)\b", t, re.I)
         or re.search(r"verdict[^\n]*\n+\s*\**\s*(pass|fail)\b", t, re.I))
    return m.group(1).upper() if m else None


def gate_of(heading):
    if re.search(r"\bREVIEW\b|post-?implementation", heading):
        return "Review"
    if re.search(r"\bANALYZE\b|pre-?implementation", heading, re.I):
        return "Analyze"
    return None


def parse_findings(text):
    out, lines, i = [], text.splitlines(), 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("|") and re.search(r"\|\s*Severity\s*\|", line, re.I):
            cols = [c.strip().lower() for c in line.strip().strip("|").split("|")]
            i += 2  # skip separator row
            while i < len(lines) and lines[i].startswith("|"):
                cells = [c.strip() for c in lines[i].strip().strip("|").split("|")]
                row = dict(zip(cols, cells))
                sev = strip_md(row.get("severity", "")).upper().split(" ")[0] if row.get("severity") else ""
                sev = SEV_ALIASES.get(sev, sev)
                if sev in SEVERITIES:
                    desc = next((row[k] for k in ("finding", "summary", "description", "issue", "evidence") if row.get(k)), "")
                    fix = next((row[k] for k in ("recommended fix", "recommendation", "suggestion", "resolution") if row.get(k)), "")
                    status = strip_md(row.get("status", "")) or None
                    full = desc + " " + fix
                    out.append({"id": strip_md(row.get("id", "")), "severity": sev,
                                "tasks": sorted(set(re.findall(r"\bT\d{3}\b", full))),
                                "stories": sorted(set(re.findall(r"\bUS\d+\b", full))),
                                "owner": strip_md(row.get("owner", "")) or None, "status": status,
                                "resolved": bool(status and status.split()[0].lower().strip(".,:") in RESOLVED),
                                "text": clip(desc, 300), "fix": clip(fix, 240)})
                i += 1
            continue
        i += 1
    return out


def parse_review(text):
    """Split review.md into passes. A pass starts at a ## section that states a
    verdict; sections without a verdict (e.g. "## Findings") attach to the
    pass before them. The last pass is the current one."""
    if text is None:
        return None
    title = re.search(r"^# (.+)$", text, re.M)
    title = strip_md(title.group(1)) if title else "Review"
    parts = re.split(r"(?m)^(?=## )", text)
    preamble, secs = parts[0], parts[1:]
    passes = []
    for sec in secs:
        heading = sec.splitlines()[0][3:].strip()
        v = find_verdict(sec)
        f = parse_findings(sec)
        if v or not passes:
            date = re.search(r"\d{4}-\d{2}-\d{2}", heading) or re.search(r"^\*\*Date\*\*:\s*(\d{4}-\d{2}-\d{2})", sec, re.M)
            passes.append({"label": strip_md(heading), "gate": gate_of(heading) or gate_of(title),
                           "verdict": v, "date": (date.group(1) if date.re.groups else date.group(0)) if date else None,
                           "findings": f})
        else:
            passes[-1]["findings"] += f
            passes[-1]["verdict"] = passes[-1]["verdict"] or v
    pre_date = re.search(r"^\*\*Date\*\*:\s*(\d{4}-\d{2}-\d{2})", preamble, re.M)
    if not passes:
        passes = [{"label": title, "gate": gate_of(title), "verdict": find_verdict(preamble),
                   "date": pre_date.group(1) if pre_date else None, "findings": []}]
    elif passes[0]["date"] is None and pre_date:
        passes[0]["date"] = pre_date.group(1)  # the review's header date belongs to its first pass
    # A preamble "Current verdict" line wins for the overall verdict.
    cur = re.search(r"current verdict[^:]*:\s*\**\s*(pass|fail)", PASS_NAME_RE.sub("", preamble), re.I)
    latest = passes[-1]
    verdict = latest["verdict"] or (cur.group(1).upper() if cur else None) or find_verdict(preamble)
    for p in passes:  # dedupe IDs within a pass (keep the last mention)
        seen = {}
        for x in p["findings"]:
            seen[x["id"] or x["text"]] = x
        p["findings"] = list(seen.values())
    return {"title": title, "verdict": verdict, "passes": passes}

# ---------------------------------------------------------------- checklists

def parse_checklists(d):
    out = []
    cdir = os.path.join(d, "checklists")
    if not os.path.isdir(cdir):
        return out
    for name in sorted(os.listdir(cdir)):
        text = read(os.path.join(cdir, name)) or ""
        items = re.findall(r"^\s*-\s*\[([ xX])\]", text, re.M)
        out.append({"name": name, "total": len(items),
                    "done": sum(c != " " for c in items)})
    return out

# ---------------------------------------------------------------- stage

# Pipeline order. "Fragment" = folder without spec.md (sits outside the pipeline).
STAGES = ["Specified", "Clarified", "Planned", "Tasked", "Analyzed", "Building", "Done"]


STATUS_STAGE = [  # Status-line prefixes trusted when tasks.md is missing (human decision 2026-11-08)
    (r"(implemented|delivered|shipped|done|complete)", "Done"),
    (r"in progress", "Building"),
]


def derive_stage(f):
    """Returns (stage, source). Source is "artifacts" or "status"."""
    s, files = f["spec"], set(f["files"])
    tasks = f["tasks"] or []
    total = sum(p["total"] for p in tasks)
    done = sum(p["done"] for p in tasks)
    if s is None:
        return "Fragment", "artifacts"
    if total and done == total:
        return "Done", "artifacts"
    if total and done:
        return "Building", "artifacts"
    if not total:
        st = re.sub(r"^[*\s]+", "", s["status"] or "").lower()
        for pat, stage in STATUS_STAGE:
            if re.match(pat, st):
                return stage, "status"
    if "review.md" in files and total:
        return "Analyzed", "artifacts"
    if total:
        return "Tasked", "artifacts"
    if "plan.md" in files:
        return "Planned", "artifacts"
    if s["sessions"]:
        return "Clarified", "artifacts"
    return "Specified", "artifacts"

# ---------------------------------------------------------------- main

def main(repo, out):
    specs_dir = os.path.join(repo, "specs")
    features = []
    for name in sorted(os.listdir(specs_dir)):
        d = os.path.join(specs_dir, name)
        if not os.path.isdir(d):
            continue
        m = re.match(r"^(\d+)-(.+)$", name)
        files = sorted(os.listdir(d))
        contracts = sorted(os.listdir(os.path.join(d, "contracts"))) if "contracts" in files else []
        f = {
            "id": name,
            "ticket": m.group(1) if m else name,
            "slug": m.group(2) if m else name,
            "files": files,
            "contracts": contracts,
            "spec": parse_spec(read(os.path.join(d, "spec.md"))),
            "tasks": parse_tasks(read(os.path.join(d, "tasks.md"))),
            "review": parse_review(read(os.path.join(d, "review.md"))),
            "checklists": parse_checklists(d),
            "lastCommit": git(repo, "log", "-1", "--format=%cI", "--", f"specs/{name}") or None,
            "firstCommit": (git(repo, "log", "--diff-filter=A", "--format=%cs", "--", f"specs/{name}").splitlines() or [None])[-1],
        }
        f["stage"], f["stageSource"] = derive_stage(f)
        features.append(f)

    # Cross-spec references: ticket numbers (WB-12345 or 12345) that belong to another feature folder.
    tickets = {f["ticket"] for f in features}
    for f in features:
        text = read(os.path.join(specs_dir, f["id"], "spec.md")) or ""
        found = {m for m in re.findall(r"\b(?:WB-)?(\d{5})\b", text)}
        f["refs"] = sorted(t for t in found if t in tickets and t != f["ticket"])

    current = None
    fj = read(os.path.join(repo, ".specify", "feature.json"))
    if fj:
        try:
            current = json.loads(fj)
        except json.JSONDecodeError:
            pass

    data = {
        "project": os.path.basename(os.path.abspath(repo)),
        "generated": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "head": git(repo, "rev-parse", "--abbrev-ref", "HEAD"),
        "current": current,
        "stages": STAGES,
        "features": features,
    }
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(data, fh, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
