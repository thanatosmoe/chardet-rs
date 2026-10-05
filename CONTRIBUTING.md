# Contributing

Thanks for your interest in `chardet-rs`! This is an independent Rust rewrite
of [chardet 7](https://github.com/chardet/chardet). The detection results are
meant to stay byte-for-byte identical to the reference implementation, so
correctness changes are judged against Python chardet 7.

## Development setup

Rust (stable) is all you need for the core:

```bash
git clone https://github.com/thanatosmoe/chardet-rs.git
cd chardet-rs
cargo build --workspace
cargo test  --workspace
```

For the Python bindings, create a virtualenv and use
[maturin](https://www.maturin.rs/):

```bash
python3 -m venv .venv && . .venv/bin/activate
pip install maturin
cd chardet-py && maturin develop --release
python -c "import chardet; print(chardet.detect(b'hello'))"
```

## Testing

```bash
cargo test --workspace          # Rust unit + integration tests
cargo run --release --example bench   # micro-benchmark
```

### Parity against the reference

Parity is checked with generated corpora and the official corpus. The scripts
in `scripts_compare/` accept these environment variables:

- `CHARDET_SRC` — a checkout of the reference chardet source (its `src/` dir)
- `CHARDET_BIN` — the `chardetect` binary to test (default `target/debug/chardetect`)
- `CHARDET_SO` — the built extension for `api_parity.py`

```bash
# Reference source (version 7.6.1.dev); the generated _version.py is required.
git clone --depth 1 https://github.com/chardet/chardet.git /tmp/ref-chardet
echo '__version__ = "7.6.1.dev0"' > /tmp/ref-chardet/src/chardet/_version.py

export CHARDET_SRC=/tmp/ref-chardet/src CHARDET_BIN=target/debug/chardetect
cargo build
python scripts_compare/parity.py        # hand-written multilingual samples
python scripts_compare/edge_parity.py   # BOM/markup/EBCDIC/truncation/magic
python scripts_compare/fuzz.py 1 500    # randomized/structured inputs

# Full official corpus (gitignored at tests/data/)
git clone --depth 1 https://github.com/chardet/test-data tests/data
python scripts_compare/full_corpus.py tests/data
```

All parity scripts exit non-zero on any mismatch, so they are safe to run in
CI. **Any behavior change must keep these at 100%.**

## Linting and formatting

CI enforces both; run them before opening a pull request:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

## Generated data tables

Several files are generated from CPython so the Rust codecs match Python's
behavior exactly. **Do not edit them by hand**; regenerate instead:

```bash
python3 scripts_generate/gen_registry.py     > chardet/src/registry_data.rs
python3 scripts_generate/gen_codec_tables.py > chardet/src/codecs/sbcs_data.rs
python3 scripts_generate/gen_dbcs_tables.py  > chardet/src/codecs/dbcs.bin
python3 scripts_generate/gen_categories.py   > chardet/src/codecs/categories_data.rs
```

The generated statics carry `#[rustfmt::skip]`, and the generators emit
rustfmt-clean output, so `cargo fmt --check` passes after regeneration.

The bundled model artifacts under `chardet/models/` (`models.bin`, `idf.bin`,
`rowmax.bin`, `confusion.bin`) come from chardet 7 and must not be modified.

## Conventions

- Rust 2021, `#![forbid(unsafe_code)]`.
- Keep the pipeline stages in `chardet/src/pipeline/` consistent with the
  reference stage order and semantics.
- Prefer returning `DetectionResult` values that match the reference exactly;
  if a divergence is unavoidable, document it.
- Do not commit `tests/data/` or build artifacts (`target/`, `*.so`).

## Pull requests

- Keep changes focused; describe the behavior change and why.
- Add or update tests for behavior changes.
- Ensure `cargo test`, `cargo fmt --check`, `cargo clippy -D warnings`, and the
  parity scripts all pass. CI runs these automatically on every PR.
- For result-affecting changes, include the parity numbers (before/after).

## Releasing

Version lives in `Cargo.toml`. To cut a release, tag and push:

```bash
# bump version in Cargo.toml first
git tag vX.Y.Z && git push origin vX.Y.Z
```

The `Release` workflow publishes to crates.io and PyPI, builds wheels for
Linux/macOS/Windows plus an sdist, and creates a GitHub Release. It requires
the repository secrets `CARGO_REGISTRY_TOKEN` and `PYPI_API_TOKEN`.

## License

By contributing you agree that your contributions are licensed under the
[0BSD license](LICENSE), the same license as chardet 7.
