#!/usr/bin/env python3
"""Dump detect() results for every file in a directory as TSV."""
import sys
from pathlib import Path

import chardet

d = Path(sys.argv[1])
for p in sorted(d.iterdir()):
    if not p.is_file():
        continue
    r = chardet.detect(p.read_bytes())
    print(f"{p.name}\t{r['encoding']}\t{r['confidence']}\t{r['language']}\t{r['mime_type']}")
