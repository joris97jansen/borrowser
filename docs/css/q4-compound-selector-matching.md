# Q4: Implement Compound Selector Matching

Last updated: 2026-04-14  
Status: implemented via Q3

This document records the status of Milestone Q issue 4:
implementing compound selector matching on a single element.

Historical note:

- Q4 was closed based on the Q3 implementation
- Q5 later extended the matcher from element-local compound matching to full
  complex-selector traversal

Related code:
- `crates/css/src/selectors/matching.rs`

Related documents:
- `docs/css/q3-simple-selector-matching.md`
- `docs/css/q2-selector-matching-context.md`

## Status

Q4 required:

- compound selector evaluation on a single element
- conjunction across all simple selector components
- deterministic evaluation behavior
- integrated validity handling
- regression coverage for representative compound selector combinations

That runtime behavior is already implemented by the Q3 matcher work.

No additional runtime change is required for Q4.

## Existing Implementation

The load-bearing implementation is:

- `SelectorMatchingContext::matches_compound_selector(...)`

This method evaluates one `CompoundSelector` against one target element by:

- matching the optional type selector on the same element
- requiring every subclass selector to match on the same element
- short-circuiting deterministically on the first failing component

This is exactly the Q4 runtime boundary.

The surrounding selector-list entrypoint is already in place through:

- `SelectorMatchingContext::match_selector_list(...)`
- and its conservative compatibility variant
  `SelectorMatchingContext::match_selector_list_conservative(...)`

That means compound selector matching is already integrated into the current
element-local evaluator surface rather than existing as an isolated helper.

## Validity And Matchability Handling

Q4 does not require separate validity plumbing beyond what Q3 already added.

At Q4 closure time, the matcher surface preserved:

- `Invalid` parse results as `Invalid` match outcomes
- parser-level `Unsupported` selector input as `Unsupported` match outcomes
- the current temporary matcher-level `Unsupported` fallback for parsed
  selectors outside the active evaluator subset

That was sufficient to satisfy Q4’s validity-handling requirement. Q5 later
removed the temporary matcher-level fallback by implementing structural
selector traversal for the supported IR.

## Regression Coverage

Representative Q4 coverage already exists in the selector matcher tests:

- compound selector matches on a single element
- compound selector fails when the type selector fails
- compound selector fails when a subclass selector fails
- selector-list matching reuses the same compound-matching primitive

Those tests were added with Q3 because that issue already introduced the real
compound matcher.

## Conclusion

The correct milestone bookkeeping is:

- Q3 delivered both simple-selector matching and compound-selector conjunction
- Q4 was subsumed by that implementation
- Q4 may be treated as closed without further runtime work

The next meaningful implementation step after Q4 was complex-selector
traversal, not more compound-selector code. That traversal work later landed as
Q5.
