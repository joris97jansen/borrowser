# AF4b: Selector DOM Query Contract For Parser-Created DOM

Last updated: 2026-08-16
Status: implemented

AF4b defines the production selector-DOM projection used by CSS matching,
cascade, computed style, style-tree reconstruction, and their deterministic
debug surfaces. It refines the Milestone Q adapter without moving CSS selector
meaning into HTML or Browser/runtime.

This document is the normative contract for selector-DOM construction and
queries. The older Q, R, S, U, and hardening contracts remain authoritative for
their own layers only where they agree with this AF4b refinement.

Related implementation:

- `crates/css/src/selectors/matching/dom_index.rs`
- `crates/css/src/selectors/matching/context/dom.rs`
- `crates/css/src/selectors/matching/context/queries.rs`
- `crates/css/src/selectors/matching/context/attributes.rs`
- `crates/css/src/selectors/matching/context/compound.rs`
- `crates/css/src/selectors/matching/comparison.rs`
- `crates/css/src/selectors/matching/host_language.rs`
- `crates/css/src/dom_attributes.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/cascade/integration/limits.rs`
- `crates/css/src/cascade/integration/debug_snapshot.rs`
- `crates/css/src/computed/document/`
- `crates/css/src/computed/style_tree.rs`

Related contracts:

- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q2-selector-matching-context.md`
- `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af4a-document-matching-environment.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`
- `docs/html5/ae1-html-parser-dom-ownership-contract.md`
- `docs/html5/ae2-parser-created-dom-node-model.md`
- `docs/html5/ae10-template-tree-construction-contract.md`
- `docs/html5/invariants.md`
- `docs/html5/node-identity-contract.md`

## Ownership Boundary

HTML owns parser-created nodes and the facts stored on them:

- node kind and ordinary child storage
- parser-created document structure
- canonical expanded element names
- ordered qualified attributes and exact values
- exact text data
- the typed association between a `template` host and its
  `DocumentFragment` contents
- source DOM node identity

CSS owns:

- the fallible projection of those facts into selector-query form
- the CSS-local `SelectorDomElementId` identity domain
- selector attribute-name and value comparison policy
- ID equality and class tokenization
- selector matching, including combinator and future pseudo-class semantics
- cascade, computed-style, and selector debug/error contracts

Browser/runtime may transport DOM values, matching environments, computed
artifacts, and typed CSS errors. It must not provide selector facts or interpret
selector semantics. Layout and Paint remain consumers of computed/layout data
and are outside this boundary.

## Explicit Construction Modes

There is no generic, infallible root constructor. In particular,
`SelectorDomIndex::from_root(...)` is not part of the AF4b contract.

The authoritative document constructor is:

```rust
SelectorDomIndex::try_from_document(root: &html::Node)
    -> Result<SelectorDomIndex<'_>, SelectorDomBuildError>
```

Style resolution uses a crate-private bounded form of the same document path
so it can enforce its separate styled-element resource budget during
preflight. Both production document paths accept only `Node::Document`.

The unbounded element-subtree constructor is only a test seam where compiled:

```rust
#[cfg(test)]
SelectorDomIndex::try_from_element_subtree(root: &html::ElementNode) // crate-private
    -> Result<SelectorDomIndex<'_>, SelectorDomBuildError>
```

The document constructor accepts only `Node::Document`. A caller passing an
element, doctype, text, comment, or processing-instruction root receives a
typed invalid-root build error. The test-only subtree seam accepts an
`&ElementNode`, making every non-element root kind unrepresentable at that
boundary.

Production cascade, computed-style, style-tree, and Browser paths use document
construction. The legacy `attach_styles` compatibility bridge uses the
explicit crate-private bounded element-subtree construction path for its
historically accepted element input; it does not make the unbounded test seam
available in production. Both subtree paths carry the same `ElementSubtree`
provenance and closed-boundary semantics described below. No generic root
constructor exists.

The projection retains typed provenance equivalent to:

```rust
enum SelectorDomProjectionRoot {
    Document {
        document_element: Option<SelectorDomElementId>,
    },
    ElementSubtree {
        root_element: SelectorDomElementId,
    },
}
```

An isolated subtree contains its root and ordinary descendants. Its root has no
in-projection parent or siblings, relationships outside the boundary are
unavailable, and `document_element()` is always `None`. Parentlessness is
therefore not document-element identity.

## Document-Element Identity

Document-element identity is an explicit projection fact:

```rust
fn document_element(&self) -> Option<SelectorDomElementId>;
```

It is established only by successful document construction:

- no direct document element is valid and produces `None`
- exactly one direct document element produces its exact selector element ID
- a second direct document element is a typed
  `MultipleDocumentElements` build failure
- a subtree root is never promoted to document element
- element spelling is not used to infer identity

This is the narrow validation CSS needs to avoid ambiguous `:root` foundations.
It does not require an element named `html` and does not expand CSS into a
general HTML/DOM validator.

## Fallible Iterative Construction

Construction is iterative and fallible from the first heap-backed traversal
allocation. It does not use recursive DOM traversal and does not claim recovery
from an allocator abort or general process OOM.

The first pass uses a depth-first child-frame stack. Before each stack growth,
the builder checks the new length with `checked_add` and uses Rust's fallible
`Vec::try_reserve` API. This pass:

- validates the selected root mode
- rejects every nested `Node::Document`
- determines document-element cardinality
- validates the canonical HTML local-name invariant
- counts elements and direct ordinary text nodes with checked arithmetic
- proves that every element has a representable `SelectorDomElementId`
- records the maximum traversal depth
- optionally reports a caller-supplied style element budget through a separate
  bounded-construction boundary

After preflight, the builder fallibly reserves the exact element-record and
direct-text capacities and enough second-pass traversal capacity for the
observed maximum depth. Every ID, range endpoint, and collection length is
checked before mutation. The second pass then materializes the projection
without an unplanned growth path.

`SelectorDomBuildError` is restricted to selector projection failures:

- `InvalidDocumentRoot` for a non-document document-constructor root
- `NestedDocument` for a nested `Node::Document`
- `MultipleDocumentElements` for ambiguous direct document elements
- `NonCanonicalHtmlElementLocalName` for an HTML local name containing ASCII
  uppercase; canonical HTML atomization preserves non-ASCII
- `ElementIdRepresentationExhausted` for selector element-ID exhaustion
- `ProjectionCapacityExceeded` for checked projection-capacity failure
- `StorageReservationFailed` for a failure reported by Rust's fallible
  reservation APIs

The style-resolution limit `max_styled_elements_per_document` is not a
`SelectorDomBuildError`. Cascade uses the private
`BoundedSelectorDomConstructionError` boundary, which distinguishes:

```text
selector projection failure
    -> StyleResolutionError::SelectorDomBuild(...)

style element budget failure
    -> StyleResolutionError::LimitExceeded(
           StyledElementsPerDocument
       )
```

Consequently, a valid selector projection does not become structurally invalid
because one style caller selected a lower resource budget. The bounded
projection traversal replaces the old independent styled-element count walk,
so invalid nested documents cannot be normalized or hidden by a separate
prepass.

## CSS-Local Projection Identity

`SelectorDomElementId` is a checked, nonzero, CSS-owned identity assigned in
projection document order. Its representation is validated before
materialization. Element identity may be derived from the checked element
record position instead of being redundantly stored in every record.

`u32::MAX` is the maximum valid selector element ID. The element iterator uses
checked zero-based `usize` position/end bounds, so it can yield that final ID
exactly once without ever representing `u32::MAX + 1` in a `u32`. Iterator
termination uses bounds rather than saturation, and conversion back to an ID
relies only on bounds established by successful projection construction; no
public constructor can manufacture an out-of-range iterator.

This identity is distinct from:

- `html::internal::Id`
- DOM patch keys
- retained-render identities
- Browser cache identities
- Layout or Paint identities

The index may map a source DOM ID to a selector element ID for incremental
dirty-node integration. That is an explicit mapping between identity domains,
not reuse or equivalence. Source DOM IDs do not appear in selector-DOM debug
snapshots as selector identity.

## Neutral Element And Relationship Facts

`SelectorMatchDom` exposes neutral facts only:

- actual document-element identity
- parent element
- previous element sibling
- next element sibling
- canonical element local name
- element namespace
- ordered neutral attributes
- ordinary direct element children
- exact ordinary direct text children

Element sibling links skip text, comments, processing instructions, and
doctypes. Those non-element nodes never change previous/next element sibling
relationships. The root `Node::Document` is the projection container outside
element sibling axes. Every nested `Node::Document` is an invalid structure
rejected during projection construction; it is never skipped through,
flattened, normalized, or represented on an axis. Parent, previous sibling,
and next sibling are indexed during construction. Because records are
preorder, the first element child is derived in `O(1)` by checking the next
record's parent; further children follow the next-sibling links. Selectors do
not repeatedly rescan HTML child storage.

The contract does not expose pseudo-specific helpers such as:

- `is_root_for_css`
- `is_empty_for_css`
- `is_first_child_for_css`
- `has_non_whitespace_text_for_css`

Future CSS pseudo-class implementations must derive their meaning from neutral
facts inside CSS.

## Neutral Attribute Facts

The neutral borrowed attribute view exposes:

```rust
pub struct SelectorDomAttribute<'a> { /* private fields */ }

impl SelectorDomAttribute<'_> {
    pub const fn namespace(self) -> html::AttributeNamespace;
    pub const fn local_name(self) -> &str;
    pub const fn value(self) -> &str;
}
```

`SelectorMatchDom::attributes(element)` returns an allocation-free,
deterministically ordered, exact-size iterator of these views. It does not
accept a selector-provided name and does not select an effective value.

The private CSS-wide helper in `crates/css/src/dom_attributes.rs` may find the
first effective unqualified attribute from neutral facts. It owns only:

- the requirement that the attribute namespace be `None`
- HTML-element selector/request-side ASCII-lowercased versus foreign-element
  exact local-name comparison
- first-provider-order selection

For HTML elements, this name rule lowercases ASCII on the selector/request side
only and then compares identically to the exact actual local name. It is not
symmetric ASCII-insensitive equality. The distinction preserves the
parser-created canonical-name contract without defining future noncanonical
DOM-mutation behavior accidentally.

Selector matching consumes that helper for `[attr]`, `#id`, `.class`, and
attribute operators, then applies selector-owned value/operator or token
semantics. Cascade integration consumes the same helper independently to find
the inline `style` attribute; it does not call selector-matching-specific query
methods. The shared helper does not implement attribute operators, ID equality,
class tokenization, or pseudo semantics.

AF4c retains the complete effective `SelectorDomAttribute` after name
resolution so CSS can select attribute-value policy from semantic identity:
candidate element namespace, effective attribute namespace, effective actual
local name, and exact borrowed value. Raw authored selector spelling is not a
value-policy key. The normative HTML insensitive-value inventory is consulted
only for an HTML element and an unqualified effective attribute, using exact
lookup of its canonical actual local name. None of that value/operator policy
is added to the shared helper used for inline `style` discovery.

The DOM adapter does not expose or implement `has_attribute`,
`attribute_value(name)`, `element_has_id`, or `element_has_class`.

## Exact Direct Text Storage

Every ordinary direct `Node::Text` value is preserved as a separate borrowed
string. The projection does not concatenate, trim, classify, tokenize, or drop
text, including empty strings.

Each element stores a range into a global direct-text vector. The required
construction invariant is owner-first materialization: when the second pass
indexes an element, it scans that element's ordinary `children()` and appends
all of that element's direct text nodes, in child order, before descending into
any child element.

For:

```html
<div>
  before
  <span>descendant</span>
  after
</div>
```

the arena and ranges are equivalent to:

```text
arena = ["before", "after", "descendant"]
div   = 0..2
span  = 2..3
```

The query results are therefore:

```text
div  direct text = ["before", "after"]
span direct text = ["descendant"]
```

Descendant text cannot interleave an owner's range because descent happens only
after that owner's complete direct-text group has been appended. Child order
within each owner remains exact. Empty text, ASCII/document whitespace, NBSP,
and ordinary non-whitespace text remain distinct entries with unchanged byte
content. This is a fact surface for a later `:empty`; AF4b does not implement
that pseudo-class.

## Template Boundary

Projection traverses only `ElementNode::children()`. It does not inspect tag
spelling or enter the separate `template_contents`/`DocumentFragment`
association.

Therefore:

- template-associated fragment descendants receive no ordinary-host child or
  sibling relationships
- associated fragment text is not direct text of the template host
- an actual ordinary child stored on the template host remains an ordinary
  child
- no selector-specific template exception exists

The boundary follows AE's typed storage relationship and remains correct for
any element carrying such an association; CSS does not detect templates by
local-name spelling.

## Representation And Complexity

The hot element record stores only the immutable source `ElementNode` reference,
three optional selector links (parent, previous sibling, and next sibling), and
the direct-text range. Source ID, local name, namespace, and attributes are read
through that immutable element reference; selector ID and first-element-child
lookup are derived from checked preorder record position. A first-child field
and direct-child count are not stored because the required presence/iteration
contract does not need them.

On the supported 64-bit target this shape currently measures 40 bytes per
element before vector allocator overhead. At one million elements that is about
38 MiB for element records. Each direct-text reference is another 16 bytes on a
64-bit target. A target-gated test enforces a reviewed upper bound of 48 bytes,
not an exact private `repr(Rust)` layout. Rust struct layout is not a public ABI
contract.

Complexity is:

- preflight plus materialization: `O(N + T)` time
- projection storage: `O(E + T + D)` for element records, direct-text
  references, and traversal depth
- document element, parent, previous sibling, next sibling, and direct-child
  presence: `O(1)`
- direct element-child iteration: `O(k)` with `O(1)` setup
- direct text-child iteration: `O(t)` with `O(1)` range setup
- ordered attribute iteration or effective lookup: `O(a)` without allocation
- integration-only source DOM ID to selector ID mapping: `O(E)` per query; the
  distinct identity domains are mapped explicitly without adding a source-ID
  index in AF4b

Existing allocation and performance guardrails remain in force. AF4b does not
add selector caches, bloom filters, source-ID indexes, or invalidation indexes.

## Error Propagation

Every authoritative production API that constructs a projection preserves
typed build failures:

- selector matching/debug convenience construction returns
  `SelectorDomBuildError`
- cascade document and cascade-input resolution wrap it in
  `StyleResolutionError::SelectorDomBuild`
- incremental cascade constructs before any genuine `Ok(None)` reuse fallback
- an incremental-eligible computed plan with no retained artifacts runs the
  same bounded selector-DOM preflight before reporting
  `IncrementalUnavailable`; this validates structure, representation, and the
  styled-element budget without materializing and discarding a full index
- computed style preserves cascade's wrapped error or uses its own typed
  selector-DOM build variant when it constructs independently
- style-tree reconstruction propagates its independent build failure
- Browser style/cache/frame/render-debug callers propagate the existing typed
  CSS error chain

A selector-DOM build failure is never converted into:

- ordinary selector no-match
- invalid or unsupported selector state
- `Ok(None)` incremental-unavailable state
- a default document mode
- an empty selector projection
- a successful debug string containing an error message

Document-level debug convenience APIs therefore return `Result<String, ...>`.
Snapshot methods on an already successfully built index may remain infallible
apart from internal formatting assertions.

## Legacy `attach_styles` Compatibility Exception

`attach_styles(...)` retains its documented compatibility-only degradation
contract and unit return type. It is not an authoritative cascade, computed-
style, or Browser API.

The bridge explicitly chooses document construction for a `Node::Document` and
element-subtree construction for its historically accepted `Node::Element`
input. A leaf root, selector-DOM build failure, style-resolution failure, or
legacy projection mismatch clears all stale legacy style vectors and returns
without fabricating a partial result. This is a deliberate compatibility
exception, not an accidental swallowed authoritative error.

The bridge does not regain ownership of selector matching or cascade. Retiring
it after all compatibility consumers migrate remains separate follow-up work.

## Deterministic Debug And Regression Contract

Selector-DOM snapshots identify their projection mode and report document-
element identity independently from parent relationships. They serialize, in
deterministic element order:

- CSS-local element ID
- element namespace and canonical local name
- parent, previous sibling, next sibling, and direct element children
- ordered namespace/local-name/value attribute facts
- exact escaped direct-text entries grouped by owner

AF4b advances both the standalone selector-DOM snapshot and the integrated
selector-matching snapshot that embeds its body to `version: 3`.

The global direct-text arena order is an implementation detail and must not be
described as whole-DOM text order. Invalid construction has no successful
selector-DOM snapshot.

Regression coverage includes parser-created document identity; explicit
subtree semantics; invalid root kinds; zero, one, and multiple document
elements; nested documents; ID/capacity seams; text/comment/PI sibling
boundaries; HTML/SVG/MathML names; qualified attributes; exact interleaved
direct text; template-associated and actual ordinary host children; existing
combinators; cascade/computed/incremental/Browser propagation; deterministic
snapshots; fuzzing; benchmarks; and allocation/performance guards.

## Deliberate Exclusions

AF4b does not implement:

- `:root`
- `:empty`
- `:first-child`, `:last-child`, `:only-child`, or `:nth-child()`
- new selector parsing or specificity
- selector invalidation expansion
- broad HTML/DOM validation
- a Browser/runtime-owned selector provider
- Layout or Paint changes
- runtime DOM mutation or JavaScript DOM APIs
- selector caches, bloom filters, or unrelated performance indexing
- a rewrite of the right-to-left combinator matcher
- retirement or redesign of the legacy style bridge

These remain later work. AF4b supplies the neutral, invariant-safe, fallible
query foundation without claiming broader selector or Milestone AF completion.
