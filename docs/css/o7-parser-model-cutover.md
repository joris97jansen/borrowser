# O7: Migrate Parser Output To The New CSSOM/Rule Model

Last updated: 2026-04-08  
Status: implemented

This document records the Milestone O cutover that makes Borrowser's
engine-facing CSS model the default stylesheet parse product.

Related code:
- `crates/css/src/lib.rs`
- `crates/css/src/model/entry.rs`
- `crates/css/src/cascade.rs`
- `crates/browser/src/page.rs`
- `crates/css/tests/model_cutover.rs`

Related documents:
- `docs/css/o6-canonical-model-serialization.md`
- `docs/css/o8-cssom-contract-and-legacy-retirement.md`

## Implemented Result

The stylesheet parser pipeline is now model-first:

- crate-root stylesheet parse entrypoints (`css::parse_stylesheet`,
  `css::parse_stylesheet_with_options`) now produce the engine-facing
  `css::model` parse result
- syntax-layer stylesheet parse entrypoints remain available explicitly under
  `css::syntax::...` and via root aliases such as
  `css::parse_syntax_stylesheet_with_options`
- the browser page pipeline now stores model parse artifacts rather than a
  compatibility stylesheet as its primary CSS representation
- `attach_styles` now consumes model parse artifacts directly and builds its
  migration-only compatibility view internally at the current cascade boundary

This means the structured rule/declaration/value model is no longer
aspirational. It is the effective stylesheet handoff contract produced by the
main parser pipeline.

## Why This Exists

Milestones O2 through O6 defined the engine-facing CSS model and its stable
serialization/debug contracts, but until O7 the main browser pipeline still
treated compatibility/raw-string outputs as the primary stylesheet product.

That was no longer acceptable once the model existed:

- later selector and cascade work needs a real model-owned handoff
- the browser should not store `CompatStylesheet` as its canonical stylesheet
  state
- compatibility projection should happen only at the migration boundary that
  still needs it

## Delivered Changes

- flipped the crate-root stylesheet parse API to the model-layer parse result
- preserved explicit syntax-layer access for syntax tests and parser work
- updated the browser page state to store ordered model stylesheet parses
- moved compatibility projection for the current cascade path into
  `css::cascade`
- kept rule ordering deterministic across multiple stored stylesheets by
  preserving stylesheet insertion order and per-stylesheet rule order
- added cutover tests covering the root parse API and parser-to-model-to-cascade
  flow

## Current Boundary

After O7 the intended CSS data flow is:

1. `css::syntax` parses source text into structured syntax output
2. `css::model` converts that structured output into the engine-facing
   stylesheet/rule/declaration/value model
3. the browser stores model parse artifacts as stylesheet state
4. the current cascade layer derives its temporary compatibility view from the
   model parse artifacts at attachment time

What remains intentionally transitional:

- selector matching still uses a compatibility-scoped selector subset
- inline `style=""` attribute parsing still uses the declaration-list
  compatibility path
- computed-style parsing still consumes raw property/value strings selected by
  the current cascade bridge

Those are later-milestone follow-ups. O7's job is the parser/model cutover, not
finishing selector or cascade semantics.

## Exit Criteria

- crate-root parser output produces the structured model
- raw-string/compat outputs are no longer the primary stored stylesheet
  representation
- parser-to-model integration is covered by tests
- later milestones can treat the model parse result as the default stylesheet
  contract
