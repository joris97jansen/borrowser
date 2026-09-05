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
  capture-identity, bounds, and trend contracts.
- AG9 Stage 1, AG9a, and AG9b: one typed aggregate execution/accounting model,
  deterministic bounded aggregate summary/detail report projections, and the
  validated source-neutral external-capture/advisory-registry plane.

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

The staged AG9 contract is documented in
[`ag9-cross-engine-comparison-reporting.md`](ag9-cross-engine-comparison-reporting.md).
Stage 0 freezes the architecture and later-stage contracts. Stage 1 provides
the typed `AggregateRun` and checked logical/variant accounting. AG9a adds
`borrowser-conformance-aggregate-summary-v1` and
`borrowser-conformance-aggregate-detail-v1`, exact logical-member/source-set
identity, a derived 6,073-byte summary ceiling, and a 32 MiB detail bound. Both
reports derive from the same typed run; parser V1, CSS V1, and rendering V1/V2
remain unchanged and authoritative for subsystem evidence. AG9b implements the
complete cross-engine registry V1 schema, deterministic validation phases,
full comparable-artifact validation, same-object confined reads and
verified-byte lifetime, explicit resource/cumulative artifact bounds, opaque
capture-ID construction, and the separate advisory evidence plane.

AG9b loads and validates checked-in external capture declarations, artifacts,
advisory tracks, typed aggregate attachments, and baseline notes. It does not
add external capture tooling, external browser comparison, advisory comparison
verdicts, trend parsing/execution/comparison, aggregate CLI/CI publication,
browser automation, raster comparison, WPT-specific aggregate loading, or
production runtime behavior.

## AG9c comparable DOM and selected advisory operations

The independent Rust and JavaScript V1 producers use one neutral reviewed corpus
in `tests/contract-vectors/web-observable-dom-tree-v1/`, outside AG2 discovery.
Do not generate expected bytes from either producer. Selected operations retain
one exact variant's comparable observation from its existing evaluation; they
are not complete advisory-population reports. External evidence never changes
Borrowser semantic outcomes, policy, identities, or existing reports.

Real-browser capture remains unsupported until a mechanism proves the frozen
input context; do not populate the real registry with synthetic results. See
[AG9c ownership, APIs, capture restrictions, and validation](ag9c-external-dom-capture.md).
