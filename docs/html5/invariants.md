# HTML5 DOM And Patch Invariants

Last updated: 2026-03-27  
Scope: `crates/html/src/html5/tree_builder` (`feature = "html5"`)

This document is the K1 contract for structural DOM-state checks and
`DomPatch` batch checks used by tests, fuzz targets, and strict integration
drivers.

Related contracts:
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/ae2-parser-created-dom-node-model.md`](ae2-parser-created-dom-node-model.md)

## Goals

- Keep HTML5 tree-builder output panic-free and deterministic under malformed input.
- Make DOM shape assumptions explicit and machine-checkable.
- Validate patch batches against a concrete pre-batch DOM state.
- Reuse the same checks in tests and future fuzz targets.

## DOM Invariants

These are checked by `check_dom_invariants(dom)` over
`html::html5::tree_builder::DomInvariantState`.

Allowed baseline:
- an empty state is valid before the first `CreateDocument`
- otherwise the state must be rooted

Required invariants:
- the tree is acyclic
- there is at most one document node, and it is the declared root
- the root, when present, is a document node and has no parent
- every non-root node has exactly one parent
- every parent/child edge is bidirectionally consistent
- child lists contain no duplicate node references
- every referenced parent and child exists
- sibling order is explicit and preserved by the stored child vector
- only document and element nodes may have children
- doctype, text, and comment nodes are leaves
- a doctype node, when present, is a direct child of the document
- at most one doctype child is present
- the doctype child appears before the first document element child

Operational interpretation:
- "children order is stable" means the DOM state carries one concrete child
  order per parent and the checker rejects duplicate or contradictory sibling
  references that would make that order ambiguous
- detached non-root nodes are invalid final DOM state

## Patch Batch Invariants

These are checked by `check_patch_invariants(patches, dom_state)`.

Baseline rules:
- `dom_state` itself must already satisfy `check_dom_invariants`
- patch validation is batch-scoped and order-sensitive

Required invariants:
- `PatchKey::INVALID` must never appear
- `Create*` operations must introduce a fresh key before any later reference
- `Clear` may only appear as the first patch in a batch
- a `Clear` batch must re-establish a rooted document by the end of the batch
- `AppendChild` / `InsertBefore` must reference existing nodes
- structural parents must be container nodes (`Document` or `Element`)
- `InsertBefore.before` must already be a child of the specified parent
- move/reparent operations must not move a document node
- move/reparent operations must not move the document root element
- move/reparent operations must not create ancestor cycles
- `RemoveNode` must target a live attached node or the root
- `SetAttributes` only targets element nodes
- `SetText` and `AppendText` only target text nodes
- the final post-batch DOM state must satisfy the DOM invariants above

## API Surface

Current checker entrypoints:
- `check_dom_invariants(&DomInvariantState) -> Result<(), DomInvariantError>`
- `check_patch_invariants(&[DomPatch], &DomInvariantState) -> Result<DomInvariantState, PatchInvariantError>`
- `Html5TreeBuilder::dom_invariant_state() -> DomInvariantState`

Recommended usage:
1. capture the builder state before a batch
2. run the parser step and collect emitted patches
3. call `check_patch_invariants` with the pre-batch state
4. compare the returned post-batch state to `builder.dom_invariant_state()`

This keeps the emitted patch stream and the builder's internal live tree under
the same structural contract.

## Internal Live-Tree Boundary

`LiveTree` inside the HTML5 tree builder is an internal structural mirror, not a
public typed validator.

Contract:
- it mirrors structural edits that the tree builder already considers authoritative
- it is assertion-based by design
- a `LiveTree` panic indicates a Borrowser tree-builder bug, not malformed HTML
- fuzzers and invariant-aware tests should rely on `check_dom_invariants` and
  `check_patch_invariants` for typed failure reporting

## AE11 Expanded-Name Stack Cache

- the stack of open elements is bound to one `NameInterner` domain;
- `ExpandedNameKey` is an opaque `(ElementNamespace, NameAtomId)` projection of
  the canonical `ExpandedElementName`, never an independent semantic identity;
- stack entries, cache updates, and exact-name lookups must use atoms from that
  bound domain, and cross-domain input is rejected before mutation;
- `name_counts` is a `Vec<(ExpandedNameKey, usize)>`, so lookup and update are
  linear in the number of distinct expanded names currently on the stack;
- missing entries and count underflow are engine invariant failures;
- cache-assisted and scan-based scope answers must agree after push, pop,
  replacement, suffix removal, table clearing, adoption-agency mutation, and
  malformed recovery;
- the accepted representative mixed-content measurement is 5.490707 scan steps
  per operation against an approved maximum of 32;
- the deep-distinct adversarial measurement is approximately 384.751460 scan
  steps per operation at distinct-name/depth 770, honestly reflecting the
  linear representation, but it did not regress against the immutable baseline;
- `max_open_elements_depth = 1024` is the default parser resource ceiling.

Crossing the representative scan threshold or the approved ordinary/mixed
baseline-regression limits is a mandatory architecture-review stop. A later
indexed implementation must retain the one canonical expanded-name/interner
identity rather than introduce a second semantic key.

## AE8 Table Parser-State Invariants

AE8 adds parser-state invariants for the supported table tree-construction
subset. These invariants are internal regression contracts, not public runtime
APIs.

Pending table-character state:

- `InTableText` owns one pending table-text state containing both the
  table-character buffer and recorded original table-family insertion mode.
- the recorded mode inside that state must be one of `InTable`, `InTableBody`,
  or `InRow`;
- entering `InTableText` while pending table-text state already exists is an
  internal invariant violation and must not replace or discard the active state;
- an `InTableText` mode without pending table-text state is an internal
  invariant violation, not a recoverable HTML parse error;
- leaving `InTableText` through a non-character token or EOF must flush the
  buffer and take the return mode before reprocessing that token;
- the pending buffer and recorded return mode must both be clear together after
  the flush;
- parser finalization through EOF must not leave pending table-character state.

Foster-parenting state:

- foster parenting resolves an `InsertionLocation` before text or element
  insertion;
- the location is either append-to-parent or insert-before-anchor;
- when the relevant table has a live parent, the emitted structural patch for a
  newly created foster-parented node must use that parent and the table as the
  `InsertBefore.before` anchor;
- when the relevant table has no live parent, the fallback parent is the stack
  entry immediately above the table;
- foster parenting must not use `RemoveNode`, DOM rebuilding, post-parse
  normalization, runtime repair, or layout ancestry.

Table stack operations:

- table-scope checks are read-only stack probes;
- table-context, table-body-context, and table-row-context clearing pop parser
  stack entries only;
- stack clearing does not remove, detach, or reorder DOM nodes;
- implied table wrappers are real parser-created DOM nodes with fresh
  `PatchKey` identities.

Table cell and AFE interaction:

- entering `td` or `th` pushes an AFE marker after inserting the cell;
- closing a cell, explicit or implied, clears AFE entries back to the last
  marker;
- cell close recovery must not expand into unrelated adoption-agency behavior.

## AE9a Form And Void-Insertion Invariants

- A form pointer is absent or identifies one successfully parser-created form
  `PatchKey`; clearing it never removes a DOM node.
- Form end processing clears the pointer before scope validation and exact stack
  removal; a failed validation leaves it clear.
- Exact-key stack removal preserves other entry order, counts, caches, and DOM.
- Pending textarea initial-LF state is valid only for the active textarea RCDATA
  entry and is cleared by first text consumption, non-text handling, or text-mode exit.
- A void insertion restores retained stack length/order after one real push/pop;
  high-water records the transient observed depth.
- start-tag dispatch finalizes every original self-closing flag exactly once
  for the AE9a/AE9b semantic-insertion paths:
  an unacknowledged AE9 non-void flag records its trailing-solidus error after
  the tag-specific recovery error, including a recoverably ignored token.
- The frozen deprecated insertion helper retains pre-AE9 skip-stack behavior;
  only AE9 semantic void insertion changes stack-transition observability.

## AE9b Current Select Invariants

- `select` is a shared general-scope boundary; button/list-item scope inherit
  it and table scope remains unchanged.
- supported implied-end tags are exactly `p`, `li`, `option`, and `optgroup`;
  exception handling is shared rather than handler-local popping.
- generic InBody end tags perform one reverse SOE scan. Normal outcomes are a
  stable matched entry or an earlier HTML special barrier; exhausting a rooted
  full-document SOE is `EngineInvariantError`.
- generic end-tag scan calls/steps use dedicated counters and do not increment
  scope-scan counters.
- one HTML-namespace 83-name special-category taxonomy serves AAA and generic
  end-tag scanning. Foreign namespaces remain unsupported.
- foster parenting changes the adjusted insertion location only while enabled
  and the current target is `table`, `tbody`, `tfoot`, `thead`, or `tr`.
- select/input/HR and generic-end recovery mutates SOE only; it never emits
  `RemoveNode` or repairs the materialized DOM.
- mandatory stack recovery remains committed when a later semantic insertion
  is rejected by resource limits.
- state snapshots and progress witnesses contain no select insertion mode,
  return mode, remembered select flag, or runtime select-control state.

## AE10 Template Invariants

- A supported parser-created template host is an ordinary element with exactly
  one typed `TemplateContents` fragment; it has no parser-created ordinary
  children.
- The association is one-to-one while live, is not an ordinary edge, and its
  independently stored arena endpoints agree. A hosted fragment has no
  ordinary parent.
- Every independent live/validation/runtime arena records the explicit
  `TemplateContents` fragment kind, and materialization rejects a kind mismatch.
- Removing a host or ordinary ancestor removes the associated fragment
  subgraph. Direct hosted-fragment removal, ordinary parenting, duplicate
  association, and re-association are rejected atomically.
- Open parser-created template keys, owner-aware template-mode entries, and
  live contents associations have identical nesting order. The top template
  mode belongs to the innermost open template.
- Each accepted template start inserts one typed diagnostic AFE marker.
  Formatting-boundary, caption, table-cell, and template markers are equivalent
  algorithmic boundaries; kind/owner metadata never changes the exactly-once
  last-marker clear performed by template close and each EOF unwind.
- Reset-insertion-mode stops at the active template boundary and uses its
  owner-matched current template mode rather than inferring state below the
  boundary.
- The exact final insertion parent's child vector is reserved before template
  start commit. A rejected reservation or start changes no patch, key, counter,
  live node, SOE,
  AFE, template mode, insertion mode, frameset state, text-coalescing state, or
  association.
- Child-insertion reservation reports typed failures. Structural endpoint or
  arithmetic failures are engine invariants; only allocation denial is handled
  as the template-start resource-limit parse error.
- Template validation epochs and accepted-template counts advance with checked
  arithmetic before mutation. Overflow cannot wrap into an earlier fast-path
  identity or partially commit a transition.
- The template stack stores only the narrow `TemplateInsertionMode` set; a
  general non-template mode cannot enter through its owning API.
- Same-token compact fingerprints exclude patch history and only select a
  collision bucket. Exact modes, SOE keys, typed AFE entries, template modes,
  pointers, bounded allocation state, and pending-table state decide equality.
  Fingerprint collision alone is never an invariant failure. An exactly
  repeated state is an invariant failure even after patch emission or
  idempotent mutation; a separate bounded progress measure must advance.
  Dedicated EOF recovery retains no per-template exact snapshots, uses O(1)
  auxiliary recovery memory, and decreases open-template depth exactly once per
  iteration.
- Per-token production validation is O(1) when template state is untouched and
  transition-local otherwise: accepted start, mode replacement, close, and EOF
  inspect only their bounded suffix/top/depth/reset evidence. Complete ordered
  SOE/AFE/template/live-model audits are test/fuzz/invariant work, not
  unconditional production work.
- EOF counter tests at depths 16 and 256, including nested table/template state,
  require one close and owner scan per template, owner-scan steps equal SOE
  entries removed, no added scope scans, and linearly bounded reset scans.
- Full-model traversal exposes fragment identity and children in host/fragment/
  ordinary-child order through the centralized typed visitor. Active-document
  traversal does not cross the
  association; Layout and retained rendering suppress the typed host, and
  Paint receives no artifact.

## AE11 namespace and foreign-content invariants

- `NodeId`/materialized `Id` and `PatchKey` are stable numeric identity domains;
  neither encodes namespace or local name.
- Every parser-created element has one typed `ElementNamespace`; semantic
  element comparison uses namespace plus exact interned local name.
- Stack expanded-name cache keys contain namespace and a domain-bound exact
  name atom. The stack rejects a foreign interner domain before mutation, and
  invariant/fuzz lanes prove cached counts agree with live stack entries and
  live-tree expanded names after replacement, removal, and recovery.
- Parser-created attributes are valid by construction. Semantic identity is
  namespace plus local name; prefix is serialization information; every value
  is a string; stored encounter order is preserved end to end.
- Exact DOM/patch/snapshot equality is attribute-order-sensitive. Noah's Ark
  attribute equality is order- and prefix-insensitive but namespace-, local-
  name-, and value-sensitive.
- Foreign dispatch is a per-token decision over a semantic adjusted-current-
  node view. The active insertion mode remains HTML-owned. The tokenizer may
  query only that view's namespace for the CDATA markup-declaration boundary.
- Namespace selection precedes and is independent of template contents,
  foster parenting, and other adjusted insertion locations.
- Breakout reprocesses the exact token only after a stack change establishes
  progress. Foreign end-tag scanning never corrupts stack caches.
- Unknown foreign elements retain their inherited foreign namespace. Results,
  patches, errors, and attribute order do not depend on chunk boundaries.
- Layout inability cannot alter DOM/style truth; unsupported SVG/MathML roots
  suppress complete box subtrees at the centralized Layout decision boundary.
