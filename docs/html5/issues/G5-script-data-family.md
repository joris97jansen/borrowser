# G5 — Implement Full HTML Script-Data State Family

Status: landed
Milestone: G — Rawtext / RCDATA / script correctness

## Landed Behavior

The tokenizer now implements a dedicated Core-v0 script-data family for the
promoted script surface rather than treating `<script>` as the shared text-mode
subset.

This landed implementation compresses some WHATWG named script states into
shared helpers, but the promoted escaped and double-escaped transitions are
covered as behaviorally equivalent tokenizer paths rather than as a literal
1:1 state-name clone.

The landed family includes:

- script data
- script data escaped
- script data double escaped
- comment-like `<!--` entry into escaped script
- `<script` entry into double-escaped script
- `</script` exit from double-escaped back to escaped script
- `-->` exit from escaped script back to script data
- chunk-safe appropriate-end-tag handling throughout the script family

## Landed Requirements

- chunk safety
- linear-time scanning
- deterministic token boundaries

## Remaining Out Of Scope

- parser pause/suspension and script execution integration

## Evidence

- dedicated script-family unit coverage in `crates/html/src/html5/tokenizer/tests/script_data.rs`
  now covers:
  - escaped comment entry,
  - double-escaped nested `<script>...</script>` handling,
  - split safety and bytewise growth across `<!--`, `<script>`, `</script>`, and `-->`,
  - near-miss `<scriptx` / `</scriptx>` handling around escaped and double-escaped transitions,
  - preservation of the existing G8/G11 script close-tag guarantees.
- direct boundary-helper coverage in
  `crates/html/src/html5/tokenizer/tests/script_tag_boundary.rs` now covers:
  - `<script` / `</script` incremental growth,
  - `>`, `/`, and HTML-space delimiter recognition,
  - near-miss names such as `<scriptx` and `</scriptx>`,
  - incomplete prefix handling via `NeedMoreInput`.
- focused script-family acceptance fixture:
  - `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-escaped-comment-family`
- vendored WPT tokenizer case:
  - `tests/wpt/vendor/html/syntax/parsing/tokenizer-script-escaped.html`
  - `tests/wpt/expected/tokenizer-script-escaped.tokens.txt`
