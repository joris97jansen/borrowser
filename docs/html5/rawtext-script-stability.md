# RAWTEXT / Script Stability Notes

Last updated: 2026-03-31
Scope: `crates/html/src/html5/tokenizer` (`feature = "html5"`)

This note summarizes the Borrowser hardening posture for HTML5 text-mode
termination in:

- RAWTEXT (`<style>`)
- RCDATA (`<title>`, `<textarea>`)
- script data (`<script>`)

Related documents:
- [`docs/html5/html5-core-v0.md`](html5-core-v0.md)
- [`docs/security/html5-tokenizer-hardening.md`](../security/html5-tokenizer-hardening.md)
- [`docs/html5/spec-matrix-tokenizer.md`](spec-matrix-tokenizer.md)

## Why Script Termination Is Tricky

Text-mode termination is one of the easiest tokenizer areas to regress because
the parser must combine:

- ASCII case-insensitive end-tag name matching
- streaming chunk boundaries, including 1-byte chunking
- near misses such as `</scriXpt>` or `</script` at EOF
- large runs of literal `<` bytes that must stay text
- script-specific escaped and double-escaped comment-like branches

The failure mode is rarely "simple wrong token". More often it is:

- candidate close tags restarting from the same `<` repeatedly
- different behavior between whole-input and chunked execution
- accidental termination on a near miss
- failure to terminate on a real close tag
- performance drift from resumable matching into repeated rescans

For Borrowser, stability here means both correctness and boundedness.

## Invariants We Enforce

The tokenizer and test/fuzz harnesses enforce the following invariants for
RAWTEXT, RCDATA, and script:

- whole-input and chunked-input execution must produce the same observable
  token stream for equivalent bytes
- `push_input_until_token()` is the token-granular integration API; when the
  caller drains the queue between pumps, each pump yields at most one newly
  emitted token
- tokenizer controls (`EnterTextMode` / `ExitTextMode`) are applied only at
  token boundaries
- text-mode end-tag matching is incremental and resumable across chunk growth;
  pending candidates do not restart from the candidate `<` on every pump
- per-candidate scan work is bounded by
  `TokenizerLimits::max_end_tag_match_scan_bytes`
- an oversized close-tag candidate is abandoned deterministically and emitted as
  literal text instead of scanning an unbounded tail
- pending text-mode state is fixed-size matcher metadata plus span/index
  bookkeeping into shared decoded input; there is no separately copied
  close-tag-prefix buffer and no copied RAWTEXT/script text buffer
- RCDATA performs character-reference decoding; RAWTEXT and script do not
- debug / test / `parser_invariants` builds fail fast on tokenizer invariant
  breaches and stall-guardrail violations

These invariants are checked by a mix of unit tests, fixture regressions,
deterministic corpus replay, targeted fuzzing, and performance guardrails.

## Core v0 Rules

Core v0 text-mode behavior is defined in
[`docs/html5/html5-core-v0.md`](html5-core-v0.md). The parts that matter most
for stability are:

- RAWTEXT is supported for HTML RAWTEXT containers through the shared text-mode
  matcher
- RCDATA is supported for `title` / `textarea`, including current tokenizer
  character-reference decoding behavior
- script uses a dedicated script-data state family, including escaped and
  double-escaped comment-like branches, while still using the shared script
  close-tag matcher
- close-tag recognition is ASCII case-insensitive on the expected tag name
- after the matched name, `>`, HTML-space-led attribute continuations, and `/`
  self-closing continuations are treated as real end-tag tails
- if the matched name is followed by any other continuation byte, the entire
  candidate remains text
- end-tag tails are consumed incrementally and chunk-safely until `>`
- attribute-bearing and self-closing end-tag tails close the element and record
  tokenizer parse errors because those forms are syntactically invalid for end
  tags
- parser scripting interaction is not implemented in Core v0; tokenizer
  stability here is about tokenization and streaming contracts, not script
  execution pauses

This is intentionally narrower than full WHATWG parity in every corner, but the
implemented behavior is contractual and regression-tested.

## Fuzz Targets

The targeted fuzz targets for this surface are:

- script data: [`fuzz/fuzz_targets/html5_tokenizer_script_data.rs`](../../fuzz/fuzz_targets/html5_tokenizer_script_data.rs)
- RAWTEXT: [`fuzz/fuzz_targets/html5_tokenizer_rawtext.rs`](../../fuzz/fuzz_targets/html5_tokenizer_rawtext.rs)
- RCDATA: [`fuzz/fuzz_targets/html5_tokenizer_rcdata.rs`](../../fuzz/fuzz_targets/html5_tokenizer_rcdata.rs)

Committed seed corpora:

- `fuzz/corpus/html5_tokenizer_script_data/`
- `fuzz/corpus/html5_tokenizer_rawtext/`
- `fuzz/corpus/html5_tokenizer_rcdata/`

Triaged fuzz regressions:

- `fuzz/regressions/html5_tokenizer_script_data/`
- `fuzz/regressions/html5_tokenizer_rawtext/`
- `fuzz/regressions/html5_tokenizer_rcdata/`

Deterministic replay and smoke-lane commands are documented in
[`fuzz/README.md`](../../fuzz/README.md).

## Stable Regression Surfaces

Two regression layers lock this behavior after fuzzing discovers edge cases:

- curated script close-tag fixtures:
  `crates/html/tests/fixtures/html5/script_regressions/`
- promoted stable RAWTEXT/script regressions:
  `crates/html/tests/regressions/html5/rawtext_script/`

The curated script fixture suite is for explicit close-tag behavior and
whole-vs-boundary-chunk equivalence. The promoted regression suite is the
"fuzz finding -> minimized input -> stable test" surface, with provenance
metadata and token/DOM oracles.

## Limits And Guardrails

The relevant tokenizer limit is:

- `TokenizerLimits::max_end_tag_match_scan_bytes`

Reference files:

- API surface:
  `crates/html/src/html5/tokenizer/api.rs`
- limit enforcement:
  `crates/html/src/html5/tokenizer/limits.rs`
- limit tests:
  `crates/html/src/html5/tokenizer/tests/limits.rs`
- perf guardrails:
  `crates/html/src/html5/tokenizer/tests/perf.rs`

Current recovery contract:

- if a text-mode close-tag candidate exceeds the configured scan bound, the
  tokenizer records `ParseErrorCode::ResourceLimit`
- the oversized candidate is emitted as literal text
- recovery is deterministic across whole-input and chunked execution
- subsequent text and later real close tags continue to tokenize normally

This limit exists to prevent repeated full rescans and unbounded candidate-tail
scanning on adversarial inputs.

## Reproducing And Triaging Fuzz Failures

### Deterministic Replay

Replay committed text-mode corpora through normal test infrastructure:

```sh
make test-html5-tokenizer-script-data-fuzz-corpus
make test-html5-tokenizer-rawtext-fuzz-corpus
make test-html5-tokenizer-rcdata-fuzz-corpus
```

Replay a specific seed through the actual fuzz target:

```sh
cargo fuzz run html5_tokenizer_script_data fuzz/corpus/html5_tokenizer_script_data/<seed-name>
cargo fuzz run html5_tokenizer_rawtext fuzz/corpus/html5_tokenizer_rawtext/<seed-name>
cargo fuzz run html5_tokenizer_rcdata fuzz/corpus/html5_tokenizer_rcdata/<seed-name>
```

### CI Smoke Reproduction

PR CI runs short fixed-seed smoke lanes via:

```sh
make test-html5-tokenizer-script-data-fuzz-smoke
make test-html5-tokenizer-rawtext-fuzz-smoke
make test-html5-tokenizer-rcdata-fuzz-smoke
```

On failure, the smoke scripts print:

- the fixed seed
- the exact corpus/regression directories used
- the artifact path if libFuzzer materialized a reproducer
- a direct-binary reproduction command
- a `cargo fuzz run` reproduction command

Those scripts live under `tools/ci/` and are the authoritative reproduction
surface used by PR CI.

### Promotion Workflow

When a fuzz finding is real:

1. Reproduce it with the logged command or deterministic replay.
2. Minimize the bytes until one behavior remains.
3. Commit the minimized byte input under the matching `fuzz/regressions/`
   directory.
4. If the bug needs a stable parser-level assertion, promote it into
   `crates/html/tests/regressions/html5/rawtext_script/` with `meta.txt` plus
   `tokens.txt` and/or `dom.txt`.
5. Re-run corpus replay, the targeted smoke lane, and the promoted regression
   suite.

## What To Check When This Area Regresses

When investigating a text-mode bug, check:

- did the tokenizer terminate only on the expected tag name and allowed tail?
- did a near miss stay literal text?
- did whole-input and chunked execution diverge?
- did the matcher resume from prior progress instead of restarting from `<`?
- did RCDATA decode entities while RAWTEXT/script kept them literal?
- did a resource limit or stall guardrail fire, and if so, was recovery
  deterministic?

If the answer to any of those changes unexpectedly, treat it as a Borrowser
tokenizer bug and add or update a committed regression in the surfaces above.
