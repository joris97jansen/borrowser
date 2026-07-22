# Canonical HTML Parser Conformance Fixtures

This is the native fixture root for `borrowser-html-parser-fixture-v1`. Discovery
is recursive and sorted by normalized repository-relative bundle path. Add a
directory containing `fixture.toml`, exact input, and declared snapshots; no Rust
registration is required.

The canonical integration test executes every discovered fixture in this order
and aggregates all failures with fixture ID and repository-relative path. A
directory containing `fixture.toml` is a leaf; nested bundles are rejected.

Native fixtures in this directory must be `source = native` and
`disposition.status = active`. Xfail, skip, and expected-unsupported entries
belong only to later external/adapted inputs or a separately identified
quarantine source. Fixture-v1 permits skips only for an exact unsupported
capability; broad external-source and environment skips are rejected.

Use `input.html` only for valid UTF-8 input whose intended checkout form has LF
line endings. Use `input.bin` for CRLF, lone CR, trailing CR, invalid UTF-8,
byte delivery, and any byte-sensitive case. `input.html` containing a carriage
return is rejected. Always update the mandatory SHA-256 from the exact stored
bytes; the loader never trims input.

AE13a executes only whole-input standalone-tokenizer fixtures with a declared
`tokens.txt` in `html5-token-v1`. Other fixture-v1 surfaces are declarable but
fail explicitly as unsupported expectations until their owning AE13 slice lands.

See `docs/html5/parser-fixture-format-v1.md` for the complete schema and
`docs/html5/ae13-parser-conformance-regression-harness.md` for ownership and
slice boundaries.

Run this corpus with:

```text
cargo test -p html --features parser-conformance --test html5_parser_conformance --locked
```
