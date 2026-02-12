# HTML5 Tokenizer Golden Fixtures

Each fixture is a directory containing:

- `input.html`: the HTML input string
- `tokens.txt`: expected token stream in `html5-token-v1` format

`tokens.txt` format:

- Header lines begin with `#` and use `key: value`.
- Required header: `# format: html5-token-v1`
- Optional header: `# status: active | xfail | skip`
- Optional header (required if `status: xfail` or `status: skip`): `# reason: <text>`

Token lines are one per line, with the following forms:

- `DOCTYPE name=<name|null> public_id=<\"string\"|null> system_id=<\"string\"|null> force_quirks=<true|false>`
- `START name=<name> attrs=[<attr> ...] self_closing=<true|false>`
- `END name=<name>`
- `COMMENT text="..."`
- `CHAR text="..."`
- `EOF`

Attribute formatting:

- Attributes are emitted in the tokenizer order.
- Boolean attributes have no value: `attrs=[disabled]`.
- Valued attributes are `name="value"`, with `value` escaped.

Escaping rules (applies to `text` and attribute values):

- `\n`, `\r`, `\t`, `\\`, and `\"` are escaped.
- Other control chars (< 0x20) are encoded as `\u{XX}`.
- All other characters are emitted as-is.

Chunking policy:

- The harness uses the shared chunk plan generator in `html::chunker`.
- Deterministic plans include fixed sizes (1,2,3,4,8,16,32,64) and semantic boundaries around `<`, `</`, `>`, quotes, etc.
- Seeded fuzz plans are generated per fixture for CI reproducibility.
