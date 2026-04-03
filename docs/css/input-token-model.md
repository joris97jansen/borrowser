# CSS Input And Token Model (N2)

Last updated: 2026-04-03  
Scope: `crates/css/src/syntax/input.rs` and `crates/css/src/syntax/token.rs`

This document defines the foundational syntax-layer types introduced by N2:
decoded CSS input handling, byte spans and positions, token payload storage, and
the parser-neutral CSS token model.

Related code:
- `crates/css/src/syntax/input.rs`
- `crates/css/src/syntax/token.rs`
- `crates/css/src/syntax/mod.rs`

## Goals

- make CSS source input explicit and typed
- define a tokenizer-ready lexical model before parser implementation expands
- keep spans deterministic and easy to resolve in tests and diagnostics
- keep token shapes neutral with respect to selector parsing, cascade, and
  computed values

## Input Model

`CssInput` is the decoded source-text container for the CSS syntax layer.

Current contract:
- input is append-only while spans are live
- spans are source-bound via `CssInputId`
- spans are byte-based and must align to UTF-8 character boundaries
- each input buffer has an opaque `CssInputId`
- callers can resolve:
  - `CssSpan`
  - raw source slices
  - `CssPosition { byte_offset, line, column }`

Position behavior:
- line and column are 1-based
- column counts Unicode scalar values within the line
- line breaks are CSS-aware:
  - `\n`
  - `\r`
  - `\r\n`
  - `\u{000C}`
- positions are derived deterministically from the decoded source text

## Span Model

`CssSpan` is the stable source-range type for tokens and future diagnostics.

Current contract:
- `CssSpan` carries the owning `CssInputId`
- `start <= end`
- spans are validated against the owning `CssInput`
- spans must not resolve against a different `CssInput`
- empty spans are allowed
- span storage is byte-based; later tokenizer/parser code may build richer
  abstractions on top of it, but this is the stable base type

## Token Payload Model

`CssTokenText` stores textual token payloads as either:
- `Span(CssSpan)` for source-backed text
- `Owned(String)` for normalized or synthesized text

This allows the tokenizer and later parser stages to preserve zero-copy source
references where possible without forcing all payloads to remain source-backed.

Invariant:
- span-backed payloads only resolve successfully against the owning `CssInput`

## Token Model

`CssToken` is the lexical unit type:
- `kind: CssTokenKind`
- `span: CssSpan`

The token model is intentionally parser-neutral. It includes core CSS lexical
forms needed for stylesheet parsing, including:
- identifiers and function-like forms
- at-keywords and hash tokens
- strings and URLs
- number-like tokens:
  - number
  - percentage
  - dimension
- structural punctuation and block delimiters
- CSS match operators (`~=`, `|=`, `^=`, `$=`, `*=`, `||`)
- comment/trivia and whitespace
- `CDO`, `CDC`, `EOF`

The model does not encode:
- selector semantics
- cascade semantics
- computed values

Foundational invariants:
- `CssNumber` is lexical source text, not a parsed semantic numeric value
- `CssUnicodeRange` is validated on construction:
  - `start <= end`
  - `end <= 0x10FFFF`

## Debug And Snapshot Contract

Stable token snapshots are provided by:
- `serialize_tokens_for_snapshot(input, tokens)`

Snapshot guarantees:
- stable token ordering
- stable token-kind labels
- stable span rendering
- text payloads are resolved deterministically through `CssInput`
- snapshot formatting does not depend on Rust derived `Debug`
