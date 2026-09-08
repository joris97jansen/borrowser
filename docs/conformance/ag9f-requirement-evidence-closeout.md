# AG9f requirement and evidence closeout audit

## Authority and reviewed scope

This is a **non-normative historical audit index**, not a second AG specification.
It records the disposition of authoritative requirements; it does not redefine
them. GitHub requirements, existing AG contracts, frozen report/identity
contracts, and subsystem implementation remain authoritative in their respective
roles. A conflict is resolved in favor of the authoritative source and remains
an audit finding. This index is not a continuously authoritative substitute for
those sources.

The source implementation revision is
`e011d103801a81ababd385b4024875d7b766b1d1`. The reviewed AG9f boundary is that
revision plus the documentation-only Phase A diff. The separate Phase A review
packet identifies the exact diff; a later Phase B packet must identify the
pushed revision and its actual hosted checks. No hosted result for the source
revision validates the AG9f working-tree changes.

Sources are [Milestone AG](https://github.com/joris97jansen/borrowser/milestone/93)
(the milestone requirements supplied for this audit),
[AG9 #1108](https://github.com/joris97jansen/borrowser/issues/1108),
[AG9f #1377](https://github.com/joris97jansen/borrowser/issues/1377), and the
[AG contracts](README.md). The fetched #1108 requirement body has source update
timestamp `2026-06-25T07:09:09Z` and no acceptance comments. #1377 (source update
`2026-09-02T14:38:41Z`) agrees with the supplied AG9f text.
[AG9c #1374](https://github.com/joris97jansen/borrowser/issues/1374) explicitly
requires synthetic comparison tests and forbids fabricated real captures; its
examined comment thread contains no parent-scope acceptance either.
Source-order enumeration yields **19** milestone requirements
and **10** AG9f requirements, correcting the preliminary plan's 20/11 counts;
there are no invented padding requirements.

All `AGF-*` identifiers below are closeout-local references only. They are not
runtime, report, API, conformance, or original contract identities. Suffixes
identify separately assessable parts of one source item. Requirement text is
summarized for identification; follow the original source for its meaning.

**Satisfied** means the bounded obligation has an existing implementation or
document and the linked stable evidence supports it. It is not a test-run or
whole-milestone verdict. **Deliberately deferred** requires an explicit existing
scope/acceptance reference. **Unresolved** includes partial implementation,
missing evidence, and unaccepted deferral. No parent comparison requirement is
marked deliberately deferred. Run-specific commands, failures, platforms, SHAs,
and hosted URLs belong in the review packet, not these stable evidence rows.

## Evidence and implementation owners

References in the matrices resolve through this table. Paths are relative to the
repository root. Tests are evidence for their asserted domain, not proof of all
possible browser behavior.

| Ref | Existing owner and implementation | Authoritative document and concrete evidence |
| --- | --- | --- |
| INV | Tests/tooling: `crates/conformance_test_support/src/` inventory and manifest | [AG2](ag2-fixture-inventory-manifest-contract.md); `crates/conformance_test_support/tests/{inventory,manifest,cli}.rs`; `make check-conformance-manifest` |
| META | Tests/tooling: conformance support classification and assessment | [AG1](ag1-conformance-harness-architecture-no-js-scope.md), [AG3](ag3-expected-results-classification-contract.md); `tests/conformance/expected-results.toml`; `crates/conformance_test_support/tests/{expected_results,assessment_profile}.rs` |
| HTML | HTML/parser canonical observations; runner `src/html_parser.rs` delegates to `html_test_support` | [AG4](ag4-parser-dom-conformance-runners.md); `crates/conformance_runner/tests/repository_parser.rs`, `crates/html/tests/html5_parser_conformance.rs`; `make check-conformance-parser` |
| CSS | CSS semantics and `css_test_support`; runner `src/css_runner.rs` | [AG5](ag5-css-conformance-runners.md); `crates/conformance_runner/tests/repository_css.rs` (seven profiles and policy states); `make check-conformance-css` |
| RENDER | Layout/Paint observations via `rendering_test_support`; runner `src/rendering_runner.rs` | [AG6](ag6-layout-paint-structural-conformance.md), [AG7](ag7-static-structural-reference-comparison.md); `crates/conformance_runner/tests/repository_rendering.rs`; `make check-conformance-rendering` |
| SOURCE | WPT interpretation: `wpt_test_support`; provenance: `external_test_provenance`; rendering adaptation: `rendering_test_support` | [AG8](ag8-wpt-import-filtering-classification.md); `crates/wpt_test_support/tests/repository.rs`, `crates/rendering_test_support/tests/wpt_adapter.rs`, runner `repository_rendering.rs::ag8_derived_wpt_fixture_executes_only_as_a_semantic_paint_relation`; `make check-conformance-wpt` |
| AE | HTML/parser external expected-output workflow, independent of AG discovery | [AE13e](../html5/ae13e-external-fixture-and-snapshot-workflow.md); `tests/wpt/external/allowlist.toml`; `crates/html/tests/html5_external_wpt.rs::pinned_external_records_are_adapter_source_of_truth_and_run_canonically`; `make test-html5-external-fixtures` |
| RUN | Tests/tooling: runner `src/aggregate/{runner,model,projection,accounting}.rs` | [AG9 accounting](ag9-cross-engine-comparison-reporting.md#accounting-populations-and-granularity); `repository_aggregate.rs::aggregate_run_reconciles_the_complete_inventory_and_keeps_populations_distinct`; accounting unit truth tables |
| REPORT | Tests/tooling: runner `src/aggregate/{report,identity}.rs` | [AG9 report grammar](ag9-cross-engine-comparison-reporting.md#aggregate-report-v1-grammar); `repository_aggregate.rs::aggregate_v1_reports_have_exact_golden_bytes`; `tests/data/aggregate-{summary,detail}-v1.txt`; identity unit vectors |
| REG | Runner `src/aggregate/external_registry/`; source-neutral `external_test_provenance/src/capture_v1/` | [AG9 registry](ag9-cross-engine-comparison-reporting.md#cross-engine-comparison-registry-v1); runner `tests/external_registry.rs` (schema, all track invariants, tampering, phase order, multiplicities, empty evidence); provenance unit tests |
| DOM | HTML test-support `src/web_observable_dom/`; independent JS inspector; runner `src/aggregate/advisory/` | [AG9c](ag9c-external-dom-capture.md); Rust codec unit tests, `tools/conformance/web-observable-dom-tree-v1.test.mjs`, runner `tests/advisory_dom.rs`, private `advisory/comparison/tests.rs`; reviewed DOM vectors |
| HISTORY | Runner `src/aggregate/{baseline,trend}/` | [AG9d](ag9d-historical-baseline-trend.md); runner `tests/baseline_trend.rs` (golden identity, verified input, lane-excluded round trip), decoder/comparison unit tests; `tests/contract-vectors/{conformance-baseline-v1,conformance-trend-v1,external-capture-id-v1}/` |
| CLI | Runner `src/bin/cli/`; `tools/conformance/aggregate-workflow.py`; Make targets | [AG9e](ag9-cross-engine-comparison-reporting.md#ag9e-cli-and-publication-contract); runner `tests/{cli,make_workflows}.rs` and CLI unit tests |
| BOUNDARY | Existing Cargo dependency ownership and historical compatibility | Runner `tests/dependency_boundary.rs`; [compatibility table](#historical-compatibility); `make check-conformance-runner-features` |
| CI | Tests/tooling: `Makefile`, `.github/workflows/ci.yml`, `tools/ci/conformance_aggregate_runtime_smoke.py` | Explicit aggregate/adapter tests, summary publication, trace-parser unit tests, Linux isolation job, and `make ci`; exact-revision execution is a separate packet gate |
| DOC | Docs/contracts | [Contributor workflow](../../tests/conformance/README.md#aggregate-reports-and-historical-evidence), [architecture](../architecture/ARCHITECTURE.md), [tracker](../engine-feature-gap-tracker.md); Phase A diff/architecture review |

Runner test paths without a crate prefix in this table are under
`crates/conformance_runner/`. A named evidence ref supplies both owner and
concrete proof paths to each row below.

## Milestone AG requirements

Source: Milestone AG, Requirements, in original order.

| Local ref | Requirement identification | Owner/evidence | Disposition and limit |
| --- | --- | --- | --- |
| AGF-M-R01 | Dedicated harness structure | INV, RUN | Satisfied: federated tooling crates and inventory |
| AGF-M-R02 | Supported no-JS static categories | META, HTML, CSS, RENDER | Satisfied within declared profiles |
| AGF-M-R03 | Unsupported/deferred dynamic categories | META | Satisfied: capability, harness and environment remain distinct; see gap ledger |
| AGF-M-R04 | In-repo discovery | INV | Satisfied |
| AGF-M-R05 | Import/reference WPT-style sources without full execution | SOURCE, AE | Satisfied: bounded sources only |
| AGF-M-R06 | Expected pass/fail, unsupported, skipped, flaky, unclassified metadata | META, RUN | Satisfied; six dimensions assessed separately below |
| AGF-M-R06.a | Expected pass | META | Satisfied: expectation, not assumed observation |
| AGF-M-R06.b | Expected fail | META, RUN | Satisfied: xfail policy does not become semantic pass |
| AGF-M-R06.c | Unsupported | META | Satisfied: explicit capability unavailability |
| AGF-M-R06.d | Skipped | META, RUN | Satisfied: lane exclusion differs from non-runnable |
| AGF-M-R06.e | Flaky | META | Satisfied: stability metadata |
| AGF-M-R06.f | Not yet classified | META | Satisfied: explicit unresolved classification |
| AGF-M-R07 | Deterministic reporting | REPORT, BOUNDARY | Satisfied |
| AGF-M-R08 | Parser output comparison | HTML | Satisfied within canonical parser profiles |
| AGF-M-R09 | DOM tree comparison | HTML | Satisfied: parser-created tree, not DOM API behavior |
| AGF-M-R10 | CSS selector/cascade/computed-style comparison | CSS | Satisfied within seven profiles |
| AGF-M-R11 | Supported layout tree/geometry comparison | RENDER | Satisfied: owner structural bytes |
| AGF-M-R12 | Supported paint operation/semantic comparison | RENDER | Satisfied: no pixels |
| AGF-M-R13 | Renderer-appropriate reftest-style infrastructure | RENDER | Satisfied: exact structural/semantic relations |
| AGF-M-R14 | Selected cross-engine workflow where feasible | DOM, REG, SOURCE, AE | Unresolved: external expectations exist, but parent collection/comparison sufficiency is not established; F1 |
| AGF-M-R15 | CI-safe subset | META, CLI, CI | Satisfied implementation; exact-revision hosted gate pending |
| AGF-M-R16 | Local extended subset | META, CLI | Satisfied: explicit named lane, not scheduler |
| AGF-M-R17 | Adding/classifying/updating tests documentation | DOC, INV, META | Satisfied |
| AGF-M-R18 | Tracker reflects AG completion and remaining gaps | DOC | Unresolved: gaps reconciled; AG completion cannot yet be asserted |
| AGF-M-R19 | No broad WPT/full compatibility claim | DOC, SOURCE | Satisfied |

## Milestone AG exit criteria

| Local ref | Criterion identification | Owner/evidence | Disposition and limit |
| --- | --- | --- | --- |
| AGF-M-E01 | Dedicated harness | INV, RUN | Satisfied |
| AGF-M-E02 | Deterministic discovery/classification/execution/reporting | INV, META, RUN, REPORT | Satisfied in supported static scope |
| AGF-M-E03 | Distinguish runnable/xfail/unsupported/skipped/unclassified | META, RUN | Satisfied: independent dimensions |
| AGF-M-E04 | No-JS scope encoded in metadata | META, SOURCE | Satisfied |
| AGF-M-E05 | Parser/DOM/CSS/layout/paint stable fixtures | HTML, CSS, RENDER | Satisfied within supported observations |
| AGF-M-E06 | Structural/semantic reftest infrastructure | RENDER | Satisfied |
| AGF-M-E07 | Cross-engine workflow documented | DOM, DOC, AE, SOURCE | Unresolved: truthful limitations are not a proven collection workflow; F1 |
| AGF-M-E08 | CI-safe/local-extended separation | META, CLI | Satisfied |
| AGF-M-E09 | Explicit expected failures | META, RUN | Satisfied |
| AGF-M-E10 | Honest unsupported JS/platform classification | META, SOURCE | Satisfied |
| AGF-M-E11 | Tracker completion without overstatement | DOC | Unresolved: parent completion remains unresolved |
| AGF-M-E12 | Future coverage without reinventing harness | INV, SOURCE, BOUNDARY | Satisfied architecture: versioned packages and owner adapters; not proof that arbitrary future coverage exists |

The supplied milestone has twelve exit bullets, not thirteen. This enumeration
preserves those bullets rather than adding a criterion from descriptive prose.
The milestone's descriptive questions map to R04–R17/E02–E12: fixture location,
recognized forms, selection, expectations, comparisons, cross-engine workflow,
history (HISTORY), and future expansion. The cross-engine question remains F1.

## Parent AG9 requirements and exit criteria

Source: #1108 Requirements and Exit Criteria. The description's repeatable
known-browser/external-output comparison goal is assessed in R05/E04 and F1.

| Local ref | Requirement identification | Owner/evidence | Disposition and limit |
| --- | --- | --- | --- |
| AGF-P-R01 | Deterministic report format | REPORT | Satisfied |
| AGF-P-R02 | Headline counts | RUN, REPORT | Satisfied; exact subrows below |
| AGF-P-R02.a | Total | RUN | Satisfied: logical cases |
| AGF-P-R02.b | Pass | RUN | Satisfied: strict selected-variant predicate |
| AGF-P-R02.c | Fail | RUN | Satisfied: semantic failure only |
| AGF-P-R02.d | Expected fail | META, RUN | Satisfied: expectation |
| AGF-P-R02.e | Unsupported | META, RUN | Satisfied: capability only |
| AGF-P-R02.f | Skipped | RUN | Satisfied: excluded runnable case |
| AGF-P-R02.g | Flaky | META, RUN | Satisfied: stability |
| AGF-P-R02.h | Unclassified | META, RUN | Satisfied |
| AGF-P-R03 | Subsystem grouping | RUN, REPORT | Satisfied under AG9 owner/surface/comparison-kind contract, not ten invented owners |
| AGF-P-R03.a | HTML tokenizer | REPORT | Satisfied: surface grouping |
| AGF-P-R03.b | HTML tree construction | REPORT | Satisfied: surface grouping |
| AGF-P-R03.c | DOM snapshot | REPORT | Satisfied: DOM surface |
| AGF-P-R03.d | CSS parsing | REPORT | Satisfied: property/value surface |
| AGF-P-R03.e | Selectors | REPORT | Satisfied: selector surface |
| AGF-P-R03.f | Cascade | REPORT | Satisfied: cascade surface |
| AGF-P-R03.g | Computed style | REPORT | Satisfied: computed-style surface |
| AGF-P-R03.h | Layout | REPORT | Satisfied: owner and geometry surface |
| AGF-P-R03.i | Paint | REPORT | Satisfied: owner and paint surface |
| AGF-P-R03.j | Reftest-style rendering | REPORT, RENDER | Satisfied: comparison-kind grouping, not a subsystem |
| AGF-P-R04 | Optional external expected-behavior fields | REG, HISTORY | Satisfied: separate advisory declarations/evaluation, not authoritative pass/fail fields |
| AGF-P-R05 | External-browser output collection documentation where feasible | DOM, DOC, AE, SOURCE | Unresolved: no admitted capture mechanism or acceptance of deferral; F1 |
| AGF-P-R06 | Baseline notes | REG, HISTORY | Satisfied: explicit typed attachment and optional capture |
| AGF-P-R07 | Practical trend output | HISTORY | Satisfied: explicit two-file comparison |
| AGF-P-R08 | CI-safe summary | REPORT, CLI, CI | Satisfied implementation; hosted execution pending |
| AGF-P-R09 | Local detailed report | REPORT, CLI | Satisfied |
| AGF-P-R10 | Deterministic/reviewable reports | REPORT, HISTORY, BOUNDARY | Satisfied |
| AGF-P-R11 | Advisory/spec/WPT authority documented | DOC, REG | Satisfied: accepted assertions may support expectations; external captures themselves never become authoritative |
| AGF-P-E01 | Deterministic reports produced | REPORT, CLI | Satisfied |
| AGF-P-E02 | Grouped reports | RUN, REPORT | Satisfied |
| AGF-P-E03 | Xfail/unsupported visible | META, RUN, REPORT | Satisfied |
| AGF-P-E04 | Cross-engine comparison workflow documented | DOM, DOC, AE, SOURCE | Unresolved; R05/F1 |
| AGF-P-E05 | CI-safe/local-detail available or separated | CLI | Satisfied implementation |
| AGF-P-E06 | Closeable without full browser automation | DOC, CLI | Satisfied scope/closure constraint: full external-browser automation is explicitly not required, so its absence is not a blocker. AG9 remains not closeable because R05/E04 and dependent requirements are unresolved |

## Normative AG9 contract reconciliation

The rows index the original sections, including nested grammar/table obligations.
They do not reproduce frozen field tables or introduce exceptions. Detailed AG9c
and AG9d sections remain authoritative where the main contract delegates to them.
“Satisfied” below is bounded implementation evidence, not exhaustive formal proof.

| Local ref | Original section / obligation family | Owner/evidence | Disposition |
| --- | --- | --- | --- |
| AGF-C-accounting-01 | [Populations and headline predicates](ag9-cross-engine-comparison-reporting.md#accounting-populations-and-granularity): distinct identities, overlap, strict pass/fail/skipped | RUN; accounting truth-table/overflow/orthogonality tests | Satisfied |
| AGF-C-sealing-01 | [Sealing](ag9-cross-engine-comparison-reporting.md#authoritative-aggregaterun-sealing): primary state, exact source identity, owner evidence, immutable projection | RUN, REPORT; aggregate model/report invariant tests | Satisfied |
| AGF-C-selection-01 | [Eligibility and lanes](ag9-cross-engine-comparison-reporting.md#eligibility-named-lane-selection-and-attempts): EmptyV1, selected/excluded/not-applicable, conservation, no synthetic lane | RUN, CLI, HISTORY; AG9d1 round trip | Satisfied |
| AGF-C-state-01 | [Orthogonal state](ag9-cross-engine-comparison-reporting.md#orthogonal-conformance-state-and-unsupported-accounting): no infrastructure-to-capability relabeling | META, RUN | Satisfied |
| AGF-C-owner-01 | [Owner and comparison kind](ag9-cross-engine-comparison-reporting.md#subsystem-ownership-and-comparison-kind): surface-owner mapping and separate oracle | RUN, REPORT; `aggregate_identity_selection_and_comparison_kind_remain_orthogonal` | Satisfied |
| AGF-C-terminal-01 | [Terminal outcomes](ag9-cross-engine-comparison-reporting.md#aggregate-terminal-outcomes): typed resource/invariant/incomplete projection and two-side precedence | runner `aggregate/projection.rs` unit tests, RENDER | Satisfied; Timeout is reserved zero, not implemented detection |
| AGF-C-dependency-01 | [Dependency direction](ag9-cross-engine-comparison-reporting.md#cross-engine-ownership-and-dependency-direction): generic provenance, runner attachments, CSS-owned failure taxonomy | BOUNDARY; aggregate dependency and trusted-constructor compile probes | Satisfied |
| AGF-C-operation-01 | [Selected-operation boundary](ag9-cross-engine-comparison-reporting.md#ag9c-implementation-and-selected-operation-boundary) and AG9c scope/handoff | DOM; `advisory_dom.rs` operation/selection/discovery tests | Satisfied: partial selected scope only |
| AGF-C-dom-01 | [DOM artifact grammar](ag9-cross-engine-comparison-reporting.md#artifact-grammar): canonical lines, framing and resource limits | DOM, REG; neutral written-contract vectors and malformed-input tests | Satisfied machinery |
| AGF-C-dom-02 | [String encoding](ag9-cross-engine-comparison-reporting.md#string-encoding): scalar validation, escapes, byte ordering | DOM; escaping/UTF-8 vectors | Satisfied machinery |
| AGF-C-dom-03 | [Document/node semantics](ag9-cross-engine-comparison-reporting.md#document-and-node-semantics) | DOM; nodes vectors and rejection tests | Satisfied machinery |
| AGF-C-dom-04 | [Namespaces/names/attributes](ag9-cross-engine-comparison-reporting.md#namespaces-names-and-attributes) | DOM, REG; namespaces-attributes vectors | Satisfied machinery |
| AGF-C-dom-05 | [Templates](ag9-cross-engine-comparison-reporting.md#template-contents) | DOM; templates vectors and shadow/fragment inspector tests | Satisfied machinery |
| AGF-C-dom-06 | [Unsupported states/input assumptions](ag9-cross-engine-comparison-reporting.md#unsupported-states-and-fixture-assumptions) | DOM; public capture rejects unsupported mechanism | Satisfied rejection boundary; admitted collection unresolved in F1 |
| AGF-C-dom-07 | [Projection boundaries](ag9-cross-engine-comparison-reporting.md#projection-boundaries), AG9c differences/failures | DOM; private synthetic comparison tests | Satisfied machinery; no real agreement evidence |
| AGF-C-artifact-01 | [AG9b artifact validation](ag9-cross-engine-comparison-reporting.md#ag9b-external-artifact-validation-boundary) | REG; `capture_v1/artifact.rs` tests | Satisfied |
| AGF-C-read-01 | [Confined reads and byte lifetime](ag9-cross-engine-comparison-reporting.md#confined-read-threat-model-and-verified-byte-ownership) | provenance `confined_file.rs`, REG; tampering/symlink/length tests | Satisfied supported-platform boundary; not proof of capture history |
| AGF-C-provenance-01 | [External provenance](ag9-cross-engine-comparison-reporting.md#external-capture-provenance): required fields and fail-closed omissions | REG; provenance model/identity tests | Satisfied validation |
| AGF-C-capture-id-01 | [Capture ID](ag9-cross-engine-comparison-reporting.md#externalcaptureid-v1): domain, primitive framing, all tags, ordered arguments/set sorting, exclusions, authority | REG, HISTORY; capture identity unit tests and independent preimage vectors; BOUNDARY constructor probes | Satisfied |
| AGF-C-attachment-01 | [Attachment/note identity](ag9-cross-engine-comparison-reporting.md#attachment-and-baseline-note-identity): exact variant and invariant track tuple | REG; `every_advisory_track_invariant_is_enforced`, note/reference tests | Satisfied |
| AGF-C-registry-01 | [Registry V1](ag9-cross-engine-comparison-reporting.md#cross-engine-comparison-registry-v1): fixed roots, closed schema, authoritative parsers and field grammar | REG; schema/path tests | Satisfied |
| AGF-C-registry-02 | [Captures](ag9-cross-engine-comparison-reporting.md#captures): every field, applicability union, canonical collection and invocation rules | REG; field/multiplicity/identity tests | Satisfied validation |
| AGF-C-registry-03 | [Tracks](ag9-cross-engine-comparison-reporting.md#advisory_tracks), [attachments](ag9-cross-engine-comparison-reporting.md#attachments), [notes](ag9-cross-engine-comparison-reporting.md#baseline_notes): keys, uniqueness, references | REG; track invariant, duplicate, reconciliation tests | Satisfied validation |
| AGF-C-sources-01 | [Algorithm/configuration sources](ag9-cross-engine-comparison-reporting.md#capture-algorithm-and-configuration-source-semantics), AG9c exact-byte source checks | DOM; `advisory/sources.rs` and comparison tests | Satisfied: source agreement is not context admission |
| AGF-C-validation-01 | [Registry validation](ag9-cross-engine-comparison-reporting.md#deterministic-registry-validation): phases 1–8, ranking/key tables, allocation precedence, stable diagnostic vocabulary | REG; phase-four/five/six precedence, shuffled declarations, allocation and bound tests | Satisfied |
| AGF-C-authority-01 | [Advisory separation](ag9-cross-engine-comparison-reporting.md#advisory-evidence-authority-separation) | REG, HISTORY; `exact_empty_registry_reconciles_without_changing_aggregate_truth`, advisory drift tests | Satisfied |
| AGF-C-bounds-01 | [Fixed limits](ag9-cross-engine-comparison-reporting.md#fixed-limits-and-engineering-basis): every byte/multiplicity ceiling, checked arithmetic, fallible storage, no truncation, raw-payload qualification | REG, REPORT, DOM, HISTORY; bounds/allocation tests and compile-time historical ceilings | Satisfied bounded contracts, not an RSS guarantee |
| AGF-C-logical-id-01 | [Logical identity](ag9-cross-engine-comparison-reporting.md#aggregate-logical-population-identity-v1): framing, branch tags, exact source reconciliation, opaque constructors | REPORT, BOUNDARY; frozen member digests and field-sensitivity tests | Satisfied |
| AGF-C-logical-id-02 | [Source-set identity](ag9-cross-engine-comparison-reporting.md#borrowser-conformance-logical-case-source-set-v1): ordering/duplicates, empty vector, domain versus membership | REPORT, HISTORY; frozen empty digest and source-set ordering tests | Satisfied |
| AGF-C-report-01 | [Report grammar](ag9-cross-engine-comparison-reporting.md#aggregate-report-v1-grammar): summary/detail field order, closed labels, zero rows, framing, absent states and bounds | REPORT; exact golden bytes, AG3 branch and projection unit tests | Satisfied |
| AGF-C-publication-01 | [Deterministic publication](ag9-cross-engine-comparison-reporting.md#deterministic-reports-and-publication-boundaries): typed projection, stable legacy reports, construct-before-write | REPORT, BOUNDARY, CLI | Satisfied |
| AGF-C-trend-01 | [Trend semantics](ag9-cross-engine-comparison-reporting.md#deterministic-trend-semantics): compatibility axes, four populations, fingerprint authority, no implicit latest/percentage | HISTORY; independent-axis, membership, advisory/note-isolation unit tests | Satisfied |
| AGF-C-history-01 | [AG9d binary primitives and baseline grammar](ag9d-historical-baseline-trend.md#binary-primitives): seven sections, versions, canonical decoding, evaluation grammar, historical sealing | HISTORY; independent decoder vectors, duplicate/dangling identity and framing tests | Satisfied |
| AGF-C-history-02 | [AG9d trend grammar](ag9d-historical-baseline-trend.md#trend-v1-grammar) and fingerprints | HISTORY; exact trend vector, change-mask and scope tests | Satisfied |
| AGF-C-history-03 | [AG9d bounds/file API](ag9d-historical-baseline-trend.md#bounds-and-file-api): two same-object digest-verified reads, ceilings and supported-platform refusal | HISTORY; verified-file and bound tests | Satisfied supported-platform boundary |
| AGF-C-history-04 | [AG9d1 policy projection](ag9d-historical-baseline-trend.md#ag9d1-historical-policy-projection) | HISTORY; `ag9d_lane_excluded_baseline_round_trip` and detail policy tests | Satisfied |
| AGF-C-ci-01 | [External-browser/CI boundary](ag9-cross-engine-comparison-reporting.md#external-browser-and-ci-boundaries): no browser provisioning/network/automation/raster | CLI, CI; actual Linux isolation required separately | Satisfied bounded implementation: existing code/workflow defines the frozen boundary. Revision-specific Linux isolation and exact-revision hosted validation remain pending; AG9f validation/closeout gates are unchanged |
| AGF-C-stage-01 | [Staged decision](ag9-cross-engine-comparison-reporting.md#staged-decision): distinguish stages and unsupported real capture | DOC, DOM | Satisfied status distinction; parent remains unresolved |
| AGF-C-cli-01 | [AG9e CLI](ag9-cross-engine-comparison-reporting.md#ag9e-cli-and-publication-contract): command/flag matrix, explicit lane/descriptors, policy and exit 0–4 | CLI; argument matrix, baseline/trend errors, closed stdout and publication tests | Satisfied |
| AGF-C-cli-02 | [Feature/workflow boundaries](ag9-cross-engine-comparison-reporting.md#feature-and-workflow-boundaries): no-adapter default, aggregate composition, literal argv, clean stdout, OS-native paths | CLI, BOUNDARY; `make_workflows.rs`, non-UTF-8 CLI tests | Satisfied implementation; hosted/isolation gate separate |

## AG9f requirements and exit criteria

| Local ref | Requirement identification | Owner/evidence | Disposition |
| --- | --- | --- | --- |
| AGF-F-R01 | Finalize implementation-status sections | DOC, REPORT, DOM, HISTORY, CLI | Satisfied documentation; no completion inference |
| AGF-F-R02 | Update index | DOC | Satisfied |
| AGF-F-R03 | Contributor workflows | DOC | Satisfied documentation within actual limits; parent collection remains F1 |
| AGF-F-R03.a | Aggregate reports | DOC, CLI | Satisfied |
| AGF-F-R03.b | External collection | DOC, DOM | Satisfied truthful admission/inspection guidance; no operational capture claim |
| AGF-F-R03.c | Provenance review | DOC, REG | Satisfied |
| AGF-F-R03.d | Advisory tracks | DOC, REG | Satisfied |
| AGF-F-R03.e | Baseline notes | DOC, REG | Satisfied |
| AGF-F-R03.f | Trend baselines | DOC, HISTORY | Satisfied |
| AGF-F-R04 | Architecture documentation | DOC, BOUNDARY | Satisfied |
| AGF-F-R05 | Feature-gap tracker | DOC | Satisfied reconciliation; no blanket completion |
| AGF-F-R06 | Mark completed capabilities | DOC, REPORT, REG, DOM, HISTORY, CLI | Satisfied machinery only |
| AGF-F-R07 | Explicit minimum remaining gaps | DOC | Satisfied; exhaustive ledger below retains more |
| AGF-F-R07.a | Raster/screenshots | RENDER, DOC | Satisfied documentation of absence |
| AGF-F-R07.b | Automation | DOM, DOC | Satisfied documentation of absence |
| AGF-F-R07.c | Broad WPT | SOURCE, DOC | Satisfied documentation of absence |
| AGF-F-R07.d | Dynamic/JavaScript | META, DOC | Satisfied documentation of absence |
| AGF-F-R07.e | Broader external surfaces | DOM, DOC | Satisfied documentation of absence |
| AGF-F-R08 | Existing report compatibility | BOUNDARY | Satisfied stable tests; execution results in packet |
| AGF-F-R09 | Full AG9 validation and full CI | CI, DOM, BOUNDARY | Unresolved until all required execution evidence, including exact-revision hosted CI, exists |
| AGF-F-R10 | Final review packet | DOC | Unresolved final packet: Phase A packet prepared separately; Phase B pending |
| AGF-F-E01 | Parent requirements demonstrably satisfied | all parent rows | Unresolved: F1 and validation gates |
| AGF-F-E02 | Tracker reflects AG completion honestly | DOC | Unresolved: accurate tracker cannot assert unsupported completion |
| AGF-F-E03 | External evidence explicitly advisory | REG, HISTORY, DOC | Satisfied |
| AGF-F-E04 | CI/local workflows documented and tested | CLI, CI, DOC | Unresolved final evidence: exact-revision hosted/isolation gate pending |
| AGF-F-E05 | Parent can close | parent rows | Unresolved |

## Historical compatibility

All paths below are within `crates/conformance_runner/`. No expected bytes are
regenerated for this closeout.

| Surface | Stable proof |
| --- | --- |
| Parser report V1 | `src/report.rs::parser_report_v1_has_an_exact_byte_contract`, `tests/data/parser-report-v1-compat.txt` |
| CSS report V1 | `src/css_report.rs::css_report_v1_has_an_exact_byte_contract_and_omits_singleton_variant_identity`, inline literal bytes |
| Rendering report V1 | `src/rendering_report.rs::report_v1_remains_snapshot_only_while_default_report_is_v2`, `tests/data/rendering-report-v1-compat.txt` |
| Rendering report V2 | Same test, `tests/data/rendering-report-v2.txt` |
| Direct CLI modes/check ordering | `tests/cli.rs::legacy_direct_modes_and_check_order_are_unchanged` across no-adapter and individual adapter feature builds |
| Direct diagnostics | `tests/cli.rs::legacy_argument_rejections_keep_exact_diagnostic` |
| Historical not-attempted rejection | `parser_report_v1_rejects_named_lane_only_not_attempted_state`, `css_report_v1_rejects_named_lane_only_not_attempted_state`, `historical_rendering_reports_reject_named_lane_only_not_attempted_state` |

CLI comparisons against current builders complement the frozen-byte tests; they
do not independently freeze the builder output. Synthetic protocol vectors are
not collected external-browser results.

## Existing-gap ledger

Sources: [tracker, conformance status](../engine-feature-gap-tracker.md), AG1
scope/eligibility, AG4–AG8 limitations, AG9 staged boundaries, AG9c admission,
AG9d remaining scope, and AE13e capability classification. “Remains” is an
unresolved capability, not acceptance of deferring a required parent obligation.
Explicit milestone non-goals authorize excluding broad execution from AG scope;
they do not authorize deferring the selected comparison requirement.

| Local ref | Existing gap | Owner/evidence | Disposition |
| --- | --- | --- | --- |
| AGF-G-tracker-01 | Aggregate execution/accounting/reports | RUN, REPORT | Satisfied by Stage 1/AG9a |
| AGF-G-tracker-02 | Aggregate scheduling | CLI, CI | Partially satisfied: AG9e explicit lanes/publication; no general aggregate scheduler; parser/performance cron jobs are not that scheduler |
| AGF-G-tracker-03 | Browser/runtime adapters | Browser/runtime, RENDER | Unresolved; controlled-page inventory has zero execution variants |
| AGF-G-tracker-04 | Runtime retention/invalidation/epochs/reuse/work planning/repaint observation | Browser/runtime, RENDER | Unresolved; static capture does not observe these |
| AGF-G-tracker-05 | Broader pinned source adapters | SOURCE, AE | Partially satisfied: exact AG8 and AE13e proofs only |
| AGF-G-tracker-06 | Real capture admission | DOM, REG | Unresolved; F1 |
| AGF-G-tracker-07 | Pixel/raster reference infrastructure | Paint/GFX, RENDER | Unresolved capability; explicitly outside AG9, not silently implemented |
| AGF-G-tracker-08 | JS, DOM APIs, events, timers/microtasks, CSSOM, mutation | production owners, META | Deliberately deferred from AG static scope by Milestone AG description and AG1 unavailable categories; remains absent |
| AGF-G-tracker-09 | WebDriver/interaction/navigation/history/storage/cookies/platform APIs | Browser/runtime, META | Deliberately deferred from AG static scope by Milestone AG description and AG1 unavailable categories; remains absent |
| AGF-G-ag4-01 | Repeated-body parser limitation | HTML, META | Unresolved capability retained in expected-results metadata; do not repair in AG9f |
| AGF-G-ag4-02 | AE parser observation byte-payload accounting | HTML test support, AG4 tracker paragraph | Unresolved separate AE follow-up; AG transport bounds do not settle AE accounting |
| AGF-G-ag5-01 | Contextual fragment execution | HTML/CSS, CSS | Unresolved: representable request, no canonical contextual parser |
| AGF-G-ag5-02 | Broad CSS/property/selector coverage and media/custom-property/animation/platform behavior | CSS, AG5 scope | Unresolved broader conformance; profile support is not comprehensive CSS support |
| AGF-G-ag6-01 | Reference document execution | RENDER | Satisfied for AG7 structural/semantic paired profiles only |
| AGF-G-ag7-01 | WPT relation-link interpretation | SOURCE | Partially satisfied by bounded AG8 graphs; no generalized WPT reftest runner |
| AGF-G-ag7-02 | Viewport height, DPR and platform-font conformance | Layout/Paint, RENDER | Unresolved; available width and synthetic text metrics do not establish them |
| AGF-G-ag7-03 | External resources/network execution | Browser/runtime, RENDER, SOURCE | Partially satisfied provenance/closure bookkeeping; no generalized loading/crawling/server execution |
| AGF-G-ag8-01 | Broad WPT/full manifest/full checkout execution | SOURCE | Deliberately deferred by Milestone AG's explicit no-full-WPT scope; bounded seven-source proof remains |
| AGF-G-ag8-02 | Manual/visual/crashtest/print-reftest/parser .dat forms in AG8 | SOURCE, AE | Partially satisfied only by AE-owned .dat path; other AG8 forms remain unresolved |
| AGF-G-ag8-03 | Fuzzy raster semantics/reftest-wait | SOURCE, RENDER | Unresolved: represented metadata is non-executable |
| AGF-G-ag8-04 | WPT server substitution, pipes/headers, special origins, live resources | SOURCE, Browser/runtime | Unresolved; positively classified blockers remain in accounting |
| AGF-G-ag9-01 | External advisory fields/notes/identity validation | REG | Satisfied machinery, empty admitted population |
| AGF-G-ag9-02 | CLI/CI publication formerly excluded by AG9b/d | CLI | Satisfied by AG9e; historical exclusions are stage-local |
| AGF-G-ag9-03 | Historical baseline/trend output | HISTORY | Satisfied by AG9d/d1/e within frozen two-input domain |
| AGF-G-ag9-04 | Broader cross-engine observable surfaces | DOM, REG | Unresolved: only singleton comparable DOM machinery |
| AGF-G-ag9-05 | Complete advisory evaluator/selected-operation merger | DOM, HISTORY | Unresolved: selected-only evaluation is not all-declared coverage |
| AGF-G-ag9-06 | Automatic history discovery/latest/storage/dashboard/percentage/regression gating | HISTORY | Unresolved broader infrastructure; explicit files and passive trends do not supply it |
| AGF-G-ag9-07 | Actual timeout detection | RUN | Unresolved: reserved zero category only |
| AGF-G-ae-01 | External expectation representation: namespaced prefixes/templates/multiline values | AE | Unresolved adapter limits documented in AE13e; DOM codec capability does not rewrite AE expectations |
| AGF-G-ae-02 | Broader html5lib/parser/full HTML compatibility | HTML, AE | Unresolved; bounded canonical and upstream proofs do not establish broad conformance |

## Parent comparison finding and follow-up draft F1

The source revision has genuine external expected-output evidence:

- AE13e pins WPT `.dat` source/file/record hashes, licence and attribution,
  adapts one eligible script-off full-document case, and runs the canonical
  parser against upstream expectations. Its two scripting-required records
  remain explicitly unsupported. This workflow is independent of AG discovery.
- AG8 accounts for all seven upstream assertions as not selected for direct
  execution and separately executes one exact-copy Paint-semantic adaptation.
  Its lineage is real; its result is not an upstream raster pass.
- AG9b/c supplies validated advisory machinery, but
  `tests/conformance/external/cross-engine-comparisons.toml` has zero captures,
  tracks, attachments, and notes. `captureWebObservableDomTreeV1()` reports
  unsupported mechanism, and public `SelectedDomOperationRun::compare_external`
  rejects unproven capture context. Only private tests admit synthetic context.

These facts support bounded external-assertion comparison, not a demonstrated
external-browser collection workflow. #1108 permits external expected outputs
and does not require full automation; nevertheless no acceptance record in the
examined sources establishes that the AE/AG8 workflows discharge its external-browser collection
requirement and repeatable known-browser comparison goal. The audit therefore
leaves AGF-P-R05/E04, AGF-M-R14/E07 and dependent closeout criteria unresolved.
It neither declares automation necessary nor treats capture absence as accepted
deferral. A documented, accepted sufficiency decision could resolve the scope
question without implementing new browser automation.

**Draft only; no issue creation or implementation belongs to AG9f.**

- **Milestone:** existing AG if needed to fulfill the current commitment. A new
  “Controlled external-browser capture and advisory comparison” milestone is
  appropriate only after explicit acceptance of deferring that commitment.
- **Title:** AG follow-up — Establish an admitted static DOM external-browser
  comparison workflow.
- **Owner:** Tests/tooling for evidence orchestration, source-neutral provenance
  owner for identity, HTML test-support for comparable DOM. Production parser
  semantics remain HTML-owned.
- **Description:** Resolve #1108's selected comparison/collection obligation
  through a repeatable bounded workflow for existing fixtures. Establish the
  frozen AG9c input context and provenance before admitting real observations;
  preserve advisory isolation. Full external-browser automation is not required.
- **Bounded acceptance criteria:** identify selected existing fixtures and exact
  input bytes; prove delivery/MIME/UTF-8, scripting-disabled pre-parse state,
  parser completion, non-mutation and controlled resources; retain reproducible
  engine/tool/configuration provenance; demonstrate repeatable real comparisons;
  test deterministic admission/rejection; document collection/review; preserve
  existing report/policy/identity contracts. No new surfaces, raster, broad WPT,
  or dynamic behavior are implied.
- **Closure impact:** AG9 remains unresolved; AG9f cannot satisfy parent-closeable
  criteria; Milestone AG's selected cross-engine requirement remains unresolved.
  Existing unrelated capability gaps above are retained, not newly discovered
  semantic defects or additional implementation work for this closeout.

## Provisional closure and evidence phases

Phase A performs audit, documentation, local validation and review, then stops
before committing/pushing. Actual commands/results and any local limitations
are in its separate packet. Exact-revision hosted CI remains **pending**.
Linux isolation requires actual Linux, noninteractive privilege, namespaces and
tracing; Node tests and trace-parser tests are separate evidence, not substitutes.

Phase B belongs to a later reviewed/committed/pushed revision: identify its SHA,
inspect actual applicable hosted runs/jobs (including tested merge SHA where
applicable), record attempts/events/platforms/conclusions and missing/skipped
checks, and finalize that packet's hosted evidence. Earlier AG9e CI is not proof
for AG9f. This historical matrix does not promise a Phase B pass.

- **AG9f:** not closeable yet; parent satisfaction and final validation/packet
  gates remain unresolved.
- **AG9 #1108:** not closeable yet; F1 is unresolved, independently of automation.
- **Milestone AG:** not closeable yet; selected comparison and dependent tracker
  completion criteria remain unresolved. Infrastructure and honest classification
  do not establish broad compatibility.
