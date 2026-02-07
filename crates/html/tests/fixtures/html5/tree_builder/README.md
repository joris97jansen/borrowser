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
- The serializer matches `html::dom_snapshot::DomSnapshot` with the configured options.

Why DOM snapshots:

- The golden contract is *DOM semantics*, not patch sequencing.
- Patch logs are great for debugging but churn when patching evolves (keys, ordering, batching).
- A stable DOM snapshot lets us refactor patch emission without rewriting the corpus, as long as the resulting DOM is identical.

Node line grammar (examples):

- Document: `#document` or `#document doctype="html"`
- Element: `<div class="a" disabled>`
- Text: `"hello"`
- Comment: `<!-- comment -->`

Escaping rules (for doctype/text/attributes/comments):

- `\n`, `\r`, `\t`, `\\`, and `\"` are escaped.
- ASCII is emitted as-is.
- Non-ASCII is encoded as `\u{HEX}`.

Streaming policy:

- This harness uses UTF-8 aligned chunking only (matching the current `Input::push_str` pipeline).
- Byte-stream chunking will be added once the HTML5 byte decoder path is wired into the session.
- Chunk plans come from the shared generator in `html::chunker`.
- Deterministic plans include fixed sizes (1,2,3,4,8,16,32,64) and semantic boundaries around `<`, `</`, `>`, quotes, etc.
- Seeded fuzz plans are generated per fixture for CI reproducibility.
