## Summary

<!-- What does this change and why? -->

## Type of change

- [ ] Bug fix (results diverge from the reference)
- [ ] New feature
- [ ] Refactor / cleanup
- [ ] Documentation
- [ ] Build / CI

## Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] Parity scripts pass (`scripts_compare/parity.py`, `edge_parity.py`, `fuzz.py`)
- [ ] For result-affecting changes: included before/after parity numbers
- [ ] Did not hand-edit generated files under `chardet/src/codecs/` or `chardet/src/registry_data.rs`
- [ ] Did not modify bundled model artifacts under `chardet/models/`

## Related issues

<!-- e.g. Fixes #123 -->
