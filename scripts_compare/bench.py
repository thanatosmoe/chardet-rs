#!/usr/bin/env python3
"""Throughput benchmark: Rust port vs Python chardet 7 over a corpus.

Usage:
    CHARDET_SRC=/path/to/chardet/src python3 bench.py [data-dir]

Environment:
    CHARDET_SRC   prepend to sys.path (reference chardet source)
    CHARDET_BIN   path to the Rust `chardetect` binary (default target/release/chardetect)
"""
from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

CHARDET_SRC = os.environ.get("CHARDET_SRC")
if CHARDET_SRC:
    sys.path.insert(0, CHARDET_SRC)
import chardet  # noqa: E402

RUST = os.environ.get("CHARDET_BIN", "target/release/chardetect")
MAX_BYTES = 200_000


def human(n: float) -> str:
    return f"{n:,.0f}"


def main() -> None:
    data_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("tests/data")
    files = sorted(p for p in data_dir.rglob("*") if p.is_file())
    if not files:
        print(f"no files under {data_dir}")
        sys.exit(1)

    # Preload bytes so I/O is not measured (both detectors read the same bytes).
    payloads: list[bytes] = []
    total_bytes = 0
    for p in files:
        try:
            data = p.read_bytes()[:MAX_BYTES]
        except OSError:
            continue
        payloads.append(data)
        total_bytes += len(data)

    # --- Python chardet ---
    t0 = time.perf_counter()
    for data in payloads:
        chardet.detect(data)
    py_elapsed = time.perf_counter() - t0

    # --- Rust port (single process over all files) ---
    paths = [str(p) for p in files]
    t0 = time.perf_counter()
    subprocess.run(
        [RUST, "--raw", *paths],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )
    rs_elapsed = time.perf_counter() - t0

    n = len(payloads)
    mib = total_bytes / (1024 * 1024)

    def row(label: str, elapsed: float) -> None:
        print(
            f"{label:<12} {elapsed:7.3f} s  {human(n / elapsed):>9} files/s  "
            f"{mib / elapsed:8.1f} MiB/s"
        )

    print(f"corpus: {data_dir}  ({n} files, {mib:.1f} MiB, capped at {MAX_BYTES} B/file)\n")
    row("python", py_elapsed)
    row("rust", rs_elapsed)
    print(f"\nspeedup: {py_elapsed / rs_elapsed:.1f}x")


if __name__ == "__main__":
    main()
