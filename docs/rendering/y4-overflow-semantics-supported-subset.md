# Y4: Overflow Semantics Supported Subset

Last updated: 2026-05-09  
Status: implemented overflow semantics for Milestone Y issue 4

Y4 defines the first end-to-end overflow contract across CSS, layout, and paint.
The goal is not full scrolling or all CSS overflow-axis behavior. The goal is a
deterministic, layout-owned model that later scrolling, clipping, hit testing,
formatting-context, and stacking work can build on without reinterpreting raw
CSS declarations in downstream phases.

## Ownership

CSS owns:

- parsing the supported `overflow` shorthand keyword
- cascade, inheritance, and initial/default selection
- computed-value normalization to the canonical `css::Overflow` enum
- future `overflow-x` / `overflow-y` parsing and computed-value axis coupling

Layout owns:

- mapping canonical computed overflow values to `OverflowPolicy`
- storing the policy on `LayoutBox`
- deriving `OverflowClip` metadata from the policy and final layout geometry
- exposing deterministic debug labels for layout and paint phase boundaries
- deciding layout effects that follow from overflow policy

Paint owns:

- consuming layout-provided `OverflowClip` metadata
- applying the clip to descendant/content painting
- avoiding raw CSS overflow interpretation

Paint must not decide whether a box clips, creates a scroll container, or
establishes an independent formatting context. Those are layout-owned
semantics.

## Supported CSS Surface

The supported CSS surface is the single-keyword `overflow` shorthand:

- `visible`
- `hidden`
- `clip`
- `scroll`
- `auto`

Unsupported values are rejected at specified-value parsing. Until
`overflow-x` and `overflow-y` exist, layout receives one canonical keyword and
materializes a uniform inline/block `OverflowPolicy`.

## Policy Semantics

`OverflowPolicy` is layout-owned vocabulary. It is not a raw CSS declaration
representation.

The current keyword effects are:

- `visible`: no paint clip, no scroll container, no independent formatting context
- `hidden`: paint clip, scroll container, independent formatting context contract
- `clip`: paint clip, no scroll container, no independent formatting context
- `scroll`: paint clip, scroll container, independent formatting context contract
- `auto`: paint clip, scroll container, independent formatting context contract

The independent-formatting-context result is materialized as generated-box and
layout metadata for block/list-item boxes. Full BFC behavior for float
containment, parent/child margin-collapse boundaries, and fragmentation is
deferred.

Overflow effects apply only to boxes whose current layout participation can
produce stable clipping geometry: root boxes, block-level boxes, and atomic
inline boxes. Ordinary non-replaced inline boxes retain their overflow policy
for debug and future layout work, but they do not produce `OverflowClip`
metadata in Y4.

## Layout Sizing

Overflow does not inflate used width or height in the supported subset. A box
with fixed width/height and clipped overflowing descendants keeps the same used
border-box size as a visible-overflow box.

Auto block-size continues to follow the existing normal-flow content
contribution model. Scrollable overflow dimensions, scroll offsets, scrollbar
reservation, and scrollport sizing are deferred.

The layout invariant is:

```text
used box geometry is computed by normal layout;
overflow policy derives clipping/scroll-container semantics from that geometry;
overflowing descendant visual content does not retroactively change used size.
```

## Paint Clipping

`OverflowClip` is derived from `OverflowPolicy` and the box's final border-box
rectangle. The current no-border subset clips to the border box, which is
equivalent to the padding box until border geometry exists.

Paint applies this clip to the box's inline content and descendant subtree.
Background painting remains tied to the box's own border-box geometry. List
marker behavior remains the existing marker paint behavior and is not a Y4
scrolling/overflow feature.

The paint invariant is:

```text
paint consumes OverflowClip;
paint does not inspect raw CSS overflow values;
paint does not create positioning or scrolling geometry.
```

## Anonymous Boxes

Anonymous generated boxes expose visible overflow for Y4. They do not inherit
or reinterpret their anchor node's authored overflow value. This keeps generated
box semantics deterministic and prevents anonymous wrappers from introducing
unexpected clips.

## Debug And Determinism

Layout debug snapshots include:

- `policy=(inline=<keyword> block=<keyword>)`
- `clip=none` for non-clipping policies
- `clip=x=<px> y=<px> w=<px> h=<px>` for clipping policies

Paint phase input snapshots reuse the layout debug surface, so the
layout-to-paint handoff exposes the exact overflow policy and clip rectangle
paint will consume.

The output is deterministic for a fixed DOM, style tree, viewport, and
measurer.

## Deferred Work

Y4 deliberately defers:

- `overflow-x` and `overflow-y`
- computed-value axis coupling for mixed visible/clip/scroll values
- scrollbar UI, scrollbar gutter, and scrollbar sizing effects
- scroll offset storage and scrollable overflow dimensions
- root element / body overflow propagation to the viewport
- viewport scrolling behavior
- hit-test clipping
- full overflow-created BFC behavior
- float containment and clearance interaction
- fragmentation
- stacking context and z-order interaction
- writing-mode-specific physical clip mapping
