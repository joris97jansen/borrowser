# HTML5 Tree Builder Patch Golden Fixtures

Each fixture directory contains:

- `input.html`: source HTML.
- `patches.txt`: expected patch stream in `html5-dompatch-v1`.

`patches.txt` headers:

- `# format: html5-dompatch-v1` (required)
- `# status: active | xfail` (optional, default `active`)
- `# reason: ...` (required for `xfail`)

Body format:

- One deterministic patch record per line.
- Batch boundaries are explicit:
  - `Batch index=<n> size=<k>` precedes every batch.
  - Following lines are ordered patch records for that batch.

Input normalization policy:

- Loader strips one terminal line ending from `input.html` (`\n` or `\r\n`).
- This prevents editor-dependent trailing-newline drift in patch fixtures.

Contract intent:

- Patch line order is normative.
- Batch boundary/size lines are normative for whole-input goldens.
- Chunked/fuzz runs enforce patch ordering equivalence and materializability; batch partition may differ with input chunk boundaries.
- Any change in patch ordering produces a direct line diff.
- Attributes in patch text formatting are rendered in lexicographic name order, then value, for deterministic output.
- Value tie-break ordering is `None` (rendered as `<none>`) before `Some("...")`.

Milestone H corpus:

- `h8-nested-*`: well-formed supported-formatting patch sequencing.
- `h8-aaa-*`: mis-nesting / adoption agency patch sequencing and fresh-key recreation.
- `h8-reconstruct-*`: reconstruction patch sequencing after generic ancestor pops.
- `h8-special-*`: repeated `a` / `nobr` recovery sequencing.
- `h8-marker-*`: marker-boundary interaction coverage with formatting state.
- `h10-aaa-*`: canonical `AppendChild` / `InsertBefore` move encoding and stable key-preserving AAA reparenting.


Regenerate expected outputs:

```bash
BORROWSER_HTML5_PATCH_FIXTURE_UPDATE=1 cargo test -p html --test html5_golden_tree_builder_patches --features html5
```
