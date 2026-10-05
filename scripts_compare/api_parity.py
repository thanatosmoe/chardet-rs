#!/usr/bin/env python3
"""Compare detect_all and UniversalDetector between Python chardet 7 and the
Rust extension module."""
from __future__ import annotations

import importlib.util
import os
import random
import sys
from pathlib import Path

# Python chardet 7 (from CHARDET_SRC, else the installed package).
CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)
import chardet as py_chardet  # noqa: E402

# Rust extension under a different name.
SO = os.environ.get("CHARDET_SO", "target/debug/libchardet.so")
if not Path(SO).exists():
    print(f"skip: extension not found at {SO} (set CHARDET_SO)")
    sys.exit(0)
spec = importlib.util.spec_from_file_location("chardet_rs", SO)
rs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rs)

WORDS = ["the", "quick", "café", "naïve", "über", "señor", "русский", "текст",
         "日本語", "한국어", "中文", "hello", "détection", "Straße", "piñata"]
ENCODINGS = ["ascii", "utf-8", "cp1252", "iso8859-1", "iso8859-5", "iso8859-7",
             "koi8-r", "cp1251", "cp1250", "mac-roman", "gb18030", "cp932",
             "euc_jis_2004", "cp949", "euc_kr", "cp1140", "tis-620"]

rng = random.Random(99)
cases = []
for _ in range(300):
    enc = rng.choice(ENCODINGS)
    n = rng.randint(1, 30)
    text = " ".join(rng.choice(WORDS) for _ in range(n))
    try:
        cases.append(text.encode(enc))
    except Exception:  # noqa: BLE001
        continue
for _ in range(100):
    cases.append(bytes(rng.randint(0, 255) for _ in range(rng.randint(0, 200))))

# --- detect_all ---
all_mismatch = 0
for i, data in enumerate(cases):
    a = py_chardet.detect_all(data)
    b = rs.detect_all(data)
    a_norm = [{k: v for k, v in d.items()} for d in a]
    b_norm = [{k: v for k, v in d.items()} for d in b]
    # Compare encoding/lang/conf per position (mime can differ if not returned).
    if len(a_norm) != len(b_norm):
        all_mismatch += 1
        if all_mismatch <= 10:
            print(f"detect_all len mismatch #{i}: py={len(a_norm)} rs={len(b_norm)}")
        continue
    for x, y in zip(a_norm, b_norm):
        if x["encoding"] != y["encoding"] or x["language"] != y["language"] or abs(x["confidence"] - y["confidence"]) > 1e-12:
            all_mismatch += 1
            if all_mismatch <= 10:
                print(f"detect_all mismatch #{i}: py={x} rs={y}")
            break

# --- UniversalDetector ---
ud_mismatch = 0
for i, data in enumerate(cases):
    d = py_chardet.UniversalDetector()
    d.feed(data)
    a = d.close()
    e = rs.UniversalDetector()
    e.feed(data)
    b = e.close()
    if a["encoding"] != b["encoding"] or a["language"] != b["language"] or abs(a["confidence"] - b["confidence"]) > 1e-12:
        ud_mismatch += 1
        if ud_mismatch <= 10:
            print(f"UD mismatch #{i}: py={a} rs={b}")

print(f"cases: {len(cases)}")
print(f"detect_all mismatches: {all_mismatch}")
print(f"UniversalDetector mismatches: {ud_mismatch}")
sys.exit(1 if (all_mismatch or ud_mismatch) else 0)
