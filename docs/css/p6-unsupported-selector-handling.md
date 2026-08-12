# P6: Define Handling Of Unsupported Selector Features

Last updated: 2026-04-10  
Status: complete

This document records the implementation of Milestone P issue P6:
deterministic handling for syntactically valid but currently unsupported
selector features.

Related code:
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/parser.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p3-selector-parser-core-subset.md`
- `docs/css/p5-invalid-selector-handling.md`
- `docs/css/syntax-parser-contract.md`

## Implemented Result

Borrowser now has an explicit unsupported-selector handling strategy owned by
`css::selectors`.

Syntactically valid but unsupported selectors are:

- preserved as `SelectorListParseResult::Unsupported(UnsupportedSelectorList)`
- treated as non-matchable for the current engine stage
- classified by explicit `UnsupportedSelectorFeature` categories
- normalized into stable first-encounter feature order with deduplication
- retained with its source span and unsupported feature categories for
  diagnostics and future tracking; because this is not a lossless unsupported
  selector AST, future support may require reparsing the preserved
  source/prelude

The in-code handling strategy is exposed as
`UnsupportedSelectorHandling::PreserveAsUnsupported`.
For Milestone P, this strategy is fixed rather than configurable per parse
mode or feature gate.

## Strategy

Milestone P does not reject syntactically valid unsupported selectors as
invalid, and it does not silently ignore unsupported syntax.

The chosen strategy is:

- preserve the selector list as unsupported
- classify the unsupported feature categories explicitly
- treat the whole selector list as non-matchable
- leave feature enablement for later milestones rather than partially
  reinterpreting the selector now

This is a forward-compatible, feature-gated direction without introducing
partial support semantics prematurely.

## Parser Behavior

Unsupported selector handling is consistent across the parser:

- unsupported simple selectors mark the containing selector segment as
  unsupported rather than invalid
- unsupported combinators are classified explicitly
- unsupported features do not corrupt parsing of surrounding supported tokens
- invalid selector precedence still wins if malformed input is also present

Selector-list policy remains:

- any invalid selector in the list makes the whole result `Invalid`
- otherwise any unsupported selector in the list makes the whole result
  `Unsupported`
- only fully supported lists produce `Parsed`

## Current Unsupported Categories

The currently classified unsupported selector features are:

- namespaces
- attribute case modifiers
- pseudo-classes
- functional pseudo-classes
- pseudo-elements
- relative selectors
- nesting selector `&`
- column combinator `||`
- forgiving selector lists

## Regression Coverage

Representative tests cover:

- stable unsupported snapshots for pseudo, namespace, and attribute-modifier
  selectors
- deduplicated unsupported-feature ordering
- unsupported handling strategy exposure in the public API
- unsupported features inside otherwise supported selector structure
- stable unsupported-feature aggregation across selector lists
- invalid-over-unsupported precedence

## Exit Criteria

P6 is complete when:

- unsupported selector handling strategy is explicit in code and docs
- unsupported selector behavior is deterministic and consistent
- unsupported selector features do not corrupt surrounding selector parsing
- representative unsupported selector tests pass
- the strategy is documented clearly for later milestones

Repository status:

- the P6 unsupported-selector-handling issue is complete and may be treated as
  closed
- Milestone P and AF2 now provide the permanent selector/model parse-result
  path and selector diagnostic projection
- selector matching and cascade winner resolution remain separate downstream
  consumers
