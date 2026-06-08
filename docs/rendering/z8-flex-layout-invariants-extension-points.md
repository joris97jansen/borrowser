# Z8: Flex Layout Invariants And Extension Points

Last updated: 2026-06-08
Status: final flex layout contract for Milestone Z issue 8

This document consolidates Borrowser's Milestone Z flex layout contract. It
does not add flexbox behavior. It records the supported subset, subsystem
ownership boundaries, invariants, limitations, and extension points that future
flex work must preserve.

Related code:

- `crates/css/src/values.rs`
- `crates/css/src/specified/display.rs`
- `crates/css/src/computed/normalize.rs`
- `crates/css/src/properties/data.rs`
- `crates/layout/src/box_tree/display.rs`
- `crates/layout/src/box_tree/formatting.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/flex.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/sizing.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/debug.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/z3-flex-main-axis-layout-core-subset.md`
- `docs/rendering/z4-flex-cross-axis-layout-core-subset.md`
- `docs/rendering/z5-flex-layout-integration-hardening.md`
- `docs/rendering/z6-flex-unsupported-feature-handling.md`
- `docs/rendering/z7-deterministic-flex-debug-regressions.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`
- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Current Supported Subset

Milestone Z supports a deliberately scoped flex subset:

- `display: flex`
- block-level flex containers
- generated direct in-flow children as flex items
- nested flex containers as independent flex formatting contexts
- row-only main-axis layout
- single-line layout
- physical block axis as the cross axis
- deterministic generated child order
- internal default `flex-grow: 0`
- internal default `flex-shrink: 1`
- internal `flex-basis: auto`
- start packing when positive free space remains
- proportional shrink when negative free space is present
- default stretch for auto-height items in the supported row subset
- preservation of explicit item heights
- exclusion of out-of-flow children from flex distribution
- integration with the existing sizing, flow, positioning metadata, and debug
  contracts

The only authored flex-related CSS surface in this subset is `display: flex`.
Authored flex properties such as `flex-direction`, `flex-wrap`,
`justify-content`, `align-items`, `align-self`, `gap`, `order`, and
`flex-basis` are not supported CSS properties yet.

## Ownership Boundaries

CSS owns authored syntax, cascade, computed values, and unsupported CSS
filtering. For Milestone Z, CSS accepts `display: flex` and rejects or ignores
unsupported flex-related values and properties before they can affect computed
style.

CSS must not:

- decide generated flex container or flex item identity
- construct flex lines
- resolve flex item used sizes
- distribute main-axis free space
- compute cross-axis sizes or offsets
- make paint or runtime decisions from flex declarations

Layout owns flex semantics after computed style and generated box-tree
metadata exist. Layout maps computed `Display::Flex` to
`DisplayBoxBehavior::FlexContainer`, assigns `FormattingContextKind::Flex`,
derives `FlexFormattingParticipation::FlexItem`, resolves main-axis and
cross-axis layout for the supported subset, writes final `LayoutBox` geometry,
and retains deterministic flex metadata for debug output.

Layout must not:

- walk DOM children to rediscover flex items during geometry
- inspect raw CSS declarations
- locally reimplement sizing constraints owned by the sizing subsystem
- approximate unsupported flex features with block-flow patches
- rely on paint or runtime to repair missing flex behavior

Paint consumes final layout geometry, overflow clips, text fragments, and
replaced-element geometry. Paint must not inspect raw CSS, DOM shape, or flex
debug metadata to infer flex behavior.

Browser/runtime orchestration owns invalidation, retained style artifacts,
resources, and frame lifecycle. It must not emulate flex layout or infer flex
semantics from DOM, CSS declarations, or final rectangles.

## Flex Container And Item Invariants

A flex container is a generated principal layout box with
`DisplayBoxBehavior::FlexContainer` and `FormattingContextKind::Flex`. The
role belongs to the generated box, not to the DOM node, CSS rule, paint
primitive, or runtime object.

A flex item is an eligible generated in-flow child box of a flex container.
Flex item identity is determined after display suppression, generated-box
normalization, replaced-element classification, anonymous box generation, list
marker handling, flow participation, and containing-block metadata have been
resolved by the existing layout pipeline.

Required invariants:

- flex item participation is generated layout identity
- direct generated in-flow children of a flex container become flex items
- out-of-flow generated children do not become flex items
- nested flex containers may be flex items of an outer flex container
- nested flex containers establish their own independent flex formatting
  context for their generated in-flow children
- generated child order is the flex item order for the current subset
- anonymous and replaced boxes can participate only through generated box-tree
  rules, not through DOM shortcuts
- box-tree debug surfaces and layout debug surfaces expose flex metadata as
  internal regression contracts

## Main-Axis Invariants

The Milestone Z main axis is `FlexMainAxis::Row`. For this subset, the main
axis maps to the physical inline axis.

The main-axis algorithm consumes layout-owned `FlexItemMainAxisInput` values
constructed from generated flex items. Production layout supplies internal
defaults equivalent to grow `0`, shrink `1`, and auto basis for every eligible
flex item. These defaults are not authored CSS support.

Required invariants:

- main-axis layout consumes generated flex item participation metadata
- item order follows deterministic generated child order
- basis resolution routes through the existing sizing and intrinsic sizing
  machinery
- auto flex item basis uses the flex item main-axis sizing mode rather than
  block-level stretch behavior
- min/max constraints remain centralized in the sizing subsystem
- positive free space with zero grow factors remains at the end of the line
  through start packing
- negative free space uses proportional shrink with scaled shrink factors
- zero shrink denominator produces deterministic no-shrink behavior
- final item main-axis offset and width are written to `LayoutBox`
- main-axis debug metadata records retained typed decisions, not
  rectangle-inferred conclusions

## Cross-Axis Invariants

The Milestone Z cross axis is `FlexCrossAxis::Block`. For this subset, the
cross axis maps to the physical block axis.

The cross-axis algorithm consumes layout-owned `FlexItemCrossAxisInput` values
after main-axis sizing has produced item widths. It computes hypothetical item
cross sizes, line cross size, container used cross size, stretch target sizes,
and deterministic offsets for the supported row-only, single-line subset.

Required invariants:

- cross-axis layout consumes generated flex item metadata and sizing inputs
- auto container cross size comes from the maximum item outer hypothetical
  cross size
- explicit container height becomes available cross size when it resolves to a
  definite content block size
- default stretch applies only to auto-height items
- explicit item heights remain explicit
- stretch target sizes route through the sizing subsystem before final
  geometry is assigned
- out-of-flow descendants do not contribute to cross-axis sizing or placement
- final item cross-axis offset and height are written to `LayoutBox`
- cross-axis debug metadata records retained typed decisions, not
  rectangle-inferred conclusions

## Sizing, Flow, And Positioning Invariants

Flex layout is integrated with the W box-tree, X sizing, and Y advanced-flow
contracts. It is not a standalone geometry pass.

Required sizing invariants:

- flex basis and distributed sizes use `SizeResolutionInput`
- distributed inline sizes use the centralized flex distributed inline sizing
  path
- distributed block sizes for stretch use the centralized flex distributed
  block sizing path
- padding expansion, content-box versus border-box conversion, used content
  size, and min/max clamping remain sizing-owned
- percentages preserve the distinction between containing size and available
  space
- item margins can narrow available space for auto and shrink-to-fit behavior
  but must not change the containing-size basis for percentage resolution

Required flow and positioning invariants:

- flex containers participate in their parent formatting context as
  block-level normal-flow boxes for the current subset
- flex containers produce their auto block size through flex cross-axis layout
  before parent block-flow placement consumes it
- in-flow flex items participate in flex distribution
- out-of-flow generated children remain present in the generated and projected
  layout trees
- out-of-flow generated children keep source identity, flow participation,
  containing-block, and positioned-containing-block metadata
- positioned descendants inside flex containers are tracked through generated
  `BoxId` and `PositionedContainingBlockId` metadata, not DOM ancestry
- Milestone Z does not complete absolute or fixed positioning geometry

## Debug And Regression Invariants

Flex debug output is an internal deterministic regression contract.

Algorithm-level tests use:

- `FlexMainAxisLayout::to_debug_snapshot()`
- `FlexCrossAxisLayout::to_debug_snapshot()`

Integrated layout tests use:

- `LayoutPhaseOutput::to_flex_debug_snapshot()`

Required debug invariants:

- debug output serializes retained flex metadata from `LayoutBox`
- debug output does not reconstruct flex decisions from final rectangles
- generated layout traversal order is deterministic
- flex item indexes follow direct generated child order
- `BoxId` and source labels use existing layout debug helpers
- CSS pixel formatting remains stable
- unsupported flex declarations do not appear as layout decisions
- debug snapshots are not public APIs

Changing a flex debug format is an intentional contract change. It requires
updated tests and updated rendering docs.

## Unsupported Features And Limitations

Unsupported flex features remain explicit follow-up work. They must not be
approximated through CSS parser shortcuts, layout special cases, paint-time
inference, or browser/runtime emulation.

Milestone Z does not implement:

- `display: inline-flex`
- authored `flex-direction`
- column or reverse flex axes
- authored `flex-wrap` or multi-line layout
- authored `flex-flow`
- authored `justify-content`
- authored `align-items`, `align-self`, or `align-content`
- baseline alignment
- authored `gap`, `row-gap`, or `column-gap`
- `order`
- authored `flex`, `flex-grow`, `flex-shrink`, or `flex-basis`
- full browser-compatible min-content and max-content flexbox nuance
- full absolute or fixed positioning geometry inside flex containers
- relative visual offsets
- flex-specific paint behavior
- browser/runtime flex behavior

Unsupported authored flex property names are ignored by the CSS property
registry and cascade. Unsupported display values such as `inline-flex` do not
create deferred or approximate layout modes. Layout's internal typed hooks for
future axes, factors, alignment, and stretch are not authored CSS support until
CSS parsing, cascade, computed style, layout adaptation, tests, and docs are
extended together.

## Extension Points

Future flex work must extend the existing subsystem contracts instead of
bypassing them.

### Authored Flex Properties

Support for `flex-grow`, `flex-shrink`, `flex-basis`, `flex`, `flex-direction`,
`justify-content`, alignment properties, gaps, or `order` must start in CSS:
property registration, parsing, cascade, computed-value normalization,
serialization, and unsupported-value handling. Layout may consume those values
only after they are represented in computed style.

### Column And Reverse Axes

Column, row-reverse, and column-reverse support must extend the typed axis
model and the mapping between logical flex axes and physical geometry. The
work must update main-axis layout, cross-axis layout, sizing inputs, debug
snapshots, and regression coverage together.

### Wrapping And Multi-Line Layout

Wrapping must introduce explicit flex line construction and line metadata.
Line breaking, per-line main-axis distribution, cross-line sizing, and
`align-content` behavior must be layout-owned data structures, not inferred
from child rectangles after placement.

### Gaps

Gap support must enter through CSS computed values and layout-owned spacing
inputs. Gaps must participate in main-axis free-space calculation, wrapping,
cross-line sizing, debug output, and tests. They must not be synthesized as
margins or anonymous spacer boxes unless a future contract explicitly defines
that representation.

### Order

`order` support must preserve generated box identity while introducing a
deterministic flex ordering phase. Debug output must distinguish generated
tree order from used flex order, and paint/input behavior must continue to
consume layout-owned ordering metadata rather than raw CSS declarations.

### Advanced Alignment

Authored main-axis and cross-axis alignment must extend typed layout inputs and
debug metadata. Baseline alignment requires explicit baseline data from inline
layout and replaced-element layout before flex layout can consume it.

### Inline Flex Containers

`inline-flex` requires the display model to represent outer and inner display
roles explicitly. It must integrate with inline formatting participation,
atomic inline sizing, baseline behavior, and paint/input geometry before it is
accepted as a supported display value.

### Positioned Layout

Future absolute, fixed, and relative positioning work inside flex containers
must consume the existing generated containing-block and positioned
containing-block metadata. It must not rediscover relationships through DOM
ancestry after flex layout has run.

### Debug Coverage

Every new flex feature must add deterministic debug coverage for the typed
decisions it introduces. Broad layout snapshots can prove geometry, but flex
debug snapshots should continue to expose the flex-specific retained metadata
needed to diagnose distribution, alignment, line construction, and ordering.

## Milestone Z Close Contract

Milestone Z is closeable from a flex layout contract perspective while these
conditions remain true:

- the supported subset is documented as block-level, row-only, single-line
  `display: flex`
- unsupported flex features are explicit and are not approximated
- CSS, layout, paint, and browser/runtime ownership boundaries remain intact
- flex container and flex item identity remain generated layout concepts
- sizing, flow, positioning, and debug behavior stay integrated with existing
  contracts
- deterministic regression surfaces cover representative flex structure,
  algorithm, integration, unsupported-feature, and debug behavior
- future flex expansion has explicit extension points instead of relying on
  ad-hoc patches
