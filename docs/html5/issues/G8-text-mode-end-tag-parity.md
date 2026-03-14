# G8 — Reach HTML End-Tag-State Parity For RAWTEXT/RCDATA/Script

Status: follow-up to the Core-v0 shared text-mode close-tag subset
Milestone: G — Rawtext / RCDATA / script correctness

## Current Core-v0 Boundary

The shared Core-v0 text-mode matcher for RAWTEXT, RCDATA, and the bounded
script subset currently:

- matches the expected end-tag name ASCII-case-insensitively,
- recognizes the close only when the matched end-tag name is followed by zero
  or more HTML space bytes and then `>`,
- keeps the whole candidate sequence in text when attribute-bearing or
  slash-bearing continuations follow the matching end-tag name.

Examples currently treated as literal text until a later plain close tag:

- `</style class=x>`
- `</script type=text/plain>`
- `</title/>`

## Follow-up Goal

Reach full HTML tokenizer parity for appropriate end-tag handling in RAWTEXT,
RCDATA, and script text modes after the end-tag name has matched.

This includes:

- whitespace-triggered transitions after the matching name,
- attribute-like continuations on end tags,
- slash-bearing continuations before `>`,
- correct parse-error behavior while preserving chunk safety and deterministic
  token boundaries.

## Requirements

- Preserve the shared reusable matcher architecture introduced for Core v0.
- Remain streaming-safe across chunk boundaries.
- Preserve linear-time scanning on adversarial inputs.
- Add fixture-backed and primitive-level tests for attribute/slash continuations
  in RAWTEXT, RCDATA, and script.
