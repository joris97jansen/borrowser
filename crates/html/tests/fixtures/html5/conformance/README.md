# Canonical HTML Parser Conformance Fixtures

This is the native fixture root for `borrowser-html-parser-fixture-v2`. Discovery
is recursive and sorted by normalized repository-relative bundle path. Add a
directory containing `fixture.toml`, exact input, and declared snapshots; no Rust
registration is required.

The canonical integration test executes every discovered fixture in this order
and aggregates all failures with fixture ID and repository-relative path. A
directory containing `fixture.toml` is a leaf; nested bundles are rejected.

Native fixtures in this directory must be `source = native` and
`disposition.status = active`. Xfail, skip, and expected-unsupported entries
belong only to later external/adapted inputs or a separately identified
quarantine source. Fixture-v2 permits skips only for an exact unsupported
capability; broad external-source and environment skips are rejected.

Use `input.html` only for valid UTF-8 input whose intended checkout form has LF
line endings. Use `input.bin` for CRLF, lone CR, trailing CR, invalid UTF-8,
byte delivery, and any byte-sensitive case. `input.html` containing a carriage
return is rejected. Always update the mandatory SHA-256 from the exact stored
bytes; the loader never trims input.

AE13b5 executes supported whole-input standalone-tokenizer and document
fixtures from typed canonical observations. Ordinary surfaces are unioned on
the reference delivery; transition expectations may name another declared
whole delivery. Each planned delivery executes once. Unused declared whole
deliveries are capability-checked but do not execute.

Canonical sidecars use the exact AE13b5 formats, including `html5-token-v2`,
`html5-dom-v3`, and `html5-dompatch-v3`. Header-only diagnostic, transition,
unsupported-feature, tree, and patch snapshots represent requested empty
collections. Fixture-v1 remains an isolated compatibility format.

See `docs/html5/parser-fixture-format-v2.md` and
`docs/html5/ae13b5-parser-snapshot-formats.md` for the schema/codecs, and
`docs/html5/ae13-parser-conformance-regression-harness.md` for ownership and
slice boundaries.

Run this corpus with:

```text
cargo test -p html --features parser-conformance --test html5_parser_conformance --locked
```
