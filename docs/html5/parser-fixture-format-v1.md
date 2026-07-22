# Borrowser HTML Parser Fixture Format v1

## Version and bundle

Each fixture is a directory containing `fixture.toml`, one exact input file,
and declared expectation sidecars. The required format identifier is:

```toml
format = "borrowser-html-parser-fixture-v1"
```

Core fields are strict. Unknown fields at any core nesting level are malformed
fixture errors. Future optional metadata belongs only under `extensions`.

Minimal AE13a fixture:

```toml
format = "borrowser-html-parser-fixture-v1"
id = "tokenizer-character-data"

[source]
kind = "native"

[input]
path = "input.html"
kind = "utf8-text"
sha256 = "<64 lowercase hexadecimal digits>"

[execution]
reference_delivery = "whole"

[execution.target]
kind = "standalone-tokenizer"

[[execution.deliveries]]
name = "whole"
unit = "unicode-scalars"
strategy = "whole"

[expectations]
tokens = "tokens.txt"

[disposition]
status = "active"
```

No Rust source registration is required for an ordinary bundle.

## Input

Input kinds are `utf8-text` and `raw-bytes`.

- `utf8-text` requires `input.html`, valid UTF-8, Unicode-scalar delivery, and
  repository LF endings.
- `raw-bytes` requires `input.bin` and byte delivery.

CRLF, lone CR, trailing CR, invalid UTF-8, byte chunking, and byte-sensitive
offset fixtures must use `input.bin`. A `utf8-text` input containing any `CR`
byte is malformed. Reading is byte-exact and never removes a terminal newline.
The declared SHA-256 is mandatory and covers the file bytes as stored in the
checkout.

### Normalized event-position coordinates

Future parse-error and diagnostic sidecars will use the HTML-owned normalized
parser-position contract. The normalized UTF-8 byte offset is zero-based. Line
and column are one-based, and column counts Unicode scalar values. Positions
refer to the point before the triggering normalized scalar; EOF uses the
terminal point after the final scalar.

Coordinates are assigned after decoding and CR/LF preprocessing. CRLF and lone
CR each contribute one normalized LF. An invalid UTF-8 subsequence contributes
the resulting U+FFFD replacement (three normalized UTF-8 bytes and one scalar
column), independent of byte-chunk boundaries. Original source-byte offsets are
unavailable without a separate provenance map; AE13a neither creates such a map
nor reconstructs source offsets approximately. These position-bearing
serializers remain planned rather than executable in AE13a.

## Identifiers and paths

Fixture IDs and delivery names match lowercase ASCII kebab case:

```text
[a-z0-9]+(-[a-z0-9]+)*
```

Paths use `/`, are relative to the bundle, and contain only normal components.
Absolute paths, `..`, `.`, backslashes, symlinks, missing targets, and paths that
are not regular files are rejected. IDs, bundle paths, and declared paths must
not collide after ASCII case folding.

A directory containing `fixture.toml` is a fixture leaf. Nested fixture bundles
are rejected; discovery never silently ignores a descendant `fixture.toml`.

## Targets and deliveries

Targets are `standalone-tokenizer`, `document`, and `fragment`. Fragment targets
require explicit fragment context. Scripting metadata is legal only for document
or fragment targets.

Delivery units are `bytes` and `unicode-scalars`. Strategies are `whole` and
`boundaries`. Boundary lists must be strictly increasing interior offsets.
Every transition expectation must name a declared delivery. One exhaustive
conversion creates validated input/target/delivery enums; runners never receive
the unvalidated cross-product.

Fragment namespaces are exactly `html`, `svg`, or `mathml` and become
`html::ElementNamespace` values during validation. Unknown namespace strings
are malformed fixture semantics and never survive in a validated target.
Omitted `scripting` on document and fragment targets has the fixture-v1 default
`disabled`; the validated target always contains a concrete scripting mode.
Standalone-tokenizer targets cannot declare scripting metadata.

Validated fixture IDs, delivery names, snapshot paths, targets, inputs,
deliveries, expectations, sources, and dispositions are opaque after this
conversion. Public consumers receive read-only accessors; they cannot construct
or mutate a `ValidatedFixtureSpec`. Future adapters must produce the serialized
declaration model and use this same validation boundary.

AE13a executes only standalone-tokenizer, UTF-8 text, whole Unicode-scalar
delivery. Other legal combinations return typed unsupported outcomes.

## Expectations

All fixture-v1 surfaces are optional and independently declared:

```toml
[expectations]
tokens = "tokens.txt"
parse_errors = "parse-errors.txt"
implementation_diagnostics = "implementation-diagnostics.txt"
document_mode = "document-mode.txt"
tree = "tree.txt"
patches = "patches.txt"
unsupported_features = "unsupported-features.txt"
final_invariants = "final-invariants.txt"

[[expectations.transitions]]
delivery = "whole"
path = "transitions.whole.txt"
```

At least one expectation is required. Absence means not declared; it never means
expected empty. A declared empty diagnostic snapshot will later mean capture was
requested and completed with zero events.

Recognized default sidecars that are present but undeclared are rejected. A
declared sidecar must exist even when its execution surface belongs to a later
AE13 slice. In AE13a, only `tokens.txt` with `html5-token-v1` executes; an active
fixture requesting another surface fails with typed `UnsupportedExpectation`.

## Dispositions and sources

Native fixtures must use:

```toml
[source]
kind = "native"

[disposition]
status = "active"
```

External and quarantine sources, and non-active dispositions, are schema support
for later adapters. Expected unsupported, expected failure, and skipped entries
require a non-empty reason, an exact typed classification, and a tracking or
provenance reference. Matching is exact and unexpected success is XPASS.

Execution failures distinguish snapshot reads, snapshot formats, tokenizer
driver failures, and validated-runner invariant failures. Expectation mismatches
name one exact surface; invariant failures name one exact invariant code. An
unsupported expectation matches only the corresponding expectation capability,
and unsupported-capability skips carry that exact capability. One evaluator
applies these rules to active and non-active fixtures.

Fixture-v1 retains only `unsupported-capability` as a skip classification. Its
capability is mandatory and the completed-capability registry rejects attempts
to skip completed token expectations or document execution. Broad external or
environment skips are not valid fixture-v1 values. A duplicate, malformed,
unlicensed, or out-of-profile upstream source record belongs to the future
adapter's import report; it is not a blessed Borrowser parser-fixture result.

A skipped unsupported capability must also be directly relevant to the
fixture's validated declarations. Validation derives relevance from the input
kind, parser target and scripting state, every declared delivery plan, exact
expectation surfaces, and exact unknown required extension IDs before the
fixture reaches the runner. Substituting an unrelated capability is an invalid
disposition, not a passing skip. Relevance does not replace completed-capability
policy: both checks must pass.

The delivery rule considers every declared plan because every plan is required
fixture semantics. The reference name chooses the ordinary comparison baseline,
while transition expectations can name any declared plan; neither role narrows
relevance. `byte-delivery` requires a byte-unit plan and
`unicode-scalar-chunking` requires a Unicode-scalar boundaries plan. Required
extension relevance uses an exact extension ID and excludes optional
extensions. Expectation relevance uses the exact surface declared as a
comparison, including at least one transition expectation for
`transitions-expectation`.

The runner can return a non-executed skip only for a sealed fixture that has
passed this validation. It does not reevaluate relevance. External import
exclusions remain adapter-report concerns and are not fixture-v1 skip
classifications.

Example external declaration:

```toml
[source]
kind = "external"
provenance = "upstream/file.dat#case-17"

[disposition]
status = "expected-unsupported"
reason = "Fragment parsing is deferred."
capability = { kind = "fragment-parsing" }
reference = { kind = "tracking-issue", value = "#1420" }
```

## Extensions

```toml
[extensions."org.example.metadata-v1"]
required = false
value = { source_case = "17" }
```

An extension ID has at least three lowercase kebab-case dot-separated segments,
with the final segment ending in `-v` plus decimal digits. Unknown required
extensions are unsupported semantics. Unknown optional extensions are retained
as metadata and cannot change input, target, delivery, expectations, comparison,
or disposition. Known semantic extensions will require exact-version typed
adapters when a real use case is implemented.

## Format inventory

`html5-token-v1` is executable in AE13a. Existing `html5-dom-v2` and
`html5-dompatch-v2` remain compatibility formats. Native patch output is planned
as `html5-dompatch-v3`, with first-semantic-appearance labels and transport batch
boundaries excluded from the normative contract. Other native AE13 serializer
formats are planned/reserved and are not implemented or stable in AE13a.

The compatibility reader requires exactly one `# format: html5-token-v1`
header, valid `html5-token-v1` token lines, and a final `EOF`. Snapshot-format
failures are distinct from fixture-TOML failures.

Bare token names follow the production tokenizer and compatibility-writer
grammar: they must be non-empty, and ASCII whitespace is rejected because it is
structural syntax. Non-ASCII Unicode whitespace is not treated as a delimiter
by this compatibility reader. This preserves writer-reader compatibility and
does not claim broader tokenizer conformance.

## Feature ownership

`html::conformance` requires the non-default `html/parser-conformance` feature.
The canonical loader and runner require the non-default
`html-test-support/parser-fixtures` feature, which activates the HTML model and
optional SHA-256 dependency. These are internal engine-test contracts, not
public web-platform APIs.
