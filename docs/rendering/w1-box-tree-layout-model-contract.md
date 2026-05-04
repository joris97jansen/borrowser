# W1: Box Tree Architecture And Layout Model Contract

Last updated: 2026-05-02  
Status: implemented architecture contract for Milestone W issue 1

This document is the source-of-truth contract for Milestone W1. It defines
Borrowser's box-tree architecture and formatting-context model before later
Milestone W issues expand box generation, anonymous boxes, containing blocks,
positioning, overflow, and advanced layout modes.

Related code:
- `crates/layout/src/lib.rs`
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/inline/mod.rs`
- `crates/layout/src/inline/tokens.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/css/src/computed/style_tree.rs`
- `crates/css/src/computed/style.rs`
- `crates/browser/src/rendering/types.rs`
- `crates/browser/src/rendering/frame.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/w5-containing-block-relationships.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Purpose

Borrowser already has meaningful block and inline layout, but the next layout
features need a clearer model than "walk styled nodes and compute rectangles".
W1 establishes the vocabulary and boundaries that later work must implement
against:

```text
DOM tree
  -> computed style data
  -> styled tree view
  -> box generation
  -> box tree
  -> formatting-context layout
  -> layout-phase output
  -> paint
```

The box tree is a distinct internal model. It is derived from DOM and computed
style, but it is not identical to either one.

## Model Boundaries

### DOM Tree

The DOM tree owns document structure and source identity:

- node kind, element names, attributes, and text
- parent/child/sibling relationships
- stable runtime node IDs
- document lifecycle and mutation state

The DOM tree does not own:

- cascade winners or computed values
- generated layout boxes
- anonymous boxes, list marker boxes, or future pseudo boxes
- geometry, line boxes, containing blocks, or formatting-context membership

Layout may inspect DOM node kind and attributes only for intrinsic element
semantics that are not CSS display defaults, such as replaced-element
classification. Display behavior for ordinary elements must come from computed
style.

### Computed Style And Styled Tree

Computed style owns the resolved CSS values selected for each styleable DOM
node. `StyledNode` is the frame-scoped style-to-layout view that pairs DOM
identity with computed style and child order.

Computed style and `StyledNode` own:

- inherited, initial, and cascaded property values after style resolution
- computed `display` values, including UA defaults and author overrides
- deterministic style-tree construction from retained style artifacts

Computed style and `StyledNode` do not own:

- whether an anonymous layout box must be generated
- formatting-context root assignment
- used geometry, line wrapping, or fragment positions
- paint ordering beyond style data needed by later phases

The style phase must not precompute layout-only concepts merely because layout
currently rebuilds every frame.

### Box Tree

The box tree is the layout engine's generated structural model for one layout
pass. Each box represents a layout participant produced from a styled source,
anonymous generation rule, marker rule, or future generated-content rule.

A layout box owns or records:

- its box kind and layout participation category
- the computed style snapshot or reference used for layout decisions
- source identity for DOM-backed boxes
- deterministic parent/child order in the generated box tree
- layout metadata such as list markers or replaced-element classification
- geometry after layout has run

A layout box is not:

- a DOM node
- a stylesheet rule or cascade winner
- a paint command
- a retained display-list item
- necessarily one-to-one with a `StyledNode`

W2 introduces `BoxTree` as Borrowser's frame-local generated box-tree
structure. `LayoutBox` remains the current geometry projection consumed by
paint and hit testing. Later Milestone W work may split generated boxes from
laid-out fragments further, but must preserve the phase handoffs defined by
Milestone V.

## Box Generation Responsibilities

Box generation is the first layout-owned step after the style-to-layout handoff.
It consumes `StyledNode` plus explicit layout environment data and produces the
layout box structure that later layout algorithms operate on.

Box generation owns:

- mapping supported computed `display` values to box generation behavior
- suppressing `display: none` element subtrees
- selecting block-level, inline-level, inline-block, list-item, and replaced
  layout participation for the supported subset
- preserving DOM child order for generated boxes
- creating deterministic list marker metadata for supported lists
- creating anonymous boxes required by supported block/inline mixing rules
- creating future pseudo/generated boxes when those features exist
- assigning future box-generation roles such as root box, ordinary box,
  anonymous box, marker box, and pseudo box

W3 defines this mapping through explicit `DisplayBoxGeneration`,
`PrincipalBox`, and `DisplayBoxBehavior` decisions before geometry projection.

Box generation must not:

- parse CSS text
- infer UA display defaults from HTML tag names
- inspect stylesheet ordering or cascade provenance
- compute final used sizes or line breaks
- emit paint primitives

### Transitional Root Handling

The current layout code still contains transitional root/document-element
handling while Milestone W has not yet introduced the full root-box model. This
is not a UA display-default shortcut. Ordinary element display behavior must
continue to come from computed style, and document-element classification must
come from tree context rather than tag name alone.

The intended destination is an explicit role model, for example:

- document root
- document element
- root box
- initial containing block
- ordinary element box
- anonymous box
- marker box

Later Milestone W issues should replace tag-name root handling with those
roles rather than attempting to remove all special behavior around document
roots.

## Layout And Formatting Responsibilities

After box generation, layout algorithms compute used geometry and fragment
placement. These algorithms consume the generated box tree, computed style,
viewport constraints, text measurement, and replaced-element metadata.

Layout owns:

- used width, height, x, and y computation
- block flow placement
- inline tokenization, line construction, and text fragment placement
- inline-block and replaced inline atomic layout behavior for the supported
  subset
- margin, padding, and box-metric application
- future containing-block resolution
- future intrinsic sizing and constraint solving

Layout must not mutate DOM or computed style. Any future retained layout cache
must be an explicit retained artifact with invalidation rules, not an accidental
extension of the current frame-local `BoxTree` or `LayoutBox` lifetimes.

## Formatting Context Model

A formatting context is an algorithmic scope that defines how boxes inside it
participate in layout. It is not a CSS rule, a DOM node, or a paint primitive.

Borrowser's current supported model is:

- a root block-flow context for document layout
- block formatting behavior for block-level children in normal flow
- inline formatting behavior inside block containers that contain inline-level
  content
- atomic inline participation for inline-block and replaced inline boxes

The model must evolve toward explicit context assignment:

- `BlockFormattingContext` for block-flow layout scopes
- `InlineFormattingContext` for line-box construction inside block containers
- future `FlexFormattingContext`, `GridFormattingContext`, and table-specific
  contexts when those display modes are implemented
- future independent formatting contexts created by overflow, floats,
  containment, positioning, or display values

Formatting-context assignment belongs to layout/box generation, not CSS
parsing. Computed style provides the values; layout decides which generated
boxes establish or participate in which context.

## Containing Blocks

W5 implements the first explicit containing-block model for the supported
in-flow subset. A containing block is a layout relationship used to resolve
sizes and positions. It is distinct from DOM parentage and from style
inheritance.

The current model records deterministic relationships for:

- initial containing block
- root element containing block
- normal-flow containing block

Later W issues must extend this deliberately for:

- absolute/fixed positioning containing block
- formatting-context root containing block

The invariant is that containing-block resolution must be modeled as layout
data, not rediscovered ad hoc in paint or input routing.

## Determinism And Debug Expectations

Box generation and formatting-context assignment must be deterministic for a
fixed DOM, computed style set, viewport environment, text measurer, and
replaced-element metadata.

Required invariants:

- generated boxes appear in deterministic preorder
- DOM-backed boxes preserve stable DOM node identity
- comment nodes do not generate layout boxes
- anonymous boxes must receive deterministic source relationships or IDs when
  introduced
- `display: none` subtrees are consistently absent from layout output
- author and inline CSS overrides affect layout only through computed style
- layout debug output reports semantic box data, not backend paint artifacts
- paint and hit testing consume the selected layout output without regenerating
  boxes
- no later phase silently creates layout boxes to compensate for missing box
  generation behavior

The existing `LayoutPhaseInput::to_debug_snapshot()` and
`LayoutPhaseOutput::to_debug_snapshot()` surfaces remain the deterministic
regression boundary until W-specific box-generation snapshots are introduced.

## Deferred Work And Non-Goals

W1 is an architecture contract. It intentionally does not ship:

- a separate retained box-tree data structure
- full anonymous box generation beyond the supported W4 block/inline mixing
  subset
- full formatting-context IDs and containing-block behavior beyond the
  supported W5 in-flow subset
- flexbox, grid, floats, positioning, overflow, fragmentation, or transforms
- a complete table formatting model
- retained layout caching
- display-list or paint tree changes
- CSSOM integration

Those are later work items. They must extend this contract instead of mixing
DOM, style, box generation, geometry, and paint responsibilities back together.

## Extension Points

Future Milestone W implementation should prefer explicit internal concepts over
tag-name or phase-local shortcuts:

- `BoxGenerationRole`
- `BoxKind` expansion for anonymous, marker, root, and formatting-specific
  boxes
- `FormattingContextKind`
- `FormattingContextId`
- deterministic anonymous-box source metadata
- a generated box tree and later fragment structures separated further if
  fragmentation or retained layout makes that split necessary

The current code does not need all of these types immediately. The contract is
that later additions use named model concepts and typed handoffs rather than
encoding layout semantics implicitly in DOM traversal.

## Close-Out Criteria For W1

W1 is satisfied while these conditions hold:

- the box tree is documented as distinct from DOM and computed style
- DOM, style, box generation, layout, formatting-context, and paint
  responsibilities are explicit
- formatting contexts are defined as layout-owned algorithmic scopes
- containing-block modeling exists as structured layout data for the supported
  W5 in-flow subset
- current root-element handling is documented as transitional
- deferred Milestone W work is named as non-goals rather than hidden in
  implementation comments
