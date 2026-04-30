# CSS Representative Page Corpus

This corpus validates the structured CSS pipeline against curated
real-world-style HTML and author CSS inputs.

Each fixture directory contains:

- `input.html`: representative page or component markup
- `author.css`: author stylesheet applied to the page
- `meta.txt`: fixture metadata with a required `# guard:` line
- `computed.snap`: deterministic computed-style snapshot

Purpose:

- Exercise parsing, selector matching, cascade, inheritance, inline style, and
  computed-style assembly on realistic page structures.
- Capture regressions as reproducible fixtures rather than one-off synthetic
  tests.
- Complement focused unit tests, fuzz regressions, and performance guards.

These fixtures are curated snippets, not archived third-party pages. They are
not a normative web-platform conformance suite.

Update snapshots:

```bash
BORROWSER_CSS_REPRESENTATIVE_UPDATE=1 cargo test -p css --test representative_pages
```

Filter a subset:

```bash
BORROWSER_CSS_REPRESENTATIVE_FILTER=dashboard cargo test -p css --test representative_pages
```
