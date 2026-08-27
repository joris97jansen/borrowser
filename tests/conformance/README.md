# Conformance fixture inventory

This directory contains the repository-owned AG conformance fixture inventory.
Each logical fixture is a directory bundle containing exactly one authoritative
`fixture.toml` descriptor plus only the payload and optional reference file
declared by that descriptor. See
[`docs/conformance/ag2-fixture-inventory-manifest-contract.md`](../../docs/conformance/ag2-fixture-inventory-manifest-contract.md)
for the normative layout, schema, identity, validation, and manifest contract.

To add a fixture:

1. Create a bundle directory below `tests/conformance/fixtures/`. Directory
   grouping is for contributors only; it does not define fixture semantics.
   Every path component must follow the contract's lowercase ASCII portable
   component grammar.
2. Add a strict `borrowser-conformance-fixture-v1` `fixture.toml` with a new,
   stable logical ID and explicit scope, observation surface, source kind, and
   bundle-relative payload path.
3. Add only the files named by `test_path` and optional `[reference].path`.
4. Run `make update-conformance-manifest`.
5. Run `make check-conformance-manifest` and the
   `conformance-test-support` tests.

`fixture.toml` is the source of truth. `manifest.toml` is a checked-in,
deterministically generated review artifact and must not be edited by hand.
The check operation is read-only and reports a missing or stale manifest.

Fixture payloads are byte-preserving inputs. Do not assume that `.html`,
`.css`, `.txt`, or any other extension permits line-ending normalization or
UTF-8 decoding. Git attributes intentionally mark all bundle content as binary
first and opt only `fixture.toml` back into LF-normalized infrastructure text.

Inventory presence means only that a descriptor was discovered and validated.
It does not mean the fixture is runnable, passes, conforms to a standard, is a
WPT reftest, or demonstrates browser compatibility.
