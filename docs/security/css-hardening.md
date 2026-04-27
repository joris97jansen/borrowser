# CSS Hardening

Last updated: 2026-04-27  
Status: implemented through Milestone T

This document defines Borrowser's implemented CSS hardening posture for
untrusted input, the limits currently enforced across the CSS pipeline, and the
expected contributor workflow for fuzzing, regression preservation, failure
reproduction, and triage.

The CSS implementation discussed here lives under:

- `crates/css/src/syntax/`
- `crates/css/src/model/`
- `crates/css/src/selectors/`
- `crates/css/src/cascade/`
- `crates/css/src/specified/`
- `crates/css/src/computed/`

The stylesheet runtime integration lives under:

- `crates/runtime_css/src/`
- `crates/browser/src/`

This is the operational hardening guide. For the Milestone T threat model and
strategy contract, see
[`docs/css/t1-css-hardening-strategy-threat-model.md`](../css/t1-css-hardening-strategy-threat-model.md).

Related CSS contracts:

- [`docs/css/n7-resource-limits-parser-invariants.md`](../css/n7-resource-limits-parser-invariants.md)
- [`docs/css/q8-selector-matching-invariants-extension-hooks.md`](../css/q8-selector-matching-invariants-extension-hooks.md)
- [`docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`](../css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md)
- [`docs/css/s9-property-system-computed-style-runtime-contract.md`](../css/s9-property-system-computed-style-runtime-contract.md)

Related HTML hardening reference:

- [`docs/security/html5-tokenizer-hardening.md`](./html5-tokenizer-hardening.md)

## Threat Model

Borrowser must treat the CSS pipeline as a hostile-input surface.

Inputs in scope:

- external stylesheet bytes fetched through runtime/networking
- decoded stylesheet text handed to CSS parser entry points
- inline `style` attribute text
- selector preludes and declaration values
- malformed or adversarial DOM structure exposed through selector adapters
- large stylesheet sets or deep/wide DOMs that amplify selector/cascade work

Security goals:

- no panics from malformed CSS when documented API contracts are respected
- no unbounded parsing, selector matching, cascade, or value normalization work
- deterministic recovery once a limit or guardrail is hit
- explicit typed failures on authoritative engine paths
- reproducible fuzz, regression, and CI failures

Non-goals / out of scope:

- making internal API misuse silently recoverable
- converting impossible invariant breaches into ordinary parse errors
- recovering from process-level out-of-memory or OS resource exhaustion
- implementing unsupported CSS standards surface as part of hardening

## What "Panic-Free" Means

For Borrowser's CSS pipeline, "panic-free" means:

- malformed stylesheets, selectors, declaration values, and DOM-driven selector
  cases must not panic public CSS entry points
- invalid UTF-8 decoded through fuzz harnesses must not reveal crash paths in
  tokenizer, parser, selector, cascade, specified-value, or computed-style code
- deterministic corpus replay, promoted regression replay, and CI smoke lanes
  should not discover panic paths from attacker-controlled input alone

It does **not** mean:

- internal API misuse can never panic
- impossible engine invariant breaches are treated as ordinary authored-input
  errors

Compatibility degradation is allowed only on explicitly named bridge paths such
as legacy DOM projection helpers. Authoritative engine paths keep typed
`Result`-based failures.

## Implemented Hardening Boundaries

The current CSS hardening staircase is:

1. `css::syntax`
   - decoded-text entry points
   - deterministic tokenization and parsing
   - bounded diagnostics and stable syntax snapshots
2. `css::model`
   - structured stylesheet/rule/declaration/value model construction
   - explicit invalid/unsupported selector preservation
3. `css::selectors`
   - selector parsing under syntax-owned ceilings
   - selector matching with explicit axis-step budgets
4. `css::cascade`
   - bounded per-style-pass rule/declaration/element accounting
   - typed style-resolution limit and invariant failures
5. `css::specified` and `css::computed`
   - bounded property-aware specified-value parsing
   - deterministic computed normalization and total computed-style assembly
6. fuzz, regression, and CI tooling
   - deterministic replay, stable summaries, promoted regression fixtures, and
     fixed-budget smoke jobs

Downstream systems such as layout and paint consume `ComputedStyle` and
`StyledNode`. They are not CSS recovery owners.

## Enforced Limits

Limit ownership is explicit. Contributors must adjust the central limit
structs, not add one-off caps in local handlers.

### Syntax Limits

Owned by `SyntaxLimits` in
`crates/css/src/syntax/options.rs`.

| Limit field | Default | Recovery posture |
| --- | ---: | --- |
| `max_stylesheet_input_bytes` | `4 * 1024 * 1024` | Whole-stylesheet text is bounded before tokenization |
| `max_declaration_list_input_bytes` | `64 * 1024` | Inline style text is bounded before tokenization |
| `max_lexical_tokens` | `262_144` | Tokenization stops before unbounded token growth |
| `max_rules` | `16_384` | Structured parsing stops before unbounded rule growth |
| `max_selectors_per_rule` | `256` | Selector-list fanout stays bounded |
| `max_selector_component_values` | `4_096` | Selector prelude complexity stays bounded |
| `max_selector_segments_per_selector` | `128` | Selector structural depth stays bounded |
| `max_simple_selectors_per_compound` | `128` | Compound selector fanout stays bounded |
| `max_declarations_per_rule` | `1_024` | Declaration-list growth stays bounded |
| `max_component_values_per_container` | `4_096` | Function/block component fanout stays bounded |
| `max_component_nesting_depth` | `256` | Parser recursion depth stays bounded |
| `max_diagnostics` | `128` | Diagnostic storage is bounded |

Limit hits surface through `hit_limit` and typed diagnostics where diagnostics
are enabled and capacity remains.

### Selector Matching Limits

Owned by `SelectorMatchingLimits` in
`crates/css/src/selectors/matching/context.rs`.

| Limit field | Default | Recovery posture |
| --- | ---: | --- |
| `max_axis_steps_per_match` | `65_536` | Authoritative match APIs return `SelectorMatchingLimitError::AxisStepLimitExceeded` |

This budget bounds aggregate ancestor/sibling traversal work for one selector
match attempt. Authoritative selector and cascade paths preserve this as a
typed error. Conservative compatibility helpers may intentionally degrade it to
non-matchable outcomes.

### Style Resolution Limits

Owned by `StyleResolutionLimits` in
`crates/css/src/cascade/integration.rs`.

| Limit field | Default | Recovery posture |
| --- | ---: | --- |
| `max_stylesheets_per_style_pass` | `4_096` | Style resolution returns `StyleResolutionError::LimitExceeded` |
| `max_style_rules_per_document` | `262_144` | Style resolution returns `StyleResolutionError::LimitExceeded` |
| `max_matched_rules_per_element` | `4_096` | Style resolution returns `StyleResolutionError::LimitExceeded` |
| `max_declaration_inputs_per_element` | `65_536` | Style resolution returns `StyleResolutionError::LimitExceeded` |
| `max_inline_style_bytes` | `64 * 1024` | Inline style parsing is rejected on the authoritative path |
| `max_inline_declarations_per_element` | `1_024` | Style resolution returns `StyleResolutionError::LimitExceeded` |
| `max_styled_elements_per_document` | `1_000_000` | Style resolution rejects oversized documents before building selector index |
| `selector_matching` | `SelectorMatchingLimits::default()` | Selector traversal budget is enforced during style resolution |

`StyleResolutionError::UnsupportedConfiguration` rejects unrepresentable limit
settings early instead of saturating internal ordering identities.

### Specified Value Limits

Owned by `SpecifiedValueLimits` in
`crates/css/src/specified/parse.rs`.

| Limit field | Default | Recovery posture |
| --- | ---: | --- |
| `max_components_per_value` | `4_096` | Specified-value parsing returns `ResourceLimitExceeded` |

The syntax layer already bounds nested component structure. This limit prevents
property-local parsing from assuming unbounded top-level value fanout.

## Invariants Contributors Must Preserve

### Syntax And Model

- decoded stylesheet and inline-style entry points remain deterministic
- token streams consumed by structured parsing remain source-bound, monotonic,
  and EOF-terminated
- malformed syntax recovers only at documented structural boundaries
- model construction must not reparse raw CSS strings
- invalid and unsupported selectors remain explicit model data
- stable debug output must use versioned serializers, not incidental Rust
  `Debug`

### Selectors

- selector matching consumes selector IR, not reparsed selector text
- invalid and unsupported selector states remain explicit and non-matchable
- authoritative selector matching preserves `Result`-based limit failures
- complex selector evaluation remains observationally equivalent to the current
  right-to-left semantics
- DOM adapters expose deterministic parent/sibling/name/attribute facts only

### Cascade

- `resolve_document_styles(...)` remains an authoritative fallible path
- style resolution does not degrade hardening failures into fabricated normal
  results
- winner resolution remains deterministic for equivalent stylesheet order,
  selector outcomes, and DOM projection
- legacy projection helpers may degrade conservatively, but must stay clearly
  scoped as compatibility surfaces

### Specified And Computed Values

- invalid supported declarations do not become cascade candidates
- computed styles remain total for the supported property subset
- computed normalization does not reparse authored strings
- layout, paint, and browser view code do not implement their own CSS fallback
  logic for supported properties

### Tooling

- fuzz harnesses remain deterministic for fixed bytes and seeds
- promoted regression summaries remain versioned and use stable labels rather
  than Rust enum `Debug` output
- CI smoke failures must print direct repro commands and preserve failure
  artifacts when materialized

## Fuzzing And Deterministic Replay

The CSS hardening workflow has three committed-input surfaces:

1. seed corpus
   - `fuzz/corpus/css_tokenizer/`
   - `fuzz/corpus/css_parser/`
   - `fuzz/corpus/css_selector_parser/`
   - `fuzz/corpus/css_selector_matching/`
   - `fuzz/corpus/css_cascade/`
   - `fuzz/corpus/css_values/`
2. raw fuzz/regression staging inputs
   - matching `fuzz/regressions/css_*` directories
3. promoted permanent regression fixtures
   - `crates/css/tests/regressions/css_fuzz/`

### Replay Committed Fuzz Corpus Deterministically

Normal test-lane replay targets:

```sh
make test-css-tokenizer-fuzz-corpus
make test-css-parser-fuzz-corpus
make test-css-selector-parser-fuzz-corpus
make test-css-selector-matching-fuzz-corpus
make test-css-cascade-fuzz-corpus
make test-css-values-fuzz-corpus
```

These replay the committed corpus through deterministic Rust test harnesses
without requiring libFuzzer.

### Replay Promoted Regression Fixtures

Replay the promoted CSS regression-fixture set:

```sh
make test-css-fuzz-regressions
```

Equivalent direct command:

```sh
cargo test -p css --features css-fuzzing --test css_fuzz_regressions --locked
```

This test validates:

- fixture metadata format and source references
- fixture directory naming
- exact replay through the real tool-specific CSS fuzz harness
- exact `summary.txt` match for every promoted fixture
- at least one promoted regression fixture per CSS fuzz target

### Run Deterministic Smoke Lanes

Local deterministic smoke commands:

```sh
make test-css-tokenizer-fuzz-smoke
make test-css-parser-fuzz-smoke
make test-css-selector-parser-fuzz-smoke
make test-css-selector-matching-fuzz-smoke
make test-css-cascade-fuzz-smoke
make test-css-values-fuzz-smoke
```

Long/manual deterministic lanes:

```sh
make test-css-tokenizer-fuzz-long
make test-css-parser-fuzz-long
make test-css-selector-parser-fuzz-long
make test-css-selector-matching-fuzz-long
make test-css-cascade-fuzz-long
make test-css-values-fuzz-long
```

The smoke scripts live in `tools/ci/` and print:

- fixed seed
- corpus and regression directories
- executed input directories
- failing artifact path when materialized
- direct-binary repro command
- `cargo fuzz run` repro command

## CI Hardening Lanes

GitHub Actions now runs three CSS hardening jobs:

- `css_fuzz_regressions`
- `css_syntax_fuzz_smoke`
- `css_pipeline_fuzz_smoke`

The workflow lives in `.github/workflows/ci.yml`.

Current CI smoke overrides:

| Lane | Seed | Runs | Input timeout | Wall timeout |
| --- | ---: | ---: | ---: | ---: |
| tokenizer | `1873819023` | `128` | `5s` | `90s` |
| parser | `2718281828` | `128` | `5s` | `90s` |
| selector parser | `3141592653` | `128` | `5s` | `90s` |
| selector matching | `1618033988` | `128` | `5s` | `90s` |
| cascade | `1414213562` | `128` | `5s` | `90s` |
| values | `2449489742` | `128` | `5s` | `90s` |

The smoke jobs are intentionally grouped so CI amortizes fuzz-target build
cost:

- syntax job: tokenizer + parser
- downstream job: selector parser + selector matching + cascade + values

## Reproducing Failures

### CI Smoke Failure

When a smoke script fails in CI:

1. download the uploaded artifact bundle if one exists
2. run the logged direct repro command from the job log, or:

```sh
cargo fuzz run <target> <artifact-path>
```

3. if no artifact was materialized, rerun the exact logged command with the
   same input directories and seed

The scripts intentionally log the full executed command and input set so local
reproduction matches CI.

### Promoted Regression Drift

If `css_fuzz_regressions` fails because a committed fixture summary drifted,
the test prints the exact refresh command. The generic form is:

```sh
cargo run -p css --features css-fuzzing --bin css_fuzz_regression_summary -- \
  --tool <tool> \
  --profile <default|selector-limit-zero> \
  --input crates/css/tests/regressions/css_fuzz/<fixture>/input.bin \
  --seed <u64> \
  > crates/css/tests/regressions/css_fuzz/<fixture>/summary.txt
```

Do not hand-edit `summary.txt`. Regenerate it from the real renderer.

### One-Off Stable Summary Rendering

Helper target:

```sh
make print-css-fuzz-regression-summary \
  TOOL=css_values \
  INPUT=crates/css/tests/regressions/css_fuzz/<fixture>/input.bin \
  PROFILE=default \
  SEED=0
```

## Promoting A New Regression Fixture

When a fuzz finding should become a permanent regression:

1. keep the raw or minimized reproducer in the matching `fuzz/regressions/css_*`
   directory
2. minimize the input until one guarded behavior remains
3. create a fixture directory under:

```text
crates/css/tests/regressions/css_fuzz/rg-<tool>-<slug>-YYYY-MM-DD/
```

4. add:
   - `meta.txt`
   - `input.bin`
   - `summary.txt`
5. render `summary.txt` with `css_fuzz_regression_summary`
6. run:

```sh
make test-css-fuzz-regressions
```

7. rerun the matching smoke lane

`meta.txt` must include:

- `format`
- `tool`
- `profile`
- `seed`
- `date`
- `issue`
- `guard`
- optional `source`

The promoted regression-fixture README at
`crates/css/tests/regressions/css_fuzz/README.md` is the authoritative format
contract.

## Adjusting Limits Safely

Only change limits through:

- `SyntaxLimits`
- `SelectorMatchingLimits`
- `StyleResolutionLimits`
- `SpecifiedValueLimits`

When adjusting a limit:

1. update the central limit struct or the caller configuration
2. preserve deterministic failure behavior
3. keep authoritative paths fallible instead of degrading them into fabricated
   normal results
4. add or update targeted tests for the limit behavior
5. rerun:

```sh
make test-css-fuzz-regressions
make test-css-tokenizer-fuzz-corpus
make test-css-parser-fuzz-corpus
make test-css-selector-parser-fuzz-corpus
make test-css-selector-matching-fuzz-corpus
make test-css-cascade-fuzz-corpus
make test-css-values-fuzz-corpus
```

6. rerun the relevant smoke lanes
7. update this document and any affected contract docs if the operational
   default changed

Guidance:

- lowering limits is allowed, but expect more lossy recovery and regression
  churn
- raising limits needs a concrete product/runtime reason and should be checked
  against build time, memory, corpus behavior, and CI runtime
- compatibility helpers may degrade conservatively, but new authoritative APIs
  should preserve typed hardening failures

## Contributor Checklist

Before landing CSS hardening-sensitive changes:

- keep malformed input on deterministic recovery paths
- preserve explicit invalid/unsupported/limit-exceeded states
- avoid new unbounded loops, recursion, or fanout
- keep debug output versioned and stable
- preserve exact repro commands in scripts and tests
- update promoted regression fixtures instead of weakening assertions
- keep the HTML and CSS hardening docs aligned on workflow discipline, even if
  their implementation details differ

## Related Files

- strategic CSS hardening contract:
  `docs/css/t1-css-hardening-strategy-threat-model.md`
- syntax hardening contract:
  `docs/css/n7-resource-limits-parser-invariants.md`
- selector matching invariants:
  `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- cascade/runtime handoff contract:
  `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`
- property/computed-style contract:
  `docs/css/s9-property-system-computed-style-runtime-contract.md`
- fuzz overview and smoke budgets:
  `fuzz/README.md`
- promoted CSS regression fixture format:
  `crates/css/tests/regressions/css_fuzz/README.md`
- CI smoke scripts:
  `tools/ci/css_*_fuzz_smoke.sh`
- GitHub Actions workflow:
  `.github/workflows/ci.yml`
