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

Regenerate expected outputs:

```bash
BORROWSER_HTML5_PATCH_FIXTURE_UPDATE=1 cargo test -p html --test html5_golden_tree_builder_patches --features html5
```
