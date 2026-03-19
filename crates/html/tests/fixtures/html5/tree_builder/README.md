# HTML5 Tree Builder Golden Fixtures

Each fixture is a directory containing:

- `input.html`: the HTML input string
- `dom.txt`: expected DOM snapshot in `html5-dom-v1` format

`dom.txt` format:

- Header lines begin with `#` and use `key: value`.
- Required header: `# format: html5-dom-v1`
- Optional header: `# status: active | xfail`
- Optional header (required if `status: xfail`): `# reason: <text>`
- Optional headers:
  - `# ignore_ids: true | false` (default `true`)
  - `# ignore_empty_style: true | false` (default `true`)

Snapshot lines:

- One line per node, using a deterministic, depth-indented format (2 spaces per level).
- The serializer is `html::html5::tree_builder::serialize_dom_for_test_with_options`.
- Attributes are emitted in lexical name order for snapshot stability.
- If `ignore_ids=false`, node IDs are rendered as a trailing `id=<n>` suffix on
  node lines (not as synthetic DOM attributes).

Why DOM snapshots:

- These fixtures are semantic regression coverage for materialized DOM behavior.
- Patch-contract acceptance and sequencing guarantees are covered by
  `tests/fixtures/html5/tree_builder_patches`.
- A stable DOM snapshot helps catch semantic tree regressions while patch
  protocol tests evolve independently.

Contract status:

- For Core v0 patch-contract acceptance, patch-level golden fixtures in
  `tests/fixtures/html5/tree_builder_patches` are authoritative.
- This DOM fixture corpus remains useful for semantic regression coverage.

Milestone H corpus:

- `h8-nested-*`: well-formed nested supported formatting coverage.
- `h8-aaa-*`: mis-nesting and AAA-driven end-tag recovery coverage.
- `h8-reconstruct-*`: active formatting reconstruction after ancestor pops.
- `h8-special-*`: repeated `a` / `nobr` start-tag recovery coverage.
- `h8-marker-*`: marker-boundary interaction coverage around `applet` / marker tags.

Core F11 coverage baseline:

- implicit document shell insertion (`html/head/body`) for fragment-like inputs,
- comment handling (before root and inside body subtrees),
- doctype propagation to document snapshots,
- nested element structure and in-body ordering,
- text coalescing semantics under whole + chunked + seeded-fuzz input delivery.

Node line grammar (examples):

- Document: `#document` or `#document doctype="html"`
- Element: `<div class="a" disabled>`
- Text: `"hello"`
- Comment: `<!-- comment -->`

Comment serialization rule:

- Comments are serialized as `<!-- {text} -->` with single surrounding spaces.

Escaping rules (for doctype/text/attributes/comments):

- `\n`, `\r`, `\t`, `\\`, and `\"` are escaped.
- ASCII is emitted as-is.
- Non-ASCII is encoded as uppercase-hex `\u{HEX}`.

Streaming policy:

- This harness uses UTF-8 aligned chunking only (matching the current `Input::push_str` pipeline).
- Byte-stream chunking will be added once the HTML5 byte decoder path is wired into the session.
- Chunk plans come from the shared generator in `html::chunker`.
- Deterministic plans include fixed sizes (1,2,3,4,8,16,32,64) and semantic boundaries around `<`, `</`, `>`, quotes, etc.
- Seeded fuzz plans are generated per fixture for CI reproducibility.

Input normalization policy:

- Loader strips one terminal line ending from `input.html` (`\n` or `\r\n`).
- This avoids editor-dependent semantic drift from POSIX-style trailing newlines.
- Any trailing newline that is semantically required by a fixture must be encoded explicitly in content, not as file-formatting newline.
