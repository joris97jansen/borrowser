# S9: Property System, Computed-Style Contract, And Runtime Handoff

Last updated: 2026-04-22  
Status: implemented

This document closes Milestone S. It is the downstream-facing contract for
Borrowser's CSS property system, specified/computed value pipeline, final
`ComputedStyle` representation, and runtime consumption boundaries.

Related code:
- `crates/css/src/properties.rs`
- `crates/css/src/specified.rs`
- `crates/css/src/computed.rs`
- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/cascade/document.rs`
- `crates/browser/src/view.rs`
- `crates/layout/src`
- `crates/gfx/src`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s2-property-identifiers-property-registry.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`
- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/s6-computed-style-assembly-pipeline.md`
- `docs/css/s7-structured-computed-style-representation.md`
- `docs/css/s8-deterministic-debug-regression-coverage.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Milestone S Result

Milestone S establishes a typed, deterministic CSS property/value pipeline:

```text
model DeclarationValue
  -> SpecifiedPropertyValue
  -> ResolvedStyle / ResolvedDocumentStyle
  -> ComputedValue
  -> ComputedStyle / ComputedDocumentStyle
  -> StyledNode
```

The runtime-facing result is `ComputedStyle`: a total, normalized style object
for the supported property subset. Layout, paint, text measurement, and browser
view construction consume `ComputedStyle` or `StyledNode`; they do not parse
CSS text, inspect cascade winners, or recover from invalid declarations.

The production document path is:

```text
DOM + StylesheetParse[]
  -> compute_document_styles(...)
  -> build_style_tree_with_stylesheets(...)
  -> StyledNode tree
```

Compatibility APIs still exist, but they are not the primary runtime contract.

## Ownership Boundaries

The CSS pipeline has explicit layer ownership:

- `css::model` owns parsed stylesheet/rule/declaration/value structure.
- `css::properties` owns the supported property universe and metadata.
- `css::specified` owns property-aware specified-value parsing and validation.
- `css::cascade` owns selector-driven winner resolution, inheritance source
  selection, and initial/default source selection.
- `css::computed` owns normalization, computed-style assembly, and styled-tree
  construction from structured computed styles.
- `layout` owns geometry and fragmentation decisions.
- `gfx` owns paint and interactive rendering behavior.

Downstream crates must treat CSS as an already-computed runtime dependency.
They may use typed computed accessors, `ComputedStyle::get(PropertyId)`, or
`ComputedStyle::entries()`. They must not introduce property-specific string
parsing, fallback recovery, or duplicate property metadata.

## Property System Contract

`PropertyId` is the stable identity for one supported CSS property.
`property_registry()` is the first-class registry for the supported property
universe. `PropertyId::metadata()` is the normative source for:

- inheritance behavior
- initial/default value
- specified-value kind
- computed-value kind
- invalid-value policy
- length sign policy

No downstream subsystem may restate those facts in a second match table. If a
layout or paint feature needs a property fact, that fact belongs in
`PropertyMetadata` or in a derived typed computed value, not in the consumer.

Canonical property iteration comes from the registry. Debug output and total
style assembly must not depend on map order, struct field order, or caller
insertion order.

## Parsed, Specified, And Computed Values

Milestone S keeps three value layers separate:

1. Parsed authored value
   - represented by `model::DeclarationValue`
   - preserves syntax-derived component structure
   - is not a runtime value
2. Specified value
   - represented by `SpecifiedPropertyValue`
   - selected by `PropertySpecifiedValueKind`
   - validates grammar and property-local range policy
   - preserves authored/canonical boundaries needed before computation
3. Computed value
   - represented by `ComputedValue`
   - selected by `PropertyComputedValueKind`
   - normalized into runtime-ready color, display, length, auto, or none forms
   - accepted by `ComputedStyleBuilder`

The computed layer normalizes validated specified data. It must not reparse
authored strings or inspect raw parser internals.

## Normalization Contract

Current normalization is deterministic and layout-independent:

- supported colors normalize to RGBA channels
- supported display keywords normalize to the runtime `Display` enum
- supported lengths normalize to CSS px
- supported sizing percentages normalize to finite fractions and remain
  unresolved until layout receives a containing-size basis
- unitless zero normalizes to `0px`
- `auto` remains the controlling branch for width, height, and min-width
- `none` remains the controlling branch for max-width
- runtime scalar overflow returns `ComputedValueNormalizationError`

Milestone S does not resolve percentages, relative units, `calc(...)`, custom
properties, CSS-wide keywords, or layout-dependent values. Supported sizing
percentages are preserved for the layout sizing model rather than resolved in
CSS. Future support for broader value families must extend the specified and
computed value families without moving value resolution into paint.

## Invalid Value Handling

The current invalid-value policy is
`PropertyInvalidValuePolicy::RejectDeclaration`.

Invalid supported declarations are rejected before cascade winner resolution.
They can appear in debug surfaces as invalid declarations, but they cannot
become candidates, winners, specified values in computed assembly, or computed
style entries.

Fallback is owned by cascade:

1. another valid winning declaration for the property
2. inherited value when the property inherits and a parent style exists
3. property initial/default value from metadata

Computed style never stores invalid values. Layout, paint, and other runtime
systems must not implement post-hoc recovery for supported properties.

## ComputedStyle Contract

`ComputedStyle` is a total immutable value object for the supported property
subset.

Construction paths:

- `ComputedStyle::initial()`
- `ComputedStyle::from_resolved_style(...)`
- `compute_style_from_resolved_style(...)`
- `compute_document_styles(...)`
- `compute_document_styles_from_resolved_styles(...)`
- `ComputedStyleBuilder`

Runtime consumers read through:

- typed accessors such as `color()`, `background_color()`, `font_size()`,
  `display()`, `box_metrics()`, `width()`, `height()`, `min_width()`, and
  `max_width()`
- `ComputedStyle::get(PropertyId)` for one property-aware value
- `ComputedStyle::entries()` for deterministic enumeration

Fields are private by design. `ComputedStyleBuilder` remains the invariant
gate for total property fill, duplicate-property rejection, and computed-value
kind checking.

Grouped runtime fields, such as `BoxMetrics`, are allowed only as lossless
projections over individual supported properties. Grouping must not become a
second property universe.

## Document And Styled-Tree Contract

`ResolvedDocumentStyle` is the structured cascade output. `ComputedDocumentStyle`
is the document-order collection of computed element styles. `StyledNode` is
the tree consumed by layout and paint.

`build_style_tree_from_computed_styles(...)` validates that the computed
document style matches the target DOM by selector identity and element name.
It rejects mismatched handoffs instead of pairing styles by shape alone.

Document, text, and comment nodes receive inherited or initial styles as
needed for tree traversal. Element styles come from `ComputedDocumentStyle`.

## Downstream Expectations

Browser view construction should use `build_style_tree_with_stylesheets(...)`
or an equivalent structured composition of `compute_document_styles(...)` and
`build_style_tree_from_computed_styles(...)`.

Layout may:

- read typed computed values through `ComputedStyle`
- keep references to `StyledNode` and `ComputedStyle`
- perform layout-dependent geometry decisions from already-computed values
- define layout-specific algorithms for block, inline, replaced, and text
  behavior
- consume CSS-owned computed-style layout-impact classification when deciding
  whether style changes affect layout

Layout must not:

- parse CSS text or declaration values
- inspect cascade candidates, winners, or resolved-style provenance
- reinterpret invalid authored values
- duplicate property inheritance/default/value-kind metadata

Paint and `gfx` may:

- read computed color, box, display, sizing, and typography values
- use `ComputedStyle` for text measurement and interactive control rendering
- apply paint/backend-specific rendering policy after layout has produced
  geometry

Paint and `gfx` must not:

- parse authored CSS
- infer missing supported property values
- recover from invalid declarations
- mutate `ComputedStyle`

## Layout-Impact Classification

AC6 adds CSS-owned computed-style layout-impact classification:

- `ComputedStyle::layout_impact_against(...)`
- `ComputedDocumentStyle::layout_impact_against(...)`

The classifier compares computed-style outputs and returns a conservative
impact result for the currently supported property subset. Paint-only
properties such as color, background color, outline, and text decoration may
preserve retained layout. Display, font metrics, box metrics, sizing,
positioning, overflow, z-index, and unknown document-shape changes are
layout-affecting or unknown and must conservatively dirty layout.

Browser/runtime may consume the classification result to decide retained
layout reuse. Browser/runtime must not duplicate the property table or infer
CSS property impact by inspecting declarations.

## Legacy Compatibility Boundary

The following APIs remain compatibility surfaces:

- `attach_styles(...)`
- `compute_style(...)`
- `build_style_tree(...)`

They exist for older tests and callers that still use DOM-attached declaration
vectors. Supported declarations crossing those bridges still go through
property lookup, specified-value parsing, computed normalization, and
`ComputedStyle` invariant checks.

Bridge-era HTML/UA behavior, such as element default display mapping and
button/link defaults, is not the CSS initial-value contract. New runtime code
should use the structured pipeline and should not depend on those temporary
bridge behaviors unless the test is explicitly about compatibility.

## Known Transitional Boundaries

Milestone S closes the property and computed-style architecture, but these
bounded compatibility pieces remain intentionally transitional:

- `attach_styles(...)`, `compute_style(...)`, and `build_style_tree(...)`
  remain legacy bridge APIs. They are allowed to exist for older callers, but
  they must not become the primary runtime style path again.
- `legacy_declaration_value(...)` reparses synthetic stylesheet text in the
  bridge to avoid creating a second declaration parser. This is bridge-only
  behavior until a first-class declaration-value or declaration-list parse
  entrypoint exists.
- HTML/UA-ish defaults such as element display mapping, link color, and button
  defaults remain temporary compatibility behavior. The long-term model is an
  explicit UA-origin stylesheet or equivalent first-class UA styling layer that
  participates in cascade.

## Debug And Regression Contract

Milestone S exposes stable debug surfaces:

- `computed_value_debug_snapshot(PropertyId, &DeclarationValue)`
- `ComputedValue::to_debug_label()`
- `ComputedStyle::to_debug_snapshot()`
- `ComputedDocumentStyle::to_debug_snapshot()`

These surfaces are architecture-facing regression contracts. Changes to their
labels, ordering, or field set must be reviewed as intentional contract
changes. They should describe property ids, specified/computed contracts,
normalized values, stable error labels, and final computed styles rather than
raw parser implementation details.

The package-level golden fixtures under
`crates/css/tests/fixtures/computed/` cover representative normalization,
invalid values, fallback, inheritance, defaults, and final document computed
styles.

## Extending Supported Property Coverage

Adding or changing a supported property must follow this checklist:

1. Add or update the `PropertyId` and registry registration.
2. Define metadata for inheritance, initial/default value, specified kind,
   computed kind, invalid policy, and value-range policy.
3. Extend specified-value parsing in `specified.rs` without reparsing strings.
4. Extend `ComputedValue` normalization if the property needs a new runtime
   value family.
5. Extend `ComputedStyleBuilder` and `ComputedStyle` storage/accessors if the
   property is stored in a new or grouped runtime field.
6. Keep grouped fields lossless through `ComputedStyle::get(...)` and
   `ComputedStyle::entries()`.
7. Add targeted unit tests for lookup, parsing, invalid handling,
   normalization, assembly, and downstream access.
8. Update golden fixtures and docs when debug output or contracts change.

Do not add property behavior directly to layout or paint as a shortcut. If
downstream code needs a new computed fact, model it in the property/value
pipeline first.

## Milestone S Invariants

Milestone S is complete with these invariants:

- one registry-backed supported-property universe
- property-aware specified values for supported declarations
- deterministic specified-to-computed normalization
- invalid supported values rejected before computed style
- total `ResolvedStyle` and `ComputedStyle` outputs
- builder-enforced computed-style invariants
- document-level structured style assembly without DOM style mutation
- deterministic debug snapshots and golden regression coverage
- explicit downstream consumption boundaries for layout and paint

Future milestones may broaden CSS coverage, add layout-dependent value
resolution, replace compatibility bridges, and introduce UA stylesheet rules.
They must preserve the ownership boundaries and invariants documented here
unless the implementation, tests, and docs are changed together.
