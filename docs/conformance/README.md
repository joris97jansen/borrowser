# Borrowser conformance contracts

- AG9g0d Pass 1: [AWS EC2 authority-generation foundation](ag9g0d-qualification-host-lifecycle.md).
  Independent acquisition/recovery/cancellation tooling; no host qualification.
  Production controller verification and real provider lifecycle validation remain external prerequisites.

- AG9g Stage 0: [Chromium/Linux mechanism qualification](ag9g-admitted-static-dom-capture.md)
  and [future evidence/admission contracts](ag9g-collection-evidence-admission-v1.md).
  Real mechanism feasibility remains unqualified; admission and collection are unavailable.

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
- AG9 Stage 1/AG9a: typed aggregate execution/accounting and bounded reports.
- AG9b/AG9c: validated advisory registry and comparable-DOM machinery; no
  admitted real-browser capture mechanism.
- AG9d/AG9d1: deterministic historical baselines/trends and named-lane validation.
- AG9e: aggregate CLI/CI publication and local workflows.
- AG9f: [non-normative requirement/evidence closeout audit](ag9f-requirement-evidence-closeout.md).
  Parent comparison/collection sufficiency remains unresolved; AG completion is
  not asserted. Run-specific local and pending hosted evidence is kept separately
  in the Phase A review packet.

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

AG9d adds the binary `borrowser-conformance-baseline-v1` historical envelope
and deterministic `borrowser-conformance-trend-v1` comparison of exactly two
digest-verified files. Logical cases, execution variants, advisory comparison
points, and baseline notes retain separate membership and change accounting.
Independent reviewed protocol vectors for the baseline, trend, and historical
capture identity live under `tests/contract-vectors/`; each vector directory
documents exact section or field offsets and SHA-256 identities.
See
[`ag9d-historical-baseline-trend.md`](ag9d-historical-baseline-trend.md).

AG9b loads and validates checked-in external capture declarations, artifacts,
advisory tracks, typed aggregate attachments, and baseline notes. AG9c adds
selected advisory operations and AG9e publication. The checked-in registry has
zero captures, attachments, tracks, and notes; synthetic vectors are machinery
tests only. Browser automation, admitted real capture, raster comparison, broader
external-source loading, and production runtime adapters remain gaps. See the
[gap reconciliation](ag9f-requirement-evidence-closeout.md#existing-gap-ledger)
and [contributor workflows](../../tests/conformance/README.md#aggregate-reports-and-historical-evidence).

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

## AG9e CLI and local publication

Build/run the binary with `--no-default-features --features aggregate`. The
[AG9 command/exit contract](ag9-cross-engine-comparison-reporting.md#ag9e-cli-and-publication-contract)
keeps ordinary summary/detail independent of optional capture evidence.

| Make target | Required environment values | Artifact |
| --- | --- | --- |
| `check-conformance-aggregate` | none; explicit normal-ci check | Aggregate summary V1 |
| `conformance-aggregate-detail` | `LANE` | Aggregate detail V1 |
| `conformance-aggregate-baseline` | `LANE` | Baseline V1, repository evidence reconciled without evaluation |
| `conformance-aggregate-external-baseline` | `LANE`, `TEST_ID` | Baseline V1, selected DOM advisory operation |
| `conformance-aggregate-trend` | `FROM_ROOT`, `FROM`, `FROM_SHA256`, `TO_ROOT`, `TO`, `TO_SHA256` | Trend V1 |

Only the summary belongs to normal publication CI. Local targets use report-only
policy; use the CLI's optional `--check` for checked local execution. Values are
forwarded as argv data; prefer environment values for literal shell-sensitive
paths, since Make command-line assignments also have Make's own expansion rules.
No browser provisioning is required. See the
[baseline/checksum/trend workflow](ag9d-historical-baseline-trend.md#ag9e-local-baseline-and-trend-workflow)
and [selected-operation limits](ag9c-external-dom-capture.md#ag9e-selected-baseline-publication).
AG9d1 aligns historical validation with existing named-lane policy, including
valid lane-excluded baseline/trend round trips.
