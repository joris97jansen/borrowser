# W5: Containing-Block Relationships

Last updated: 2026-05-04
Status: implemented containing-block model for Milestone W issue 5

This document defines Borrowser's current containing-block relationship model
inside the generated layout-owned `BoxTree`.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/layout/src/inline/refine.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W5 makes containing-block relationships explicit generated-box metadata. This
gives later sizing, positioning, percentage resolution, and out-of-flow layout
work a deterministic base instead of relying on incidental parent traversal.

The current pipeline shape is:

```text
StyledNode
  -> DisplayBoxGeneration
  -> BoxTree with explicit parent/child and containing-block links
  -> LayoutBox geometry projection
```

Containing-block assignment is part of box generation. It is not inferred by
paint, hit testing, or ad hoc layout traversal.

## Current Model

Each generated `BoxNode` records:

- `containing_block: Option<ContainingBlockId>`
- `establishes_containing_block: bool`

`ContainingBlockId` is a frame-local wrapper around the establishing `BoxId`.
It is intentionally separate from raw parent identity: a box's parent and its
containing block are not always the same box. Inline boxes, for example, do not
establish containing blocks in the current subset, so their text descendants
continue to resolve against the nearest ancestor that does.

The document-root box represents the initial containing block for the current
subset:

- the document root has no containing block
- the document root establishes the initial containing block
- the document element resolves against the document root

## Establishment Rules

The following generated boxes establish containing blocks in the current
supported subset:

- document root
- document element
- block boxes
- list-item principal boxes
- inline-block boxes
- anonymous block boxes

The following generated boxes do not establish containing blocks yet:

- inline boxes
- text-run boxes
- replaced inline boxes
- marker boxes

For a newly generated box, its containing block is:

- the nearest ancestor generated box that establishes a containing block
- `None` only for the document-root box

This lookup is deterministic because it is assigned during preorder box
generation and stored directly on each `BoxNode`.

## Anonymous Boxes

W4 anonymous block boxes participate in the containing-block model as real
generated boxes:

- the anonymous block resolves against its parent block container
- the anonymous block establishes a containing block for its wrapped inline run
- wrapped text and inline descendants resolve against the anonymous block unless
  they enter a nested box that establishes a new containing block

This keeps generated structure and containing-block ownership aligned.

## Debug Surface

`BoxTree::to_debug_snapshot()` includes:

- `cb=<box-id|none>`
- `establishes-cb=<yes|no>`

This is the deterministic regression surface for W5. Layout-phase snapshots
also expose containing-block identity after `BoxTree` metadata is projected into
`LayoutBox`.

## Deferred Work

W5 intentionally does not implement:

- `position` parsing or positioned containing-block rules
- absolute/fixed/sticky positioning
- percentage sizing resolution
- transforms establishing containing blocks
- scroll containers and overflow clipping relationships
- out-of-flow formatting roots
- retained containing-block identity across layout generations
- separate viewport object identity distinct from the document-root box

Those features must extend `ContainingBlockId`, establishment rules, debug
snapshots, and tests deliberately.

## Regression Surface

The layout crate tests cover:

- document root as the initial containing-block root
- document element resolving against document root
- block containers establishing containing blocks
- inline boxes not establishing containing blocks
- inline-blocks establishing containing blocks for descendants
- anonymous block boxes establishing containing blocks for wrapped inline runs
- `LayoutBox` projection preserving containing-block metadata
- deterministic `BoxTree::to_debug_snapshot()` output with containing-block
  metadata
