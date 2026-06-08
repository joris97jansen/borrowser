# Z6: Flex Unsupported Feature Handling

Last updated: 2026-06-07
Status: implemented unsupported-feature contract for Milestone Z issue 6

This document records how Borrowser handles flexbox features outside the
current Milestone Z production subset. Z6 does not add authored flexbox
behavior. It makes unsupported behavior explicit at the subsystem boundary
where it belongs and pins that behavior with deterministic tests.

Related code:

- `crates/css/src/properties/data.rs`
- `crates/css/src/properties/tests.rs`
- `crates/css/src/specified/display.rs`
- `crates/css/src/specified/tests.rs`
- `crates/layout/src/box_tree/tests/display.rs`
- `crates/layout/src/box_tree/tests/projection.rs`
- `crates/layout/src/flex.rs`
- `crates/layout/src/inline/refine.rs`
- `crates/layout/src/lib.rs`

Related documents:

- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/rendering/z1-flex-layout-architecture-contract.md`
- `docs/rendering/z2-flex-box-tree-structure.md`
- `docs/rendering/z3-flex-main-axis-layout-core-subset.md`
- `docs/rendering/z4-flex-cross-axis-layout-core-subset.md`
- `docs/rendering/z5-flex-layout-integration-hardening.md`
- `docs/rendering/w3-display-to-box-generation-behavior.md`
- `docs/rendering/x10-sizing-invariants-extension-hooks.md`
- `docs/rendering/y9-advanced-flow-invariants-extension-points.md`

## Current Production Subset

The current Milestone Z production subset supports:

- `display: flex`
- block-level flex container behavior
- direct generated in-flow children as flex items
- row-only layout
- single-line layout
- internal default `flex-grow: 0`
- internal default `flex-shrink: 1`
- internal `flex-basis: auto` behavior routed through the sizing subsystem
- default supported cross-axis stretch behavior for auto-height items
- integration with existing sizing, flow, out-of-flow, and debug systems

No authored flex property other than `display: flex` is supported in the
current CSS property system.

## Boundary Handling Policy

Unsupported flex behavior is handled at one of four boundaries.

| boundary | examples | handling |
| --- | --- | --- |
| CSS unsupported property | `flex-direction`, `flex-wrap`, `flex-flow`, `justify-content`, `align-items`, `align-self`, `align-content`, `gap`, `row-gap`, `column-gap`, `order`, `flex`, `flex-grow`, `flex-shrink`, `flex-basis` | Ignored by the CSS property registry and cascade as unsupported properties. They do not enter computed style and cannot affect layout. |
| CSS unsupported value | `display: inline-flex`, `display: grid` | Rejected by specified-value parsing through the existing unsupported or invalid value fallback path. They do not create generated flex containers or deferred layout modes. |
| Layout unsupported internal mode | column axes, reverse axes, wrapping, multi-line layout, authored alignment, authored gaps, order sorting, baseline alignment | Unrepresentable or not constructible in the Milestone Z production layout path. Production layout constructs row/block-axis internal inputs from generated in-flow flex items and defaults only. |
| Paint/browser runtime | any missing flex behavior | Downstream only. Paint consumes final layout geometry. Browser/runtime owns invalidation and frame lifecycle. Neither repairs or emulates unsupported flex features. |

## Unsupported Feature Matrix

| feature | boundary | result |
| --- | --- | --- |
| `inline-flex` | CSS unsupported value | Rejected as an unsupported `display` keyword; computed `display` falls back through the existing CSS fallback path. |
| `flex-direction` | CSS unsupported property | Ignored before computed style. Layout remains row-only. |
| `flex-wrap` | CSS unsupported property | Ignored before computed style. Layout remains single-line. |
| `flex-flow` | CSS unsupported property | Ignored before computed style. |
| `justify-content` | CSS unsupported property | Ignored before computed style. Main-axis packing remains the current start/default behavior. |
| `align-items` | CSS unsupported property | Ignored before computed style. Cross-axis behavior remains the current default stretch subset. |
| `align-self` | CSS unsupported property | Ignored before computed style. |
| `align-content` | CSS unsupported property | Ignored before computed style. Multi-line cross-axis distribution is not constructible. |
| `gap`, `row-gap`, `column-gap` | CSS unsupported property | Ignored before computed style. Layout does not synthesize spacing. |
| `order` | CSS unsupported property | Ignored before computed style. Flex items remain in deterministic generated child order. |
| `flex` shorthand | CSS unsupported property | Ignored before computed style. |
| `flex-grow` | CSS unsupported property | Ignored before computed style. Production inputs keep internal grow `0`. |
| `flex-shrink` | CSS unsupported property | Ignored before computed style. Production inputs keep internal shrink `1`. |
| `flex-basis` | CSS unsupported property | Ignored before computed style. Production basis remains internal auto-basis behavior. |
| column or reverse layout | Layout unsupported internal mode | Not represented in production layout. No block-layout approximation is used. |
| wrapping or multi-line layout | Layout unsupported internal mode | Not represented in production layout. Overflow/wrapping behavior is not synthesized. |
| authored alignment | Layout unsupported internal mode | Not represented in production layout because the authored CSS properties are not supported. |
| authored gaps | Layout unsupported internal mode | Not represented in production layout because gap properties are not supported. |
| order sorting | Layout unsupported internal mode | Not represented in production layout. Generated child order is preserved. |
| baseline alignment | Layout unsupported internal mode | Not represented in production layout. |

## Layout Internal Shape

`crates/layout/src/flex.rs` deliberately exposes only the internal axes that
the current implementation can produce:

- `FlexMainAxis::Row`
- `FlexCrossAxis::Block`

The production adapter in `crates/layout/src/inline/refine.rs` constructs
`FlexItemMainAxisInput::default_row_auto_basis(...)` for each generated
in-flow flex item and constructs default row cross-axis stretch inputs. This is
not authored CSS support. It is the deterministic internal default behavior
for the implemented subset.

Future issues that add authored flex properties must extend CSS parsing,
cascade, computed style, layout inputs, docs, and tests together. They must not
reinterpret ignored declarations retroactively inside layout.

## Regression Coverage

Z6 is covered by targeted deterministic tests for:

- unsupported authored flex property names not being registered CSS
  properties
- `display: inline-flex` being rejected as an unsupported display value
- unsupported `display` values not reaching box generation as flex containers
- unsupported authored flex declarations not changing the current row-only,
  single-line layout behavior, generated child order, default factors, or
  default cross-axis behavior
- internal flex axes remaining row/block for the current algorithms

## Invariants

- CSS owns unsupported property and unsupported value filtering.
- Layout consumes computed style and generated box-tree metadata only.
- Unsupported authored flex declarations do not appear in computed style.
- Unsupported display values do not create fake layout behavior.
- Layout does not approximate column, reverse, wrap, gap, order, baseline, or
  authored alignment behavior.
- Paint and browser/runtime remain downstream consumers.
- Debug surfaces record retained layout decisions; they are not used to infer
  unsupported behavior.

## Deliberate Exclusions

Z6 does not implement:

- `inline-flex`
- authored `flex-direction`
- authored `flex-wrap`
- authored `flex-flow`
- authored `justify-content`
- authored `align-items`, `align-self`, or `align-content`
- `gap`, `row-gap`, or `column-gap`
- `order`
- authored `flex`, `flex-grow`, `flex-shrink`, or `flex-basis`
- column, reverse, wrapping, multi-line, baseline, gap, or order-sorting layout
- paint-time or browser/runtime flex emulation

These features require separate issues that introduce their CSS and layout
contracts explicitly.
