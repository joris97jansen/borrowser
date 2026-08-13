# AF3: Selector Specificity Contract

Last updated: 2026-08-12
Status: reconciled and implemented

This document is the Milestone AF3 contract for selector specificity. AF3
reconciles the existing Milestone P4 specificity implementation with the
Milestone AF architecture and the Q6 selector-matching and R2 cascade
handoffs. It does not introduce a second specificity model.

## Ownership

`css::selectors` owns selector specificity semantics. Specificity is derived
from the typed selector IR under `crates/css/src/selectors` and is independent
of DOM matching traversal, declaration parsing, cascade winner resolution, and
computed-style construction.

The authoritative production path is:

```text
selector parse
  -> typed selector IR
  -> selector-specific specificity
  -> selector-list matching
  -> MatchedSelector specificity
  -> SelectorListMatchOutcome::highest_specificity()
  -> CascadeRuleMatch
  -> CascadeRuleContext
  -> CascadeSpecificity / CascadePriority
  -> cascade winner resolution
```

Cascade consumes selector-produced specificity. It does not reconstruct
specificity from selector structure, source text, declaration order, or parser
vector ordering. Browser/runtime, Layout, and Paint do not own or fabricate
selector specificity.

## Current tuple semantics

The typed `Specificity` value represents the bounded selector tuple `(A, B, C)`:

- A counts ID selectors;
- B counts class selectors and supported attribute selectors;
- C counts named type selectors.

Universal selectors contribute zero. The supported combinators—descendant,
child, next-sibling, and subsequent-sibling—do not directly contribute to any
component. Specificity is additive across the compounds of one complex
selector.

Pseudo-classes and pseudo-elements currently contribute nothing because they
are unsupported selector features in AF2. They are not supported selectors
with zero specificity. Parser-produced unsupported pseudo-class and
pseudo-element selector lists remain non-matchable and cannot contribute
cascade candidates.

## Comparison and bounded representation

`Specificity` stores private `u16` components for A, B, and C. Its derived
ordering is intentionally lexicographic A → B → C. Therefore an ID component
outranks any number of class or type components, a class component outranks
any number of type components when A is equal, and C is compared only after A
and B are equal.

Specificity addition uses saturating arithmetic. Components clamp at
`u16::MAX` rather than wrapping. This keeps hostile or unusually large
supported selector inputs deterministic and prevents arithmetic overflow from
reversing precedence.

The public typed constructor and accessors remain part of the CSS crate's
structured API. Public construction of a `Specificity` value does not create
an alternative production calculation path: selector IR remains the
authoritative source for specificity used by matching and cascade. Downstream
subsystems must consume selector-produced values rather than calculate their
own values.

## Selector-list semantics

A selector list has no intrinsic aggregate specificity. Each
`ComplexSelector` in a parsed `SelectorList` has its own specificity.

When matching one selector list against one element, only selectors that
actually match produce `MatchedSelector` entries. The effective specificity is
the greatest specificity among those actual matches. An unmatched selector
with higher specificity does not affect the result.

Invalid and unsupported selector lists expose no usable matching specificity.
They produce explicit non-matchable outcomes, contain no matched selectors,
and are rejected before cascade candidate construction.

## Debug and regression contract

Specificity is exposed through deterministic CSS-owned debug surfaces:

- selector snapshots show specificity per selector and compound;
- selector-match snapshots show actual matched selector entries and the
  effective highest specificity;
- cascade snapshots show selector-derived specificity on rule inputs,
  candidates, and winners.

These serializers are explicit snapshot contracts. Rust's derived `Debug`
format is not the permanent selector or cascade diagnostic format.

## Computation lifecycle

AF3 keeps specificity as an inexpensive, deterministic derived property of
the immutable semantic selector IR. The parsed AST remains the semantic source
of truth, and AF3 does not cache specificity in `ComplexSelector` or duplicate
it as mutable or potentially stale state.

A future compiled-selector representation may retain derived matching metadata,
including:

- precomputed specificity;
- rightmost-selector lookup keys;
- class, ID, and attribute dependencies;
- fast-rejection metadata;
- combinator traversal requirements;
- selector invalidation dependencies.

That belongs in a future selector compilation/indexing architecture, not in
the AF3 parsed selector AST.

Future special pseudo-class specificity rules also belong inside
`css::selectors` when those pseudo-classes are deliberately supported. AF3
does not expand pseudo-class or pseudo-element coverage.

## Deliberate exclusions

AF3 does not implement:

- pseudo-class or pseudo-element support;
- namespace-selector expansion;
- additional selector matching features;
- cascade layers;
- animations or transitions;
- CSSOM or JavaScript-facing style APIs;
- computed-style changes unrelated to the specificity handoff;
- a replacement selector AST;
- a parallel cascade-owned specificity implementation.

The historical P4 document remains the record of the original specificity
implementation. This AF3 document records its current architectural ownership,
handoff, and hardening status.
