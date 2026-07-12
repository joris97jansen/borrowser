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

AE8 fixture band:

- `ae8-*` fixtures are the Milestone AE table-construction contract band. They
  cover explicit table modes, implied row/body construction, multiple table
  bodies, malformed row/cell recovery, foster-parented text/elements, and EOF
  flushing of pending table text.

Execution policy:

- whole-input execution is golden-checked
- deterministic chunk plans are checked
- seeded fuzz chunk plans are checked

The main DOM and patch golden harnesses load this corpus alongside the legacy
`tree_builder` and `tree_builder_patches` roots.
