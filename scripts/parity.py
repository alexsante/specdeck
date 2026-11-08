#!/usr/bin/env python3
"""Compare the Rust parser's JSON with the Python reference.

Usage: python3 -I scripts/parity.py python.json rust.json
Exits 1 and prints the first differences when they disagree.
"""
import json
import sys


def diff(a, b, path, out):
    if len(out) >= 25:
        return
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a or k not in b:
                out.append(f"{path}.{k}: only in {'rust' if k in b else 'python'}")
            else:
                diff(a[k], b[k], f"{path}.{k}", out)
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            out.append(f"{path}: length {len(a)} (python) vs {len(b)} (rust)")
        for i, (x, y) in enumerate(zip(a, b)):
            diff(x, y, f"{path}[{i}]", out)
    elif a != b:
        out.append(f"{path}: {a!r} (python) vs {b!r} (rust)")


def main(py_path, rs_path):
    py, rs = json.load(open(py_path)), json.load(open(rs_path))
    py.pop("generated", None)
    rs.pop("generated", None)
    # Name features by id so differences are readable.
    for d in (py, rs):
        d["features"] = {f["id"]: f for f in d["features"]}
    out = []
    diff(py, rs, "", out)
    if out:
        print("\n".join(out))
        sys.exit(1)
    print(f"identical: {len(py['features'])} features")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
