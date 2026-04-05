# N7: Add Resource Limits And Parser Invariants

Last updated: 2026-04-05  
Status: implemented

## Implemented Result

N7 hardened the CSS syntax layer with explicit operational ceilings and
tokenizer/parser invariant checks so malformed or hostile input remains bounded
and deterministic.

The syntax layer now enforces:

- origin-specific input byte limits
- lexical token count limits
- rule and declaration count limits
- component nesting depth limits for structured parser recursion
- bounded diagnostic collection
- tokenizer-to-parser token stream invariants before parsing begins

## Why This Exists

Milestone N already established deterministic tokenization, structured parsing,
recovery behavior, and stable snapshots. N7 adds the guardrails needed to make
that subsystem safe under oversized or adversarial input.

This issue exists so the syntax layer:

- does not allocate or recurse without bound
- does not depend on implicit parser progress
- does not assume tokenizer output is always structurally valid
- is ready for later fuzzing and deeper hardening work

## Delivered Changes

- added `max_lexical_tokens` to bound tokenizer growth independently of input
  byte ceilings
- added `max_component_nesting_depth` to prevent unbounded recursive descent in
  nested blocks/functions
- made tokenizer stop deterministically at token-count limits with
  `limit-exceeded`
- added parser-side token stream invariant validation before structured parsing
- introduced `invariant-violation` diagnostics for broken tokenizer/parser
  transition assumptions
- added progress guards so declaration recovery cannot remain stuck on a
  non-advancing cursor
- added tests for token-count limits, nesting-depth limits, and invariant
  validation

## Hardening Contract

The syntax layer now guarantees:

- malformed or oversized CSS does not cause uncontrolled tokenizer growth
- malformed or oversized CSS does not cause unbounded parser recursion
- parser entry points validate that token streams are source-bound, monotonic,
  and EOF-terminated before consuming them
- structured parser entry points share one canonical tokenizer-to-parser
  invariant validator rather than maintaining separate boundary checks
- recovery either advances the parse cursor or terminates parsing
- limit hits are explicit through `hit_limit` and typed diagnostics

## Exit Criteria

- syntax-layer resource limits are implemented
- core tokenizer/parser invariants are enforced
- malformed or oversized inputs do not trigger uncontrolled behavior
- hardening behavior is covered by tests
- the syntax layer is ready for later fuzzing/hardening milestones
