# R1: Define Cascade Architecture And Style Resolution Contract

Last updated: 2026-04-14  
Status: architecture contract implemented

This document is the source-of-truth contract for Milestone R issue 1: the
architecture boundary, ownership model, and output contract for Borrowser's
cascade and style-resolution engine.

Milestone O established the structured stylesheet/rule/declaration/value model.
Milestone Q established the selector matching contract and its deterministic
match-result surface. The repository still ships a legacy bridge in
`css::cascade::attach_styles` that mutates `html::Node::style` directly.

R1 exists so the next cascade work does not continue growing inside that bridge.

Related code:
- `crates/css/src/cascade.rs`
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/model/mod.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/computed.rs`

Related documents:
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/q6-validity-specificity-match-results.md`
- `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`
- `docs/css/r7-structured-resolved-style-output.md`
- `docs/css/r8-cascade-style-resolution-debug-output.md`
- `docs/architecture/ARCHITECTURE.md`

## Implemented Result

Milestone R now has an explicit in-repository contract for:

- the boundary between stylesheet matching inputs and cascade winner resolution
- the precedence model Borrowser uses to compare declaration candidates
- the current supported property subset resolved by cascade
- the inheritance/default-value responsibilities owned by cascade
- the deterministic resolved-style output shape later computed-style work will
  consume
- the legacy bridge boundary that remains temporary until the R cutover is
  complete
- the major non-goals deferred to later computed-value, invalidation, and
  performance milestones

Follow-up implementation work must conform to this contract. It must not invent
new ad hoc declaration maps, attach new style state directly to DOM nodes, or
re-derive selector semantics inside cascade code.

## Why This Exists

The repository has already done the hard work to establish clean upstream CSS
contracts:

1. `css::model` owns long-lived stylesheet/rule/declaration/value storage.
2. `css::selectors` owns selector structure, specificity, matchability, and
   element matching.
3. `SelectorListMatchOutcome` already gives cascade a deterministic handoff
   surface.

The remaining gap is style resolution itself. Today the shipped browser path:

- walks the DOM directly in `attach_styles`
- flattens matched declarations into raw `(String, String)` pairs
- writes the winners back into `Node::style`
- relies on later computed-style code to perform inheritance/defaulting from
  that compatibility vector

That is acceptable as a temporary bridge. It is not an acceptable long-term
engine architecture.

## Subsystem Boundary

The Milestone R boundary is:

1. `css::model`
   - owns parsed stylesheet/rule/declaration/value structure
   - preserves source order and declaration order
   - does not own selector matching or cascade ordering
2. `css::selectors`
   - owns selector IR, validity, specificity, and matching semantics
   - emits `SelectorListMatchOutcome`
   - does not own declaration winner resolution or defaulting
3. `css::cascade`
   - consumes ordered stylesheets, inline-style declarations, selector match
     outcomes, and parent resolved style
   - filters unsupported declarations to the supported property subset
   - resolves winners with explicit precedence keys
   - fills inherited and initial/default values for supported properties
   - emits deterministic `ResolvedStyle`
4. later computed-style work
   - consumes `ResolvedStyle`
   - parses/interprets authored values into typed computed values
   - remains separate from winner selection, inheritance policy, and source
     ordering

Normative ownership rules:

- selector validity and specificity stay owned by `css::selectors`
- declaration/value storage stays owned by `css::model`
- inheritance/default policy for the supported subset is owned by
  `css::cascade`
- computed-value interpretation is downstream of cascade, not mixed into it

## Current Milestone R Scope

Milestone R's first issue defines the architecture for the current shipped CSS
subset, not the full web platform.

Current cascade inputs in scope:

- ordered stylesheet parse results in page insertion order
- style rules only
- selector match outcomes per rule/element
- inline style attributes parsed as declaration lists
- the current supported property subset already interpreted by
  `css::computed`
- parent resolved style for inheritance

Current property subset in scope:

| property | inherits | initial/default contract |
| --- | --- | --- |
| `background-color` | no | `transparent` |
| `color` | yes | `black` |
| `display` | no | `inline` |
| `font-size` | yes | `16px` |
| `height` | no | `auto` |
| `margin-bottom` | no | `0px` |
| `margin-left` | no | `0px` |
| `margin-right` | no | `0px` |
| `margin-top` | no | `0px` |
| `max-width` | no | `none` |
| `min-width` | no | `auto` |
| `padding-bottom` | no | `0px` |
| `padding-left` | no | `0px` |
| `padding-right` | no | `0px` |
| `padding-top` | no | `0px` |
| `width` | no | `auto` |

Current origin/priority scope:

- author stylesheet declarations
- author inline style declarations
- declaration-level normal and `!important` ordering within the current
  structured author-origin model

The contract already reserves explicit origin and importance slots so later
issues can add user-agent, user, animation, transition, cascade layers, and
other priority levels without redesigning the winner-resolution model.

## Selector Matching Handoff

Selector matching and cascade interact through one explicit rule-level handoff:

- cascade receives a `SelectorListMatchOutcome` for each style rule against one
  target element
- unsupported, invalid, and parsed-no-match outcomes contribute no declaration
  candidates
- when a selector list matches, cascade uses
  `SelectorListMatchOutcome::highest_specificity()` as the rule's effective
  specificity for winner comparison

This is the required contract for comma-separated selectors:

- one style rule may match through more than one selector entry
- declarations from that rule participate only once per property
- the rule's precedence uses the highest specificity among the selectors that
  actually matched
- unmatched selector entries do not contribute specificity

Cascade does not:

- reparse selector text
- decide selector validity
- recompute specificity from raw selector syntax

## Winner-Resolution Model

Cascade winner resolution is defined over declaration candidates.

Each candidate is conceptually:

- one supported property
- one authored declaration source
- one fully ordered precedence key

The precedence key is lexicographic:

1. origin/importance band
2. selector specificity or inline-style specificity slot
3. rule order in stylesheet insertion/source order
4. declaration order within the rule or inline style attribute

Normative rules:

- rule order is stable and deterministic for equivalent stylesheet insertion and
  parse order
- declaration order is preserved exactly from source
- inline style declarations participate as author-origin declarations with their
  own top specificity slot inside the current scope
- winner selection groups candidates by canonical property id
- each property resolves to at most one winning authored declaration

Borrowser's code-level contract for this model now lives in
`crates/css/src/cascade/contract.rs` through:

- `CascadeRuleMatch`
- `CascadePriority`
- `CascadeSpecificity`
- `CascadeDeclarationSource`

## Inheritance And Defaulting Responsibilities

Milestone R assigns inheritance/default fill to cascade for the supported
property subset.

That means:

- if a supported property has a winning authored declaration, resolved style
  records that winner
- if no authored declaration wins, the property inherits, and a parent resolved
  style exists, resolved style records `Inherited`
- if no authored declaration wins, the property inherits, and no parent
  resolved style exists, resolved style records the property's initial/default
  value
- if no authored declaration wins and the property does not inherit, resolved
  style records the property's initial/default value

This keeps the output total over the supported subset and removes
inheritance/default behavior from ad hoc downstream mutation paths.

Important boundary:

- cascade decides whether the source is authored, inherited, or initial
- computed-style code later interprets the chosen value into typed runtime data

The code-level defaulting contract lives in:

- `CascadePropertyId::initial_value()`
- `InitialStyleValue`
- `resolve_initial_style()`
- `resolve_cascade_style(...)`
- `resolve_document_styles(...)`

Examples:

- `color` with no local winner inherits from the parent resolved style
- `background-color` with no local winner falls back to `transparent`
- `width` with no local winner falls back to `auto`

The contract deliberately stops before typed value parsing, unit conversion,
relative-value resolution, or layout-facing normalization.

## Resolved-Style Output Contract

The output of cascade is `ResolvedStyle`.

`ResolvedStyle` is defined as:

- deterministic
- property-addressable
- ordered by canonical property id rather than discovery order
- suitable for snapshots and regression tests
- independent from `html::Node` mutation

Each entry records:

- the property id
- whether the value came from:
  - a winning authored declaration
  - inheritance
  - an initial/default value

For authored winners, the output carries:

- stable source identity
- the precedence key that caused the declaration to win
- the winning authored value through the structured model-layer declaration
  value surface

That last point is important: `ResolvedStyle` is no longer only provenance. It
is the handoff object computed-style work can read directly without re-looking
up winning declarations through stylesheet storage.

This is the handoff object later computed-value work must consume. It is the
replacement for the current `(String, String)` winner vector attached to DOM
elements.

## Determinism And Invariants

Milestone R establishes these invariants:

- equivalent stylesheet insertion order and source order produce the same rule
  order
- equivalent selector match outcomes produce the same effective specificity
- equivalent candidate sets produce the same winners
- resolved-style entry order is canonical and independent of candidate
  discovery order
- each supported property appears at most once in final resolved style
- inherited/default entries are explicit rather than implicit absence
- final `ResolvedStyle` construction is total over `CascadePropertyId::ALL`
- selector matchability state does not get collapsed or reinterpreted inside
  cascade

Debug/regression requirement:

- cascade-facing contract types must expose stable, deterministic debug output
  suitable for regression tests

The maintained Milestone R debug surfaces include:

- `cascade_evaluation_debug_snapshot(...)`
- `CascadeWinnerSet::to_debug_snapshot()`
- `ResolvedStyle::to_debug_snapshot()`
- `ResolvedDocumentStyle::to_debug_snapshot()`
- `resolve_document_styles_debug_snapshot(...)`

## Legacy Bridge Status

The current `attach_styles` path is now explicitly legacy.

It remains in place only so the existing browser path can keep running while
computed-style consumption migrates. It is now a projection from structured
document-level resolved styles into:

- `html::Node::style`
- `css::computed::compute_style`
- any browser/layout consumers that still assume a DOM-attached declaration
  vector

This bridge is not the normative architecture for style resolution anymore;
`ResolvedDocumentStyle` is the structured cascade output for DOM-level style
resolution.

## Deferred Work And Non-Goals

R1 does not implement or redefine:

- typed computed values
- percentage/relative-unit resolution
- shorthand expansion
- CSS-wide keywords such as `inherit`, `initial`, `unset`, or `revert`
- custom-property substitution
- at-rule activation such as `@media` or `@supports`
- animation/transition cascade levels
- selector invalidation, dependency tracking, or caching
- incremental restyle scheduling
- performance-oriented storage/layout choices beyond deterministic contract
  types
- retirement of the legacy `Node::style` bridge in the shipped browser path

It also does not broaden property coverage just to make the cascade model look
more complete.

## Exit Condition For This Issue

This issue is complete when later Milestone R implementation work can answer
all of the following without reinterpretation:

- What does selector matching hand to cascade?
- How are declaration candidates compared?
- Who owns inheritance and default values?
- What does style resolution produce?
- What is intentionally still outside the cascade engine?

That contract now exists in both documentation and code.
