# X2: Structured Size-Resolution Model And Sizing Inputs

Last updated: 2026-05-05  
Status: implemented structured sizing inputs for Milestone X issue 2

This document defines the size-resolution input model introduced by X2. X1
established the architecture and ownership contract; X2 makes the inputs that a
future resolver consumes explicit, typed, and testable.

Related code:
- `crates/layout/src/sizing.rs`
- `crates/layout/src/lib.rs`
- `crates/css/src/computed/style.rs`
- `crates/css/src/values.rs`

Related documents:
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Purpose

X2 moves Borrowser toward a resolver shape where size decisions are made from a
single structured input object rather than from incidental layout traversal
state. The model separates:

- containing block size used as a percentage basis
- available space offered by the formatting context
- style-provided preferred/min/max sizes
- style box metrics needed by content-box resolution
- intrinsic content contributions

The current layout pass does not yet consume `SizeResolutionInput`. Later X
issues should migrate concrete width, height, intrinsic, percentage, and
shrink-to-fit behavior to this input model incrementally.

## Input Model

The X2 input stack is:

```text
ContainingSize
AvailableSpace
  -> ConstraintSpace

ComputedStyle
  -> StyleSizeInputs

IntrinsicSizes

ConstraintSpace + StyleSizeInputs + IntrinsicSizes
  -> SizeResolutionInput
```

`SizeResolutionInput` is the intended resolver handoff for one generated box.
It is deliberately independent from DOM identity and paint state.

## Containing Size And Available Space

`ContainingSize` records the containing block content-box dimensions:

- `containing_block`
- inline size
- block size

`AvailableSpace` records the content-box space currently offered by the
formatting context. It may match `ContainingSize` for simple normal flow, but
it is separate because later layout modes can narrow available space without
changing the percentage basis.

Examples where they may diverge later:

- floats reducing line or block available space
- multicol/fragmentation constraints
- positioned or shrink-to-fit formatting modes
- scroll/overflow constraints

`ConstraintSpace` combines both concepts. The existing `ConstraintSpace::new`
constructor remains a normal-flow convenience where containing size and
available space are initially the same.

## Style Size Inputs

`StyleSizeInputs` is a deterministic materialization of the style fields used by
sizing. It is not a second style system and must remain a lossless layout-facing
view over computed style.

It contains:

- inline-axis `AxisStyleSizeInput`
- block-axis `AxisStyleSizeInput`
- `StyleBoxMetrics`

Each `AxisStyleSizeInput` separates:

- `StylePreferredSize`: `auto`, length, or future percentage
- `StyleMinimumSize`: `auto`, length, or future percentage
- `StyleMaximumSize`: `none`, length, or future percentage

For the current supported CSS property subset:

- inline preferred size comes from `width`
- block preferred size comes from `height`
- inline minimum size comes from `min-width`
- inline maximum size comes from `max-width`
- block min/max are explicit extension points and currently materialize as
  `auto`/`none`

`StyleBoxMetrics` separates signed margins from non-negative padding:

- margins use `SignedCssPx` because negative margins are valid CSS input
- padding uses `CssPx` because padding must be non-negative

This prevents future resolvers from mixing signed style lengths with
non-negative used size values.

## Percentages

`Percentage` is represented as a non-negative finite fraction:

- `1.0` means 100%
- `0.5` means 50%

CSS parsing does not yet produce percentage sizing values, but X2 can represent
them so later percentage support does not require reshaping resolver inputs.

Percentage resolution must use `ContainingSize` when the relevant basis is
definite. Resolvers must not accidentally use narrowed `AvailableSpace` as the
percentage basis unless a specific CSS rule explicitly requires available space.
If the containing basis is indefinite, the resolver must preserve a
deterministic deferred result according to the X1 contract.

## Validation

`StyleSizeInputs::from_computed_style` validates the computed style handoff
before resolver use:

- width/height/min-width/max-width must be non-negative finite CSS px when
  present
- padding must be non-negative finite CSS px
- margins must be finite CSS px and may be negative
- negative zero is normalized by scalar input types

Computed style should already enforce these invariants. X2 still validates at
the layout boundary because sizing inputs are a phase contract, and deterministic
validation errors make future regressions easier to diagnose.

## Determinism And Tests

The regression surface for X2 is unit coverage in `crates/layout/src/sizing.rs`.
Tests cover:

- signed and non-negative scalar behavior
- percentage resolution only against definite bases
- separating containing size from available space
- validating signed margins and non-negative padding
- rejecting invalid non-negative sizing lengths
- rejecting non-finite margins
- materializing current computed width/height/min-width/max-width inputs
- preserving `SizeResolutionInput` components

These tests intentionally exercise inputs, not final layout behavior. Concrete
used-size algorithms belong to later X issues.

## Deferred Work

X2 intentionally does not implement:

- width or height resolution algorithms
- applying `StyleSizeInputs` to `LayoutBox` geometry
- CSS percentage parsing
- min-height or max-height computed properties
- margin auto behavior
- border and `box-sizing` inputs
- writing-mode-specific logical/physical mapping
- flex, grid, table, positioned, float, or fragmentation sizing modes

Those features must extend the input model deliberately instead of adding
resolver-only side channels.

## Exit Contract

X2 is complete while these remain true:

- size-resolution input structures exist in code
- containing size and available space are represented explicitly
- style-driven sizing inputs are represented explicitly
- inputs can be materialized deterministically from current computed style
- intrinsic contributions remain part of the resolver input bundle
- tests cover representative sizing input behavior
