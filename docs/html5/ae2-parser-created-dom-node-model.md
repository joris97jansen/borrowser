# AE2: Parser-Created DOM Node Model

Last updated: 2026-07-01
Status: Milestone AE architecture contract
Scope: `crates/html/src/html5`, `html::Node`, parser-created DOM snapshots,
and downstream consumer handling

This document defines the parser-created DOM model that the HTML tree builder
constructs. It is an internal browser-engine data model for static HTML
parsing and rendering input. It is not a public DOM API contract.

Related contracts:

- [`docs/html5/ae1-html-parser-dom-ownership-contract.md`](ae1-html-parser-dom-ownership-contract.md)
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/invariants.md`](invariants.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/html5-core-v0.md`](html5-core-v0.md)
- [`docs/html5/ae12-processing-instruction-contract.md`](ae12-processing-instruction-contract.md)

## Ownership Boundary

HTML/parser owns parser-created DOM semantics. CSS, browser/runtime
materialization, Layout, and Paint consume the model and may ignore node kinds
that are irrelevant to their domain, but they must not reinterpret HTML parser
semantics.

`DomPatch` may carry parser-created DOM effects to a materializer. It is not an
independent source of truth for HTML semantics, and `DomStore` does not decide
doctype, attribute, or tree-construction behavior.

## Node Kinds

Parser-created DOM output uses explicit node kinds:

- `Document`: the root container for one parsed document.
- `DocumentType`: a parser-created doctype node with optional `name`,
  `public_id`, and `system_id`.
- `Element`: an element with a normalized element name and canonical stored
  attributes.
- `Text`: text data.
- `Comment`: comment data.
- `ProcessingInstruction`: an AE12 parser-created leaf with an exact target and
  separate data string.

`DocumentType` and `ProcessingInstruction` are real traversable internal nodes.
They are represented in
`html::Node`, `DomPatch::CreateDocumentType`, tree-builder live invariant
state, strict patch validation state, browser `DomStore`, and deterministic DOM
snapshots. Processing instructions use the corresponding typed
`DomPatch::CreateProcessingInstruction` opcode rather than comment, text, or
element storage.

`ProcessingInstructionNode` construction is checked in all build profiles.
The internal factory returns the shared parser-created PI validation error for
invalid target/data rather than relying on `debug_assert!`. PatchValidationArena
and Browser validate untrusted patch payloads before commit; Browser consumes
the same validator through `html::internal` and does not duplicate it.

The legacy `Document { doctype }` field and `DomPatch::CreateDocument {
doctype }` payload are compatibility metadata for older paths. HTML5
parser-created output emits `CreateDocument { doctype: None }` and represents
the doctype as a `DocumentType` child. Do not use the legacy document field as
doctype node identity.

## Document Mode

Document mode is parser/document metadata derived from doctype handling. It is
owned by HTML tree construction state, not by the `DocumentType` node's
identity.

Supported behavior:

- the tokenizer emits doctype token fields;
- the tree builder selects the supported document mode from those fields while
  in early bootstrap state;
- accepted initial doctypes can create a `DocumentType` node;
- document mode is not encoded into the `DocumentType` node payload;
- late or duplicate doctypes after the supported initial boundary do not mutate
  document mode or create a second document doctype child.

Future public DOM API exposure of `DocumentType` remains out of scope.

## Element And Attribute Representation

Every parser-created element stores an `ExpandedElementName` consisting of a
typed `ElementNamespace` and an interned, exact local name. The document-owned
`NameInterner` ASCII-folds HTML-tokenized input where required and retains
canonical mixed-case SVG adjustments and unknown foreign names exactly.

Parser-created attributes are an ordered `Vec<ParserCreatedAttribute>`. Each
entry stores a valid-by-construction `QualifiedAttributeName` (namespace,
canonical prefix shape, and exact local name) plus a `String` value. Valueless
syntax and explicit empty syntax both have DOM value `""`; source spelling is
not DOM state. Tokenizer duplicate removal is first-wins. Foreign adjustment
then performs deterministic first-wins hardening for synthetic collisions by
expanded attribute name without adding a non-standard HTML parse error.

The stored first-surviving encounter order is preserved through the live tree,
patches, `DomStore`, materialization, structural comparison, and snapshots.
Snapshots never sort the DOM attribute list. Noah's Ark/parser semantic
attribute equality is a separate order-insensitive comparison over
`(namespace, local name, value)`; prefixes do not participate there.

## Tree Invariants

Parser-created DOM tree shape is represented through ordered child lists plus
validator/live-tree parent maps. Explicit sibling pointers are not required in
the current model; sibling order is the child-vector order.

Required invariants:

- a materialized parser-created document has one root document node;
- the document root has no parent;
- every non-root live node has exactly one parent;
- parent/child edges are bidirectionally consistent in validator state;
- child lists contain no duplicate node references;
- the tree is acyclic;
- only `Document`, `Element`, and typed parser-created fragments may have
  children;
- `DocumentType`, `Text`, `Comment`, and `ProcessingInstruction` are leaf
  nodes;
- a `DocumentType` node, when present, is a direct child of the document;
- at most one `DocumentType` child is present;
- the `DocumentType` child precedes the first document element child;
- structural insertion order is deterministic.

Within the currently supported HTML5 tree-construction subset, the parser
constructs an `html` element as the document element and routes supported
`head` and `body` nodes beneath it. Full HTML DOM API document-element,
`head`, and `body` accessors are not implemented.

## Identity Domains

Parser-created node identity is stable within its own domain:

- `PatchKey` identifies nodes in the parser output patch stream.
- `html::internal::Id` identifies nodes in materialized `html::Node` trees.
- browser/runtime materialization may map `PatchKey(n)` to `Id(n)` as an
  implementation bridge.
- `RetainedRenderId` identifies retained render artifacts.

These domains are separate. Consumers must not rely on numeric equality across
`PatchKey`, `html::internal::Id`, materialized DOM store identity, and
`RetainedRenderId`.

`DocumentType` participates in parser-created and materialized DOM identity.
It does not create a retained render identity anchor because it does not
generate renderable content.

AE12 processing instructions likewise receive parser key, live-tree,
`DomStore`, and materialized DOM identities while receiving no retained render,
layout, paint, or stacking identity. Exact target case is preserved and is not
interned through the HTML element-name path.

## Consumer Rules

Consumers handle `DocumentType` explicitly:

- browser `DomStore` materializes it as a node;
- CSS selector indexing ignores it because selectors operate over element
  axes;
- CSS style-tree construction skips it during normal document/element child
  traversal because it is not a style input;
- stylesheet and form-control discovery ignore it;
- Layout suppresses it before box generation;
- Paint receives no paintable artifact for it.

None of these consumer behaviors transfer doctype semantics out of HTML/parser.

Consumers preserve AE12 processing instructions as typed non-element leaves
where the intermediate representation requires them. Selector indexing remains
element-only; Layout centrally suppresses PI box generation; Paint consequently
receives no PI-derived artifact. Consumers do not recognize PI syntax or
reinterpret target/data semantics.

## Non-Goals

AE2 does not implement:

- public DOM APIs or mutation APIs;
- JavaScript-facing DOM bindings;
- event behavior;
- `document.write`;
- custom elements;
- shadow DOM;
- resource loading;
- navigation;
- full DOM `DocumentType` API exposure;
- broad tree-construction conformance beyond the supported Milestone AE scope.
- complete DOM `ProcessingInstruction` APIs, including constructors,
  pseudo-attribute maps, attribute/CharacterData mutation, cloning, and public
  live mutation.

## AE10 Typed Parser-Created Fragments

AE10 extends the internal parser-created model with
`DocumentFragmentNode { id, kind: TemplateContents, children }`, physically
owned by the private `template_contents` field of the opaque `ElementNode`
payload in `Node::Element { element }`. The slot cannot hold
an arbitrary `Node`, has independent stable identity, and stores ordered normal
child edges. It stores no host ID; recursive ownership is authoritative and
prevents stale host references after `Node::set_id()`.

Arena-shaped representations store host and contents records independently and
therefore maintain validated bidirectional keys through one owning operation.
The association is neither an ordinary parent edge nor a hidden child vector.
Full-model traversal explicitly visits host, fragment and descendants, then
ordinary host children. Ordinary `children()` traversal never enters the
fragment. The internal type leaves room for later parser-created fragment kinds
without implementing public `DocumentFragment`, template `.content`, owner-
document/adoption, cloning, or mutation APIs.

The stable ordinary Rust surface uses `Node`/`ElementNode` accessors for name,
attributes, style, identity, and ordinary children. It cannot access or mutate
the association. With `internal-api`, engine crates receive controlled
canonical template construction and read-only fragment inspection only;
fragment mutation remains in the `html` crate (apart from a test-harness-gated
whole-model legacy ID transformation).

Ordinary-child mutation on this surface is retained legacy structural
behavior, not live DOM mutation or `HTMLTemplateElement.content`. Manually
inserted ordinary host children remain on the ordinary active tree and cannot
be converted into template contents through the public API. The controlled
cross-crate template constructor preserves such children only for generic or
legacy validated materialization; strict AE10 parser output supplies none and
is checked by the separate parser-output validator.
