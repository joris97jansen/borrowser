# N4: Replace Compatibility Parser Outputs With A Structured Syntax AST

Last updated: 2026-04-04  
Status: queued after N3

## Issue

Replace the remaining compatibility-scoped parser outputs with a real
syntax-layer stylesheet representation so the CSS parser no longer projects its
primary results directly into `CompatSelector`, `CompatRule`, and
`CompatStylesheet`.

N3 established the tokenizer as the lexical source of truth and moved
stylesheet/declaration parsing onto token streams. The main remaining syntax
gap is that the structured parse result is still compatibility-shaped for the
current cascade path.

## Why This Exists

The current architecture is intentionally transitional:

- tokenization is real and syntax-layer owned
- parser recovery is token-driven and deterministic
- parser results are still projected immediately into compatibility types for
  the existing cascade integration

That projection was the right migration strategy for N1 through N3, but it is
not the end-state. The syntax layer now needs its own AST boundary so later
milestones can expand selectors, rules, and values without treating the
compatibility adapter as the permanent parser contract.

## Goals

- introduce a syntax-layer stylesheet AST that is not compatibility-scoped
- define structured rule/prelude/block nodes suitable for later parser work
- replace `String`-backed declaration values with structured syntax/value
  representations or token-backed component-value storage
- preserve the current browser behavior through an explicit compatibility
  projection into `CompatStylesheet`
- keep tokenizer, parser, and cascade responsibilities clearly separated

## Non-Goals

- full selector matching semantics
- cascade redesign beyond the compatibility projection boundary
- computed-style or property-value semantic interpretation
- streaming tokenizer work across network chunk boundaries

## Preferred Direction

Preferred architecture:

1. `parse_stylesheet_with_options` returns a syntax-layer stylesheet result
2. the syntax-layer result owns structured rules, preludes, and block contents
3. declaration values stop being parser-owned raw strings
4. a separate projection step produces `CompatStylesheet` for the current
   cascade path
5. `parse_stylesheet` may continue returning `CompatStylesheet` during the
   migration window

## Exit Criteria

- `StylesheetParse` no longer exposes `CompatStylesheet` as its primary parse
  result
- a real syntax-layer AST exists for stylesheet parsing
- declaration values are no longer represented only as raw `String` output from
  the parser
- an explicit compatibility projection exists for the current cascade path
- docs explain the relationship between syntax-layer AST output and
  compatibility projection
