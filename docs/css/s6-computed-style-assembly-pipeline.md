# S6: Computed-Style Assembly Pipeline

Last updated: 2026-04-21  
Status: implemented

This document is the contract for Milestone S issue 6: deterministic assembly
of final `ComputedStyle` objects from structured cascade output.

Related code:
- `crates/css/src/computed.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/cascade/document.rs`
- `crates/browser/src/page.rs`
- `crates/browser/src/view.rs`

Related documents:
- `docs/css/s1-property-system-architecture-computed-style-contract.md`
- `docs/css/s4-computed-value-representations-normalization.md`
- `docs/css/s5-invalid-value-handling-fallback.md`

## Implemented Result

S6 adds the document-level computed-style pipeline:

```text
DOM + stylesheets
  -> ResolvedDocumentStyle
  -> ComputedDocumentStyle
  -> StyledNode tree
```

The core entrypoints are:

- `compute_document_styles(...)`
- `compute_document_styles_from_resolved_styles(...)`
- `build_style_tree_with_stylesheets(...)`
- `build_style_tree_from_computed_styles(...)`

The browser view path now builds its style tree through
`build_style_tree_with_stylesheets(...)` instead of relying on
`attach_styles(...)` to mutate `Node::Element::style`.

## Assembly Flow

For each element in selector document order:

1. cascade resolves a total `ResolvedStyle`
2. the computed layer finds the parent element's already-computed style, if
   one exists
3. `compute_style_from_resolved_style(...)` materializes each property source:
   - winning specified values normalize through S4
   - inherited entries copy the parent computed value
   - initial entries use the property metadata initial/default contract
4. `ComputedStyleBuilder` records every supported property and enforces total
   property coverage plus computed-value kind correctness
5. the result is stored as a `ComputedElementStyle` inside
   `ComputedDocumentStyle`

This keeps inheritance/defaulting and property computation in one explicit
pipeline without mutating a partially assembled `ComputedStyle`.

## Styled Tree Construction

`build_style_tree_from_computed_styles(...)` consumes a precomputed document
style result and the DOM tree in lockstep. It:

- assigns computed styles to element nodes in selector document order
- gives document/text/comment nodes inherited or initial style as appropriate
- validates each computed entry's `SelectorDomElementId`, element namespace,
  and canonical local name
  against a fresh selector index for the target DOM
- rejects mismatched DOM/style inputs instead of silently pairing styles with
  the wrong element
- does not read from or write to `Node::Element::style`

`build_style_tree_with_stylesheets(...)` composes document style resolution and
styled-tree construction for runtime consumers.

## Determinism

S6 preserves these invariants:

- element order comes from `SelectorDomIndex`
- property order comes from the property registry
- all computed styles are total over the supported property set
- computed document snapshots are stable
- malformed handoffs return `ComputedStyleResolutionError`
- styled-tree construction validates numeric selector identity and expanded
  element-name identity
- browser rendering no longer depends on DOM-attached style mutation

## Legacy Boundary

`attach_styles(...)`, `compute_style(...)`, and `build_style_tree(...)` still
exist as compatibility APIs for older tests and consumers. They are no longer
the primary runtime path for browser view construction.

Future cleanup should retire those compatibility APIs once all remaining
callers use the structured pipeline.

## Test Surface

S6 adds and relies on tests for:

- document-level cascade/inheritance/default/computation integration
- computed document-style snapshot determinism
- computing from an already materialized `ResolvedDocumentStyle`
- styled-tree construction without mutating `Node::style`
- mismatched DOM/computed-style name and identity rejection

These tests define the current computed-style assembly contract.
