# AF4a document matching environment contract

AF4a propagates the HTML parser's selected `DocumentMode` to CSS selector
evaluation. HTML owns mode selection; runtime and Browser transport and retain
the selected value without interpreting selector semantics. CSS owns the
immutable `SelectorMatchingEnvironment` used by matching, cascade, computed
style, debug, and retained-artifact validation.

## Parser readiness

The parser lifecycle distinguishes `Unselected` from `Selected(DocumentMode)`.
`NoQuirks` is never an unresolved sentinel. Initial comments, processing
instructions, and whitespace may be emitted while the mode is unselected; a
non-DOCTYPE Initial fallback selects Quirks before reprocessing. Successful
completed output carries a selected mode, and a selected mode cannot change for
the parser-created document.

## Publication boundary

A runtime `DocumentPublication` is one typed envelope containing a `DomHandle`,
selected mode, and exactly one patch payload. The patch payload owns the sole
`DomVersion` `from -> to` transition. The parser-local `html::DomPatchBatch`
sequence is not the Browser publication version; legacy snapshot events are
not part of the authoritative production protocol.

While mode is unselected, runtime retains patches in a hard-bounded
pre-selection buffer. Threshold pressure cannot publish metadata-free DOM.
Budget exhaustion is a typed terminal failure. Once selected, every subsequent
publication for the handle carries the same mode.

Browser stages and validates a candidate publication before committing its
`DomStore`, handle/version, retained mode, materialized DOM, `PageState`, and
affected render state. A failed publication leaves the state immediately before
the attempt unchanged. Navigation-start reset semantics are outside AF4a.

## CSS environment and artifact binding

`SelectorMatchingEnvironment` contains only the parser-selected
`DocumentMode`; it has no runtime identity, DOM generation, or retained-render
identity and has no `Default` implementation. Every authoritative matching
entry point receives it explicitly, including namespace-constrained context
reconstruction. Resolved and computed CSS artifacts retain the environment used
to produce them. Incremental reuse rejects an environment mismatch before
reusing prior CSS results. Browser cache eligibility remains separate from this
CSS semantic validity check.

AF4a does not implement quirks-mode ID/class comparison or complete historical
DOCTYPE classification.
