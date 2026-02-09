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

**Manifest Schema**
- `id`: unique case identifier.
- `path`: relative path to the vendored HTML fixture.
- `expected`: relative path to the expected snapshot file.
- `kind`: optional (`dom` default, or `tokens` for tokenizer snapshots).
- `status`: optional (`xfail` to mark expected failures).
- `reason`: optional (required when `status` is `xfail`).

## Update Workflow

1. Add or update the HTML file in `vendor/` (matching upstream WPT paths when possible).
2. Update `manifest.txt` with the new case.
3. Generate or edit the expected snapshot in `expected/`.
4. Run `cargo test -p html --test wpt_runner` to validate.

## Source / Upstream

These files are a curated subset of upstream WPT tests. When adding new cases,
record the upstream path and (when known) the upstream WPT commit in the manifest
or in a short note alongside the test.

## Notes

- Early on, tests can be marked `xfail` in the manifest with a reason.
- The runner uses `html::dom_snapshot::DomSnapshot` and the same UTF-8 aligned streaming policy as the golden harnesses.
