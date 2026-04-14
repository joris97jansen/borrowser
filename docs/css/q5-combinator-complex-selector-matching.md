# Q5: Implement Combinator And Complex Selector Matching

Last updated: 2026-04-14  
Status: implemented

This document is the source-of-truth contract for Milestone Q issue 5:
implementing structural selector matching for the supported combinators and
full complex-selector evaluation over Borrowser's current selector IR.

Related code:
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/selectors/matching/context.rs`
- `crates/css/src/selectors/matching/result.rs`
- `crates/css/src/selectors/matching/dom_index.rs`
- `crates/css/src/selectors/matching/tests.rs`

Related documents:
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q2-selector-matching-context.md`
- `docs/css/q3-simple-selector-matching.md`
- `docs/css/q4-compound-selector-matching.md`

## Implemented Result

Q5 adds real structural selector evaluation through:

- `SelectorMatchingContext::matches_complex_selector(...)`
- the updated `SelectorMatchingContext::match_selector_list(...)`

The matcher now evaluates the full parsed selector subset supported by
Milestone P:

- universal selectors
- type selectors
- id selectors
- class selectors
- supported attribute selectors
- descendant combinators
- child combinators
- adjacent-sibling combinators
- general-sibling combinators

This means parsed selector lists are now evaluated from the selector IR rather
than being conservatively rejected when combinators are present.

## Evaluation Strategy

Complex selectors are evaluated right-to-left.

For a selector represented as:

- head compound
- followed by source-ordered combined selectors on the right

the matcher:

1. starts at the target element and checks the rightmost compound
2. walks leftward across combinators
3. explores structural candidates using the DOM/query helpers from Q2
4. succeeds only if every compound/combinator requirement in the chain is
   satisfied

This keeps the evaluator aligned with the selector IR while preserving a clear
boundary between:

- compound matching on one element
- structural traversal across element axes

The current implementation keeps that evaluator in a direct recursive form.
That is deliberate for Milestone Q:

- correctness and contract clarity come before traversal optimization
- recursive backtracking keeps structural semantics explicit
- iterative reformulation, pruning, or memoization can be introduced later
  without changing the matcher-facing contract

## Combinator Traversal Rules

Traversal is explicit and deterministic:

- `descendant` explores ancestors nearest-first
- `child` checks only the nearest parent element
- `next-sibling` checks only the nearest previous element sibling
- `subsequent-sibling` explores previous element siblings nearest-first

For descendant and general-sibling combinators, candidate search backtracks as
needed across multiple structural candidates. The matcher does not stop at the
first candidate that matches one compound if that candidate fails the remaining
left-hand side of the selector chain.

These rules are important because they make complex selector matching:

- deterministic across equivalent DOM projections
- independent of incidental DOM storage details
- correct for selectors that require structural backtracking

## Matchability And Validity Handling

Q5 removes the temporary matcher-level fallback introduced during Q3 staging.

Current behavior is:

- `Parsed` selector lists are fully evaluated for the supported selector IR
- parser-level `Unsupported` selector input remains `Unsupported`
- `Invalid` selector input remains `Invalid`

That means matcher-level `Unsupported` no longer has a second “parsed but not
yet evaluable” origin for the current supported selector subset.

## Tree Assumptions

The current matcher assumes selector evaluation runs over an acyclic DOM tree.

That is an acceptable engine contract for Milestone Q because:

- `SelectorMatchDom` exposes only parent and previous-element-sibling axes
- Borrowser's owned `SelectorDomIndex` adapter projects `html::Node` into a
  deterministic tree-shaped selector view
- complex matching therefore does not need separate cycle detection or depth
  guards for the current DOM providers

Future DOM providers must preserve those selector-facing tree invariants.

## Determinism Requirements

Q5 matching is deterministic by contract:

- selector lists are evaluated in source order
- match results are recorded by authoritative `selector_index`
- specificity comes from selector IR rather than ad hoc recomputation
- ancestor and sibling candidate search order is explicit and tested
- equivalent DOM constructions that expose the same element axes produce the
  same match outcome
- equivalent raw selector formatting produces the same match outcome once
  parsed into IR

## Implementation Organization

Q5 is also the point where the matcher split along the boundaries documented in
earlier Q issues:

- `matching/result.rs` owns matchability and result surfaces
- `matching/context.rs` owns the DOM contract, matcher-facing context, and
  selector evaluator
- `matching/dom_index.rs` owns the deterministic owned-tree DOM adapter
- `matching/tests.rs` owns the regression surface

This prevents full structural evaluation logic from accumulating in one file
after the “split when complex matching lands” trigger was reached.

## Performance Posture

Q5 intentionally does not add selector-matching optimizations yet.

In particular, the current matcher does not:

- add early rejection heuristics for descendant or sibling search
- cache partial structural match results
- integrate bloom filters, ancestor summaries, or style-sharing shortcuts

That is deliberate. Milestone Q is establishing correct and explicit matching
semantics first so later optimization work can build on a stable contract
rather than changing the meaning of selector matching.

## Regression Coverage

Q5 adds regression coverage for:

- each supported combinator class
- representative multi-combinator selectors
- right-to-left structural backtracking across ancestors and siblings
- selector-list matching with complex selectors and stable source-order results
- equivalent DOM-construction paths
- equivalent raw parse formatting

## Non-Goals

Q5 does not:

- integrate final cascade winner resolution with the new matcher yet
- add caching or invalidation
- add selector classes outside the current supported parser/model subset
- optimize the recursive structural matcher beyond what is needed for clear and
  deterministic semantics

Those remain later Milestone Q work. The next meaningful consumer of this
matcher is cascade integration, not additional ad hoc selector-matching churn.
