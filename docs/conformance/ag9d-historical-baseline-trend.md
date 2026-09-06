# AG9d historical baseline and trend contract

AG9d implements `borrowser-conformance-baseline-v1` and
`borrowser-conformance-trend-v1`. A baseline passively projects one validated
aggregate run and separately reconciled advisory state. A trend compares
exactly two explicit baseline files. Neither operation reruns Borrowser,
reloads a registry, discovers a browser or previous run, reads Git history,
uses a clock or database, or accesses a network.

The existing `borrowser-conformance-aggregate-detail-v1` writer, public API,
32 MiB ceiling, and bytes remain unchanged. Baseline section 2 contains those
standalone bytes verbatim, including their final LF.

## Binary primitives

All multibyte integers are unsigned and big-endian. `U8`, `U16`, `U32`, and
`U64` have widths 1, 2, 4, and 8 bytes. `B(x)` is `U64(byte_length(x)) || x`.
`S(x)` is `B(UTF-8(x))`. `O(x)` is exactly `U8(0)` when absent and
`U8(1) || B(x)` when present. Native layout, enum discriminants, Serde,
`Debug`, alignment, and padding are never wire semantics.

Every section is `U16(tag) || B(body)`. Every counted record is `B(body)`.
Counts are `U32`. Nested readers must consume their complete frame. The
envelope ends after section 7, with no trailing bytes, padding, or envelope LF.
Embedded textual artifacts retain their required final LF.

A variant key is:

```text
S(TestId) || S(observation-surface) || S("singleton")

or

S(TestId) || S(observation-surface) || S("rendering") ||
S("synthetic-text-metrics-v1") || U32(available-width-css-px)
```

Ordered capture invocation arguments retain order. Set-like declarations and
trend records use ascending canonical typed-key order. Declaration and
filesystem order are non-semantic. Duplicate or out-of-order keys fail closed.

## Baseline V1 grammar

The envelope prefix is the ASCII bytes
`borrowser-conformance-baseline-v1`, then `U8(0)` and `U16(7)`. Its sections are:

1. **Versions:** exactly 13 `S(version)` values in `BASELINE_VERSIONS_V1`
   order: aggregate detail, aggregate granularity, logical membership,
   advisory membership, baseline note, advisory result, four fingerprint
   contracts, external capture provenance, external capture identity, and DOM
   first-difference evidence.
2. **Aggregate detail:** raw standalone
   `borrowser-conformance-aggregate-detail-v1` bytes. The section frame is its
   sole envelope length frame.
3. **Capture declarations:** `U32(count)`, then records ordered by capture
   digest. A record is `raw-32-byte-capture-SHA-256 || B(canonical-capture-ID-
   preimage)`. The preimage is the authoritative 26-field
   `borrowser-external-capture-id-v1` preimage.
4. **Advisory tracks:** `U32(count)`, then records ordered by track ID. A record
   has ten strings: track ID, engine product, platform OS family, architecture,
   comparable surface, capture algorithm, capture algorithm version, target
   parser input context, collection policy, and collection policy version.
5. **Advisory comparison points:** `U32(count)`, then records ordered by
   `(variant key, comparable surface, track ID)`. A record is `variant-key ||
   S(comparable) || S(track-id) || raw-32-byte-capture-ID`.
6. **Advisory evaluation:** one of
   `S("none") || S("not-requested")`,
   `S("selected-variant-only") || variant-key || S(comparable) ||
   S("completed")`, or `S("all-declared") || S("completed")`. It is followed
   by `L(O(result))`: `U32(point-count)` and exactly one independently framed
   `B(slot)` per section-5 point in canonical order. An absent slot body is
   `U8(0)`; a present slot body is `U8(1) || B(result)`. `none` requires every
   slot absent; selected scope requires
   results exactly for matching points; all-declared requires every slot
   present. Zero matching points does not promote selected scope.
7. **Baseline notes:** `U32(count)`, then records ordered by note ID. A record
   is `S(note-id) || variant-key || S(comparable) || S(text) ||
   O(raw-32-byte-capture-ID)`.

Zero is the canonical empty capture, track, point, or note count. It may only
come from complete, successfully reconciled membership. Unknown membership is
not empty. Supporting captures and tracks may be unreferenced; they affect the
baseline digest but do not form another trend population.

Unknown versions, section counts/order/tags, labels, option discriminants, or
fields are rejected. Malformed or overflowing lengths, premature EOF,
unconsumed nested bytes, invalid references, duplicate identities, invalid
accounting, noncanonical ordering, and trailing bytes are rejected.

## Stable advisory results and failures

An evaluated result is `S("equivalent")`, `S("different") || B(exact
borrowser-advisory-dom-first-difference-v1 bytes)`, or `S("failure") || failure`.

A failure is `S("observation") || observation-failure`, or one of
`unsupported-capture-context`, `source-identity-mismatch`,
`algorithm-source-mismatch`, `configuration-source-mismatch`,
`fixture-mismatch`, `incompatible-surface`, `invalid-artifact`, `invariant`,
`resource`, and `allocation` as one `S(label)`.

An observation failure is `unknown-variant`, `unsupported-selection`,
`not-attempted`, or `duplicate-handoff`, or `S("preparation") ||
preparation-failure`. Preparation failures are `unavailable`,
`execution-failure`, `resource`, `incomplete`, `invariant`,
`unsupported-context`, or `S("serialization") || serialization-failure`.
Serialization failures are `invalid-structure`, `invalid-attribute`,
`duplicate-attribute`, `too-large`, `overflow`, or `allocation`.

These are closed labels. Rust names, parser errors, paths, and diagnostic text
are not persisted. Different evidence reuses and validates the existing
first-difference artifact rather than defining a second authority.

An advisory operation that fails before a completed operation exists cannot be
sealed. Its operation-wide failure is not assigned to points or changed into
`none/not-requested`.

## Historical identity and sealing

`external-test-provenance` decodes a canonical historical capture preimage and
recomputes its capture ID with the live algorithm. The resulting
`HistoricalCaptureIdentityV1` proves consistency of recorded provenance,
artifact length/digest, and capture-ID claim. It does not claim the artifact
body was read or reverified and cannot construct live artifact/capture
authority.

Baseline sealing accepts reconciled evidence carrying a private reference to
its originating `AggregateRun`. Runtime reference identity proves association
even for empty evidence; equal-valued distinct runs fail. The reference never
enters bytes, fingerprints, diagnostics, or historical identity. A completed
selected operation already owns evidence bound to its run.

Baseline and trend grammar, sealing, historical aggregate validation,
fingerprints, and comparison remain under `conformance-runner::aggregate` and
are available only with the aggregate feature. `conformance-runner` depends on
the source-neutral `external-test-provenance` capture-ID and same-object file
primitives. The provenance crate has no dependency on the runner, aggregate
model, HTML/CSS/layout/paint crates, or trend semantics. No production-engine
crate owns or depends on the historical protocols.

Live and historical aggregate validation feed the same typed case/variant
accounting accumulator and the same reconciliation predicates. Live AG9a output
retains its existing map-backed public accounting type. Historical decoding uses
fixed arrays for the closed V1 owner, observation, comparison, and terminal
dimensions, so untrusted persisted accounting cannot allocate map nodes.

## Populations, fingerprints, and compatibility

The populations are logical cases, execution variants, advisory comparison
points, and baseline notes. Added/removed follow key membership. Surviving keys
are unchanged only when versioned fingerprints match. Each population checks:

```text
old = removed + unchanged + changed
new = added + unchanged + changed
```

Logical identity is `TestId`; its fingerprint preimage is
`domain || B(canonical-logical-metadata-prefix) || U32(variant-count) ||
B(VariantKey || H(variant-fingerprint))...` in canonical typed variant-key
order. Each complete key/hash pair is one list-item frame. Thus the same
`TestId` with changed member/source state is changed.

Variant identity is its typed variant key; its fingerprint contains parent
metadata, the exact key, and complete canonical variant record. Surviving
variants inherit parent drift. A changed key is removed plus added.

Advisory point identity is `(typed attachment, stable track ID)`. Its
fingerprint contains the point record, referenced capture preimage, and optional
result. Unevaluated-to-evaluated and the reverse are changes. Capture,
provenance, browser version, verdict, or failure drift changes only this
population. A reused track ID with changed invariant identity is incompatible.
Two unchanged unevaluated points mean unchanged recorded evidence state, not
proof of unchanged browser behavior.

Note identity is stable note ID and its fingerprint is the canonical note
record. Note changes affect only notes. Advisory state and notes never enter
Borrowser logical or variant fingerprints.

The compatibility tuple is exactly inventory scope, aggregate/granularity
contract, named lane, and execution-environment assessment mode. Source-set
membership/digest and advisory evaluation scope may differ. Every V1 envelope,
component, result, and fingerprint version must be the exact supported value;
cross-version fingerprints are never compared.

## Trend V1 grammar

The prefix is ASCII `borrowser-conformance-trend-v1`, then `U8(0)` and
`U16(7)`. Sections are:

1. `S("borrowser-conformance-baseline-v1")`, followed by the complete thirteen
   baseline V1 version declarations in their frozen `BASELINE_VERSIONS_V1`
   order. No subset or inferred component version is permitted.
2. `S(inventory-scope) || S(aggregate-contract) || S(named-lane) ||
   S(environment-assessment)`, followed in order by raw old baseline, new
   baseline, old source-set, and new source-set SHA-256 values.
3. `B(old-operation-descriptor) || B(new-operation-descriptor)`, using the
   section-6 scope prefix grammar without slots, followed by six `U64` values:
   old advisory total, old evaluated, old unevaluated, new advisory total, new
   evaluated, and new unevaluated. The totals and evaluated partitions must
   reconcile with the advisory population and each baseline's evaluation
   slots.
4. Logical-case population.
5. Execution-variant population.
6. Advisory-comparison-point population.
7. Baseline-note population.

The section tag fixes the population; there is no population-name field in the
payload. Each population is six `U64` counts in old, new, added, removed,
unchanged, changed order, then `U32(record-count)` and ascending records. A
common record is:

```text
S(change-kind) || B(canonical-key) || O(H(old-fingerprint)) ||
O(H(new-fingerprint))
```

For advisory comparison points, the canonical key payload is exactly
`VariantKey || S(comparable) || S(track-id)`. The common record's one outer
`B(canonical-key)` frame encloses that complete payload; `VariantKey` has no
additional inner `B(...)` frame.

Kinds are `added`, `removed`, `unchanged`, and `changed`. Fingerprints are raw
32-byte SHA-256 values. Only section 6 advisory records append
`O(U8(old-evaluated)) || O(U8(new-evaluated)) || U8(change-mask)`. The option
values are exactly zero or one. Added/removed records have one applicable
fingerprint and an absent evaluation value for the missing side. Unchanged
records have equal fingerprints and zero mask; changed records have unequal
fingerprints and a nonzero mask. Advisory mask bit 0 (`0x01`) is
capture/provenance drift, bit 1 (`0x02`) is evaluation-presence drift, and bit
2 (`0x04`) is differing canonical result bytes when both sides are evaluated.
No other bit is valid. Presence transitions never set result drift. Logical,
variant, and note records contain no mask or evaluation extension. No percentage
or flattened status exists.

All four record lists are sorted by typed semantic keys before encoding:
logical `TestId` UTF-8 bytes; variant `(TestId, observation, typed variant)`;
advisory `(variant key, comparable surface, track ID)`; and note-ID UTF-8
bytes. The decoder validates those semantic orders. Length-framed encoded key
bytes are not an ordering authority.

## Bounds and file API

The exact capture preimage ceiling is **33,920 bytes**. Baseline fixed framing
and versions consume at most 785 bytes. Its four 256-record advisory/note
collections plus operation descriptor and independently framed result slots
consume at most 13,664,066 bytes, from record maxima 33,968 (capture), 1,181
(track), 379 (point), 302 (operation descriptor), 16,426 (slot), and 1,420
(note). With one 33,554,432-byte detail artifact the exact ceiling is:

```text
33,554,432 + 785 + 13,664,066 = 47,219,283
```

Trend fixed framing, versions, context, evaluation summary, and population
headers consume 1,967 bytes. The Stage 0 structural proof establishes that each
logical-case or execution-variant contribution consumes at least 306 disjoint
bytes of the 33,554,432-byte aggregate-detail artifact. Each input therefore
contains at most `floor(33,554,432 / 306) = 109,655` combined logical-case and
variant records, and the two-input trend union contains at most 219,310 records.
The maximum common logical/variant trend record is 337 bytes:

```text
B(record)                         8
S("unchanged")                   17
B(max rendering VariantKey)     230
O(old fingerprint), present      41
O(new fingerprint), present      41
                                 ---
                                 337
```

The logical and variant populations consequently consume at most
`219,310 * 337 = 73,907,470` bytes. Disjoint old/new membership maximizes the
bounded external populations. Advisory removed/added records are 424/422 bytes
and note removed/added records are 209/207 bytes, giving:

```text
advisory: 256 * (424 + 422) = 216,576
notes:    256 * (209 + 207) = 106,496
external populations total = 323,072

73,907,470 + 323,072 + 1,967 = 74,232,509
```

The frozen accepted Trend V1 output ceiling remains **74,281,149 bytes**. The
corrected conservative syntactic proof is `<= 74,232,509`, leaving exactly
48,640 bytes of reviewed headroom. The accepted ceiling is deliberately not
described as a tight, jointly attainable serialized maximum. Compile-time
assertions fix every subterm and its relationship to the frozen ceiling.

Arithmetic/conversions are checked. Builders reserve fallibly and reject excess
without truncation. Canonical set ordering uses allocation-free in-place sorting
after fallible collection reservation; equal canonical identities are rejected,
so their relative order is non-semantic. Writers build complete bytes before
`write_all`.

The file API accepts exactly two explicit `(root, relative path, expected
SHA-256)` inputs. It opens each once with the source-neutral same-object confined
reader, sentinel-bounds reads, retains bytes, verifies both digests, and only
then decodes those same buffers. It never reopens paths. A platform lacking the
strong guarantee returns typed `UnsupportedPlatform`.

AG9d adds no automatic discovery, `latest`, persistent storage, dashboard,
CLI/CI publication, browser capture/automation, complete advisory evaluator,
selected-operation merger, percentage, broad WPT claim, or production-engine
behavior.

Independent protocol vectors live in
`tests/contract-vectors/conformance-baseline-v1/`,
`tests/contract-vectors/conformance-trend-v1/`, and
`tests/contract-vectors/external-capture-id-v1/`. Their READMEs record semantic
contents, offsets, and reviewed SHA-256 identities. Tests decode those committed
bytes independently and separately prove that representative producers match
the fixed vectors.
