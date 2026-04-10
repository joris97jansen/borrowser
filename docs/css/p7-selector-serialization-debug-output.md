# P7: Add Deterministic Serialization And Debug Output For Selectors

Last updated: 2026-04-10  
Status: complete

This document records the implementation of Milestone P issue P7:
stable selector serialization and debug output for regression testing and
debugging.

Related code:
- `crates/css/src/selectors/serialize.rs`
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/tests/selector_golden.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p3-selector-parser-core-subset.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/syntax-parser-contract.md`

## Implemented Result

Borrowser now treats selector serialization as an explicit, stable contract
rather than relying on derived `Debug`.

The selector subsystem provides:

- `serialize_selector_list_for_snapshot`
- `serialize_selector_parse_result_for_snapshot`
- `SelectorList::to_debug_snapshot()`
- `SelectorListParseResult::to_debug_snapshot()`

These serializers reflect selector IR structure rather than original CSS
formatting.

## Format Contract

The selector snapshot format is deterministic and versioned.

Current guarantees:

- snapshots begin with `version: 1`
- the version header is contract metadata; meaningful snapshot-format changes
  must be handled deliberately through intentional fixture updates or a format
  version bump
- output is structured by semantic IR nodes rather than source formatting
- selector lists serialize in source order
- combinators serialize explicitly by kind
- specificity is shown per selector and compound selector
- node spans and meaningful payload spans serialize explicitly
- invalid and unsupported parse outcomes serialize their category fields rather
  than implementation-specific debug output

## Regression Surface

Selector serialization now has both unit coverage and package-level golden
coverage.

Golden tests cover:

- a representative parsed selector list and its list serializer output
- a representative parsed selector result and its parse-result serializer
  output
- representative unsupported selector output
- representative invalid selector output

This keeps the selector snapshot surface aligned with the repository's syntax
and model golden-test approach.

## Exit Criteria

P7 is complete when:

- selector debug output is deterministic
- selector serialization is stable across runs
- golden tests exist and pass
- output remains aligned with selector IR structure
- snapshots are usable for debugging and regression testing

Repository status:

- the P7 selector-serialization issue is complete and may be treated as closed
- the next Milestone P work should wire selector parse results into the
  stylesheet rule/model path
- selector matching and cascade winner resolution remain intentionally out of
  scope for that step
