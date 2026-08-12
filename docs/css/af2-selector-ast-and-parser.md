# AF2: Selector AST, Parser, And Selector Diagnostics

Last updated: 2026-08-12
Status: implemented

AF2 completes the Milestone AF selector-parser boundary by reconciling the
existing Milestone P selector AST/parser with the shared stylesheet diagnostic
transport. It does not introduce a second selector AST or expand selector
matching coverage.

## Ownership

`css::syntax` owns generic CSS tokenization, component values, stylesheet
parsing, generic recovery, syntax diagnostics, and syntax limits.

`css::selectors` owns selector-specific structure and semantics:

- selector AST construction and parsing;
- supported and unsupported selector policy;
- invalid-selector classification;
- selector source spans and source order;
- typed selector diagnostic classes and levels;
- typed invalid reasons and unsupported feature categories;
- stable selector labels and selector diagnostic messages;
- selector resource-limit classification;
- selector snapshots.

`css::model` stores `StyleRule::selectors` and mechanically projects
selector-owned diagnostics into the shared `SyntaxDiagnostic` transport. It
does not interpret selector grammar, invalid reasons, or unsupported features.

## Existing Selector Foundation

The permanent selector implementation was introduced by Milestone P under
`crates/css/src/selectors` and includes:

- selector lists;
- complex and compound selectors;
- type and universal selectors;
- ID and class selectors;
- attribute presence and supported comparison selectors;
- descendant, child, next-sibling, and subsequent-sibling combinators;
- explicit `Parsed`, `Unsupported`, and `Invalid` outcomes;
- deterministic spans, specificity hooks, and snapshots.

AF2 preserves that implementation unchanged except for the diagnostic boundary
and canonical label reuse.

## Selector Diagnostic Classification

Before transport projection, `css::selectors` normalizes a parse result into a
typed selector diagnostic descriptor. Its normalized classes are:

- `EmptySelectorList`;
- `InvalidSelector`;
- `UnsupportedSelector`;
- `InvariantViolation`;
- `LimitExceeded`.

The descriptor also carries a selector-owned warning/error level and typed
details. Ordinary authored invalid selectors retain their typed
`InvalidSelectorReason`; unsupported selectors retain ordered
`UnsupportedSelectorFeature` values. Empty-selector-list,
invariant-violation, and resource-limit outcomes are normalized into dedicated
typed detail variants.

The descriptor uses one typed detail state as the source for its normalized
class, level, and stable message. Inconsistent class/detail/level combinations
are therefore not representable. Unsupported selector lists also require at
least one deduplicated unsupported feature. The public
`UnsupportedSelectorList::from_features` constructor returns `None` for an
empty input; all in-repository construction paths handle this explicitly.

The model boundary maps normalized classes and levels mechanically to
`DiagnosticKind` and `DiagnosticSeverity`. It does not derive policy from
individual selector reasons or features.

Internal selector span/IR failures remain `InvariantViolation` diagnostics.
Selector resource exhaustion remains `LimitExceeded`. Authored malformed
selectors remain `InvalidSelector`, and syntactically understood but unsupported
selectors remain `UnsupportedSelector` warnings.

## Diagnostic Ordering And Statistics

Syntax diagnostics retain the exact encounter order emitted by
`css::syntax`. AF2 does not globally sort diagnostics by byte offset.

During model construction, selector diagnostics are projected as each style
rule is traversed. They are appended in deterministic model style-rule/source
order after the syntax diagnostic stream.

The shared diagnostic transport stores a byte offset. Projection uses the
selector descriptor span start, or the containing rule span start when the
selector descriptor has no span.

`ParseStats` remains structurally unchanged. `diagnostics_emitted` counts every
classified diagnostic event even when diagnostics are disabled or storage is
capped by `max_diagnostics`. Selector `LimitExceeded` classifications also set
aggregate parse `hit_limit`.

Selector messages are built only when a diagnostic will be retained. Stable
selector labels are defined once on selector reason/feature types and are
shared by selector snapshots and diagnostic messages. Derived Rust `Debug` is
not a diagnostic or snapshot contract.

## Applicability And Non-Goals

Invalid and unsupported selector lists remain explicitly non-matchable and do
not contribute cascade candidates. Selector lists are not partially salvaged.

AF2 does not implement:

- selector matching changes;
- specificity changes;
- pseudo-classes or pseudo-elements;
- namespace selectors;
- cascade or computed-style changes;
- selector dependency invalidation;
- a lossless unsupported-selector AST;
- CSSOM, JavaScript style APIs, media queries, custom properties, animations,
  or transitions.

`UnsupportedSelectorList` currently retains its source span and normalized
unsupported feature categories, not a lossless unsupported selector tree.
Future support may therefore require reparsing the preserved source/prelude.

## Contract Reconciliation

The original N9 proposal for a second syntax-layer selector AST is superseded
by the later Milestone P architecture. `css::selectors` is the permanent
selector-specific AST/parser owner; `css::syntax` remains the generic syntax
layer. N8, the syntax parser contract, O1, P6, AF1, and the feature-gap tracker
must describe this same boundary.

Validation coverage keeps the phase boundary explicit: the existing selector
parser fuzz corpus protects selector parsing and classification, while AF2's
model unit tests and model golden cover selector diagnostic projection. The
parser corpus is not treated as coverage of the model diagnostic transport.
