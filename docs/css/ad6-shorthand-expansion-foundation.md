# AD6: Shorthand Expansion Foundation

Last updated: 2026-06-28
Status: implemented contract for Milestone AD issue 6

This document defines Borrowser's CSS-owned shorthand expansion foundation and
the first narrow supported shorthand subset: `outline`.

AD6 does not add broad shorthand coverage. It deliberately excludes `border`,
`background`, `font`, `margin`, `padding`, `text-decoration`, custom
properties, `var(...)`, `calc(...)`, complete CSS-wide keyword behavior for
`revert`/`revert-layer`, and unsupported values outside the current longhand
parsers.

Related code:

- `crates/css/src/properties/shorthand.rs`
- `crates/css/src/specified/shorthand.rs`
- `crates/css/src/cascade/integration/declarations.rs`
- `crates/css/src/cascade/contract/declarations.rs`
- `crates/css/src/cascade/contract/snapshot.rs`
- `crates/css/src/computed`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad2-typed-core-css-value-model.md`
- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/rendering/aa4-outline-rendering-box-decoration.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

Shorthand expansion is CSS-owned. A supported shorthand declaration must become
ordinary registered longhand declarations before longhand metadata, specified
values, cascade winners, computed values, or invalidation impact are consumed.

This keeps Layout, Paint, GFX, and Browser/runtime unaware of shorthand syntax.
Those downstream systems continue to consume computed CSS values, layout data,
and paint inputs only.

## Registry

`css::properties` now has a supported shorthand registry separate from the
supported longhand registry:

- `ShorthandId`
- `ShorthandRegistration`
- `ShorthandRegistry`
- `shorthand_registry()`

`ShorthandId` names supported shorthands. `PropertyId` continues to name only
supported longhands. A shorthand must not be inserted into the longhand
registry merely because its name is recognized.

For AD6, the only supported shorthand is:

```text
outline -> outline-color, outline-style, outline-width
```

That longhand order is deterministic and is the normative expansion order for
debug output and tests.

## Expansion Pipeline

The supported pipeline is:

```text
model::Declaration
  -> shorthand lookup
  -> raw longhand DeclarationValue expansion
  -> existing CascadeSpecifiedValue::parse(longhand, value)
  -> cascade candidates
  -> resolved style
  -> computed values
  -> CSS-owned invalidation impact comparisons
```

Shorthand parsing emits raw longhand declaration values. Cascade integration
then feeds each emitted value through the same longhand property registry and
specified-value parser used by authored longhands. This prevents shorthand
support from bypassing longhand metadata, CSS-wide keyword handling, computed
normalization, or invalidation impact metadata.

## Atomic Rejection

Shorthand expansion is atomic.

If a shorthand declaration contains an invalid, unsupported, duplicate, or
ambiguous component, it emits no longhands. Cascade records the declaration as
an invalid shorthand value for deterministic debug visibility, but it produces
no cascade candidates.

This prevents partially applied values such as accepting an `outline-style`
component while silently dropping an invalid `outline-width` component from the
same shorthand.

## Omitted Components

For AD6, omitted `outline` components reset their corresponding longhands by
emitting internal longhand `initial` values.

For example:

```text
outline: solid
```

expands internally as:

```text
outline-color: initial
outline-style: solid
outline-width: initial
```

This is an internal shorthand reset representation. It does not mean the author
literally wrote those longhand declarations, and it does not create a new
public CSSOM serialization contract.

## Ordering

All longhands emitted by one shorthand preserve the authored declaration's
source and authored declaration order. `expansion_order` is recorded only as a
deterministic source/debug-order fact among emitted longhands from the same
shorthand.

Cascade precedence remains controlled by authored declaration order, rule
order, specificity, origin, and importance. `expansion_order` is not a new
cascade precedence layer.

Examples:

```text
outline-width: 4px;
outline: solid;
```

The later shorthand wins for `outline-width` and resets it to initial.

```text
outline: solid;
outline-width: 4px;
```

The later authored longhand wins for `outline-width`.

## CSS-Wide Keywords

The supported AD3 CSS-wide subset works for supported shorthands:

- `outline: initial`
- `outline: inherit`
- `outline: unset`

These expand to the same CSS-wide keyword on every constituent longhand and
then flow through the existing longhand CSS-wide parsing and cascade
resolution.

`revert` and `revert-layer` remain recognized but unsupported because the
current cascade origin/layer model cannot implement them correctly.

## Invariants

- Shorthand recognition, parsing, expansion, and ordering are owned by CSS.
- Supported shorthand names are registered separately from supported longhands.
- Expanded longhands always use registered `PropertyId` longhands.
- Expanded longhands flow through the existing longhand specified-value parser.
- Invalid shorthand declarations emit no longhands.
- Unsupported shorthand syntax is rejected deterministically.
- Omitted shorthand components reset through internal longhand `initial`
  values.
- Layout, Paint, GFX, and Browser/runtime do not parse or infer shorthand
  behavior.
- Debug output is deterministic and distinguishes authored declaration order
  from shorthand expansion order.

## Deliberate Exclusions

AD6 deliberately excludes:

- `border` and border component shorthands;
- `background`, `font`, `margin`, `padding`, and `text-decoration`
  shorthands;
- `outline-offset`;
- `outline: auto`;
- dashed, dotted, double, inset, outset, groove, and ridge outline styles;
- color functions such as `rgb(...)`;
- relative units and `calc(...)`;
- custom properties and `var(...)`;
- CSSOM shorthand serialization;
- Layout, Paint, GFX, or Browser/runtime shorthand handling.
