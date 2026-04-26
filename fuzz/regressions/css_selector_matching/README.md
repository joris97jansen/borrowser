# CSS Selector Matching Regressions

Promote minimized selector-matching crashing or invariant-breaking inputs here
after triage. Regressions in this directory are replayed deterministically by:

`make test-css-selector-matching-fuzz-corpus`

Replay one regression through the actual fuzz target with:

`cargo fuzz run css_selector_matching fuzz/regressions/css_selector_matching/<name>`
