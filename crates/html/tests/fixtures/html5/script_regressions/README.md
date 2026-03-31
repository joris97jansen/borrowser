# HTML5 Script Close-Tag Regression Fixtures

This suite is a curated regression corpus for script end-tag detection.

Each fixture is a directory containing:

- `input.html`: the HTML source for the regression
- `tokens.txt`: expected token stream in `html5-token-v1` format

Additional fixture contract:

- `tokens.txt` must include `# guard: ...`
- the `# guard:` text explains exactly what the regression is protecting
- the dedicated test runner executes each fixture in:
  - whole-input mode
  - deterministic every-UTF-8-boundary chunk mode

Input normalization:

- the loader strips exactly one trailing line ending from `input.html`
- this keeps fixture files editable with normal text-editor newlines without
  injecting an extra trailing text token into the regression

Scope:

- ASCII case-insensitive `</script>` termination
- near misses that must remain literal text
- incomplete close tags at EOF
- exact close tags embedded in longer source text
- repeated partial close-tag prefixes across chunk boundaries
