# AA8: Paint Debug Visual Regression Surface

Last updated: 2026-06-12
Status: implemented deterministic paint-operation debug surface for Milestone AA issue 8

This document defines Borrowser's AA8 visual regression surface for the current
paint model. AA8 uses deterministic paint-operation snapshots derived from
paint-owned semantic primitives and AA paint ordering rules. It does not add
pixel comparison, raster screenshot infrastructure, egui command serialization,
runtime draw recording, retained display lists, scene graphs, compositor state,
GPU abstractions, or full CSS painting order.

Related code:
- `crates/gfx/src/paint/debug.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/contracts.rs`
- `crates/gfx/src/paint/mod.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/aa3-border-rendering-box-decoration.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/rendering/aa5-text-decoration-rendering-subset.md`
- `docs/rendering/aa6-overflow-clipping-paint-behavior.md`
- `docs/rendering/aa7-deterministic-paint-ordering-layering-rules.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`

## Purpose

Earlier AA issues made paint primitives and ordering explicit. AA8 adds a
reviewable visual regression surface over those concepts:

```text
PaintPhaseInput
  -> PaintInput
  -> PaintTree
  -> PaintPrimitive
  -> paint-operation debug snapshot
```

The snapshot is structural. It answers "what visual operations does the current
paint model intend to perform, and in what order?" without relying on platform
font rasterization, backend shape internals, texture identifiers, GPU handles,
or screenshot comparison.

## Ownership

Layout owns geometry, child order, inline fragment geometry, list marker
metadata, replaced-element layout metadata, and overflow clip metadata.

Paint owns construction of semantic paint primitives and serialization of those
primitives into deterministic paint-operation debug snapshots.

GFX/backend owns low-level immediate drawing execution. Backend objects such as
egui painters, shapes, texture identifiers, and platform font details are not
part of the AA8 snapshot contract.

Browser/runtime orchestration may expose paint-owned debug snapshots as
orchestration/debug plumbing, but it must not define paint semantics or
reinterpret paint ordering.

## Snapshot API

The paint-owned API is:

```rust
PaintInput::to_operation_debug_snapshot() -> String
```

Every snapshot begins with a stable header:

```text
version: 1
paint-operation-snapshot
layout-root-id: ...
viewport-width: ...
document-rect: ...
```

Operation lines use deterministic indices, stable field names, fixed decimal
formatting for geometry, stable color formatting, explicit source identifiers,
and AA paint phase labels.

## Operation Model

AA8 normalizes the current primitive vocabulary into structural operations:

- `Background` -> `fill-rect` with `detail=background`
- `Border` -> visible physical edge `fill-rect` operations in top, right,
  bottom, left order
- `Outline` -> visible physical edge `fill-rect` operations in top, right,
  bottom, left order
- `ListMarker` -> `draw-list-marker`
- `Clip` -> `begin-clip` and `end-clip`
- `Text` -> `draw-text`
- `TextDecoration` -> `fill-rect` with `detail=text-decoration`
- `InlineBox` -> `inline-box`
- `Replaced` -> `replaced`

These operation names are Borrowser-owned debug vocabulary. They are not egui
draw commands and they are not a retained display list.

## Clip Scope

Overflow clipping is serialized explicitly as a scoped operation pair. For a
box with layout-owned `OverflowClip` metadata:

- the box's own background, border, and list marker appear before `begin-clip`;
- inline content and child subtrees appear between `begin-clip` and `end-clip`;
- the box's own outline appears after `end-clip`;
- descendant operations reached through child subtrees remain inside active
  ancestor clips.

This matches the AA6 and AA7 supported subset. The snapshot does not introduce
retained clip nodes, stacking contexts, or compositor layers.

## Determinism Invariants

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource-independent paint input, and input state:

- operation snapshot output is deterministic;
- operation order follows layout-owned traversal plus paint-owned AA sequencing;
- output is not sorted after construction to manufacture determinism;
- field order and field names are stable;
- floating-point geometry uses fixed decimal precision;
- colors use stable `rgba(r,g,b,a)` formatting;
- source identifiers include box id, node id, and anonymous-box state;
- phase labels come from `PaintOrderPhase`;
- image/replaced behavior is represented structurally, without serializing
  resource URLs, texture ids, or backend image state.

## Regression Coverage

AA8 tests cover representative paint-operation snapshots for:

- deterministic repeated snapshot output;
- backend-independent output that does not expose egui internals;
- backgrounds, borders, outlines, text, and underline text decorations;
- overflow clipping scope for contents and descendants;
- list markers and inline content;
- replaced image primitives as structural operations;
- operation ordering aligned with the AA7 supported paint order.

## Deliberate Exclusions

AA8 deliberately does not implement:

- pixel or raster screenshot comparison;
- platform-dependent image snapshots;
- egui shape or backend draw-command serialization;
- runtime draw-call recording;
- texture id, GPU handle, or platform font detail serialization;
- retained display lists;
- retained paint scenes;
- scene graphs;
- compositor or GPU abstractions;
- full CSS painting order;
- stacking contexts or `z-index`;
- scrollbars or scroll offset painting;
- new visual behavior.

Pixel/raster visual regression infrastructure and runtime draw-call recording
must be introduced by explicit follow-up issues with their own determinism
contracts.
