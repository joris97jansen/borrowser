# HTML5 Tokenizer Hardening

This document defines the Borrowser HTML5 tokenizer hardening posture for
untrusted input, the limits currently enforced by the tokenizer, and the
expected local workflow for reproducing and triaging fuzz findings.

The tokenizer implementation discussed here lives under
`crates/html/src/html5/tokenizer/`.

For RAWTEXT / RCDATA / script-specific termination rules, limits, fuzz lanes,
and promoted regression surfaces, see
[`docs/html5/rawtext-script-stability.md`](../html5/rawtext-script-stability.md).

## Threat Model

The HTML5 tokenizer must treat document input as untrusted.

Inputs in scope:
- arbitrary network-provided HTML bytes
- invalid UTF-8 byte streams before decoding
- adversarial chunking, including 1-byte chunk boundaries
- malformed markup intended to trigger parser edge cases
- oversized constructs intended to drive pathological CPU or memory behavior

Security goals:
- no panics from adversarial document bytes when tokenizer API contracts are respected
- no infinite loops or silent stalls
- bounded tokenizer behavior under configured resource limits
- deterministic recovery once a limit or guardrail is hit
- chunk-equivalent output for equivalent byte streams

Non-goals / out of scope:
- making internal API misuse silently recoverable
- recovering from engine invariant breaches such as foreign `Input` /
  `AtomTable` bindings
- bypassing process-level out-of-memory conditions

## What "Panic-Free" Means

For Borrowser, "panic-free" means:

- untrusted document bytes must not panic the tokenizer
- invalid UTF-8, malformed markup, and adversarial chunking must be handled by
  deterministic recovery or parse errors
- fuzzing and deterministic corpus replay should not discover panic paths from
  document input alone

It does **not** mean:

- internal API misuse can never panic
- engine invariant breaches are converted into normal parse errors

The tokenizer explicitly documents this distinction in
`crates/html/src/html5/tokenizer/mod.rs`.

## Enforced Limits

The central limit surface is [`TokenizerLimits`](../../crates/html/src/html5/tokenizer/api.rs)
in `crates/html/src/html5/tokenizer/api.rs`. It is carried via
`TokenizerConfig`.

Current default limits:

| Limit field | Default | Recovery posture |
| --- | ---: | --- |
| `max_tokens_per_batch` | `1024` | Stop the current pump and yield so the caller can drain queued tokens |
| `max_tag_name_bytes` | `1024` | Truncate the emitted tag name to the allowed UTF-8 prefix |
| `max_attribute_name_bytes` | `1024` | Truncate the emitted attribute name to the allowed UTF-8 prefix |
| `max_attribute_value_bytes` | `16 * 1024` | Truncate the attribute value before entity decoding |
| `max_attributes_per_tag` | `256` | Drop additional attributes deterministically |
| `max_comment_bytes` | `64 * 1024` | Truncate the emitted comment token |
| `max_doctype_bytes` | `8 * 1024` | Force doctype quirks / bogus recovery |
| `max_end_tag_match_scan_bytes` | `64 * 1024` | Abandon the oversized RAWTEXT / RCDATA / script end-tag candidate and treat it as literal text |

Limit hits record `ParseErrorCode::ResourceLimit` with limit-specific detail
strings from `crates/html/src/html5/tokenizer/limits.rs`.

These limits intentionally prefer:
- boundedness
- state integrity
- deterministic recovery

over exact token fidelity once a construct has already exceeded policy.

### Why These Limits Exist

- `max_tokens_per_batch` prevents unbounded queue growth inside one pump call.
- name/value/comment/doctype byte ceilings prevent a single construct from
  forcing unbounded buffering or repeated growth.
- `max_attributes_per_tag` prevents adversarial attribute floods.
- `max_end_tag_match_scan_bytes` bounds resumable text-mode close-tag matching
  in RAWTEXT / RCDATA / script states.

## Stall Guardrail

Separate from the resource limits, the tokenizer also has a consecutive stalled
progress guardrail in `crates/html/src/html5/tokenizer/stall.rs`.

If the state machine repeatedly reports `Progress` without consuming input or
emitting tokens:
- debug / test / `parser_invariants` builds fail fast
- other builds recover deterministically by clearing transient tokenizer state
  and consuming one scalar as literal text

That recovery records `ParseErrorCode::ImplementationGuardrail`.

## Invariants and Debug Hardening

In debug, test, and `parser_invariants` builds, the tokenizer additionally
checks:
- observable pump progress contracts
- cursor and internal byte offsets
- queued span validity
- EOF / EOS consistency

The invariant surface lives in
`crates/html/src/html5/tokenizer/invariants.rs`.

## Fuzzing and Deterministic Replay

Committed inputs:
- seed corpus: `fuzz/corpus/html5_tokenizer/`
- triaged regressions: `fuzz/regressions/html5_tokenizer/`

The deterministic byte-stream fuzz harness lives under:
- `crates/html/src/html5/tokenizer/fuzz/`
- `crates/html/src/html5/tokenizer/fuzz/tests/corpus.rs`
- `fuzz/fuzz_targets/html5_tokenizer.rs`

### Replay Committed Inputs Deterministically

Replay the committed corpus and committed regressions through normal test
infrastructure:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

Equivalent Make target:

```sh
make test-html5-tokenizer-fuzz-corpus
```

Replay a single committed seed deterministically:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_single_committed_seed_deterministically
```

### Run the Actual Fuzz Target

If `cargo-fuzz` is installed:

```sh
cargo fuzz run html5_tokenizer \
  fuzz/corpus/html5_tokenizer \
  fuzz/regressions/html5_tokenizer
```

### PR Smoke and Long-Run Lanes

Deterministic PR smoke:

```sh
make test-html5-tokenizer-fuzz-smoke
```

Deterministic nightly / long run:

```sh
make test-html5-tokenizer-fuzz-long
```

Both lanes are driven by:

```sh
bash ./tools/ci/html5_tokenizer_fuzz_smoke.sh
```

and print:
- seed
- input directories
- failing artifact path when materialized
- direct-binary repro command
- `cargo fuzz run` repro command

## Reproducing CI Failures Locally

When CI uploads a crashing artifact:

1. Download the artifact.
2. Run the logged direct-binary repro command, or:

```sh
cargo fuzz run html5_tokenizer <artifact-path>
```

When no artifact was materialized, rerun the logged command using the exact
input directories printed by the script. The script intentionally logs the full
executed input set so nightly/manual runs that included regressions can be
reproduced faithfully.

## Adding a Regression From a Fuzz Finding

1. Reproduce the failure locally.
2. Minimize the reproducing bytes.
3. Commit the minimized input under:

```text
fuzz/regressions/html5_tokenizer/<descriptive-name>
```

4. Re-run deterministic replay:

```sh
cargo test -p html --features html5 --lib \
  html5::tokenizer::fuzz::tests::corpus::replay_committed_html5_tokenizer_corpus_deterministically
```

5. Re-run the smoke lane:

```sh
make test-html5-tokenizer-fuzz-smoke
```

Use small, focused inputs. One file should ideally capture one failure mode.

## Adjusting Limits Safely

Only adjust tokenizer limits through [`TokenizerLimits`](../../crates/html/src/html5/tokenizer/api.rs).
Do not introduce one-off hardcoded caps in individual state handlers.

When changing a limit:

1. Update the relevant `TokenizerLimits` field or caller configuration.
2. Keep recovery deterministic.
3. Preserve the parse-error path (`ParseErrorCode::ResourceLimit`) unless the
   policy is intentionally changing.
4. Add or update targeted limit tests in
   `crates/html/src/html5/tokenizer/tests/limits.rs`.
5. Re-run deterministic corpus replay and fuzz smoke.

Guidance:
- Lowering limits is acceptable, but expect more lossy recovery and regression
  churn.
- Raising limits should be justified with a concrete product need and checked
  against perf, memory, and fuzz behavior.
- `max_attributes_per_tag` intentionally allows `0`; byte-oriented limits are
  clamped to at least one byte internally.

## Related Files

- tokenizer API and limits:
  `crates/html/src/html5/tokenizer/api.rs`
- limit enforcement:
  `crates/html/src/html5/tokenizer/limits.rs`
- invariant checks:
  `crates/html/src/html5/tokenizer/invariants.rs`
- stall detector:
  `crates/html/src/html5/tokenizer/stall.rs`
- fuzz harness:
  `crates/html/src/html5/tokenizer/fuzz/`
- committed replay tests:
  `crates/html/src/html5/tokenizer/fuzz/tests/corpus.rs`
- CI runner:
  `tools/ci/html5_tokenizer_fuzz_smoke.sh`
- fuzz workflow notes:
  `fuzz/README.md`
