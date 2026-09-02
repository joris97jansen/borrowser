# Borrowser conformance contracts

- AG1: federated ownership, scope, and orthogonal state.
- AG2: fixture discovery, logical `TestId`, packages, and manifests.
- AG3: logical-test classification and expected-result metadata.
- AG4: parser/DOM execution and parser report V1.
- AG5: CSS execution and CSS report V1.
- AG6: Layout/Paint structural execution and rendering report V1.
- AG7: static document-reference relations and rendering report V2.
- AG8: bounded WPT-source interpretation, filtering, provenance, and complete
  source-record accounting.
- AG9 Stage 0: aggregate-reporting, cross-engine evidence, comparable DOM,
  capture-identity, bounds, and trend contracts. Execution and publication are
  not implemented by Stage 0.

AG6 is documented in
[`ag6-layout-paint-structural-conformance.md`](ag6-layout-paint-structural-conformance.md).
It adds backend-independent authored structural regression coverage. AG7 is
documented in
[`ag7-static-structural-reference-comparison.md`](ag7-static-structural-reference-comparison.md).
AG7 adds static exact-owner-byte reference comparison, not broad WPT, browser
compatibility, screenshots, or pixels.

AG8 is documented in
[`ag8-wpt-import-filtering-classification.md`](ag8-wpt-import-filtering-classification.md).
It adds an exact seven-record WPT proof population and one subsystem-owned
derived Paint-semantic fixture. It does not add a WPT runner, JavaScript,
WebDriver, WPT-server execution, networking, screenshots, or raster comparison.

The AG9 Stage 0 contract is documented in
[`ag9-cross-engine-comparison-reporting.md`](ag9-cross-engine-comparison-reporting.md).
It freezes future aggregate accounting, advisory cross-engine evidence,
`web-observable-dom-tree-v1`, capture identity, fixed bounds, trend semantics,
and CI/local boundaries. Stage 0 is documentation only: aggregate execution,
new reports, external capture loading/comparison, trends, and CLI/CI publication
remain unimplemented.
