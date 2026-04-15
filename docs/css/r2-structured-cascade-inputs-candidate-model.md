# R2: Introduce Structured Cascade Inputs And Candidate Declaration Model

Last updated: 2026-04-15  
Status: contract and code implemented

This document is the source-of-truth contract for Milestone R issue 2: the
intermediate rule-input and declaration-candidate structures the Borrowser
cascade engine uses after selector matching and before winner resolution.

R1 defined the high-level cascade architecture and resolved-style contract. R2
fills in the next layer down: the explicit post-match inputs and comparable
declaration candidates the cascade engine will resolve.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/model/mod.rs`

Related documents:
- `docs/css/r1-cascade-architecture-style-resolution-contract.md`
- `docs/css/q6-validity-specificity-match-results.md`
- `docs/css/q8-selector-matching-invariants-extension-hooks.md`

## Implemented Result

Milestone R now has explicit code-level structures for:

- matched rule inputs entering cascade
- rule-level origin/specificity/order context
- declaration-level applicability state
- declaration-level importance and source order
- supported declaration candidates ready for winner comparison
- deterministic candidate ordering keys

This removes the need for cascade comparison logic to depend on incidental
ordering in parser output vectors or DOM-attached style mutation paths.

R2 also hardens the construction boundary:

- stylesheet rule inputs have a canonical constructor from `CascadeRuleMatch`
- inline style rule identity is explicit through `InlineStyleRuleRef`
- malformed rule/declaration ownership now fails with
  `CascadeRuleInputBuildError` in all builds
- candidate sorting has an engine-owned stable helper for equal-key behavior

## Why This Exists

R1 already established:

- the selector-to-cascade handoff shape
- the precedence key shape
- the resolved-style output contract

That was necessary, but not sufficient for implementation. The cascade engine
still needed an explicit intermediate layer answering:

- What is one matched rule as far as cascade is concerned?
- What is one declaration before it becomes a candidate?
- Which declaration states produce candidates and which do not?
- Where do declaration order and `!important` live?

R2 answers those questions in code so later winner-resolution work can be
implemented against one deterministic model instead of reconstructing these
facts ad hoc.

## Rule Input Model

The post-match rule input is `CascadeRuleInput`.

It contains:

- `CascadeRuleSource`
- `CascadeRuleContext`
- ordered `CascadeDeclarationInput` values

### Rule Source

`CascadeRuleSource` identifies where the matched rule came from:

- `Stylesheet(StylesheetRuleRef)`
- `InlineStyle(InlineStyleRuleRef)`

This is rule-level identity only. Declaration-level identity remains explicit
through `CascadeDeclarationSource`.

For stylesheet matches, the canonical handoff is:

1. selector matching produces `CascadeRuleMatch`
2. `CascadeRuleInput::from_stylesheet_match(...)` validates that the match
   contributes to cascade and derives the `CascadeRuleContext`
3. the resulting `CascadeRuleInput` becomes the single rule-level input to
   candidate generation

For inline styles, `CascadeRuleInput::from_inline_style(...)` provides the
equivalent canonical constructor with explicit inline-rule identity.

### Rule Context

`CascadeRuleContext` carries the rule-level metadata shared by every declaration
input in that matched rule:

- `origin`
- `specificity`
- `rule_order`

This is intentionally separate from declaration-level importance because
importance is a declaration property, not a rule property. The final
origin/importance band is still synthesized later, when a declaration becomes a
candidate and declaration-level importance is known.

## Declaration Input Model

Each declaration entering cascade is represented by `CascadeDeclarationInput`.

It carries:

- declaration source identity
- declaration order within the rule or inline style attribute
- declaration-level importance
- structured property identity
- applicability state
- structured authored value

### Property Identity

`CascadeDeclarationInput` now preserves property identity through
`CascadeDeclarationProperty` rather than a loose `Option<String>`.

That surface distinguishes:

- `Supported(CascadePropertyId)`
- `Unsupported(String)`
- `Custom(String)`
- `Invalid`

This keeps property-name semantics explicit without entangling them with
computed-style parsing or collapsing multiple states into one nullable string.

### Applicability

`CascadeDeclarationApplicability` makes declaration filter state explicit:

- `Supported(CascadePropertyId)`
- `UnsupportedProperty`
- `CustomProperty`
- `InvalidPropertyName`

Only `Supported(...)` declarations generate winner-resolution candidates.
Everything else remains visible on the input surface for tests and debugging,
but is filtered before comparison.

This is important because cascade should not silently lose track of why a
declaration did not participate.

## Candidate Model

Supported declarations become `CascadeDeclarationCandidate`.

Each candidate carries:

- resolved supported property id
- declaration source identity
- fully materialized `CascadePriority`
- structured authored value

The transition from declaration input to candidate is deterministic:

1. take one `CascadeDeclarationInput`
2. require `CascadeDeclarationApplicability::Supported(property)`
3. combine rule context + declaration importance + declaration order into
   `CascadePriority`
4. emit one `CascadeDeclarationCandidate`

This means winner resolution later only compares candidate objects and no
longer needs to infer missing metadata from surrounding storage.

## Construction Invariants

`CascadeRuleInput` is no longer a soft contract.

- `CascadeRuleInput::new(...)` returns `Result<_, CascadeRuleInputBuildError>`
- every declaration source must belong to the claimed `CascadeRuleSource`
- inline-style ownership is checked against explicit `InlineStyleRuleRef`

This matters because the rule-input layer is supposed to be a trustworthy
boundary, not a debug-only convention.

## Ordering Contract

Candidate comparison remains grounded in the R1 precedence model:

1. origin/importance band
2. specificity
3. rule order
4. declaration order

R2 adds `CascadeDeclarationCandidateKey` as the deterministic ordering key for
candidate collections, plus `sort_candidates_by_cascade_order(...)` as the
engine-owned stable sort helper.

Sorting by that key:

- groups candidates by property
- orders candidates within a property group by the cascade precedence key

This produces exactly the comparison surface winner resolution needs.

Equal candidate keys preserve incoming order by contract. That behavior is now
deliberate and covered by tests instead of being left as an incidental detail
of caller-chosen sorting code.

## Determinism Requirements

The R2 candidate/input layer is deterministic by contract:

- declaration input order is preserved exactly from source
- rule context is explicit, not inferred from parser iteration shape
- declaration applicability is explicit and testable
- candidate generation preserves declaration source order for equal rule
  context
- sorting by `sort_candidates_by_cascade_order(...)` is deterministic and
  stable, including equal-key preservation

## Non-Goals

R2 does not:

- pick winners yet
- compute inherited/default entries
- interpret authored values into computed values
- optimize storage for performance
- remove the legacy `attach_styles` bridge

It exists strictly to make the next cascade steps explicit and testable.

## Exit Condition For This Issue

This issue is complete when the cascade engine can answer the following in code
without implicit parser or DOM ordering assumptions:

- What is one matched rule input?
- What is one declaration input?
- Which declarations generate candidates?
- How are candidates compared deterministically?

That contract now exists and is covered by unit tests.
