# AE13e external fixtures and snapshot workflow

AE13e establishes a controlled path toward broader parser-relevant html5lib
and WPT coverage. It does not claim complete html5lib support, complete WPT
support, complete HTML conformance, DOM API conformance, scripting conformance,
or event-loop conformance.

## Pinned external proof

The representative proof uses exact pinned upstream WPT records. The
allowlist is the authoritative source for the upstream project, revision,
source paths, hashes, licence, attribution, and selected record identities:

- allowlist and source metadata: tests/wpt/external/allowlist.toml
- raw files:
  tests/wpt/external/raw/html/syntax/parsing/resources/tests1.dat
  and webkit02.dat
- licence:
  tests/wpt/external/LICENSE-3-Clause.txt

The raw files are the source of truth. The allowlist identifies records by
source path, one-based ordinal, and record SHA-256. A display name is only a
local label and never replaces the upstream-derived identity.

Each generated provenance record carries a validated licence identifier,
licence notice, attribution, and adaptation description. The canonical
validator rejects external declarations whose structured provenance omits any
of these fields.

The adapter parses the ordered `.dat` record grammar. `#data` preserves the
input exactly except for the one structural LF immediately before `#errors`.
WPT error text is provenance only: `#errors` and `#new-errors` contribute an
expected count, not typed Borrowser error identities. The adapter emits ordinary
fixture-v3 declarations and never constructs
ValidatedFixtureSpec, runs the parser, or compares trees.

## Capability classification

Eligibility is default-deny. A record is eligible only when it is a static
full-document case with an explicitly supported scripting state and a tree
that the adapter can represent without semantic invention. The current
normal-CI proof contains one #script-off case.

The following are explicit unsupported classifications, not silent skips:

- fragment DOM API or fragment parsing;
- scripting or a missing script marker (which means upstream requires both
  scripting modes);
- document.write, DOM bindings, events, navigation, resources/networking, and
  rendering are wrapper/platform exclusions documented for future WPT work;
  the raw `.dat` adapter does not infer them from literal HTML text.
- valid WPT tree data that cannot be represented by the current canonical
  expectation format without inventing precision is classified as
  unsupported-expectation-representation. This includes the current
  namespace-designated attribute prefix, template-content, and multiline-value
  limitations.
- unsupported-parser-feature is reserved for a production parser capability
  outside the supported AE parser profile; the current narrow adapter has no
  such inferred classification path.
- malformed or unimportable external record.

Capabilities encoded by the `.dat` record are distinguished from requirements
introduced by a WPT browser wrapper. AE13e does not build a generalized WPT
runner or implement deferred platform capabilities to make a case pass.

## Generated fixture check/update

Generated bundles under
crates/html/tests/fixtures/html5/external-wpt/ are derived artifacts. They
are never semantic authority and are not hand-edited.

Normal tests and CI run the read-only check and canonical execution:

    make test-html5-external-fixtures

The explicit update operation is separate:

    make update-html5-external-fixtures

The operation regenerates only the restricted external-WPT output root from
the vendored raw source and allowlist. It reports sorted added, removed,
changed, and unchanged bundles plus explicitly classified unsupported
records. It does not fetch upstream data. Review the resulting Git diff
normally. No parser execution path writes snapshots or generated fixtures.

## CI lanes

Normal `.github/workflows/ci.yml` runs the native canonical parser corpus and
the tiny external subset. `.github/workflows/html5-conformance.yml` is the
correctness-owned scheduled/manual extended lane; its
`test-html5-external-fixtures-extended` target additionally runs the existing
`crates/html/tests/diff_html5.rs` whole/chunked token and DOM parity corpus with
`DIFF_MODE=both`, deterministic seed, and no random fuzz expansion. The
performance-owned `.github/workflows/perf.yml` remains unchanged.

## Contributor workflow

1. Pin the exact upstream revision and vendor the exact source .dat file.
2. Add an allowlist entry with source path, source-file hash, ordinal, and
   record hash.
3. Run the external adapter test and inspect its capability classification.
4. Run make update-html5-external-fixtures only when intentionally changing
   derived artifacts.
5. Run make test-html5-external-fixtures and the relevant native parser lane.
6. Review provenance, licence, generated diff, expectation strength, and any
   unsupported classification before proposing a change.

Do not broaden the allowlist by directory heuristic or claim the complete
html5lib/WPT suite from this proof.
