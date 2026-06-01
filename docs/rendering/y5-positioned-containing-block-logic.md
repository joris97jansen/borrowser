# Y5: Positioned Containing-Block Logic

Last updated: 2026-05-31  
Status: implemented containing-block relationships for positioned elements

Y5 establishes the layout-owned relationship between CSS positioned boxes and
the generated box that provides their positioned containing block. It does not
implement final absolute, fixed, relative, or sticky geometry.

## Ownership

CSS owns parsing, cascade, and computed-value normalization for the supported
`position` keywords:

- `static`
- `relative`
- `absolute`
- `fixed`
- `sticky`

Layout maps computed `css::Position` into `PositioningScheme`, records normal
or out-of-flow participation, and resolves a frame-local
`PositionedContainingBlockId`.

Paint must consume final layout geometry. It must not rediscover positioned
containing blocks from DOM ancestry or CSS declarations.

## Resolution Rules

The existing `ContainingBlockId` remains the normal-flow containing block used
for sizing and in-flow placement. Y5 adds `PositionedContainingBlockId` because
positioned layout can resolve against a different ancestor.

The supported positioned containing-block rules are:

- `static`, `relative`, and `sticky` use the normal-flow containing block for
  their current in-flow geometry.
- `relative`, `absolute`, `fixed`, and `sticky` establish positioned containing
  blocks for positioned descendants.
- `absolute` resolves against the nearest generated ancestor whose
  `establishes_positioned_containing_block` metadata is true.
- `absolute` falls back to the initial containing block when no positioned
  ancestor exists.
- `fixed` resolves against the initial containing block in the current subset.
- The document/root box represents the initial containing block for Y5 lookup.

## Flow Participation

`PositioningScheme::flow_participation()` records the layout family:

- `static`, `relative`, and `sticky` remain in normal flow.
- `absolute` records `out-of-flow:absolute`.
- `fixed` records `out-of-flow:fixed`.

Y5 removes absolute and fixed boxes from parent normal-flow contribution, but
does not yet compute their final out-of-flow geometry. Static-position capture,
out-of-flow queues, inset resolution, and final positioned painting are
deferred to later Milestone Y issues.

Y6 formalizes the next handoff by tracking absolute and fixed boxes in a
deterministic layout-phase out-of-flow participant registry. That registry
consumes the `PositionedContainingBlockId` resolved here; it does not recompute
containing-block relationships.

## Determinism

Resolution is deterministic for a fixed generated box tree:

- generated boxes receive deterministic preorder `BoxId`s
- positioned containing-block lookup walks generated ancestors only
- fallback to the initial containing block is explicit
- anonymous boxes are always `position: static`

## Deferred Work

Y5 deliberately defers:

- final relative visual offsets
- absolute/fixed/sticky layout geometry
- out-of-flow queue construction
- static-position capture
- inset properties
- transformed containing blocks
- viewport-specific fixed positioning
- root/body viewport propagation interactions
