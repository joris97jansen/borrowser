# Q1: Define Selector Matching Architecture And DOM Contract

Last updated: 2026-08-16
Status: architecture contract implemented

This document is the source-of-truth contract for Milestone Q issue 1:
selector matching architecture, DOM interaction rules, matching invariants,
and the extension boundaries later selector-matching work must follow.

Milestone P established Borrowser's selector IR, specificity model, and the
explicit `Parsed | Unsupported | Invalid` selector parse-result contract.
Milestone Q builds on that foundation to define how parsed selectors are
matched against DOM elements without coupling selector semantics to one DOM
storage format, parser mode, or cascade implementation detail.

This issue does not finish selector evaluation. It defines the architecture
and code contract the later matching implementation must obey.

Related code:
- `crates/css/src/selectors/mod.rs`
- `crates/css/src/selectors/matching.rs`
- `crates/css/src/dom_attributes.rs`
- `crates/css/src/lib.rs`
- `crates/browser/src/dom_store/arena.rs`
- `crates/html/src/types.rs`

Related documents:
- `docs/css/p1-selector-architecture.md`
- `docs/css/p2-selector-ir-data-structures.md`
- `docs/css/p4-specificity-calculation.md`
- `docs/css/p5-invalid-selector-handling.md`
- `docs/css/p6-unsupported-selector-handling.md`
- `docs/css/p8-selector-model-integration.md`
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`
- `docs/html5/node-identity-contract.md`

## Implemented Result

Milestone Q issue 1 now has an explicit in-repository architecture contract
for selector matching:

- a DOM adapter boundary in `css::selectors::matching::SelectorMatchDom`
- a deterministic element-only DOM indexing surface for the owned
  `html::Node` tree path in `SelectorDomIndex`
- an explicit match-result surface in `SelectorListMatchOutcome`
- a deterministic matched-result construction path in
  `SelectorListMatchBuilder`
- a defined integration rule from selector parse result state into selector
  matchability through `SelectorMatchability`
- deterministic debug snapshots for the selector DOM index and selector match
  outcomes

This means later matching work can now be implemented against a stable DOM
contract and result surface rather than reaching directly into ad hoc DOM
details or re-inventing selector applicability behavior inside cascade code.

## Why This Exists

Before Q1, the repository had:

1. a production selector IR in `css::selectors`
2. a temporary compatibility matcher inside `css::cascade`
3. two relevant DOM shapes:
   - the owned `html::Node` snapshot tree
   - the runtime patch-applier arena in `browser::dom_store`

That left a major architectural gap: full selector matching needs parent and
sibling traversal, but `css` cannot depend directly on the browser crate's
runtime arena, and the owned tree does not expose parent links directly.

Without an explicit matching contract, later work would be pushed toward one
of two bad outcomes:

- selector matching grows against one incidental DOM representation and later
  needs to be rewritten for another
- matching semantics leak into cascade code again through temporary bridging

Q1 exists to close that gap before the full matcher lands.

## Layer Boundary

The selector matching boundary is now:

1. `css::syntax`
   - owns tokenization and syntax recovery
   - does not own selector matching
2. `css::selectors`
   - owns selector IR
   - owns selector specificity
   - owns `Parsed | Unsupported | Invalid` selector state
   - owns the DOM-facing selector matching contract
   - does not own cascade winner resolution or computed style generation
3. DOM providers
   - expose the minimal element relationships, canonical name/namespace, and
     ordered neutral attribute facts required through `SelectorMatchDom`
   - may store the DOM however they want as long as they honor the contract
4. later cascade work
   - consumes selector match results plus selector specificity
   - resolves declaration winners
   - stays separate from selector-evaluation mechanics

Normative rule:

- selector evaluation consumes selector IR / parsed selector structures,
  `SelectorMatchDom` facts through the `SelectorMatchingContext` query
  abstraction, and an explicit CSS-owned immutable
  `SelectorMatchingEnvironment`
- the current matching environment contains the parser-selected HTML
  `DocumentMode`; CSS owns the interpretation of that semantic input
- Browser/runtime identity and DOM generation are not members of the matching
  environment
- selector evaluation must not depend on parser-internal state, raw selector
  source reparsing, one specific DOM storage layout, or reconstruction of
  document mode from the DOM or parser internals
- an unavailable environment is an explicit failure condition and must never
  imply `DocumentMode::NoQuirks`

## DOM Contract

Selector matching is defined over elements only.

The engine is allowed to depend on exactly these DOM-facing facts:

- actual document-element identity
- nearest parent element
- nearest previous element sibling
- nearest next element sibling
- canonical element local name and namespace
- ordered neutral attribute namespace/local-name/value facts
- ordinary direct element children
- exact ordinary direct text children

The current code contract is the `SelectorMatchDom` trait:

- `parent_element(element)`
- `previous_sibling_element(element)`
- `next_sibling_element(element)`
- `document_element()`
- `element_local_name(element)`
- `element_namespace(element)`
- `attributes(element)`
- `first_element_child(element)` plus `next_sibling_element(...)` iteration
- `direct_text_children(element)`

AF4b is the normative refinement of this Q1 fact surface. It deliberately does
not expose selector- or pseudo-specific predicates.

Selector matching is not allowed to depend on:

- raw parser insertion-mode state
- DOM builder open-element stacks
- node allocation order outside the adapter contract
- text/comment/processing-instruction/doctype/document nodes as match subjects
- style data already attached to the DOM
- computed values
- layout state
- browser event/input state

### Element Axes

For the matching contract:

- selectors match elements only
- document nodes never match selectors
- text/comment/processing-instruction nodes never match selectors
- child and descendant traversal use element ancestors only
- adjacent and general sibling traversal use previous element siblings only
- text, comment, processing-instruction, and doctype siblings are skipped for
  sibling combinators; the root document is outside the axes and nested
  documents are rejected during projection construction
- forward element-sibling queries use the same element-only axis

This is the required invariant that keeps matching stable across the owned
tree path and the runtime arena path even though the underlying node storage
differs.

### Name And Attribute Semantics

For Borrowser's current parser-created DOM:

- HTML element names have ASCII uppercase folded while non-ASCII is preserved
- foreign element local names retain their canonical namespace-specific case
- attributes are exposed as ordered namespace/local-name/value facts
- for HTML type and unqualified attribute names, CSS ASCII-lowercases only the
  selector/request-side name and then compares identically to the exact actual
  name; foreign names compare exactly
- CSS applies document-mode-aware ID/class value comparison and HTML default
  attribute-value comparison as refined by AF4c
- CSS selects the first matching unqualified attribute in provider order for
  the currently supported selector subset

This is important for determinism: selector matching depends on neutral ordered
facts plus CSS-owned comparison policy, not on incidental parser recovery
history.

The canonical-name assumption is explicit, not incidental. The current owned
HTML path relies on the HTML layer's atom/canonicalization guarantees:

- `crates/html/src/types.rs` documents canonical tag/attribute storage with no
  ASCII uppercase for the current `AtomTable`
- `crates/html/src/html5/shared/atom.rs` documents ASCII-lowercasing for the
  HTML atomization path

The selector adapter therefore treats non-canonical HTML element names as an
upstream invariant violation rather than silently normalizing them.

The provider does not collapse attributes or decide whether a selector-provided
name matches a stored name. ID equality, class tokenization, attribute
operators, and selector name comparison remain CSS semantics.

AF4c further requires CSS to keep asymmetric document-name matching distinct
from symmetric sensitive/ASCII-insensitive value comparison. Attribute-value
policy is selected from the complete effective attribute's semantic identity,
not from raw authored selector spelling, and operator execution receives an
already selected value policy. See
`docs/css/af4c-html-host-language-selector-comparison.md` for the current
parser-created HTML-document contract and its XML/escape exclusions.

## Matching Contract

### Matchability

Selector parse state directly constrains matching:

- `Parsed` selector lists are matchable
- `Unsupported` selector lists are explicit non-matchable inputs
- `Invalid` selector lists are explicit non-matchable inputs

Q1 formalizes that with `SelectorMatchability` and
`SelectorListParseResult::matchability()`.

Normative rule:

- unsupported or invalid selector lists must not be silently downgraded to
  `NotMatched`
- matching consumers must be able to distinguish:
  - parsed but not matched
  - unsupported and therefore not matchable
  - invalid and therefore not matchable

### Match Result Shape

`SelectorListMatchOutcome` is the selector-engine result surface for one
selector list against one target element.

Its guarantees are:

- matchability state is explicit
- matched selector entries are carried in source-order index form
- `selector_index` is the authoritative source-order identity for one selector
  list entry
- matched selector entries are deduplicated by selector index
- duplicate selector indices with differing specificity are invalid internal
  state
- specificity is attached per matched selector entry
- highest matched specificity is derived from the matched entries, not
  recomputed through a separate code path

This is the required handoff shape for later cascade work because one rule may
contain multiple comma-separated selectors that all match the same element.

The intended matcher-side construction path is `SelectorListMatchBuilder`:

- the matcher records one selector-list hit at a time
- selector indices are coalesced as they are recorded
- source order is preserved by construction
  because `selector_index` is the ordering identity, not discovery order
- conflicting specificity for one selector index remains a debug-time engine
  invariant violation

The lower-level duplicate-normalization path remains only as a defensive
backstop inside the selector subsystem, not as the preferred matcher contract.

## Evaluation Strategy

The normative matching strategy for Milestone Q is:

1. evaluate selector lists in source order
2. evaluate one complex selector right-to-left from the target element
3. evaluate one compound selector as a conjunction of its simple selectors
4. on combinators, move along the required DOM axis and continue matching the
   selector segment to the left

High-level rules:

- the rightmost compound is tested against the subject element first
- descendant combinators walk parent elements until a match is found or the
  root is reached
- child combinators test only the nearest parent element
- next-sibling combinators test only the nearest previous element sibling
- subsequent-sibling combinators walk previous element siblings until a match
  is found or siblings are exhausted

Within one compound selector, evaluation is deterministic:

- type or universal selector first when present
- subclass selectors after that in source order

Matching may short-circuit on failure, but any optimized implementation must
remain observationally equivalent to this normative algorithm.

## Supported Scope For Milestone Q

Q1 locks the matching scope for the first implementation of the Milestone Q
engine.

In scope:

- matching parsed selector IR from Milestone P
- selector lists
- complex selectors
- compound selectors
- combinators:
  - descendant
  - child
  - next sibling
  - subsequent sibling
- simple selectors:
  - universal
  - named type
  - id
  - class
  - attribute existence
  - attribute match operators already in the selector IR
- integration of selector validity/support state with matching
- deterministic debug and regression surfaces for DOM indexing and match
  outcomes

Out of scope for Q1 and still deferred beyond this issue:

- pseudo-classes beyond AF4d's typed tree-structural subset
- functional pseudo-classes
- pseudo-elements
- CSS `@namespace` and namespace-selector syntax
- relative selectors
- nesting selector `&`
- forgiving selector lists
- selector matching caches or invalidation systems
- cascade winner resolution
- computed style generation
- layout-facing selector optimization

## Determinism Requirements

The selector engine must behave identically when:

- the same selector IR is provided through different parse-call paths
- the DOM was built through different construction paths but exposes the same
  document identity, element relationships, and neutral attribute/name facts
  through `SelectorMatchDom`

Required invariants:

- selector list order is source order
- matched selector indices are reported in source order
- specificity values come from the selector IR and are stable
- element axes are defined only in terms of parent, previous/next element
  siblings, and ordinary direct element children
- matching never depends on raw node ids assigned by one DOM builder path
- current matching never depends on text/comment/processing-instruction node
  placement except insofar as sibling traversal skips them deterministically;
  exact direct text remains a neutral fact for later pseudo work
- unsupported and invalid selector states remain distinguishable from ordinary
  no-match results

## Extension Points

Q1 intentionally leaves explicit places for later selector-class expansion.

### DOM Surface Extension

The core `SelectorMatchDom` trait is minimal because the supported subset needs
only neutral document identity, tree relationships, canonical name/namespace,
ordered attribute, and exact child/content facts.

Later selector classes may require additional neutral facts. Each fact must
enter selector matching through the narrowest explicit input/query contract
supplied by the subsystem that legitimately owns it. Tree-local facts may
extend `SelectorMatchDom` and `SelectorMatchingContext`; future dynamic state
is not automatically owned by the parser-created selector-DOM adapter.

Legitimate extensions may include:

- CSS namespace-prefix/default-namespace resolution beyond AE11's typed
  element-namespace query
- additional tree relationships, document identity/provenance, exact
  child/content facts, or tree-boundary/scoping information
- neutral runtime, input, focus, navigation, or document-state facts exposed
  through an explicit selector-matching input/query contract by their owning
  subsystem

Providers expose facts only. CSS selector matching interprets those facts as
selector and pseudo-class semantics. Extensions must not add pseudo-specific
provider methods such as `is_first_child_for_css`, `is_empty_for_css`,
`matches_root`, `matches_nth_child`, `matches_hover`, `matches_focus`, or
`is_active_for_css`. They must keep storage, layout, and runtime implementation
details behind their owning boundaries and must not smuggle selector meaning
through HTML, Browser/runtime, Layout, Paint, or generic DOM providers.

### AE11 namespace refinement

AE11 adds `element_namespace` to `SelectorMatchDom` and carries the canonical
HTML/SVG/MathML namespace through `SelectorDomIndex`. For HTML type names, CSS
ASCII-lowercases the selector side and compares identically to the exact actual
name; foreign canonical names are case-sensitive. Unprefixed attribute
selectors query only no-namespace attributes.

Author selectors remain unconstrained under the currently supported
no-default-namespace semantics. Internal UA rule groups may provide
`SelectorNamespaceConstraint::Exact(Html)`. The context propagates that
constraint through selector lists, combinators, and every visited compound,
including universal and typeless compounds. Supported nested selector-bearing
pseudo-classes do not exist yet, so there is no additional recursion entry
point to constrain. Full namespace-selector syntax remains deferred.

### Selector Evaluation Extension

The matching architecture is explicitly split into:

- compound simple-selector evaluation
- combinator axis traversal
- selector-list result aggregation

Future selector classes should attach to one of those surfaces rather than
rewriting the whole engine contract.

Structural pseudo-classes added later must derive their meaning from AF4b's
neutral document-element, sibling, direct-element-child, and exact direct-text
facts. They must not add pseudo-specific provider methods.

Stateful or dynamic pseudo-classes must likewise consume neutral state from the
subsystem that owns it and leave interpretation in CSS. New pseudo-class work
must extend CSS-owned selector IR/evaluation, specificity, debug, and
invalidation surfaces as appropriate.

### Regression Surface Extension

Q1 introduces deterministic snapshots for:

- indexed selector DOM shape
- selector match outcomes

Later matching work must preserve these stable surfaces or replace them with
strictly better explicit serializers. Rust derived `Debug` is not the
contract.

## Owned Tree Adapter

`SelectorDomIndex` is the Q1 adapter for the existing owned `html::Node`
surface, hardened by AF4b's normative construction contract.

It exists for two reasons:

- the owned tree does not store parent links directly
- matching must not depend on `Node::id()` assignment behavior

`SelectorDomIndex` therefore:

- walks the owned tree iteratively
- is fallible from its first traversal allocation
- indexes elements in document order
- stores only neutral selector-query facts
- assigns its own deterministic element ids independent from DOM node ids
- does not represent non-element nodes as match subjects
- skips text, comment, processing-instruction, and doctype nodes for sibling
  axes; the root document is outside those axes and nested documents are
  rejected during projection construction
- treats HTML element names without ASCII uppercase as an adapter invariant;
  non-ASCII remains preserved
- records actual document-element identity rather than inferring it from a
  parentless element
- rejects unexpected nested `Node::Document` values with a typed build error
- uses authoritative `try_from_document`, a test-only unbounded element-
  subtree seam, and a bounded legacy compatibility path with the same explicit
  subtree provenance; there is no generic `from_root`
- excludes template-associated fragment contents by traversing only ordinary
  element children

This is intentionally a clean adapter layer, not the permanent proof that all
matching must use an indexed tree. Later DOM providers may implement
`SelectorMatchDom` differently as long as the observable contract is the same.

The complete construction, text-storage, identity, error, and complexity
contract is `docs/css/af4b-selector-dom-query-contract.md`.

## Non-Goals

Q1 does not:

- replace the temporary compatibility matcher inside `css::cascade`
- implement full selector evaluation
- decide cascade winner ordering
- decide computed-style attachment strategy
- introduce selector caches, bloom filters, or invalidation machinery
- bind the selector engine permanently to `html::Node` or to the browser DOM
  arena

Those belong to later Milestone Q issues.

## Exit Criteria

AF4a refines the normative matcher input to selector IR plus
`SelectorMatchDom` plus an explicit CSS-owned `SelectorMatchingEnvironment`.
The environment is transported from parser metadata while selector semantics
remain entirely in CSS.

Q1 is complete when:

- selector matching architecture is documented
- the DOM interaction contract is explicit in code and docs
- selector matchability rules are explicit
- deterministic match-result and selector-DOM debug surfaces exist
- Milestone Q scope and non-goals are unambiguous
- future extension points are identified

Repository status:

- the Q1 selector matching architecture issue is complete and may be treated
  as closed
- later Milestone Q work should implement selector evaluation against
  `SelectorMatchDom` and return `SelectorListMatchOutcome`
- cascade migration should consume those results rather than extending the
  current compatibility matcher further
