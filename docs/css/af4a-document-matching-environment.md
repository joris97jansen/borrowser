# AF4a document matching environment contract

Last updated: 2026-08-16

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

AF4a itself did not implement quirks-mode ID/class comparison. AF4c now
consumes this environment inside CSS and selects ASCII-insensitive ID/class
value comparison only for `DocumentMode::Quirks`; `NoQuirks` and
`LimitedQuirks` remain sensitive. Complete historical DOCTYPE classification
remains deferred.

AF4c does not broaden the environment. Host-language name and attribute-value
policy is selected from exact selector-DOM namespace/attribute facts, while
ID/class policy consumes the existing mode. Browser/runtime still transports
and retains mode without learning selector comparison semantics. See
`docs/css/af4c-html-host-language-selector-comparison.md`.

## AF4b projection relationship

Document mode and selector projection provenance are independent facts.
`SelectorMatchingEnvironment` does not identify the document element, validate
a selector-DOM root, or distinguish a document from an isolated element
subtree. AF4b document construction records actual document-element identity;
AF4b subtree construction always reports no document element even though its
root has no in-projection parent.

A selector-DOM build error is not repaired by selecting a default mode and is
not a matching-environment mismatch. Cascade, computed style, debug, and
Browser callers preserve the typed construction error separately. AF4a still
does not implement `:root` or any structural pseudo-class.

AF4e closes the environment integration proof by parsing real DOCTYPE inputs
for all three modes, asserting `ParseOutput.document_mode`, constructing
`SelectorMatchingEnvironment` from that exact value, and matching against that
same `ParseOutput.document`. No manually injected mode substitutes for the
parser-to-matcher conformance evidence.
