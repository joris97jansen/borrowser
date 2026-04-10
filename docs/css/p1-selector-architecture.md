# P1: Define Selector Architecture, IR, And Parsing Contract

Last updated: 2026-04-10  
Status: complete

This document is the source-of-truth contract for Milestone P issue P1:
selector subsystem ownership, selector IR shape, specificity accounting,
invalid/unsupported selector handling, and stable selector debug output.

Milestones N and O established the syntax layer and the engine-facing
stylesheet/rule/value model. Before Milestone P, style-rule selectors were
still preserved as generic prelude component values and reparsed ad hoc by
transitional code. Milestone P replaces that with structured selector parse
results stored directly on `StyleRule`, which is the required foundation for
later selector matching and cascade work.

P1 does not finish selector parsing or selector matching. It defines the
selector subsystem contract that later Milestone P issues must implement.

Related code:
- `crates/css/src/syntax/mod.rs`
- `crates/css/src/syntax/compat.rs`
- `crates/css/src/model/mod.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/serialize.rs`

Related documents:
- `docs/css/syntax-parser-contract.md`
- `docs/css/n9-selector-structure-expansion.md`
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/o7-parser-model-cutover.md`

## Implemented Result

Milestone P now has an explicit in-repository selector contract for:

- a dedicated selector subsystem separate from generic syntax parsing and from
  DOM matching
- an explicit selector IR centered on selector lists, complex selectors,
  compounds, combinators, and supported simple selectors
- deterministic specificity calculation for the supported selector subset
- explicit distinction between parsed, unsupported, and invalid selector
  results
- stable selector snapshot serialization that does not depend on Rust derived
  `Debug`
- a defined supported subset for Milestone P

The code contract now lives in `css::selectors` and is integrated into the
engine-facing stylesheet model through structured style-rule selector parse
results rather than preserved raw selector source.

## Why This Exists

The repository currently has a selector architecture gap:

1. `css::syntax` preserves qualified-rule preludes structurally, but not as a
   selector-specific IR
2. `CompatSelector` is only a migration bridge and only models `*`, type,
   class, and id selectors
3. `css::cascade` still contains its own compatibility selector parsing and
   specificity logic

That split creates three problems:

- selector logic is duplicated across migration code
- selector behavior is too limited to support the next milestone cleanly
- later matching and cascade work would be forced to either reparse selector
  source text again or keep extending compatibility code that was never meant
  to become permanent

P1 exists to stop that fragmentation before full selector parsing lands.

## Layer Boundary

The architectural boundary for selector work is:

1. `css::syntax`
   - owns tokenization
   - owns generic stylesheet parsing and recovery
   - owns qualified-rule prelude component values
   - does not own selector specificity or selector matching
2. `css::selectors`
   - consumes syntax-layer selector source
   - owns selector IR
   - owns supported-subset parsing rules
   - owns invalid-vs-unsupported classification
   - owns specificity calculation for parsed selectors
   - does not own DOM matching, cascade ordering, or style resolution
3. later selector/cascade work
   - matches selector IR against DOM nodes
   - uses selector specificity during cascade winner resolution
   - interprets feature-gated selector forms once support is added

Required architectural rule:

- selector IR must be built from structured syntax-layer output
- selector consumers must not reparse raw stylesheet text
- `CompatSelector` is not a legal foundation for the permanent selector system

## Parsing Contract

The selector parser consumes one qualified-rule prelude from structured
`css::syntax` output and produces one of three outcomes:

- `Parsed(SelectorList)`
- `Unsupported(UnsupportedSelectorList)`
- `Invalid(InvalidSelectorList)`

This distinction is normative.

### Parsed

`Parsed` means:

- the selector list is syntactically valid for Borrowser's supported subset
- the selector list has a complete selector IR
- specificity is defined for each parsed selector
- later matching code may evaluate the selector

### Unsupported

`Unsupported` means:

- the selector input is structurally well-formed enough to classify
- at least one syntactic feature is outside the supported subset
- the selector list is preserved as non-matchable for the current engine stage
- the parser must not silently reinterpret the selector as a narrower supported
  selector

Unsupported selector handling strategy for Milestone P:

- preserve the selector source span
- classify unsupported feature categories explicitly
- preserve unsupported feature categories in stable first-encounter order
- deduplicate repeated occurrences of the same unsupported feature category
- treat the whole selector list as non-matchable
- allow later milestones to upgrade the same source into a parsed selector IR
  without changing the parser boundary

### Invalid

`Invalid` means:

- the selector input is malformed for the supported grammar
- the selector list must not participate in matching or specificity
- the parser reports a deterministic invalid-selector reason

Invalid selector handling strategy for Milestone P:

- reject the selector list as a parsed selector IR
- preserve source span information for diagnostics/debugging
- treat the whole selector list as non-matchable

## List-Level Failure Policy

Borrowser will not partially salvage selector lists at this layer.

Normative rule:

- if any selector in a comma-separated selector list is invalid, the whole
  selector list result is `Invalid`
- if no selector is invalid but any selector uses an unsupported feature, the
  whole selector list result is `Unsupported`
- only selector lists where every selector is fully supported produce
  `Parsed`

This avoids implementation-defined partial matching and keeps behavior aligned
with CSS selector-list invalidation expectations.

## Contracted Selector IR

The selector IR is explicit and source ordered.

### SelectorList

`SelectorList` owns:

- an optional full-list source span
- ordered parsed selectors

It is the selector-side handoff object later style-rule and cascade work will
consume.

### ComplexSelector

One parsed selector is represented as:

- one head `CompoundSelector`
- zero or more `CombinedSelector` tail segments

The representation is left-to-right and deterministic. This is the intended
matching traversal input for later work, even if later optimization chooses a
different internal matching plan.

### CombinedSelector

Each `CombinedSelector` owns:

- one combinator
- the compound selector to its right
- a span covering the combined segment

Supported combinators for Milestone P:

- descendant
- child (`>`)
- next-sibling (`+`)
- subsequent-sibling (`~`)

Unsupported combinators:

- column combinator (`||`)

### CompoundSelector

Each supported compound selector owns:

- an optional type selector
- zero or more subclass selectors in source order
- one compound span

Supported type-selector forms:

- universal selector (`*`)
- named type selector

Supported subclass selector forms:

- id selector
- class selector
- attribute selector

Supported attribute selector subset:

- existence form: `[attr]`
- match forms using `=`, `~=`, `|=`, `^=`, `$=`, `*=`
- attribute values as identifier or string

Unsupported attribute selector features for this milestone:

- namespaces inside attribute selectors
- case-sensitivity modifiers (`i`, `s`)

### Deferred Selector Forms

The following are explicitly outside the Milestone P supported subset and must
surface as `Unsupported`, not as ad hoc partial parses:

- namespace-qualified selectors
- pseudo-classes
- functional pseudo-classes including `:not()`, `:is()`, `:where()`, and
  `:has()`
- pseudo-elements
- relative selectors
- nesting selector `&`
- forgiving selector lists
- column combinator `||`

## Specificity Model

Specificity is represented as the tuple `(a, b, c)`:

- `a`: id selector count
- `b`: class selector and attribute selector count
- `c`: type selector count

Specificity rules for the supported subset:

- universal selector contributes `(0, 0, 0)`
- named type selector contributes `(0, 0, 1)`
- id selector contributes `(1, 0, 0)`
- class selector contributes `(0, 1, 0)`
- attribute selector contributes `(0, 1, 0)`
- combinators contribute nothing directly
- complex-selector specificity is the sum of its compound specificities

Specificity arithmetic is saturating on each component to keep hostile input
bounded and deterministic.

Unsupported and invalid selector lists do not have usable selector specificity.
They are non-matchable parse outcomes rather than parsed selector IR.

## Invalid Selector Categories

The first implemented invalid-reason categories are:

- empty selector list
- empty compound selector
- leading combinator
- trailing combinator
- repeated combinator
- multiple type selectors in one compound
- missing attribute name
- missing attribute value
- unexpected component value

This list may grow as implementation detail expands, but the parser must keep
the categories explicit and deterministic.

## Serialization And Debug Contract

Rust derived `Debug` is not the contract for selectors.

The selector subsystem provides stable, explicit serializers:

- `serialize_selector_list_for_snapshot`
- `serialize_selector_parse_result_for_snapshot`
- `SelectorList::to_debug_snapshot`
- `SelectorListParseResult::to_debug_snapshot`

Snapshot guarantees:

- snapshots begin with `version: 1`
- field order is explicit
- selectors serialize in source order
- compound selectors serialize in source order
- selector snapshots report the semantic node span for each selector node
- selector snapshots report payload spans separately where the payload is a
  meaningful subspan such as an identifier name or attribute value
- specificity is shown explicitly per selector and compound
- invalid and unsupported parse outcomes serialize their category rather than
  implementation-specific debug output

## Diagnostic Ownership Boundary

Selector diagnostics may travel through the shared `SyntaxDiagnostic` envelope
during stylesheet parsing, but selector semantics remain selector-owned.

Normative rule:

- `css::selectors` owns selector invalid-vs-unsupported classification
- `css::syntax` may expose shared diagnostic transport/types
- `css::syntax` must not become the place where selector-semantic parsing or
  selector support policy is implemented

## Integration Status

P1 defined the architecture and code contract; later Milestone P issues
completed selector parsing, specificity, invalid/unsupported handling,
serialization, and model integration.

Current repository state after P1:

- `css::selectors` exists as the permanent selector IR contract
- selector specificity logic exists independently from `css::cascade`
- selector invalid/unsupported behavior is explicit in code and docs
- stable selector snapshots exist for regression tests
- `StyleRule` now stores structured selector parse results rather than
  preserved selector source
- `CompatSelector` remains migration-only and is not the selector-system target
- Milestone P is complete and provides the selector/model foundation for
  Milestone Q matching work

## Exit Criteria

P1 is complete when:

- selector architecture is documented
- selector IR is explicit in code
- specificity model is documented and represented in code
- invalid and unsupported selector behavior is explicitly defined
- supported subset scope is unambiguous
- selector snapshot/debug expectations are explicit and testable

Repository status:

- the P1 selector architecture issue is complete and may be treated as closed
- Milestone P is complete; the next work should begin with Milestone Q
  selector matching over the structured selector/model pipeline, not by
  reopening the architecture contract
