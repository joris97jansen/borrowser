# AG8 deterministic reporting and bounded suites

AG8 is harness tooling. It selects existing fixtures, summarizes existing
verdicts, and makes failures reproducible. It adds no engine semantics, fixtures,
WPT coverage, expected-difference baseline or execution backend. AG5 observations,
AG6 discovery and AG7 relationships remain authoritative.

## Selection and bounded cost

`--suite ci` and `--suite extended` select fixed, reviewed ID lists in
`crates/conformance/src/fixtures.rs`. The lists are independent of current results:
a required member remains required when it regresses. Restoring green CI by
removing a regressing member is not an acceptable suite update.

| CI member(s) | Coverage and regression value |
| --- | --- |
| `canvas/root`, `canvas/cascade` | Production root canvas paint and cascade specificity, sampled through the guarded opaque canvas collector. |
| `dom/structure`, `dom/template-namespaces` | Production parser structure, meaningful text/attributes, template contents and namespaces. |
| `style/cascade-inheritance` | CSS-owned computed values across cascade, inheritance and display, with explicit targets. |
| `layout/block-boxes` | Production fixed block geometry, including hidden, zero-sized and absent targets. |
| `wpt-style/background-color-mismatch` | A direct mismatch relationship over present-target computed background color, executing test and reference separately. |

All seven cases use embedded static inputs, fixed requests and environment,
independent expectations or an explicit relationship. None uses Chromium, AWS,
page script, network resources, time or live input. Their small DOM/style/layout
surfaces and eight executable documents give a bounded regression check across
the current engine surface. Membership is coverage-driven, not a list computed
from passing results. Normal XFAIL/UNSUPPORTED/SKIP semantics still apply if
reviewed fixture metadata describes that behavior.

Extended has those seven plus `wpt/rgb-alias-p6`, `wpt/rgb-comments-p6`,
`wpt/unsupported/first-child-js` and `wpt/unsupported/testdriver`: eleven cases,
nine runnable cases, twelve executable documents and two preclassified exclusions.
The RGB cases retain independently expected failures, successful scoped
relationships and incomplete original visual coverage.

Adding a discovered fixture expands neither named suite. Membership changes
require an explicit reviewed list change, coverage/cost justification and updates
to exact membership/document-count tests. There is no registry, suite file,
metadata flag, inheritance or automatic eligibility policy. The ordinary CI step
has a two-minute timeout after the existing debug build; warm execution should
ordinarily take seconds. This is an external bound, not a new harness timeout or
a timing-based correctness assertion. The existing parser deadline is not a
whole-render deadline. Compilation is outside the warm-run expectation.

The entire catalogue is validated before filtering. A selected named suite also
requires every listed member to exist exactly once, in sorted order. No-argument
and exact-ID selection preserve AG6 behavior, including independent temporary
catalogues without the named members. `--suite ci canvas/root` narrows to that
member; an ID outside the suite, unknown suite, duplicate option or missing value
is an invocation error. Results retain lexicographic catalogue order and each
case's test-before-reference order. References are documents, not extra cases.

```sh
make conformance-ci
make conformance-extended
cargo run -q -p borrowser-conformance --locked -- --suite ci canvas/root
# Existing all-catalogue and exact-ID paths:
make conformance
cargo run -q -p borrowser-conformance --locked -- canvas/root
# Explicit supported native Linux x86-64 comparison/capture:
cargo run -q -p borrowser-conformance --locked -- --compare --suite ci
cargo run -q -p borrowser-conformance --locked -- --compare --suite extended
cargo run -q -p borrowser-conformance --locked -- --chromium --suite ci
```

Chromium commands use the pinned `BORROWSER_CHROMIUM_EXECUTABLE` or explicit
`--chromium-executable PATH`. Selection is resolved by harness orchestration;
Chromium receives already-selected `CaptureInput` values. It has no suite policy.
AWS keeps its existing all-catalogue/exact-ID interface and remains suite-unaware.
It is an optional remote Chromium backend, never an ordinary engine CI prerequisite.

## Case totals and independent facts

`report::CaseReport::status()` remains the aggregate case authority;
`independent_status()` remains independent expectation authority. `CaseCounts`
is only a derived count of those aggregate statuses, not another verdict type.

| Existing aggregate | Count field | Local exit contribution |
| --- | --- | --- |
| PASS | `passed` | 0 |
| FAIL | `failed` | 1 |
| XFAIL | `expected_failed` | 0 |
| XPASS | `unexpected_passed` | 1 (stale expectation is actionable) |
| UNSUPPORTED | `unsupported` | 0 |
| SKIP | `skipped` | 0 |
| ERROR | `errored` | 2 |
| INCOMPLETE | `incomplete` | 3 |

Each evaluated case contributes once. Local summary order is always
`PASS FAIL XFAIL XPASS UNSUPPORTED SKIP ERROR INCOMPLETE`, including zero counts.
`INDEPENDENT_EXPECTATIONS XFAIL=n` is a separate fact count, never added to case
totals. `RUN harness_errors=0` describes a completed invocation; a real entered
case ERROR remains in `SUMMARY ERROR`, and must be checked as well.

Keep AG5/AG7 precedence exactly: local ERROR takes precedence over FAIL/XPASS,
then INCOMPLETE, then success. A partial requested projection that matches is
INCOMPLETE. A known mismatch can still aggregate XFAIL while reporting partial
coverage; AG8 does not reinterpret that existing contract. Runnable original
upstream visual assertions make aggregate results INCOMPLETE even when their
independent expectation is XFAIL and their scoped relationship is satisfied.

The initial CI suite returns 0 with seven PASS cases. Extended locally returns
3 with seven PASS, two INCOMPLETE, two UNSUPPORTED and two independent XFAIL
facts (aggregate XFAIL is zero). `make` propagates recipe failure as its own
nonzero status; it does not preserve the binary's numeric exit 3.

Cross-engine fields, reference documents and relationship assertions retain their
existing counters and precedence: any field difference, independent FAIL/XPASS
or relationship failure returns 1; otherwise incomplete cases, unsupported
fields or no equal fields return 3; otherwise 0. Fatal invocation errors return
2. Explicitly excluded cases can therefore yield incomplete comparison coverage
without becoming failing Borrowser cases. Existing four AG7 RGB differences
remain visible and make extended comparison exit 1. No accepted-nonzero wrapper,
expected-difference suppression, fallback or conversion into skips is permitted.
The qualification test checks the exact known facts and rejects unrelated changes;
it is not a production baseline mechanism.

## Comparison schema and reproduction

Comparison reports use **`borrowser.cross-engine-phases.v3`**. The previous schema
has no documented additive-field compatibility promise, and existing consumers
assert its identity/summary structure. V3 deliberately adds:

- `cases[].borrowser_status`: the existing aggregate Borrowser case status;
- `summary.borrowser_cases`: the eight derived totals above;
- `selection`: suite (`ci`, `extended`, or legacy `catalogue`) and execution mode;
- `cases[].reproduce`: a canonical exact-ID command within that selection.

Existing `cases[].status`, `borrowser_expectation`, projections, field differences,
relationships, coverage and summary facts preserve their meanings. A cross-engine
`different` can coexist with Borrowser PASS. An independent XFAIL can coexist with
aggregate INCOMPLETE. The reference document receives neither a test verdict nor
an independent expectation. A comparison ERROR during construction is fatal and
never published as a manufactured case.

The Chromium wire schema stays `borrowser.chromium-phases.v2`, profile stays
`ag7-static-references-v1`, AG4 manifest stays version 2, and remote capture format,
limits and strict decoding stay unchanged. No schema negotiation is introduced.
Historical AG7 qualification reports retain their original v2 identity.

Local reports include case ID, source and metadata paths, suite, exact-ID command,
reference source when applicable, fixed environment and existing expected/actual
differences. Metadata identifies the request and authored expectation file.
Comparison reports retain descriptors, source hashes, requests, engine capture
identity, projections and paths/values for differences. Commands use explicit
configuration placeholders (`$BORROWSER_CHROMIUM_EXECUTABLE`, or `$AG4_CONFIG`,
`$AG4_ARTIFACT_DIR`, `$AG4_STATE_DIR`), avoiding machine paths in stable stdout.
Run from the checkout used to build the executable; a copied executable can use
the same arguments without Cargo or source files. On exit 2, stderr also gives
the shell-quoted actual invocation and relevant Chromium executable setting.
Do not infer suite membership from results or include timestamps, durations,
operational AWS identities or arbitrary environment dumps in stable reports.

## Fatal run failures and publication

A real case ERROR means a local case entered execution/evaluation and failed.
Configuration, selection, capture preparation, transport and publication failures
can occur without any case result. They do not create ERROR, SKIP, PASS or other
outcomes for selected but unexecuted cases. Fatal comparison construction retains
all-or-nothing publication: execute/capture correspondence, projection, evaluation
and serialization finish before any JSON byte. Earlier successful work does not
permit publishing partial cases after a later fatal error.

Existing detailed diagnostics retain their primary and cleanup errors. A separate
terminal stderr record summarizes one failed invocation, for example:

```text
RUN_ERROR scope=invocation errors=1 origin=chromium code=chromium.configuration case_results=not_published
```

`errors=1` counts a failed invocation, not diagnostics, documents or cases. It is
not added to case totals and contains no fictional zero-valued case summary.
`origin=harness` identifies command/catalogue/required-data/report errors;
`origin=chromium` identifies capture/platform/configuration/cleanup errors;
`origin=aws` identifies remote execution/configuration failures. Existing detailed
AWS diagnostics retain the underlying failure and recovery information. A fixture
preflight failure is `harness` even when a Chromium/AWS command selected it.
Captured-document failures retain bounded case ID, role and source context even
for timeout/configuration/cleanup variants. Borrowser execution errors include
case ID, role and source. None changes engine ownership or error codes.

Once an output writer accepts bytes, write/flush failure cannot retract them:
the run returns 2 and reports `case_results=publication_failed`. Consumers must
reject output from exit 2. Diagnostic write failure also preserves exit 2 without
panic; closed stderr cannot promise delivery of the terminal record.

## Execution, CI and qualification boundaries

All selected Chromium captures and cleanup finish before any Borrowser parser
worker starts. AG8 retains the AG2 lifecycle and AG4 execution/cleanup machinery.
CSS owns computed style, Layout owns geometry, Paint owns output and Browser owns
production parsing/frame orchestration. No conformance-specific engine branch is
introduced. Both named local suites work with absent/invalid Chromium and AWS
settings; explicitly requested infrastructure failure is never an unsupported
fixture or a skip.

`make conformance` remains all-catalogue; `make ci` adds `conformance-ci`.
The existing GitHub `rust` job runs it after building. The existing native Linux
Chromium job additionally invokes `ag8_real_chromium_suites_are_stable` in fresh
CLI processes for CI and extended selection, checking exact exits and report
bytes. No new job, service or cloud allocation is needed. `test-ag4` is unchanged.

AG7 is completed: [PR 1436](https://github.com/joris97jansen/borrowser/pull/1436)
merged as `4ebae1ebd9b18d12ecbf8a7470f7cfbe58678f1b`, following successful native
Linux qualification [run 39925036784, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39925036784)
of tree `822c20cd0a9ad06eae4511ab5524d773f02e6c0d`. The
[AG7 completion record](https://github.com/joris97jansen/borrowser/issues/1109#issuecomment-6361546562)
is authoritative over older staged pending-language. Native macOS Chromium remains
withdrawn; the historical evidence gaps are not reconstructed or reopened by AG8.
That qualification is baseline evidence, not execution of AG8's new tests.

AG8 excludes new fixtures/WPT coverage, rendering changes, generic suite/backends,
retry/flaky-test machinery, scheduling, expected-difference lists, dashboards,
analytics, automatic issue filing and AWS lifecycle redesign.
