# CSS Fuzz Regressions

This directory is the promotion target for minimized CSS fuzz findings that
must remain permanent regression fixtures.

Layout per fixture:

- `meta.txt`: required provenance + intent metadata
- `input.bin`: authoritative input bytes replayed through the matching CSS fuzz
  harness
- `summary.txt`: stable harness summary rendered by
  `css_fuzz_regression_summary`

`summary.txt` contract:

- `termination` uses stable lowercase labels such as `completed`,
  `rejected-max-input-bytes`, or `selector-matching-limit-exceeded`
- committed summaries must not depend on Rust `Debug` formatting or enum
  variant names

`meta.txt` format:

- `# format: css-fuzz-regression-v1`
- `# tool: css_tokenizer | css_parser | css_selector_parser | css_selector_matching | css_cascade | css_values`
- `# profile: default | selector-limit-zero`
- `# seed: <u64>`
- `# date: YYYY-MM-DD`
- `# issue: <tracking issue URL or stable issue id>`
  Accepted forms: `https://...`, `#1234`, `BOR-123`, `Milestone-T/T6`
- `# guard: <one-line statement of the protected behavior>`
- optional `# source: <fuzz corpus/regression path>`

Naming convention:

- fixture directories use `rg-<tool>-<slug>-<date>`
- use lowercase ASCII kebab-case only
- keep the slug short and behavior-oriented

Promotion workflow:

1. Reproduce the fuzz finding from `fuzz/corpus/` or `fuzz/regressions/`.
2. Minimize the input until only the guarded behavior remains.
3. Create `tests/regressions/css_fuzz/<fixture>/`.
4. Write `meta.txt` with the provenance headers above.
5. Add `input.bin` with the exact replay bytes.
6. Generate `summary.txt` with:

```sh
cargo run -p css --features css-fuzzing --bin css_fuzz_regression_summary -- \
  --tool <tool> --profile <profile> \
  --input crates/css/tests/regressions/css_fuzz/<fixture>/input.bin --seed <u64>
```

7. Run:

```sh
cargo test -p css --features css-fuzzing --test css_fuzz_regressions --locked
```

What the harness checks:

- metadata is well-formed and versioned
- every promoted fixture replays through the real CSS fuzz harness for its tool
- the rendered stable summary matches the committed oracle exactly
- every CSS fuzz tool has at least one promoted regression fixture
