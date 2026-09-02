# AG1 Conformance Harness Architecture And No-JS Scope

Status: architecture and scope contract for Milestone AG issue 1

Last updated: 2026-08-26

AG1 defines the ownership, classification, accounting, and current no-JavaScript
scope for Borrowser's future conformance infrastructure. It is a contract-only
issue. It does not add a generic harness crate, fixture schema, adapter, runner,
reporter, CI lane, cross-engine capture tool, raster comparison, or production
engine behavior.

Milestone AG is intended to make conformance evidence visible and honest for
Borrowser's current static HTML/CSS renderer. AG1 does not claim broad Web
Platform Tests coverage, full HTML or CSS conformance, rendered-output reftest
support, or general browser compatibility.

## Related Contracts And Evidence

The existing subsystem contracts remain authoritative:

- [AE13 parser conformance and regression harness](../html5/ae13-parser-conformance-regression-harness.md)
- [AE13e external fixture and snapshot workflow](../html5/ae13e-external-fixture-and-snapshot-workflow.md)
- [parser fixture format v3](../html5/parser-fixture-format-v3.md)
- [AE14 HTML parser foundation closeout](../html5/ae14-html-parser-foundation-closeout.md)
- [AF1 selector, cascade, and computed-style architecture](../css/af1-selector-cascade-computed-style-architecture-contract.md)
- [AF10 selector, cascade, and computed-style closeout](../css/af10-selector-cascade-computed-style-conformance-closeout.md)
- [V1 rendering ownership and phase contracts](../rendering/v1-rendering-architecture-ownership-phase-contracts.md)
- [V6 deterministic rendering debug surfaces](../rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md)
- [W8 Layout box-generation debug surfaces](../rendering/w8-box-generation-formatting-debug-surfaces.md)
- [AA8 Paint operation regression surface](../rendering/aa8-paint-debug-visual-regression-surface.md)
- [AA9 Paint invariants and future raster boundary](../rendering/aa9-paint-model-invariants-extension-points.md)
- [AC10 retained rendering runtime closeout](../rendering/ac10-retained-rendering-runtime-closeout.md)

The existing `tests/wpt/` tree and `html-test-support` fixture infrastructure
are architectural evidence and migration inputs. Their current manifest and
disposition models are not the complete future AG state or accounting model.

## Conformance Unit And Granularity

AG distinguishes the following concepts:

- a **source record** is the native or external material from which tests are
  derived;
- a **logical case** is one classified test case from that source;
- an **execution variant** is a case combined with relevant execution
  parameters, such as parser delivery strategy or rendering environment;
- an **observation surface** identifies the subsystem-owned result being
  compared; and
- a **lane or execution request** selects runnable variants under a named
  policy.

One source record may yield multiple logical cases. One logical case may yield
multiple variants or observation surfaces. Classification and accounting must
therefore declare their granularity; counts at different granularities must not
be combined as if they described the same population.

## Federated Ownership

AG is the system-wide owner of:

- discovery;
- classification;
- lane and run selection;
- fixture, source, and provenance bookkeeping;
- delegation to subsystem execution boundaries;
- normalized result collection for implemented adapters;
- reporting and summaries; and
- cross-engine artifact bookkeeping.

The intended architecture is:

```text
AG orchestration
  -> subsystem adapter or existing canonical subsystem runner
  -> production subsystem
  -> subsystem-owned canonical observation
  -> future AG normalized result/accounting layer
```

AG must not become an independent implementation of HTML parsing, CSS parsing
or styling, Layout, Paint, or Browser/runtime behavior. A subsystem adapter may
translate orchestration requests and subsystem-owned results, but it must not
reimplement algorithms, manufacture semantic precision, or create a second
semantic truth path.

AG4 realizes the first execution slice with a separate test/tooling
`conformance-runner` crate. `conformance-test-support` remains the generic,
engine-independent inventory/classification layer; `conformance-runner`
depends downward on it and on `html-test-support`; `html-test-support` remains
independent of AG and delegates to `html::conformance`. This is the future home
for subsystem-neutral orchestration and additional adapters, but AG4 implements
only the HTML parser adapter.

The subsystem boundaries are:

| Owner | Conformance responsibility | AG must preserve |
| --- | --- | --- |
| HTML/parser | tokenizer behavior, tree construction, parser-created DOM, parser diagnostics and canonical parser observations | `html::conformance` remains the parser-owned observation boundary |
| `html-test-support` | parser fixture validation, expectation comparison, disposition evaluation, and canonical parser-fixture execution | it remains the one canonical parser-fixture runner |
| CSS | CSS syntax/property behavior, selector matching, cascade, specified/computed behavior, and CSS-owned deterministic projections | no selector, cascade, property, or computed-style semantics move into AG |
| Layout | layout structures, used geometry, and Layout-owned deterministic projections | AG does not infer geometry from DOM or CSS inputs independently |
| Paint/GFX | semantic paint primitives, paint order, operation output, and backend execution | AG does not reconstruct paint operations or treat them as pixels |
| Browser/runtime | end-to-end static document execution, scheduling, retained artifacts, and rendering debug output | AG does not reinterpret retained-state or orchestration semantics |
| AG tests/tooling | cross-subsystem coordination and accounting | delegation and normalized bookkeeping only |

Production and conformance execution must use the same subsystem implementation
paths. Internal debug snapshots are versioned regression contracts, not public
browser APIs and not inputs that may steer production behavior.

### AG8 external-source boundaries

AG8 preserves the federated boundary with these direct test-tooling edges:

```text
html-test-support -> external-test-provenance
html-test-support -> html
conformance-test-support -> external-test-provenance
wpt-test-support -> external-test-provenance
wpt-test-support -> conformance-test-support
wpt-test-support -> html5ever
rendering-test-support -> wpt-test-support
rendering-test-support -> conformance-test-support
rendering-test-support -> html, css, layout, gfx
conformance-runner -> conformance-test-support
conformance-runner -> selected subsystem test-support crates
```

`conformance-runner` has no direct WPT dependency. `external-test-provenance`
contains only source-neutral identity, path, revision, digest, licence, and
attribution primitives and has no dependency on Borrowser engine or AG crates.
`conformance-test-support` never names a WPT type. WPT interpretation projects
only generic `SourceRequirements` downward; WPT-specific forms, reference
graphs, automation/readiness/server facts, and summaries remain WPT-owned. No
production library or binary target depends on this test tooling.

## Orthogonal Conformance State

AG's conformance state is multidimensional. The following dimensions must not
be collapsed into one mutually exclusive status enum or one overloaded result:

| Dimension | Meaning | Conceptual values |
| --- | --- | --- |
| Classification completeness | whether required case/variant metadata has been established | classified, not yet classified |
| Engine/platform capability availability | whether Borrowser's production engine/platform has a sufficient execution path for the capability required by the case/variant and requested surface | available, unavailable with explicit missing capability and reason, not yet established |
| Harness executability/readiness | whether AG has truthful delegation, source-format handling, expectation-representation, observation, and comparison/capture infrastructure for the case/variant and requested surface | ready, not ready with an explicit infrastructure limitation, not yet established |
| Execution eligibility | whether the declared engine execution boundary, harness path, observation/comparison surface, and required environment permit a truthful execution attempt | runnable, not runnable with explicit unmet prerequisites, not yet established |
| Expectation | the anticipated semantic or execution result | expected pass, expected fail with an explicit expected failure class |
| Lane/run selection | whether a runnable case/variant is included in a named lane or execution request | selected, excluded/skipped with lane and reason |
| Stability | what repeated execution history says about reliability | stable, flaky, not yet established |
| Execution-attempt state | whether this run actually attempted execution | attempted, not attempted |
| Observed execution outcome | the terminal result of an attempted execution | semantic pass, semantic fail, execution error, resource failure, timeout |

An observed execution outcome exists only after an attempted execution. `Not
attempted` is an execution-attempt state, not an observed outcome. Likewise,
`flaky` is stability metadata derived from execution history or repetition; it
is not an observed outcome.

AG4 makes this invariant structural in its normalized model: the attempted
branch owns exactly one terminal observed execution outcome, while the
not-attempted branch cannot contain one. Typed evaluation information produced
before a subsystem executor is invoked may be retained separately, but it is
not relabeled as an observed execution outcome.

Selection is relative to a named lane or concrete execution request. A test
can be runnable, expected-failing, known-flaky, excluded from normal CI, and
selected in a scheduled or local extended lane at the same time.

Engine/platform capability availability and harness readiness are separate
truths. Borrowser may have a production execution path for the required
capability while AG lacks an adapter, representable expectation, observation,
or comparison path needed to execute the case truthfully. Capability
availability states only that the production path exists; it does not state
that its behavior is standards-correct, broadly feature-complete, or expected
to pass the case. For example, this is a valid combination:

```text
engine capability available
harness ready
runnable
expected pass
attempted
observed semantic fail
```

Some genuinely absent platform capabilities, such as a JavaScript execution
environment for a script-dependent test, remain engine/platform capability-
availability gaps and can also make a truthful attempt impossible. The engine
gap and the resulting execution ineligibility must still be reported
separately.

`Runnable` therefore means that the prerequisites for a truthful attempt are
available. It does not mean that Borrowser is expected to pass, or that the
behavior under test is conformant. The conceptual inputs are:

```text
engine/platform capability availability
        +
harness readiness
        +
required execution environment
        -> execution eligibility
```

Future implementations may derive eligibility from declared case requirements,
these inputs, and the requested observation/comparison surface, but AG1 does
not define a Rust formula or precedence algorithm.

Engine/platform capability unavailability is not a synonym for skipped. A
harness limitation is not an engine/platform capability-availability gap.
Skipped or excluded describes a lane policy applied to an otherwise runnable
case/variant. Not-yet-classified cases cannot be treated as
capability-unavailable or harness-not-ready merely to make classification
appear complete.

## Expectation, Policy, And Observed Conformance

Expectations and observed outcomes remain independently reportable:

| Expectation | Observed outcome | Derived policy label | Conformance meaning |
| --- | --- | --- | --- |
| expected pass | semantic pass | expected pass | observed pass |
| expected pass | semantic fail | unexpected fail | observed fail |
| expected fail | matching semantic fail | expected failure / XFAIL | observed fail; policy expectation matched |
| expected fail | semantic pass | unexpected pass / XPASS | observed pass; expectation is stale |
| expected fail | different failure or non-semantic outcome | unexpected outcome | preserve the exact observed outcome |

A matching expected failure may allow a regression policy to succeed, but it
must never be converted into a standards-conformance pass. Execution errors,
resource failures, and timeouts remain distinct from semantic failures even if
a narrowly scoped engine-hardening fixture anticipates one of those outcomes.

### AE13 Disposition Reconciliation

The current AE13 `DispositionEvaluation::Pass` means that a parser fixture's
observed outcome satisfied its declared disposition. It can therefore describe
an active fixture that completed, a matching expected failure, or a matching
expected unsupported result. This is a parser-runner policy result, not a
universal standards-conformance result.

AG4 parser normalization preserves the underlying expectation,
engine/platform capability availability, harness executability, execution
eligibility, execution-attempt state, and observed outcome instead of
translating AE13's policy-level `Pass` directly into a conformance pass. AG1
itself does not change AE13 behavior; AG4 consumes a narrow rich evaluation
view derived from the same canonical AE execution/comparison path.

## Independent Classification Axes

Source provenance, upstream test form, and Borrowser observation surface are
separate axes. None can be inferred safely from another.

### Source And Provenance

The source classes include:

- native Borrowser fixture;
- pinned or adapted WPT source;
- other pinned external source; and
- controlled real static-page fixture.

External source identity, revision, hashes, licence, attribution, and
adaptation records follow the default-deny discipline established by AE13e.
Fixture presence alone does not establish provenance or standards coverage.

### Upstream Or Source Test Form

Where applicable, source forms include:

- WPT reftest;
- `testharness.js`;
- wdspec/WebDriver;
- manual or visual test;
- parser `.dat`; and
- other source-specific forms.

The source form describes how upstream material is authored or intended to be
executed. It does not state that Borrowser implements the behavior or that AG
has an automated execution/comparison path. A manual or visual source test can
exercise behavior for which Borrowser has a production capability path while
remaining not runnable through AG because no truthful automated observation
and comparison path exists.

### Borrowser Observation Surface

Borrowser-owned conformance surfaces include:

- tokenizer;
- tree construction;
- DOM;
- CSS syntax and property behavior;
- selector, cascade, and computed style;
- Layout geometry;
- Paint semantic operations;
- Browser/runtime end-to-end semantic output; and
- future raster output.

Tokenizer, Layout, and Paint fixture categories are Borrowser conformance
surfaces. They are not official WPT test types. A pinned WPT input can be
adapted to a Borrowser surface only when the mapping and loss of information
are explicit and do not invent stronger semantics than the source provides.

## Current No-JavaScript Conformance Scope

Milestone AG targets deterministic tests for the current static renderer. Its
recognized architecture categories are:

- tokenizer fixtures;
- tree-construction fixtures;
- parser-created DOM tree snapshots;
- CSS syntax, declaration, property, and value fixtures;
- selector, cascade, and computed-style fixtures;
- Layout tree and geometry fixtures;
- Paint semantic primitive and operation fixtures;
- Browser/runtime end-to-end semantic fixtures;
- semantic or structural reference fixtures; and
- controlled real static-page fixtures with pinned inputs and resources.

This list defines recognized Borrowser observation categories. AG1 does not
claim that one generic harness already discovers or executes every category.
Existing execution remains with current subsystem tests and canonical runners
until later AG implementation issues add delegation and bookkeeping.

### Capability-Based Classification

Classification depends on the actual capabilities required by a case, variant,
and requested observation surface. It must not rely on filenames, directory
names, or crude searches through HTML text.

For example, an HTML document containing a `<script>` element may be runnable
for tokenizer, tree-construction, or inert parser-created DOM observation. It
has an unavailable engine/platform capability for a variant whose required
execution includes JavaScript, parser-blocking script semantics, script-driven
DOM mutation, or another absent platform facility. That variant is also not
runnable until the execution-enabling platform and conformance prerequisites
exist, but these remain separate classifications.

The current engine/platform behavior categories whose required capability is
unavailable or deferred include:

- JavaScript execution;
- script-dependent `testharness.js` cases;
- DOM APIs requiring JavaScript bindings;
- events and event-loop behavior;
- timers and microtasks;
- wdspec/WebDriver test execution;
- CSSOM and `getComputedStyle()` API behavior;
- interaction tests;
- navigation and history tests;
- storage and cookie tests; and
- dynamic mutation behavior requiring unavailable DOM, script, event, or
  platform/runtime facilities.

Engine/platform capability-unavailable classification must identify the
required missing browser capability. Harness-not-ready classification must
instead identify the conformance-infrastructure limitation. Such limitations
can include:

- no AG or subsystem adapter for the source form;
- unimplemented source-format support;
- an expectation that the current representation cannot encode without
  semantic invention;
- no suitable observation, comparison, or capture surface; or
- another explicitly classified conformance-infrastructure limitation.

These limitations can make a case not runnable even when Borrowser has
the relevant production capability path. They must not increase Borrowser's
engine/platform capability-unavailable count.

Required execution-environment availability is an execution-eligibility
prerequisite, not harness readiness. It can include controlled fonts, a
required platform configuration, a versioned external browser, or a declared
rendering/capture environment. If the harness path exists but the environment
for a particular execution request is unavailable, the case is not runnable
for that environment reason; the harness does not become not ready. If AG
instead lacks infrastructure needed to provision, describe, or represent the
environment, that missing infrastructure is a harness-readiness limitation as
well as an execution-eligibility prerequisite.

### AE13e Classification Reconciliation

AE13e already distinguishes production parser capability gaps from adapter and
representation limitations. Its `unsupported-parser-feature` classification
identifies a production parser capability outside the supported AE profile.
`unsupported-expectation-representation` identifies a case whose source
expectation cannot be represented truthfully by the current canonical format.
Malformed or unimportable records describe source/adapter handling rather than
browser behavior.

Future AG models must generalize these distinctions across subsystems. They
must not flatten AE13e's adapter/source-format or expectation-representation
limitations into an HTML/parser unsupported-feature count.

## Reference Comparison Terminology

A WPT reftest compares rendered output against one or more rendered references.
Borrowser must reserve **WPT reftest** and **rendered-output reftest** claims for
an appropriate future rendered or raster comparison path.

Current deterministic outputs are semantic and structural regression surfaces.
AG uses explicit terms such as:

- semantic reference fixture;
- structural reference fixture;
- layout-geometry reference fixture;
- paint-operation reference fixture; and
- rendering-phase reference fixture.

DOM snapshots, `LayoutPhaseOutput` snapshots, Paint operation snapshots, and
Browser rendering-phase snapshots can compare a Borrowser-owned semantic
result with a declared semantic reference. They do not constitute WPT reftest
execution, screenshot comparison, pixel equality, or raster conformance.

AG1 does not define screenshot infrastructure, raster tolerance, fuzzy image
matching, font rasterization policy, platform rendering normalization, or
pixel-comparison semantics. Those require later issues with explicit
determinism and environment contracts.

## Cross-Engine Comparison Boundary

Chromium, Firefox, WebKit, and other engines do not expose Borrowser's internal
computed-style representation, Layout tree, Paint primitives, or retained
rendering state as shared interfaces. Borrowser-internal snapshots remain
valuable regression evidence, but they are not automatically cross-engine
comparison formats.

Future comparison must use a deliberately comparable external observation or
captured artifact whose semantics are defined independently of either engine's
internal representation. Possible future classes include a standards-oriented
parser expectation, a normalized externally observable DOM or selected style/
geometry projection, or a controlled raster artifact once raster comparison
has its own contract.

Another browser engine supplies comparative evidence, not an unquestionable
correctness oracle. Differences require review against the relevant standard,
source expectation, capture method, and known engine behavior.

### External Capture Provenance

An externally captured artifact must record the relevant reproducibility
context, including:

- engine or product;
- version or revision;
- platform and relevant architecture;
- viewport;
- device scale;
- controlled font environment where relevant;
- resource and network policy;
- fixture source revision and content hash;
- capture mechanism and tool version or configuration; and
- relevant browser or rendering flags.

Additional surface-specific environment facts may be required later. An
artifact lacking the context needed to reproduce or interpret its observation
cannot silently become authoritative comparison truth.

JavaScript, WebDriver, or browser automation required by the source test is a
test capability requirement. JavaScript, WebDriver, or browser automation used
externally only to capture an observation from another engine is capture
tooling. The latter does not mean Borrowser supports the former and must not
inflate Borrowser's JavaScript, DOM API, WebDriver, or platform claims.

## Conformance Accounting

Future AG summaries must expose separate inventory, engine/platform capability
availability, harness readiness, execution eligibility, selection, execution,
expectation, and stability accounting. At a declared and consistent
case/variant/surface granularity they include:

- discovered;
- classified and unclassified;
- engine/platform capability available, unavailable, and not yet established;
- harness ready, not ready, and not yet established;
- runnable, not runnable, and eligibility not yet established;
- selected and excluded/skipped for a named lane or run;
- attempted and not attempted;
- semantic pass and semantic fail;
- execution error, resource failure, and timeout;
- expectation match and mismatch;
- stable and flaky; and
- source, source-form, observation-surface, and lane breakdowns.

For the same population and granularity, inventory completeness may be stated
as:

```text
discovered = fully classified + not fully classified
```

When the relevant dimension has been assessed completely for a declared
population, additional dimension-specific identities may be stated without
mixing their meanings:

```text
engine-capability-availability-assessed = capability-available + capability-unavailable
harness-readiness-assessed = harness-ready + harness-not-ready
execution-eligibility-assessed = runnable + not-runnable
```

Cases whose state is not yet established remain visible outside the associated
`assessed` population. The old simplification `classified = runnable +
unsupported` is invalid because `not runnable` can result from a harness,
representation, comparison-surface, or environment limitation rather than an
unavailable Borrowser engine/platform capability.

When every runnable variant has received a decision for a named lane, the lane
may additionally account for runnable variants as selected plus excluded. An
execution report must keep selected-but-not-attempted variants visible rather
than deleting them. Observed outcome counts apply only to attempted executions.

Capability-unavailable, harness-not-ready, not-runnable, excluded/skipped,
unclassified, flaky, execution errors, resource failures, and timeouts must
remain visible and must not silently turn into passing conformance. Expected
failures remain observed failures even when the expectation policy matches.

Reports must expose engine/platform capability-availability gaps separately
from harness/adapter and execution-path gaps. A missing adapter or
unrepresentable expectation cannot inflate Borrowser's engine/platform
capability-unavailable count. Capability availability likewise cannot imply
standards conformance or that AG can execute every upstream source form that
exercises it. Only observed semantic outcomes provide pass/fail conformance
evidence.

AG1 does not define a global pass percentage. Any future rate must explicitly
identify its:

- numerator;
- denominator;
- case or variant granularity;
- source set;
- observation surface;
- lane; and
- exclusions.

The report must display exclusion, non-pass-outcome, and coverage counts
alongside a rate so that an apparently improving percentage cannot hide
shrinking execution or classification coverage.

## Lane Eligibility

Lane membership is selected from declared policy and capabilities; it is not a
permanent property encoded into the source test form.

### Normal CI

Normal-CI eligibility requires tests and dependencies to be:

- hermetic;
- deterministic;
- bounded in runtime, memory, output, and corpus size;
- pinned and repository-controlled;
- free of live network access and mutable upstream dependencies;
- free of locally installed external-browser requirements;
- free of uncontrolled host-font dependencies where rendering is sensitive;
- free of wall-clock-sensitive semantics;
- free of uncontrolled platform-dependent raster behavior;
- backed by validated provenance and licensing for imported sources; and
- stable rather than known-flaky unless a later explicit policy defines a
  non-conformance-gating use.

### Local, Manual, Or Scheduled Extended Lanes

Extended lanes may later permit:

- larger pinned corpora;
- longer but still bounded execution;
- explicitly versioned external browsers;
- cross-engine capture tooling;
- declared and controlled platform or font environments; and
- future raster comparison under a separate rendering determinism contract.

Every additional dependency must be explicit and reproducible. Extended-lane
results retain the same classification, observed-outcome, provenance, and
accounting requirements as normal CI; an extended lane is not an unclassified
escape hatch.

## Future WPT Expansion

Milestone AG creates a path for focused WPT adapters and selections without
turning WPT into one binary target. Future expansion must classify source form,
required capabilities, adapted observation surface, provenance, expectation,
and lane policy before execution.

Adding JavaScript, DOM APIs, event-loop behavior, WebDriver execution, CSSOM,
interaction, navigation, storage, or raster conformance requires the owning
engine/platform feature work as well as later conformance adapters. The AG
harness must not implement those browser features to make tests appear
runnable.

## Invariants

- AG coordinates and accounts; engine subsystems execute and own semantics.
- `html-test-support` remains the canonical parser-fixture runner.
- `html::conformance` remains the parser-owned observation boundary.
- Generic `conformance-test-support` remains engine-independent; subsystem
  execution dependencies belong only in `conformance-runner` adapters.
- Generic conformance code never duplicates parser, CSS, Layout, Paint, or
  Browser/runtime semantics.
- Classification completeness, engine/platform capability availability,
  harness readiness, execution eligibility, expectation, selection, stability,
  attempt state, and observed outcome remain orthogonal.
- `Not attempted` and `flaky` are never observed execution outcomes.
- Capability unavailable, harness not ready, not runnable, and skipped are
  distinct; expected failure is not conformance success.
- A missing adapter, expectation representation, or comparison surface never
  becomes an engine/platform capability-unavailable count.
- An unavailable required execution environment makes execution ineligible;
  it is not a harness-readiness failure unless AG infrastructure for
  provisioning or representing that environment is itself missing.
- Engine/platform capability availability never implies standards conformance
  or that AG already has a truthful execution and comparison path for every
  relevant source test.
- Policy-level success and observed standards-conformance outcome remain
  separately reportable.
- Source provenance, source test form, and Borrowser observation surface remain
  distinct.
- Capability classification is based on actual requirements, not filename or
  markup heuristics.
- Semantic snapshots remain internal regression contracts and do not imply
  rendered-output reftest or raster support.
- Cross-engine evidence uses a deliberately comparable observation with
  reproducible provenance.
- External capture tooling does not expand Borrowser's claimed capabilities.
- Accounting identities apply only at a consistent declared granularity.
- Normal CI remains hermetic, deterministic, bounded, pinned, and stable.
- No summary silently converts capability-unavailable, harness-not-ready,
  not-runnable, skipped, unclassified, flaky, error, resource-failure, timeout,
  or expected-failure state into a pass.

## Deliberate Exclusions

AG1 deliberately excludes:

- a generic conformance crate or Rust API;
- serialized AG metadata or normalized-result schemas;
- subsystem adapter implementations;
- fixture migrations;
- a generalized WPT runner;
- result reporters or summary generators;
- new CI targets or workflows;
- cross-engine capture automation;
- raster screenshots, fuzzy comparison, or tolerance policy;
- JavaScript, DOM bindings, events, timers, WebDriver, CSSOM, interaction,
  navigation, storage, or dynamic platform behavior;
- changes to existing parser, CSS, Layout, Paint, Browser/runtime, snapshot, or
  fixture semantics; and
- any claim of broad WPT, HTML, CSS, rendering, or browser compatibility.

Later Milestone AG issues may implement the documented orchestration and
accounting layers in small, testable slices. They must preserve this federation
and may not broaden engine capability merely through harness classification.

## AG1 Decision

AG1 is closeable when this architecture and current no-JavaScript scope are
documented and discoverable. Closing AG1 means only that the design contract is
defined. The Milestone AG harness, metadata, delegation, reporting,
cross-engine workflow, and future raster infrastructure remain outstanding.

# AG6 execution-variant refinement

AG6 materializes the AG1 logical-case/execution-variant distinction. AG2
`TestId` remains logical identity; a rendering attempt is keyed by the typed
pair `(TestId, RenderingExecutionVariantId)`. Width variants execute
independently and remain ordered by their typed environment and width values.
Parser and CSS V1 results retain an internal singleton execution variant whose
identity is deliberately absent from their existing report formats.
