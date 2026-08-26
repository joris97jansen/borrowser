# CSS Representative Page Corpus

This corpus validates the structured CSS pipeline against curated
real-world-style HTML and author CSS inputs.

`crates/css/tests/representative_pages.rs` parses every `input.html` through
AE's production `html::parse_document` entry point, carries the parser-selected
document mode into `SelectorMatchingEnvironment`, parses `author.css` through
the production CSS model path, and calls `compute_document_styles` over the
parser-created DOM. Hand-built or fixture-only DOM adapters are not used by
this corpus.

Each fixture directory contains:

- `input.html`: representative page or component markup
- `author.css`: author stylesheet applied to the page
- `meta.txt`: fixture metadata with a required `# guard:` line
- `computed.snap`: deterministic computed-style snapshot

Purpose:

- Exercise parsing, selector matching, cascade, inheritance, inline style, and
  computed-style assembly on realistic page structures.
- Provide AF10's representative document-level evidence that the supported AF
  pipeline operates on AE parser-created DOM.
- Capture regressions as reproducible fixtures rather than one-off synthetic
  tests.
- Complement focused unit tests, fuzz regressions, and performance guards.

These fixtures are curated snippets, not archived third-party pages. They are
not a normative web-platform conformance suite and do not imply broad CSS
selector, cascade, property, layout, paint, or WPT conformance.

The computed snapshots are regression records produced by the real pipeline.
They supplement the owning semantic tests and must not be regenerated as a
substitute for understanding an expected behavioral change.

Update snapshots:

```bash
BORROWSER_CSS_REPRESENTATIVE_UPDATE=1 cargo test -p css --test representative_pages
```

Filter a subset:

```bash
BORROWSER_CSS_REPRESENTATIVE_FILTER=dashboard cargo test -p css --test representative_pages
```
