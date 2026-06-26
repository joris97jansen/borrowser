# AD3: CSS-Wide Keyword Handling

Last updated: 2026-06-26
Status: implemented contract for Milestone AD issue 3

This document defines Borrowser's current CSS-owned handling for CSS-wide
keywords on supported properties.

Related code:

- `crates/css/src/values.rs`
- `crates/css/src/specified/css_wide.rs`
- `crates/css/src/specified/parse.rs`
- `crates/css/src/cascade/contract/declarations.rs`
- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/computed/document/materialize.rs`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad2-typed-core-css-value-model.md`
- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

CSS-wide keywords are declaration-level CSS semantics. They apply across
supported properties but are not ordinary property-specific values like
`display: block` or `color: red`.

AD3 introduces a shared CSS-owned representation and parser for:

- `initial`
- `inherit`
- `unset`
- `revert`
- `revert-layer`

The current implemented subset supports `initial`, `inherit`, and `unset`.
`revert` and `revert-layer` are centrally recognized but rejected as
unsupported because correct behavior depends on cascade origin and cascade
layer semantics beyond the current supported model.

## Representation

`CssWideKeyword` and `CssWideKeywordValue` live in `crates/css/src/values.rs`.

The specified layer exposes `SpecifiedDeclarationValue`, which can hold either:

- a property-specific `SpecifiedPropertyValue`;
- a CSS-wide keyword for one supported property.

This keeps CSS-wide keyword handling out of individual property parsers.
Property-specific modules such as `display`, `color`, `length`, `overflow`, and
`z_index` must not add ad hoc branches for `initial`, `inherit`, `unset`,
`revert`, or `revert-layer`.

## Parsing

CSS-wide parsing is centralized in `crates/css/src/specified/css_wide.rs`.

The declaration-level parser checks for CSS-wide keywords before dispatching to
property-specific parsers. Unknown properties are still rejected as unsupported
properties before value parsing can make them supported. For example,
`zoom: initial` remains an unsupported property.

Unsupported CSS-wide keywords produce
`SpecifiedValueParseErrorKind::UnsupportedCssWideKeyword`. This is distinct
from property-specific unsupported keywords so `revert` and `revert-layer` are
recognized-but-unsupported, not confused with ordinary invalid values.

## Cascade Behavior

Supported CSS-wide declarations remain supported declaration values through
normal candidate creation, ordering, and winner selection. They are not
resolved during token parsing or property-specific parsing.

When a CSS-wide value wins, `resolve_cascade_style(...)` resolves it through
property metadata:

- `initial` resolves to `property.initial_value()`.
- `inherit` resolves to explicit inheritance when a parent style is available.
- root or no-parent `inherit` resolves to the property's initial value.
- `unset` resolves like `inherit` for inherited-by-default properties when a
  parent style is available.
- `unset` resolves like `initial` for non-inherited properties and for
  inherited properties at the root.

Resolved style stores explicit CSS-wide provenance through
`ResolvedValueSource::CssWideKeyword(CssWideResolvedSource)`. This keeps
explicit `color: unset` distinguishable from ordinary missing-property
inheritance, and explicit `width: initial` distinguishable from ordinary
default fill.

## Computed-Style Materialization

Computed style consumes already-resolved CSS-wide sources:

- resolved CSS-wide initial sources materialize through
  `ComputedValue::from_initial(property)`;
- resolved CSS-wide inherited sources copy the parent computed property value.

The computed layer does not decide what `initial`, `inherit`, or `unset` mean.
That semantic choice belongs to the CSS cascade/resolved-style layer.

## Unsupported Behavior

`revert` and `revert-layer` are not approximated as `initial`, `inherit`, or
`unset`.

They remain unsupported until Borrowser has the cascade-origin and cascade-layer
model needed to implement them correctly.

## Invariants

- CSS-wide parsing is centralized in CSS-owned shared infrastructure.
- Supported CSS-wide keywords participate in cascade winner selection.
- Property-specific parsers do not implement CSS-wide keyword branches.
- Unknown properties do not become supported because their value is CSS-wide.
- CSS-wide resolution uses CSS-owned property metadata only.
- Browser/runtime, Layout, Paint, and GFX do not interpret CSS-wide keyword
  semantics.
- Debug output preserves explicit CSS-wide provenance.

## Test Surface

AD3 is covered by tests for:

- shared declaration-level CSS-wide parsing;
- deterministic rejection of unsupported CSS-wide keywords;
- CSS-wide values participating in cascade candidates and winner resolution;
- unknown properties with CSS-wide values remaining unsupported;
- resolved-style provenance for explicit CSS-wide initial/inherited behavior;
- computed materialization of resolved CSS-wide initial/inherited sources.
