# G8 — Reach HTML End-Tag-State Parity For RAWTEXT/RCDATA/Script

Status: landed
Milestone: G — Rawtext / RCDATA / script correctness

## Landed Behavior

The shared Core-v0 text-mode matcher for RAWTEXT, RCDATA, and the bounded
script subset now:

- matches the expected end-tag name ASCII-case-insensitively,
- treats `>`, HTML-space-led attribute continuations, and `/` self-closing
  continuations after the matched end-tag name as real end-tag tails,
- consumes those tails incrementally and chunk-safely until the final `>`,
- records tokenizer parse errors for attribute-bearing and self-closing end-tag
  tails, because end tags ignore both,
- keeps the whole candidate sequence in text only when the matched end-tag name
  is followed by any other continuation byte.

Examples now recognized as closing end tags:

- `</style class=x>`
- `</script type=text/plain>`
- `</title/>`

Examples still treated as literal text until a later plain close tag:

- `</stylex>`
- `</scriptx>`
- `</titlex>`

## Landed Requirements

- Preserve the shared reusable matcher architecture introduced for Core v0.
- Remain streaming-safe across chunk boundaries.
- Preserve linear-time scanning on adversarial inputs.
- Add fixture-backed and primitive-level tests for attribute/slash continuations
  in RAWTEXT, RCDATA, and script.

## Evidence

- primitive matcher tests in `crates/html/src/html5/tokenizer/tests/end_tag_matcher.rs`
  cover:
  - attribute-bearing continuations,
  - self-closing continuations,
  - resumable growth across quoted attribute values,
  - odd post-name tails including empty values, quoted values, unquoted values,
    `foo=bar/>`, ` / >`, and post-quoted-name continuation bytes.
- mode-specific tokenizer tests in:
  - `crates/html/src/html5/tokenizer/tests/rawtext.rs`
  - `crates/html/src/html5/tokenizer/tests/rcdata.rs`
  - `crates/html/src/html5/tokenizer/tests/script_data.rs`
  cover whole-input, split-input, and bytewise candidate growth for:
  - whitespace-bearing closes,
  - attribute-bearing closes,
  - slash-bearing closes,
  - tokenizer parse-error recording for attribute-bearing/self-closing tails.
- fixture-backed tokenizer goldens now include:
  - `tok-rawtext-style-end-tag-attrs-close`
  - `tok-rawtext-style-end-tag-slash-close`
  - `tok-rcdata-title-end-tag-attrs-close`
  - `tok-rcdata-textarea-end-tag-slash-close`
  - `tok-script-data-end-tag-attrs-close`
  - `tok-script-data-end-tag-slash-close`
