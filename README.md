# chardet-rs

[![CI](https://github.com/thanatosmoe/chardet-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/thanatosmoe/chardet-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/chardet-rs.svg)](https://crates.io/crates/chardet-rs)
[![docs.rs](https://docs.rs/chardet-rs/badge.svg)](https://docs.rs/chardet-rs)
[![PyPI](https://img.shields.io/pypi/v/chardet-rs.svg)](https://pypi.org/project/chardet-rs/)
[![License: 0BSD](https://img.shields.io/badge/License-0BSD-blue.svg)](LICENSE)

A Rust rewrite of [chardet 7](https://github.com/chardet/chardet), the
universal character encoding detector. Same detection results, native Rust
core, optional Python bindings.

This is a faithful port: it reuses chardet 7's trained bigram model artifacts
and reproduces its 13-stage detection pipeline, aiming for byte-for-byte
identical output. On the full official corpus
([chardet/test-data](https://github.com/chardet/test-data), 3,196 files) it
matches Python chardet 7 on **3,196/3,196** files — encoding, confidence
(full `f64` precision), language, and MIME type.

## Why

- **Exact parity** — identical `encoding`, `confidence`, `language`, and
  `mime_type` to chardet 7 on every corpus file.
- **Reuses the reference models** — the trained `models.bin`, `idf.bin`,
  `rowmax.bin`, and `confusion.bin` artifacts are parsed directly, so accuracy
  is chardet 7's accuracy.
- **Native Rust core** — a library crate plus a `chardetect` binary.
- **Drop-in Python bindings** — a PyO3 extension packaged as the `chardet`
  module, including the `chardet.enums`, `chardet.pipeline`, and
  `chardet.equivalences` submodules.

|  | chardet-rs | Python chardet 7 |
| --- | --- | --- |
| Corpus parity | **3,196 / 3,196 exact** | reference |
| Encoding detector | Rust core | Python (+ mypyc/Cython) |
| Python API | `detect`, `detect_all`, `UniversalDetector` | same |
| CLI | `chardetect` | `chardetect` |
| Supported encodings | 87 registry encodings | 87 |
| License | 0BSD | 0BSD |

## Layout

```
chardet/            Rust core crate (library + `chardetect` binary)
  src/codecs/       CPython-compatible codec validity/decoding
  src/models/       models.bin / rowmax.bin parsing and bigram scoring
  src/pipeline/     the 13 detection stages + orchestrator
  models/           bundled model artifacts (from chardet 7)
chardet-py/         PyO3 bindings (drop-in `chardet` Python package)
scripts_generate/   Python generators for the generated data tables
scripts_compare/    parity harnesses against Python chardet 7
```

## Installation

### Rust

```console
$ cargo add chardet-rs
```

The crate is published as `chardet-rs`; its library target is named `chardet`,
so you still `use chardet::...`:

```rust
// Cargo.toml: chardet-rs = "0.1"
use chardet::{detect, DetectOptions};
```

Or from a checkout:

```console
$ cargo build --release
$ ./target/release/chardetect --version
```

### Python

From PyPI (distribution `chardet-rs`, import module `chardet`):

```console
$ pip install chardet-rs
```

Or build from a checkout:

```console
$ python3 -m venv .venv && . .venv/bin/activate
$ pip install maturin
$ cd chardet-py && maturin build --release
$ pip install ../target/wheels/chardet_rs-*.whl
```

> **Note:** the Python distribution is named `chardet-rs` because the name
> `chardet` on PyPI belongs to the reference project. Installing it provides
> the `chardet` import module and therefore conflicts with the reference
> `chardet` package — install one or the other in a given environment.

## Quick Start

### Rust

```rust
let data = std::fs::read("unknown.txt")?;
let result = chardet::detect(&data);
println!("{:?}", result);

// All ranked candidates
let all = chardet::detect_all(&data, &chardet::DetectOptions::default())?;
```

### Python

```python
import chardet

chardet.detect(b"Python is a great programming language.")
# {'encoding': 'ascii', 'confidence': 1.0, 'language': 'en', 'mime_type': 'text/plain'}

# All candidate encodings ranked by confidence
for r in chardet.detect_all("Le café est une boisson très populaire.".encode("windows-1252")):
    print(r["encoding"], round(r["confidence"], 2))
```

### Streaming detection

```python
from chardet import UniversalDetector

detector = UniversalDetector()
with open("unknown.txt", "rb") as f:
    for line in f:
        detector.feed(line)
        if detector.done:
            break
print(detector.close())
```

### Encoding era filtering

```python
from chardet import detect_all
from chardet.enums import EncodingEra

data = "Москва является столицей Российской Федерации.".encode("windows-1251")
for r in detect_all(data, encoding_era=EncodingEra.MODERN_WEB):
    print(r["encoding"], round(r["confidence"], 2))
```

### CLI

```console
$ chardetect somefile.txt
somefile.txt: utf-8 with confidence 0.99

$ chardetect --minimal somefile.txt
utf-8

$ chardetect -l somefile.txt
somefile.txt: utf-8 en (English) with confidence 0.99
```

## How It Works

chardet-rs reproduces chardet 7's multi-stage pipeline, progressing from cheap
deterministic checks to statistical analysis:

1. BOM detection
2. BOM-less UTF-16/UTF-32 null-byte patterns
3. Escape sequences (ISO-2022, HZ-GB-2312, UTF-7)
4. Magic-number binary detection (40+ formats)
5. Binary content detection
6. Markup charset declarations (HTML/XML/PEP 263, including EBCDIC)
7. ASCII check
8. UTF-8 structural validation
9. Byte validity filtering
10. CJK gating
11. Structural probing
12. Statistical bigram scoring against the trained models
13. Post-processing rank corrections (confusion groups, niche-Latin
    demotion, KOI8-T promotion, classic-Mac line endings, decode safety)

Language detection (three tiers) and MIME type detection run alongside.

## Parity Testing

The port is validated against Python chardet 7:

```console
$ git clone --depth 1 https://github.com/chardet/test-data tests/data
$ CHARDET_SRC=/path/to/chardet/src python3 scripts_compare/full_corpus.py tests/data
mime match:     3196/3196 (100.00%)
encoding match: 3196/3196 (100.00%)
exact match:    3196/3196 (100.00%)
```

Harnesses live in `scripts_compare/`:

- `parity.py` — hand-written multilingual samples.
- `edge_parity.py` — BOMs, markup declarations, EBCDIC, niche-Latin/KOI8-T
  arbitration, CR line endings, truncation at every length, magic numbers.
- `fuzz.py` — randomized/structured byte sequences.
- `full_corpus.py` — the full `chardet/test-data` corpus.
- `api_parity.py` — `detect_all` / `UniversalDetector`.

CI runs the Rust tests, builds the Python wheel, and compares against the
reference `chardet` wheel from PyPI.

## Regenerating data tables

The generated Rust tables (single-byte codec maps, DBCS validity tables,
Unicode categories, registry and CPython aliases) are produced from CPython so
the Rust codecs match Python's behavior exactly:

```console
$ python3 scripts_generate/gen_registry.py     > chardet/src/registry_data.rs
$ python3 scripts_generate/gen_codec_tables.py > chardet/src/codecs/sbcs_data.rs
$ python3 scripts_generate/gen_dbcs_tables.py  > chardet/src/codecs/dbcs.bin
$ python3 scripts_generate/gen_categories.py   > chardet/src/codecs/categories_data.rs
```

## Supported Encodings

Full chardet 7 coverage, including Windows code pages, ISO-8859 family, Mac,
KOI8, DOS, mainframe EBCDIC, CJK (CP932/CP949/GB18030/Big5/EUC-JP/EUC-KR/Johab
and the ISO-2022/HZ escape encodings), and the Unicode family.

## Benchmarks

A Rust micro-benchmark and a corpus throughput comparison are included:

```console
$ cargo run --release --example bench
$ CHARDET_SRC=/path/to/chardet/src python3 scripts_compare/bench.py tests/data
```

The `Benchmark` workflow (manual dispatch) runs both against the full corpus
and writes the results to the GitHub step summary.

## Project History

chardet was created by [Mark Pilgrim](https://en.wikipedia.org/wiki/Mark_Pilgrim)
in 2006 as a Python port of [Mozilla's universal charset detection
library](https://www-archive.mozilla.org/projects/intl/chardet.html). After he
deleted his online accounts in 2011, the project was continued by David Cramer,
Erik Rose, Toshio Kuratomi, Ian Cordasco, and Dan Blanchard. In 2026 Dan
Blanchard rewrote chardet as a ground-up 0BSD-licensed implementation, released
as chardet 7.

**chardet-rs** is an independent Rust rewrite of chardet 7. It reuses chardet
7's trained model artifacts and mirrors its pipeline, so the detection results
match, while the implementation is native Rust. All credit for the original
algorithm, models, and training data belongs to the chardet authors listed
above.

## License

0BSD — same as chardet 7. See [LICENSE](LICENSE).
