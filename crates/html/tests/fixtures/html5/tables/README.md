# HTML5 Table Fixture Corpus

This directory contains the dedicated Milestone I table-heavy golden corpus for
I10.

Layout:

- `dom/<fixture>/input.html`
- `dom/<fixture>/dom.txt`
- `patches/<fixture>/input.html`
- `patches/<fixture>/patches.txt`

Scope:

- normal table structure
- malformed table recovery with stray text/tags
- implied wrapper/close behavior
- foster-parenting behavior
- basic nested tables

Execution policy:

- whole-input execution is golden-checked
- deterministic chunk plans are checked
- seeded fuzz chunk plans are checked

The main DOM and patch golden harnesses load this corpus alongside the legacy
`tree_builder` and `tree_builder_patches` roots.
