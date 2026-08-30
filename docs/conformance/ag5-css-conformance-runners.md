# AG5 CSS conformance runners

Status: implemented focused CSS conformance infrastructure

AG5 executes seven CSS semantic profiles through production HTML and CSS APIs.
It does not claim broad CSS, WPT, or browser compatibility.

## Ownership and dependency boundary

```text
conformance-runner (AG orchestration, outer/nested reconciliation, reports)
  |-- conformance-test-support (engine-independent inventory/metadata)
  `-- css-test-support (strict nested CSS packages and adaptation)
        |-- css (all CSS semantics)
        `-- html (document parsing and parser-created DOM)
```

The runner's default feature closure remains subsystem-neutral. CSS execution
is enabled only by its `css` adapter feature. Cargo-metadata tests prove that
the enabled closure stops before Layout, Paint/GFX, Browser/runtime,
JavaScript, networking, and application orchestration.

`conformance-runner` alone reconciles a validated AG2 outer fixture with a
validated nested CSS package. `css-test-support` has no dependency on either
generic AG crate. It owns only nested schema/file validation, neutral target
addressing, phase-specific adaptation, bounded observations, and comparison.
It never implements property parsing, selector semantics,
specificity, matching, cascade, inheritance, or computed values. The production
`css` crate contains no AG schema, fixture labels, expectations, or reporting.

## Profiles and AG2 surfaces

| CSS execution profile | AG2 observation | authoritative path |
| --- | --- | --- |
| property/value | `css-parsing` | stylesheet model parse -> declaration coordinate -> registry/specified or shorthand parse |
| selector parsing | `css-selectors` | selector source -> `parse_selector_source_with_limits` -> `parse_selector_list_with_limits` |
| selector specificity | `css-selectors` | direct selector parse -> one `ComplexSelector::specificity()` per selector |
| selector matching | `css-selectors` | document parse -> one projection -> checked matcher |
| cascade winner | `css-cascade` | explicit stylesheet inputs -> rule collection -> `StyleResolutionExecution` -> resolved winner provenance |
| inheritance/CSS-wide | `css-cascade` | resolved document style and typed source provenance |
| computed style | `computed-style` | projection-compatible resolved wrapper -> projection-compatible computed wrapper around `ComputedDocumentStyle` |

Property/value uses an authored stylesheet only because the current AD5
model-layer authored carrier for `DeclarationValue` is stylesheet parsing. Its
explicit rule/declaration coordinate selects the declaration deterministically;
the profile invokes no DOM, selector matching, cascade, or document-style path.
A new declaration-list parser would be a production CSS API addition rather
than a harness convenience and is not required by AG5.

Its strict carrier is singular and contains no cascade metadata:

```toml
[input]
stylesheet = "input.css"

[property]
rule_index = 0
declaration_index = 0
```

Selector parsing and specificity consume a strict selector-list input directly.
They do not fabricate a style rule or invoke stylesheet, DOM, matching,
cascade, or computed-style code. Invalid and unsupported selector results are
typed semantic observations; specificity is never aggregated across a list.
The source-text entry point only performs production token/component-value
parsing before delegating to the pre-existing authoritative selector-list
parser; it is not a stylesheet carrier.

Combined cascade profiles accept an ordered bounded list of author, user, and
namespace-constrained user-agent stylesheet inputs. Each declares a path,
origin, source identity, and stylesheet order. Rule collection remains the
production owner of origin/importance/source ordering. Inline styles arrive
only through parser-created `style` attributes and are never fabricated as
stylesheet rules.

Their stylesheet carrier is deliberately distinct:

```toml
[input]
stylesheets = [
  { path = "author.css", origin = "author", order = 0, source = 0 },
  { path = "user.css", origin = "user", order = 1, source = 1 },
]
html = { kind = "document", path = "document.html" }
```

Combined profiles use `html::parse_document` with fixed bounded tokenizer,
tree-builder, and error-tracking options. Fixtures cannot weaken safety limits.
Parser-selected `DocumentMode` is passed to CSS's matching environment. Inline
styles arrive only through parser-created `style` attributes and the existing
element-attached cascade path.

## Attempt and input-integrity boundaries

Package loading, strict schema validation, AG2 reconciliation, and comparison
contract validation occur before production execution. A case becomes attempted
when its first production engine API is invoked. After `parse_document` begins,
parser failure, degraded HTML semantic input, target failure, projection or
matching resource failure, cascade failure, computed failure, and required
observation failure all remain typed attempted outcomes.

AG5 consumes the HTML-owned semantic-completeness contract documented in
`docs/html5/html-parser-semantic-completeness-contract.md`. Recoverable parse
errors and dropped auxiliary error records do not block CSS. Fatal parser errors
are attempted failures; typed semantic degradation is an attempted
`HtmlSemanticInputResourceLimited` prerequisite failure. CSS does not inspect
parser counters.

## Neutral targets and projection compatibility

Fixtures identify a target by a label plus a typed structural path that starts
at the parser-created document root. Every step indexes the complete ordinary
child list, must select an element, and asserts that element's namespace and
local name. Text, comments, processing instructions, doctypes, and any other
ordinary non-element children therefore affect later indexes; there is no
special search for the document element. The address is an assertion about the
entire traversed parser-created DOM structure, not an element-only locator.
Resolution never uses the CSS matcher, a marker attribute, parser node ID, or
selector element ID. A missing child, non-element child, or intermediate/final
name or namespace substitution is a typed attempted target-resolution failure,
making parser regressions visible.

CSS then maps the resolved source element through an opaque
`StyleProjectionElementKey`. A raw `SelectorDomElementId` is never accepted as
cross-projection proof. CSS validates immutable root/source identity, selector
identity, document order, namespace/local name, projection shape, and matching
environment before accepting a key. Compatible projections over the same
immutable input may interoperate; unrelated projections may not. AG5 does not
run the independent cascade diagnostic when the requested observation is a
resolved or computed style; diagnostic-only limits cannot fail those profiles,
and cascade is evaluated once. Resolved output used by the AG5 combined lanes is carried as a
`ProjectionResolvedDocumentStyle`; its private source-root, matching-
environment, and projection-shape provenance is validated before computed
materialization. The resulting `ProjectionComputedDocumentStyle` retains the
same provenance for target lookup. Bare `ResolvedDocumentStyle` or
`ComputedDocumentStyle` artifacts cannot be combined with a projection key
through `StyleResolutionExecution`. The normal full resolved-entry identity,
namespace, and name validation remains authoritative.

## Strict V2 package reconciliation

Executable CSS cases require outer `borrowser-conformance-fixture-v2` and
exactly one nested `borrowser-css-fixture-v1` descriptor. Before execution the
CSS adapter validates exact outer/nested test ID equality, profile-to-surface
mapping, profile-specific primary authored input and `test_path`, required and
forbidden inputs/expectations, all declared support files, containment,
portability, regular-file/no-symlink rules, duplicates, missing files, and
AG2's default-deny file set. No meaning is inferred from paths or extensions.
The primary input is the declared selector-list path for selector parsing,
specificity, and matching; it is the phase-specific `[input].stylesheet` for
property/value and the first declared `[input].stylesheets` entry for combined
style profiles. Every additional stylesheet, HTML input, and expected snapshot
is an explicit support path.

Selector matching deliberately retains the authored selector list as its AG2
`test_path`. This is a non-semantic deviation from the original AG5 plan, which
named the HTML request: the selector list is the selector-owned behavior and
stable logical identity under observation, while HTML is required support
context. The HTML document or fully contextual fragment request remains
explicit and is never inferred from a path, directory, or extension.

`harness = ready` asserts that this real package and comparison path reconcile
even when engine capability makes the case ineligible. Unclassified or
harness-not-ready cases need no executable package unless the generic AG
contract requires one.

Standards-style HTML fragment requests are representable as `kind =
"fragment"` plus a strict context namespace and context local name, but
Borrowser has no canonical contextual fragment parser. Such a
case must declare that engine capability unavailable and is not executed. AG5
never wraps a fragment in a synthetic document and does not claim executable
fragment parsing.

## Semantic results, reports, and limits

Invalid/unsupported CSS is distinct from AG capability and harness readiness.
A runnable case can successfully observe invalid selectors, unsupported
selectors or properties, inactive/deferred rules, or rejected values. Resource
exhaustion and incomplete required evidence are failures, never partial
semantic success. In particular, CSS model `ParseStats::hit_limit`, selector
`ResourceLimitExceeded`, specified-value exhaustion, and shorthand-expansion
exhaustion map to closed attempted-execution resource categories rather than
authored-invalid observations.

CSS terminal outcomes preserve typed execution/resource failure, incomplete
required observation, final invariant/integrity failure, and semantic mismatch
as separate subsystem results. `borrowser-conformance-css-report-v1` is a separate bounded deterministic
framed-text report. It preserves every AG classification dimension, structural
attempt state, lossless typed CSS terminal outcome, and independently derived
policy. Expected-failure policy never rewrites an observation. The existing
`borrowser-conformance-parser-report-v1` bytes remain locked unchanged.

AG5 does not copy generic AG constants into `css-test-support`.
`conformance-runner` constructs a validated `CssFixtureLimits` at the adapter
boundary and the nested package retains that configuration for bounded loading
and observation construction. The ownership/source of each bound is:

| bound | owner and source |
| --- | --- |
| nested descriptor bytes | generic AG transport: AG2's 64 KiB descriptor limit, passed by the runner |
| required expected/actual observation bytes | generic AG report transport: AG4/AG5's 8 MiB per-observation limit, passed by the runner and enforced during construction |
| outer support paths | generic AG2 package transport: 256 paths |
| combined stylesheets | the smaller of CSS `StyleResolutionLimits` and the AG2 support-path capacity after accounting for HTML and the expectation |
| selector/stylesheet authored bytes | CSS production `SyntaxLimits` |
| HTML authored input bytes | independently versioned CSS nested-fixture bound: 4 MiB, chosen to keep authored support input below retained observation/report capacity with CI headroom |
| targets | independently versioned CSS nested-fixture bound: 256; labels are independently capped at 128 bytes and their maximum retained label evidence fits the configured AG observation capacity |
| structural target depth | HTML production `HtmlTreeBuilderLimits::max_open_elements_depth` |
| target and fragment-context local names | HTML production tokenizer tag-name byte bound |
| selected properties | canonical CSS property-registry cardinality, with unique supported names required |
| selector parsing/matching, specified values, rule collection, style resolution, computed materialization | the corresponding CSS production limits |
| HTML parsing | the fixed bounded HTML production options documented above |

The runner has compatibility tests tying configured nested descriptor and
observation bounds exactly to AG transport/report bounds, and proving combined
stylesheet file accounting fits the AG2 package envelope, CSS-owned HTML input
capacity remains below AG observation capacity, and maximum retained target
label bytes fit that capacity. `css-test-support`
separately proves its derived stylesheet and target-depth limits do not exceed
their production owners. The 4 MiB HTML and 256-target limits are explicitly
CSS nested-fixture versioned values, not aliases for AG constants.

Exact maximum and maximum-plus-one tests protect new retained dimensions.
Checked arithmetic and fallible reservation remain authoritative. Required
evidence is never silently truncated.

Expected snapshots are reviewed semantic records derived from AD/AF tests or
independently understood standards behavior. They are not generated-and-blessed
implementation output and are not correctness oracles. Rust `Debug` is not a
stable report codec.

During AG5 migration, the property, cascade, and computed seeds retained their
logical IDs only after restoring their original authored cases. The former
`css-selectors-basic-stylesheet` inventory seed could not be strictly refined:
phase-correct selector parsing requires an authored selector-list rather than a
stylesheet. It was therefore retired and replaced by the stable
`css-selector-parsing-basic-list` ID instead of silently changing TestId
meaning.

Use `make check-conformance-css` for the CI-safe repository corpus. It ends at
`ComputedDocumentStyle`: no `StylePhaseOutput`, Layout, Paint, CSSOM,
JavaScript, media queries, custom properties, animations, transitions, dynamic
pseudo-state, pixel comparison, or broad WPT import is part of AG5.
