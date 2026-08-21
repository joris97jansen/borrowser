# R2: Introduce Structured Cascade Inputs And Candidate Declaration Model

Last updated: 2026-08-21
Status: contract and code implemented

AF5 reconciliation: a matched stylesheet `CascadeRuleInput` contains a
self-contained `StylesheetRuleRef`, a validated `CascadeRuleContext`, the exact
AF4 `SelectorListMatchOutcome`, and a borrowed
`&[CascadeDeclarationInput]`. It does not borrow an
`ActiveCollectedStyleRule`, expose `RuleCollection` storage, depend on
integration arena types, or own a stylesheet declaration vector. Inline inputs
remain element-local and own their classified declarations. See the AF5
contract for collection lifecycle and storage.

This document is the source-of-truth contract for Milestone R issue 2: the
intermediate rule-input and declaration-candidate structures the Borrowser
cascade engine uses after selector matching and before winner resolution.

R1 defined the high-level cascade architecture and resolved-style contract. R2
fills in the next layer down: the explicit post-match inputs and comparable
declaration candidates the cascade engine will resolve.

Related code:
- `crates/css/src/cascade/contract.rs`
- `crates/css/src/cascade/contract/order.rs`
- `crates/css/src/cascade/contract/rules.rs`
- `crates/css/src/cascade/contract/sources.rs`
- `crates/css/src/cascade/integration/rule_inputs.rs`
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

- collection integration constructs stylesheet rule inputs through the
  contract-owned `CascadeRuleInput::from_stylesheet_match_collected(...)`
- inline style rule identity is explicit through `InlineStyleRuleRef`
- malformed rule/declaration ownership now fails with
  `CascadeRuleInputBuildError` in all builds
- candidate sorting has an engine-owned helper with lawful semantic priority
  comparison

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

Its authoritative production variants are:

- `Stylesheet(MatchedStylesheetRuleInput<'collection>)`;
- `Inline(InlineStyleRuleInput)`.

The stylesheet variant contains:

- a self-contained `StylesheetRuleRef` carrying opaque source identity and raw
  rule provenance;
- a validated stylesheet `CascadeRuleContext`;
- the exact authoritative AF4 `SelectorListMatchOutcome`;
- a borrowed `&'collection [CascadeDeclarationInput]` into the pass-scoped
  collection arena.

The stylesheet variant owns no declaration vector. Collection integration
validates its private active rule, projects the contract data, and calls the
contract-owned constructor. The cascade contract never imports the private
collection rule, range, or arena types.

The inline variant is element-local. It owns the declarations classified from
that element's style attribute, uses `CascadeRuleContext::InlineStyle`, and has
no selector-list match outcome. Inline declarations participate through author
origin, declaration importance, `CascadeSpecificity::InlineStyle`, and checked
`DeclarationOrder`.

### Rule Source

`CascadeRuleSource` identifies where the matched rule came from:

- `Stylesheet(StylesheetRuleRef)`
- `InlineStyle(InlineStyleRuleRef)`

This is rule-level identity only. Declaration-level identity remains explicit
through `CascadeDeclarationSource`.

For stylesheet matches, the authoritative handoff is:

1. collection integration invokes the checked AF4 selector-list matcher once;
2. integration passes `StylesheetRuleRef`, origin, `StylesheetRuleOrder`, the
   exact outcome, and the borrowed declaration slice to
   `CascadeRuleInput::from_stylesheet_match_collected(...)`;
3. the contract validates declaration ownership, derives effective specificity
   from that outcome, and constructs `MatchedStylesheetRuleInput` only when the
   rule contributes candidates;
4. the same input and outcome feed candidate generation and AF5 diagnostics.

For inline styles, integration classifies the element's declaration list and
calls `CascadeRuleInput::from_inline_style_collected(...)` with explicit inline
identity and the owned declarations.

`CascadeRuleInput::new(...)`, `from_stylesheet_match(...)`, and
`from_inline_style(...)` are `cfg(test)` compatibility helpers for direct
cascade-contract tests. They are not production Browser or collection entry
points.

### Rule Context

`CascadeRuleContext` is a typed enum:

```text
Stylesheet {
    origin,
    selector specificity,
    StylesheetRuleOrder,
}

InlineStyle
```

`StylesheetRuleOrder` is the semantic pair of sparse `StylesheetOrder` and
`StyleRulePosition`. `CascadeRuleContext::InlineStyle` supplies author origin,
inline specificity, and `CascadeSourceOrder::InlineStyle` through its closed
variant. The representation therefore cannot pair selector specificity with
inline source order or inline specificity with stylesheet source order.

This is intentionally separate from declaration-level importance because
importance is a declaration property, not a rule property. The final
origin/importance band is still synthesized later, when a declaration becomes a
candidate and declaration-level importance is known.

### Dependency Direction

```text
cascade contract
    ↓
collection integration
```

The cascade contract owns semantic source/order types, rule/declaration source
references, matched-input types, candidate types, and comparison semantics.
Collection integration owns the private flat arenas and checked ranges, imports
the contract, and constructs contract inputs. The dependency never points from
the contract into collection storage.

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
- `InvalidValue(CascadePropertyId)`
- `InvalidShorthandValue(ShorthandId)`
- `Unsupported(String)`
- `Custom(String)`
- `Invalid`

This keeps property-name and supported-value classification explicit without
collapsing invalid supported values, atomic shorthand failures, unsupported
properties, custom properties, and invalid names into one nullable string.

### Applicability

`CascadeDeclarationApplicability` makes declaration filter state explicit:

- `Supported(CascadePropertyId)`
- `InvalidValue(CascadePropertyId)`
- `InvalidShorthandValue(ShorthandId)`
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

- collection integration uses
  `CascadeRuleInput::from_stylesheet_match_collected(...)` and
  `CascadeRuleInput::from_inline_style_collected(...)`, each returning a typed
  result
- every declaration source must belong to the claimed `CascadeRuleSource`
- inline-style ownership is checked against explicit `InlineStyleRuleRef`
- stylesheet context is derived from the exact contributing AF4 match outcome

The generic `CascadeRuleInput::new(...)` and owned-declaration compatibility
constructors are compiled only for contract tests. They exercise the same
ownership validation but are not authoritative production construction paths.

This matters because the rule-input layer is supposed to be a trustworthy
boundary, not a debug-only convention.

## Ordering Contract

Candidate comparison remains grounded in the R1 precedence model:

1. origin/importance band
2. specificity
3. semantic source order
4. declaration order

For stylesheet declarations, `CascadeSourceOrder::Stylesheet` contains a
`StylesheetRuleOrder`, which compares sparse `StylesheetOrder` and then
`StyleRulePosition`. Inline declarations use
`CascadeSourceOrder::InlineStyle`. `DeclarationOrder` breaks ties between
declarations within the same rule input. Raw top-level rule provenance remains
separate and is not a cascade-order key.

R2 adds `CascadeDeclarationCandidateKey` as the deterministic ordering key for
candidate collections, plus `sort_candidates_by_cascade_order(...)` as the
engine-owned ordering helper. Lawful semantic priority comparison, not sort
stability, distinguishes stylesheet and inline priorities.

Sorting by that key:

- groups candidates by property
- orders candidates within a property group by the cascade precedence key

This produces exactly the comparison surface winner resolution needs.

Semantically distinct priorities compare distinctly, so cascade correctness
does not rely on sort stability. Exact duplicate candidate keys preserve
incoming order for the current deterministic degenerate-tie rule; that narrow
behavior is deliberate and covered by tests.

## Determinism Requirements

The R2 candidate/input layer is deterministic by contract:

- declaration input order is preserved exactly from source
- rule context is explicit, not inferred from parser iteration shape
- declaration applicability is explicit and testable
- candidate generation preserves declaration source order for equal rule
  context
- sorting by `sort_candidates_by_cascade_order(...)` is deterministic and
  lawful; exact duplicate keys alone use stable equal-key preservation

## Non-Goals

R2 does not:

- pick winners yet
- compute inherited/default entries
- interpret authored values into computed values
- own or expose collection arena storage (AF5 integration supplies the private
  flat storage and borrowed stylesheet declaration slices)
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
