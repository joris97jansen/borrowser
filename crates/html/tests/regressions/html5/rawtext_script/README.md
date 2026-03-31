# HTML5 Rawtext/Script Stable Regressions

This directory is the promotion target for minimized RAWTEXT/script fuzz findings
that should become stable regression tests.

Layout per fixture:

- `meta.txt`: required provenance + intent metadata
- `input.html`: minimized stable HTML input
- `tokens.txt`: optional tokenizer oracle in `html5-token-v1` format
- `dom.txt`: optional DOM oracle in `html5-dom-v1` format

At least one of `tokens.txt` or `dom.txt` must exist. A fixture may carry both
when the fuzz finding should be locked at tokenizer and semantic DOM levels.

`meta.txt` format:

- `# format: html5-rawtext-script-regression-v1`
- `# tool: html5_tokenizer_script_data | html5_tokenizer_rawtext`
- `# seed: <artifact name, corpus seed, or repro seed>`
- `# date: YYYY-MM-DD`
- `# issue: <tracking issue URL or stable issue id>`
  Accepted forms: `https://...`, `#1234`, `BOR-123`, `Milestone-L/L5`
- `# mode: script-data | rawtext-style`
- `# guard: <one-line statement of the protected behavior>`
- optional `# source: <fuzz corpus/regression path or artifact label>`

Naming convention:

- fixture directories use `rs-<mode>-<slug>-<date>`
- use lowercase ASCII kebab-case only
- keep the slug short and behavior-oriented

Promotion workflow:

1. Reproduce the fuzz finding from `fuzz/corpus/` or `fuzz/regressions/` with the
   logged `cargo fuzz run` or deterministic replay command.
2. Minimize the input until only the behavior under test remains.
3. If the original targeted fuzz seed is text-mode payload-only, wrap it into
   the smallest stable HTML document that still exercises the same RAWTEXT/script
   behavior under the real parser.
4. Create `tests/regressions/html5/rawtext_script/<fixture>/`.
5. Write `meta.txt` with the provenance headers above.
6. Add `input.html`.
7. Add `tokens.txt`, `dom.txt`, or both.
8. Run:

```sh
cargo test -p html --test html5_rawtext_script_regressions --features "html5 dom-snapshot parser_invariants"
```

What the harness checks:

- whole-input behavior matches the committed oracle
- every UTF-8 boundary split matches whole-input behavior exactly
- token regressions drive the real tokenizer control path for `script` / `style`
- DOM regressions run through the real tokenizer + tree builder pipeline

Use token snapshots when the finding is specifically about text-mode end-tag or
token emission behavior. Add a DOM snapshot when the finding also needs a stable
semantic tree-level assertion.
