# HTML parser semantic-completeness contract

Status: implemented production contract consumed by AG5

`html::parse_document(input, HtmlParseOptions)` returns a `ParseOutput` whose
`semantic_completeness` states whether the parser-created tree is a complete
downstream semantic input. This is an HTML-owned output-integrity contract for
all consumers; it is not an AG or CSS testing hook.

## Four independent states

1. Ordinary authored parse errors are recoverable HTML parsing behavior. They
   do not make the tree semantically incomplete.
2. Auxiliary diagnostic retention is separately bounded. In particular,
   `errors_dropped` means only that the legacy parse-error deque did not retain
   every diagnostic; it says nothing about tree completeness.
3. A production guardrail that actually suppresses, truncates, or substitutes
   downstream-observable parser facts records a typed
   `HtmlParseSemanticDegradationReason` at that decision. The result is
   `HtmlParseSemanticCompleteness::Degraded`.
4. Fatal reservation, patch-validation, parser-invariant, and integrity errors
   remain typed parser execution errors. They are not downgraded to semantic
   degradation.

`HtmlParseCounters` and the parser's internal counters remain instrumentation.
Consumers must never infer semantic completeness from counters, diagnostic
counts, debug output, or whether an auxiliary error buffer filled.

## Bounded deterministic representation

The degradation set is a fixed non-zero bit set. Every reason has an explicit,
exhaustive reason-to-bit mapping; enum declaration order and discriminant casts
are not part of the representation. Repeated activation is idempotent and
iteration is canonical and independent of activation order. The set retains no
per-activation history and cannot allocate.
Public consumers use typed `contains(reason)`, `len()`, and canonical
`reasons()` iteration; bit positions and enum discriminants remain private.

The current audited classifications are:

| production condition | semantic classification |
| --- | --- |
| token batch capacity/cooperative yield | preserving |
| tag name truncation | degrading |
| attribute name/value truncation | degrading |
| attribute dropped by per-tag count | degrading |
| comment truncation | degrading |
| processing-instruction target suppression/data truncation | degrading |
| bounded doctype recovery | degrading |
| bounded text-mode end-tag matching | degrading |
| bounded numeric character-reference recovery | degrading |
| open-element depth suppression | degrading |
| node creation or child insertion suppression | degrading |
| template-mode depth suppression | degrading |
| tokenizer stall recovery | degrading |
| diagnostic deque/capture capacity | auxiliary diagnostic only |
| fallible reservation, patch validation, parser invariant failure | fatal |

The resource-limit and guardrail enums are matched exhaustively. Adding a new
production guardrail therefore requires an explicit classification decision.
Merely reaching a configured threshold does not degrade the result: recording
occurs only where production actually applies its suppressing, truncating, or
substituting recovery behavior.

Open-elements depth and template insertion-mode depth have separate production
limits so their distinct recovery decisions are independently observable.
Comment closing syntax may move the tokenizer cursor beyond the retained body;
that threshold contact is not recorded as comment truncation unless comment
data is actually suppressed.

This contract preserves existing recovery algorithms. It only makes their
effect on downstream semantic input explicit.

## Downstream use

A consumer first handles the parser's fatal `Result` boundary, then inspects
`ParseOutput::semantic_completeness`. A degraded tree may still be useful for
diagnostics but cannot be treated as complete conformance input. Ordinary parse
errors and dropped auxiliary diagnostics do not prevent normal downstream use.

AG5 follows exactly this boundary before resolving fixture targets or invoking
CSS. It never compares HTML degradation as a CSS semantic expectation.
