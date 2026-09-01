# Borrowser conformance contracts

- AG1: federated ownership, scope, and orthogonal state.
- AG2: fixture discovery, logical `TestId`, packages, and manifests.
- AG3: logical-test classification and expected-result metadata.
- AG4: parser/DOM execution and parser report V1.
- AG5: CSS execution and CSS report V1.
- AG6: Layout/Paint structural execution and rendering report V1.
- AG7: static document-reference relations and rendering report V2.

AG6 is documented in
[`ag6-layout-paint-structural-conformance.md`](ag6-layout-paint-structural-conformance.md).
It adds backend-independent authored structural regression coverage. AG7 is
documented in
[`ag7-static-structural-reference-comparison.md`](ag7-static-structural-reference-comparison.md).
AG7 adds static exact-owner-byte reference comparison, not broad WPT, browser
compatibility, screenshots, or pixels.
