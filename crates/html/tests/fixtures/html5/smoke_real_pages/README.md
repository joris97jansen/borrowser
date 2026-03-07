# HTML5 Smoke Real Pages Corpus

This corpus is a curated set of real-world-style HTML snippets/pages for end-to-end
HTML5 parsing smoke coverage.

Each fixture directory contains:

- `input.html`: the page/snippet input
- `dom.txt`: expected DOM snapshot in `html5-dom-v1` format

Purpose:

- Sanity gate for HTML5 mode using realistic document shapes.
- Complements (does not replace) WPT and spec-focused golden tests.
- Regression gate for current HTML5-mode behavior under the feature flag.

Contract:

- Parse uses the HTML5 tokenizer + tree builder pipeline.
- Snapshot comparison uses deterministic DOM serialization.
- Failures produce line-based snapshot diffs.
- This corpus is not a normative conformance suite. Normative correctness is
  primarily enforced by WPT and focused golden fixtures.
- While HTML5 tree-builder behavior is still being completed, some snapshots may
  reflect current non-final behavior and should be updated toward spec-faithful
  output as implementation maturity improves.

Input normalization:

- The harness strips one terminal line ending from `input.html` (`\n` or `\r\n`)
  to avoid editor-formatting drift.

Update snapshots:

```bash
BORROWSER_HTML5_SMOKE_UPDATE=1 cargo test -p html --test html5_smoke_real_pages --features "html5 dom-snapshot"
```

Filter a subset:

```bash
BORROWSER_HTML5_SMOKE_FILTER=dashboard cargo test -p html --test html5_smoke_real_pages --features "html5 dom-snapshot"
```
