# AG9 cross-engine comparison and conformance reporting contract

Status: Stage 0 contract and limits freeze; aggregate execution, report
serialization, external capture loading and comparison, trend execution, and
CLI/CI publication are not implemented by this document

Last updated: 2026-09-02

AG9 defines the future aggregate-accounting, reporting, cross-engine evidence,
baseline-note, and trend contracts for Borrowser's current static HTML/CSS
conformance harness. This Stage 0 change is documentation only. It does not
create an aggregate runner, serialize an AG9 report, load a capture registry,
run an external browser, compare an external capture, calculate a trend, or
change a command or CI job.

AG1 through AG8 remain authoritative. In particular, AG9 preserves AG1's
federated ownership and orthogonal state, AG2 logical identity and discovery,
AG3 classification and expectation metadata, AG4 through AG6 typed subsystem
execution, AG7 structural reference semantics, and AG8 source-record and
provenance accounting. Existing parser, CSS, and rendering report formats and
behavior remain unchanged.

Cross-engine evidence is always advisory. A specification or accepted WPT
assertion may establish or support the authoritative expected behavior; it does
not make an external capture authoritative. The relationship is:

```text
accepted specification / accepted WPT assertion
    -> may establish or support the authoritative expected behavior

Borrowser observed behavior
    + authoritative expectation
    -> Borrowser semantic pass/fail and derived policy

external browser observation
    -> advisory comparison only
```

External engine agreement or disagreement never produces or modifies
`SemanticPass`, `SemanticFail`, an AG3 expectation, `DerivedPolicyResult`, or
CI success/failure.

## Accounting populations and granularity

Every AG9 count and identity declares one of these populations:

| Population | Identity and meaning |
| --- | --- |
| source record | Native or external source material from which logical cases may be derived; AG8 remains authoritative for its external WPT source records. |
| logical case | One AG2 `TestId` reconciled with exactly one AG3 record. This is the population used by AG9 headline counts. |
| execution variant | One logical case plus its exact typed execution parameters. Parser and CSS currently have an internal singleton variant; rendering uses `RenderingExecutionVariantId`. |
| observation surface | The subsystem-owned semantic result selected by AG2, independently of source form, owner, variant, or comparison kind. |
| subsystem owner | HTML/parser, CSS, Layout, Paint, or Browser/runtime, derived from the observation surface under the AG1 ownership table. |
| comparison/oracle kind | The mechanism that evaluates an observation, such as authored snapshot or AG7 static document reference. It is not a subsystem. |
| external advisory comparison | One versioned comparable-surface attachment, stable advisory track, and exact external capture evaluated against a Borrowser observation. It cannot enter Borrowser pass/fail policy. |
| baseline note | One reviewer-authored note with a stable note ID and explicit case/variant/surface attachment, and optionally an exact external capture ID. It is evidence only. |

One source record may produce multiple logical cases. One logical case may
materialize multiple execution variants. One variant may expose multiple owner
observations internally, but the AG2 observation surface remains the logical
case's declared primary surface. Counts from these populations must never be
summed or compared as if they described the same population.

`total tests` means the number of discovered AG2 logical cases that reconcile
with the complete AG3 registry for the run's source set. It does not mean
source records, variants, attempted variants, observations, or comparisons.
The current post-AG8 repository happens to contain 25 logical cases and 25 AG3
records. Current equality between some logical and materialized-variant counts
is not a contract and must not be assumed by AG9.

### Logical headline counts

All requested headline counts are projections over logical cases:

| Count | Logical-case predicate |
| --- | --- |
| `total` | Every reconciled logical case, independent of classification, eligibility, lane selection, attempt, or outcome. |
| `pass` | At least one selected variant exists, every selected variant was attempted, and every selected variant terminated with aggregate `SemanticPass`. |
| `fail` | At least one selected, attempted variant terminated with aggregate `SemanticFail`. |
| `expected fail` | The AG3 expectation is `expected-fail`; this remains an expectation/policy fact even when no execution is attempted. |
| `unsupported` | AG3 explicitly records an unavailable Borrowser engine/platform capability. |
| `skipped` | The case is runnable under AG9's fixed empty environment assessment but is excluded from this named lane/request. |
| `flaky` | AG3 stability is `flaky`. |
| `unclassified` | AG3 classification completeness is `not-yet-classified`. |

These predicates overlap and are not a partition. Their sum has no defined
relationship to `total`. For example, a logical case may be expected-failing,
flaky, and semantically failing in the same named run.

`pass` is deliberately strict. A selected-but-not-attempted variant prevents
the logical case from counting as pass, while retaining its independent typed
attempt state and reason. It does not automatically make the case a semantic
failure. Likewise, an aggregate execution failure, resource failure,
incomplete observation, invariant failure, or timeout is not a semantic fail.
Only a selected, attempted `SemanticFail` contributes to `fail`.

Cases with no selected variants, including an excluded-only runnable case, are
neither pass nor fail. An excluded-only runnable logical case contributes to
`skipped`.

## Eligibility, named-lane selection, and attempts

AG9 aggregate execution uses the existing empty
`ExecutionEnvironmentAssessment`. AG9 does not expose a caller-constructible
general environment-assessment API. A declared environment requirement for
which the empty assessment has no evidence remains eligibility
`NotYetEstablished`.

Named-lane selection is derived only after eligibility:

```text
execution eligibility
    -> Runnable
         -> named-lane decision
              -> Selected { lane }
              -> Excluded { lane, reason }
    -> NotRunnable
         -> NotApplicable
    -> NotYetEstablished
         -> NotApplicable
```

The planned typed result is equivalent to:

```rust
enum LaneSelection {
    NotApplicable,
    Selected {
        lane: LanePolicyScope,
    },
    Excluded {
        lane: LanePolicyScope,
        reason: String,
    },
}
```

`NotApplicable` does not relabel `NotYetEstablished` as `NotRunnable`, and it
does not carry an eligibility reason. The authoritative reason and state remain
in the independent typed eligibility dimension.

AG3 lane exclusions remain independent declarations. A non-runnable or
eligibility-not-established case may declare an exclusion for the requested
lane while its actual execution selection is `NotApplicable`. Reports must
show the declaration as metadata without adding that variant to either the
selected or excluded execution-selection population.

For a completely decided named run, at execution-variant granularity:

```text
materialized variants
    = runnable variants
    + not-runnable variants
    + eligibility-not-established variants

runnable variants = selected variants + excluded variants
```

Both identities use checked arithmetic. Non-runnable and
eligibility-not-established variants are reported separately and remain
outside the second identity. Only a selected runnable variant may reach a
subsystem evaluator. An excluded runnable variant must not invoke it.

AG3's controlled lane vocabulary remains exactly `normal-ci`,
`local-extended`, `scheduled-extended`, and `manual-extended`. Existing direct
parser, CSS, and rendering runner entry points may retain their current direct
execution behavior, but that behavior is not an AG lane. AG9 must not add a
compatibility, legacy, all, or other synthetic lane.

## Orthogonal conformance state and unsupported accounting

AG9 preserves these independent dimensions rather than adding one overloaded
status enum:

- classification completeness;
- engine/platform capability availability;
- harness readiness;
- environment requirements and assessment;
- execution eligibility;
- expectation;
- AG3 lane-exclusion declarations;
- named-run lane selection;
- stability;
- execution-attempt state;
- observed terminal outcome; and
- derived policy.

`expected fail` remains an expectation and derived-policy fact. It never turns
an observed semantic failure into a conformance pass. `flaky` remains stability
metadata. `skipped` is relative to a named request and is not a synonym for all
non-attempted cases.

Only explicit Borrowser engine/platform capability unavailability contributes
to `unsupported`. These remain separate and must not increase `unsupported`:

- harness not ready;
- unavailable or unassessed execution environment;
- unrepresentable authoritative expectation;
- missing observation or comparison surface;
- classification not established;
- lane exclusion;
- external-capture or registry infrastructure not implemented; and
- unavailable external evidence.

This preserves AG1 and AG3's distinction between production capability gaps
and conformance-infrastructure gaps.

## Subsystem ownership and comparison kind

Subsystem ownership is derived independently of comparison kind:

| Observation surface | Primary owner |
| --- | --- |
| HTML tokenizer | HTML/parser |
| HTML tree construction | HTML/parser |
| DOM snapshot | HTML/parser |
| CSS parsing | CSS |
| selectors | CSS |
| cascade | CSS |
| computed style | CSS |
| layout geometry | Layout |
| paint operations | Paint |
| Browser/runtime semantic | Browser/runtime |

AG7 static document reference comparison is an oracle/comparison kind over
Layout- or Paint-owned structural observations. It is not a fabricated
`reftest-style rendering` subsystem. AG9 reporting will expose the requested
reftest-style view in a separate comparison-kind section labelled as a static
document reference, without adding its cases again to subsystem totals.

AG7 does not execute WPT raster reftests. It compares exact subsystem-owned
structural bytes and provides no screenshots, pixels, fuzzy matching, viewport
height, device scale, platform-font rendering, or WPT reference-graph
execution. AG9 does not weaken those boundaries.

## Aggregate terminal outcomes

The future aggregate terminal vocabulary is:

```rust
enum AggregateTerminalOutcome {
    SemanticPass,
    SemanticFail,
    ExecutionFailure,
    ResourceFailure,
    IncompleteObservation,
    InvariantFailure,
    Timeout,
}
```

This vocabulary describes only attempted execution. Attempt state remains a
separate type. Parser, CSS, and rendering adapters must map their current
closed typed terminal outcomes through exhaustive Rust matches. They must not
classify outcomes through diagnostic text, stable-label substring matching,
filesystem paths, error messages, or other heuristics. The lossless subsystem
result remains authoritative and available.

The Stage 1 mappings are frozen as follows:

- Parser `SemanticPass` maps to `SemanticPass`; `ExpectationMismatch` and
  `ParityMismatch` map to `SemanticFail`;
  `FixtureExecutionResourceExhaustion` maps to `ResourceFailure`; other parser
  execution-failure categories, including `ValidatedFixtureInvariant`, map to
  `ExecutionFailure`; incomplete observation maps to `IncompleteObservation`;
  and final invariant failure maps to `InvariantFailure`.
- CSS semantic pass/mismatch map to semantic pass/fail. A typed execution
  failure maps to `ResourceFailure` only when an exhaustive subsystem-owned
  classifier positively identifies one of these closed causes:
  `HtmlParser(Fatal(ResourceExhaustion))`,
  `HtmlSemanticInputResourceLimited`, `ResourceLimit`, or
  `StorageAllocation`; `SelectorProjection(ElementLimitExceeded)` or a
  selector-DOM `ElementIdRepresentationExhausted`,
  `ProjectionCapacityExceeded`, or `StorageReservationFailed`;
  `SelectorMatching(Matching(AxisStepLimitExceeded))`;
  `RuleCollection(UnsupportedConfiguration | LimitExceeded | Reservation)`;
  `StyleResolution` wrapping a resource-classified selector-DOM failure,
  `LimitExceeded`, `UnsupportedConfiguration`, `SelectorMatching`,
  `StylesheetInputBuild(Reservation)`, resource-classified
  `RuleCollectionBuild`, or resource-classified `CascadeResolution`; or
  `ComputedMaterialization` wrapping a resource-classified selector-DOM or
  `StyleResolution` failure. Resource-classified `CascadeResolution` means
  exactly `CandidateCeilingOverflow`, `RuleInputCeilingOverflow`,
  `UnsupportedLocatorLimit`, `CandidateLimitExceeded`,
  `WinnerWorkspaceReservationFailed`, `WinnerOutputReservationFailed`, or
  `RuleInputStorageReservationFailed`. Other execution failures remain
  `ExecutionFailure`. The existing outer incomplete and final-invariant
  outcomes map to `IncompleteObservation` and `InvariantFailure`, including
  when incomplete observation carries an allocation cause.
- Authored rendering outcomes follow the same mapping. For an AG7 document
  reference, a semantic relation maps to semantic pass/fail; a comparison
  invariant maps to `InvariantFailure`; and a capture-terminal result uses the
  deterministic precedence final invariant, incomplete observation, typed
  resource execution failure, then other execution failure across its two
  sides. Rendering resource execution failures are exactly HTML fatal parser
  resource exhaustion, HTML or stylesheet semantic-input resource limitation,
  rendering storage allocation, or wrapped CSS rule-collection,
  style-resolution, computed-style, or style-tree failures that the exhaustive
  typed classifier identifies as one of the CSS resource causes above.

`Timeout` has no producing parser, CSS, or rendering adapter in AG9. It remains
a reserved, stable, zero-count report category. Its presence must not imply
that current runners implement timeout detection.

## Cross-engine ownership and dependency direction

Source-neutral capture identity and provenance belong to
`external-test-provenance`. That crate must not know `TestId`, AG observation
surfaces, AG lane policy, aggregate variants, rendering variants, registry
attachments, or Borrowser engine types.

Borrowser case/variant/surface attachment, the aggregate execution-variant
key, external advisory tracks, baseline-note attachments, registry parsing,
and reconciliation belong to `conformance-runner::aggregate`, the layer that
owns the authoritative aggregate variant identity.

The intended dependency direction is:

```text
external-test-provenance -> ring, serde, toml

conformance-test-support -> external-test-provenance
html-test-support        -> html, external-test-provenance
css-test-support         -> css, html
rendering-test-support   -> conformance-test-support, wpt-test-support,
                            css-test-support, html, css, layout, gfx

conformance-runner[aggregate]
    -> conformance-test-support
    -> external-test-provenance
    -> html-test-support
    -> css-test-support
    -> rendering-test-support
```

No lower generic crate depends on `conformance-runner` or subsystem execution
support. No production crate depends on AG tooling. An attachment uses the
runner-owned typed aggregate variant key. It must not store an opaque variant
string, duplicate rendering variant fields in `conformance-test-support`, or
introduce another rendering-variant grammar.

The `rendering-test-support -> css-test-support` edge is test-tooling-only and
one-way. Rendering owns classification of rendering failures, but delegates
wrapped CSS rule-collection, style-resolution, cascade, selector-DOM, and
computed-style failure semantics to the CSS-owned typed classifier. This avoids
a second CSS resource taxonomy in rendering while keeping production crates
independent of AG tooling.

## `web-observable-dom-tree-v1`

`web-observable-dom-tree-v1` is a narrow, independently specified DOM
observation for deliberately selected static fixtures. It is not an alias for
Borrowser's `html5-dom-v3` debug snapshot. Two independent implementations can
produce the same bytes using this section alone.

### Artifact grammar

The artifact is UTF-8. Physical line endings are exactly LF (`0x0a`), blank
lines are forbidden, and the artifact ends with exactly one LF. Every field
line is the ASCII field name, one space, `=`, one space, and its value.

The first two lines are exactly:

```text
format = "web-observable-dom-tree-v1"
root-count = 1
```

Exactly one root node follows. A node record begins with
`node-begin = "<kind>"` and ends with the matching
`node-end = "<kind>"`. There is no indentation. Counts are unsigned decimal
without a sign or leading zero, except that zero is written `0`. Counted
records occur immediately after their count in observable child order or the
canonical attribute order defined below.

The closed node records and fixed field orders are:

```text
node-begin = "document"
child-count = <count>
<child node records>
node-end = "document"

node-begin = "document-type"
name = <string>
public-id = <string>
system-id = <string>
node-end = "document-type"

node-begin = "element"
namespace-uri = <string>
local-name = <string>
attribute-count = <count>
<attribute records>
child-count = <count>
<ordinary child node records>
template-contents = "absent" | "present"
<template child count and records only when present>
node-end = "element"

node-begin = "text"
data = <string>
node-end = "text"

node-begin = "comment"
data = <string>
node-end = "comment"

node-begin = "processing-instruction"
target = <string>
data = <string>
node-end = "processing-instruction"
```

Present template contents are encoded immediately after
`template-contents = "present"` as:

```text
template-child-count = <count>
<template child node records>
```

An attribute record has exactly:

```text
attribute-begin = true
namespace-uri = null | <string>
prefix = null | <string>
local-name = <string>
qualified-name = <string>
value = <string>
attribute-end = true
```

The tokens `null` and `true` are unquoted ASCII literals. All other textual
values use the string encoding below.

### String encoding

Strings are delimited by ASCII double quotes. Unicode is not normalized.
Unicode scalar values are encoded as UTF-8 after applying these escapes:

| Value | Bytes in the artifact |
| --- | --- |
| backslash | `\\` |
| double quote | `\"` |
| LF | `\n` |
| CR | `\r` |
| tab | `\t` |
| other U+0000 through U+001F, and U+007F | `\u00xx` using lowercase hexadecimal |

All other Unicode scalar values, including U+2028 and U+2029, are emitted
unchanged. An unpaired UTF-16 surrogate observed by external JavaScript is not
a Unicode scalar value and fails capture.

### Document and node semantics

- The root is exactly one `Document`; multiple roots, a missing document, or a
  nested document fail closed.
- Document children are serialized in `Document.childNodes` order.
- A document type uses the standardized `name`, `publicId`, and `systemId`
  values. Borrowser's absent public/system identifiers project to empty
  strings. An absent Borrowser doctype name fails closed.
- Document mode is not part of this surface. It remains a separate parser
  observation.
- Text and comment `data` are serialized exactly as observed after string
  escaping. DOM parser line-ending normalization is observable data and is not
  reversed by the serializer.
- A processing instruction uses standardized `target` and `data`. It is
  supported only when present as an actual processing-instruction node; markup
  that the HTML parser represents as a comment remains a comment.
- Element children are serialized in `Node.childNodes` order.

### Namespaces, names, and attributes

V1 accepts exactly these element namespace URIs:

- `http://www.w3.org/1999/xhtml`;
- `http://www.w3.org/2000/svg`; and
- `http://www.w3.org/1998/Math/MathML`.

Element `local-name` is serialized exactly as observed. For the supported
complete `text/html`, scripting-disabled parser-created domain, an element
prefix is structurally absent/null. Borrowser's authoritative production
`ElementNode` stores one `ExpandedElementName` containing exactly
`ElementNamespace` and local name; the parser element-construction path has no
element-prefix input or retained field. `ObservedTreeNode::Element` therefore
preserves the complete supported element-name value rather than dropping a
meaningful prefix.

Stage 3 must prove this invariant with representative HTML-, SVG-, and
MathML-namespace parser-created elements. The external capture algorithm must
read the standardized `Element.prefix` and reject the capture when it is
non-null. It must not invent a null value or omit the check merely because
Borrowser has no prefix field. If a future production parser-created domain
admits a meaningful element prefix, V1 is no longer sufficient for that input
and must fail closed pending a versioned comparable surface.

Attribute namespace values are limited to no namespace (`null`) and these
URIs:

- XML: `http://www.w3.org/XML/1998/namespace`;
- XMLNS: `http://www.w3.org/2000/xmlns/`; and
- XLink: `http://www.w3.org/1999/xlink`.

No-namespace attributes require a null prefix. XML and XLink attributes require
the `xml` and `xlink` prefixes. The default XMLNS declaration has a null prefix
and local/qualified name `xmlns`; prefixed XMLNS attributes require prefix
`xmlns`. `qualified-name` must equal `local-name` when the prefix is null and
`prefix + ":" + local-name` otherwise.

Attributes are sorted by the tuple `(namespace-uri, local-name, prefix,
qualified-name)`. For optional values, `null` sorts before any string; strings
sort lexicographically by their unescaped UTF-8 bytes. Duplicate canonical
attribute identities fail closed. Attribute source order and `NamedNodeMap`
iteration order do not affect the bytes.

### Template contents

An HTML-namespace element with local name `template` requires
`template-contents = "present"`. Its ordinary `childNodes` and its standardized
`HTMLTemplateElement.content.childNodes` are separate ordered child lists.
Every other element requires `template-contents = "absent"`. A missing,
wrong-kind, or multiply associated template-content fragment fails closed.
Nested templates use the same recursive rules.

### Unsupported states and fixture assumptions

V1 fails closed for:

- a free-standing `DocumentFragment` other than HTML template contents;
- CDATA, entity, notation, or any unknown node type;
- unsupported element or attribute namespaces;
- an element prefix;
- shadow-root or shadow-tree state;
- duplicate canonical attributes;
- malformed Unicode;
- a DOM that does not satisfy the document/template structural rules; or
- output whose exact encoded size cannot be represented or exceeds the bound.

Selected fixtures are complete `text/html` documents. The external target
document must be created under the closed
`static-text-html-utf8-scripting-disabled-v1` parser/input context:

- the delivered body is exactly the declared fixture bytes;
- parsing uses MIME type `text/html`;
- decoding is fixed to UTF-8 through
  `Content-Type: text/html; charset=utf-8` or a capture mechanism with
  provably identical byte-delivery, MIME, and decoding semantics;
- the target `Document` has scripting disabled while parsing;
- capture begins only after its parser has completed;
- no target-document script executes;
- no mutation, custom-element reaction, event, timer, CSSOM operation, or
  post-load state affects the captured tree; and
- no uncontrolled network resource is available.

The JavaScript inspection algorithm is external capture tooling only. It must
execute in a controller, isolated inspection world, or other out-of-band
context that can read the completed standardized DOM without enabling target
document scripting and without mutating the target. Normally loading the
fixture with JavaScript enabled and subsequently running a DevTools snippet is
not equivalent and is invalid for V1. A capture mechanism that cannot prove
the required byte-delivery, MIME/UTF-8 decoding, scripting-disabled parsing,
and non-mutation conditions must classify this capture surface as unsupported
and fail closed.

The exact maximum artifact size is 8,388,608 UTF-8 bytes, including both header
lines and the final LF. Construction checks every addition before retaining
bytes. Exceeding the limit, integer overflow, encoding failure, or allocation
failure produces no artifact; truncation is forbidden.

### Projection boundaries

The Borrowser projection is derived from the production parser-created DOM in
the `CanonicalParserResult` already produced through the canonical parser
fixture execution path and its typed `ObservedTree`. It must not parse the HTML
again, reconstruct a separate DOM model, consume `html5-dom-v3` serialized
text, or introduce another parser truth path.

The external projection is a future versioned, checked-in capture algorithm
using only standardized externally observable DOM behavior: node type,
`childNodes`, `DocumentType`, `namespaceURI`, `localName`, attributes,
character data, processing-instruction data, and
`HTMLTemplateElement.content`. The algorithm is read-only. Its exact source,
version, configuration, and hashes are capture provenance. An informal manual
description or unversioned DevTools snippet is insufficient.

Stage 0 defines these boundaries but does not add either serializer or capture
script.

## External capture provenance

A completed V1 capture must retain reproducibility facts for:

- exact engine product and version, plus build/revision when available;
- platform/OS family and version, and architecture;
- the exact `static-text-html-utf8-scripting-disabled-v1` target parser/input
  context profile;
- viewport and device scale when applicable, or an explicit non-applicable
  reason;
- controlled fonts when applicable, or an explicit non-applicable reason;
- a closed resource/network policy and every pinned resource identity/hash;
- fixture source/project identity, immutable revision, and content SHA-256;
- capture mechanism/tool identity and exact version;
- capture algorithm identity/version and source SHA-256;
- capture configuration SHA-256;
- exact ordered browser/tool invocation arguments;
- explicit collection/release-channel policy identity and version; and
- artifact format, exact UTF-8 byte length, and SHA-256.

Missing required provenance fails closed. A field that is surface-dependent
cannot silently disappear; it is either applicable with a typed value or
explicitly not applicable with a non-empty reason. V1 accepts controlled
resource modes `offline`, `fixture-local-only`, and `recorded-local-closure`.
It does not accept a live, mutable network dependency as reproducible evidence.

An advisory comparison may proceed only when both sides deliberately implement
the same comparable observation version. Borrowser Layout, Paint, and debug
structures are not treated as equivalent to another engine's private
structures. AG9 V1 therefore defines only the independently observable DOM
surface above.

## `ExternalCaptureId` V1

`ExternalCaptureId` is SHA-256 of an explicit canonical preimage, displayed as
`sha256:` followed by exactly 64 lowercase hexadecimal digits. A registry
cannot choose identity independently of the validated capture.

### Domain and top-level framing

The preimage starts with these raw ASCII bytes, including the terminating NUL:

```text
borrowser-external-capture-id-v1\0
```

It is followed by exactly one field for each tag in the fixed table below, in
strictly increasing tag order. Each field is encoded as:

```text
tag     = unsigned 16-bit big-endian
length  = unsigned 64-bit big-endian payload byte length
payload = exactly `length` bytes
```

Checked conversion and addition are mandatory. Missing, duplicate, unknown,
or out-of-order fields are not constructible in the validated typed model.

### Primitive encodings

- A string payload is its raw UTF-8 bytes, with no terminator and no Unicode
  normalization. The surrounding TLV length provides framing.
- A closed enum payload is its contract-defined lowercase ASCII label as a
  string payload.
- A SHA-256 payload is exactly 32 raw digest bytes, not hexadecimal text.
- A general unsigned count or byte length is unsigned 64-bit big-endian.
- A viewport dimension is unsigned 32-bit big-endian CSS pixels.
- Device scale is a nonzero rational reduced to lowest terms and encoded as an
  unsigned 32-bit big-endian numerator followed by an unsigned 32-bit
  big-endian denominator. Floating-point and host formatting are forbidden.
- An optional value is byte `0x00` for absent, or byte `0x01` followed by an
  unsigned 64-bit big-endian nested payload length and that payload.
- Applicability is byte `0x00` followed by a length-framed, non-empty UTF-8
  non-applicable reason, or byte `0x01` followed by a length-framed canonical
  applicable value. An unknown applicability state is invalid for a completed
  capture.
- A repeated collection starts with an unsigned 32-bit big-endian count. Each
  item is an unsigned 64-bit big-endian item length followed by the canonical
  item bytes.

Semantically unordered repeated collections are validated as unique and sorted
lexicographically by complete canonical item bytes before encoding. In V1 this
rule applies only to controlled-font identities and pinned-resource identities.
Registry declaration order is not semantic for those sets.

Ordered sequences retain their authored order after validation. In particular,
raw browser/tool invocation arguments are encoded as an unsigned 32-bit
big-endian argument count followed by each argument in index order, each as an
unsigned 64-bit big-endian UTF-8 byte length and the exact argument bytes.
Arguments are not sorted or deduplicated. Repeated arguments are preserved when
the capture mechanism accepts them because position and repetition may change
invocation semantics.

### Fixed tag table

| Tag | Identity-bearing payload |
| ---: | --- |
| 1 | provenance format/version, exactly `borrowser-external-capture-provenance-v1` |
| 2 | engine product |
| 3 | exact engine version |
| 4 | optional engine build/revision identity |
| 5 | platform/OS family |
| 6 | platform/OS version |
| 7 | architecture |
| 8 | viewport applicability; applicable value is width then height as two unsigned 32-bit big-endian integers |
| 9 | device-scale applicability; applicable value is the reduced rational encoding |
| 10 | controlled-font applicability; applicable value is the canonical unordered font collection |
| 11 | resource/network policy mode |
| 12 | canonical unordered resource collection |
| 13 | fixture source/project identity |
| 14 | fixture immutable revision |
| 15 | fixture content SHA-256 |
| 16 | capture mechanism/tool identity |
| 17 | capture mechanism/tool version |
| 18 | capture algorithm identity followed by algorithm version, each as a nested length-framed string |
| 19 | capture algorithm source SHA-256 |
| 20 | capture configuration SHA-256 |
| 21 | exact ordered browser/tool invocation-argument sequence |
| 22 | external artifact format |
| 23 | external artifact UTF-8 byte length as unsigned 64-bit big-endian |
| 24 | external artifact SHA-256 |
| 25 | target parser/input context profile, exactly `static-text-html-utf8-scripting-disabled-v1` |
| 26 | collection/release-channel policy identity followed by policy version, each as a nested length-framed string |

A font item contains exactly these nested length-framed fields in order:
family string, face/style string, exact font version string, and raw 32-byte
font-file SHA-256. A resource item contains exactly a length-framed logical
resource identity followed by a raw 32-byte content SHA-256. Invocation
arguments use the ordered-sequence encoding above rather than the unordered
collection rule. Duplicate font and resource items are invalid rather than
silently deduplicated.

Raw command-line arguments are not interpreted as a semantic flag set. A
future normalized settings abstraction would require a new explicit typed
model, ordering/conflict rules, and a versioned identity contract; it cannot be
inferred from command-line strings.

### Identity exclusions

These storage, attachment, and review facts do not enter the capture-ID
preimage:

- repository-relative artifact path;
- registry path or declaration order;
- filesystem metadata or order;
- Borrowser comparison attachment;
- external advisory track ID;
- lane or execution request;
- baseline-note identity, text, or references;
- reviewer annotations;
- report path; and
- wall-clock capture or file timestamps.

Moving an artifact without changing validated provenance, format, byte length,
or content digest therefore preserves its capture ID. The fixture source
identity, revision, and content digest do participate; filesystem coincidence
does not.

Rust `Debug`, map iteration order, filesystem order, TOML order, host integer
or float formatting, and implicit Serde/library serialization are forbidden as
identity encodings.

The future loader must validate the typed provenance, read and hash the
confined artifact, verify format/length/digest, canonicalize unordered fields,
build the V1 preimage, recompute the capture ID, and compare it with the
supplied ID. Any failure rejects the complete registry before attachment,
comparison, or report publication. No partial advisory result is published.

Stage 3 must prove that identical typed captures and identical ordered argument
vectors produce the same ID independent of registry field order; changing an
argument changes the ID; reordering two arguments changes the ID; and valid
repeated arguments remain present and affect the preimage at their exact
indexes. It must separately prove canonical ordering for the genuinely
set-like font/resource collections, identity changes for every identity-bearing
field and artifact digest, storage-path non-identity, and fail-closed supplied-ID
verification.

## Attachment and baseline-note identity

The future runner-owned attachment is the typed tuple of AG2 `TestId`, AG2
observation surface, exact aggregate execution variant, and comparable surface
version. It resolves against materialized aggregate variant keys, not a path or
free-form string. Initially, external comparison is admitted only for a
singleton DOM variant and `web-observable-dom-tree-v1`.

An external advisory trend additionally uses a validated V1 advisory-track
declaration. Its invariant series identity is the tuple:

- stable advisory-track ID;
- external engine product;
- platform/OS family;
- architecture;
- comparable observation format;
- capture algorithm identity and version;
- target parser/input context profile; and
- explicit collection/release-channel policy identity and version.

This is the smallest V1 tuple that fixes the engine family, platform class,
comparable bytes, collection algorithm, parser context, and version-selection
policy while still permitting the intended exact engine-version progression.
Exact engine version/revision and platform/OS version remain capture-specific.

Every capture attached to a track must reconcile its provenance and artifact
format with all invariant track fields. A mismatch is invalid track reuse and
fails closed; it is not ordinary advisory drift. Changing a declaration's
invariant tuple under an existing track ID is likewise incompatible across
trend baselines. The collection policy is an explicit versioned identity, not
an inferred channel and never an implicit `latest` browser.

Stage 3 must test successful reconciliation, and rejection for drift in each
invariant field. Exact engine version/revision and platform/OS version changes
must remain admissible when they satisfy the unchanged versioned collection
policy.

A baseline note has its own stable note ID and always carries an explicit
attachment. It may additionally reference one exact `ExternalCaptureId`.
Changing or removing a note never changes an expected result, observed
Borrowser outcome, derived policy, or external comparison verdict.
Baseline-note and advisory-track IDs use the existing bounded semantic-ID scale
and the closed lowercase ASCII grammar
`[a-z0-9][a-z0-9-]{0,127}`; duplicate IDs fail validation.

## Fixed limits and engineering basis

Measurements were taken from the post-AG8 tree at Git revision
`59d1da77507bd059774b62a7039a25ddb18023e0`:

| Measurement | Value |
| --- | ---: |
| files below `tests/conformance` | 128 |
| total bytes below `tests/conformance` | 142,247 |
| AG2 logical fixtures in `manifest.toml` | 25 |
| AG3 records in `expected-results.toml` | 25 |
| largest current checked fixture artifact | 6,304 bytes |
| `expected-results.toml` | 15,268 bytes |
| AG8 `accounting-summary.toml` | 18,838 bytes |
| `manifest.toml` | 19,776 bytes |

The AG9 V1 bounds are:

| Retained or read data | Fixed bound | Engineering basis |
| --- | ---: | --- |
| external comparison registry source | 512 KiB | Reuses AG8's lineage-registry ceiling; over 27 times the largest current registry-like artifact and over three times the whole current conformance tree. |
| captures per registry | 256 | Matches AG8 lineage and AG2 support-path multiplicity, while current AG9 capture population is empty. |
| baseline notes per registry | 256 | Same reviewable registry multiplicity and stable attachment scale. |
| note text | 1,024 UTF-8 bytes | Reuses AG3's reason bound; 256 maximum notes retain at most 256 KiB of note text. |
| semantic identity or portable component | 128 UTF-8 bytes | Reuses AG2/AG8 test, semantic-identity, and path-component bounds. |
| ordered invocation arguments per capture | 16 | Reuses the AG8 assessment evidence-reference multiplicity while preserving exact invocation order; invocations requiring more arguments are outside V1 rather than truncated. |
| one invocation argument | 1,024 UTF-8 bytes | Reuses AG3's bounded human-evidence scale while admitting exact option/value tokens; the maximum ordered vector retains 16 KiB. |
| controlled fonts per applicable capture | 16 | Same bounded controlled-environment evidence scale; not a claim of general system-font capture. |
| capture algorithm source | 64 KiB | Reuses AG2's reviewable fixture-descriptor ceiling. |
| capture configuration source | 64 KiB | Same versioned reviewable-source ceiling. |
| comparable DOM artifact | 8 MiB | Reuses the AG4-AG7 per-observation transport/report ceiling rather than introducing a second observation-size authority. |
| first-difference excerpt | 1,024 source bytes per side | Reuses AG7's UTF-8-safe line-evidence ceiling. |
| serialized first-difference evidence | 16 KiB per comparison | Reuses AG7's reviewed evidence ceiling. |
| all retained external first differences | 4 MiB | Checked product of 256 captures and 16 KiB per comparison. |
| aggregate local detail report | 32 MiB | Reuses the existing complete-report ceiling; the aggregate detail contract does not embed complete external capture bodies. |
| each trend input and trend output | 32 MiB | Uses the versioned aggregate-detail/report ceiling; trend is local-only. |
| CI summary | exact derived V1 maximum | Its fixed row vocabulary and maximum-width unsigned counts permit an exact formula; Stage 2 must publish that derived constant and proof rather than choose a round ceiling. |

All byte and multiplicity arithmetic is checked. Bounded reads use a sentinel to
distinguish exact-boundary success from excess. Every fallible allocation is
reported as infrastructure failure. No registry, note, observation, difference,
report, or trend output is truncated.

The bounds constrain retained raw payload, not all process memory. At the
existing three independent 32 MiB subsystem evidence ceilings, a future local
aggregate may retain up to 96 MiB of subsystem evidence. A local external
detail operation may additionally retain one 8 MiB Borrowser comparable
observation, one 8 MiB external artifact, the 4 MiB difference pool, 512 KiB
registry source, 256 KiB note text, and one 32 MiB publication buffer: under
149 MiB of bounded raw payload. Normal CI has no external artifact or local
detail buffer and remains bounded by the subsystem evidence plus the exact
small summary. A local trend admits two 32 MiB inputs and one 32 MiB output.

Allocator bookkeeping, collection capacity, and typed model overhead are not
raw payload and are explicitly outside those byte sums. Implementations must
still use fallible reservation and bounded multiplicities; this document does
not present the raw-payload calculation as an exact resident-memory guarantee.

## Deterministic reports and publication boundaries

The future version labels are frozen as:

- `borrowser-conformance-aggregate-summary-v1`;
- `borrowser-conformance-aggregate-detail-v1`;
- `borrowser-conformance-trend-v1`;
- `borrowser-cross-engine-comparison-registry-v1`; and
- `borrowser-external-capture-provenance-v1`.

The comparable DOM and capture-ID versions remain
`web-observable-dom-tree-v1` and `borrowser-external-capture-id-v1`.

Future aggregate accounting consumes typed normalized runner results or a
deliberate typed projection. It must not serialize parser, CSS, or rendering
reports and parse those bytes back into an aggregate model. Existing detailed
subsystem reports remain stable and authoritative.

The planned CI-safe summary and local detailed report derive from the same
typed run. The summary is bounded and fixed-shape. Local detail may retain all
orthogonal states and bounded evidence but does not make external evidence
authoritative. Both are fully constructed and validated before the first byte
is published. Paths are omitted or repository-relative wherever identity does
not require a path. Ordering is by typed stable identities, never discovery or
filesystem order.

Normal CI will use the named `normal-ci` policy and the empty environment
assessment. No external browser, current-host browser discovery, external
capture registry, or network lookup is required for that summary.

## Deterministic trend semantics

A trend compares exactly two explicit, deterministic aggregate-detail
baselines. Both input paths and their SHA-256 identities are explicit. The
baselines must share:

- the same AG inventory scope, currently `static-html-css-no-js`;
- the same aggregate/granularity contract version;
- the same named lane; and
- the same execution-environment assessment mode, currently AG9's empty
  assessment.

These fields define the source-set domain. Different exact member/source-set
digests within that same domain are permitted and produce explicit added and
removed accounting. A different inventory/scope domain is incompatible and
fails closed even if both artifacts use an aggregate-detail container version.
AG9 must not compare a future dynamic Browser/runtime population or a full-WPT
population with the static no-JavaScript population merely because both can be
decoded.

```text
same source-set domain + changed membership
    -> valid trend with added/removed accounting

different inventory/scope domain
    -> incompatible baselines; fail closed
```

Trend output contains four separate populations, each with added, removed,
unchanged, and changed accounting:

1. logical cases;
2. execution variants;
3. external advisory comparisons; and
4. baseline notes.

A Borrowser execution-variant fingerprint contains only Borrowser/AG state:
classification, engine capability, harness readiness, requirements,
eligibility, expectation, actual named-lane selection, stability, attempt
state and not-attempted reason, Borrowser terminal outcome, derived policy, and
Borrowser oracle/comparison kind. A logical-case fingerprint uses only its AG
metadata and its Borrowser variant keys/fingerprints.

External engine/product/version, external capture ID, artifact hash, advisory
equivalent/different verdict, and baseline-note data are excluded from both
Borrowser fingerprints. Changing only external evidence or a note cannot make
a Borrowser logical case or variant appear changed.

External advisory comparisons are keyed by the typed comparison attachment and
a stable advisory-track ID. Exact capture/provenance or advisory-verdict drift
is reported only in that population. Notes are keyed by stable note ID; note
text, attachment, or optional capture-reference drift is reported only in the
note population.

There is no implicit previous run, wall clock, host-local database, network
lookup, filesystem `latest`, or moving `latest Chrome/Firefox/WebKit` identity.
AG9 reports no compatibility percentage. Such a percentage would be invalid
without an explicit denominator, population, lane, granularity, exclusions,
source set, and evidence authority.

## External-browser and CI boundaries

External collection remains a manual, local workflow for AG9. A later stage
may add a versioned, reviewable DOM-inspection script, but normal CI will not:

- install, download, discover, or launch an external browser;
- add Playwright, WebDriver, or browser-driver dependencies;
- access a live network;
- capture screenshots or raster surfaces; or
- execute broad WPT infrastructure.

AG9 adds no raster or screenshot comparison, perceptual/fuzzy diff, browser
automation, JavaScript engine, DOM bindings, events, timers, CSSOM, dynamic
mutation, navigation, or production browser behavior. Full cross-browser
automation and broad WPT/browser compatibility remain explicit non-claims.

## Stage 0 decision

Stage 0 is complete when this contract and its documentation index entry are
reviewable and consistent with AG1 through AG8. It deliberately adds no Rust
contract primitive: none is required to freeze the terminology, byte grammar,
hash preimage, limits, or future ownership before Stage 1.

Later AG9 stages must implement this contract incrementally. The existence of
this document must not be reported as deterministic aggregate reporting,
working cross-engine comparison, trend support, or completed AG9 infrastructure.
