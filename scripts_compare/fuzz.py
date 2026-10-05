#!/usr/bin/env python3
"""Fuzz the Rust port against Python chardet 7 on random and structured inputs."""
from __future__ import annotations

import os
import random
import subprocess
import sys
from pathlib import Path

CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)
import chardet  # noqa: E402

OUT = Path("/tmp/opencode/fuzz_corpus")
RUST = os.environ.get("CHARDET_BIN", "target/debug/chardetect")

WORDS = [
    "the", "quick", "brown", "fox", "café", "naïve", "über", "señor", "русский",
    "текст", "日本語", "한국어", "中文", "test", "hello", "world", "encoding",
    "détection", "Straße", "größe", "ångström", " smörgås", "piñata",
]


def gen_random(rng: random.Random) -> bytes:
    n = rng.randint(0, 400)
    return bytes(rng.randint(0, 255) for _ in range(n))


def gen_text(rng: random.Random, enc: str) -> bytes | None:
    n = rng.randint(1, 40)
    words = [rng.choice(WORDS) for _ in range(n)]
    sep = rng.choice([" ", "\n", "  ", "\t"])
    text = sep.join(words)
    if rng.random() < 0.3:
        text += rng.choice(["\r", "\r\n", "\n"]) * rng.randint(1, 4)
    try:
        return text.encode(enc, errors="strict")
    except Exception:  # noqa: BLE001
        return None


def gen_cjk(rng: random.Random) -> bytes:
    """Random CJK-ish byte sequences in plausible ranges."""
    enc = rng.choice(["cp932", "euc_jis_2004", "cp949", "euc_kr", "gb18030", "big5hkscs"])
    text_pool = {
        "cp932": "日本語のテキストです、これはテストです。",
        "euc_jis_2004": "日本語のテキストです、これはテストです。",
        "cp949": "한국어 텍스트입니다. 이것은 테스트입니다.",
        "euc_kr": "한국어 텍스트입니다. 이것은 테스트입니다.",
        "gb18030": "中文字符编码检测测试文本，这是测试内容。",
        "big5hkscs": "中文字符編碼檢測測試文本，這是測試內容。",
    }[enc]
    n = rng.randint(1, 30)
    text = "".join(rng.choice(text_pool) for _ in range(n))
    return text.encode(enc, errors="replace")


def gen_bytes_range(rng: random.Random) -> bytes:
    lo = rng.choice([0x20, 0x40, 0x80, 0xA0, 0xC0])
    hi = rng.choice([0x7E, 0x9F, 0xC0, 0xFF])
    if lo > hi:
        lo, hi = hi, lo
    n = rng.randint(1, 200)
    return bytes(rng.randint(lo, hi) for _ in range(n))


ENCODINGS = [
    "ascii", "utf-8", "cp1252", "iso8859-1", "iso8859-2", "iso8859-5",
    "iso8859-7", "iso8859-15", "koi8-r", "cp1251", "cp1250", "cp1254",
    "mac-roman", "tis-620", "gb18030", "big5hkscs", "cp932", "euc_jis_2004",
    "cp949", "euc_kr", "johab", "hp-roman8", "kz1048", "ptcp154",
    "cp437", "cp850", "cp866", "cp1140", "cp500", "iso2022_jp_2",
    "utf-7", "hz", "iso2022_kr", "iso2022_jp_ext", "mac-greek", "kz1048",
]


def main() -> None:
    seed = int(sys.argv[1]) if len(sys.argv) > 1 else 12345
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 600
    rng = random.Random(seed)
    OUT.mkdir(parents=True, exist_ok=True)
    for f in OUT.iterdir():
        f.unlink()

    cases: list[tuple[str, bytes]] = []
    for i in range(count):
        kind = rng.random()
        if kind < 0.3:
            data = gen_random(rng)
        elif kind < 0.75:
            data = gen_text(rng, rng.choice(ENCODINGS))
        elif kind < 0.9:
            data = gen_cjk(rng)
        else:
            data = gen_bytes_range(rng)
        if data is None:
            continue
        cases.append((f"f{i:04d}", data))

    paths = []
    expected = {}
    for name, data in cases:
        p = OUT / f"{name}.bin"
        p.write_bytes(data)
        py = chardet.detect(data)
        expected[p.name] = (py["encoding"], py["confidence"], py["language"], py["mime_type"])
        paths.append(str(p))

    total = len(expected)
    enc_ok = 0
    exact_ok = 0
    mime_ok = 0
    mismatches = []
    BATCH = 200
    rust = {}
    for start in range(0, len(paths), BATCH):
        proc = subprocess.run(
            [RUST, "--raw", *paths[start : start + BATCH]], capture_output=True, text=True, check=True
        )
        for line in proc.stdout.splitlines():
            fname, enc, conf, lang, mime = line.split("\t")
            rust[Path(fname).name] = (enc, float(conf), lang, mime)

    for name, (pe, pc, pl, pm) in expected.items():
        re_, rc, rl, rm = rust.get(name, ("MISSING", -1.0, "MISSING", "MISSING"))
        re_ = None if re_ == "None" else re_
        rl = None if rl == "None" else rl
        if pm == rm:
            mime_ok += 1
        if pe == re_:
            enc_ok += 1
        if pe == re_ and abs(pc - rc) < 1e-12 and pl == rl and pm == rm:
            exact_ok += 1
        else:
            mismatches.append((name, (pe, pc, pl, pm), (re_, rc, rl, rm)))

    print(f"cases:          {total}")
    print(f"mime match:     {mime_ok}/{total} ({100*mime_ok/total:.1f}%)")
    print(f"encoding match: {enc_ok}/{total} ({100*enc_ok/total:.1f}%)")
    print(f"exact match:    {exact_ok}/{total} ({100*exact_ok/total:.1f}%)")
    shown = 0
    for name, exp, got in mismatches:
        print(f"{name}  py={exp}  rust={got}")
        shown += 1
        if shown >= 40:
            print(f"... and {len(mismatches)-shown} more")
            break
    sys.exit(1 if exact_ok != total else 0)


if __name__ == "__main__":
    main()
