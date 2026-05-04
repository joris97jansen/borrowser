# W2: Structured Box Tree Data Structures

Last updated: 2026-05-02  
Status: implemented structured box-tree data model for Milestone W issue 2

This document records the code-level box-tree structures introduced after the
W1 architecture contract. W2 creates an explicit generated box-tree model while
preserving the existing layout-to-paint handoff.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/layout/src/inline/mod.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W1 defined the box tree as a distinct layout-owned model. W2 introduces that
model in code:

```text
StyledNode
  -> BoxTree / BoxNode
  -> LayoutBox geometry projection
  -> LayoutPhaseOutput
```

`LayoutBox` remains the geometry structure consumed by paint and hit testing.
`BoxTree` is now the generated structural model used before geometry is
computed.

## Implemented Structures

`BoxTree` is the frame-local generated box tree for one layout pass. It owns an
indexed list of `BoxNode` records and exposes a stable root `BoxId`.

`BoxId` is a deterministic preorder index into the generated tree. It is not a
DOM node ID and is not retained across independent layout generations.

`BoxNode` records:

- its own `BoxId`
- its parent `BoxId`, if any
- child `BoxId`s in deterministic order
- `BoxGenerationRole`
- current `BoxKind`
- `DisplayBoxBehavior`
- `BoxSource`
- computed display and style reference
- list marker metadata
- replaced-element metadata and intrinsic image size metadata

`BoxSource` separates box source identity from box-tree ownership:

- `DomNode(...)` for current DOM-backed boxes
- `Anonymous { ... }` reserved for upcoming anonymous box generation
- `Marker { ... }` reserved for future marker box generation

This means future non-DOM-backed boxes can be represented without pretending
that every layout box is a `StyledNode`.

## Generation Contract

`BoxTree::generate(...)` is the structured box-generation entry point. It
currently:

- walks the styled tree in deterministic preorder
- omits comment nodes because comments are DOM data, not layout participants
- suppresses `display: none` element subtrees
- assigns parent/child links using `BoxId`
- records document-root and context-based document-element roles
- maps computed display and replaced-element classification to `BoxKind`
- records explicit display-to-box behavior metadata
- records list marker metadata for supported list containers
- records replaced-element metadata required by current layout

The existing layout phase now builds `LayoutBox` geometry from `BoxTree` rather
than directly constructing nested `LayoutBox` records while walking styled DOM
children.

## Invariants

The W2 box tree must satisfy these invariants:

- `BoxId`s are assigned deterministically in preorder.
- Parent and child links are internal box-tree relationships, not DOM parent
  pointers.
- DOM-backed boxes preserve source node identity through `BoxSource`.
- Comment nodes must not generate boxes.
- The document-element role is assigned from box-tree context, not from an
  element's tag name alone.
- Non-DOM-backed source variants are part of the model even before anonymous
  box generation is implemented.
- `display: none` element subtrees are absent from the generated box tree.
- Metadata required by current layout is available before geometry projection.
- Supported computed display values are mapped through an explicit
  `DisplayBoxGeneration` decision.
- `BoxTree` is frame-local rebuilt state and is not retained by `PageState`.
- Paint and hit testing continue to consume `LayoutPhaseOutput`, not `BoxTree`
  directly.

## Non-Goals

W2 does not implement:

- full anonymous box generation
- marker boxes as independent generated boxes
- explicit formatting-context IDs
- containing-block IDs
- retained layout caches
- a separate fragment tree
- paint/display-list changes

Those belong to later Milestone W issues. W2 only establishes the structural
box-tree data model and uses it as the source for existing geometry output.

## Regression Surface

The layout crate tests now cover:

- deterministic preorder `BoxId` assignment
- parent/child box-tree links
- `display: none` subtree omission
- comment node omission
- context-based document-element role assignment
- list marker metadata
- layout metadata such as inline-block and replaced classifications
- display-to-box behavior metadata for supported display values
- future non-DOM-backed `BoxSource` representation
- stable `BoxTree::to_debug_snapshot()` output
