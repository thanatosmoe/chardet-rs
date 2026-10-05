#!/usr/bin/env python3
"""Full-corpus regression: compare the Rust port against Python chardet 7.

Usage:
    CHARDET_SRC=/path/to/chardet/src python3 full_corpus.py /path/to/test-data

Environment:
    CHARDET_SRC   directory to prepend to sys.path (reference chardet source)
    CHARDET_BIN   path to the Rust `chardetect` binary
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)
import chardet  # noqa: E402

RUST = os.environ.get("CHARDET_BIN", "target/debug/chardetect")
BATCH = 300


def main() -> None:
    data_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("tests/data")
    files = sorted(p for p in data_dir.rglob("*") if p.is_file())
    print(f"corpus: {data_dir} ({len(files)} files)")

    # Python reference results.
    expected: dict[str, tuple] = {}
    for p in files:
        try:
            data = p.read_bytes()
        except OSError as e:
            print(f"skip {p}: {e}")
            continue
        r = chardet.detect(data)
        expected[str(p)] = (r["encoding"], r["confidence"], r["language"], r["mime_type"])

    # Rust results (batched).
    rust: dict[str, tuple] = {}
    paths = list(expected)
    for start in range(0, len(paths), BATCH):
        proc = subprocess.run(
            [RUST, "--raw", *paths[start : start + BATCH]],
            capture_output=True, text=True,
        )
        for line in proc.stdout.splitlines():
            name, e, c, l, m = line.split("\t")
            rust[name] = (
                None if e == "None" else e,
                float(c),
                None if l == "None" else l,
                m,
            )

    total = len(expected)
    enc_ok = exact = mime_ok = 0
    mismatches: list[tuple] = []
    for name, exp in expected.items():
        got = rust.get(name, ("MISSING", -1.0, "MISSING", "MISSING"))
        if exp[3] == got[3]:
            mime_ok += 1
        if exp[0] == got[0]:
            enc_ok += 1
        if exp == got or (exp[0] == got[0] and abs(exp[1] - got[1]) < 1e-12 and exp[2] == got[2] and exp[3] == got[3]):
            exact += 1
        else:
            mismatches.append((name, exp, got))

    print(f"mime match:     {mime_ok}/{total} ({100*mime_ok/total:.2f}%)")
    print(f"encoding match: {enc_ok}/{total} ({100*enc_ok/total:.2f}%)")
    print(f"exact match:    {exact}/{total} ({100*exact/total:.2f}%)")
    for name, exp, got in mismatches[:60]:
        print(f"{name}\n   py   = {exp}\n   rust = {got}")
    if len(mismatches) > 60:
        print(f"... and {len(mismatches)-60} more")
    sys.exit(1 if exact != total else 0)


if __name__ == "__main__":
    main()
