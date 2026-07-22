# AE13 Parser Conformance and Regression Harness

## Status

This contract begins in AE13a. AE13a defines the canonical fixture and result
models, deterministic discovery and validation, exact input handling, shared
document-mode ownership, disposition policy, and one standalone-tokenizer
fixture. It does not claim completion of overarching AE13.

## Ownership

The `html` crate owns parser semantics and typed canonical observation values.
`html::DocumentMode` is the one document-mode type used by tree construction
and future parser observations. `html::conformance` owns passive semantic result
types behind the non-default `parser-conformance` feature. They are internal,
versioned engine-test contracts rather than public DOM or web-platform APIs.
AE13a does not install observation hooks in the tokenizer or tree builder.

The `html-test-support` crate owns serialized fixture-v1 loading, safe path and
input validation, deterministic bundle discovery, expectation selection,
disposition evaluation, and the canonical test runner behind the non-default
`parser-fixtures` feature. That feature activates `html/parser-conformance` and
the optional SHA-256 dependency. It may invoke existing HTML parser APIs, but
it must not reimplement parsing algorithms.

The serialized declaration is the only construction input. Validation seals
fixture IDs, delivery names, snapshot paths, exact input, execution, target,
expectation, source, and disposition state behind an opaque
`ValidatedFixtureSpec`; public consumers receive read-only accessors. A future
external adapter must enter through the same declaration and validation path.
It must not construct validated values directly.

There is one canonical parser-fixture runner. Existing golden, WPT-style, and
internal corpus infrastructure will be adapted to it in later slices rather
than being joined by another independent harness.

## AE13a execution boundary

AE13a executes one configuration:

- native, active fixture;
- exact UTF-8 `input.html`;
- standalone tokenizer;
- whole Unicode-scalar delivery;
- `tokens.txt` in `html5-token-v1`.

The runner executes every discovered fixture in deterministic
repository-relative order and aggregates failures with fixture ID and bundle
path. Adding an ordinary fixture requires no Rust registration or test edit.

The runner uses the existing tokenizer driver and `TokenFmt`. It also captures
owned typed tokens at the existing batch-drain boundary. It does not introduce
a second tokenizer, parser, or token formatter.

Every other fixture-v1 expectation remains declarable so the schema will not
need replacement as later slices land. An active fixture that requests an
unimplemented surface fails with typed `UnsupportedExpectation`. Valid but
unimplemented execution semantics fail with typed
`UnsupportedFixtureSemantics`. Neither outcome is treated as a passing active
fixture.

## Exact input and path boundary

All inputs are loaded with `fs::read`; the loader never trims or normalizes
fixture data. Every fixture declares a mandatory lowercase SHA-256 digest of
the exact stored bytes.

`input.html` is valid UTF-8 checked out with LF endings, and every carriage
return byte is rejected. `input.bin` is required for CRLF, lone CR, trailing CR,
invalid UTF-8, byte delivery, byte-position
coverage, or any other original-byte-sensitive fixture. Root `.gitattributes`
rules make those representations consistent across checkouts.

Declared paths are bundle-relative portable paths. Absolute paths, `..`, `.`
components, backslashes, missing files, non-files, and symlinks are rejected.
Recognized but undeclared snapshot sidecars are rejected as orphans. Diagnostic
paths use repository-relative `/` separators.

## Discovery and identity

Discovery recursively finds `fixture.toml` bundles and sorts them by normalized
repository-relative path. It does not depend on directory enumeration order,
timestamps, map iteration, test scheduling, or platform separators.

A directory containing `fixture.toml` is a fixture leaf. Another
`fixture.toml` below it is rejected rather than silently ignored.

Fixture IDs and delivery names use lowercase ASCII kebab case. Duplicate IDs,
case-unsafe IDs, case-insensitive ID collisions, and case-insensitive bundle or
declared-path collisions fail deterministically.

## Result and expectation semantics

The canonical result has distinct surfaces for tokens, parse errors,
implementation diagnostics, document mode, tree, patches, transitions,
unsupported parser features, and final invariants. Each observation is one of:

- not requested;
- not applicable;
- captured, including a captured empty collection;
- incomplete with a typed reason.

Incomplete capture is non-authoritative. Parse errors and recoverable
implementation diagnostics are separate. Engine invariants, impossible parser
states, finalization failures, patch-validation failures, and materialization
failures are execution or invariant failures and can never be blessed as
expected implementation diagnostics.

AE13a defines the parse-error taxonomy but does not migrate current error call
sites. Dedicated HTML Standard codes use `ParseErrorCode::Standard`.
Rule-defined tree-construction conditions without dedicated standard codes use
stable Borrowser-owned `ParseErrorCode::TreeConstruction` variants. Serialized
code names will be stable, documented rule identities with no `other` fallback;
renaming or changing their meaning requires a format-version change or an
explicit compatibility mapping. Recovery action remains separate metadata.

Tokenizer attributes are lexical name/value pairs in encounter order. Tree and
patch observations use a separate DOM attribute model containing namespace,
prefix, local name, and value. Trees structurally retain doctype public/system
identifiers and template contents beneath their host. Patch observations can
faithfully represent every current `DomPatch` variant and payload; only
`PatchKey` is replaced by caller-supplied snapshot-local labels. AE13a does not
assign stream labels or implement the patch-v3 serializer.

Transition token summaries, insertion modes, dispatch paths, and parser-context
token kinds are typed semantic values. Unsupported-feature observations are
limited to encountered preprocessing, tokenizer, and tree-construction behavior.

Document and fragment declarations default omitted `scripting` to the concrete
validated state `disabled`. Fragment namespaces validate exhaustively to
`html::ElementNamespace::{Html, Svg, MathMl}`; arbitrary strings cannot enter a
validated fragment context.

Final-invariant fields carry only `Satisfied`, typed `NotApplicable`, or
`Failed`. A failed field cannot select its own error code. Exhaustive
field-by-field collection assigns the stable `InvariantFailureCode`, so adding a
mandatory field requires updating collection and preserves deterministic field
order. Production execution of these checks remains AE13c work.

### Normalized parser positions

Canonical event positions use the production parser's normalized input space,
not a fixture-only coordinate system. `utf8_byte_offset` is zero-based and
counts bytes in the decoded UTF-8 string after CR/LF preprocessing. `line` and
`column` are one-based; column counts Unicode scalar values rather than UTF-8
bytes, UTF-16 code units, or grapheme clusters. A non-EOF position identifies
the point immediately before the normalized scalar that triggered the event.
EOF identifies the terminal point after the last normalized scalar.

CRLF and lone CR each become one LF before coordinates are assigned. That LF
occupies the current line and scalar column; the following scalar starts the
next line at column 1. Invalid UTF-8 subsequences decoded from a future raw-byte
fixture contribute their resulting U+FFFD scalar to normalized coordinates:
three normalized UTF-8 bytes and one scalar column. UTF-8 carry and pending CR
state must therefore produce identical positions for whole and chunked delivery.

These coordinates cannot identify original source bytes after decoding and
newline normalization. `SourceBytePosition` must remain
`Unavailable(NoInputProvenanceMap)` unless a separate exact provenance map is
introduced. AE13a adds no such map, performs no approximate offset
reconstruction, and incurs none of the map's per-input memory overhead. AE13b
observation work may emit a known position only when it can satisfy this
contract; otherwise the event position remains explicitly unavailable.

## Dispositions

Fixture-v1 supports active, expected-unsupported, expected-failure, and skipped
dispositions. Non-active dispositions require a non-empty reason, an exact
typed capability or failure classification, and a tracking or provenance
reference. One evaluator owns policy for every disposition. Unsupported fixture
semantics, unsupported expectations, execution failures, expectation mismatches,
and invariant failures use exact typed matching; unexpected success is an XPASS
failure. Unsupported-capability skips retain the exact capability.

Before a skipped disposition can enter the sealed validated model, the
declaration boundary proves that its exact capability is relevant to semantics
the fixture actually declares. Relevance is derived from validated input,
target and scripting state, every declared delivery plan, enabled expectation
surfaces, and the exact set of unknown required extension IDs. An unrelated
capability substitution is a malformed disposition, even when that capability
is generally permitted for external fixtures. Capability relevance and the
completed-capability registry are separate requirements: a capability must be
both relevant and permitted to use a non-active disposition.

All declared delivery plans count as fixture semantics for this check. The
reference delivery selects the ordinary comparison baseline, and a transition
expectation may select another declared plan; neither role makes other declared
plans irrelevant. `byte-delivery` requires a byte-unit delivery,
`unicode-scalar-chunking` requires a Unicode-scalar boundaries plan, and an
expectation capability requires that exact surface to be declared.

`unsupported-capability` is the only fixture-v1 skip classification. Broad
external-source and environment skips are deliberately absent: upstream records
that are duplicate, malformed, unlicensed, or outside an imported profile must
be reported by the future adapter rather than represented as passing skipped
parser fixtures. The completed-capability registry rejects use of the retained
skip to hide completed token or document behavior.

The runner may bypass a skipped fixture only because it accepts a sealed
`ValidatedFixtureSpec` whose skip relevance has already been established. It
does not duplicate capability-relevance policy. External-source import
exclusions remain future adapter-report concerns rather than parser-fixture
skips.

Native fixtures in `crates/html/tests/fixtures/html5/conformance/` must be
active. Non-active dispositions are reserved for later external/adapted inputs
or an explicitly identified quarantine source. Completed Milestone AE behavior
must not be hidden behind a non-active disposition.

An `unsupported_features` observation means a parser limitation was encountered
during an otherwise supported parse. It is distinct from an unsupported fixture
target, delivery mechanism, or required extension.

## Extensions

Extensions use versioned namespaced IDs and strict declarations containing
`required` plus a TOML value. Unknown required extensions produce
`UnsupportedFixtureSemantics`. Unknown optional extensions are retained only as
non-semantic metadata and cannot alter core fixture behavior.

AE13a intentionally has no speculative generic adapter registry. A real known
semantic extension must later receive an exact-version typed adapter at the
single validation boundary.

## Snapshot format status

- `html5-token-v1`: executable AE13a compatibility format.
- `html5-dom-v2`: existing compatibility tree format.
- `html5-dompatch-v2`: existing compatibility patch format.
- `html5-dompatch-v3`: planned native AE13 patch format using labels assigned by
  first semantic appearance and no normative transport batch boundaries.
- Native parse-error, implementation-diagnostic, document-mode, transition,
  unsupported-feature, and final-invariant formats are reserved/planned for
  AE13b through AE13e. AE13a does not implement or claim stability for them.

The `html5-token-v1` reader lives beside the existing token formatter and emits
dedicated typed snapshot-format errors. Malformed snapshots are not reported as
fixture-TOML errors.

## Later slices

- AE13b: passive integrated observation, typed diagnostic migration, shared
  escaping, and stable serializers.
- AE13c: semantic whole/chunked parity and production final-invariant execution.
- AE13d: existing corpus consolidation and migration.
- AE13e: external html5lib/WPT adapter, intentional snapshot updates, final
  documentation, and CI coverage expansion.

Fragment execution, scripting-dependent parsing, original source-byte
provenance, Layout, Paint, JavaScript execution, navigation, and resource
loading are not implemented by AE13a.
