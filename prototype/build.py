#!/usr/bin/env python3
"""Slim data.json and inline it into template.html -> mockup.html.

Usage: python3 -I build.py   (run from the prototype folder)
"""
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def clip(s, n):
    return s if not s or len(s) <= n else s[: n - 1].rstrip() + "…"


def slim(f):
    detailed = True  # artifacts allow 16 MB; every spec carries full detail
    s = f["spec"] or {}
    out = {k: f[k] for k in ("id", "ticket", "slug", "stage", "stageSource", "files", "lastCommit", "firstCommit", "refs")}
    out["contracts"] = len(f["contracts"])
    out.update({
        "title": s.get("title") or f["slug"].replace("-", " "),
        "status": s.get("status"), "branch": s.get("branch"), "created": s.get("created"),
        "clickup": s.get("clickup"), "input": clip(s.get("input"), 260),
        "fr": s.get("fr", 0), "nfr": s.get("nfr", 0), "sc": s.get("sc", 0),
        "markers": s.get("markers", []),
        "stories": [[st["n"], st["title"], st["priority"], st.get("body")] for st in s.get("stories", [])],
        "sessions": [{
            "date": se["date"], "label": clip(se["label"], 90), "kind": se["kind"], "n": len(se["qa"]),
            "qa": [[clip(q["q"], 170), clip(q["a"], 210)] for q in se["qa"]] if detailed else [],
        } for se in s.get("sessions", [])],
    })
    phases = []
    for p in f["tasks"] or []:
        ph = {"title": clip(p["title"], 80), "total": p["total"], "done": p["done"], "deferred": p["deferred"],
              "stories": sorted({t["story"] for t in p["tasks"] if t["story"]}),
              # per-story counts are needed for every spec (story progress bars)
              "byStory": {}}
        for t in p["tasks"]:
            if t["story"]:
                b = ph["byStory"].setdefault(t["story"], [0, 0])
                b[1] += 1
                b[0] += t["done"]
        if detailed:
            ph["tasks"] = [[t["id"], int(t["done"]), int(t["parallel"]), t["story"],
                            clip(t["text"], 120), t.get("raw"), t.get("files", []), t.get("tags", [])] for t in p["tasks"]]
        phases.append(ph)
    out["phases"] = phases
    nxt = next((t for p in f["tasks"] or [] for t in p["tasks"] if not t["done"]), None)
    out["next"] = [nxt["id"], clip(nxt["text"], 120)] if nxt else None
    r = f["review"]
    out["review"] = {"verdict": r["verdict"], "title": clip(r["title"], 100), "passes": [{
        "label": clip(p["label"], 90), "gate": p["gate"], "verdict": p["verdict"], "date": p.get("date"),
        "findings": [{**x, "text": clip(x["text"], 240), "fix": clip(x["fix"], 180)} for x in p["findings"]],
    } for p in r["passes"]]} if r else None
    out["checklists"] = [sum(c["done"] for c in f["checklists"]),
                         sum(c["total"] for c in f["checklists"])]
    return out


def main():
    data = json.load(open(os.path.join(HERE, "data.json"), encoding="utf-8"))
    data["features"] = [slim(f) for f in data["features"]]
    blob = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/")
    html = open(os.path.join(HERE, "template.html"), encoding="utf-8").read()
    html = html.replace("/*__DATA__*/null", blob)
    with open(os.path.join(HERE, "mockup.html"), "w", encoding="utf-8") as fh:
        fh.write(html)
    print(f"mockup.html: {len(html.encode()) / 1024:.0f} KB")
    # Artifact version: no doctype/html/head/body (the host adds the skeleton).
    import re
    head = re.search(r"<head>(.*?)</head>", html, re.S).group(1)
    head = re.sub(r'<meta charset[^>]*>\s*|<meta name="viewport"[^>]*>\s*', "", head)
    body = re.search(r"<body>(.*)</body>", html, re.S).group(1)
    with open(os.path.join(HERE, "spec-deck-mockup.html"), "w", encoding="utf-8") as fh:
        fh.write(head.strip() + "\n" + body.strip() + "\n")


if __name__ == "__main__":
    main()
