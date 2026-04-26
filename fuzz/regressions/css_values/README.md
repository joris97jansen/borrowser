# CSS Values Regressions

Promote minimized property/value crashing or invariant-breaking inputs here
after triage. Regressions in this directory are replayed deterministically by:

`make test-css-values-fuzz-corpus`

Replay one regression through the actual fuzz target with:

`cargo fuzz run css_values fuzz/regressions/css_values/<name>`
