# W8: Box Generation And Formatting Debug Surfaces

Last updated: 2026-05-04
Status: implemented deterministic box-generation and formatting-foundation debug coverage for Milestone W issue 8

This document defines Borrowser's current deterministic debug surface and
regression coverage for generated box-tree structure and formatting
foundations.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/browser/src/rendering/tests/phase_boundaries.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W8 makes the Milestone W layout foundations inspectable and
regression-testable. The goal is stable structural output for how styled DOM
input becomes generated layout boxes, not pixel snapshots or backend paint
commands.

The current debug path is:

```text
StyledNode
  -> BoxTree::generate(...)
  -> BoxTree::to_debug_snapshot()
  -> LayoutBox geometry projection
  -> LayoutPhaseOutput::to_debug_snapshot()
  -> PaintPhaseInput / browser phase-boundary snapshots
```

## BoxTree Snapshot Contract

`BoxTree::to_debug_snapshot()` is the W8 box-generation debug surface. It
serializes:

- snapshot version and semantic object kind
- root generated `BoxId`
- total generated box count
- preorder generated box records
- parent and child `BoxId` relationships
- containing-block relationships
- block formatting context relationships and participation
- inline formatting context relationships and participation
- direct source node ID when present
- source anchor label for DOM-backed and generated boxes
- generation role, `BoxKind`, computed `display`, and layout-owned display
  behavior
- list marker metadata
- replaced-element metadata and intrinsic size metadata

The snapshot intentionally uses generated box identity rather than DOM identity
for internal relationships. DOM IDs are source metadata only.

## Determinism Rules

The W8 debug surface follows these rules:

- output starts with `version: 1`
- tree traversal is preorder
- indentation reflects generated box-tree structure
- `BoxId` labels are deterministic frame-local preorder IDs
- absent relationships serialize as `none`
- text and element labels use deterministic escaping
- child lists preserve generated child order
- anonymous boxes serialize with `source-id=none` and an explicit generated
  role

Any change to field order, labels, or traversal order is a debug contract
change and must update tests deliberately.

## Regression Fixtures

The layout crate now includes exact snapshot coverage for a representative
box-generation and formatting-foundation fixture:

- document root and document-element boxes
- ordinary block boxes
- inline boxes and text runs
- anonymous block boxes generated for mixed block/inline child runs
- `display: none` subtree omission
- comment node omission
- list-item display behavior and marker metadata
- replaced inline box generation
- containing-block metadata
- block formatting context metadata
- inline formatting context metadata

The browser crate phase-boundary snapshots continue to prove that the layout
debug metadata survives projection through:

```text
LayoutPhaseOutput
  -> PaintPhaseInput
  -> RenderPhaseBoundaryDebugSnapshot
```

## Scope

W8 does not add new layout behavior. It stabilizes the debug and regression
surface for the box-generation and formatting foundations introduced by W1
through W7.

Future layout milestones must extend this surface when they add new generated
box roles, display behaviors, containing-block triggers, formatting context
kinds, fragment trees, or retained layout identities.
