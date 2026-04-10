# P5: Define And Implement Invalid Selector Handling

Last updated: 2026-04-10  
Status: complete

This document records the implementation of Milestone P issue P5:
deterministic invalid-selector handling for Borrowser's selector parser.

Related code:
- `crates/css/src/selectors/parser.rs`
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/src/selectors/serialize.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p3-selector-parser-core-subset.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/syntax-parser-contract.md`

## Implemented Result

Borrowser now has an explicit invalid-selector contract owned by
`css::selectors`.

Invalid selectors are:

- rejected as parsed selector IR
- represented as `SelectorListParseResult::Invalid(InvalidSelectorList)`
- classified by an explicit `InvalidSelectorReason`
- serialized deterministically for snapshots and debugging

The parser does not silently reinterpret malformed selector input as a narrower
supported selector.

## Parsing Policy

Invalid selector handling for Milestone P is intentionally non-recovering at
the selector-list boundary.

Normative rule:

- if any selector in a selector list is invalid, the whole selector list
  result is `Invalid`
- invalid selector lists do not produce partial selector IR
- invalid selector lists are non-matchable and have no usable specificity

This keeps selector behavior deterministic and avoids implementation-defined
partial salvage.

## Detection Rules

The current invalid-selector categories are:

- empty selector list
- empty compound selector
- leading combinator
- trailing combinator
- repeated combinator
- multiple type selectors in one compound
- missing attribute name
- missing attribute value
- unexpected component value

The parser reports one deterministic primary invalid reason together with a
source span.

## Parser Stability

Malformed selectors must not corrupt parser state.

The current implementation keeps invalid handling stable by:

- parsing each comma-separated selector segment through an isolated
  `SegmentParser`
- stopping selector-list parsing immediately on the first invalid segment
- preserving invalid-vs-unsupported precedence explicitly
- keeping malformed selector outcomes outside the selector IR

This is an intentional scoped design choice for Milestone P rather than a
recovery bug or temporary shortcut.

## Regression Coverage

Representative tests cover:

- invalid leading, trailing, and repeated combinators
- empty selector-list segments and trailing commas
- multiple type selectors in one compound
- missing attribute names and values
- malformed class and pseudo selector starts
- whole-list invalidation without partial recovery
- explicit parse-result accessors for invalid vs unsupported outcomes

## Exit Criteria

P5 is complete when:

- invalid selector handling is explicitly defined
- malformed selectors produce deterministic invalid results
- malformed selectors do not produce partial selector IR
- parser behavior remains stable under representative malformed input
- regression tests cover representative invalid selector cases

Repository status:

- the P5 invalid-selector-handling issue is complete and may be treated as
  closed
- the next Milestone P work should wire selector parse results into the
  stylesheet rule/model path
- selector matching and cascade winner resolution remain intentionally out of
  scope for that step
