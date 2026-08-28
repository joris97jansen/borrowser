# Conformance fixture inventory

This directory contains the repository-owned AG conformance fixture inventory.
Each logical fixture is a directory bundle containing exactly one authoritative
`fixture.toml` descriptor plus only the payload and optional reference file
declared by that descriptor. See
[`docs/conformance/ag2-fixture-inventory-manifest-contract.md`](../../docs/conformance/ag2-fixture-inventory-manifest-contract.md)
for the normative layout, schema, identity, validation, and manifest contract.
Expected-result and classification metadata lives separately in the
human-authored `expected-results.toml`; see
[`docs/conformance/ag3-expected-results-classification-contract.md`](../../docs/conformance/ag3-expected-results-classification-contract.md).

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
5. Add exactly one record for the new logical ID to `expected-results.toml`.
   Use an explicit `not-yet-classified` record with a reason when evidence does
   not support a complete classification. Do not infer metadata from the
   fixture path or payload.
6. Run `make check-conformance-manifest`,
   `make check-conformance-expected-results`, and the
   `conformance-test-support` tests.

`fixture.toml` is the source of truth. `manifest.toml` is a checked-in,
deterministically generated review artifact and must not be edited by hand.
The check operation is read-only and reports a missing or stale manifest.

`expected-results.toml` is also a source of truth and is edited by hand. Its
check operation is read-only: it validates strict typed metadata against the
complete discovered inventory and prints an ephemeral deterministic summary.
There is intentionally no update mode and no checked-in generated summary.
When changing an expectation, capability, harness limitation, stability
declaration, environment requirement, or lane exclusion, include the required
reason and relevant contract/tracking reference. Engine availability must be
supported by authoritative production contracts or deterministic tests; a
fixture's presence and expected-pass declaration are not evidence.
Before marking a case classified, verify that its payload supplies every
context needed to identify the logical observation without invention. Harness
readiness must separately account for delegation, whether an authoritative
expected observation has been authored, whether an existing expectation can be
represented truthfully, the subsystem-owned observation, and comparison
infrastructure; a missing adapter is not automatically the only limitation.

Fixture payloads are byte-preserving inputs. Do not assume that `.html`,
`.css`, `.txt`, or any other extension permits line-ending normalization or
UTF-8 decoding. Git attributes intentionally mark all bundle content as binary
first and opt only `fixture.toml` back into LF-normalized infrastructure text.

Inventory presence means only that a descriptor was discovered and validated.
It does not mean the fixture is runnable, passes, conforms to a standard, is a
WPT reftest, or demonstrates browser compatibility.
