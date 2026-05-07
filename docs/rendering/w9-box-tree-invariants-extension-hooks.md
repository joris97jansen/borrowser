# W9: Box Tree Invariants And Extension Hooks

Last updated: 2026-05-04
Status: implemented close-out contract for Milestone W issue 9

This document closes Milestone W by collecting the box-tree invariants,
ownership rules, formatting-context assumptions, and extension hooks that later
layout milestones must preserve.

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
- `docs/rendering/w6-block-formatting-context-foundations.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Milestone W Result

Milestone W establishes Borrowser's current layout foundation:

```text
DOM tree
  -> computed style
  -> StyledNode view
  -> DisplayBoxGeneration
  -> BoxTree / BoxNode
  -> LayoutBox geometry projection
  -> LayoutPhaseOutput
  -> PaintPhaseInput
```

The important architectural result is that `StyledNode` is not the layout
tree. Layout owns generated boxes, source relationships, display-to-box
behavior, anonymous box generation, containing-block metadata, block formatting
metadata, inline formatting metadata, and deterministic debug surfaces.

## Ownership Invariants

The DOM tree owns:

- source node identity
- element names, attributes, text, comments, and document structure
- runtime mutation identity

The DOM tree does not own:

- generated boxes
- anonymous boxes
- containing blocks
- formatting-context membership
- layout geometry

Computed style owns:

- total computed CSS values for styled DOM nodes
- UA, author, and inline style cascade outcomes
- computed `display`

Computed style does not own:

- principal box generation
- anonymous box generation
- containing-block assignment
- formatting-context establishment or participation
- used geometry or line construction

`BoxTree` owns:

- frame-local generated box identity through deterministic `BoxId`s
- parent/child generated box relationships
- `BoxSource` for DOM-backed and generated boxes
- `BoxGenerationRole`
- `DisplayBoxBehavior`
- containing-block metadata
- block formatting context metadata
- inline formatting context metadata
- list marker metadata
- replaced-element classification and intrinsic metadata

`LayoutBox` currently represents:

- geometry projection for each generated box
- the generated `BoxId`
- the generated source relationship
- projected containing-block and formatting-context metadata
- bridge access to the source anchor `StyledNode`

Future fragment trees, retained layout caches, or display-list structures must
not reuse DOM identity as layout identity. They must introduce explicit
layout-owned identity and invalidation rules.

## Box Generation Invariants

Box generation is the first layout-owned step after style handoff.

For the supported subset:

- comments do not generate boxes
- `display: none` element subtrees do not generate boxes
- document root and document element roles are context-based
- ordinary element display behavior comes from computed `display`
- text nodes generate text-run boxes in the current bridge model
- replaced element classification is layout-owned intrinsic element semantics,
  not a UA display-default shortcut
- generated boxes are assigned deterministic preorder `BoxId`s
- generated `BoxId`s are stable only within one box-tree generation and must
  not be treated as retained identity across layout passes
- parent/child relationships are box-tree relationships, not DOM parent
  pointers

Adding a new `Display` value or box-generation role must update:

- `DisplayBoxGeneration`
- `PrincipalBox`
- `DisplayBoxBehavior`
- box-tree debug serialization
- layout projection, if geometry needs new metadata
- representative tests and documentation

## Anonymous Box Invariants

W4 implements anonymous block generation for the supported mixed block/inline
child subset.

Current rules:

- mixed block-level and inline-level direct children of supported block
  containers are normalized by wrapping each contiguous inline-level run in an
  anonymous block box
- all-inline child sets remain direct inline formatting content
- all-block child sets do not generate anonymous blocks
- suppressed children do not split anonymous runs
- anonymous boxes are `BoxSource::Anonymous`, not fake DOM nodes
- anonymous boxes have deterministic source anchors and `source-id=none` in
  box-tree debug output

Future anonymous inline boxes, table anonymous boxes, marker boxes, pseudo
boxes, and generated-content boxes must be represented as generated boxes, not
as DOM or style-tree mutations.

## Containing-Block Invariants

W5 models containing blocks as generated-box relationships:

- `ContainingBlockId` wraps the establishing `BoxId`
- containing-block identity is not DOM parent identity
- a box's parent and containing block may differ
- current relationships are assigned during deterministic box generation
- `LayoutBox` preserves containing-block metadata after projection

Future sizing and positioning work must extend containing-block establishment
rules deliberately for:

- percentage sizing
- absolute, fixed, and sticky positioning
- transforms
- containment
- overflow and `flow-root`
- viewport and initial containing block refinements

Those features must not rediscover containing blocks ad hoc while walking
paint, input, or DOM structures.

## Formatting-Context Invariants

W6 and W7 separate participation from context establishment.

Block formatting invariants:

- Borrowser's current W6 bridge model treats the document root and document
  element as block formatting context roots for deterministic root-flow
  ownership
- inline-block boxes establish block formatting contexts for descendants
- ordinary block, list-item, and anonymous block boxes participate as
  block-level normal-flow boxes without automatically establishing independent
  BFCs
- inline and text-run boxes participate as inline-level content
- replaced inline boxes participate atomically

Inline formatting invariants:

- `InlineFormattingContextId` wraps the establishing `BoxId`
- block/list-item/document-element boxes establish an IFC only when their
  direct supported child set is inline-only
- W4 anonymous block boxes establish IFCs for wrapped inline runs
- inline-block boxes participate atomically in the parent IFC
- inline-block boxes establish an internal IFC only when their supported child
  set directly forms an inline formatting context
- unsupported mixed inline/block children inside an inline-block do not inherit
  the outer IFC across the atomic boundary
- line-building consumes `InlineFormattingParticipation`, not raw `BoxKind`

Future floats, margin collapsing, overflow-created BFCs, flex, grid, table,
fragmentation, bidi, and writing-mode support must extend these typed
formatting concepts rather than overloading `BoxKind` or DOM shape.

## Debug And Regression Invariants

W8 establishes `BoxTree::to_debug_snapshot()` as the deterministic
box-generation debug surface.

Debug output must remain:

- semantic rather than pixel/backend specific
- preorder and deterministic
- explicit about generated `BoxId`s
- explicit about source IDs and generated source roles
- explicit about containing-block, block formatting, and inline formatting
  metadata
- aligned with `LayoutPhaseOutput::to_debug_snapshot()` and browser
  phase-boundary snapshots

Any debug field order or label change is a contract change. It must update
tests deliberately and should be documented in the relevant W/V document.

The debug snapshot is an internal regression contract, not a public API.
Changing it is allowed when the layout model deliberately changes, but such
changes must be reviewed, documented, and covered by updated tests.

## Extension Hooks For Later Layout Milestones

Later layout milestones should attach at these points:

- display expansion: add CSS value support, then update display-to-box
  generation before geometry code consumes it
- anonymous generation expansion: extend `BoxSource`, roles, and generation
  normalization before paint or layout projection relies on the new boxes
- sizing and intrinsic layout: use the Milestone X sizing contract; consume
  generated box metadata and computed style; preserve explicit environment
  inputs for viewport, text measurement, and replaced metadata
- positioning: extend containing-block rules and out-of-flow generated box
  participation explicitly
- overflow and flow-root: extend formatting-context and containing-block
  establishment rules explicitly
- flex/grid/table layout: add new formatting-context kinds and display
  behaviors rather than treating them as block fallbacks
- fragmentation and advanced inline layout: introduce fragment/line identity
  deliberately instead of treating `LayoutBox` as a permanent fragment model
- retained layout: add runtime-owned retained artifacts and invalidation rules;
  do not retain borrowed `StyledNode` or frame-local `BoxTree` references

## Deferred Work

Milestone W intentionally does not complete:

- full CSS box-generation behavior
- independent marker boxes
- full anonymous inline/table box generation
- margin collapsing
- floats and clearance
- absolute/fixed/sticky positioning
- percentage and intrinsic sizing completeness
- overflow, transforms, containment, or `flow-root`
- flex, grid, or table formatting contexts
- bidi, vertical writing modes, full white-space handling, or text shaping
- fragmentation or a separate fragment tree
- retained layout caches
- paint/display-list/layer architecture

These are no longer hidden gaps in the layout core. They are explicit future
extensions on top of the Milestone W model.

## Test Alignment

The current regression surface pins the Milestone W contract through:

- W0 UA display default cascade and override tests in browser phase-boundary
  coverage
- `BoxTree` parent/child and deterministic preorder tests
- display-to-box behavior tests
- anonymous block generation tests
- containing-block relationship tests
- block formatting context tests
- inline formatting context tests
- `LayoutBox` projection preservation tests
- `BoxTree::to_debug_snapshot()` exact structural snapshots
- browser render phase-boundary snapshots proving layout metadata survives the
  layout-to-paint handoff

Future changes to box generation or formatting foundations should update both
the narrow structural tests and the deterministic debug snapshots. If a feature
cannot be described in these terms, it probably needs a new explicit layout
model concept before implementation.

## Milestone W Close-Out

Milestone W is complete while these conditions remain true:

- box generation is explicit and layout-owned
- DOM, style, box tree, layout geometry, and paint responsibilities are
  separated
- display handling enters layout through computed style and explicit
  display-to-box decisions
- anonymous boxes are represented as generated boxes
- containing blocks are generated-box relationships
- block and inline formatting context metadata is explicit
- debug surfaces expose structural layout semantics deterministically
- deferred layout work has named extension hooks rather than implicit
  shortcuts
