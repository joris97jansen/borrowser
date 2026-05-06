# X1: Sizing Architecture And Flow-Correctness Contract

Last updated: 2026-05-05  
Status: implemented architecture contract for Milestone X issue 1

This document is the source-of-truth contract for Milestone X1. It defines how
Borrowser's sizing model must be structured before later Milestone X issues
strengthen width/height resolution, intrinsic sizing, min/max constraints,
percentages, shrink-to-fit behavior, and normal-flow correctness.

Related code:
- `crates/layout/src/lib.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/document.rs`
- `crates/layout/src/geometry.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/mod.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/replaced.rs`
- `crates/layout/src/replaced/intrinsic.rs`
- `crates/layout/src/replaced/size.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/css/src/computed/style.rs`

Related documents:
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`

## Purpose

Milestone W made generated boxes, containing blocks, and formatting-context
membership explicit. Milestone X builds on that foundation by making sizing a
first-class layout subsystem instead of allowing width and height decisions to
remain scattered across geometry projection, block stacking, inline layout, and
replaced-element helpers.

X1 establishes the architecture, responsibilities, invariants, and non-goals.
Later Milestone X implementation must move behavior into this model
incrementally while preserving deterministic debug and regression surfaces.

The intended pipeline shape is:

```text
StyledNode + BoxTree metadata + layout environment
  -> ConstraintSpace for each generated box
  -> IntrinsicSizes for content that can contribute intrinsic dimensions
  -> width/height used-size resolution
  -> normal-flow placement using resolved sizes
  -> LayoutBox geometry projection and debug snapshots
```

## Current Transitional State

The current layout engine already has useful sizing behavior, but it is not yet
centralized:

- `layout_document` passes one `available_width` through the initial geometry
  projection.
- `Rectangle` stores physical x/y/width/height as f32 CSS px.
- `content_x_and_width`, `content_y`, and `content_height` centralize padding
  subtraction for current content-box helpers.
- Inline layout computes line boxes from measured text and atomic inline boxes.
- Replaced elements carry `IntrinsicSize` metadata on `BoxNode` and
  `LayoutBox`.
- Image sizing is partially centralized in `replaced::size`; form-control
  sizing still contains local intrinsic/fallback logic in inline layout.
- CSS computed style currently supports px-only `width`, `height`,
  `min-width`, and `max-width`; percentages and min/max-height are deferred.

Milestone X must not treat these existing paths as separate authorities. They
are transitional call sites that should migrate toward the X sizing contract.

## Sizing Ownership

Layout owns all used sizing. CSS owns computed values only.

CSS computed style provides:

- computed `width` and `height`, where `None` currently represents `auto`
- computed `min-width`, where `None` currently represents `auto`
- computed `max-width`, where `None` currently represents `none`
- future computed percentage and min/max-height value representations
- computed display and box metrics required by layout

CSS computed style must not:

- resolve percentages against a containing block
- perform shrink-to-fit calculations
- choose intrinsic fallbacks for replaced elements or form controls
- compute used content, padding, border, or margin box dimensions
- inspect generated box-tree relationships

Box generation provides:

- generated `BoxId` identity
- generated parent/child order
- `ContainingBlockId`
- block and inline formatting participation
- replaced-element classification
- replaced-element intrinsic metadata when available from runtime resources

Box generation must not:

- compute used width/height
- inspect child measured text to decide principal box generation
- clamp min/max constraints
- resolve percentages

The sizing subsystem owns:

- constructing a `ConstraintSpace` for each generated box
- resolving definite and auto width/height behavior for the supported subset
- collecting and exposing `IntrinsicSizes`
- applying min/max constraints in the correct order
- resolving percentages only when their basis is definite
- returning deterministic `UsedContentSize` values with explicit
  `SizeResolutionReason`
- defining when a value remains deferred because the containing size is
  indefinite or the feature is outside the milestone scope

Normal-flow layout owns:

- placing boxes after their relevant used sizes are known
- stacking block-level children in block formatting contexts
- building inline lines from inline participants and atomic inline sizes
- feeding content-size results back into auto block-size calculations

Normal-flow layout must not rediscover width/height rules locally. It may ask
the sizing subsystem for used sizes and intrinsic contributions.

## Contract Types

`crates/layout/src/sizing.rs` defines the code-level vocabulary for Milestone X:

- `CssPx`: non-negative finite CSS px scalar for sizing algorithms.
- `AspectRatio`: positive finite inline-size / block-size ratio.
- `SizeAxis`: logical inline or block axis.
- `AvailableSize`: definite or indefinite available space.
- `ConstraintSpace`: containing-block and available-size inputs for a box.
- `IntrinsicSizes`: min-content, max-content, preferred size, and ratio
  contributions.
- `AxisSizeConstraints` and `SizeConstraints`: min/max constraints with the
  invariant that min wins if min and max cross.
- `AppliedSizeConstraint`: records whether min, max, or neither changed the
  preferred size.
- `UsedAxisSize`: resolved content-box size for one axis, preserving both the
  preferred value/reason and the final value after constraints.
- `UsedContentSize`: resolved logical content-box size for both axes.
- `SizeResolutionReason`: deterministic explanation for a used-size result.

Later implementation may add resolver functions, debug labels, and richer value
types, but it must preserve this separation between inputs, intrinsic
contributions, constraints, and used outputs.

`ConstraintSpace` available sizes are content-box bases from the containing
formatting context. They are the basis for percentage resolution and line-width
selection. A box's own margins, borders, and padding are handled by used-size
resolution and flow placement, not by redefining the containing available size.

`UsedContentSize` is always the used logical content-box size. Padding, border,
and margin expansion belong to box-metrics conversion and flow placement, not
to this primitive.

## Width Resolution Responsibilities

The width resolver is responsible for the logical inline axis in the current
horizontal writing-mode subset.

For Milestone X, it must support:

- px `width`
- `width: auto` for in-flow block-level boxes as stretch-to-containing-block
  behavior after margins/padding are accounted for
- px `min-width` and px `max-width`
- percentage `width` once the CSS value model supports percentages, but only
  when the containing block inline size is definite
- replaced-element intrinsic width, fallback width, and aspect-ratio transfer
- atomic inline and inline-block shrink-to-fit behavior for the supported
  subset
- deterministic fallback for unsupported or indefinite cases

Width resolution must consume:

- computed style width/min/max values
- generated box role and formatting participation
- containing-block relationship
- definite or indefinite available inline size
- intrinsic contributions when content-based sizing is required
- box metrics that affect content-box versus border-box calculations

Width resolution must produce:

- used content inline size
- reason metadata suitable for deterministic debug snapshots
- no DOM, style, or box-tree mutations

## Height Resolution Responsibilities

The height resolver is responsible for the logical block axis in the current
horizontal writing-mode subset.

For Milestone X, it must support:

- px `height`
- `height: auto` as content-based block-size calculation for normal-flow boxes
- replaced-element intrinsic height, fallback height, and aspect-ratio transfer
- text line-box contribution to auto height
- form-control intrinsic/fallback heights
- future min/max-height extension points, but not full implementation until the
  CSS property model supports them

Height resolution must consume:

- computed style height value
- inline line layout results for inline formatting contexts
- child block-flow results for block formatting contexts
- replaced/form-control intrinsic dimensions
- definite available block size only when a supported rule explicitly requires
  it

Height resolution must not assume that viewport height is a definite containing
block height for ordinary document flow. Indefinite block-size behavior must be
represented explicitly.

## Intrinsic Sizing Responsibilities

Intrinsic sizing is a layout-owned artifact, not a property of DOM or CSS.

Supported intrinsic contributors for Milestone X are:

- text runs measured through `TextMeasurer`
- inline formatting content summarized as min-content and max-content inline
  contributions
- images with runtime intrinsic metadata or deterministic fallback size
- text inputs, textareas, checkboxes, radio buttons, and buttons with
  deterministic UA-like fallback metrics
- inline-block boxes summarized from their internal content for shrink-to-fit

Intrinsic sizing must expose logical contributions through `IntrinsicSizes`.
The collector may use existing replaced-element metadata, DOM attributes, text
measurement, and computed style, but the result must be a layout artifact that
can be debugged independently from DOM traversal.

Intrinsic sizing must not:

- depend on paint backend state
- use nondeterministic font fallback observations outside `TextMeasurer`
- mutate child layout boxes while merely collecting contributions
- silently clamp to available size unless the caller requested a used-size
  resolution

## Percentage Scope

Milestone X targets percentage sizing only where the basis is explicit and
definite.

In scope:

- percentage width against a definite containing-block inline size
- percentage min/max-width if the CSS value model adds those representations
  during the milestone
- deterministic deferral when the basis is indefinite

Out of scope for X1 and deferred unless a later X issue explicitly includes it:

- percentage height for ordinary auto-height document flow
- percentages involving writing modes other than the current horizontal subset
- percentage margins/padding if the CSS property model does not yet represent
  them
- cyclic percentage dependencies requiring multi-pass constraint solving
- viewport-percentage units

The contract rule is simple: a percentage may resolve only when the relevant
`AvailableSize` is `Definite`. Otherwise the resolver must either use the
specified CSS fallback for that property or return a deterministic deferred
reason.

## Min/Max Constraint Scope

Milestone X must centralize min/max constraint application.

In scope:

- px `min-width`
- px `max-width`
- min overriding max when constraints cross
- recomputing aspect-ratio-derived counterpart sizes when a constrained inline
  size changes and the opposite axis is auto
- debug visibility into whether a size came from the preferred result or a
  constraint

Deferred:

- `min-height` and `max-height` until the CSS property model supports them
- intrinsic keywords such as `min-content`, `max-content`, `fit-content`
- `box-sizing`
- margin auto distribution
- complete CSS 2.1 block formatting constraint equations

Constraint application belongs after preferred size selection for the axis. It
must not be duplicated in replaced-element, inline, or block layout helpers.
Constraint metadata must preserve the preferred value and preferred
`SizeResolutionReason`; min/max application is an operation after preferred-size
resolution, not a replacement for the original sizing reason.

## Shrink-To-Fit Scope

Shrink-to-fit behavior is required where a box's used inline size depends on
both available containing size and intrinsic content contributions.

For Milestone X, shrink-to-fit applies to:

- inline-block atomic inline boxes
- replaced inline boxes when their preferred size exceeds available inline
  space in the supported subset
- future block-level shrink-to-fit cases only when a later milestone introduces
  floats, absolute positioning, or similar formatting modes

The target formula for supported shrink-to-fit behavior is:

```text
min(max(min-content, available), max-content)
```

The exact caller may use a narrower equivalent only when the supported content
type cannot distinguish min-content from max-content yet, but the limitation
must be explicit in code and debug output.

## Flow-Correctness Contract

Normal flow must be size-driven and deterministic:

- A block container's child available inline size is derived from the
  container's resolved content inline size, not from the original viewport
  width.
- Inline formatting uses the containing block content inline size as the line
  width basis.
- Anonymous block boxes participate as real generated boxes and receive their
  own constraint spaces.
- Inline-blocks are atomic in the parent inline formatting context and establish
  internal sizing/layout for descendants.
- Auto block sizes are content-derived after children or line boxes are laid
  out.
- Replaced elements expose intrinsic dimensions before line placement consumes
  them.
- Layout must not infer containing blocks by walking DOM parents; it must use
  generated containing-block metadata.

The layout pass may remain recursive while Milestone X is implemented, but each
recursive step must carry a typed sizing environment. Passing raw viewport width
to every descendant is a transitional behavior, not the architecture target.

## Determinism And Debug Expectations

Sizing decisions must be deterministic for a fixed:

- DOM tree and attributes
- computed style tree
- generated box tree
- viewport/layout environment
- text measurement implementation
- replaced-element metadata provider

Debug surfaces must expose enough information to identify sizing regressions:

- input available sizes and whether they are definite or indefinite
- containing-block identity used as percentage basis
- intrinsic contributions for supported content types
- preferred size before constraints where meaningful
- preferred `SizeResolutionReason`
- applied min/max constraints
- final used content size after constraints

The existing `LayoutPhaseInput::to_debug_snapshot`,
`BoxTree::to_debug_snapshot`, and `LayoutPhaseOutput::to_debug_snapshot`
remain the deterministic regression surfaces. Later X issues should extend
them deliberately rather than creating backend-specific pixel snapshots.

## Deferred Work

X1 intentionally does not implement:

- complete CSS 2.1 block width equations
- margin collapsing
- floats
- absolute/fixed/sticky positioning
- flex, grid, table, or fragmentation sizing
- writing modes beyond the current horizontal model
- `box-sizing`
- borders as part of sizing
- percentage height in ordinary auto-height flow
- viewport units
- intrinsic sizing keywords in CSS values
- retained layout caches or incremental constraint invalidation
- full scroll/overflow sizing semantics

These features must extend `ConstraintSpace`, intrinsic contribution
collection, containing-block rules, formatting-context rules, debug snapshots,
and tests explicitly when implemented.

## Milestone X1 Exit Contract

X1 is complete while these conditions remain true:

- sizing architecture is documented in this file
- code-level sizing contract types exist in `crates/layout/src/sizing.rs`
- width/height resolution responsibilities are explicit
- intrinsic sizing responsibilities are explicit
- min/max, percentage, and shrink-to-fit scope is unambiguous
- flow-correctness expectations are tied to containing-block and formatting
  metadata from Milestone W
- non-goals are named instead of hidden in current implementation behavior
- future Milestone X issues have a single sizing model to implement against
