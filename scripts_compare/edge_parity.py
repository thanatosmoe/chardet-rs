#!/usr/bin/env python3
"""Targeted edge-case parity suite against Python chardet 7."""
from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)
import chardet  # noqa: E402

OUT = Path("/tmp/opencode/edge_corpus")
RUST = os.environ.get("CHARDET_BIN", "target/debug/chardetect")

cases: list[tuple[str, bytes]] = []


def add(name: str, data: bytes) -> None:
    cases.append((name, data))


def enc(text: str, encoding: str) -> None:
    try:
        digest = hashlib.md5(f"{encoding}:{text}".encode()).hexdigest()[:10]
        add(f"enc_{encoding}_{digest}", text.encode(encoding))
    except Exception:  # noqa: BLE001
        pass


# --- BOMs ---
add("bom_utf8sig", "\ufeffHello world".encode("utf-8-sig"))
add("bom_utf16le", "Hello world".encode("utf-16"))  # BOM LE on LE platform
add("bom_utf16be", b"\xfe\xff" + "Hello world".encode("utf-16-be"))
add("bom_utf32le", b"\xff\xfe\x00\x00" + "Hello world".encode("utf-32-le"))
add("bom_utf32be", b"\x00\x00\xfe\xff" + "Hello world".encode("utf-32-be"))
add("bom_utf32_badpayload", b"\xff\xfe\x00\x00" + b"abc")
add("bom_utf7", "\ufeffHello".encode("utf-7"))

# --- Markup declarations ---
for declared in ["utf-8", "iso-8859-1", "windows-1252", "shift_jis", "euc-kr"]:
    body = f"<html><head><meta charset=\"{declared}\"></head><body>caf\xe9</body></html>"
    try:
        add(f"html_{declared}", body.encode("ascii", "replace"))
    except Exception:  # noqa: BLE001
        pass
add("xml_latin1", '<?xml version="1.0" encoding="ISO-8859-1"?><root>caf\xe9</root>'.encode("latin-1"))
add("pep263", "#!/usr/bin/env python3\n# -*- coding: latin-1 -*-\nprint('caf\xe9')\n".encode("latin-1"))
add("pep263_line3_ignored", b"# comment\n# comment\n# -*- coding: latin-1 -*-\n")

# Markup superset promotion: Shift_JIS declared page that actually uses CP932.
try:
    sjis_promo = "<html><meta charset=\"shift_jis\">\u2160\u2161\u2162</html>".encode("cp932")
    add("html_shiftjis_promote", sjis_promo)
except Exception:  # noqa: BLE001
    pass
try:
    kr_promo = "<html><meta charset=\"euc-kr\">\ud55c\uad6d\uc5b4</html>".encode("cp949")
    add("html_euckr_promote", kr_promo)
except Exception:  # noqa: BLE001
    pass

# --- EBCDIC ---
for e in ["cp037", "cp500", "cp1026", "cp273", "cp875", "cp1140"]:
    enc("The quick brown fox jumps over the lazy dog " * 3, e)
enc(f"<html><head><meta charset=\"cp037\"></head><body>{'Hello world ' * 5}</body></html>", "cp037")

# --- Niche Latin demotion ---
enc("Le café est une boisson très populaire en France et dans le monde entier.", "cp1252")
enc("Le café est une boisson très populaire en France et dans le monde entier.", "iso8859-1")
enc("Le café est une boisson très populaire en France et dans le monde entier.", "iso8859-10")
enc("Les gallois aiment le rugby et la bière dans les pubs de Cardiff.", "iso8859-14")
enc("Türkçe metinlerde ğüşiöç karakterleri bulunur ve sık kullanılır.", "cp1254")
enc("The HP-UX terminal prints text with ¼ and ½ symbols on the screen.", "hp-roman8")

# --- KOI8-T promotion ---
enc("Тоҷикӣ забон дар Осиёи Марказӣ гуфтугӯ мешавад ва хеле зебост.", "koi8-t")
enc("Русский текст для проверки кодировки КОИ8 и её вариантов.", "koi8-r")

# --- CR line endings (classic Mac) ---
add("cr_mac", b"Line one\rLine two\rLine three\rLine four\r")
add("crlf", b"Line one\r\nLine two\r\nLine three\r\n")
add("cr_short", b"a\rb")

# --- Truncation of multibyte text at every length ---
euc = "日本語の文字コード検出テストです。このテキストは日本語です。".encode("euc_jp")
for k in range(1, 21):
    add(f"trunc_euc_{k}", euc[:k])
utf8 = "café ☕ — произведение 日本語".encode("utf-8")
for k in range(1, 25):
    add(f"trunc_utf8_{k}", utf8[:k])
gb = "中文字符编码检测测试文本，这是测试内容。".encode("gb18030")
for k in range(1, 20):
    add(f"trunc_gb_{k}", gb[:k])

# --- Short inputs for many encodings ---
short_texts = {
    "ascii": "Hi",
    "utf-8": "é",
    "cp1252": "é",
    "iso8859-5": "я",
    "koi8-r": "я",
    "cp1251": "я",
    "cp932": "あ",
    "euc_jis_2004": "あ",
    "gb18030": "中",
    "big5hkscs": "中",
    "cp949": "한",
    "euc_kr": "한",
}
for e, t in short_texts.items():
    enc(t, e)
    enc(t * 2, e)

# --- Magic numbers ---
add("png", b"\x89PNG\r\n\x1a\n" + b"\x00" * 64)
add("jpeg", b"\xff\xd8\xff\xe0" + b"\x00" * 64)
add("gif", b"GIF89a" + b"\x00" * 64)
add("pdf", b"%PDF-1.7\n" + b"\x00" * 64)
add("zip", b"PK\x03\x04" + b"\x00" * 64)
add("gzip", b"\x1f\x8b\x08\x00" + b"\x00" * 64)
add("elf", b"\x7fELF" + b"\x00" * 64)
add("wasm", b"\x00asm\x01\x00\x00\x00" + b"\x00" * 32)
add("bmp", b"BM" + b"\x00" * 64)
add("riff_wav", b"RIFF\x00\x00\x00\x00WAVE" + b"\x00" * 32)
add("form_aiff", b"FORM\x00\x00\x00\x00AIFF" + b"\x00" * 32)
add("ftyp_mp4", b"\x00\x00\x00\x18ftypmp42" + b"\x00" * 32)
add("ftyp_avif", b"\x00\x00\x00\x18ftypavif" + b"\x00" * 32)
add("ttf", b"\x00\x01\x00\x00" + b"\x00" * 64)
add("cafebabe_java", b"\xca\xfe\xba\xbe\x00\x00\x00\x3d" + b"\x00" * 32)
add("cafebabe_macho", b"\xca\xfe\xba\xbe\x00\x00\x00\x02" + b"\x00" * 32)
add("tar", b"\x00" * 257 + b"ustar\x00" + b"\x00" * 64)
add("random_binary", bytes(range(0, 256)))


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for f in OUT.iterdir():
        f.unlink()

    paths = []
    expected = {}
    for name, data in cases:
        p = OUT / f"{name}.bin"
        p.write_bytes(data)
        py = chardet.detect(data)
        expected[p.name] = (py["encoding"], py["confidence"], py["language"], py["mime_type"])
        paths.append(str(p))

    rust = {}
    BATCH = 200
    for start in range(0, len(paths), BATCH):
        import subprocess

        proc = subprocess.run(
            [RUST, "--raw", *paths[start : start + BATCH]],
            capture_output=True, text=True, check=True,
        )
        for line in proc.stdout.splitlines():
            fname, e, c, l, m = line.split("\t")
            rust[Path(fname).name] = (None if e == "None" else e, float(c), None if l == "None" else l, m)

    total = len(expected)
    enc_ok = exact = mime_ok = 0
    mismatches = []
    for name, (pe, pc, pl, pm) in expected.items():
        re_, rc, rl, rm = rust.get(name, ("MISSING", -1.0, "MISSING", "MISSING"))
        if pm == rm:
            mime_ok += 1
        if pe == re_:
            enc_ok += 1
        if pe == re_ and abs(pc - rc) < 1e-12 and pl == rl and pm == rm:
            exact += 1
        else:
            mismatches.append((name, (pe, pc, pl, pm), (re_, rc, rl, rm)))

    print(f"cases:          {total}")
    print(f"mime match:     {mime_ok}/{total} ({100*mime_ok/total:.1f}%)")
    print(f"encoding match: {enc_ok}/{total} ({100*enc_ok/total:.1f}%)")
    print(f"exact match:    {exact}/{total} ({100*exact/total:.1f}%)")
    for name, exp, got in mismatches[:50]:
        print(f"{name}\n   py   = {exp}\n   rust = {got}")
    if len(mismatches) > 50:
        print(f"... and {len(mismatches)-50} more")
    sys.exit(1 if exact != total else 0)


if __name__ == "__main__":
    main()
