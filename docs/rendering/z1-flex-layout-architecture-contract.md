# Z1: Flex Layout Architecture And Integration Contract

Last updated: 2026-06-02
Status: architecture contract for Milestone Z issue 1

This document defines how Borrowser will introduce flex layout as a
first-class layout system. It is a pre-implementation contract. It does not
ship parser support, computed-style fields, box-tree behavior, layout
algorithms, paint behavior, or runtime behavior.

Related code for future implementation:

- `crates/css/src/values.rs`
- `crates/css/src/computed/style.rs`
- `crates/css/src/computed/builder.rs`
- `crates/layout/src/box_tree/display.rs`
- `crates/layout/src/box_tree/formatting.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/flow.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/phase.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:

- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`
- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Purpose

Milestone Z introduces flexbox layout support for a scoped but meaningful
subset. Z1 establishes the architecture before implementation so flex does not
become an isolated layout island, a block-flow patch, or a collection of
special cases in paint or browser orchestration.

The intended future pipeline shape is:

```text
DOM + stylesheets
  -> CSS parsing, cascade, and computed flex-related values
  -> StyledNode tree
  -> DisplayBoxGeneration with explicit flex container behavior
  -> BoxTree with generated flex formatting context metadata
  -> flex item collection from generated in-flow child boxes
  -> sizing and intrinsic contribution resolution
  -> flex line construction and main/cross-axis layout
  -> LayoutBox geometry and deterministic flex debug metadata
  -> paint consuming final layout geometry
```

Flex layout must extend the existing box-tree, formatting-context, sizing,
normal-flow, out-of-flow, and debug contracts. It must not bypass them.

## Ownership Boundaries

CSS owns authored flex syntax and computed values:

- parsing `display: flex` and supported flex properties
- cascade, inheritance, initial values, and computed-value normalization
- exposing canonical computed values to layout

CSS must not:

- decide which generated boxes are flex containers or flex items
- build flex lines
- resolve used flex item sizes
- distribute free space
- compute final geometry
- infer paint ordering or clipping from flex properties

Layout owns flex semantics after computed style is available:

- mapping supported computed display values to flex container box generation
- assigning flex formatting contexts to generated boxes
- determining flex item participation from generated in-flow child boxes
- constructing main-axis and cross-axis layout inputs
- consuming sizing and intrinsic sizing primitives
- distributing free space for the supported subset
- producing final `LayoutBox` geometry and deterministic debug metadata
- excluding out-of-flow boxes from flex item participation while preserving
  their generated identity and positioned handoff metadata

Paint consumes layout output:

- final geometry
- text fragments and replaced-element geometry
- overflow clip metadata when present
- future stacking or paint-order metadata supplied by layout

Paint must not inspect DOM shape, raw CSS declarations, or computed flex values
to decide flex container behavior, flex item participation, axes, alignment,
or free-space distribution.

Browser/runtime orchestration owns invalidation, retained style artifacts,
resources, and frame lifecycle. It must not infer flex layout semantics from
DOM or CSS. Runtime-visible behavior must continue to flow through explicit
style, layout, and paint phase outputs.

## Flex Container Role

A flex container is a generated principal layout box whose computed display
value maps to an explicit flex display behavior. The role belongs to the
generated box, not to the DOM node.

Future implementation should introduce code vocabulary equivalent to:

```text
css::Display::Flex
DisplayBoxBehavior::FlexContainer
FormattingContextKind::Flex
```

Those names are future direction, not Z1 code changes.

A flex container:

- generates a principal box through normal box generation
- establishes a flex formatting context for eligible generated children
- participates in its parent formatting context according to its outer display
  behavior for the supported subset
- owns flex item collection for its direct generated in-flow children
- provides the containing-size and available-space inputs used by its flex
  layout algorithm
- produces final geometry for itself and its flex items through layout

For the initial Milestone Z subset, `display: flex` represents a block-level
flex container. Inline flex containers are deferred until the display model
explicitly supports that outer/inner distinction.

## Flex Item Role

A flex item is an eligible generated in-flow child box of a flex container. It
is not a DOM child and not simply a styled node.

Flex item participation must be derived from the flex container's generated
box-tree children after display suppression, replaced-element classification,
anonymous box generation, list marker handling, and any supported generated-box
normalization have run.

In the initial supported subset:

- direct generated in-flow child boxes of a flex container become flex items
- out-of-flow generated boxes remain in the box tree but do not become flex
  items
- text and inline child runs must become flex items only through explicit
  generated-box rules, not by walking DOM text nodes directly
- replaced boxes may be flex items when they are generated in-flow child boxes
- anonymous boxes may be flex items when box generation creates them as direct
  generated children of the flex container

Future implementation should introduce explicit layout vocabulary for this
relationship, such as:

```text
FlexItemRole
FlexFormattingParticipation
FlexItemId
```

The exact Rust names may differ, but the contract is fixed: flex item identity
is generated layout identity and must be deterministic for one box-tree
generation.

## Box-Tree Integration

Flex integrates at box generation and formatting metadata assignment. It must
not be added by a later geometry pass that checks DOM tag names or raw CSS.

Future `display: flex` support must update:

- display parsing and computed display exposure in CSS
- `DisplayBoxGeneration`
- `PrincipalBox`
- `DisplayBoxBehavior`
- formatting-context establishment rules
- box-tree debug serialization
- layout projection and phase debug output
- representative tests
- this document or a follow-up implementation contract

Box generation remains responsible for deterministic generated `BoxId`s,
parent/child relationships, source attribution, containing-block metadata,
replaced-element metadata, list marker metadata, and anonymous box metadata.
Flex work must preserve those relationships and add flex-specific metadata
alongside them when implementation begins.

Adding `display: flex` must not silently route through
`DisplayBoxBehavior::Block`. Unsupported display values must remain explicit
until the corresponding layout contract and tests exist.

## Formatting-Context Integration

Flex is a formatting context kind. It must be represented beside block and
future grid/table contexts, not as a mode flag inside block flow.

Future implementation should extend the current formatting-context vocabulary
with a flex context. A flex container establishes that context for its flex
items. Its items do not participate as ordinary block-level or inline-level
children in the container's block or inline formatting context.

The container itself still participates in its parent context. For the initial
subset, a `display: flex` container is a block-level participant in the parent
normal-flow context while establishing an internal flex formatting context for
its children.

Flex formatting context assignment must preserve the existing distinction
between:

- the context a generated box participates in
- the context a generated box establishes for descendants
- containing-block relationships
- inline formatting context relationships
- out-of-flow positioned containing-block relationships

Those concepts must not be collapsed into one flex-specific parent pointer.

## Sizing And Intrinsic Sizing Integration

Flex sizing must use the Milestone X sizing vocabulary. It may introduce
flex-specific wrappers, but it must not reintroduce raw viewport width,
parent-width shortcuts, or local min/max clamp logic.

Future flex layout should consume:

- `ContainingSize`
- `AvailableSpace`
- `ConstraintSpace`
- computed width, height, min/max values
- box metrics
- intrinsic text, inline, replaced, and control contributions
- generated box metadata

Future flex layout should produce:

- used main-axis and cross-axis sizes for the container and items
- deterministic reasons for flex basis selection
- deterministic free-space distribution metadata
- final content-box and border-box geometry through existing layout outputs

Flex basis resolution should be documented and implemented as a layout-owned
size selection step. `flex-basis: auto` must integrate with the existing style
width/height and intrinsic sizing contracts instead of duplicating them inside
the flex algorithm.

Min/max constraints remain centralized sizing behavior. Flex item grow/shrink
results must be clamped through structured constraint handling and must expose
debug metadata when constraints affect the final size.

Full CSS min-content and max-content flexbox nuance is deferred. The initial
subset may define deterministic approximations only when they are explicitly
documented as supported behavior rather than browser-perfect compatibility.

## Normal-Flow And Out-Of-Flow Interaction

A flex container participates in its parent normal flow according to its outer
display behavior. For Milestone Z's initial subset, `display: flex` is a
block-level normal-flow participant.

Inside the flex container, flex layout replaces ordinary block or inline child
placement for flex items. Direct flex items must not also be stacked as normal
block children or tokenized as inline children by the container's internal
layout pass.

Out-of-flow descendants remain generated layout boxes with source identity,
style, containing-block metadata, and debug visibility. They do not participate
as flex items and do not contribute to the flex container's in-flow line
sizing, free-space distribution, or auto size contribution in the supported
subset.

Positioned containing-block lookup must continue to use the layout-owned
generated-box relationships introduced in Milestone Y. Flex must not invent a
parallel positioned ancestor lookup.

Nested formatting contexts are allowed only through explicit generated-box
roles. A flex item may establish its own block, inline, flex, or future layout
context for descendants when the corresponding display and formatting-context
contracts support that behavior.

## Main-Axis And Cross-Axis Responsibilities

Flex layout owns both main-axis and cross-axis item placement.

The main axis is selected from computed `flex-direction`. For the initial
Milestone Z subset:

- `row` maps to the current horizontal inline axis
- `column` maps to the current vertical block axis
- reverse directions are deferred unless a follow-up issue explicitly supports
  them
- writing-mode-dependent axis remapping is deferred

The main-axis pass is responsible for:

- collecting flex items in deterministic generated child order
- resolving each item's flex basis
- summing base sizes and margins for the supported subset
- computing available main-axis free space
- applying supported `flex-grow` and `flex-shrink`
- applying supported main-axis distribution
- producing final main-axis offsets and sizes

The cross-axis pass is responsible for:

- resolving container cross size where supported
- resolving item cross sizes from explicit size, auto size, intrinsic
  contribution, or stretch behavior in the supported subset
- applying supported cross-axis alignment
- producing final cross-axis offsets and sizes

Flex axis code must use logical-axis vocabulary and only convert to physical
coordinates at the layout geometry boundary for the current horizontal
writing-mode subset.

## Supported Milestone Z Subset

Milestone Z should target a small, deterministic, extensible subset:

- `display: flex` as a block-level flex container
- single-line flex containers
- `flex-direction: row`
- `flex-direction: column`
- basic `flex-grow`
- basic `flex-shrink`
- basic `flex-basis`, including `auto` and finite length values once CSS
  exposes them
- deterministic main-axis free-space distribution
- basic `justify-content` values needed for representative modern layouts
- basic cross-axis sizing and alignment
- nested normal-flow content inside flex items through existing formatting
  contexts
- replaced elements as flex items when generated as in-flow child boxes
- deterministic debug output for flex containers, items, axes, basis,
  distribution, and final item geometry

The exact supported property keywords should be finalized by follow-up issues
when CSS parsing and computed style are introduced. Unsupported keywords must
be rejected, ignored, or mapped according to explicit CSS contracts before
layout consumes them.

## Deferred Features And Non-Goals

Z1 and the initial Milestone Z subset deliberately defer:

- parser support and computed-style code changes in Z1
- flex layout algorithm code in Z1
- inline flex containers
- multi-keyword outer/inner display decomposition
- `flex-flow` shorthand
- wrapping and multi-line flex layout details
- advanced `align-content`
- baseline alignment
- `gap`, unless a later issue explicitly introduces it
- `order`
- reverse directions unless explicitly scoped later
- writing modes
- fragmentation
- floats inside flex formatting behavior beyond existing generated-box
  contracts
- browser-perfect min-content and max-content flex sizing nuance
- exhaustive web-compatible flexbox edge cases
- paint-time flex inference
- runtime-specific flex behavior

Deferred features must stay visible as unsupported behavior. They must not be
approximated by falling back to block layout unless a follow-up contract
explicitly defines that fallback.

## Determinism And Debug Expectations

Flex debug output is an internal regression contract, not a public API.

Future flex debug surfaces must be deterministic for a fixed DOM, computed
style tree, viewport, text measurer, and replaced-element metadata. They should
record semantic flex decisions instead of reconstructing them from final
rectangles.

Expected future debug metadata includes:

- generated flex container `BoxId`
- flex formatting context id
- flex item generated `BoxId`s in layout order
- source generated child order before any future ordering feature
- main axis and cross axis
- flex direction
- flex basis per item
- grow and shrink factors per item
- available main size and free space
- distribution decision per item
- applied min/max constraint effects
- cross-axis size and alignment decision
- line membership for the single-line subset
- explicit labels for unsupported or deferred behavior where relevant

Debug labels must come from typed layout vocabulary. Floating-point output
must use stable formatting consistent with existing layout debug snapshots.

## Future Issue Breakdown

Follow-up Z issues should implement flex in small, testable steps:

1. Add CSS parsing and computed-value support for `display: flex` and the
   minimal flex property subset.
2. Add layout code vocabulary for flex display behavior, flex formatting
   context establishment, and debug serialization without implementing full
   geometry.
3. Add generated-box flex item participation from direct generated in-flow
   children, including deterministic box-tree tests.
4. Add flex container sizing and flex basis resolution through the shared
   sizing model.
5. Add single-line row layout with deterministic main-axis distribution.
6. Add column layout using the same axis abstraction.
7. Add basic cross-axis sizing and alignment.
8. Add replaced-element and nested-content flex item coverage.
9. Add out-of-flow descendant regression coverage inside flex containers.
10. Add representative layout/debug snapshots and browser phase-boundary
    preservation tests.

Each follow-up issue should state the supported subset, update contracts when
introducing new engine concepts, and add targeted deterministic tests. Full CI
can wait until the implementation series or milestone is ready to close unless
a follow-up issue crosses broader subsystem boundaries.

## Close Criteria For Z1

Z1 is closeable when:

- flex is documented as a first-class formatting context
- flex containers and flex items are defined as generated layout boxes
- CSS, layout, paint, and browser/runtime ownership boundaries are explicit
- box-tree, formatting-context, sizing, normal-flow, out-of-flow, and debug
  integration contracts are explicit
- the Milestone Z supported subset and non-goals are documented
- follow-up implementation issues have a clear sequence
- no parser, computed-style, layout algorithm, paint, or runtime behavior is
  implied to exist before implementation
