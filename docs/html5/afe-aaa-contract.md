# HTML5 AFE + AAA Contract (Milestone H)

Last updated: 2026-03-16  
Scope: `crates/html/src/html5/tree_builder` (`feature = "html5"`)  
Status: normative implementation contract for Milestone H; current repository code may still be partial

Related contracts:
- [`docs/html5/spec-matrix-treebuilder.md`](spec-matrix-treebuilder.md)
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)

This document defines Borrowser's implementation contract for HTML5 active formatting elements (AFE), reconstruction of active formatting elements, and the adoption agency algorithm (AAA).
It is the scope and invariants document to implement against before feature code lands.

## Definitions

- "Present on SOE" means there is a live stack-of-open-elements entry whose element identity references the same live DOM node as the AFE entry's `PatchKey`.
- SOE presence is identity-based, not tag-name-based. A matching tag name with a different `PatchKey` does not satisfy "present on SOE".
- "Attribute-equal" means:
  - attribute names compare by `AtomId` equality
  - attribute values compare by exact string equality
  - `None` is distinct from `Some("")`
  - attribute lists must have the same length and the same encounter-order sequence

## Goals

- Implement HTML5 inline formatting recovery with deterministic behavior.
- Preserve SOE, DOM, and patch-stream invariants while handling mis-nested formatting markup.
- Define the exact formatting-element surface that Borrowser will support in Milestone H.
- Define the data model needed to reconstruct formatting elements across chunk boundaries without span-lifetime bugs.

## Non-Goals

- This contract does not change current shipping status by itself.
- This contract does not add table insertion modes, foster parenting, or template insertion mode stack behavior.
- This contract does not permit full-document rebuilds or `Clear`-based recovery for inline formatting errors.

## Supported Formatting Elements

Borrowser Milestone H treats the following HTML elements as formatting elements for AFE/AAA purposes:

- `a`
- `b`
- `big`
- `code`
- `em`
- `font`
- `i`
- `nobr`
- `s`
- `small`
- `strike`
- `strong`
- `tt`
- `u`

These tags are the Milestone H core set for both:

- insertion into the list of active formatting elements, and
- end-tag handling through the adoption agency algorithm.

### Special Start-Tag Paths In Scope

Milestone H explicitly includes the HTML5 special handling for:

- start tag `a`
- start tag `nobr`

These start-tag paths are not treated as generic formatting-element insertion.
They must follow the dedicated spec recovery behavior that interacts with existing AFE state before creating a new entry.

Contractual recovery behavior:

- start tag `a`: if an active `a` entry already exists after the last marker, Borrowser must run the prescribed recovery path against that existing `a` before inserting a new `a`
- start tag `nobr`: if a `nobr` element is already present on SOE in scope, Borrowser must run the prescribed recovery path before reinserting `nobr`

## Deferred / Explicitly Not Formatting Elements In Borrowser

The following are not inserted into AFE in Milestone H:

- any HTML tag not listed in `Supported Formatting Elements`
- inline phrasing tags such as `abbr`, `cite`, `dfn`, `kbd`, `mark`, `q`, `samp`, `span`, `sub`, `sup`, `var`
- custom elements
- foreign-content elements (SVG / MathML)

Notes:

- This is intentional. Borrowser will not invent a broader "inline formatting" category beyond HTML5 formatting elements.
- `applet`, `marquee`, and `object` are not formatting elements, but they do participate in marker handling in `In body` and therefore affect AFE boundaries.
- `font` is in scope only as an HTML5 tree-builder formatting element for AFE/AAA/reconstruction behavior. Legacy presentation semantics of `font` attributes are out of scope for Milestone H.

## Marker Scope

Markers are required sentinel entries in the AFE list.
In Milestone H they are inserted for the supported `In body` marker-producing paths:

- `applet`
- `marquee`
- `object`

Deferred marker-producing contexts:

- table-cell related markers (`td`, `th`) because table insertion modes remain deferred
- template-related boundaries because template insertion modes remain out of scope

Marker contract:

- markers have no `PatchKey`
- markers are never mirrored as DOM nodes or SOE entries
- markers partition duplicate checks, reconstruction scans, and AAA searches
- `clear-to-last-marker` removes entries after the last marker and then removes that marker
- if no marker exists, `clear-to-last-marker` clears the entire list

## Data Model

Milestone H AFE storage must use an explicit tagged entry model:

```rust
enum AfeEntry {
    Marker,
    Element(AfeElementEntry),
}

struct AfeElementEntry {
    key: PatchKey,
    name: AtomId,
    attrs: Vec<AfeAttributeSnapshot>,
}

struct AfeAttributeSnapshot {
    name: AtomId,
    value: Option<String>,
}
```

Representation requirements:

- `key` is the live DOM/tree-builder identity for the element node referenced by the AFE entry.
- `name` is the HTML tag atom stored redundantly with `key` so AFE comparisons do not depend on DOM lookups.
- `attrs` is an owned snapshot of tokenizer-normalized attributes.
- attribute names stay atomized (`AtomId`); attribute values must be owned strings or `None`.
- `TextSpan`-backed or resolver-epoch-backed attribute storage is forbidden in AFE entries.

Rationale:

- reconstruction and AAA may run after the source token batch that created the element has expired
- chunk-equivalence requires AFE state to be independent of temporary tokenizer span storage
- the Noah's Ark duplicate rule compares tag name plus attributes; that comparison must not depend on patch formatting or DOM serialization

## Attribute Snapshot Contract

Attribute snapshots used by AFE must preserve:

- tokenizer-normalized attribute names
- encounter order as delivered by the tokenizer
- first-wins duplicate-attribute semantics already applied by the tokenizer
- exact optional-value semantics (`None` vs empty string)

Duplicate matching for Noah's Ark uses:

- same tag name, and
- same attribute list length, order, names, and values

Formal comparison rules:

- tag name equality is `AtomId` equality
- attribute name equality is `AtomId` equality
- attribute value equality is exact string equality on owned values
- `None` and empty-string values are never interchangeable

Borrowser Milestone H does not use hash-map equality for duplicate matching.
Comparison order must be linear and deterministic.

## Noah's Ark Rule

Borrowser will implement the HTML5 "Noah's Ark clause" with the following exact rule:

1. When pushing a formatting element entry, scan backward from the end of AFE to the last marker.
2. Count entries whose `name` and `attrs` are equal to the candidate entry under the formal comparison rules above.
3. If fewer than three matching entries exist in that suffix, push the new entry.
4. If three matching entries already exist, remove the earliest matching entry in that suffix, then push the new entry.

Determinism requirements:

- the scan order is always from newest to oldest, bounded by the last marker
- the removed entry is always the oldest matching entry in the suffix
- no hash iteration or unstable container ordering is allowed

This is the only duplicate-bound policy permitted for Milestone H.
Any simplification weaker than "keep at most three matching entries after the last marker" is out of contract.

## Core Invariants

### AFE Structural Invariants

- AFE order is stable and significant.
- Non-marker entries refer only to element nodes, never text/comment/document nodes.
- A non-marker entry's `key` must reference a live node already created in the patch stream.
- AFE contains at most one live entry for a given `PatchKey`.
- Markers do not carry tag names, attributes, or node identities.

### SOE / AFE Cross-Invariants

- Every formatting element inserted into AFE must first exist as a DOM node and SOE entry.
- An element may remain in AFE after it is no longer on SOE only when the HTML5 algorithm explicitly permits that state.
- AFE must not retain dangling keys. If AAA or another algorithm removes or replaces a formatting element node, the corresponding AFE entry must be removed or replaced in the same token-bounded algorithm step.
- If an AFE entry's `key` is present on SOE, the SOE tag atom and AFE tag atom must match.
- Reconstruction must use identity-based SOE presence checks against `PatchKey`; tag-name-only probes are forbidden.

### Marker Invariants

- Marker insertion and marker clearing are explicit operations, never inferred from tag names already on SOE.
- Duplicate scans, reconstruction scans, and AAA searches never cross the most recent marker.
- Marker behavior is deterministic even when malformed input creates repeated marker-producing elements.

## Reconstruction Contract

Borrowser Milestone H must implement reconstruction of active formatting elements where required by the HTML5 `In body` algorithm within the supported scope.

Reconstruction behavior:

- starts from the last AFE entry that is neither a marker nor an element already present on SOE
- walks forward from that point to the end of AFE
- recreates each required formatting element in order
- allocates a fresh `PatchKey` for each recreated node
- creates the DOM node using the stored `name` and `attrs`
- pushes the recreated element onto SOE
- replaces the corresponding AFE entry in place with the newly created `PatchKey`

Determinism requirements:

- reconstruction order is oldest-missing to newest-missing
- recreated nodes use fresh monotonic keys; keys are never reused within the session
- the replacement happens in-place in AFE so later scans observe stable ordering

Milestone H reconstruction call sites include the supported `In body` branches that insert phrasing content through normal body insertion flow, including:

- character insertion paths
- generic phrasing/content insertion paths that require reconstructed formatting
- formatting-element start-tag paths
- the special `a` and `nobr` start-tag paths

Deferred reconstruction call sites remain deferred with their parent algorithms, notably table-family insertion modes and template modes.

## Adoption Agency Algorithm Contract

Milestone H implements AAA for the same core tag set listed in `Supported Formatting Elements`.

AAA contract:

- it is only entered for end tags whose tag name is in the supported formatting-element set
- search boundaries respect the last marker
- the algorithm operates deterministically over SOE and AFE without recursion
- the outer AAA loop is capped at 8 iterations, matching the HTML5 algorithm
- the inner ancestor-walk loop must be implemented as an explicit bounded scan over the actual SOE/DOM ancestor distance for the current token; it must not use unbounded retry logic
- the spec-defined inner-loop counter threshold at 3 is normative for the branch that removes/replaces entries while walking ancestors; Borrowser must not replace this with an arbitrary heuristic cap

Implementation-level requirements:

- the "formatting element", "furthest block", and "common ancestor" selections must be based on explicit, deterministic scans
- if the formatting element is absent from AFE, fallback is the normal `In body` end-tag path for that tag
- if the formatting element is in AFE but not on SOE, parse error recovery removes it from AFE deterministically
- if the formatting element is on SOE but not in scope, parse error recovery leaves DOM invariants intact and exits deterministically

Milestone H must not ship a tag-by-tag partial AAA subset inside the supported set.
Once the set above is declared supported, every listed end tag uses AAA.

## Patch Emission Contract

AFE reconstruction and AAA must emit `DomPatch` directly.
The implementation must not fall back to:

- rebuilding the whole document
- emitting `Clear`
- emitting ad hoc patch sequences that depend on chunking

Patch ordering rules for AFE/AAA work:

1. Any newly created replacement or reconstructed element must emit `CreateElement` before first structural reference.
2. Structural placement patches (`AppendChild` / `InsertBefore`) must appear in the same logical order as the algorithm's tree mutations.
3. `RemoveNode` may only be emitted for nodes that the algorithm actually detaches from the live DOM.
4. Unaffected node keys remain stable across recovery.
5. Recreated nodes always receive new keys; existing nodes that remain in the tree keep their keys.

Move/reparenting semantics:

- AAA may require identity-preserving node moves.
- Borrowser permits either of these runtime encodings, provided semantics are identical:
  - insertion patches (`AppendChild` / `InsertBefore`) on an already-parented node are defined as implicit reparenting
  - an explicit detach-then-insert sequence is used
- whichever encoding is used, node identity must be preserved across the move.
- move encoding must not change relative ordering semantics, and must remain deterministic under chunk-equivalent runs.

Chunk-equivalence requirements:

- whole-input and chunked-input runs must produce the same final DOM
- whole-input and chunked-input runs must produce patch streams with the same semantics and key-allocation order
- temporary tokenizer storage strategy (`Span` vs owned text) must not affect AFE/AAA outcomes

## Runtime Dependency Boundary

AAA requires structural reparenting semantics.
Milestone H therefore depends on a patch-application path that can represent the required tree mutations without violating node-identity rules.

Contractual consequence:

- if a required AAA tree mutation cannot be represented by the currently supported strict runtime patch semantics, Milestone H is blocked until the patch/runtime contract is upgraded
- Borrowser must not "fake" AAA correctness by clearing and rebuilding large subtrees or by silently changing key stability rules

This document therefore constrains both parser implementation and any supporting runtime patch work needed to make AAA patches materializable.

Canonical move encoding for runtime/application traces is intentionally left open in this contract and is tracked as follow-up [`docs/html5/issues/H2-runtime-patch-move-semantics.md`](issues/H2-runtime-patch-move-semantics.md).

## Parse-Error Observability Policy

AFE/AAA parse errors are part of parser control flow, but are not directly observable in patch output as standalone patch events.

Contract:

- parse errors may increment parser/session diagnostics and debug counters
- parse errors may influence recovery behavior and therefore final DOM/patch shape
- Milestone H does not introduce a dedicated `DomPatch` variant for parse-error reporting
- patch consumers observe parse-error handling only through the resulting deterministic DOM/patch stream
- numeric parse-error counts are not a public compatibility contract unless a later document promotes them

## Determinism Rules

The following are mandatory for Milestone H:

- no hash-based iteration in AFE duplicate detection, reconstruction selection, or AAA node selection
- no recursion in reconstruction or AAA dispatch
- no dependence on allocator addresses or pointer identity
- no dependence on batch-drain timing
- parse-error recovery choices must be tied to explicit scan order and explicit scope checks

For a fixed byte input and fixed tokenizer normalization behavior, the following must be identical between runs:

- AFE entry ordering
- SOE mutation ordering
- created `PatchKey` sequence
- emitted `DomPatch` order
- final DOM shape

## Test And Evidence Expectations

Milestone H implementation is not complete until all of the following exist:

- targeted golden fixtures for AFE markers, Noah's Ark duplicate handling, reconstruction, and mis-nested formatting recovery
- WPT formatting/adoption-agency slice wired into policy manifests with explicit `active`, `xfail`, or `skip` states
- chunk-parity tests proving whole vs chunked input yields identical DOM/patch semantics for representative AFE/AAA cases

Representative required cases include:

- mis-nested `b`/`i`
- nested duplicate formatting elements that exercise Noah's Ark eviction
- reconstruction after character insertion following interrupted formatting context
- marker-boundary isolation with `applet`, `marquee`, or `object`

## Change Control

The following changes require an explicit contract update before code lands:

- adding or removing supported formatting tags
- weakening the Noah's Ark duplicate bound
- storing non-owned attribute data in AFE
- using `Clear` or whole-subtree rebuilds as part of AFE/AAA recovery
- changing key-stability rules for reconstructed or adopted nodes
