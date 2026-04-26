# CSS Cascade Regressions

Promote minimized cascade-resolution crashing or invariant-breaking inputs here
after triage. Regressions in this directory are replayed deterministically by:

`make test-css-cascade-fuzz-corpus`

Replay one regression through the actual fuzz target with:

`cargo fuzz run css_cascade fuzz/regressions/css_cascade/<name>`
