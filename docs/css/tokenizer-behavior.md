# CSS Tokenizer Behavior (N3)

Last updated: 2026-04-04  
Scope: `crates/css/src/syntax/tokenizer.rs`

This document defines the implemented tokenizer contract introduced by N3.
It complements the broader syntax-layer contract by describing the tokenizer's
entry points, deterministic output guarantees, malformed-input behavior, and
test surface.

Related code:
- `crates/css/src/syntax/mod.rs`
- `crates/css/src/syntax/token.rs`
- `crates/css/src/syntax/tokenizer.rs`

## Public Entry Points

The tokenizer exposes two public entry points:

- `tokenize_str(input) -> CssTokenization`
- `tokenize_str_with_options(input, options) -> CssTokenization`

`CssTokenization` owns:
- the bounded decoded `CssInput`
- emitted `CssToken` values in source order
- ordered `SyntaxDiagnostic` values
- `CssTokenizationStats`

## Core Lexical Coverage

The tokenizer emits the token forms required for the current stylesheet and
declaration-list parser stages:

- whitespace and comments
- identifiers, functions, and at-keywords
- hash tokens
- quoted strings and bad strings
- `url(...)` and bad URLs
- numbers, percentages, and dimensions
- unicode ranges
- delimiters and structural punctuation
- match operators
- `CDO`, `CDC`, and `EOF`

The tokenizer remains lexical only. It does not encode selector semantics,
cascade behavior, or property-specific value interpretation.

## Determinism Contract

For the same input text and `ParseOptions`, tokenization must produce the same:

- bounded input bytes
- token kinds and token order
- source spans
- diagnostics
- debug/snapshot output

`CssInputId` remains per-buffer identity and is not part of the golden
determinism contract. Stable tests should compare token snapshots rather than
raw `CssToken` equality across independently-created inputs.

## Malformed Input Recovery

Tokenizer recovery is defined and panic-free.

Current malformed-input behavior:
- unterminated comments emit a comment token and an `unterminated-comment`
  diagnostic
- unterminated strings emit `bad-string` and an `unterminated-string`
  diagnostic
- malformed `url(...)` input emits `bad-url` and a `bad-url` diagnostic
- input truncation at configured limits emits `limit-exceeded`

Recovery remains deterministic:
- the tokenizer does not panic on malformed lexical input
- malformed constructs produce explicit token/diagnostic output
- tokenization always terminates with `EOF`

## Limits And Snapshot Surface

Tokenizer input limits are origin-sensitive:

- stylesheet tokenization uses `max_stylesheet_input_bytes`
- declaration-list tokenization uses `max_declaration_list_input_bytes`

Stable regression surfaces:

- `serialize_tokens_for_snapshot(input, tokens)`
- `CssTokenization::to_debug_snapshot()`

Snapshot output intentionally excludes free-form diagnostic prose and uses only:

- token kind labels
- source spans
- diagnostic severity
- diagnostic kind
- byte offset
- stable stats fields
