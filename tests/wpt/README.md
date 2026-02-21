# WPT Parsing Subset

This folder contains a **curated subset** of Web Platform Tests (WPT) focused on **HTML parsing** (not layout).
The goal is to validate the tree builder and DOM snapshot outputs against a stable, minimal baseline while keeping
CI fast and deterministic.

## Selection Criteria

- Parsing-only behavior (tokenization, tree construction, DOM shape).
- No layout or rendering dependencies.
- Tests that exercise edge cases in HTML syntax and insertion modes.
- Small, deterministic fixtures that are easy to debug.

## Structure

- `manifest.txt`: list of curated tests and their expected snapshots.
- `vendor/`: vendored WPT HTML files (minimal subset).
- `expected/`: expected DOM snapshots in `html5-dom-v1` format.
  - Tokenizer cases use `html5-token-v1` with `kind: tokens` in the manifest.
- `tokenizer/skips.toml` + `tokenizer/skips.json`: tokenizer-only skip/xfail manifest
  with explicit reason and tracking issue reference; both files must stay in sync.
  - Policy: tokenizer `manifest.txt` entries stay `active` by default; temporary
    and policy exclusions are applied via `tokenizer/skips.*`.

**Manifest Schema**
- `id`: unique case identifier.
- `path`: relative path to the vendored HTML fixture.
- `expected`: relative path to the expected snapshot file.
- `kind`: optional (`dom` default, or `tokens` for tokenizer snapshots).
- `status`: optional (`active` default, `xfail` for expected failures, `skip` for policy-excluded cases).
- `reason`: required when `status` is `xfail` or `skip`.
- Policy: tokenizer out-of-scope/temporary exclusions are tracked in
  `tokenizer/skips.toml` + `tokenizer/skips.json` with reason + tracking issue.

## Update Workflow

1. Add or update the HTML file in `vendor/` (matching upstream WPT paths when possible).
2. Update `manifest.txt` with the new case.
3. Generate or edit the expected snapshot in `expected/`.
4. Run `cargo test -p html --features html5,dom-snapshot --test wpt_html5` to validate DOM+token mixed WPT cases.
5. Run `cargo test -p html --features html5 --test wpt_html5_tokenizer` for the tokenizer slice.

## Source / Upstream

These files are a curated subset of upstream WPT tests. When adding new cases,
record the upstream path and (when known) the upstream WPT commit in the manifest
or in a short note alongside the test.

## Notes

- Early on, tests can be marked `xfail` in the manifest with a reason.
- Tokenizer slice skips/xfails are maintained in `tokenizer/skips.toml` and
  mirrored in `tokenizer/skips.json` (the tokenizer runner validates parity).
- The runner uses `html::dom_snapshot::DomSnapshot` and the same UTF-8 aligned streaming policy as the golden harnesses.
- Filters: set `WPT_KIND=dom|tokens|all`, `WPT_FILTER=<substring>`, or `WPT_IDS=id1,id2` to run a subset.
- Chunked runs: set `WPT_CHUNKED=1` (optional `WPT_FUZZ_RUNS` and `WPT_FUZZ_SEED`).
- By default, chunked tokenizer runs are skipped when whole-input already mismatched expected to reduce noise; set `WPT_CHUNKED_FORCE=1` to force chunked diagnostics.
