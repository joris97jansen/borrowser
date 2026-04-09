# P3: Implement Selector Parser (Core Subset)

Last updated: 2026-04-09  
Status: complete

This document records the implementation of Milestone P issue P3:
introducing the core selector parser that converts syntax-layer selector
preludes into selector IR.

Related code:
- `crates/css/src/selectors/parser.rs`
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/tests.rs`
- `crates/css/src/lib.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p2-selector-ir-data-structures.md`

## Implemented Result

Borrowser now has a token-driven selector parser that consumes structured
syntax-layer component values and produces `SelectorListParseResult`.

The implemented parser supports:

- type selectors
- class selectors
- id selectors
- universal selectors
- attribute selectors for the supported subset
- compound selectors
- selector lists
- descendant, child, next-sibling, and subsequent-sibling combinators

The parser also classifies unsupported features explicitly, including:

- namespaces
- pseudo-classes
- functional pseudo-classes
- pseudo-elements
- nesting selector `&`
- column combinator `||`
- attribute case-sensitivity modifiers
- forgiving selector lists in supported functional-pseudo-class detection

## Parser Boundary

The selector parser is:

- token-driven through `CssComponentValue`
- independent from DOM matching
- independent from cascade winner resolution
- deterministic for identical syntax-layer input

It does not perform selector matching or optimize for matching performance.

## Deterministic Behavior

The parser distinguishes:

- valid parsed selector IR
- unsupported selector features
- invalid selector structure

Representative behaviors covered by tests include:

- comment-only gaps do not create descendant combinators
- whitespace gaps do create descendant combinators
- invalid leading, trailing, and repeated combinators are classified
  deterministically
- unsupported selector forms are preserved as unsupported rather than being
  partially reinterpreted

## Exit Criteria

P3 is complete when:

- the selector parser produces valid IR from syntax-layer tokens/component
  values
- the supported selector subset parses correctly
- parsing is deterministic
- parsing is token-driven rather than string-splitting
- representative selector tests pass

Repository status:

- the P3 core selector-parser issue is complete and may be treated as closed
- the next Milestone P work should wire `SelectorListParseResult` into the
  stylesheet rule/model path
- selector matching and cascade winner resolution remain intentionally out of
  scope for that integration step
