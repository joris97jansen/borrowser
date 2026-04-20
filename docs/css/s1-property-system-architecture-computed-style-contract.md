# S1: Property System Architecture And Computed-Style Contract

Last updated: 2026-04-20  
Status: architecture contract implemented

This document is the source-of-truth contract for Milestone S issue 1: the
shared property-system architecture, the specified-versus-computed boundary,
the current normalization and invalid-value rules, and the `ComputedStyle`
contract Borrowser exposes to downstream runtime systems.

Related code:
- `crates/css/src/properties.rs`
- `crates/css/src/computed.rs`
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/architecture/ARCHITECTURE.md`

## Implemented Result

Milestone S now has an explicit in-repository contract for:

- one engine-owned property identifier universe shared by cascade and computed
  style
- per-property metadata for inheritance, initial/default values, specified
  value shape, computed value shape, and invalid-value policy
- a typed `ComputedStyle` contract with deterministic per-property iteration
- a deterministic `ComputedStyleBuilder` that enforces totality and value-kind
  correctness
- stable debug/test surfaces for the current computed-style contract
- explicit scope limits for layout-dependent and broader CSS value semantics

Follow-up computed-value implementation work must build on this contract. It
must not introduce a second property table, ad hoc string-keyed computed-style
maps, or layout-owned property parsing rules.

## Why This Exists

Milestone R ended at `ResolvedStyle`: a total, structured cascade result that
records whether each supported property came from a winning authored
declaration, inheritance, or the initial/default contract.

That solved winner selection and default fill, but it intentionally did not
solve the next layer:

- property-specific specified-value parsing
- computed-value normalization
- typed runtime storage
- an explicit computed-style handoff to layout and painting

Without an S1 contract, those responsibilities would tend to leak into:

- `computed.rs` bridge code
- layout modules
- painting helpers
- property-specific string parsing scattered across subsystems

That would repeat the same architectural drift Milestones O and R already
eliminated upstream.

## Shared Property System Boundary

The ownership boundary is now:

1. `css::model`
   - owns parsed stylesheet/rule/declaration/value structure
   - preserves authored declaration values as structured syntax-derived data
   - does not own cascade winner resolution or computed values
2. `css::cascade`
   - owns winner resolution, inheritance/default source selection, and
     `ResolvedStyle`
   - consumes the shared property table from `css::properties`
   - does not own typed computed-value normalization
3. `css::properties`
   - owns the supported property identifier universe and shared metadata
   - defines the expected specified-value and computed-value shapes
   - defines the current invalid-value policy for the supported subset
   - does not own selector matching, precedence, or runtime layout semantics
4. `css::computed`
   - owns typed computed-style assembly and the runtime-facing `ComputedStyle`
     contract
   - must consume `ResolvedStyle` as the normative cascade handoff once the
     bridge is retired
   - does not own layout, painting, or layout-dependent value resolution

## Property Identifier Model

The supported property subset is represented by one canonical enum:
`PropertyId`.

Normative rules:

- `PropertyId::ALL` is the canonical property order for debug output, tests,
  and total style assembly
- property names are canonical lowercase CSS names through `PropertyId::name()`
- conversion from model-layer declaration names into the supported property
  subset is explicit through `PropertyId::from_name(...)`
- `PropertyId` is the stable property identity and `PropertyId::metadata()` is
  the normative source for inheritance, initial/default, specified-value-kind,
  computed-value-kind, and invalid-value facts
- downstream code must not re-encode those facts in separate match tables
- the cascade contract keeps its established `CascadePropertyId` surface as an
  alias over the shared property table; it is not a second property universe

Current supported properties:

| property | inherits | initial/default | specified value kind | computed value kind |
| --- | --- | --- | --- | --- |
| `background-color` | no | `transparent` | `Color` | `AbsoluteColor` |
| `color` | yes | `black` | `Color` | `AbsoluteColor` |
| `display` | no | `inline` | `DisplayKeyword` | `DisplayKeyword` |
| `font-size` | yes | `16px` | `AbsoluteLength` | `AbsoluteLength` |
| `height` | no | `auto` | `AbsoluteLengthOrAuto` | `AbsoluteLengthOrAuto` |
| `margin-bottom` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `margin-left` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `margin-right` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `margin-top` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `max-width` | no | `none` | `AbsoluteLengthOrNone` | `AbsoluteLengthOrNone` |
| `min-width` | no | `auto` | `AbsoluteLengthOrAuto` | `AbsoluteLengthOrAuto` |
| `padding-bottom` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `padding-left` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `padding-right` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `padding-top` | no | `0px` | `AbsoluteLength` | `AbsoluteLength` |
| `width` | no | `auto` | `AbsoluteLengthOrAuto` | `AbsoluteLengthOrAuto` |

## Specified Versus Computed Boundary

Borrowser now has an explicit staged contract:

1. authored parsed value
   - represented today by `DeclarationValue` and `CascadeSpecifiedValue`
   - retains authored structure and token ordering
   - is not normalized into runtime value form
2. typed specified value
   - defined architecturally by `PropertySpecifiedValueKind`
   - this is the property-parser output shape the later S-issue
     implementation work must produce
   - remains property-typed, but not yet the final runtime-normalized value
3. typed computed value
   - represented by `ComputedValue` and stored in total `ComputedStyle`
   - is the layout/painting/runtime handoff surface for the supported subset

Normative rule: layout and painting must consume computed values, not authored
token lists and not `ResolvedStyle` provenance objects directly.

## Normalization And Canonicalization Contract

Normalization is defined per property through the computed value kind.

Current-scope guarantees:

- `AbsoluteColor` means computed colors are normalized to engine-owned RGBA
  tuples, not kept as authored keywords or hashes
- `AbsoluteLength` means computed lengths are normalized to the current CSS-px
  runtime contract
- `AbsoluteLengthOrAuto` and `AbsoluteLengthOrNone` preserve the controlling
  keyword while normalizing any length branch into CSS px
- `DisplayKeyword` remains a canonical keyword enum, not a raw string

Normative rule: `ComputedStyle` stores canonical runtime values, not authored
serialization. Stable debug output may serialize those values for tests, but
that serialization is secondary to the typed contract.

## Invalid Value Handling

The current invalid-value policy is explicit: `RejectDeclaration`.

That means:

- if a declaration is not in the supported property subset, it never becomes a
  comparable candidate in cascade
- if a declaration is in the supported subset but later cannot be parsed into
  the property's typed specified-value representation, the declaration is
  discarded for that property
- that rejection happens in the property pipeline layer, before layout,
  painting, and other runtime consumers observe the value
- layout, painting, and other runtime consumers must not attempt post-hoc
  recovery for supported properties
- after rejection, fallback behavior comes from the already-defined cascade
  contract: another winning declaration, inheritance, or the initial/default
  value

Milestone S does not introduce "best effort" partial parsing, downstream
layout fixes, or per-consumer recovery rules for invalid property values.

## ComputedStyle Contract

`ComputedStyle` is now an explicit, total runtime style object for the current
supported subset.

Responsibilities:

- store one typed computed value for every supported property
- expose deterministic per-property access through `ComputedStyle::get(...)`
- expose deterministic canonical iteration through `ComputedStyle::entries()`
- provide a stable debug snapshot through `ComputedStyle::to_debug_snapshot()`
- remain independent from layout tree shape, painting state, and DOM mutation

Structured field grouping inside `ComputedStyle`, such as `BoxMetrics`, is
allowed for runtime ergonomics only when it remains a lossless materialization
of one computed value per supported property.

`ComputedStyleBuilder` is the required assembly surface for new computed-style
pipeline work. It enforces:

- total property fill
- one value per property
- value-kind correctness for each property
- canonical assembly into the structured `ComputedStyle` fields

## Determinism And Invariants

The following invariants are part of the S1 contract:

- there is exactly one engine-owned supported property table
- property order is canonical through `PropertyId::ALL`
- `ComputedStyle` is total over the supported property subset
- `ComputedStyleBuilder` rejects duplicate property insertion
- `ComputedStyleBuilder` rejects computed-value kind mismatches
- grouped runtime fields remain lossless over the supported property table
- computed-style snapshots iterate properties in canonical order
- computed-style assembly remains independent from layout and painting logic
- bridge-phase HTML/UA display defaults remain outside the CSS initial-value
  contract and are applied later than `ComputedStyle::initial()`

If future work changes any of these invariants, code, tests, and this document
must all change together.

## Current Scope And Explicit Non-Goals

S1 defines the architecture for the current subset. It does not yet implement
the full computed-value engine.

Out of scope in this issue:

- full property-specific specified-value parsers from `ResolvedStyle`
- relative units, percentages, and any layout-dependent value resolution
- CSS-wide keywords such as `inherit`, `initial`, `unset`, `revert`, or
  `revert-layer`
- custom property substitution and `var(...)`
- shorthand expansion
- computed-value caching, invalidation, or performance-oriented storage
  redesign
- retiring the current legacy `compute_style(...)` bridge input path
- moving UA defaults like element display mapping into explicit UA-origin
  stylesheet rules

## Test And Debug Surface

S1 adds and relies on these contract surfaces:

- `properties::tests::property_metadata_matches_the_supported_property_contract`
- `computed::tests::computed_style_initial_snapshot_is_total_and_canonical`
- `computed::tests::computed_style_builder_materializes_structured_fields_from_property_entries`
- `computed::tests::computed_style_get_round_trips_all_builder_supported_properties_losslessly`
- `computed::tests::computed_style_builder_rejects_duplicate_property_records`
- `computed::tests::computed_style_builder_rejects_value_kind_mismatches`
- `computed::tests::computed_style_builder_requires_total_property_fill`

Those tests are part of the product contract for the current supported
property subset. Follow-on implementation work should extend them rather than
replacing them with looser assertions.
