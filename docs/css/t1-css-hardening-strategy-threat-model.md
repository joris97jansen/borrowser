# T1: CSS Hardening Strategy, Invariants, And Threat Model

Last updated: 2026-04-23  
Status: hardening strategy contract implemented

This document is the source-of-truth contract for Milestone T issue 1. It
defines the hostile-input model, subsystem invariants, expected failure modes,
determinism requirements, and resource-bound expectations for Borrowser's CSS
pipeline before the follow-up Milestone T implementation work adds additional
guards, fuzz targets, corpus replay, and CI smoke lanes.

Note: this document intentionally preserves the T1 strategy/threat-model
framing. For the implemented Milestone T limits, commands, regression
workflow, and CI practice, use `docs/security/css-hardening.md` as the
operational source.

The CSS pipeline is a hostile-input surface. A stylesheet, inline style
attribute, selector prelude, declaration value, or DOM shape influenced by
network content must be treated as attacker-controlled input.

Related code:
- `crates/runtime_css/src/lib.rs`
- `crates/css/src/syntax`
- `crates/css/src/model`
- `crates/css/src/selectors`
- `crates/css/src/cascade`
- `crates/css/src/properties`
- `crates/css/src/specified`
- `crates/css/src/computed`
- `crates/browser/src/tab/css.rs`

Related documents:
- `docs/css/syntax-parser-contract.md`
- `docs/css/n7-resource-limits-parser-invariants.md`
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/p1-selector-architecture.md`
- `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`
- `docs/security/css-hardening.md`
- `docs/security/html5-tokenizer-hardening.md`

## Goals

Milestone T hardens the CSS subsystem to the same operational standard as the
HTML5 parser hardening work:

- malformed CSS must be normal recoverable input, not an engine failure
- attacker-controlled CSS must not cause panics when public API contracts are
  respected
- parser, selector, cascade, value, and computed-style work must remain bounded
- failure behavior must be deterministic and reproducible
- fuzz findings must become durable corpus or regression fixtures
- CI must include cheap, deterministic hardening smoke coverage
- hardening assumptions must be documented near the CSS contracts they protect

T1 is a strategy issue. It does not claim every guard already exists. Follow-up
Milestone T issues must use this document to decide which guards, fuzz targets,
tests, and CI lanes are required.

For the implemented day-to-day contributor workflow after Milestone T, see
`docs/security/css-hardening.md`.

## Alignment With HTML Hardening

Milestone T adopts the same hardening disciplines already used for the HTML
tokenizer and parser work:

- explicit limit ownership
- deterministic malformed-input recovery
- panic-free public entry points under documented API contracts
- bounded diagnostics and debug output
- committed regression corpus promotion
- deterministic replay commands
- fixed-budget CI smoke coverage

CSS hardening does not duplicate the HTML implementation. It applies the same
operational standard to the CSS-owned pipeline stages and keeps the CSS layer
boundaries defined by Milestones N through S.

## Threat Model

An attacker can influence:

- external stylesheet bytes fetched through the networking/runtime path
- decoded stylesheet text handed to `css::parse_stylesheet_with_options(...)`
- inline `style` attribute text handed to declaration-list parsing
- style-rule selector preludes
- declaration names, declaration values, `!important` placement, and comments
- at-rule names, preludes, and preserved blocks
- HTML/DOM shape used by selector matching, including deep and wide trees
- duplicate attributes, unusual attribute values, and element names visible
  through selector DOM adapters
- stylesheet insertion order through document content and load timing

Hostile CSS input classes in scope:

- arbitrary malformed stylesheets with unmatched delimiters, unterminated
  comments, bad strings, bad URLs, malformed at-rules, trailing garbage, and
  EOF-sensitive recovery cases
- extremely large stylesheet and inline-style inputs
- lexical floods such as many tiny tokens, long identifiers, long comments,
  long strings, repeated delimiters, and diagnostic-heavy inputs
- deeply nested blocks and functions inside values, at-rule bodies, and
  future value syntaxes
- selector lists with many entries, long compound selectors, long combinator
  chains, unsupported pseudo selectors, malformed attribute selectors, repeated
  combinators, leading/trailing combinators, and empty selector segments
- pathological selector matching cases over adversarial DOM projections, such
  as descendant or general-sibling selectors over deep or wide element trees
- cascade explosions from many stylesheets, many rules, many matched rules per
  element, many declarations per rule, many inline declarations, and many
  invalid declarations that still need deterministic accounting
- property/value edge cases such as unsupported properties, custom properties,
  invalid property names, malformed colors, bad units, numeric overflow,
  unsupported keywords, and unsupported future syntax such as `var(...)` or
  `calc(...)`
- recovery-heavy inputs where nearly every construct is malformed but later
  sibling rules or declarations should still be preserved when a defined
  boundary exists

Out of scope for this CSS hardening contract:

- making internal API misuse silently recoverable
- continuing after impossible internal invariant breaches such as foreign
  source spans, invalid selector IR construction, or contradictory DOM adapter
  facts
- guaranteeing survival after process-level out-of-memory or OS resource
  exhaustion
- implementing the full CSS standards surface as part of hardening
- origin policy, sandboxing, cache policy, CSP, and broader browser security
  policy

## Attack Surfaces

The hardening boundary covers every stage that consumes attacker-influenced CSS
or DOM shape:

1. `runtime_css`
   - receives stylesheet chunks, assembles decoded stylesheet text, handles
     abort/done events, and dispatches parse work
2. `css::syntax`
   - tokenizes, parses, recovers, emits diagnostics, enforces syntax limits,
     and produces structured syntax results
3. `css::model`
   - converts syntax output into long-lived stylesheet/rule/declaration/value
     structures and invokes selector parsing for style-rule preludes
4. `css::selectors`
   - parses selectors, computes specificity, validates selector structure, and
     matches parsed selectors against selector-facing DOM adapters
5. `css::cascade`
   - collects matched rule inputs, filters declaration applicability, resolves
     winners, applies inheritance/default source selection, and emits resolved
     document styles
6. `css::specified` and `css::computed`
   - parse supported declaration values, reject invalid values, normalize
     typed computed values, and assemble total runtime styles
7. debug, snapshot, corpus, and fuzz tooling
   - serializes results for deterministic regression review and must not
     introduce nondeterministic or unbounded output paths

Layout and paint are downstream consumers of `ComputedStyle` and `StyledNode`.
They are not CSS parse-recovery owners. If layout or paint needs a new CSS
fact, that fact must be modeled in the CSS property/value pipeline first.

## Global Invariants

These invariants apply across the CSS pipeline:

- malformed CSS input is recoverable input, not an engine failure
- public CSS entry points must not panic from malformed CSS when their API
  contracts are respected
- public CSS entry points include parse, model conversion, selector
  parsing/matching, cascade and document style computation, stable snapshot
  serialization, and committed corpus replay paths
- every untrusted-input loop must either have an explicit resource limit or a
  simple monotonic progress argument that can be tested
- recursive or stack-sensitive processing must have an explicit depth limit
- new CSS pipeline work must not introduce unbounded fanout or unexpected
  superlinear behavior on hostile input without an explicit documented bound,
  justification, and targeted regression coverage
- diagnostics must be bounded, typed, ordered by encounter order, and
  deterministic
- equivalent decoded input, DOM projection, stylesheet order, options, and
  feature configuration must produce equivalent results and snapshots
- invalid and unsupported selector states must remain explicit and
  non-matchable
- invalid and unsupported declarations must not become cascade candidates
- computed styles must be total over the supported property subset and must not
  store invalid values
- stable regression output must use versioned serializers, not incidental Rust
  `Debug`
- hardening limits must be configured through central options or limit structs,
  not scattered one-off constants
- CSS hardening does not guarantee graceful recovery from process-level
  allocation failure, but CSS code must enforce limits early enough to avoid
  avoidable allocation amplification from hostile input
- future use of `unsafe` in CSS pipeline code requires an explicit design
  review; the current CSS crate has no `unsafe` implementation surface

Internal invariant assertions are allowed for impossible engine states. They
must not be used as the normal failure path for malformed CSS input.

## Syntax Invariants

`css::syntax` remains the first CSS-owned hostile-input boundary after text
decoding.

Required invariants:

- syntax entry points consume decoded `&str` and a `ParseOptions` value
- input is bounded according to `CssParseOrigin`
- `CssSpan` values are source-bound, monotonic, and UTF-8 boundary checked
- token streams handed to structured parsing are source-bound, monotonic, and
  terminated by exactly one trailing EOF sentinel
- tokenization and parsing are deterministic for fixed input and options
- parsing never reparses raw strings through split-based fallback logic
- malformed constructs recover only at documented structural boundaries
- every recovery path advances the token cursor or terminates parsing
- limit hits set `hit_limit` and emit `DiagnosticKind::LimitExceeded` when
  diagnostics are enabled and capacity remains
- diagnostic collection is bounded by `max_diagnostics`
- stable syntax snapshots preserve explicit ordering and format-version
  contracts

The current syntax limits are defined by `SyntaxLimits`:

| Limit | Default | T1 expectation |
| --- | ---: | --- |
| `max_stylesheet_input_bytes` | `4 * 1024 * 1024` | Whole-stylesheet text is truncated before tokenization. |
| `max_declaration_list_input_bytes` | `64 * 1024` | Inline style text is truncated before tokenization. |
| `max_lexical_tokens` | `262_144` | Tokenization stops before unbounded token growth. |
| `max_rules` | `16_384` | Structured parsing stops before unbounded rule growth. |
| `max_selectors_per_rule` | `256` | Selector-list fanout must be enforced or explicitly audited by Milestone T follow-up work. |
| `max_declarations_per_rule` | `1_024` | Declaration parsing stops before unbounded declaration growth. |
| `max_component_nesting_depth` | `256` | Nested block/function parsing is depth bounded. |
| `max_diagnostics` | `128` | Diagnostic storage is bounded. |

Milestone T follow-up work must preserve these syntax guarantees while adding
fuzz replay and broader cross-pipeline coverage.

## Model Invariants

The engine-facing stylesheet model is built from structured syntax output.

Required invariants:

- model construction must not reparse raw stylesheet text
- model construction must preserve stylesheet, rule, selector, and declaration
  source order
- style rules must store `SelectorListParseResult` directly
- at-rules and unsupported blocks may be preserved structurally without adding
  unsupported semantics
- declaration values remain structured component trees, not runtime values
- property names are classified as standard, custom, or invalid explicitly
- selector parsing failures remain model data; they must not panic model
  construction
- spans carried by model nodes must resolve against the owning `CssInput` or be
  represented as absent debug spans

Malformed CSS may produce a partial model with diagnostics and omitted invalid
constructs. It must not produce structurally contradictory model data.

## Selector Invariants

Selector parsing and matching are attack surfaces because selectors are compact
inputs that can drive large DOM traversals.

Required parser invariants:

- selector parsing consumes syntax component values, not raw text reparsing
- selector parse results are exactly `Parsed`, `Unsupported`, or `Invalid`
- unsupported selector features remain explicit and non-matchable
- invalid selector syntax remains explicit and non-matchable
- parsed selector IR enforces structural invariants at construction time
- specificity is computed from parsed selector IR only
- selector debug snapshots are deterministic and versioned

Required matching invariants:

- matching consumes parsed selector IR, never raw selector text
- non-element nodes are not selector subjects
- traversal uses element-only parent and previous-sibling axes exposed through
  `SelectorMatchDom`
- unsupported and invalid selector results produce unsupported/invalid match
  outcomes, not parsed no-match outcomes
- selector lists are evaluated in source order
- complex selectors are evaluated right-to-left
- matching output is deterministic for equivalent DOM projections
- future selector optimizations must be observationally equivalent to the
  documented matcher semantics

Hardening must account for both the cost of one selector match attempt and the
aggregate repeated work of many selectors over many elements in one style pass.
Limits, fuzz harness budgets, and CI smoke budgets must account for both
dimensions.

Milestone T follow-up work must make selector parser and matcher resource
ceilings explicit. The expected limit families are:

- maximum selector entries per selector list
- maximum complex-selector segments per selector
- maximum simple selectors per compound selector
- maximum selector component values consumed per style-rule prelude
- maximum DOM-axis steps per selector match attempt or per element style pass
- maximum selector diagnostics or failure records stored per parse operation

## Cascade Invariants

Cascade hardening protects the fanout from many rules and many elements into
candidate declaration sets.

Required invariants:

- cascade consumes `SelectorListMatchOutcome`; it must not reparse selectors or
  recompute selector specificity from raw text
- unsupported, invalid, and no-match selector outcomes contribute no
  declaration candidates
- unsupported properties, custom properties, invalid property names, and
  invalid supported values remain explicit but non-comparable
- declaration candidates carry an explicit source and priority key
- rule order and declaration order are deterministic and source-order based
- winner resolution is stable for equal keys
- each supported property has at most one authored winner
- `ResolvedStyle` is total over the supported property subset
- structured document-level style resolution does not mutate DOM-attached
  compatibility style vectors
- legacy `attach_styles(...)` remains a compatibility projection and must not
  regain ownership of cascade semantics

Milestone T follow-up work must define and enforce cascade fanout ceilings for
document-level style resolution. The expected limit families are:

- maximum stylesheets per style pass
- maximum style rules considered per document
- maximum matched rules per element
- maximum declaration candidates per element
- maximum inline declarations per element
- maximum styled elements per document style pass for fuzz/smoke harnesses

## Property And Computed-Value Invariants

The property system is the boundary between authored declarations and runtime
style data.

Required invariants:

- `PropertyId` and the property registry are the only supported-property
  universe
- property metadata owns inheritance, initial/default values, value kind, and
  invalid-value policy
- supported declarations are parsed into typed specified values before
  computed normalization
- invalid supported values follow `RejectDeclaration`
- unsupported properties and custom properties do not become computed values
- runtime numeric overflow is reported as a normalization error, not converted
  into an arbitrary value
- computed style assembly goes through `ComputedStyleBuilder`
- `ComputedStyle` is immutable and total after construction
- layout and paint must not perform post-hoc CSS parsing or invalid-value
  recovery

Milestone T follow-up work must add hostile-value coverage for:

- malformed color syntax
- oversized numeric literals and runtime length overflow
- unsupported units and keywords
- empty values and trivia-only values
- bad function and block structures
- unsupported future syntax such as `calc(...)`, `var(...)`, CSS-wide
  keywords, and custom-property substitution

## Failure Modes

CSS failures fall into three categories.

Recoverable malformed-input outcomes:

- syntax diagnostics with deterministic recovery
- invalid or unsupported selector parse results
- invalid or unsupported declaration/property/value classification
- no-match selector outcomes
- discarded cascade candidates
- rejected specified values
- computed-value normalization errors that cause declaration rejection or a
  typed hardening failure in tests

Resource-limit outcomes:

- input truncation
- tokenization stop at token budget
- parser stop at rule/declaration/depth budget
- bounded diagnostic collection
- deterministic selector, matching, cascade, or style-pass termination once
  follow-up Milestone T limits are implemented

Unrecoverable engine failures:

- invalid public API usage after an object enters a terminal state
- foreign spans or structurally impossible IR created outside normal parsers
- contradictory DOM adapter facts such as cycles in a selector-facing tree
- invariant-check failures that indicate a Borrowser bug rather than malformed
  CSS
- process-level allocation failure

Forbidden malformed-input outcomes:

- panic, abort, or assertion failure from CSS text alone
- infinite loops or non-advancing recovery
- unbounded recursion
- unbounded diagnostics or snapshots
- nondeterministic parse, match, cascade, or computed-style output
- invalid selectors matching elements
- invalid values reaching `ComputedStyle`

## Determinism Requirements

Determinism is a hardening requirement, not only a testing convenience.

The following must be deterministic for fixed inputs:

- tokenization results, diagnostics, stats, and snapshots
- syntax parse results, diagnostics, stats, and snapshots
- model conversion and selector parse result storage
- selector specificity and matching outcomes
- selector DOM snapshots for equivalent DOM projections
- cascade candidate ordering, winner resolution, resolved-style output, and
  snapshots
- specified-value parsing, computed normalization, computed-style assembly, and
  snapshots
- fuzz harness seed derivation, chunk plans, failure artifacts, and replay
  commands once CSS fuzz targets are added

The CSS pipeline must not depend on:

- hash-map iteration order for externally observable results
- wall-clock time
- thread scheduling order
- allocator addresses
- Rust derived `Debug` output as a golden oracle
- random seeds that are not logged or reproducible

CI and fuzz smoke failures must print enough information to reproduce the same
input locally, including target name, seed, corpus directories, artifact path
when available, and the direct replay command.

Stable debug snapshots, replay artifacts, and fuzz failure summaries must have
an explicit truncation or summarization policy once bounded output limits are
reached. They must not attempt to serialize arbitrarily large internal state in
full.

## Resource-Bound Expectations

The resource limit policy for CSS is:

- every limit must have one owning type or configuration surface
- every limit must identify its enforcement unit explicitly, such as per parse
  operation, per stylesheet, per selector list, per selector match attempt, per
  element, or per document style pass
- defaults must be documented near the owning contract
- limit hits must be observable through a typed diagnostic, stats field,
  termination reason, or stable debug label
- recovery after a limit hit must be deterministic
- fuzz and unit tests must be able to lower limits to exercise each branch
- new CSS features that add untrusted-input loops must add or justify limits in
  the same change

Limits must not be documented only as global concepts without an owning
enforcement scope. Limit enforcement should happen before untrusted input can
induce unbounded or superlinear intermediate storage growth.

Existing syntax limits live in `SyntaxLimits`. Milestone T must extend the same
discipline across selector parsing, selector matching, cascade fanout,
property/value handling, document-level style computation, fuzz harnesses, and
CI smoke scripts.

Limit enforcement should prefer preserving engine integrity over authored CSS
fidelity. Once input exceeds policy, it is acceptable to ignore later
constructs, drop excess diagnostics, stop matching additional candidates, or
fail a fuzz harness with a typed resource-limit result, as long as the behavior
is deterministic and documented.

## Milestone T Scope

In scope for Milestone T:

- CSS hardening strategy and threat model
- explicit resource limits across CSS syntax, selectors, matching, cascade,
  values, and document style computation
- panic-free malformed-input regression tests
- CSS fuzz targets for syntax/model parsing, selector parsing/matching, and
  property/value handling
- committed CSS seed corpus and regression corpus directories
- deterministic replay tests for committed CSS corpus and regressions
- CI smoke scripts with fixed seeds, fixed budgets, artifact logging, and
  reproduction commands
- documentation updates for hardening assumptions, limits, and triage workflow

Out of scope for Milestone T:

- full CSS Syntax, Selectors, Cascade, Values, or CSSOM spec completion
- selector matching optimization beyond what is necessary to enforce bounds
- incremental style invalidation and caching architecture
- layout-dependent percentage/relative-unit resolution
- JavaScript, DOM mutation APIs, style recalculation scheduling, and origin
  security policy

## Contributor Rules

When changing CSS code after T1:

1. Treat malformed CSS, unsupported CSS, and adversarial DOM shape as expected
   inputs.
2. Do not introduce `unwrap`, `expect`, `assert`, or `unreachable` paths that
   can be reached from malformed CSS through public entry points.
3. Add an explicit limit before adding any new untrusted-input loop,
   recursion, traversal, or diagnostic collection path.
4. Identify the enforcement unit for every new hardening limit.
5. Do not introduce new superlinear fanout or repeated traversal behavior
   without a documented bound, justification, and targeted regression coverage.
6. Preserve `Parsed | Unsupported | Invalid` selector distinctions.
7. Preserve supported/unsupported/invalid declaration distinctions before
   cascade.
8. Keep invalid values out of `ComputedStyle`.
9. Extend stable snapshots or corpus fixtures when behavior changes
   intentionally.
10. Promote every minimized hardening regression into the matching committed
   regression corpus before closing the bug.

T1 is complete when future Milestone T issues can answer these questions
without rediscovering policy:

- what CSS inputs are considered hostile?
- which CSS layers are attack surfaces?
- what invariants must never be violated by malformed CSS?
- which failures are recoverable, resource-limit outcomes, or engine bugs?
- what makes a CSS hardening failure reproducible?
- where must new resource limits be added as the pipeline evolves?
