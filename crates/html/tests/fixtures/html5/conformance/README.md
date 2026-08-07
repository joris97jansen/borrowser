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

AE13c executes supported standalone-tokenizer and document fixtures through an
authoritative whole-input baseline plus bounded declared and representative
scalar/byte strategies. Semantic aliases execute once. Fixed-one and
fixed-seven delivery are incremental and allocate no boundary vector; the
edge-triplet strategy is bounded and generic. Strategy generation never scans
HTML syntax. Every applicable canonical surface and the mandatory production
final audit must match the baseline.

Parity compares typed canonical values before serialization. The runner keeps
one owned baseline and disposes successful parity-only candidates; it does not
deep-clone the baseline. Final-audit failure injection is attached to each real
fallible reservation and reports the exact observation-resource site. Explicit
scalar boundary resolution is the fixture-harness resource
`scalar-boundary-execution-offsets`; fixed scalar delivery remains allocation
independent of chunk count. Boundary digests are lazy, streaming diagnostic
metadata and are never used for semantic equality.

Canonical sidecars use the exact AE13b5 formats, including `html5-token-v2`,
`html5-dom-v3`, `html5-dompatch-v3`, and the exact 16-record
`html5-final-invariants-v1`. Header-only diagnostic, transition,
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
