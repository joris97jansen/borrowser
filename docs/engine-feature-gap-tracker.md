# Engine Feature Gap Tracker

Status: living tracker

This is a small, non-normative checklist for visible engine gaps. Detailed
contracts still live in the milestone docs under `docs/css/`, `docs/rendering/`,
and `docs/html5/`.

Use this file to answer: "what major browser/CSS features are still missing?"

## Flexbox

Current supported subset:

- `display: flex`
- block-level flex containers
- direct generated in-flow children as flex items
- row-only, single-line layout
- internal default grow/shrink/basis behavior
- default cross-axis stretch behavior for auto-height items

Missing:

- `inline-flex`
- authored `flex-direction`, including column and reverse directions
- authored `flex-wrap` and multi-line layout
- `flex-flow`
- `justify-content`
- authored `align-items`, `align-self`, and `align-content`
- `gap`, `row-gap`, and `column-gap`
- `order`
- authored `flex`, `flex-grow`, `flex-shrink`, and `flex-basis`
- baseline alignment
- full min-content/max-content flexbox behavior
- flex-specific paint behavior

Notes:

- Paint does not implement flexbox. Layout computes flex geometry; paint only
  consumes final layout rectangles.
- Unsupported flex behavior is tracked in
  `docs/rendering/z6-flex-unsupported-feature-handling.md`.

## CSS Property Coverage

Current supported property set is intentionally small. Major missing families:

- borders:
  - full `border` shorthand support
  - `border-width`, `border-style`, and `border-color` shorthands
  - additional border styles beyond the supported subset
  - border radius
  - border images
  - logical border properties
- CSS shorthand and multi-component declaration expansion:
  - broad shorthand parsing is not yet supported
  - declarations that expand into multiple longhands are intentionally limited
  - current feature work should prefer explicit longhands unless shorthand
    support is part of the issue scope
- fonts: `font-family`, `font-weight`, `font-style`, line-height variants
- text: `white-space`, text alignment, text decoration, text transform
- backgrounds: images, repeat, position, size, attachment, multiple backgrounds
- box effects: shadows, opacity, transforms, filters
- layout: floats, clear, grid, table layout, multi-column layout
- positioning: complete absolute/fixed/sticky geometry and z-index/stacking
- sizing: full intrinsic sizing keywords and browser-compatible min/max nuance
- overflow: scrollbars, scroll containers, overflow-x/y split behavior
- selectors/media: broad selector coverage, media queries, pseudo-classes,
  pseudo-elements
- custom properties and variables
- animations and transitions

## Layout

Missing or incomplete:

- CSS Grid
- table layout
- floats and clear
- full positioned layout geometry
- stacking contexts and z-index ordering
- writing modes and logical-axis remapping
- fragmentation and pagination
- full inline formatting behavior, including bidi and advanced line breaking
- full replaced-element and intrinsic-size compatibility
- margin/border/padding completeness across all formatting contexts and edge
  cases

## Paint / GFX

Missing or incomplete:

- full border rendering beyond the supported physical solid-border subset
- border radius
- background images and advanced background painting
- shadows, transforms, opacity, filters, and clipping interactions
- stacking-context paint ordering
- scrollbars and scrollable overflow painting
- selection painting outside supported text-control paths
- font fallback and advanced text shaping
- flex/grid/table-specific paint behavior where future layout data requires it

## Browser Runtime / Platform

Missing or incomplete:

- JavaScript execution and DOM bindings
- event loop, timers, microtasks, and script-triggered invalidation
- full DOM mutation APIs
- full form submission and navigation behavior
- focus management and keyboard navigation
- origin/security policy model
- storage, cookies, history, and session behavior
- resource loading for fonts, media, and broader image formats
- accessibility tree

## HTML / DOM

Missing or incomplete:

- full DOM API surface
- script integration
- form algorithms beyond current control-state support
- full editing/contenteditable behavior
- full custom element/template/shadow DOM behavior

## How To Update

When a feature lands:

1. Remove or narrow the item here.
2. Link to the relevant contract doc if the behavior is subtle.
3. Keep this file short; move detailed rules to subsystem docs.
