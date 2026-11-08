#!/usr/bin/env python3
"""Build the PR-review mockup: review.html + template.html's styles + a real spec.md.

Usage: python3 -I prototype/build_review.py <repo> <spec-dir> <pr-number> <head-sha> <files> <spec-files>
Writes prototype/review-mockup.html (git-ignored: it contains the repo's spec text).
"""
import json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
repo, spec_dir, pr, head, files, spec_files = sys.argv[1:7]
css = re.search(r"<style>(.*?)</style>", open(os.path.join(HERE, "template.html"), encoding="utf-8").read(), re.S).group(1)
spec = open(os.path.join(repo, spec_dir, "spec.md"), encoding="utf-8").read()
branch = subprocess.run(["git", "-C", repo, "branch", "--show-current"], capture_output=True, text=True).stdout.strip()
remote = subprocess.run(["git", "-C", repo, "remote", "get-url", "origin"], capture_output=True, text=True).stdout.strip()
slug = re.sub(r"(\.git)?$", "", remote.split("github.com")[-1].lstrip(":/"))
me = subprocess.run(["gh", "api", "user", "-q", ".login"], capture_output=True, text=True).stdout.strip() or "you"
data = {"repo": slug, "branch": branch, "dir": spec_dir, "spec": spec, "me": me,
        "pr": {"number": int(pr), "head": head[:7], "files": int(files), "specFiles": int(spec_files)}}
html = open(os.path.join(HERE, "review.html"), encoding="utf-8").read()
html = html.replace("/*__CSS__*/", css).replace("/*__DATA__*/null", json.dumps(data, ensure_ascii=False).replace("</", "<\\/"))
out = os.path.join(HERE, "review-mockup.html")
open(out, "w", encoding="utf-8").write(html)
print(f"{out}: {len(html.encode()) / 1024:.0f} KB")
