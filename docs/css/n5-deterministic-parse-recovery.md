# N5: Define And Implement Deterministic CSS Parse Recovery

Last updated: 2026-04-05  
Status: implemented

Tracker note:
- this is the canonical implemented `N5` issue for Milestone N
- the previously queued selector-structure follow-up was renumbered to `N6`
  when deterministic parse recovery became the active `N5` work item

## Implemented Result

N5 formalized and implemented deterministic recovery behavior for malformed CSS
input in the structured syntax parser.

The parser no longer relies on incidental skipping. Recovery points are now
explicit, progress-safe, bounded, and testable for the supported Milestone N
subset.

## Why This Exists

A browser-grade CSS parser must keep moving after malformed input without
corrupting parser state or silently depending on prototype behavior.

By N4 the syntax layer already had:

- explicit tokenizer output
- structured stylesheet parsing
- explicit compatibility projection

The remaining gap was recovery discipline: malformed rules and declarations
needed explicit resynchronization points so later valid input would still parse
deterministically.

## Delivered Changes

- defined top-level stylesheet recovery points for malformed qualified rules and
  malformed at-rules
- defined declaration-list recovery points for malformed declarations
- made parser progress explicit so recovery always advances or terminates
- preserved later valid rules/declarations after representative malformed input
- added focused regression tests for malformed stylesheet and declaration cases
- documented syntax-layer recovery behavior as part of the contract

## Recovery Points

Current recovery behavior:

1. Top-level stylesheet recovery
   - unexpected top-level `;` is skipped as a single-token recovery point
   - unexpected top-level `}` is skipped as a single-token recovery point
   - malformed qualified rules recover at top-level `;`, top-level `}`, or EOF
   - malformed at-rules recover at `;`, `{`, unexpected `}`, or EOF
2. Declaration-list recovery
   - malformed declarations recover at top-level `;`, declaration-block end
     `}`, or EOF
   - when a deterministic next declaration start exists (`ident ... :` at the
     current block depth), recovery resynchronizes there instead of discarding
     later valid declarations
3. Nested component-value recovery
   - simple blocks and functions remain structurally consumed
   - unmatched nested structures recover at EOF without panicking

## Progress And Safety Invariants

- recovery never rewinds parser state
- each recovery path either advances the token cursor or terminates parsing
- malformed input cannot cause unbounded retry loops
- recovery remains bounded by existing parser limits and tokenizer limits
- diagnostics remain deterministic and ordered by encounter

## Exit Criteria

- recovery behavior for representative malformed inputs is implemented
- invalid CSS is handled through defined recovery rules rather than ad hoc
  skipping
- recovery behavior is deterministic and covered by tests
- parser remains progress-safe and bounded on malformed input
