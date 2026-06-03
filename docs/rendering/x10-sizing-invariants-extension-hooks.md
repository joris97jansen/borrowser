# X10: Sizing Invariants And Extension Hooks

Status: documented Milestone X close-out invariants and extension hooks

This document closes Milestone X by collecting the implemented sizing model,
normal-flow contracts, debug surfaces, and deliberate extension points into one
source-of-truth summary. It does not replace the issue-specific documents; it
defines the invariants later layout milestones must preserve when adding
positioning, floats, overflow behavior, flex, grid, tables, writing modes, or
more complete CSS sizing features.

Related documents:

- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
- `docs/rendering/z1-flex-layout-architecture-contract.md`

Related code:

- `crates/layout/src/sizing.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/debug.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/box_tree/tests/projection.rs`
- `crates/css/src/values.rs`
- `crates/css/src/specified/length.rs`
- `crates/css/src/computed/value.rs`
- `crates/css/src/computed/style.rs`

## Milestone X Outcome

Milestone X establishes sizing as an explicit layout-owned model rather than an
ad hoc geometry correction. The supported normal-flow subset now has typed
inputs, resolver outputs, intrinsic contributions, min/max constraints,
percentage basis handling, shrink-to-fit behavior, flow propagation contracts,
and deterministic debug snapshots.

The implemented handoff is:

```text
ComputedStyle + generated LayoutBox metadata + intrinsic content measurement
  -> StyleSizeInputs + IntrinsicSizes
  -> ContainingSize + AvailableSpace
  -> ConstraintSpace
  -> SizeResolutionInput
  -> resolve_normal_flow_inline_size / resolve_normal_flow_block_size
  -> UsedAxisSize / ResolvedAxisSize
  -> LayoutBox::rect + LayoutBox::used_content_size
  -> deterministic sizing debug snapshots
```

Future layout features must integrate through this handoff or an explicit
extension of it. They must not reintroduce viewport-width shortcuts, implicit
percentage bases, or local min/max clamp logic outside the sizing model.

## Ownership Rules

CSS owns parsed and computed values only. CSS may preserve `<length-percentage>`
values, `auto`, and `none`, but it must not resolve percentages or produce used
layout sizes.

Box generation owns generated box identity, source attribution, display-to-box
behavior, containing-block metadata, formatting-context metadata, and inline or
block participation. It must not compute used width or height.

Intrinsic sizing owns content contributions. Text, supported replaced/control
fallbacks, inline-flow runs inside ordinary or anonymous block boxes, and
inline-block descendants contribute content-box intrinsic sizes. Author CSS
padding is not part of intrinsic content contribution; padding is applied by
sizing and flow geometry.

The sizing resolver owns preferred content-box size selection, percentage
resolution, shrink-to-fit, min/max application, post-preferred adjustment
metadata, and border-box expansion through currently supported padding inputs.

Normal-flow layout owns placement, recursive containing-size propagation,
available-space narrowing, auto block-size content measurement, and conversion
of resolver output into `LayoutBox::rect`.

Debug surfaces own stable semantic explanations of the above state. They must
record resolver-produced metadata where it exists and must not infer sizing
reasons from final rectangles after the fact.

## Core Type Invariants

`CssPx` is non-negative and finite. It is the scalar for used dimensions,
intrinsic content sizes, and available sizes.

`SignedCssPx` is finite and may be negative. It is currently used for margins.
Used dimensions and available sizes must not use signed scalars.

`AvailableSize::Definite` and `AvailableSize::Indefinite` must remain distinct.
Indefinite values may not be silently treated as zero unless an explicit
supported fallback documents that behavior.

`ContainingSize` is the percentage basis. In normal flow, a child percentage
inline size resolves against the parent resolved content-box inline size.

`AvailableSpace` is the formatting-context space offered to the box. It may be
narrower than `ContainingSize`, for example after subtracting child margins or
future float intrusions.

`ConstraintSpace` carries both `ContainingSize` and `AvailableSpace`. Future
layout modes should extend this structure or introduce mode-specific wrappers
instead of passing raw viewport or parent widths.

`UsedAxisSize` preserves both the preferred result and the final adjusted
result:

- preferred content-box value
- preferred `SizeResolutionReason`
- final content-box value
- applied post-preferred adjustment

`UsedContentSize` is always logical content-box size. Padding, borders, and
margins belong to metrics conversion and flow placement, not to this primitive.

## Width And Height Resolution

Supported normal-flow inline-size resolution works in content-box terms:

1. Select a preferred content-box size from style, percentage basis, auto
   stretch, intrinsic preferred size, or shrink-to-fit.
2. Apply style max constraint.
3. Apply the atomic-inline available-space clamp where applicable.
4. Apply style min constraint last.
5. Add supported padding to produce border-box geometry.

Supported normal-flow block-size resolution works in content-box terms:

1. Use explicit height when provided.
2. Resolve percentage height only against a definite containing block size.
3. If percentage height has an indefinite containing block in normal flow, use
   auto content block-size while recording `DeferredIndefinitePercentage`.
4. Use measured auto content block-size for `height: auto`.
5. Apply supported block-axis constraints from structured inputs.
6. Add supported padding to produce border-box geometry.

Text and comment boxes do not independently resolve normal-flow used sizes in
the current bridge. Their visual contribution belongs to inline layout and
intrinsic contribution collection. Sizing debug output must report
`used-size=none` for those boxes.

## Intrinsic Sizing Invariants

Intrinsic sizes are layout-owned content-box contributions.

Supported contributors include:

- text runs measured by the layout text measurer
- inline-flow runs inside ordinary or anonymous block boxes
- inline-block descendants contributing their outer inline size to an ancestor
- supported replaced/control fallbacks such as images, inputs, textareas, and
  buttons

Intrinsic inline sizing currently models:

- min-content inline size
- max-content inline size
- optional preferred inline size
- optional preferred block size
- optional aspect ratio

Author CSS padding must not be baked into intrinsic content sizes. CSS padding
is applied by the resolver or by outer contribution calculation exactly once.
Fallback control chrome may be included only when it represents internal
UA-like content/control contribution rather than author padding.

## Percentage Invariants

Percentage sizing resolves in layout, not CSS.

Supported percentage values flow through:

```text
specified CSS
  -> computed LengthPercentage
  -> StyleSizeInputs
  -> SizeResolutionInput
  -> containing-size-based used-value resolution
```

Inline percentages, percentage `min-width`, and percentage `max-width` resolve
against `ConstraintSpace::containing_size_for_axis(SizeAxis::Inline)`.

Percentage height resolves against a definite block-axis containing size. In
ordinary normal flow with an indefinite containing block height, the supported
fallback is auto content height plus `DeferredIndefinitePercentage` metadata.

Child margins may narrow `AvailableSpace`; they must not narrow the
`ContainingSize` percentage basis.

## Constraint Invariants

Min/max handling is centralized in `AxisSizeConstraints`. Future layout code
must not apply independent min/max fixups after resolver output.

The supported ordering is:

```text
preferred content-box size
  -> style max
  -> atomic-inline available-space clamp
  -> style min
  -> padding expansion
```

`AppliedSizeConstraint` records the final post-preferred operation that
produced the used content size. It does not replace `SizeResolutionReason`.
Preferred-size selection and post-preferred adjustment must remain separate in
debug output.

When min and max constraints cross, min wins because style max is applied
before style min.

## Shrink-To-Fit And Containing-Size-Dependent Behavior

Supported shrink-to-fit is centralized as:

```text
min(max(min-content, available), preferred-ceiling)
```

`preferred-ceiling` is the intrinsic preferred inline size clamped between
min-content and max-content, or max-content if no separate preferred intrinsic
size exists.

Shrink-to-fit uses available content space, not the containing-size percentage
basis. Percentage min/max constraints still resolve against containing size and
apply after the shrink-to-fit preferred size is selected.

`ShrinkToFitDecision` is the deterministic branch metadata for supported
shrink-to-fit behavior. Later layout modes should add explicit branch metadata
instead of relying on final numeric equality.

## Flow Propagation Invariants

Normal flow derives child inputs from the resolved parent content box.

For in-flow children:

- child border `x` starts at parent content inline-start plus child margin-left
- child `ContainingSize` inline basis is parent content-box inline size
- child `AvailableSpace` inline size is parent content-box inline size minus
  child horizontal margins
- child percentage widths resolve against `ContainingSize`
- auto/stretch and shrink-to-fit behavior use `AvailableSpace`

The root-element bridge must place root children from the root content box, not
the root border box. Full viewport/root/body propagation remains deferred.

## Debug And Regression Invariants

Milestone X debug surfaces are semantic regression contracts:

- `SizeResolutionInput::to_debug_snapshot(...)`
- `LayoutPhaseOutput::to_sizing_debug_snapshot()`
- existing `LayoutPhaseOutput::to_debug_snapshot()`
- existing `BoxTree::to_debug_snapshot()`

Sizing snapshots must remain:

- versioned
- deterministic preorder where tree-based
- fixed-precision for numeric output
- based on logical layout concepts
- aligned with typed resolver inputs and outputs
- independent of renderer/backend pixel output

Layout boxes that independently resolve normal-flow used sizes should carry
`used_content_size`. Text/comment boxes should report `used-size=none`.

## Extension Hooks

### Borders And Box Sizing

Add border inputs to style-facing sizing materialization and extend content-box
to border-box conversion deliberately. Do not overload padding-only helpers to
mean border-box behavior.

`box-sizing` should be represented before resolver use so the resolver can
convert authored border-box constraints into content-box used sizes without
changing `UsedContentSize` semantics.

### Margin Auto And Margin Collapsing

Margin auto distribution and margin collapsing belong to flow placement and
block formatting context behavior. They should consume resolver output rather
than changing preferred content sizes after the fact.

Debug output should distinguish margin resolution metadata from content-size
resolution metadata.

### Floats And Available Space Intrusions

Floats should narrow `AvailableSpace` without changing the relevant
`ContainingSize` percentage basis unless a CSS rule explicitly says otherwise.

Float-aware line and block layout should extend `ConstraintSpace` or wrap it
with mode-specific available-region data rather than passing raw narrowed
widths.

### Positioning

Positioned layout should build explicit constraint spaces for positioned
formatting contexts. Insets, static position, and containing block rules should
feed structured sizing inputs instead of bypassing normal size resolution.

### Flex, Grid, Tables, And Advanced Formatting Modes

Flex, grid, and table algorithms may need mode-specific sizing passes, but they
should reuse the scalar types, intrinsic contribution types, percentage basis
distinction, constraint application metadata, and debug conventions from
Milestone X.

Mode-specific resolvers should preserve:

- content-box used-size outputs
- preferred versus final adjusted metadata
- explicit containing-size versus available-space inputs
- deterministic branch/debug labels

### Replaced Elements And Aspect Ratio

The transitional replaced-element helper remains outside the complete shared
resolver path. Future work should migrate replaced sizing to use
`SizeResolutionInput`, intrinsic sizes, aspect ratio transfer, percentage
bases, and min/max constraints consistently.

When min/max constraints change one axis and the opposite axis is auto,
aspect-ratio transfer must be recomputed through explicit resolver metadata.

### Writing Modes And Fragmentation

Milestone X is horizontal-writing-mode oriented. Orthogonal writing modes
should introduce logical-axis conversion before sizing resolution, not by
special-casing physical margins and padding inside the resolver.

Fragmentation should treat fragmentainer constraints as available space while
preserving containing-size percentage bases according to the relevant CSS rules.

## Deferred Work

Milestone X deliberately does not implement:

- full CSS 2.1 block width equations
- `margin-left/right: auto`
- margin collapsing
- CSS borders
- `box-sizing`
- `min-height` and `max-height` CSS properties
- percentage margins and padding
- `calc(...)`
- authored intrinsic sizing keywords such as `min-content`, `max-content`, and
  `fit-content`
- full replaced-element sizing migration
- aspect-ratio transfer after post-preferred constraints
- floats
- positioned layout
- overflow-created formatting contexts
- flex, grid, and table layout
- orthogonal writing modes
- fragmentation

These deferred features are extension work on top of Milestone X, not
exceptions to the invariants above.

## Milestone X Close-Out Contract

Milestone X is complete when:

- sizing inputs are explicit and typed
- width and height resolution use the shared resolver model
- intrinsic contributions exist for the supported content subset
- min/max constraints are centralized
- supported percentages resolve against containing size
- shrink-to-fit uses explicit intrinsic and available-space inputs
- normal flow propagates containing-size and available-space inputs separately
- debug surfaces expose resolver decisions deterministically
- deferred work is documented as extension work rather than hidden behavior

Future layout milestones should treat this document as the baseline sizing
contract. Changes to these invariants should be deliberate, documented, and
covered by regression tests.
