# P8: Integrate Selector Parsing Into Stylesheet Rule Model

Last updated: 2026-04-10  
Status: complete

This document records the implementation of Milestone P issue P8:
integrating selector parsing into the engine-facing stylesheet rule model.

Related code:
- `crates/css/src/model/mod.rs`
- `crates/css/src/model/entry.rs`
- `crates/css/src/model/serialize.rs`
- `crates/css/src/model/tests.rs`
- `crates/css/src/cascade.rs`
- `crates/css/tests/model_golden.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p3-selector-parser-core-subset.md`
- `docs/css/p5-invalid-selector-handling.md`
- `docs/css/p6-unsupported-selector-handling.md`
- `docs/css/p7-selector-serialization-debug-output.md`

## Implemented Result

Borrowser style rules now carry structured selector parse results in the model
layer.

`StyleRule` stores:

- `selectors: SelectorListParseResult`
- `declarations: DeclarationBlock`

Raw preserved selector component lists are no longer the primary rule-model
representation for style-rule selectors.

## Model Boundary

Selector integration now happens during syntax-to-model conversion:

- `css::syntax` still parses qualified-rule preludes structurally
- `css::model::entry` invokes `css::selectors::parse_selector_list(...)`
- style rules store the selector parse result directly
- later consumers use the structured selector result rather than reparsing raw
  selector source

This keeps selector parsing independent from matching while making selectors a
real part of the stylesheet pipeline.

## Parse Result In Rules

Style-rule selector state is represented as one of:

- `Parsed(SelectorList)`
- `Unsupported(UnsupportedSelectorList)`
- `Invalid(InvalidSelectorList)`

This means model-layer rules now preserve selector validity/support state
explicitly rather than hiding it behind preserved prelude text.

This is intentionally a heavier rule payload than a parsed-selectors-only
surface. For Milestone P, keeping `Parsed | Unsupported | Invalid` inside the
model is the correct tradeoff because later cascade and matching work still
needs explicit selector support-state and invalidation semantics.

## Serialization And Regression Surface

Model snapshots now serialize style-rule selectors through the selector parse
result contract.

Representative coverage includes:

- unit tests for parsed, unsupported, and invalid selector results on style
  rules
- stable rule serializer coverage using structured selector output
- updated model golden fixtures showing structured selector state inside style
  rules
- continued cascade-bridge coverage using model parse results

## Compatibility Bridge

The current cascade bridge remains intentionally limited and matching-scoped.

For Milestone P:

- it consumes `StyleRule::selectors` instead of reparsing raw selector source
- it only adapts the currently bridgeable simple parsed selectors needed by the
  existing temporary matcher
- unsupported, invalid, or non-bridgeable complex selectors do not participate
  in the current compatibility matcher
- compatibility matching still carries its own temporary specificity type and
  simple-selector matching logic; that duplication is intentional and remains
  isolated until Milestone Q replaces the bridge entirely

This preserves the existing matching scope while removing raw selector reparsing
from the engine-facing model path.

The bridge is therefore intentionally lossy and not spec-complete. That is
acceptable only because:

- the model preserves the full selector parse result without loss
- selector semantics remain owned by `css::selectors`
- Milestone Q is expected to replace the compatibility bridge with real
  selector matching over the structured IR

Selector parsing remains eager during model construction. That is an intentional
design choice for Milestone P: it keeps the stylesheet pipeline deterministic
and avoids lazy selector caches or deferred reparsing paths before the selector
boundary is fully established.

## Exit Criteria

P8 is complete when:

- style rules contain structured selector parse results
- preserved raw selector storage is no longer the primary style-rule selector
  representation
- syntax-to-model conversion populates selector parse results directly
- model integration tests and goldens pass
- selectors are ready for later matching work without revisiting the model
  boundary

Repository status:

- the P8 selector-model-integration issue is complete and may be treated as
  closed
- Milestone P selector parsing and model integration is complete
- Milestone Q can now start from structured model selectors rather than raw
  rule preludes
