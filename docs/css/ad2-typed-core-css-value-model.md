# AD2: Typed Core CSS Value Model

Last updated: 2026-06-25
Status: implemented contract for Milestone AD issue 2

This document defines Borrowser's CSS-owned reusable core value model for the
currently supported and near-term property parsing subset. AD2 does not
implement full CSS Values and Units coverage, new property families,
shorthands, CSS-wide keywords, resource loading, invalidation classification,
or runtime dirty-state integration.

Related code:

- `crates/css/src/values.rs`
- `crates/css/src/specified/core.rs`
- `crates/css/src/specified/value.rs`
- `crates/css/src/specified/parse.rs`
- `crates/css/src/specified/{border,color,display,length,outline,overflow,position,text_decoration,z_index}.rs`
- `crates/css/src/computed/normalize.rs`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/s3-parsed-specified-value-representations.md`
- `docs/css/s4-computed-value-representations-normalization.md`
- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/css/syntax-parser-contract.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

AD2 gives supported property parsers one shared CSS-owned primitive value
surface instead of letting each property parser invent its own representation
for identifiers, numbers, integers, lengths, percentages, colors, URLs,
strings, and functions.

The value model is intentionally narrow. It supports the categories needed by
the current property subset and leaves broad CSS syntax families explicitly
unsupported until separate issues add them with tests and docs.

## Layer Boundaries

Borrowser keeps value layers separate:

| layer | representative types | owner | meaning |
| --- | --- | --- | --- |
| syntax tokens/component values | `CssToken`, `CssComponentValue` | `css::syntax` | Parser/tokenizer output. Not property-aware. |
| model declaration values | `DeclarationValue`, `ValueComponent`, `ValueToken` | `css::model` | Source-preserving declaration value components. Still not valid property values. |
| reusable core CSS values | `CssKeywordValue`, `CssNumberValue`, `CssLengthValue`, `CssColorValue`, etc. | `css::values` | CSS-owned primitives extracted from model components and validated for the current supported subset. |
| property specified values | `SpecifiedValue`, `SpecifiedLengthPercentageOrAuto`, property keyword enums | `css::specified` | Property-aware values after the registry-selected parser accepts a declaration. |
| computed values | `ComputedValue`, `Length`, `Percentage`, `LengthPercentage` | `css::computed` / `css::values` | Runtime-facing normalized values consumed by layout, paint, and browser orchestration. |

Parser-level tokens and model components must not be treated as specified or
computed values. Runtime-facing computed values must not carry parser tokens,
spans, or raw authored CSS strings.

## Core Value Types

`crates/css/src/values.rs` is the canonical home for reusable CSS value
primitives.

Current AD2 primitive types:

- `CssKeywordValue`: ASCII-lowercase identifier/keyword value. Property
  parsers decide which keywords are supported.
- `CssNumberScalar` and `CssNumberValue`: finite numeric scalar plus
  deterministic authored numeric representation.
- `CssIntegerValue`: range-validated integer primitive for current `i32`
  integer needs.
- `CssLengthUnit` and `CssLengthValue`: current `px` and unitless-zero length
  subset.
- `CssPercentageValue`: authored percentage scalar. Computed normalization
  later converts it to the runtime `Percentage` fraction.
- `CssLengthPercentageValue`: current `<length-percentage>` subset used by
  supported sizing properties.
- `CssColorKeyword`, `CssHexColor`, `CssColorSyntax`, and `CssColorValue`:
  supported color keyword and 3/6-digit hex subset.
- `CssUrlValue`: authored URL wrapper only. It does not resolve URLs, apply a
  base URL, handle origins, fetch resources, load images, or interact with the
  network.
- `CssStringValue`: authored string wrapper only.
- `CssFunctionValue`: authored function wrapper for future property work. AD2
  does not accept functions for any currently supported property; current
  property parsers reject function components deterministically.

Property-level wrappers such as `SpecifiedColor`, `SpecifiedLength`,
`SpecifiedPercentage`, and `SpecifiedZIndex` now wrap or delegate to these core
value types. Property-specific keyword enums remain property-specific because
they define each supported property's grammar.

## Supported Value Categories

AD2 supports only the current property parser subset:

- identifiers/keywords for supported keyword-only properties;
- finite numbers as the source primitive for lengths, percentages, and
  integers;
- `i32` integers for `z-index`;
- `px` lengths and unitless zero where the property grammar allows it;
- percentages only for currently supported sizing values;
- length-percentage branches for `width`, `height`, `min-width`, and
  `max-width`;
- supported color keywords and 3/6-digit hex colors;
- URL and string wrappers as CSS-owned authored primitives, with no accepting
  property grammar in AD2;
- function detection for deterministic rejection.

## Unsupported Value Categories

AD2 intentionally does not support:

- full CSS Values and Units;
- relative units such as `em`, `rem`, `vh`, `vw`, `pt`, or `cm`;
- broad color syntax such as `rgb()`, `hsl()`, color spaces, alpha notation,
  or named colors beyond the current subset;
- `calc()`, `var()`, `env()`, gradients, transforms, filters, or broad
  function syntax;
- broad URL grammar semantics, URL resolution, fetching, image loading, origin
  handling, or network behavior;
- shorthands, CSS-wide keywords, custom properties, media queries, animations,
  or invalidation classification.

Unsupported or malformed syntax is rejected deterministically by the
specified-value parser. Unsupported functions, URLs, and strings are not
preserved as accepted opaque values for current properties.

## Invariants

- CSS owns typed value parsing and property semantics.
- Layout, paint, GFX, and browser/runtime consume computed CSS output only and
  must not parse CSS values.
- Core values are built from `css::model` components, not by reparsing
  serialized CSS strings.
- Property metadata still selects the specified-value shape and computed-value
  shape.
- Property-specific keyword enums remain the grammar boundary for each
  supported property.
- Computed-value normalization remains separate from parser-level and
  specified-value concerns.
- Invalid or unsupported supported-property values do not enter cascade
  winner resolution or computed style.

## Extension Points

Future issues may add supported URL-valued properties, string-valued
properties, CSS-wide keywords, shorthands, relative units, broader color
syntax, or function grammars. Those issues must extend the CSS-owned core
value model, property specified values, computed normalization, docs, and tests
together. They must not move CSS value parsing into layout, paint, GFX, or
browser/runtime.
