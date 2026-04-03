# N2b: Make CssInput Line-Record Maintenance Incremental

Last updated: 2026-04-03  
Status: queued after N2

## Issue

Refine `CssInput` so append operations maintain line records incrementally
rather than rebuilding them from the full buffer on every `push_str()`.

## Why This Exists

N2 established the correct integrity model for CSS decoded input:
- source-bound spans
- CSS-aware line-boundary handling
- deterministic position resolution

The current `push_str()` implementation is intentionally simple and correct, but
it rebuilds line records from the entire buffer after each append. That is
acceptable for the current runtime shape because `runtime_css` still assembles a
full decoded stylesheet before syntax work. It is not the long-term
implementation shape we want once tokenizer input becomes more incremental.

## Goals

- update line records incrementally on append
- preserve the current external `CssInput` contract exactly
- keep CSS-aware line-break semantics unchanged for:
  - `\n`
  - `\r`
  - `\r\n`
  - `\u{000C}`
- avoid repeated full-buffer rescans under append-heavy usage

## Non-Goals

- changing `CssSpan` or `CssInputId` semantics
- changing `CssPosition` line/column meaning
- changing tokenizer/parser contracts

## Exit Criteria

- `push_str()` updates line records incrementally
- line/column semantics remain unchanged
- tests cover mixed append patterns and CSS line-break forms
- docs continue to describe the same external contract
