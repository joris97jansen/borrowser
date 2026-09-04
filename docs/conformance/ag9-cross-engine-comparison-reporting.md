# AG9 cross-engine comparison and conformance reporting contract

Status: Stage 0 contract and limits freeze, AG9 Stage 1 typed aggregate
execution/accounting, AG9a deterministic aggregate summary/detail reports, and
AG9b source-neutral capture provenance plus runner-owned advisory registry are
implemented; external capture tooling, external DOM comparison, trend
execution, and aggregate CLI/CI publication remain unimplemented

Last updated: 2026-09-03

AG9 defines the future aggregate-accounting, reporting, cross-engine evidence,
baseline-note, and trend contracts for Borrowser's current static HTML/CSS
conformance harness. Stage 1 adds the typed aggregate runner and accounting
projection. AG9a adds the two bounded aggregate V1 report projections defined
below. Neither stage loads an external capture registry, runs an external
browser, compares an external capture, calculates a trend, or changes a command
or CI job.

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

### Authoritative `AggregateRun` sealing

`AggregateRun` is sealed from primary case state only. Its canonical
construction boundary accepts inventory scope, named request, the typed
environment-assessment mode, and the complete `AggregateCaseResult`
population. Callers do not supply `AggregateAccounting` or the root logical
source-set digest.

Before construction succeeds, sealing validates fixture/case identity and
scope, observation ownership, source-identity branch consistency, AG3 branch
shape, unique logical and variant identities, exact member digests, variant
ownership and subsystem projections, requested-lane selection, and the frozen
eligibility/selection/attempt relationships. It then calls the existing
aggregate-accounting owner over those exact cases and the existing identity
owner over those exact `TestId`/member-digest pairs. Consequently every
successfully constructed run makes both relationships structural:

```text
run.accounting() == build_accounting(run.cases())

run.logical_case_source_set_digest()
    == source_set_digest(
           run.inventory_scope(),
           every run case TestId/member-digest pair
       )
```

Neither the model nor reporting duplicates subsystem projection, accounting,
or hashing semantics. Reports receive an immutable sealed run and only project
its typed state.

Parser and CSS subsystem evidence retain their complete normalized case state.
Rendering retains the authoritative `AgCaseState` from the originating
`RenderingCaseResult` exactly once as logical-case evidence, while each
materialized variant retains its subsystem-owned `RenderingVariantResult`
losslessly. Sealing first requires that the case-level rendering evidence equal
the aggregate logical case exactly, then independently validates every variant
identity, execution, policy, and comparison projection. In particular,
changing an aggregate rendering expectation cannot preserve old rendering
evidence or an old expected-pass policy: the changed case must be executed and
derived again.

## Eligibility, named-lane selection, and attempts

AG9 aggregate execution seals the applied assessment policy in
`AggregateRun` as `AggregateEnvironmentAssessmentMode`. AG9a exposes exactly
one variant, `EmptyV1`. The runner constructs that mode alongside the existing
`ExecutionEnvironmentAssessment::empty()` used for eligibility evaluation;
reports project the mode from the completed run rather than supplying run
context at serialization time. AG9 does not expose a caller-constructible
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

The Stage 1 aggregate terminal vocabulary is:

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

The later comparable-DOM projection/capture stage must prove this invariant
with representative HTML-, SVG-, and MathML-namespace parser-created elements.
The external capture algorithm must
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

Stage 0 defines these boundaries. AG9b adds the complete external-artifact
validator, but it does not add either serializer/capture script or a
Borrowser-side comparable-DOM projection.

### AG9b external-artifact validation boundary

For AG9b, verifying the declared `web-observable-dom-tree-v1` format means
validating the complete bounded artifact grammar and structural rules above.
Checking only the first `format` line, searching for the format label, or
accepting arbitrary UTF-8 under a recognized declaration is insufficient. The
validator consumes the exact artifact bytes and must verify, without recursive
call-stack traversal:

- the 8 MiB byte ceiling, UTF-8 validity, LF-only physical lines, absence of
  blank lines, and exactly one final LF;
- the exact two-line header, field spelling and order, canonical unsigned
  decimal and string encodings, and complete consumption with no trailing
  records;
- exactly one document root, matched node begin/end kinds, counted child,
  template-child, and attribute records, and the document/template structural
  rules;
- the closed node kinds, supported element and attribute namespaces, required
  namespace/prefix/qualified-name relationships, and canonical attribute
  ordering; and
- rejection of duplicate canonical attributes, unsupported states, malformed
  escapes or Unicode, count overflow, size arithmetic overflow, and fallible
  allocation failure.

This validation establishes only that the bytes are a canonical instance of
the independently specified comparable surface. It does not compare them with
Borrowser, prove that an external browser produced them, or implement the
external capture algorithm. The capture provenance and its identity-bearing
algorithm/configuration digests remain the reproducibility claim for how the
artifact was produced.

### Confined read threat model and verified-byte ownership

AG9 capture and registry inputs do not assume that a repository pathname stays
bound to the same filesystem object between a metadata check and a later
open. A local process may concurrently replace a path component or final file.
The current `external-test-provenance::read_confined_regular_file` first checks
the pathname and then calls `fs::read` by pathname, so it is not sufficient for
this stronger boundary.

The AG9b implementation must use a same-opened-object confined read. It opens
the fixed repository root and traverses each validated relative component
without following symlinks, opens the final component without following a
symlink, establishes regular-file status and the bounded length from that
opened object, and performs the sentinel-bounded read from that same object.
If the host cannot provide those guarantees, loading fails closed. A path
replacement after the final object is opened cannot redirect the read.

Concurrent writes to the opened regular file are not treated as trusted
metadata. Trust attaches only to the exact bounded byte sequence actually
read: declared length, full format validation, SHA-256, and capture-ID
recomputation all operate on those same bytes. A validated capture owns that
exact immutable byte sequence. Downstream comparison must borrow or consume
those retained bytes; it must never reopen `artifact_path`. Revalidation may
construct a new validated capture from a new same-object read, but it does not
change the bytes owned by an existing validated capture.

To make that lifetime compatible with the frozen retained-payload model, one
loaded registry has an 8 MiB cumulative verified-artifact-byte ceiling in
addition to the 8 MiB per-artifact ceiling. The loader validates and retains
all capture bytes before the sealed registry can escape. Thus a valid registry
may contain up to 256 small captures or one maximum-size capture, but cannot
silently retain 256 maximum-size artifacts.

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

The AG9b loader must validate the typed provenance, read and hash the
confined artifact, verify format/length/digest, canonicalize unordered fields,
build the V1 preimage, recompute the capture ID, and compare it with the
supplied ID. Any failure rejects the complete registry before attachment,
comparison, or report publication. No partial advisory result is published.

### Construction authority

`ExternalCaptureId` is an opaque computed identity, not a validated spelling
for an arbitrary caller-supplied digest. Public construction from
`Sha256Digest`, raw digest bytes, hexadecimal text, Serde deserialization, or
an unchecked struct literal is forbidden. The only production constructor
builds the exact domain-separated TLV preimage above from fully validated typed
capture provenance plus the verified artifact facts and returns the resulting
identity.

The registry's `capture_id` string is parsed into a distinct untrusted claim
type. References in attachment and note wire records are likewise unresolved
claims until registry reconciliation. The loader computes
`ExternalCaptureId`, compares its exact 32 digest bytes with the supplied
claim, and rejects a mismatch before exposing either the capture or any
reference to it. A successfully reconciled public model carries only computed
`ExternalCaptureId` values or references to validated captures; it cannot turn
a syntactically valid supplied SHA-256 into trusted identity.

This authority boundary does not alter any Stage 0 identity byte. The domain,
tags, framing, primitive encodings, optional/applicability encodings, ordered
argument sequence, and canonical set encodings above remain exact. In
particular, TOML bytes, Serde output, Rust `Debug`, host formatting, and
collection iteration never become identity input.

The AG9b implementation must prove that identical typed captures and identical
ordered argument vectors produce the same ID independent of registry field
order; changing an argument changes the ID; reordering two arguments changes
the ID; and valid repeated arguments remain present and affect the preimage at
their exact indexes. It must separately prove canonical ordering for the
genuinely set-like font/resource collections, identity changes for every
identity-bearing field and artifact digest, storage-path non-identity, and
fail-closed supplied-ID verification.

## Attachment and baseline-note identity

The AG9b runner-owned attachment is the typed tuple of AG2 `TestId`, AG2
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

AG9b must test successful reconciliation, and rejection for drift in each
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

## Cross-engine comparison registry V1

AG9b freezes one repository-owned registry at exactly:

```text
tests/conformance/external/cross-engine-comparisons.toml
```

External artifact files are stored directly below exactly:

```text
tests/conformance/external/captures/
```

An `artifact_path` is the full repository-relative spelling
`tests/conformance/external/captures/<file>`. `<file>` is one AG2 portable path
component of at most 128 ASCII bytes and ends in
`.web-observable-dom-tree-v1.txt`. Nested directories, absolute paths,
backslashes, dot components, parent traversal, symlinks, and alternate capture
roots are invalid in V1. The filename is storage metadata: it need not contain
the capture ID and changing it within this layout does not change
`ExternalCaptureId`.

This path rule reuses the authoritative AG2 portable path-component grammar;
AG9b must not copy or independently reimplement that grammar in
`conformance-runner`. Although `PortablePathComponent` is currently
crate-private, implementation planning must prefer exposing the minimum
parsing/validation API from its owning `conformance-test-support` crate.

The registry is UTF-8 TOML with exactly these top-level fields:

```toml
format = "borrowser-cross-engine-comparison-registry-v1"
captures = []
attachments = []
advisory_tracks = []
baseline_notes = []
```

All five fields are required. The four collections are TOML arrays; a
non-empty array may use TOML array-of-tables syntax. Each collection may be
empty, so the exact empty registry above is valid. TOML field order and array
declaration order are non-semantic. Unknown top-level, record, or nested-table
fields; missing fields; duplicate TOML keys; wrong TOML value kinds; and TOML
datetime or floating-point substitutions are invalid. Optional fields are
omitted rather than represented by an empty string or sentinel string.

Unless a field below has a narrower grammar, a bounded identity/version value
is 1 to 128 UTF-8 bytes, has no leading or trailing whitespace or Unicode
control scalar, and is not Unicode-normalized. A non-applicable reason and
baseline-note text are 1 to 1,024 UTF-8 bytes under the same whitespace/control
rule. A SHA-256 wire value is exactly 64 lowercase hexadecimal digits; only a
`capture_id` claim adds the exact `sha256:` prefix. An immutable revision is 1
to 256 UTF-8 bytes under the existing source-neutral revision rules.

### `captures`

Each capture record has exactly these fields. Only
`engine_build_revision` is optional.

| Field | Wire value and rule | Identity role |
| --- | --- | --- |
| `capture_id` | `sha256:` plus 64 lowercase hexadecimal digits; an untrusted supplied claim | recomputed result, never direct construction input |
| `artifact_path` | canonical repository-relative path under the fixed capture root above | storage-only |
| `provenance_format` | exactly `borrowser-external-capture-provenance-v1` | tag 1 |
| `engine_product` | bounded identity | tag 2 |
| `engine_version` | bounded version | tag 3 |
| `engine_build_revision` | optional bounded identity; omission encodes absence | tag 4 |
| `platform_os_family` | bounded identity | tag 5 |
| `platform_os_version` | bounded version | tag 6 |
| `architecture` | bounded identity | tag 7 |
| `viewport` | closed applicability table below | tag 8 |
| `device_scale` | closed applicability table below | tag 9 |
| `controlled_fonts` | closed applicability table below | tag 10 |
| `resource_network_policy` | `offline`, `fixture-local-only`, or `recorded-local-closure` | tag 11 |
| `pinned_resources` | required array of zero to 32 resource records | tag 12 |
| `fixture_source_project` | bounded identity | tag 13 |
| `fixture_immutable_revision` | immutable revision | tag 14 |
| `fixture_content_sha256` | SHA-256 | tag 15 |
| `capture_mechanism` | bounded identity | tag 16 |
| `capture_mechanism_version` | bounded version | tag 17 |
| `capture_algorithm` | bounded identity | first nested tag-18 string |
| `capture_algorithm_version` | bounded version | second nested tag-18 string |
| `capture_algorithm_source_sha256` | SHA-256 | tag 19 |
| `capture_configuration_sha256` | SHA-256 | tag 20 |
| `invocation_arguments` | required ordered array of zero to 16 UTF-8 strings, each at most 1,024 bytes; empty and repeated arguments are valid | tag 21 |
| `artifact_format` | exactly `web-observable-dom-tree-v1` | tag 22 |
| `artifact_utf8_byte_length` | unsigned TOML integer representable as `u64`, no greater than 8,388,608 | tag 23 |
| `artifact_sha256` | SHA-256 | tag 24 |
| `target_parser_input_context` | exactly `static-text-html-utf8-scripting-disabled-v1` | tag 25 |
| `collection_policy` | bounded identity | first nested tag-26 string |
| `collection_policy_version` | bounded version | second nested tag-26 string |

The three applicability tables are closed tagged unions:

```toml
viewport = { applicability = "applicable", width_css_px = 1280, height_css_px = 720 }
viewport = { applicability = "not-applicable", reason = "surface-independent" }

device_scale = { applicability = "applicable", numerator = 1, denominator = 1 }
device_scale = { applicability = "not-applicable", reason = "surface-independent" }

controlled_fonts = { applicability = "applicable", items = [{ family = "Ahem", face_style = "regular", version = "1", file_sha256 = "0000000000000000000000000000000000000000000000000000000000000000" }] }
controlled_fonts = { applicability = "not-applicable", reason = "font-independent" }
```

Applicable viewport dimensions are unsigned TOML integers representable as
`u32` CSS pixels, including zero. Applicable device-scale numerator and
denominator are nonzero `u32` values and must already be reduced to lowest
terms. An applicable font collection contains 1 to 16 items. Font `family`,
`face_style`, and `version` are bounded identity/version strings;
`file_sha256` is a SHA-256. Non-applicable branches contain exactly
`applicability` and `reason`.

Each `pinned_resources` item contains exactly `identity` and
`content_sha256`; the former is a bounded identity and the latter a SHA-256.
Font and resource declaration order is non-semantic. Complete canonical item
bytes are sorted before capture-ID encoding, and duplicate complete canonical
items fail validation rather than being deduplicated. Invocation argument order
is semantic, uses the authored array indexes, and preserves duplicates.

`resource_network_policy = "offline"` does not require an empty resource
array: pinned fixture-local bytes may still be part of the closed input even
when no network fetch is permitted. None of the three modes permits live or
undeclared resource access.

### `advisory_tracks`

Each advisory-track record has exactly these required fields:

| Field | Rule |
| --- | --- |
| `track_id` | semantic ID matching `[a-z0-9][a-z0-9-]{0,127}` |
| `engine_product` | bounded identity |
| `platform_os_family` | bounded identity |
| `architecture` | bounded identity |
| `comparable_observation_surface` | exactly `web-observable-dom-tree-v1` |
| `capture_algorithm` | bounded identity |
| `capture_algorithm_version` | bounded version |
| `target_parser_input_context` | exactly `static-text-html-utf8-scripting-disabled-v1` |
| `collection_policy` | bounded identity |
| `collection_policy_version` | bounded version |

These fields are exactly the invariant series tuple defined above. Engine
version/build and platform version deliberately do not occur in a track.

### `attachments`

Each comparison-attachment record has exactly these required fields:

| Field | Rule |
| --- | --- |
| `test_id` | parsed by the authoritative AG2 `TestId` type |
| `observation_surface` | parsed by the authoritative AG2 `ObservationSurface` type; V1 admits only `dom-tree` |
| `execution_variant` | closed table exactly `{ kind = "singleton" }` |
| `comparable_observation_surface` | exactly `web-observable-dom-tree-v1` |
| `track_id` | reference to one validated advisory track |
| `capture_id` | reference to one computed, validated external capture ID |

The first four fields form the typed comparison attachment. The
`execution_variant` table decodes directly to the existing runner-owned
`AggregateExecutionVariantId::Singleton` value. V1 defines no rendering branch
and no opaque variant string, environment label, width encoding, or duplicate
rendering-variant grammar. Reconciliation requires an exact matching
`AggregateVariantKey` in the sealed `AggregateRun`, not merely a matching
`TestId` or filesystem path.

Likewise, `observation_surface` consumes the authoritative AG2
`ObservationSurface` vocabulary. AG9b must not duplicate its string parser in
the runner merely because `ObservationSurface::parse` is currently
crate-private. Implementation planning must prefer the minimum public parsing
API from the owning `conformance-test-support` crate.

One registry may contain at most one attachment for the key `(typed comparison
attachment, track_id)`. Repeating that key is a duplicate even if the records
name different captures. This gives one exact external observation for one
track/case/variant/surface point in a baseline. One capture may otherwise be
referenced by more than one distinct attachment.

### `baseline_notes`

Each baseline-note record has exactly these fields:

| Field | Rule |
| --- | --- |
| `note_id` | semantic ID matching `[a-z0-9][a-z0-9-]{0,127}` |
| `test_id` | authoritative AG2 `TestId` |
| `observation_surface` | authoritative AG2 surface; V1 admits only `dom-tree` |
| `execution_variant` | closed table exactly `{ kind = "singleton" }` |
| `comparable_observation_surface` | exactly `web-observable-dom-tree-v1` |
| `text` | 1 to 1,024 UTF-8 bytes under the bounded text rule above |
| `capture_id` | optional reference to one computed, validated external capture; omission means no capture reference |

The four attachment fields use the same typed decoding and exact aggregate
reconciliation as comparison attachments. A note does not acquire a track or
comparison verdict. Its optional capture need only exist in the same validated
registry; the note itself supplies the exact typed case attachment.

Capture, track, and note IDs are unique in their respective collections.
Unreferenced validated captures and tracks are permitted, but do not
materialize an advisory comparison. Artifact-path reuse is permitted because
paths are storage metadata; every capture declaration is still independently
validated and its ID recomputed. References to missing or invalid captures or
tracks fail closed. No record is selected by declaration position.

### Capture algorithm and configuration source semantics

AG9b validates the algorithm/configuration identity and version strings and the
two declared SHA-256 values, and includes them in capture identity exactly as
tags 18 through 20 require. It does not load, hash, or execute capture
algorithm or configuration source bytes. V1 has no algorithm-source path,
configuration-source path, inline script, or executable command field.

The 64 KiB algorithm-source and configuration-source limits are reserved
ceilings for the later checked-in capture-tool stage. That stage must define
the source layout and verify those exact bytes against the already
identity-bearing digests before producing a registry claim. Until then, AG9b
treats the digests as validated provenance claims, not proof that the omitted
source is present. Adding source paths or source-byte verification to this
registry requires an explicit contract amendment; it must not happen through
an undocumented optional field. AG9b therefore does not accidentally become
the external capture script or its executor.

## Deterministic registry validation

Registry loading is deterministic fail-fast at one typed diagnostic. It does
not return a declaration-order-dependent collection of errors. Each phase
evaluates its bounded candidates, chooses the least diagnostic by the key
defined below, and stops before the next phase when any candidate exists:

1. fixed registry-path confinement, same-object open, regular-file status,
   512 KiB sentinel-bounded read, and UTF-8;
2. TOML syntax, the closed wire shape above, required fields, and exact registry
   format;
3. top-level capture, attachment, advisory-track, and baseline-note
   cardinalities, plus checked declared cumulative artifact length;
4. record-local typed field validation, per-capture invocation-argument,
   controlled-font, and pinned-resource cardinalities, and conversion of every
   individual font/resource item into its canonical typed representation;
5. uniqueness over the already-valid canonical capture, track, note,
   font/resource item, and attachment identities;
6. same-object artifact reads, actual cumulative byte accounting, full
   `web-observable-dom-tree-v1` validation, declared length/digest checks, and
   capture-ID recomputation;
7. internal capture/track references and every advisory-track invariant; and
8. exact attachment and note reconciliation against the immutable sealed
   `AggregateRun` population.

Phase 2 maps any TOML syntax failure, duplicate TOML key, missing/unknown field,
or wrong TOML value kind to one typed `InvalidRegistrySchema` category. Parser
messages, source spans, and incidental library wording are not stable
diagnostics. `UnsupportedRegistryFormat` is distinct when the closed shape is
otherwise available.

Phase 4 does not test collection uniqueness. It establishes that every
individual field and nested item is valid and canonicalizable. Phase 5 alone
detects duplicate canonical identities, including controlled fonts and pinned
resources. Therefore any phase-4 record-local diagnostic takes precedence over
every phase-5 duplicate diagnostic in the same registry, regardless of subject
or declaration order.

Within phase 4, storage preparation and record validation proceed in frozen
subject-kind order: captures, advisory tracks, attachments, then baseline
notes. For one subject kind, its bounded output-collection reservation is
attempted first, every authored record of that kind is then validated, and the
least record diagnostic is selected before the next subject kind is touched.
Consequently a later-kind allocation failure cannot procedurally outrank an
earlier-kind record failure. Phase 8 analogously reserves and fully reconciles
attachments before it attempts baseline-note output storage or note
reconciliation.

The `artifact_path` spelling and AG2 portable-component grammar are capture
field validation in phase 4. `ArtifactPathUnsafe` in phase 6 is reserved for a
same-object confinement/open refusal discovered for an already-valid spelling;
it must not be used to reclassify a phase-4 wire-path error. Phase 6 accumulates
actual artifact bytes only after each artifact's actual length equals its
phase-3-accounted declaration. Consequently actual cumulative bytes cannot
exceed the validated declared cumulative ceiling; a per-artifact length
mismatch is `ArtifactLengthMismatch`, not a second cumulative-limit category.

The comparable-surface wire label is also a phase-4 field. A value other than
`web-observable-dom-tree-v1` is `UnsupportedComparableSurface` in phase 4.
Phase 8 uses `UnknownObservationSurface` only when the parsed AG2 surface does
not resolve for the named `TestId`, and `UnknownExecutionVariant` only when the
parsed singleton variant does not resolve in the sealed aggregate population.

Specialized record-local categories exclusively own their named conditions.
`InvalidCaptureField`, `InvalidTrackField`, `InvalidAttachmentField`, and
`InvalidNoteField` are fallbacks only when no more specific phase-4 category
applies. In particular, capture-ID claim syntax, nested collection ceilings,
invocation-argument length, track/note ID syntax, comparable-surface support,
and baseline-note text length produce their dedicated categories exactly once.
This prevents one invalid field from generating candidates at two diagnostic
kind ranks.

Within later phases, captures sort by supplied capture-ID claim bytes, tracks
by raw track-ID bytes, notes by raw note-ID bytes, and attachments by the raw
tuple `(test_id, observation_surface, execution_variant kind,
comparable_observation_surface, track_id, capture_id)`. Schema validation
guarantees these raw keys are present strings/tables before they are used for
ordering. Font and resource candidates sort by their complete canonical item
bytes. Invocation-argument diagnostics use the authored argument index because
that sequence is intentionally ordered. Duplicate identical diagnostics
collapse to the same typed key.

The stable diagnostic key is:

```text
(phase rank, subject-kind rank, canonical subject key,
 diagnostic-kind rank, typed detail key)
```

Subject-kind order is registry, capture, track, attachment, note, artifact.
For each capture, track, attachment, note, or artifact family, the collection
subject sorts before every individual subject in that family. Thus the exact
relative orders are capture collection before capture by supplied ID, track
collection before track by track ID, attachment collection before attachment
by its six-component raw tuple, note collection before note by note ID, and
artifact collection before artifact by supplied capture ID. Registry has only
its singleton registry subject.

The typed-detail variant order is exactly:

1. no detail;
2. field;
3. record collection;
4. invocation-argument index;
5. track-invariant field; and
6. diagnostic component.

Values within one invocation-argument detail compare by numeric `usize`
index. The record-collection detail order is captures, attachments, advisory
tracks, baseline notes. The track-invariant detail order is engine product, OS
family, architecture, comparable observation surface, capture algorithm,
capture algorithm version, parser/input context, collection policy, collection
policy version.

The complete diagnostic-field detail order is:

```text
allocation
architecture
artifact-format
artifact-length
artifact-path
artifact-sha256
attachment-key
capture-algorithm
capture-algorithm-source-sha256
capture-algorithm-version
capture-configuration-sha256
capture-id
capture-mechanism
capture-mechanism-version
collection-policy
collection-policy-version
comparable-observation-surface
controlled-fonts
controlled-fonts-allocation
device-scale
engine-build-revision
engine-product
engine-version
execution-variant
fixture-content-sha256
fixture-immutable-revision
fixture-source-project
font-canonical
font-face-style
font-family
font-sha256
font-version
invocation-arguments
note-id
observation-surface
pinned-resources
platform-os-family
platform-os-version
provenance-format
format
resource-canonical
resource-identity
resource-network-policy
resource-sha256
resources-allocation
target-parser-input-context
test-id
text
track-id
viewport
```

In this field-detail domain, `format` is the registry-format field identity.

The complete diagnostic-component detail order is:

```text
actual-byte-sum
artifact-length-sum
artifact-read
byte-length
capture-validation
closed-schema
cumulative-length-invariant
parser-dom-owner
provenance-invariant
registry-read
utf8
validated-capture-reference
```

These are closed semantic orders, not alphabetic sorting rules and not Rust
enum declaration order. Adding or reordering a V1 field, collection,
track-invariant, component, or detail variant requires a contract amendment.

Diagnostic-kind rank is the order in this closed category table:

| Rank group | Typed diagnostic categories in order |
| --- | --- |
| registry read (phase 1) | `RegistryPathUnsafe`, `RegistryMissing`, `RegistrySymlink`, `RegistryNotRegular`, `RegistryTooLarge`, `RegistryReadFailure`, `RegistryInvalidUtf8` |
| schema/version (phase 2) | `InvalidRegistrySchema`, `UnsupportedRegistryFormat` |
| top-level multiplicity (phase 3) | `TooManyCaptures`, `TooManyAttachments`, `TooManyAdvisoryTracks`, `TooManyBaselineNotes`, `DeclaredArtifactBytesOverflow`, `CumulativeArtifactBytesExceeded` |
| record-local (phase 4) | `InvalidCaptureIdClaim`, `InvalidCaptureField`, `TooManyInvocationArguments`, `InvocationArgumentTooLong`, `TooManyControlledFonts`, `TooManyPinnedResources`, `InvalidTrackId`, `InvalidTrackField`, `InvalidAttachmentField`, `UnsupportedComparableSurface`, `InvalidNoteId`, `InvalidNoteField`, `BaselineNoteTextTooLong` |
| duplicate canonical identity (phase 5) | `DuplicateCaptureId`, `DuplicateControlledFont`, `DuplicatePinnedResource`, `DuplicateTrackId`, `DuplicateAttachmentKey`, `DuplicateNoteId` |
| artifact/identity (phase 6) | `ArtifactPathUnsafe`, `ArtifactMissing`, `ArtifactSymlink`, `ArtifactNotRegular`, `ArtifactTooLarge`, `ArtifactReadFailure`, `ActualArtifactBytesOverflow`, `ArtifactLengthMismatch`, `ArtifactDigestMismatch`, `ArtifactFormatInvalid`, `CaptureIdMismatch` |
| internal reconciliation (phase 7) | `UnknownCaptureReference`, `UnknownTrackReference`, `TrackInvariantMismatch` |
| aggregate reconciliation (phase 8) | `UnknownTestId`, `UnknownObservationSurface`, `UnknownExecutionVariant`, `AggregateAttachmentMismatch` |

Human-readable `Display` text may improve without changing this contract. Rust
`Debug`, TOML field/declaration order, filesystem enumeration, hash-map order,
and parser-library error text are neither diagnostic identities nor sorting
keys. Checked conversion, reservation, or arithmetic failure maps to the typed
category for the phase/field being processed and fails closed.

No partially parsed, partially verified, or partially reconciled registry is
public. Only completion of all eight phases can construct the opaque validated
registry. Invalid captures cannot be skipped to preserve valid ones, and
attachments or notes cannot escape without the complete capture/track graph
and exact aggregate population having reconciled.

## Advisory evidence authority separation

The sealed `AggregateRun` remains complete Borrowser truth before the external
registry is loaded. Registry reconciliation takes an immutable borrow of that
run and constructs a separate runner-owned advisory-evidence value. The
advisory value may refer to aggregate variant keys, but it is not stored in or
permitted to mutate:

- AG3 classification, expectation, stability, capability, harness, or lane
  metadata;
- parser, CSS, Layout, Paint, or Browser/runtime subsystem results;
- aggregate attempt state, terminal outcome, comparison/oracle kind, or
  derived policy;
- aggregate accounting, logical member/source-set identity, named-lane
  selection, or CI verdict.

Changing, moving, adding, or removing only registry captures, tracks,
attachments, or notes leaves `AggregateRun`, its accounting, its logical-case
source-set digest, its Borrowser fingerprints, and its existing summary/detail
bytes unchanged. A future report may project both immutable inputs into
separate advisory sections, but external fields never enter Borrowser
fingerprints or pass/fail derivation. AG9b adds neither that publication nor an
external comparison verdict.

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
| typed comparison attachments per registry | 256 | One attachment can consume one capture and later one 16 KiB difference slot; this preserves the existing checked 4 MiB evidence-pool derivation. |
| advisory tracks per registry | 256 | Matches the capture and reviewable lineage multiplicity while allowing every current-baseline attachment to have an independent series. |
| baseline notes per registry | 256 | Same reviewable registry multiplicity and stable attachment scale. |
| note text | 1,024 UTF-8 bytes | Reuses AG3's reason bound; 256 maximum notes retain at most 256 KiB of note text. |
| semantic identity or portable component | 128 UTF-8 bytes | Reuses AG2/AG8 test, semantic-identity, and path-component bounds. |
| ordered invocation arguments per capture | 16 | Reuses the AG8 assessment evidence-reference multiplicity while preserving exact invocation order; invocations requiring more arguments are outside V1 rather than truncated. |
| one invocation argument | 1,024 UTF-8 bytes | Reuses AG3's bounded human-evidence scale while admitting exact option/value tokens; the maximum ordered vector retains 16 KiB. |
| controlled fonts per applicable capture | 16 | Same bounded controlled-environment evidence scale; not a claim of general system-font capture. |
| pinned resources per capture | 32 | Reuses AG8's `WPT_MAX_CLOSURE_FILES_PER_RECORD`: a capture resource set is the analogous closed per-input dependency closure, while the current AG8 proof needs only one static resource. |
| later capture-tool algorithm source | 64 KiB | Reserved for the later checked-in capture-tool stage and reuses AG2's reviewable fixture-descriptor ceiling; AG9b retains only its declared SHA-256. |
| later capture-tool configuration source | 64 KiB | Reserved for the later capture-tool stage under the same reviewable-source ceiling; AG9b retains only its declared SHA-256. |
| comparable DOM artifact | 8 MiB | Reuses the AG4-AG7 per-observation transport/report ceiling rather than introducing a second observation-size authority. |
| cumulative verified external artifact bytes per loaded registry | 8 MiB | Preserves Stage 0's one-artifact-pool retained-memory model while allowing up to 256 small captures and ensuring every validated capture owns the exact bytes later consumed. |
| first-difference excerpt | 1,024 source bytes per side | Reuses AG7's UTF-8-safe line-evidence ceiling. |
| serialized first-difference evidence | 16 KiB per comparison | Reuses AG7's reviewed evidence ceiling. |
| all retained external first differences | 4 MiB | Checked product of 256 typed comparison attachments and 16 KiB per comparison. |
| aggregate local detail report | 32 MiB | Reuses the existing complete-report ceiling; the aggregate detail contract does not embed complete external capture bodies. |
| each trend input and trend output | 32 MiB | Uses the versioned aggregate-detail/report ceiling; trend is local-only. |
| CI summary | 6,073 bytes | AG9a derives the exact syntactic V1 ceiling from its fixed row vocabulary, identity fields, longest stable labels, framing, and 59 maximum-width unsigned counts. |

All byte and multiplicity arithmetic is checked. Bounded reads use a sentinel to
distinguish exact-boundary success from excess. Every fallible allocation is
reported as infrastructure failure. No registry, note, observation, difference,
report, or trend output is truncated.

The bounds constrain retained raw payload, not all process memory. At the
existing three independent 32 MiB subsystem evidence ceilings, a future local
aggregate may retain up to 96 MiB of subsystem evidence. A local external
detail operation may additionally retain one 8 MiB Borrowser comparable
observation, one cumulative 8 MiB verified external-artifact pool, the 4 MiB
difference pool, 512 KiB registry source, 256 KiB note text, and one 32 MiB
publication buffer: under 149 MiB of bounded raw payload. The 32-resource
per-capture ceiling does not add another independent retained pool: all
resource identities and digests are declarations inside the already bounded
512 KiB registry source, and capture-ID preimages are built and discarded one
capture at a time with checked arithmetic. Normal CI has no external artifact
or local detail buffer and remains bounded by the subsystem evidence plus the
exact small summary. A local trend admits two 32 MiB inputs and one 32 MiB
output.

At 128 identity bytes, one maximum resource item is 168 canonical bytes: one
8-byte nested string length, 128 identity bytes, and 32 digest bytes. Tag 12's
maximum collection payload is therefore exactly
`4 + 32 * (8 + 168) = 5,636` bytes, including the collection count and each
outer item length. The tag/length frame adds 10 more bytes to the capture-ID
preimage. These calculations and the `u32` item count are checked rather than
inferred from allocation capacity.

Allocator bookkeeping, collection capacity, and typed model overhead are not
raw payload and are explicitly outside those byte sums. Implementations must
still use fallible reservation and bounded multiplicities; this document does
not present the raw-payload calculation as an exact resident-memory guarantee.

## Aggregate logical-population identity V1

Source-set domain compatibility and exact population membership are distinct:

```text
source-set domain compatibility
    !=
exact logical-case source-set identity
```

The compatibility domain remains the tuple of inventory scope,
aggregate/granularity contract, named lane, and environment-assessment mode.
Within a compatible domain, unequal exact source-set digests are valid and
later trend work reports added and removed logical cases.

AG9a defines one member digest per logical case and one digest for the exact
logical-case population. It does not add an execution-variant population
digest; detail reports retain each typed variant key directly.

### Common canonical identity framing

Both aggregate identity preimages start with their versioned ASCII domain
separator terminated by exactly one NUL. Fields follow in explicitly specified
ascending tag order:

```text
tag     = unsigned 16-bit big-endian
length  = unsigned 64-bit big-endian payload byte length
payload = exactly `length` bytes
```

All conversions and byte-size arithmetic are checked and all preimage storage
is fallibly reserved. Strings are their canonical typed UTF-8 bytes; closed
labels are their contract-defined lowercase ASCII bytes. The implementation
does not use `Debug`, Serde encoding, delimiters, host formatting, map order,
filesystem order, paths, timestamps, locale, or implicit Git state.

### `borrowser-conformance-logical-case-member-v1`

The domain separator is:

```text
borrowser-conformance-logical-case-member-v1\0
```

| Tag | Payload | Presence |
| ---: | --- | --- |
| 1 | `InventoryScope::as_str()` | always |
| 2 | `TestId::as_str()` | always |
| 3 | `ObservationSurface::as_str()` | always |
| 4 | `SourceKind::as_str()` | always |
| 5 | `SourceRecordId::as_str()` | external-derived only |
| 6 | `ExternalLineageId::as_str()` | external-derived only |
| 7 | external adapter `HarnessFeatureId::as_str()` | external-derived only |
| 8 | `ExternalAdapterVersion::as_str()` | external-derived only |

Native and controlled-static-page members contain exactly tags 1 through 4.
External-derived members contain exactly tags 1 through 8. Tags 5 through 8
are absent, not empty, in the first two branches. The tag-4 source-kind
discriminant makes the branch grammar unambiguous. `SourceRecordId` is accepted
only from an AG8 lineage registry that was fully validated and reconciled
against the exact `ValidatedInventory`; `conformance-runner` neither parses AG8
schema nor performs an unchecked registry lookup.

The Rust `AggregateLogicalSourceIdentity` is opaque. Its native and controlled
branches are constructed only by the aggregate identity owner from the
corresponding `ValidatedFixture`. Its external branch additionally requires a
declaration obtained from `ReconciledExternalFixtureLineages` for that exact
fixture; the identity owner rechecks the fixture ID, lineage, adapter, and
adapter version before retaining the declaration's `SourceRecordId`. Public
access is read-only through source-kind and optional external-field accessors.
There is no unchecked public branch constructor, and `AggregateRun::try_seal`
performs no repository I/O.

V1 currently has one typed inventory scope, `static-html-css-no-js`. Identity
tests prove tag 1 participates by mutating the canonical framed payload
directly; they do not add a fictitious production `InventoryScope` variant.

The frozen representative SHA-256 values are:

| Member | Digest |
| --- | --- |
| native `css-cascade-basic-author-rule` | `587fc9b32ef9bec4d021980da198836deab422f5e0ac506ac6de7eb1e955d270` |
| controlled static page `browser-controlled-static-page-basic` | `fc500a811a274719eccd9c519c8b72bd958c8ef7ab9c2dd70df6f920b0d68178` |
| external-derived `wpt-derived-body-background-display-none` | `0ea3d38ffb6b70a0e29d695fe1e2ec4a858e875b6557100a548de75a9844066a` |

### `borrowser-conformance-logical-case-source-set-v1`

The domain separator is:

```text
borrowser-conformance-logical-case-source-set-v1\0
```

It has exactly two fields:

| Tag | Payload | Reason |
| ---: | --- | --- |
| 1 | `InventoryScope::as_str()` | binds even an empty set to the authoritative inventory scope |
| 2 | ordered member-digest sequence | identifies exact logical membership without copying report or run-policy fields |

Tag 2 is exactly:

```text
member_count = unsigned 64-bit big-endian

repeated member_count times:
    item_length = unsigned 64-bit big-endian, exactly 32
    item        = raw 32-byte logical-case-member-v1 SHA-256
```

Members are sorted by unsigned-byte lexicographic comparison of canonical
`TestId` bytes. Duplicate `TestId` or member-digest values are errors and are
never deduplicated. Membership changes alter the digest. Lane,
environment-assessment mode, and aggregate/granularity contract remain domain
compatibility fields rather than redundant member fields.

For the empty `static-html-css-no-js` population the preimage is exactly 98
bytes:

```text
626f72726f777365722d636f6e666f726d616e63652d6c6f676963616c2d636173652d736f757263652d7365742d763100000100000000000000157374617469632d68746d6c2d6373732d6e6f2d6a73000200000000000000080000000000000000
```

Its frozen SHA-256 is:

```text
768d27de40c959c7cebd099c1104e668b06a36da11cf367767d990760adb5270
```

Reports render either digest as `sha256:` followed by exactly 64 lowercase
hexadecimal characters. Aggregate detail contains the source branch and all
external identity fields needed to interpret a historical member without
consulting the current repository or current AG8 registry.

## Aggregate report V1 grammar

Both reports use UTF-8, LF only, and exactly one final LF. They contain no
empty lines: the first record follows the header immediately and every later
record follows the preceding field or record immediately. A scalar string is
`key = "value"`; absence is `key = null`; an unsigned count is unquoted
canonical decimal with no leading zero except `0`; a Boolean is `true` or
`false`; and a string list is `[` plus comma-space-separated quoted strings
plus `]`. Strings preserve Unicode scalar values and escape backslash, quote,
LF, CR, and tab as `\\`, `\"`, `\n`, `\r`, and `\t`. Other U+0000 through
U+001F controls use uppercase, shortest-form `\u{HEX}`. No other escaping or
normalization occurs.

The common header order is:

1. `format`;
2. `inventory-scope`;
3. `aggregate-granularity-contract`, exactly
   `borrowser-conformance-aggregate-granularity-v1`;
4. `named-lane`;
5. `environment-assessment`, projected from the sealed
   `AggregateRun::environment_assessment_mode`; AG9a's only constructible mode
   is `AggregateEnvironmentAssessmentMode::EmptyV1`, serialized exactly as
   `ag9-empty-assessment-v1`;
6. `population-identity-contract`, exactly
   `borrowser-conformance-logical-case-membership-v1`;
7. `logical-case-source-set-digest`;
8. `headline-counts-overlap = true`;
9. `logical-case-population = "logical-case"`;
10. `execution-variant-population = "execution-variant"`;
11. fixed declarations `accounting-count-field-count = 59`,
    `subsystem-row-count = 5`, `observation-row-count = 10`,
    `comparison-row-count = 5`, and `terminal-row-count = 7`.

`accounting-count-field-count` counts the run-dependent unsigned count fields:
8 logical headline counts, 9 execution-variant population counts, 10 owner
counts, 20 observation counts, 5 comparison counts, and 7 terminal counts. It
does not count records or the five fixed row-declaration values.

The accounting projection then contains, in order:

- one `BEGIN logical-accounting` record with `total`, `pass`, `fail`,
  `expected-fail`, `unsupported`, `skipped`, `flaky`, and `unclassified`;
- one `BEGIN execution-variant-accounting` record with `materialized`,
  `runnable`, `not-runnable`, `eligibility-not-established`, `selected`,
  `excluded`, `selection-not-applicable`, `attempted`, and `not-attempted`;
- five `BEGIN subsystem` records in HTML/parser, CSS, Layout, Paint, and
  Browser/runtime order. Each contains `owner`,
  `logical-domain = "logical-case"`,
  `variant-domain = "execution-variant"`, `logical-cases`, and
  `execution-variants`;
- ten equivalent `BEGIN observation` records in the AG2 observation-vocabulary
  order frozen above, using `surface` instead of `owner`;
- five `BEGIN comparison` records for authored expected observation, semantic
  reference match, semantic reference mismatch, structural reference match,
  and structural reference mismatch. Each mirrors the typed comparison shape:
  `comparison-kind = "authored-expected-observation"` with both reference
  fields `null`, or `comparison-kind = "static-document-reference"` with
  independent `reference-kind` and `reference-relation` fields. Each ends with
  `execution-variants`;
- seven `BEGIN terminal` records in semantic pass, semantic fail, execution
  failure, resource failure, incomplete observation, invariant failure, and
  timeout order, each with `attempted-variants`.

All owner, observation, comparison, and terminal records are emitted even when
their count is zero. Sparse accounting maps do not define wire vocabulary.
Timeout remains reserved and zero for all current adapters.

`borrowser-conformance-aggregate-summary-v1` ends after that common accounting
projection. Its exact syntactic ceiling is 6,073 bytes. The derivation uses the
longest named-lane label and 20 digits for each of the 59 run-dependent `u64`
counts:

| Fixed grammar portion | Maximum bytes |
| --- | ---: |
| header and fixed population/count declarations | 708 |
| logical accounting | 301 |
| execution-variant accounting | 405 |
| five subsystem records | 985 |
| ten observation records | 2,098 |
| five structured comparison records | 890 |
| seven terminal records | 686 |
| **total** | **6,073** |

This is a syntactic envelope; it does not claim one semantically valid
`AggregateRun` can make every overlapping and reconciling count `u64::MAX`.

`borrowser-conformance-aggregate-detail-v1` writes the identical common
accounting projection with only the `format` value changed, then
`logical-case-detail-count`, followed by logical cases sorted by canonical
`TestId` bytes. Each `BEGIN logical-case` record contains, in this exact order:

1. `test-id`, `logical-case-member-digest`, `source-kind`, and the four
   `external-*` source fields; the latter are strings for external-derived
   cases and `null` otherwise;
2. `subsystem-owner` and `observation-surface`;
3. the classification branch described below;
4. eligibility state, blocker/unresolved counts, and typed eligibility facts;
5. expectation, expected-failure kind, and expectation reason;
6. `execution-variant-count` and every variant record.

For `classification = "not-yet-classified"`, `classification-reason` is a
string and `requirements`, `capability`, `capability-missing-count`, `harness`,
`harness-limitation-count`, `environment-requirement-count`, `stability`,
`stability-reason`, and `lane-exclusion-count` are all `null`. This means the
classified dimensions are absent. It does not create capability, harness, or
stability pseudo-states and does not serialize absent collections as empty.

For `classification = "classified"`, `classification-reason` is `null`;
`requirements` is an explicitly present sorted list, including `[]` when
empty; capability and harness use their typed closed labels and explicit
missing/limitation counts; environment requirements and lane exclusions use
explicit counts, including zero; and stability uses its typed label plus an
optional reason. Complex entries are bounded records sorted by their explicit
typed field keys, not declaration order or derived `Ord`.

The sealed-run boundary enforces both complete AG3 branches:
`not-yet-classified` requires all classified dimensions to be absent and
`AgExpectation::NotEstablished`; `classified` requires capability, harness,
and stability dimensions to be present and an established expected-pass or
expected-fail expectation. Report construction rechecks this invariant as
defense in depth.

Eligibility is independently `runnable`, `not-runnable`, or
`not-yet-established`. Its blocker and unresolved collections remain separate.
Each `BEGIN eligibility-fact` has a `role`, an explicit fact-kind label, and
only that typed branch's fields. Expectation is independently `expected-pass`,
`expected-fail`, or the existing typed `not-established`.

Variant records are sorted by an explicit V1 key: singleton first, then
rendering by environment label bytes and available width. Each
`BEGIN execution-variant` contains variant kind and parameters, comparison
kind and reference fields, actual named-run lane selection plus
`selection-lane` and optional reason, attempt state plus optional
not-attempted reason, optional terminal outcome, and derived policy. A logical
Browser/runtime case with zero variants remains present with
`execution-variant-count = 0`. Complete subsystem observation/report payloads
are not embedded.

The detail report has a fixed 32 MiB complete-report bound. Both builders use
checked count conversion, checked byte arithmetic, fallible reservation, and
no truncation. They construct and validate the complete byte vector before
calling a caller-provided `Write`. Build/allocation/validation failure therefore
publishes zero bytes. Once `Write::write_all` begins, an arbitrary sink may
accept a prefix before returning an I/O error; AG9a provides no rollback or
filesystem-atomic publication guarantee.

All AG parser V1, CSS V1, rendering V1/V2, and aggregate V1 text formats use
one crate-private canonical low-level writer for bounded growth, decimal
encoding, quoting, escaping, nulls, and lists. Each report module independently
maps writer failures into its own public error vocabulary; sharing byte grammar
does not transfer parser-report semantics into aggregate reporting.

## Deterministic reports and publication boundaries

The version labels are frozen as:

- `borrowser-conformance-aggregate-summary-v1`;
- `borrowser-conformance-aggregate-detail-v1`;
- `borrowser-conformance-trend-v1`;
- `borrowser-cross-engine-comparison-registry-v1`; and
- `borrowser-external-capture-provenance-v1`.

The comparable DOM and capture-ID versions remain
`web-observable-dom-tree-v1` and `borrowser-external-capture-id-v1`.

AG9a aggregate accounting consumes typed normalized runner results through the
Stage 1 projection. It does not serialize parser, CSS, or rendering
reports and parse those bytes back into an aggregate model. Existing detailed
subsystem reports remain stable and authoritative.

The CI-safe summary and local detailed report derive from the same
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

## Staged decision

Stage 0 froze terminology, limits, and future ownership. AG9b implements the
frozen registry wire schema, phase-gated deterministic validation, full
external-artifact validation, same-object confined reads, exact verified-byte
ownership, explicit resource/cumulative artifact limits, capture-ID
construction authority, and structural advisory separation. It changes no
production browser runtime functionality.

Stage 1 implements the typed `AggregateRun`, subsystem projections,
reconciliation, and accounting. AG9a implements the deterministic aggregate
summary/detail formats and exact logical-population identity above. AG9b
implements the frozen source-neutral capture and runner-owned registry
contracts, but not external DOM comparison or capture tooling. Later AG9 stages
remain responsible for comparisons, trend
parsing/execution/comparison, and aggregate CLI/CI publication. Neither AG9a
nor AG9b may be reported as working cross-engine comparison, trend support, or
completed AG9 infrastructure.
