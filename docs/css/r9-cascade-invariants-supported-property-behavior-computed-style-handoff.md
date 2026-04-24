# R9: Cascade Invariants, Supported Property Behavior, And Computed-Style Handoff

Last updated: 2026-04-17  
Status: milestone close-out contract implemented

This document is the source-of-truth contract for Milestone R issue 9 and the
Milestone R close-out boundary overall: the implemented cascade invariants,
supported property behavior, final resolved-style contract, and the handoff
into later computed-style work.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/computed.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/r2-structured-cascade-inputs-candidate-model.md`
- `docs/css/r3-core-cascade-winner-resolution.md`
- `docs/css/r4-rule-origin-priority-model-current-scope.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`
- `docs/css/r7-structured-resolved-style-output.md`
- `docs/css/r8-cascade-style-resolution-debug-output.md`
- `docs/architecture/ARCHITECTURE.md`

## Implemented Result

Milestone R now has an explicit end-to-end cascade contract in code,
documentation, and regression tests.

The implemented engine-owned staircase is:

1. stylesheet/value model data from `css::model`
2. selector match outcomes from `css::selectors`
3. `CascadeRuleInput`
4. `CascadeDeclarationCandidate`
5. `CascadeWinnerSet`
6. `ResolvedStyle`
7. `ResolvedDocumentStyle`

The old DOM-attached `(String, String)` declaration vector on `html::Node` is
no longer the normative style-resolution result. It remains only as a legacy
projection for compatibility callers that still read `Node::style`; the
Milestone S runtime path consumes structured resolved and computed styles.

## Final Milestone R Pipeline

For one element, Borrowser's implemented cascade flow is:

1. match stylesheet rules through `SelectorMatchingContext`
2. derive rule-level specificity from `SelectorListMatchOutcome`
3. materialize matched declarations and inline declarations as
   `CascadeRuleInput`
4. filter supported declarations into `CascadeDeclarationCandidate`
5. compare candidates by explicit `CascadePriority`
6. produce authored winners in `CascadeWinnerSet`
7. fill inheritance and initial/default values into total `ResolvedStyle`

For one DOM tree, `resolve_document_styles(...)` repeats that flow in
selector-DOM document order, stores one total `ResolvedStyle` per element in
`ResolvedDocumentStyle`, and preserves hardening failures as explicit
`StyleResolutionError` results rather than degrading them into an empty style
tree.

## Supported Property Subset

Milestone R resolves this supported property subset only:

| property | inherits | when no local winner exists | initial/default contract | notes |
| --- | --- | --- | --- | --- |
| `background-color` | no | use initial | `transparent` | non-inherited winner-or-initial property |
| `color` | yes | inherit from parent if present, else use initial | `black` | inherited property in the current subset |
| `display` | no | use initial | `inline` | HTML/UA element defaults such as block display for `div` remain outside the cascade contract until represented as explicit UA-origin rules |
| `font-size` | yes | inherit from parent if present, else use initial | `16px` | inherited property in the current subset |
| `height` | no | use initial | `auto` | non-inherited winner-or-initial property |
| `margin-bottom` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `margin-left` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `margin-right` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `margin-top` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `max-width` | no | use initial | `none` | non-inherited winner-or-initial property |
| `min-width` | no | use initial | `auto` | non-inherited winner-or-initial property |
| `padding-bottom` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `padding-left` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `padding-right` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `padding-top` | no | use initial | `0px` | non-inherited winner-or-initial property |
| `width` | no | use initial | `auto` | non-inherited winner-or-initial property |

This table is implemented by `CascadePropertyId::metadata()` and
`CascadePropertyId::initial_value()`. No downstream subsystem should invent
missing-property behavior independently for this subset.

## Winner Resolution Rules

Milestone R resolves authored conflicts through these rules:

- only declarations with
  `CascadeDeclarationApplicability::Supported(CascadePropertyId)` become
  comparable candidates
- unsupported properties, custom properties, and invalid property names remain
  explicit on rule inputs and in debug traces, but they do not participate in
  winner selection
- candidate precedence is lexicographic by:
  1. `CascadeOriginBand`
  2. `CascadeSpecificity`
  3. `rule_order`
  4. `declaration_order`
- inline style precedence comes from `CascadeSpecificity::InlineStyle`, not
  from a sentinel rule-order value
- current emitted origin/priority scope includes:
  - author stylesheet declarations
  - author inline style declarations
  - declaration-level normal and `!important` ordering
- reserved future precedence bands such as `Animation` and `Transition` remain
  modeled but are not emitted by the current hot path
- winner resolution is sparse by contract: at most one authored winner per
  supported property

Equal-key behavior is deterministic but degenerate: sort order is stable, so if
two candidates reach exactly the same candidate key, the later candidate in the
incoming slice wins. That is a maintenance rule, not normative CSS semantics.

## Inheritance And Defaulting Rules

Milestone R assigns inheritance/defaulting to the cascade layer, not to the
computed-style layer.

For each property in `CascadePropertyId::ALL`, `resolve_cascade_style(...)`
uses this order:

1. if a local authored winner exists, record `ResolvedValueSource::Winner`
2. otherwise, if the property inherits and a parent resolved style exists,
   record `ResolvedValueSource::Inherited`
3. otherwise, record `ResolvedValueSource::Initial(property.initial_value())`

That means:

- local authored winners always outrank inheritance
- local authored winners always outrank initial/default values
- only `color` and `font-size` inherit in the current supported subset
- inherited properties at the root fall back to explicit initial/default
  values, not implicit absence
- non-inherited properties never inherit accidentally
- defaulting is explicit and total, not a downstream guess

`ResolvedValueSource::Inherited` records provenance, not a copied authored
value. Downstream consumers are expected to follow the parent style chain when
interpreting inherited properties.

## Structured Output Contract

The final cascade output for one element is `ResolvedStyle`.

`ResolvedStyle` is total over `CascadePropertyId::ALL` and stores entries in
canonical property order. Each entry records exactly one of:

- `ResolvedValueSource::Winner(CascadeWinner)`
- `ResolvedValueSource::Inherited`
- `ResolvedValueSource::Initial(InitialStyleValue)`

For winner entries, `CascadeWinner` carries:

- stable declaration source identity
- the explicit precedence key that caused the declaration to win
- the winning authored value through `CascadeSpecifiedValue`

That output is the engine-owned handoff to later computed-style work. It is not
a presentation-facing CSS serialization and it is not a DOM mutation artifact.

At document scope, `ResolvedDocumentStyle` stores one per-element
`ResolvedStyle` in selector-DOM document order. This is the structured DOM-level
style output for the current runtime integration path.

## Invariants Contributors Must Preserve

The following invariants are part of the Milestone R contract:

- selector specificity is taken from matched `SelectorListMatchOutcome`
  specificity; cascade must not reparse selectors or recompute specificity from
  raw text
- rule order is deterministic for equivalent stylesheet insertion order and
  source order
- declaration order is preserved exactly from the source rule or inline style
  attribute
- candidate ordering is deterministic and stable for equal keys
- `CascadeWinnerSet` contains at most one winner per supported property and is
  stored in canonical property order
- `ResolvedStyle` contains exactly one entry per supported property
- `ResolvedStyleBuilder` rejects duplicate property insertion in all builds
- `ResolvedStyleBuilder::record_inherited(...)` is valid only for inherited
  properties
- structured document style resolution does not mutate the DOM
- legacy `attach_styles(...)` behavior is projection only; it must not become
  the owner of cascade semantics again
- snapshot label grammar and ordering are maintained contract surfaces once
  covered by exact regression tests

If future work changes any of these invariants, code, tests, and docs must all
change together.

## Handoff To Computed-Style Work

Milestone R ends at resolved specified-style output, not at typed computed
values.

The next computed-style milestone must consume the structured cascade contract
like this:

- when a property entry is `Winner`, interpret `CascadeWinner.value`
- when a property entry is `Inherited`, read the property from the parent style
  chain
- when a property entry is `Initial`, interpret `InitialStyleValue`

Computed-style work must not:

- re-run selector matching
- re-run winner resolution
- guess inheritance rules from property names
- invent initial/default values for the supported subset
- treat `html::Node::style` as the normative style-resolution source

During the current bridge phase, `computed.rs` still reads authored winner
strings projected into `Node::style`. That is transitional behavior only. The
long-term handoff object is `ResolvedStyle`, with `ResolvedDocumentStyle` as
the DOM-level structured output.

## What Remains Deferred

Milestone R intentionally does not implement or redefine:

- typed computed-value parsing and normalization
- relative-unit resolution, percentage resolution, and layout-facing value
  normalization
- CSS-wide keywords such as `inherit`, `initial`, `unset`, `revert`, or
  `revert-layer`
- custom-property substitution and `var(...)`
- shorthand expansion
- user-agent stylesheet defaults as explicit origin rules
- animation and transition cascade levels in the emitted hot path
- restyle invalidation, caching, or performance-oriented storage refactors
- retirement of the compatibility `Node::style` bridge

Those responsibilities belong to later milestones, starting with the
computed-values cutover work.

## Known Follow-On Considerations

These are not Milestone R defects, but they are the main structural pressure
points future contributors should keep in mind as the engine moves into
computed-style and runtime cutover work.

### 1. Document-level style resolution is still function-oriented

`resolve_document_styles(...)` and
`resolve_document_styles_debug_snapshot(...)` currently assemble the per-element
flow inline from:

- selector DOM indexing
- selector matching context
- rule-input collection
- winner resolution
- inheritance/default fill

That is acceptable for Milestone R because the code remains readable and the
ownership boundaries are clear. As later milestones add direct computed-style
consumption, caches, invalidation hooks, or richer debug tooling, a dedicated
internal style-resolution session object may become the cleaner runtime-owned
shape.

### 2. Inline style parsing is still integration-localized wrapper parsing

Inline style attributes currently enter the structured cascade pipeline through
localized wrapper parsing rather than through a first-class declaration-list API
owned by the syntax/model layers.

That is acceptable for Milestone R because the workaround is localized and the
structured cascade contract does not depend on synthetic selector semantics.
Long-term, the cleaner architecture is a first-class declaration-list parse
entrypoint that produces structured model declarations directly for inline style
attributes.

### 3. `ResolvedValueSource::Inherited` is provenance-only by design

`ResolvedValueSource::Inherited` records that a property resolved through
inheritance, but it does not copy the resolved parent value into the child
entry.

That is the intended design for Milestone R because it preserves provenance and
keeps inheritance/default policy separate from computed-value interpretation.
The next milestone must preserve that discipline and treat:

- `Winner`
- `Inherited`
- `Initial`

as three distinct runtime cases rather than flattening inheritance into hidden
fallback behavior.

### 4. The supported-property registration model is intentionally small-scale

The current `CascadePropertyId` plus `metadata()` model is the correct shape
for the current 16-property subset.

If the supported surface grows materially, future contributors should revisit
whether a more explicit property-registration or metadata-expansion strategy is
worth introducing. Milestone R does not require that additional machinery yet.

### 5. The compatibility bridge remains runtime-significant until cutover

Milestone R moved cascade semantics into structured code, but the shipped
runtime still projects authored winners into `html::Node::style` so
`computed.rs` can keep running through the legacy path.

That is acceptable at the end of Milestone R, but it remains the largest
architectural risk moving into the next milestone. Future work must keep
shrinking the significance of the compatibility bridge until `ResolvedStyle`
becomes the actual consumed input of computed-style construction.

### 6. The crate-root export surface is broad during milestone construction

The `css` crate currently re-exports many cascade contract types from the crate
root. That is useful while the milestone is under active construction, but it
may become worth curating the public surface more deliberately once the runtime
integration settles.

Milestone R does not require pruning that surface preemptively.

### 7. Exact debug snapshots are intentionally high-discipline maintenance surfaces

Milestone R's snapshot surfaces are versioned, deterministic, and treated as
contract-quality regression outputs. That is the right choice, but it creates a
deliberate maintenance cost: changing snapshot wording is a contract change, not
mere log churn.

Future work should continue treating snapshot grammar changes as updates that
must be reflected in code, docs, and tests together.

## Regression And Documentation Alignment

Milestone R now has aligned issue-level and milestone-level contract docs:

- R1 defines the architecture boundary
- R2 defines structured cascade inputs
- R3 defines winner resolution
- R4 defines the current origin/priority model
- R5 defines inheritance behavior
- R6 defines initial/default handling
- R7 defines structured document-level output
- R8 defines debug/regression surfaces
- R9 summarizes the final Milestone R contract and the computed-style handoff

The implemented regression surface covers:

- candidate construction and ordering
- winner-resolution precedence and deterministic tie behavior
- inheritance/defaulting behavior
- total resolved-style construction
- structured document-level style resolution
- stable cascade and resolved-style snapshots

Future contract changes should update the relevant issue-level documents and
this milestone close-out summary together.

## Exit Condition For This Issue

Milestone R can be considered complete when contributors can answer, without
reinterpretation:

- which properties the cascade engine currently supports
- how authored declarations become candidates
- how candidates are ordered and winners selected
- how inheritance and defaulting behave
- what `ResolvedStyle` and `ResolvedDocumentStyle` mean
- what runtime code may still treat as legacy compatibility behavior
- and what the computed-style milestone must consume versus what it must not
  reintroduce

That contract now exists in the repository and matches the implemented cascade
engine.
