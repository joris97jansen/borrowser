# CSS Selector Parser Regressions

Promote minimized selector-parser crashing or invariant-breaking inputs here
after triage. Regressions in this directory are replayed deterministically by:

`make test-css-selector-parser-fuzz-corpus`

Replay one regression through the actual fuzz target with:

`cargo fuzz run css_selector_parser fuzz/regressions/css_selector_parser/<name>`
