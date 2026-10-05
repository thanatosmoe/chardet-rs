#!/usr/bin/env python3
"""Generate DBCS validity tables from CPython decoders.

Emits `chardet/src/codecs/dbcs.bin` (format DBV3):

    header: b"DBV3"
    for each encoding in ORDER:
        256 bytes  valid_single[b]  (byte starts and completes a char alone)
        256 bytes  live1[b]         (byte alone is a valid but incomplete prefix)
        65536 bytes valid_pair[L*256+t] (L,t completes a char as a pair)
    euc_jis_2004 SS3 table: 65536 bytes ss3[t1*256+t2]
        complete(0x8F, t1, t2)
    euc_jp SS3 table: 65536 bytes ss3[t1*256+t2]
        complete(0x8F, t1, t2)
    gb18030 4-byte bitset: (1260*1260+7)//8 bytes, bit index = base*1260+low
        where base=(L-0x81)*10+(b2-0x30), low=(b3-0x81)*10+(b4-0x30)
"""
from __future__ import annotations

import codecs
import sys

ORDER = [
    "cp932",
    "cp949",
    "big5hkscs",
    "euc_jis_2004",
    "euc_kr",
    "shift_jis_2004",
    "johab",
    "gb18030",
    # Python codecs whose semantics differ from the *_2004 registry entries;
    # needed for markup superset promotion and public-name decode checks.
    "shift_jis",
    "euc_jp",
    # Python's plain big5 codec (narrower than registry's big5hkscs).
    "big5",
]


def probe(enc: str) -> tuple[bytes, bytes, bytes]:
    dec = codecs.getincrementaldecoder(enc)

    def status(buf: bytes) -> tuple[bool, bool]:
        """Return (decodes, complete)."""
        d = dec()
        try:
            d.decode(buf, final=False)
        except UnicodeError:
            return (False, False)
        pending, _state = d.getstate()
        return (True, len(pending) == 0)

    single = bytearray(256)
    live1 = bytearray(256)
    for b in range(256):
        ok, complete = status(bytes([b]))
        live1[b] = 1 if ok else 0
        single[b] = 1 if complete else 0

    pair = bytearray(65536)
    for lead in range(256):
        if single[lead]:
            continue
        for trail in range(256):
            _ok, complete = status(bytes([lead, trail]))
            if complete:
                pair[(lead << 8) | trail] = 1
    return bytes(single), bytes(live1), bytes(pair)


def euc_ss3(enc: str) -> bytes:
    table = bytearray(65536)
    for t1 in range(256):
        for t2 in range(256):
            try:
                bytes([0x8F, t1, t2]).decode(enc)
            except UnicodeError:
                continue
            table[(t1 << 8) | t2] = 1
    return bytes(table)


def gb18030_four() -> bytes:
    n = 1260 * 1260
    bits = bytearray((n + 7) // 8)
    for base in range(1260):
        L = 0x81 + base // 10
        b2 = 0x30 + base % 10
        for low in range(1260):
            b3 = 0x81 + low // 10
            b4 = 0x30 + low % 10
            try:
                bytes([L, b2, b3, b4]).decode("gb18030")
            except UnicodeError:
                continue
            p = base * 1260 + low
            bits[p >> 3] |= 1 << (p & 7)
    return bytes(bits)


def main() -> None:
    out = bytearray(b"DBV3")
    for enc in ORDER:
        s, l, p = probe(enc)
        out += s
        out += l
        out += p
    out += euc_ss3("euc_jis_2004")
    out += euc_ss3("euc_jp")
    out += gb18030_four()
    sys.stdout.buffer.write(bytes(out))


if __name__ == "__main__":
    main()
