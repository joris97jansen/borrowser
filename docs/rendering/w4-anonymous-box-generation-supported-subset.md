# W4: Anonymous Box Generation For The Supported Subset

Last updated: 2026-05-04  
Status: implemented anonymous block generation for Milestone W issue 4

This document defines the anonymous box generation rules currently supported by
Borrowser's layout-owned `BoxTree` generation step.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/tokens.rs`
- `crates/gfx/src/paint/mod.rs`
- `crates/gfx/src/paint/inline.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W4 makes anonymous boxes explicit generated `BoxNode`s instead of letting mixed
block/inline behavior emerge from DOM traversal. Anonymous boxes are generated
inside the box tree, before geometry projection:

```text
StyledNode
  -> DisplayBoxGeneration
  -> principal BoxNode stream
  -> anonymous box normalization
  -> BoxTree
  -> LayoutBox geometry projection
```

## Supported Rule

For the current supported subset, Borrowser generates anonymous block boxes for
mixed block-level and inline-level direct children of a block container.

The rule is:

- If a supported block container has at least one block-level generated direct
  child and at least one inline-level generated direct child, each contiguous
  inline-level child run is wrapped in an anonymous block box.
- If all generated direct children are inline-level, no anonymous block is
  generated yet; the existing inline formatting path owns that content.
- If all generated direct children are block-level, no anonymous block is
  generated.
- Suppressed children, such as comments and `display: none` subtrees, do not
  force runs to split because they do not generate boxes.

Inline-level children for this rule are:

- `DisplayBoxBehavior::Inline`
- `DisplayBoxBehavior::InlineBlock`
- `DisplayBoxBehavior::ReplacedInline`
- `DisplayBoxBehavior::TextRun`

Block-level children are generated principal boxes whose display behavior does
not participate inline under the current subset.

## Representation

Anonymous block boxes are represented as ordinary generated `BoxNode`s with:

- `BoxGenerationRole::Anonymous(AnonymousBoxKind::Block)`
- `BoxKind::Block`
- `DisplayBoxBehavior::Anonymous`
- `BoxSource::Anonymous { parent, kind: AnonymousBoxKind::Block }`
- no direct DOM node ID
- the parent styled node as a source anchor for inherited style and current
  downstream compatibility

The generated box has `display: block` metadata because it participates as a
block box. The source anchor remains separate from direct DOM identity, so
debug output can distinguish anonymous boxes from DOM-backed boxes.

## Geometry And Paint Integration

`LayoutBox` now carries both:

- `source`, the generated box source
- `node`, a temporary anchor `StyledNode` for bridge compatibility

Anonymous layout boxes are real layout participants. They no longer require a
direct `StyledNode` and no longer panic during `BoxTree` to `LayoutBox`
projection.

Anonymous boxes use the anchor style for inherited text/style data, but their
own box metrics are neutral in geometry and hit-testing. This prevents parent
margin and padding from being applied a second time. Paint also skips anonymous
box backgrounds and list markers; children and inline content remain paintable.

## Determinism

Anonymous boxes are inserted deterministically during child generation:

- `BoxId`s are still assigned in preorder.
- Child order is preserved.
- Each inline run receives exactly one anonymous block wrapper.
- Parent/child links point to the generated anonymous wrapper, not directly to
  the original block container.

`BoxTree::to_debug_snapshot()` exposes anonymous boxes with `source-id=none`,
the anchor source label, role `anonymous-block`, kind `block`, display `block`,
and behavior `anonymous`.

## Deferred Work

W4 intentionally does not implement:

- anonymous inline boxes
- anonymous table boxes
- independent marker boxes
- `display: contents`
- full CSS block-in-inline splitting
- a separate inherited-only anonymous computed style object
- retained identity for anonymous boxes across layout generations

Those must extend `BoxGenerationRole`, `AnonymousBoxKind`,
`DisplayBoxBehavior`, and regression snapshots deliberately.

## Regression Surface

The layout crate tests cover:

- mixed inline/block child runs generating anonymous block wrappers
- all-inline children not generating anonymous block wrappers
- all-block children not generating anonymous block wrappers
- deterministic parent/child links through anonymous wrappers
- `LayoutBox` projection accepting anonymous boxes as layout participants
- inline whitespace-boundary behavior across anonymous wrappers and block
  children
