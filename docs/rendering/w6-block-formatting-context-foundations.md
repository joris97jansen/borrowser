# W6: Block Formatting Context Foundations

Last updated: 2026-05-04
Status: implemented block formatting context foundations for Milestone W issue 6

This document defines Borrowser's current block formatting context foundation
for the supported normal-flow layout subset.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/tokens.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W6 makes block formatting context data explicit generated-box metadata. The
goal is not full CSS block layout correctness yet; it is to define a stable
normal-flow block layout contract that later sizing, positioning, overflow,
floats, and fragmentation work can extend deliberately.

The current pipeline shape is:

```text
StyledNode
  -> DisplayBoxGeneration
  -> BoxTree with parent/child, containing-block, and formatting-context links
  -> LayoutBox geometry projection
  -> block/inline geometry refinement
```

Formatting-context assignment belongs to layout-owned box generation. It is not
derived by paint, hit testing, or DOM traversal.

## Current Model

Each generated `BoxNode` records:

- `formatting_context: Option<FormattingContextId>`
- `establishes_formatting_context: Option<FormattingContextKind>`
- `block_formatting_participation: BlockFormattingParticipation`

`FormattingContextId` is a frame-local wrapper around the generated `BoxId`
that establishes the context. It is intentionally distinct from DOM node ID,
parent `BoxId`, and `ContainingBlockId`.

`FormattingContextKind::Block` represents Borrowser's W6 supported normal-flow
block formatting scope. It is not yet the complete CSS BFC trigger matrix.

## Establishment And Participation Rules

The following generated boxes establish a block formatting context in the
current supported subset:

- document root
- document element
- inline-block boxes

The following generated boxes participate as block-level normal-flow boxes in
the current block formatting context but do not establish independent block
formatting contexts in W6:

- block boxes
- list-item principal boxes
- anonymous block boxes

This distinction is intentional. Ordinary block boxes participate in an
existing block formatting context; they do not automatically become independent
BFC roots. Later features such as floats, margin collapsing, `overflow`,
`flow-root`, containment, and positioning must extend establishment rules
deliberately.

The following generated boxes do not establish a block formatting context yet:

- inline boxes
- text-run boxes
- replaced inline boxes
- marker boxes

For a newly generated box, its current formatting context is:

- the nearest ancestor generated box that establishes a formatting context
- `None` only for the document-root box

This lookup is assigned during deterministic preorder box generation and stored
directly on each `BoxNode`.

## Block Participation

`BlockFormattingParticipation` records the supported block-flow role of each
generated box:

- `Root` seeds the initial block formatting context.
- `BlockLevel` participates as a block-level normal-flow child.
- `InlineLevel` participates in the inline flow of the current block scope.
- `AtomicInline` participates as an atomic inline-level box while establishing
  its own block context for descendants.
- `None` is reserved for generated boxes outside the current block-flow subset.

Block geometry refinement consumes this metadata when deciding which children
belong to inline layout and which children are stacked as block-level
participants. This prevents the layout algorithm from relying only on
`BoxKind` or DOM node shape.

## LayoutBox Projection

The geometry projection preserves the W6 metadata:

- `LayoutBox::formatting_context()`
- `LayoutBox::establishes_formatting_context()`
- `LayoutBox::block_formatting_participation()`

Future geometry algorithms can consume formatting-context relationships without
rebuilding or retaining the original `BoxTree`.

## Debug Surface

`BoxTree::to_debug_snapshot()` and `LayoutPhaseOutput::to_debug_snapshot()`
include:

- `fc=<box-id|none>`
- `establishes-fc=<block|none>`
- `block-participation=<root|block-level|inline-level|atomic-inline|none>`

These fields are part of the deterministic regression surface for W6.

## Deferred Work

W6 intentionally does not implement:

- full CSS BFC establishment triggers
- margin collapsing
- floats
- absolute/fixed/sticky positioning
- overflow-created formatting contexts
- transforms, containment, or flow-root
- retained formatting-context identity across layout generations
- independent inline formatting context IDs
- flex, grid, table, or fragmentation contexts

Those features must extend `FormattingContextKind`, establishment rules, debug
snapshots, and tests deliberately.

## Regression Surface

The layout crate tests cover:

- document root seeding the initial block formatting context
- document element/root-element bridge establishing a block formatting context
- ordinary block-level boxes participating without establishing block scopes
- inline boxes and text retaining the nearest block context without
  establishing one
- inline-blocks participating atomically while establishing a block context for
  descendants
- anonymous block boxes participating without establishing block scopes
- projection of formatting-context metadata into `LayoutBox`
- deterministic debug snapshots exposing formatting-context metadata
