# AF5: Stylesheet Rule Collection And Source Order Contract

Status: implemented

Last updated: 2026-08-20

AF5 introduces the immutable CSS-owned boundary between available stylesheet
attachments and per-element cascade matching. It closes issue #1082 without
redesigning final winner selection.

## Ownership and lifecycle

Browser owns HTML stylesheet discovery and attachment lifecycle,
`StylesheetSlotId`, resource availability, document-order slots, loading,
decoding, and recomputation orchestration. Pending, failed, and aborted slots
remain Browser state and are not CSS collection records.

CSS owns `StylesheetSourceId`, source-order coordinates, origin, namespace
matching constraints, stylesheet condition classification, rule collection,
selector matchability, declaration classification and expansion, matching,
and the matched-declaration handoff to the existing cascade pipeline.
`CssParseOrigin` is parser-entry metadata and is never converted into
`CascadeOrigin`.

The authoritative lifetime is:

```text
available ordered StylesheetCollectionInput values
  -> fallibly build one immutable RuleCollection
  -> build one selector DOM
  -> match all recomputed elements against that collection
  -> discard the collection after the style execution
```

Incremental execution and its full fallback borrow the same collection. AF5
does not retain or cache collections across passes and does not convert
stylesheet storage to `Arc`.

## Identity and order

These are distinct types and domains:

- Browser `StylesheetSlotId`: attachment identity owned by Browser;
- CSS `StylesheetSourceId`: opaque provenance identity, represented as `u64`;
- `StylesheetOrder`: sparse document/style-input order, represented as checked
  `u32`;
- `RawRuleIndex`: top-level parser-model position including at-rules;
- `StyleRulePosition`: style-rule position excluding top-level at-rules;
- `DeclarationSourceIndex`: parser-model declaration position;
- `DeclarationOrder`: declaration cascade order after collection;
- `StylesheetRuleOrder`: the semantic pair `(StylesheetOrder,
  StyleRulePosition)`.

`StylesheetSourceId` never participates in precedence. Its private encoding
keeps the built-in UA source, Browser attachment slots, compatibility inputs,
and explicit in-memory inputs collision-free. URL, stylesheet text, content
equality, parse-object address, and pointer identity are not source identity.
Duplicate-content and duplicate-URL attachments remain distinct.

Browser slot IDs are stable across current source-key reconciliation within a
Browser stylesheet-set generation, including media-only metadata changes.
Changing inline text or external URL may create a new current slot; AF5 does
not claim DOM source-node or full attachment-lifetime identity. Compatibility
and in-memory IDs are deterministic for one stylesheet-input generation.
Resolved winners retain self-contained source IDs and coordinates; therefore
each supplied ID must remain meaningful for the retained resolved artifact's
lifetime. AF5 does not claim stable DOM source-node identity beyond the current
Browser stylesheet generation; AJ owns that broader attachment model.

Sparse order is preserved. If slots 0, 2, and 3 are available while slot 1 is
pending, CSS receives orders 0, 2, and 3. A later pass may receive 0, 1, 2,
and 3. Network completion order and compact active-vector position are never
cascade order.

## Checked representation and failures

Pass-local coordinates are compact `u32` newtypes. Every conversion from an
observed length/index is checked and every counter increment uses checked
arithmetic. The authoritative path does not use saturation, sentinel reuse,
wrapping, or unchecked narrowing. Duplicate source IDs, duplicate order,
non-monotonic order, collection-arena coordinates, configured/observed
collection limits, and explicit collection-container reservation failures are
typed `RuleCollectionBuildError` outcomes. That type is exclusive to
`RuleCollection::try_new` and helpers constructing its stylesheet, rule, and
declaration arenas. Browser and compatibility input-list construction share the
separate `StylesheetCollectionInputBuildError` boundary; all of their
coordinate, source-identity, and reservation failures become
`StyleResolutionError::StylesheetInputBuild` and are never mislabeled as
collection storage failures.

`StyleResolutionLimits` independently bounds stylesheets, all top-level rules
(including at-rules), collected declaration inputs after shorthand expansion,
matched rules per element, matched declaration inputs per element, inline
style input, styled elements, and selector matching. Collection-owned vectors
use fallible reservation. This is not a promise to recover from process-level
allocator exhaustion or from every nested standard-library allocation.
Per-element matched-declaration counting is an element-level
`DeclarationInputsPerElement` limit. Inline declaration source-coordinate
preparation is a `StyleResolutionError::SourceCoordinate` execution failure.
Neither occurs while constructing `RuleCollection`, so neither may masquerade
as `RuleCollectionBuildError`. The opt-in allocation guard likewise has its own
measurement-only counter error.

Invalid and unsupported CSS are normal non-candidate states, not build
failures. Collection failures propagate through full, incremental, computed,
Browser recomputation, and debug paths. They are not converted into no-match,
empty/default style, truncation, `IncrementalUnavailable`, or silent full
fallback.

## Collection shape

`RuleCollection` owns flat private vectors for stylesheet records, collected
rules, and declaration inputs. Checked ranges connect stylesheet records to
rules and active rules to one declaration arena. The collection and its arena
are opaque outside CSS integration; public inspection flows through the AF5
diagnostic projection. Private `Vec` storage is immutable through visibility.
Semantic source/order types live in the lower cascade contract, so contract
modules do not depend on collection integration.

`CollectedRule` prevents contradictory states:

- `ActiveStyle` alone carries a parsed supported selector list,
  `StylesheetRuleOrder`, and a declaration range;
- `InactiveStyle` carries condition-deferred, invalid-selector, or
  unsupported-selector reason and no classified declarations;
- `SkippedAtRule` carries raw provenance and a typed skip reason.

Invalid and unsupported selector lists retain raw rule and style-rule
positions but contribute no declaration classification or candidates. Rules
inside a condition-inactive stylesheet likewise classify no declarations.
Parser-recovered malformed declarations that never entered `css::model` are
not fabricated as collection entries; valid neighboring model declarations
remain collectable.

Active stylesheet declarations are classified once while the collection is
built. That includes property-name state, unsupported/custom properties,
specified-value parsing, invalid-value state, shorthand expansion and atomic
rejection, `!important`, declaration order, and expansion order. Matched
stylesheet inputs contain only contract data: a self-contained rule reference,
validated rule context, exact match outcome, and borrowed declaration slice.
They neither expose the active collection rule nor own a declaration vector.
Inline style attributes remain element-local and are
parsed/classified at most once for that element's style calculation.

## Matching and precedence

Every active selector list is matched once per target element. The exact AF4
`SelectorListMatchOutcome` is retained by the matched stylesheet input. Its
matched selector indexes and AF3 highest actual-match specificity feed the
matched declaration input, AF5 diagnostics, and existing candidate pipeline.
There is no diagnostic rematch and no specificity reconstruction from text.

`CascadePriority` compares semantic dimensions explicitly:

1. origin/importance band;
2. selector or inline specificity;
3. semantic `CascadeSourceOrder`;
4. declaration order.

`CascadeRuleContext` makes selector/stylesheet-order and
inline-specificity/inline-order pairings unrepresentable. Cross-kind source
order is a lawful total order: stylesheet orders compare semantically with one
another and sort before inline style; equal comparison therefore implies
structural equality. Enum declaration order and stable sorting are not
precedence. Inline normal beats author normal
through `CascadeSpecificity::InlineStyle`; author important beats inline normal
through the origin/importance band; inline important beats author important
through inline specificity after equal important bands. Inline declaration
ties use declaration order. `CascadeSourceOrder::InlineStyle` is provenance,
not the reason inline wins.

## Conditions and at-rules

Browser transports the exact optional raw `media` attribute but does not parse
it. CSS's current fail-closed subset is deliberately narrow:

- missing, empty, or ASCII-whitespace-only input is active;
- every other value is typed deferred/unsupported and inactive.

AF5 does not recognize `all`, `screen`, `print`, negation, lists, or media
features. AQ3/AQ4 own real media parsing and evaluation. Internal `@media`,
`@supports`, `@import`, and unknown at-rules are separate typed skipped records;
preserved blocks are never flattened or recursively parsed by AF5.

## Diagnostics and compatibility

`rule_collection_diagnostic` is a typed, deterministic, versioned, bounded
pre-winner surface. It reports source ID/order/origin/namespace/condition,
raw/style rule coordinates, active/inactive/skipped state, declarations,
importance/applicability/errors, and the exact AF4 match outcome/effective
specificity. Declaration records include typed supported, invalid-value,
invalid-shorthand, unsupported, custom, and invalid-name property projections;
bounded specified/preserved value text; source/declaration/expansion order;
importance; applicability; and stable invalid reasons. Record, retained
storage-byte, condition-text, at-rule-name, declaration-property,
declaration-value, and serialized-byte limits are separate from
style-resolution limits. Retained vector capacities and every retained
diagnostic string/vector are included in storage accounting. Stable error and
record serialization uses explicit labels, never derived Rust `Debug` grammar.
Every serialized `BoundedDiagnosticText` appends
`[original-bytes=<count>]` when truncated, including media text, at-rule names,
unsupported/custom property names, and declaration values. Untruncated text
keeps the compact quoted form, so shortened text can never be mistaken for a
complete source name or value.

R8 winner/resolved snapshots remain downstream specialized surfaces. AF4's
selector-conformance diagnostic also remains separate. It may evaluate a
selector in a condition-inactive sheet, but serializes CSS source ID, sparse
order, condition status, selector result, and `cascade-state=inactive-condition`
so the match cannot be mistaken for cascade participation. It does not rematch
inside the AF5 production path. Neither selector nor winner diagnostics
substitute for AF5 collection evidence.

Allocation guards measured the accepted AF5 boundary and the corrected review
pass at 28,095,354 allocated bytes for the 1,025-entry representative style
fixture. Focused measurements record 45,334 bytes/387 allocations/5
reallocations to build a 64-rule/128-declaration arena and identical matched
input allocation (84,368 bytes/387 allocations) for one versus 64 borrowed
declarations per matched rule across 128 elements. Candidate/winner allocation
remains the existing AF6/R concern.

`try_attach_styles` is the typed fallible legacy projection. `attach_styles`
remains the documented non-authoritative degrading wrapper and clears stale
legacy projections on failure. Browser production styling uses neither
degrading behavior nor the legacy projection.

## Deliberate exclusions

AF5 does not add selector indexes, compiled selectors, bloom filters, fast
rejection, style sharing, cross-pass collection caching, networking/loading,
`@import` loading, media parsing/evaluation, AJ stylesheet-set algorithms,
cascade layers, runtime user stylesheets, animations, transitions, CSSOM, new
selector/property coverage, or an AF6 winner/computed-style redesign.
