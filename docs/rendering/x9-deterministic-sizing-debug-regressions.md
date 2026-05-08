# X9: Deterministic Sizing Debug And Regression Surfaces

X9 adds stable semantic debug output for Milestone X sizing behavior. The goal
is to make size-resolution decisions inspectable without depending on backend
paint output, screenshots, or incidental floating-point formatting.

Related contracts:

- `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
- `docs/rendering/x2-structured-size-resolution-model-inputs.md`
- `docs/rendering/x3-width-height-resolution-supported-subset.md`
- `docs/rendering/x4-intrinsic-sizing-supported-content.md`
- `docs/rendering/x5-min-max-sizing-constraints.md`
- `docs/rendering/x6-percentage-sizing-targeted-subset.md`
- `docs/rendering/x7-shrink-to-fit-containing-size-dependent-sizing.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`

## Debug Surfaces

X9 introduces two sizing-focused surfaces:

- `SizeResolutionInput::to_debug_snapshot(mode, auto_content_block_size)`
- `LayoutPhaseOutput::to_sizing_debug_snapshot()`

`SizeResolutionInput::to_debug_snapshot` is the resolver-level surface. It
prints:

- normal-flow sizing mode
- auto content block-size input
- containing-size percentage basis
- available formatting space
- style preferred/min/max inputs
- margin and padding inputs
- intrinsic size contributions
- resolved inline and block content-box sizes
- preferred-size reasons
- post-preferred adjustments
- border-box axis sizes after padding expansion

`LayoutPhaseOutput::to_sizing_debug_snapshot` is the integrated flow surface. It
prints each layout box in deterministic preorder with:

- generated box identity and source
- containing-block relationship
- block and inline flow participation
- border-box geometry
- content-box geometry
- margin and padding metrics
- recorded used content-size metadata
- child count

## Used-Size Metadata

`LayoutBox` now carries `used_content_size: Option<UsedContentSize>`. The
normal-flow refinement pass fills this from the actual resolver outputs used to
set `LayoutBox::rect`.

This keeps debug output aligned with internal sizing structures:

```text
SizeResolutionInput
  -> resolve_normal_flow_inline_size
  -> resolve_normal_flow_block_size
  -> LayoutBox::rect
  -> LayoutBox::used_content_size
  -> deterministic sizing debug snapshot
```

The snapshot must preserve both parts of the X1/X5 used-size contract:

- preferred value and preferred `SizeResolutionReason`
- final value and applied post-preferred adjustment

Generated boxes that do not independently resolve normal-flow used sizes may
record zero used-size metadata or `none`; the snapshot must not imply that text
runs or comments independently stretched to the containing block.

## Determinism Rules

Sizing snapshots must remain:

- versioned
- preorder-stable
- independent from hash-map iteration
- formatted with fixed decimal precision
- based on logical sizing concepts, not renderer pixels
- aligned with typed sizing inputs and resolver outputs

Changing snapshot text is allowed when the sizing contract changes, but such
changes should be reviewed as semantic regression-contract updates rather than
incidental formatting churn.

## Regression Coverage

X9 pins representative behavior through exact snapshots:

- resolver-level intrinsic/shrink-to-fit sizing
- percentage max constraint after shrink-to-fit
- margin and padding sizing inputs
- preferred value/reason and final adjustment metadata
- layout-level child percentage basis versus margin-reduced available space
- content-box and border-box flow geometry

These snapshots complement the narrower numeric regression tests from X3-X8.
Numeric tests still assert individual invariants directly; X9 snapshots assert
that the full sizing explanation remains stable and inspectable.

## Deferred Work

The X9 surfaces are intentionally semantic and extensible. Later milestones may
extend them with:

- margin auto distribution metadata
- margin collapsing metadata
- border and `box-sizing` inputs
- replaced-element aspect-ratio transfer details
- float and positioned formatting context inputs
- flex/grid/table sizing modes
- fragmentation and orthogonal writing-mode sizing inputs

New fields should be appended deliberately and covered by exact snapshot tests.
