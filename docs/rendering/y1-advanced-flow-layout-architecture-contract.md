# Y1: Advanced Flow Layout Architecture Contract

Last updated: 2026-05-09  
Status: implemented architecture contract for Milestone Y issue 1

This document is the source-of-truth contract for Milestone Y1. It defines how
Borrowser will extend its current normal-flow layout engine with margin
collapsing, overflow semantics, positioned containing blocks, and out-of-flow
layout participation without turning each feature into an isolated patch.

Related code:

- `crates/layout/src/lib.rs`
- `crates/layout/src/flow.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/debug.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/css/src/computed/style.rs`

Related documents:

- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`

## Purpose

Milestones W and X made box generation, formatting participation, containing
size, available space, and used sizing explicit. Milestone Y builds on that
foundation by defining the advanced flow concepts that real pages rely on:

- vertical margin handling and supported margin collapsing
- overflow behavior split between layout containment and paint clipping
- containing-block lookup for positioned descendants
- out-of-flow participation boundaries for positioned boxes

Y1 is an architecture issue. It introduces contracts and code-level vocabulary,
not parser support for new CSS properties and not complete positioned layout.
Later Y issues must extend these contracts deliberately.

The intended shape is:

```text
ComputedStyle + BoxTree metadata + layout environment
  -> flow contract inputs
  -> margin resolution and collapse decisions
  -> overflow layout effects and paint clip metadata
  -> containing-block lookup for normal-flow and positioned descendants
  -> in-flow layout plus queued out-of-flow layout work
  -> LayoutBox geometry and deterministic debug snapshots
  -> paint with explicit clip/overflow inputs
```

## Current Baseline

The current layout engine already has a useful normal-flow subset:

- generated boxes carry `BoxId`, `ContainingBlockId`, block formatting context,
  inline formatting context, and participation metadata
- the sizing model separates `ContainingSize` from `AvailableSpace`
- block-flow placement accounts for physical margins when placing in-flow
  children
- inline layout accounts for atomic inline margin boxes
- paint consumes `LayoutBox` geometry and does not create layout boxes

Y work must not replace those foundations with DOM traversal shortcuts. It must
extend the generated-box, sizing, and flow contracts.

## Margin Contract

Margins are flow-placement inputs. They do not change a box's used content
size, and they must not be folded into `UsedContentSize`.

Supported Y scope:

- finite px margins from computed style
- negative margins through `SignedCssPx`
- horizontal margins narrowing `AvailableSpace` without changing
  `ContainingSize`
- vertical margins contributing to block-axis placement
- adjoining vertical margin collapse for supported normal-flow block cases

Supported collapse categories are named by `MarginCollapseCase`. These names do
not by themselves authorize collapse. The implementation must still validate
all relevant adjoining-margin preconditions and boundaries before producing a
collapse decision.

- adjacent block siblings
- parent block-start with first in-flow child
- parent block-end with last in-flow child
- empty block self-collapse

For parent/child and empty-block collapse, the case only applies when the
parent or box has no boundary that separates the margins, such as non-zero
padding or border, inline formatting content, clearance, out-of-flow
participation, or an independent formatting context. Parent block-end collapse
is additionally limited to the supported auto block-size subset; fixed or
min-constrained block-size interactions are deferred until those sizing
features exist.

The collapse value for one adjoining set follows the CSS positive/negative
rule implemented by `CollapsedMargin::from_adjoining(...)`:

```text
collapsed = largest positive margin + most negative margin
```

If all margins are positive, the largest positive margin wins. If all margins
are negative, the most negative margin wins. Missing positive or negative
groups contribute zero.

Collapse boundaries are named by `MarginCollapseBoundary` and include:

- root element
- independent formatting context
- out-of-flow participation
- inline formatting content
- non-zero padding or border
- clearance
- overflow-created formatting context
- fragmentation

Later implementation must record collapse decisions separately from size
resolution metadata. Sizing debug output may show margin inputs, but margin
collapse debug output must explain which adjoining set collapsed or which
boundary prevented collapse.

## Overflow Contract

Overflow has separate layout and paint responsibilities.

Layout owns:

- mapping canonical computed overflow values to `OverflowPolicy`
- deciding whether overflow creates an independent formatting context
- preserving used sizes; overflow must not silently resize content to avoid
  clipping
- exposing scroll-container or clipping metadata for later phases

CSS owns overflow shorthand and longhand parsing plus computed-value
normalization, including later axis-coupling rules. Layout must not duplicate
those CSS computed-value rules; it consumes the canonical result and turns it
into layout effects.

Paint owns:

- applying overflow clip rectangles supplied by layout
- clipping descendants according to the layout-owned policy
- eventually painting scrollable overflow according to scroll state

Paint must not infer overflow behavior by inspecting CSS directly. Layout must
hand paint explicit clip/scroll metadata once overflow is integrated into
`LayoutBox` or a future fragment/display-list structure.

Y1 defines these keyword effects:

```text
visible: no paint clip, no scroll container, no independent BFC
hidden:  paint clip, scroll-container semantics, independent BFC
clip:    paint clip, no scroll container, no independent BFC
scroll:  paint clip, scroll-container semantics, independent BFC
auto:    paint clip, scroll-container semantics, independent BFC
```

Axis-specific computed overflow interaction remains CSS-owned and is deferred
until the CSS property model supports the relevant properties. Until then,
`OverflowPolicy` models canonical inline and block axes explicitly so later
writing-mode and axis behavior has a place to land without making layout a
second CSS engine.

## Positioning And Containing Blocks

Normal-flow containing blocks remain the W5 generated-box relationship. Y adds
the positioned containing-block contract without replacing that relationship.

`PositioningScheme` defines the supported architecture:

- `Static` participates in normal flow and does not establish a positioned
  containing block.
- `Relative` participates in normal flow, establishes a positioned containing
  block for descendants, and will later apply layout-owned visual offsets
  without changing normal-flow contribution.
- `Absolute` is out of flow, establishes a positioned containing block for its
  descendants, and resolves against the nearest positioned ancestor with the
  initial containing block as fallback.
- `Fixed` is out of flow, establishes a positioned containing block for its
  descendants, and resolves against the initial containing block in the current
  supported subset.
- `Sticky` participates in normal flow, establishes a positioned containing
  block for descendants, and defers scroll-dependent offsets.

The existing `ContainingBlockId` remains frame-local generated-box identity.
Later positioned layout must add explicit lookup metadata or resolved IDs for
positioned containing blocks. It must not rediscover this relationship in paint
or hit testing.

## Out-Of-Flow Contract

Out-of-flow boxes are generated boxes with normal source identity and style,
but they do not contribute to the parent normal-flow auto size, block stacking,
or inline line building.

Supported Y1 out-of-flow families are:

- absolutely positioned boxes
- fixed positioned boxes

The normal-flow pass should eventually produce:

- ordinary in-flow geometry for in-flow descendants
- static-position information for out-of-flow descendants when needed
- an ordered out-of-flow work list tied to generated `BoxId`
- enough containing-block data for the positioned layout pass

Out-of-flow layout must still use the sizing resolver. Insets, shrink-to-fit
behavior, and available space should become structured inputs to sizing rather
than local width/height calculations inside a positioning pass.

## Phase Responsibilities

CSS owns parsed and computed property values. CSS may eventually expose
computed `position`, `overflow-x`, and `overflow-y`, but it must not decide
containing blocks, margin collapse, out-of-flow queues, or paint clipping.

Box generation owns generated identity, source relationships, display-derived
box roles, and baseline containing-block/formatting-context metadata. It may
record style-derived flow classification once the relevant computed properties
exist, but it must not compute used geometry.

Normal-flow layout owns in-flow placement, margin application, margin collapse,
static-position capture, and out-of-flow queue construction. It consumes
resolver output; it does not reimplement sizing rules.

Positioned layout owns final geometry for out-of-flow positioned boxes. It must
consume generated-box identity, positioned containing-block relationships,
static-position data, style insets once supported, and the shared sizing model.

Paint owns drawing and clipping from layout-provided geometry. It must not
create layout boxes, resolve CSS overflow, or choose containing blocks.

## Determinism And Debug

Advanced flow behavior must be deterministic for a fixed DOM, computed style
tree, viewport, text measurer, and replaced-element metadata.

Required invariants:

- generated `BoxId` remains the identity used by flow metadata
- margin collapse decisions are recorded in stable tree order
- collapsed margin values use fixed positive/negative rules
- overflow layout effects are separated from paint clipping effects
- positioned containing-block lookup is represented as layout data
- out-of-flow work lists are ordered by generated tree order unless a later
  stacking contract explicitly says otherwise
- debug output describes semantic flow decisions, not backend pixel output

Y1 introduces `advanced_flow_contract_debug_snapshot()` as a stable contract
surface for the named flow primitives. Later implementation must add
layout-output snapshots for actual margin collapse, overflow clips, positioned
containing blocks, and out-of-flow queues.

## Deferred Work

Y1 deliberately does not implement:

- parsing or computed-style storage for `position`, `overflow`, or inset
  properties
- full CSS 2.1 block width equations
- `margin-left/right: auto`
- percentage margins or padding
- borders and `box-sizing`
- floats and clearance behavior beyond naming clearance as a collapse boundary
- final absolute, fixed, relative, or sticky positioning
- scroll state, scrollbars, or scroll hit testing
- transforms establishing containing blocks
- stacking contexts, z-index, or full paint-order changes
- fragmentation and writing-mode-specific margin/overflow behavior

These are extension points on top of the Y1 contracts.

## Close-Out Criteria For Y1

Y1 is satisfied while these conditions hold:

- margin handling and supported collapse cases are explicitly defined
- overflow layout responsibilities are separated from paint responsibilities
- positioned containing-block strategy is named and deterministic
- out-of-flow participation boundaries are explicit
- non-goals and deferred work are unambiguous
- code-level flow primitives expose deterministic debug labels and tests
- later Y issues can integrate behavior through these contracts instead of
  adding feature-specific shortcuts
