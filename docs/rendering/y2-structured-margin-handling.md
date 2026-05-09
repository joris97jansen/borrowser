# Y2: Structured Margin Handling

Last updated: 2026-05-09  
Status: implemented structured margin handling for Milestone Y issue 2

This document records the margin handling model introduced by Y2. It builds on
Y1's advanced flow architecture by replacing direct margin arithmetic in normal
flow with layout-owned margin primitives.

Related code:

- `crates/layout/src/flow.rs`
- `crates/layout/src/layout_box.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/inline/geometry.rs`
- `crates/layout/src/inline/engine/atomic.rs`
- `crates/layout/src/inline/tokens.rs`
- `crates/layout/src/inline/intrinsic.rs`
- `crates/layout/src/inline/replaced.rs`
- `crates/layout/src/debug.rs`

Related documents:

- `docs/rendering/y1-advanced-flow-layout-architecture-contract.md`
- `docs/rendering/x8-flow-correctness-varied-sizing.md`
- `docs/rendering/x9-deterministic-sizing-debug-regressions.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`

## Purpose

Margins are flow-placement inputs, not sizing outputs. Y2 makes that boundary
explicit by introducing `FlowMargins` as the shared representation consumed by
normal-flow placement, inline atomic margin boxes, intrinsic outer
contributions, and deterministic layout debug output.

The intended handoff is:

```text
ComputedStyle box metrics
  -> LayoutBox::flow_margins()
  -> FlowMargins
  -> margin-adjusted child position and available space
  -> normal-flow LayoutBox geometry
  -> deterministic margin debug output
```

## FlowMargins

`FlowMargins` stores finite signed logical margins for the current horizontal
writing-mode subset:

- `block-start` maps to physical top
- `inline-end` maps to physical right
- `block-end` maps to physical bottom
- `inline-start` maps to physical left

The type uses `SignedCssPx` because CSS margins may be negative. It only
produces `CssPx` when deriving non-negative quantities such as available inline
space or margin-box size.

Anonymous layout boxes expose zero `FlowMargins` through
`LayoutBox::flow_margins()`. They do not inherit the source anchor's margins
for flow placement.

## Normal-Flow Placement Rules

For in-flow block-level children:

- child border inline-start is parent content inline-start plus
  `inline-start`
- child containing inline size remains the parent content-box inline size
- child available inline size is parent content-box inline size minus
  `inline-start` and `inline-end`, clamped at zero
- negative inline margins may move the border box outside the parent content
  start and may increase available inline space
- child border block-start is the current block cursor plus `block-start`
- after child layout, the block cursor advances by child border block-size plus
  `block-end`

These rules preserve the Milestone X invariant that margins may affect
available space but must not alter the containing-size percentage basis.

Y2 does not implement margin collapsing. Until a later Y issue adds collapse
decisions, adjoining vertical margins are applied additively and
deterministically.

## Inline Atomic Margins

Inline-block and replaced inline boxes use the same `FlowMargins` type for
margin-box advance derivation and border-box paint-rect placement. Inline
tokens carry border-box sizes; the inline engine derives the non-negative
margin-box advance from those sizes and the box's margins. This keeps inline
wrapping, line metrics, and paint rect extraction aligned with the normal-flow
margin vocabulary.

The current supported subset clamps negative total margin-box advance to zero
so inline layout never moves the inline cursor backwards. Token processing
remains finite because the inline engine advances through a finite token stream.
Paint rect size is never reconstructed from that clamped advance; it remains the
actual border-box size and may be offset outside the advance rect by negative
margins.

Intrinsic outer inline contributions include non-negative inline margins only,
preserving the Milestone X intrinsic-sizing contract.

## Debug And Tests

`LayoutPhaseOutput::to_sizing_debug_snapshot()` now prints logical margin
sides:

```text
margin=(block-start=... inline-end=... block-end=... inline-start=...)
```

Y2 regression coverage includes:

- finite signed margin materialization from computed box metrics
- non-finite margin rejection with side metadata
- margin-adjusted child inline placement
- negative margin available-space expansion
- block-axis child positioning and parent auto-height contribution
- anonymous boxes not inheriting anchor margins
- inline atomic negative margins not inflating border-box paint rects
- replaced inline sizing using margin-adjusted available inline size
- deterministic sizing snapshot margin labels

## Deferred Work

Y2 deliberately does not implement:

- margin collapsing decisions
- margin auto distribution
- percentage margins
- writing-mode-specific logical/physical conversion beyond the current
  horizontal subset
- borders as margin-collapse boundaries
- floats and clearance
- positioned out-of-flow margin behavior

Those features must extend `FlowMargins` and margin debug surfaces deliberately
rather than reintroducing direct margin arithmetic at call sites.
