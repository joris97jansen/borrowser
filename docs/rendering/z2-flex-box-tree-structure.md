# Z2: Flex Box-Tree Structure

Last updated: 2026-06-03
Status: implemented structural contract for Milestone Z issue 2

This document records Borrowser's first structural flex layout support. Z2
does not implement the flex layout algorithm. It introduces the layout-owned
box-tree and layout-projection vocabulary that later flex algorithm work must
consume.

Related code:

- `crates/css/src/values.rs`
- `crates/css/src/specified/display.rs`
- `crates/css/src/specified/value.rs`
- `crates/css/src/computed/normalize.rs`
- `crates/layout/src/box_tree/display.rs`
- `crates/layout/src/box_tree/formatting.rs`
- `crates/layout/src/box_tree/model.rs`
- `crates/layout/src/box_tree/builder.rs`
- `crates/layout/src/box_tree/debug.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/document.rs`
- `crates/layout/src/debug.rs`

Related documents:

- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/y6-out-of-flow-layout-participation-groundwork.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`

## Implemented Support

CSS now accepts `display: flex` as a supported display keyword and preserves it
through specified-value parsing, computed-value normalization, and boundary
debug serialization as `css::Display::Flex`.

Layout maps computed `Display::Flex` on ordinary generated element boxes to:

- `DisplayBoxBehavior::FlexContainer`
- `FormattingContextKind::Flex`
- a block-level participant in the parent formatting context for the current
  supported subset

A flex container is a generated principal box. It is not a DOM node, a CSS
rule, or a paint primitive.

## Flex Item Participation

Flex item participation is layout-owned metadata on generated boxes:

```text
FlexFormattingParticipation::None
FlexFormattingParticipation::FlexItem
```

A generated box becomes a flex item when:

- its generated parent box has `DisplayBoxBehavior::FlexContainer`
- the child is a direct generated child of that flex container
- the child participates in normal flow

Out-of-flow generated boxes remain present in the box tree with their
positioning and containing-block metadata, but they do not become flex items.

Nested flex containers are represented by two independent facts:

- the nested container can be a `FlexItem` of its parent flex container
- the nested container can establish its own `FormattingContextKind::Flex` for
  its direct generated in-flow children

Direct generated text or inline boxes may carry flex-item metadata when box
generation has produced them as direct in-flow children of a flex container.
Z2 does not add new anonymous flex item generation rules beyond the existing
box-generation pipeline.

## Projection And Debug Surfaces

`BoxNode` records flex participation before geometry is computed. `LayoutBox`
projects the same metadata so later flex layout work can consume the structure
without re-inspecting DOM shape or CSS declarations.

The deterministic debug surfaces expose:

- computed `display=flex`
- `behavior=flex-container`
- `establishes-fc=flex`
- `flex-participation=flex-item`

These debug labels are regression contracts for structure only. They do not
claim that flex geometry has been computed.

## Ownership Boundaries

CSS owns:

- parsing `display: flex`
- cascade and computed display output
- display keyword serialization

Layout owns:

- mapping computed display to generated box behavior
- assigning flex formatting contexts
- deriving flex item participation from generated box-tree children
- projecting flex metadata into `LayoutBox`

Paint and browser/runtime orchestration must not infer flex behavior from DOM
shape, raw CSS declarations, or computed display. They must continue to consume
layout output.

## Deliberate Exclusions

Z2 does not implement:

- flex line construction
- `flex-direction`
- `flex-wrap`
- `flex-grow`, `flex-shrink`, or `flex-basis` algorithms
- `justify-content`
- `align-items` or `align-self`
- `order`
- min/max flex sizing behavior
- main-axis or cross-axis placement
- flex-specific paint behavior
- runtime invalidation or retained-state changes

Unsupported flex features must remain explicit in later contracts and tests.
They must not be approximated through block layout special cases.

## Invariants

- Flex containers and flex items are layout-owned generated-box concepts.
- Flex items are derived from generated box-tree children, not DOM children.
- Out-of-flow boxes do not participate as flex items.
- Existing block, inline, list-item, anonymous, replaced, containing-block,
  positioned-containing-block, and debug contracts remain active.
- Z2 records structure only; it does not compute flex geometry.
- Later flex algorithm issues must consume this structure instead of adding
  geometry-time DOM or CSS shortcuts.
