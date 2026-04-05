# CSS Syntax Layer Contract (Milestone N)

Last updated: 2026-04-05  
Scope: `crates/css/src/syntax/mod.rs`, `crates/css/src/syntax/compat.rs`,
`crates/css/src/syntax/input.rs`, `crates/css/src/syntax/token.rs`,
`crates/css/src/syntax/tokenizer.rs`, `crates/css/src/syntax/parser.rs`,
`crates/runtime_css/src/lib.rs`, and the
current browser integration path in `crates/browser/src/page.rs`

This document is the source-of-truth contract for Milestone N's CSS syntax
layer. It defines what the tokenizer and parser own, what malformed-input
recovery means in this engine, which parts of the current CSS stack are being
replaced, and what downstream milestones may assume.

Related code:
- `crates/css/src/syntax/mod.rs`
- `crates/css/src/syntax/compat.rs`
- `crates/css/src/syntax/input.rs`
- `crates/css/src/syntax/token.rs`
- `crates/css/src/syntax/tokenizer.rs`
- `crates/css/src/syntax/parser.rs`
- `crates/css/src/cascade.rs`
- `crates/browser/src/page.rs`
- `crates/runtime_css/src/lib.rs`

## Goals

- Replace ad hoc stylesheet parsing with an explicit syntax subsystem.
- Separate tokenization responsibilities from parsing responsibilities.
- Keep parser behavior deterministic under malformed or hostile input.
- Define bounded resource behavior before the implementation grows.
- Provide a stable, explicit debug/snapshot contract for regression tests.
- Preserve the current crate boundary and current high-level browser pipeline.

## Non-Goals For Milestone N

- Full selector grammar or selector matching semantics.
- Full CSS Syntax Level 3 or CSSOM coverage.
- At-rule semantics such as `@media`, `@supports`, or `@keyframes`.
- Cascade resolution, inheritance, or computed-value logic.
- Streaming tokenization across network chunk boundaries.
  For Milestone N, `runtime_css` may continue to assemble a complete decoded
  stylesheet string before handing it to `css::syntax`.
- Byte decoding, charset sniffing, or transport concerns.

## Architecture Boundary

Milestone N keeps the current crate and runtime topology:

1. `runtime_css`
   - owns byte accumulation for external stylesheets
   - owns incremental UTF-8 assembly
   - does **not** own CSS tokenization or syntax parsing
2. `css::syntax`
   - owns CSS tokenization and parser entry points
   - owns syntax diagnostics, recovery behavior, and parser limits
   - owns stable syntax debug/snapshot output for tests
3. `css::cascade` and `css::computed`
   - consume syntax-layer output
   - do **not** reach into tokenizer or parser internals
4. `browser::page`
   - remains the high-level integration point that merges stylesheet results
     into the page state

## Implementation Status

N1 establishes the syntax-layer contract and public API surface.

Current repository status:
- syntax-layer input abstraction exists in code
- explicit token definitions exist in code
- source spans are bound to their owning decoded input identity
- source positions use CSS-aware line-boundary handling
- a standalone tokenizer implementation exists in code
- tokenizer diagnostics are typed and deterministic
- a structured stylesheet parser exists in code
- stylesheet parse results are syntax-layer oriented rather than
  compatibility-scoped
- declaration-block parsing is token-driven and produces structured syntax
  declarations
- compatibility projection now lives in `crates/css/src/syntax/compat.rs`
  rather than defining the primary parse result
- compatibility outputs still preserve the pre-N cascade path and are not the
  normative long-term syntax tree for later milestones

## Current Components: Retained Vs Replaced

Retained in Milestone N:
- `crates/css` as the CSS crate boundary
- `runtime_css` as the stylesheet transport/assembly runtime
- `browser::page` as the integration point for applying parsed CSS
- `cascade` and `computed` modules as downstream consumers

Replaced in Milestone N:
- naive `split('}')`, `split('{')`, and `split(';')` parsing as the real parser
- naive lexical boundary detection based on raw string splitting
- implicit malformed-input skipping without diagnostics or fixed rules
- syntax logic coupled directly to selector matching assumptions
- unstable, incidental Rust `Debug` output as the test surface

Compatibility note:
- `Declaration` remains part of the syntax-layer contract
- `CompatSelector`, `CompatRule`, and `CompatStylesheet` remain available as
  adapter outputs for the existing cascade path during the Milestone N rollout
- those `Compat*` types are not the long-term selector/rule AST and must not be
  treated as the permanent tokenizer/parser boundary

## Tokenizer Responsibilities

The tokenizer is the first stage inside `css::syntax`.

It MUST:
- consume decoded `&str` input only
- emit an explicit token model with token kind, source span/offset, and
  normalized trivia handling
- classify structural syntax needed by stylesheet parsing:
  identifiers, delimiters, braces, brackets, parentheses, strings, numbers,
  comments/trivia, colon, semicolon, comma, at-keywords, and EOF
- be deterministic for byte-identical input
- stop at configured resource limits with a typed diagnostic rather than
  panicking or allocating without bound

It MUST NOT:
- perform selector matching
- resolve cascade order
- interpret computed values
- hide malformed-input recovery inside tokenizer-specific heuristics

Implementation note:
- tokenizer ownership is part of the N1 contract
- N2 introduces the input model, spans, and token definitions
- N3 implements the tokenizer and stable tokenization entry points

## Parser Responsibilities

The parser consumes tokens and produces syntax-layer results.

It MUST:
- expose structured entry points for:
  - whole stylesheet parsing
  - inline declaration-list parsing (`style=""`)
- define fixed recovery boundaries
- emit diagnostics through typed parser results
- preserve source order
- enforce parser invariants and configured limits
- remain panic-free for malformed CSS input

It MUST NOT:
- match selectors against DOM nodes
- compute specificity for semantic selector trees beyond what the current
  compatibility bridge still requires
- evaluate cascade precedence
- parse computed values or value semantics beyond syntax boundaries

## Entry Points And Expected Outputs

Milestone N defines two syntax entry points:

- `parse_stylesheet_with_options(input, options) -> StylesheetParse`
- `parse_declarations_with_options(input, options) -> DeclarationListParse`

The parser result contract is:

- parsed output
  - syntax-layer stylesheet rules or declaration list in source order
- diagnostics
  - ordered, typed, bounded, deterministic
- stats
  - input bytes consumed
  - emitted object counts
  - total diagnostics emitted
  - whether a configured limit was hit

Compatibility wrappers remain available:

- `parse_stylesheet(input) -> CompatStylesheet`
- `parse_declarations(input) -> Vec<Declaration>`

Downstream code may keep using these wrappers during the migration, but new
Milestone N work should target the structured parse-result entry points.

Current stylesheet parse shape:

- `StylesheetParse` owns:
  - bounded decoded `CssInput`
  - `CssStylesheet`
  - diagnostics
  - parse stats
- compatibility projection is explicit:
  - `StylesheetParse::to_compat_stylesheet()`
  - `parse_stylesheet(input)` remains the migration convenience wrapper

## Recovery Philosophy

Malformed CSS is recoverable syntax input, not an engine failure.

Recovery rules for this milestone:
- the parser never panics on malformed stylesheet input
- malformed constructs are skipped using explicit structural boundaries
- one malformed declaration must not discard unrelated valid declarations in the
  same block when a deterministic boundary exists
- one malformed rule must not discard later sibling rules when a deterministic
  boundary exists
- unmatched or trailing input at EOF is reported deterministically
- limit exhaustion is reported as a typed error and parsing stops in a defined
  way

Recovery boundaries for this stage:
- declaration-level recovery uses `;` and block end `}`
- rule-level recovery uses block end `}`
- EOF recovery is explicit and deterministic

## Resource Limits And Invariants

The syntax layer must remain bounded under malformed or hostile input.

Milestone N invariants:
- parser input is already decoded text
- token spans now exist as syntax-layer types
- token spans are bound to one owning `CssInputId`
- span-backed token payloads must not resolve against a different input
- source positions treat `\n`, `\r`, `\r\n`, and `\u{000C}` as line breaks
- parse diagnostics still expose byte offsets only; richer diagnostic span usage
  remains deferred until tokenizer/parser integration lands
- parser entry points are pure with respect to one input string and one options
  struct
- equivalent input and limit configuration must produce equivalent output,
  diagnostics, and snapshots
- output order is source order
- diagnostics are emitted in encounter order
- hitting a limit must set an explicit flag and emit a typed diagnostic

Milestone N limit categories:
- maximum stylesheet input bytes
- maximum declaration-list input bytes
- maximum emitted rules
- maximum selectors per compatibility rule
- maximum declarations per rule/declaration list
- maximum stored diagnostics

## Testing And Debug Contract

Rust's derived `Debug` is not the contract.

The syntax layer must provide stable, explicit serializers for regression tests:
- `serialize_stylesheet_for_snapshot`
- `serialize_compat_stylesheet_for_snapshot`
- `serialize_declarations_for_snapshot`
- `serialize_tokens_for_snapshot`
- `CssTokenization::to_debug_snapshot`
- `StylesheetParse::to_debug_snapshot`
- `DeclarationListParse::to_debug_snapshot`

Snapshot guarantees:
- field order is explicit
- rule/declaration order is preserved
- diagnostics and stats are serialized in a stable order
- stable snapshots are based on severity, diagnostic kind, and byte offset;
  free-form human messages are intentionally excluded from the golden contract
- output does not depend on allocator layout or Rust debug formatting details

Milestone N test coverage should expand from here to include:
- stylesheet snapshot goldens
- inline declaration-list snapshot goldens
- token snapshot goldens
- malformed-input recovery cases
- hostile-input and limit-enforcement cases

## Downstream Contract

Downstream milestones may assume:
- syntax entry points are deterministic
- syntax diagnostics are structured and bounded
- snapshot output is stable enough for golden tests
- malformed CSS does not crash the engine

Downstream milestones must not assume:
- selector parsing is final
- at-rule parsing is complete
- current compatibility structs are the permanent syntax AST
- syntax parsing performs cascade or computed-style work

## Related Next Steps And References

The next queued syntax-layer follow-ups for this contract are:

- [`docs/css/n5-selector-structure-expansion.md`](n5-selector-structure-expansion.md)
- [`docs/css/n2b-incremental-line-record-maintenance.md`](n2b-incremental-line-record-maintenance.md)

Historical reference:

- [`docs/css/n2a-decouple-structured-parse-results.md`](n2a-decouple-structured-parse-results.md)
  is now subsumed by `n4-structured-syntax-ast.md`

Related reference for the N2 source/token layer:

- [`docs/css/input-token-model.md`](input-token-model.md)

Related reference for the N3 tokenizer layer:

- [`docs/css/tokenizer-behavior.md`](tokenizer-behavior.md)

Related reference for the N4 structured parser work:

- [`docs/css/n4-structured-syntax-ast.md`](n4-structured-syntax-ast.md)

Related reference for the next selector-structure expansion work:

- [`docs/css/n5-selector-structure-expansion.md`](n5-selector-structure-expansion.md)
