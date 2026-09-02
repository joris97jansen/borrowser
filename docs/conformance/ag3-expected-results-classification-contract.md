# AG3 expected-result and classification contract

## Status and ownership

This document is the normative AG3 contract for logical-test classification
and expected-result metadata. AG1 remains authoritative for the federated
conformance architecture and orthogonal state model. AG2 remains authoritative
for fixture discovery, stable `TestId`, `InventoryScope`, `ObservationSurface`,
versioned execution-package declarations, and Manifest V2.

The test/tooling-only `conformance-test-support` crate owns parsing,
validation, reconciliation, sealed metadata, and repository-stable summary
output. No production HTML, CSS, Layout, Paint/GFX, or Browser/runtime behavior
reads AG3 metadata. Subsystems retain their existing semantic and observation
ownership.

AG3 does not make the GitHub issue's labels into a flat status enum. It
represents them through independent facts:

| Issue wording | AG3 representation |
| --- | --- |
| expected pass | `Expectation::ExpectedPass` |
| expected fail | typed expected-failure classification plus reason |
| unsupported | unavailable engine/platform capability with exact typed capability, optional typed feature, and reason |
| skipped | exclusion relative to one named lane-policy declaration, with reason |
| flaky | stability metadata with reason |
| not yet classified | explicit classification-completeness record with reason |

These facts can coexist where meaningful. They do not represent an execution
attempt, observed result, XFAIL/XPASS evaluation, or normalized run result.

## Separate versioned registry

`tests/conformance/expected-results.toml` is a human-authored registry with the
strict envelope:

```toml
format = "borrowser-conformance-expected-results-v1"
granularity = "logical-test"
```

It is keyed one-to-one by AG2's stable logical `TestId`. Classification truth
is deliberately separate from fixture/inventory truth. AG3 adds no field to
either AG fixture descriptor version or `borrowser-conformance-manifest-v2`;
all AG2 schemas default-deny AG3 fields.

The public loader accepts a repository root and validated AG2 inventory; it
always resolves `tests/conformance/expected-results.toml`. Path-taking is an
internal implementation detail, not a configurable registry API. AG3 has no
alternate registry, overlay, environment-specific registry, or search path.
Registry-level diagnostics use the fixed repository-relative registry identity
rather than a host-dependent absolute path.

The public Rust boundary includes `load_expected_results`,
`serialize_expected_results_summary`, the opaque `ValidatedExpectedResults`,
immutable lossless AG4 consumer views, the canonical contextual eligibility
evaluator, and the displayable `ExpectedResultsErrors` failure type. It has no
public constructors, serde/schema values, or mutable records, and the error
exposes no typed diagnostic internals. Only the closed vocabulary and borrowed
reason-bearing views needed for execution are public; reference records,
scalar identifier constructors, diagnostic detail, and format/path constants
remain crate-private.

The registry is bounded to 4 MiB, must be UTF-8, must be a regular file inside
the repository, and may not be reached through a symlinked path. Every TOML
table denies unknown fields. Unknown values are rejected rather than accepted
as future extensions. Schema evolution requires a new explicit format version.

## V1 record shapes

An explicitly unclassified logical test records only why classification is not
yet honest and optional Borrowser references:

```toml
[[tests]]
id = "layout-example"
classification = "not-yet-classified"
reason = "The comparison contract and rendering requirements are not established."
references = [
  { kind = "documentation", path = "docs/rendering/w8-box-generation-formatting-debug-surfaces.md" },
]
```

Omission is not unclassified metadata. Every discovered AG2 ID must have a
record. A `not-yet-classified` record forbids classified dimensions so partial
or implied classification cannot leak through.

A classified logical test declares each authoritative dimension explicitly:

```toml
[[tests]]
id = "layout-example"
classification = "classified"
requirements = ["no-js", "requires-layout-feature"]
lane_exclusions = [
  { policy = "normal-ci", reason = "Excluded by a declared policy until its stability history is established." },
]
references = [
  { kind = "documentation", path = "docs/rendering/x3-width-height-resolution-supported-subset.md" },
  { kind = "tracking-issue", issue = 1234 },
]

[tests.engine]
availability = "unavailable"
missing = [
  { kind = "layout-feature", feature = "css-grid", reason = "Grid layout is not implemented." },
]

[tests.harness]
readiness = "not-ready"
limitations = [
  { kind = "missing-subsystem-adapter", reason = "AG has no Layout delegation adapter yet." },
]

[tests.environment]
requirements = [
  { kind = "viewport-configuration", profile = "mobile-320", reason = "The expected geometry assumes this viewport profile." },
]

[tests.expectation]
kind = "expected-fail"
reason = "The supported comparison currently exposes a known semantic mismatch."
failure = { kind = "semantic-mismatch" }

[tests.stability]
state = "flaky"
reason = "Repeated synthetic executions demonstrate unstable observations."
```

The example shows representability, not a declaration about a repository seed.
Real fixture metadata must be evidence-backed. In particular, fixtures are not
marked flaky just to exercise the format.

## Typed dimensions

### Classification completeness

V1 has `classified` and `not-yet-classified`. A classified record requires all
classified dimensions, including explicit empty lists. A not-yet-classified
record requires a non-empty reason and forbids those dimensions. This makes the
population complete without inventing facts.

### Engine/platform capability availability

Availability is `available`, `unavailable`, or `not-yet-established`.
`unavailable` requires one or more exact missing capabilities and a reason for
each. V1 capability kinds are JavaScript execution, DOM API, networking, HTML
parser feature, CSS feature, Layout feature, Paint feature, font feature,
Browser/runtime feature, and user interaction. Capability kinds whose scope is
not already exact require a lowercase typed `CapabilityFeatureId`.
`CapabilityFeatureId` is a semantic capability-feature identity, not a generic
machine or host identifier.

A missing capability must correspond to a declared requirement tag. A missing
adapter, source-format handler, expected observation, expectation
representation, observation or comparison surface, or environment-description/
provisioning facility is a harness limitation and never an unsupported engine
feature.

`available` asserts only an existing production capability path supported by
authoritative evidence. It does not assert conformance, expected success,
harness readiness, stability, or runnable status. Fixture presence,
observation category, path, payload contents, or expected-pass metadata are not
evidence of capability availability.

### Harness readiness

Readiness is `ready`, `not-ready`, or `not-yet-established`. `not-ready`
requires one or more typed, reason-bearing limitations. V1 distinguishes a
missing subsystem adapter, unsupported source format, missing expected
observation, unsupported expectation representation, missing observation
surface, missing comparison surface, missing environment description, and
missing environment provisioning.

`missing-expected-observation` means that no authoritative expected semantic
value or artifact has been authored for the logical test.
`unsupported-expectation-representation` means that an authoritative
expectation does exist, but AG's current schema or adapter representation cannot
encode it truthfully without loss or semantic invention. These are independent,
separately counted harness states; neither is an engine capability gap.

Readiness describes AG infrastructure only. It neither adds nor removes a
production browser capability.

### Environment requirements

`[tests.environment]` declares what a logical test needs; it never records
whether a current host, CI worker, lane, browser installation, or runtime can
satisfy that need. V1 contains only requirement kinds justified by AG1's
static-renderer and later comparison boundary:

- controlled font set;
- viewport configuration;
- device scale;
- platform configuration;
- controlled resources;
- external browser;
- pixel-capture environment; and
- user-interaction environment.

Each declaration has a controlled lowercase `EnvironmentProfileId` and
explicit reason. `EnvironmentProfileId` is a distinct domain type even though
V1 gives it the same lexical grammar as `CapabilityFeatureId`; the two values
cannot be interchanged in Rust. Neither type represents a host or machine
identity. Duplicate kind/profile pairs are rejected. An empty list truthfully
states that this logical test has no special execution-environment requirement
beyond a future execution request's base contract. AG3 does not inspect or
provision an environment.

### Expectation and expected failure

Expectation is `expected-pass` or `expected-fail`. Expected failure always has
both a machine-readable `semantic-mismatch` classification and a non-empty
human reason. V1 deliberately does not serialize an observation inside the
failure. The reconciled AG2 fixture's single `ObservationSurface` is the sole
observation authority. A future independently meaningful failure sub-surface
requires a deliberate schema/API version rather than duplicated V1 data.

Expectations are not outcomes. AG3 has no attempt, semantic pass/fail,
execution error, timeout, XFAIL, or XPASS value.

### Stability

Stability is `stable`, `flaky` with a reason, or `not-yet-established`.
Flakiness is metadata about repeated-execution reliability, not an observed
outcome and not a reason to convert a semantic failure into a pass.

### Requirements and inventory scope

Requirement tags are closed Rust values: `no-js`, `requires-js`,
`requires-dom-api`, `requires-networking`,
`requires-html-parser-feature`, `requires-css-feature`,
`requires-layout-feature`, `requires-paint-feature`,
`requires-font-feature`, `requires-browser-runtime-feature`,
`requires-pixel-comparison`, and `requires-user-interaction`. Duplicate tags
and `no-js` combined with `requires-js` are invalid.

These tags describe capabilities a logical case requires. They do not replace
or refine AG2's authoritative `InventoryScope::StaticHtmlCssNoJs`, and scope is
never inferred from either dimension.

### Lane exclusions

An exclusion is relative to one controlled policy declaration:
`normal-ci`, `local-extended`, `scheduled-extended`, or `manual-extended`.
Every exclusion has a reason. There is no global skipped state. These names are
inert metadata; AG3 does not define lane selection, lane environments, or CI
execution. A lane name is never an environment assessment.

### Primary subsystem ownership

V1 serializes no owner. The canonical primary owner is derived from the
reconciled AG2 observation:

| AG2 observation | Primary owner |
| --- | --- |
| tokenizer, tree construction, DOM tree | HTML/parser |
| CSS parsing, selectors, cascade, computed style | CSS |
| Layout geometry | Layout |
| Paint operations | Paint |
| Browser/runtime semantic | Browser/runtime |

This is the AG1 ownership table, not contributor assignment. AG3 introduces no
second subsystem or user-name tag vocabulary because V1 has no independently
useful ownership fact that would justify one.

### Borrowser references

References are optional, sorted metadata. A documentation reference must be a
lowercase portable `docs/.../*.md` repository-relative path to an existing
regular file and may not traverse or use symlinks. A tracking issue is a
positive integer. These links explain classification evidence; they do not
alter semantics.

## Validation and reconciliation

The pipeline is:

```text
strict bounded V1 parse
  -> typed field and cross-field validation
  -> complete reconciliation with validated AG2 inventory
  -> sealed ValidatedExpectedResults sorted by TestId
  -> deterministic repository metadata summary
```

Reconciliation uses the complete validated AG2 inventory, not the checked-in
manifest as a second source. Duplicate registry IDs, registry IDs absent from
inventory, and discovered IDs lacking records are errors. Invalid declared
records still count as declarations for reconciliation so diagnostics do not
add a misleading omission error. Diagnostics sort by logical subject, typed
kind rank, and an exhaustive per-variant semantic detail key. The ordering key
uses only explicit typed fields and fixed variant order; Rust `Debug` output and
TOML declaration order are not ordering contracts.

## Contextual execution eligibility

AG3 persists neither execution eligibility nor environment availability. The
conceptual boundary is:

```text
logical-test metadata
  -> later execution request/environment assessment
  -> derived execution eligibility
  -> later execution attempt
```

The crate contains a narrow pure evaluation model implementing AG1's
semantics. AG4 exposes it through immutable typed views and one public
`evaluate_execution_eligibility` function shaped by the first real consumer;
it still does not freeze a general lane or provisioning API. It combines engine
availability, harness readiness, and a synthetic typed assessment of each
declared environment requirement. All known blockers coexist and sort
deterministically. Unknown prerequisites remain explicitly unresolved. One or
more known blockers establish `NotRunnable` even when other prerequisites are
unresolved; without blockers, unresolved prerequisites establish
`NotYetEstablished`; only no blockers and no unresolved prerequisites establish
`Runnable`.

`ValidatedExpectedResults::get(TestId)` and deterministic `iter()` return
borrowed views. Those views preserve `available`, `unavailable` with every
typed feature/reason, `not-yet-established`, `ready`, `not-ready` with every
limitation/reason, classification reason, expected-failure reason, flaky
reason, environment reason, and lane-exclusion reason. They expose no serde
schema, constructors, or mutable records. AG4 supplies only the empty parser
environment assessment because current parser cases declare no special
environment requirements. Generic named-lane selection and broad environment
assessment remain later AG work.

AG4 evaluates these metadata dimensions and canonical eligibility before it
requires execution infrastructure. Not-yet-classified and harness-not-ready or
harness-readiness-not-yet-established cases therefore remain honestly
reportable without an executable subsystem package. A `Ready` harness claim is
an affirmative infrastructure assertion: AG4 still loads and reconciles its
declared package even when engine capability makes the case non-runnable. Only
a runnable case with a successfully reconciled ready package is evaluated.

## Deterministic summary and contributor workflow

`conformance-expected-results --check` validates the AG2 inventory, validates
and reconciles the registry, and writes an ephemeral LF-terminated UTF-8
summary to standard output. There is no checked-in generated AG3 summary and no
`--update` mode. The human-authored registry is the only classification truth.

The summary format is
`borrowser-conformance-expected-results-summary-v1` at `logical-test`
granularity. It has fixed section and field order and reports only repository-
stable facts: classification completeness, engine capability availability,
harness readiness, expectation and failure class, stability, lane declarations,
missing capabilities, harness limitations, environment requirement kinds and
profiles, requirement tags, and derived primary owners. Records and keyed
details sort bytewise by closed typed values and `EnvironmentProfileId`.

It contains no timestamps, durations, host paths, filesystem ordering, locale
formatting, machine state, environment availability, or runnable counts. Exact-
byte tests fix encoding, category order, blank lines, and final newline.

When adding or changing a fixture:

1. update AG2 fixture inventory and its generated manifest through the AG2
   workflow;
2. add or update the same logical ID in
   `tests/conformance/expected-results.toml`;
3. use `not-yet-classified` rather than omitting the ID or inventing facts;
4. cite authoritative contracts/evidence for strong capability assertions;
5. run `make check-conformance-manifest` and
   `make check-conformance-expected-results`;
6. inspect the summary and run the `conformance-test-support` tests.

## Seed evidence and harness-readiness audit

AG3 originally reviewed 11 AG2 seeds without execution or host inspection. The
following table records that historical pre-AG4 readiness audit:

| Seed | Future subsystem-owned observation | Authoritative expected semantic value at AG3 completion | Would an adapter alone permit truthful pass/fail? | Final AG3 state |
| --- | --- | --- | --- | --- |
| `html-tokenizer-basic-document` | HTML-owned canonical tokenizer observation through `html::conformance` | none; the AG2 bundle contains input only and no expected token stream | no; AG also lacks an authored expected observation and comparison path | classified because AE13/AE14 establish the narrow production capability; harness not ready for missing adapter, missing expected observation, and missing comparison surface |
| `html-tree-construction-basic-document` | HTML-owned canonical tree-construction observation | none; no expected tree-construction observation is authored | no, for the same two additional harness gaps | classified because AE13/AE14 establish the narrow production capability; the same three harness limitations apply |
| `dom-tree-basic-document` | HTML-owned canonical parser-created DOM observation | none; no expected DOM snapshot is authored | no, for the same two additional harness gaps | classified because AE2/AE14 establish the narrow production capability; the same three harness limitations apply |
| `css-parsing-basic-stylesheet` | CSS-owned deterministic stylesheet parse observation from the self-contained CSS source | none; no typed or snapshot parse expectation is authored | no; AG lacks an authored expected observation and comparison infrastructure as well as delegation | classified because N8 and the syntax-parser contract establish the narrow self-contained production parse path; all three harness limitations apply |
| `css-selectors-basic-stylesheet` | not uniquely established: selector parsing and selector matching are different CSS-owned observations | none; the bundle also has no DOM or matching context | no; an adapter would have to invent the requested observation and matching inputs | explicitly not yet classified despite AF2/AF10 proving underlying selector capabilities |
| `css-cascade-basic-author-rule` | CSS-owned cascade winner for a specified target and candidate context | none; no target element, matching result, competing candidate context, or expected winner is authored | no; an adapter would have to invent cascade inputs and an expected value | explicitly not yet classified despite AF10 proving underlying cascade capability |
| `computed-style-basic-author-rule` | CSS-owned computed style for a specified element within a document/inheritance context | none; no target DOM element, parent/inheritance context, or expected style is authored | no; an adapter would have to invent document inputs and an expected value | explicitly not yet classified despite AF10/S9 proving underlying computed-style capability |

At AG3 completion, the remaining four records were explicitly not yet
classified:

| Seed | Reason |
| --- | --- |
| `layout-geometry-basic-block-flow` | W8/X3 establish Layout capabilities, but this fixture's AG expectation and truthful viewport/text profile are unspecified |
| `paint-operations-basic-background` | AA8/AA9 establish Paint regression surfaces, but this fixture's expected paint observation and text/rendering requirements are unspecified |
| `paint-semantic-reference-basic` | the AG semantic-comparison contract and rendering requirements are unspecified |
| `browser-controlled-static-page-basic` | V5/AC10 establish runtime orchestration, but this fixture's AG observation and viewport/text/resource requirements are unspecified |

### AG4 parser readiness transition

AG4 expands the inventory to 15 logical tests. The three original parser
records and the new tokenizer, tree-recovery, and representative-DOM records
are now `harness.readiness = "ready"` only because each has a strict V2 package,
canonical AE loading, a matching adapter/profile, an independently reviewed
expected observation, and an implemented comparison surface. Their engine
capability is available and their current environment requirements are empty,
so the canonical evaluator marks them runnable.

`html-tree-construction-repeated-body-unavailable` also has a ready harness and
standards-derived expectation, but declares the
`merge-attributes-into-existing-body-element` `html-parser-feature` gap. The
expected final tree requires the existing body to retain `a=one` and acquire
the missing `b=three` and `c=four` attributes. The case's exact parse errors,
document mode, and final tree do not observe the separate repeated-body
`frameset_ok = false` transition, so that production gap remains in AE tracking
without becoming a blocker for this logical AG case. The case is therefore not
runnable, not attempted, and has no observed parser outcome. It is not an
XFAIL. AG4 has no repository XFAIL seed because the reviewed known parser gaps
are capability gaps rather than honest runnable semantic mismatches; typed
orchestration tests cover XFAIL/XPASS policy normalization.

The initial four available assertions were narrow production-path assertions,
not broad conformance claims. AG4's six available parser records remain equally
narrow even though their package, adapter, expectation, and comparison evidence
now permits execution. At the AG4 checkpoint, CSS execution had not yet received
the package, adapter, and comparison work recorded by the next transition.

### AG5 CSS readiness transition

AG5 expands the current inventory to 18 logical tests. The following seven CSS
records are now classified and `harness.readiness = "ready"` in
`tests/conformance/expected-results.toml`:

- `css-parsing-basic-stylesheet`;
- `css-selector-parsing-basic-list`;
- `css-selector-specificity-list`;
- `css-selector-matching-parser-dom`;
- `css-cascade-basic-author-rule`;
- `css-inheritance-wide-keywords`;
- `computed-style-basic-author-rule`.

Each readiness assertion is backed by a strict AG2 V2 outer package, a strict
`borrowser-css-fixture-v1` nested package, the CSS subsystem adapter, an
explicit production execution profile, an independently reviewed expected
observation, an implemented semantic comparison surface, and typed AG3
capability, readiness, and expectation state. These records exercise seven
focused production paths; they do not establish broader selector, property,
CSS, WPT, rendering, or browser compatibility.

The historical `css-selectors-basic-stylesheet` seed was underspecified between
selector parsing and selector matching and supplied a stylesheet rather than a
direct selector-list input. AG5 retired it and introduced
`css-selector-parsing-basic-list` as a new stable logical selector-parsing case.
Retaining the former `TestId` would have changed both its authored carrier and
its observation meaning, so the identity change is deliberate rather than an
implicit seed repurposing.

The current manifest and expected-results registry leave exactly these four
records `not-yet-classified`:

| Seed | Remaining current gap |
| --- | --- |
| `layout-geometry-basic-block-flow` | no Layout execution adapter or fully specified AG expectation and viewport/text profile |
| `paint-operations-basic-background` | no Paint/GFX execution adapter or fully specified paint observation and rendering requirements |
| `paint-semantic-reference-basic` | no semantic Paint comparison adapter or fully specified rendering requirements |
| `browser-controlled-static-page-basic` | no Browser/runtime execution adapter or fully specified observation and viewport/text/resource requirements |

### AG8 source-record accounting interoperability

AG8 expands the inventory to 25 logical tests. The migrated
`paint-semantic-reference-basic` and the new equivalent-layout,
equivalent-cascade, and intentional-mismatch records are classified, stable,
harness-ready, and expected-pass. Their V3 outer reference relation is generic
inventory truth; AG3 expectation metadata does not duplicate or override it.

AG3 remains the canonical V1 classification contract at logical `TestId`
granularity. AG8 reuses the unchanged closed `RequirementTag`,
`EngineCapabilityKind`, `HarnessLimitationKind`, and
`EnvironmentRequirementKind` vocabulary for source-neutral requirements and
assessments, but source-record accounting is a different population and wire
format. It does not replace or infer AG3 records. The AG3 V1 serialized names,
validation, and summary remain byte-compatible; the one derived V4 fixture has
an ordinary explicit AG3 logical-test record.

`layout-reference-grid-unavailable` has a valid ready paired package but records
the genuine `css-grid` Layout capability gap. It is not runnable, is not
attempted, and projects to `not-run`; it is not an observed comparator
unsupported result and not an XFAIL.

The permanent corpus contains no artificial AG7 expected failure. XFAIL and
XPASS projection is exercised with temporary test repositories. Those IDs do
not enter the manifest, registry, or repository conformance totals. For paired
cases, only a successfully executed but unsatisfied relation is a semantic
mismatch eligible for XFAIL. Capture, resource, serialization, allocation, or
invariant terminals remain unexpected outcomes.

## Non-claims and deferred work

AG3 itself adds immutable classification metadata and accounting only. It does
not implement a runner, subsystem adapter, execution request, environment
inspection or provisioning, lane selection, source/WPT importer, cross-engine
capture, rendered/raster comparison, browser automation, observed outcome,
attempt state, result normalization, XFAIL/XPASS evaluation, or production
browser behavior. AG4 consumes AG3 views and eligibility for parser execution;
AG5 consumes the same metadata boundary for CSS execution.

AG6 consumes the same logical metadata boundary for authored Layout/Paint
snapshots, and AG7 consumes it after static document-reference relation
evaluation. Browser/runtime execution adapters, broader lane selection and
environment provisioning, broad WPT/source importing, cross-engine capture,
rendered/raster comparison, and browser automation remain future Milestone AG
work.

AG3 therefore closes the expected-result metadata issue without closing
Milestone AG or claiming broad WPT, HTML, CSS, rendering, or browser
compatibility.

# AG6 logical metadata rule

AG3 V1 metadata remains one-to-one with logical AG2 `TestId`. Rendering widths
and selected Layout/Paint profiles do not carry expectation, capability,
harness, environment, stability, or lane overrides. Expected-fail policy is
derived for each aggregate variant assertion. A future AG3 schema version is
required if genuine cases need different policy by width or profile.
