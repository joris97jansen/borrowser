# S7 - Structured ComputedStyle Representation

## Status

Implemented.

## Runtime Contract

`ComputedStyle` is the primary runtime CSS contract for layout, paint, and
browser-facing consumers. It is a total, normalized style for the supported CSS
property subset. Consumers must not parse authored CSS text, inspect cascade
winners, or recover from invalid declarations after this point.

The only supported construction paths are:

- `ComputedStyle::initial()`
- `ComputedStyle::from_resolved_style(...)`
- `compute_style_from_resolved_style(...)`
- `compute_document_styles(...)`
- `ComputedStyleBuilder`

The legacy `compute_style(...)` bridge also returns `ComputedStyle`, but it is
compatibility-only for callers that still provide DOM-attached string
declarations. It resolves supported declarations through the same property id,
specified-value, computed-value, and `ComputedStyle` invariant path before
applying temporary HTML/UA defaults. New runtime code should use the structured
resolved-style pipeline above.

Only valid supported authored declarations affect computed style or suppress
bridge-era HTML/UA defaults. Unsupported declarations and invalid supported
values are ignored before computed-style assembly.

The public fields are intentionally private. Downstream code reads values
through typed accessors such as `color()`, `font_size()`, `box_metrics()`, and
`width()`, or through the generic `get(PropertyId)` / `entries()` property
surface.

## Representation

The internal representation is optimized for current runtime consumers while
remaining lossless over the supported property registry:

- scalar runtime fields for frequently consumed values, such as colors,
  display, font size, and sizing properties
- a `BoxMetrics` grouping for margin and padding properties
- generic property access materialized deterministically from the same stored
  fields

`BoxMetrics` is an ergonomic projection, not a second property universe.
Grouped storage remains valid only while every supported margin and padding
property round-trips through `ComputedStyle::get(...)`.

## Determinism

`ComputedStyle::entries()` iterates in registry order. This keeps computed-style
snapshots and downstream enumeration independent from map order, struct field
order, or caller insertion order.

`ComputedStyleBuilder` remains the invariant gate:

- every supported property must be recorded exactly once
- recorded values must match the property's computed-value kind
- final assembly is total over the registry

`ComputedStyle::with_property(...)` exists for focused tests and bridge code
that need to adjust one property. It rebuilds through the same builder path
instead of exposing field mutation.

## Invalid Values

Invalid authored declarations are rejected before cascade winner resolution
according to property metadata. `ComputedStyle` never stores invalid values and
does not perform post-layout or post-paint fallback recovery.

## Scope

This issue does not add new CSS property coverage or layout-dependent computed
semantics. Percentages, relative units, CSS-wide keywords, and richer shorthand
expansion remain outside this S7 representation work.
