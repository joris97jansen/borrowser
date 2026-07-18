# AE10 Template Tree-Construction Contract

Status: implemented Milestone AE contract

Normative HTML source: WHATWG HTML repository revision
`88ae68cb961651f0f92c5d2046049f53ecdfc6cf` (2026-07-11), sections
“The template element”, “The stack of template insertion modes”, “The rules
for parsing tokens in HTML content”, “The rules for parsing tokens in the `in
template` insertion mode”, “The stack of open elements”, and “Reset the
insertion mode appropriately”. The immutable repository source is normative;
live specification URLs are convenience links only.

Conformance evidence uses WPT revision
`2c705104a295c48053eeddf7fe0170d790a4e853`, file
`html/syntax/parsing/resources/template.dat`. Every adapted case records the
exact input bytes and SHA-256, source case, upstream errors and tree,
full-document context, scripting stance, and Borrowser representation
translation. Locally authored fixtures are labeled as local WHATWG-subset
tests, not WPT imports.

## Scope and ownership

AE10 implements static, full-document parsing of ordinary HTML `template`
elements. HTML owns the template insertion-mode algorithms and the
parser-created contents association. DOM patch validation and Browser
materialization own structural protocol enforcement. Active-document
consumers use ordinary-tree traversal and do not cross the association.

AE10 does not implement public `DocumentFragment`,
`HTMLTemplateElement.content`, the template contents owner-document rules,
adoption, cloning, fragment parsing, scripting, custom elements, declarative
shadow roots, shadow DOM, live mutation, resource activation, or rendering of
template contents.

## Parser-created node model

The canonical recursive materialized representation is:

```rust,ignore
enum ParserCreatedFragmentKind {
    TemplateContents,
}

struct DocumentFragmentNode {
    id: Id,
    kind: ParserCreatedFragmentKind,
    children: Vec<Node>,
}

pub enum Node {
    Element { element: ElementNode },
    // ...
}

pub struct ElementNode {
    // ordinary element fields are private and exposed through ordinary accessors
    template_contents: Option<Box<DocumentFragmentNode>>,
}
```

This is an internal parser-created fragment model, not a public DOM API.
`ElementNode` is an opaque public payload: ordinary consumers can inspect and
mutate the already-supported ordinary identity, name, attributes, style, and
ordinary-child surfaces through methods, but cannot obtain the association
slot. An external consumer cannot detach, replace, swap, or move template
contents independently of the complete host value.

Public ordinary-child mutation remains legacy structural behavior. It is not
live DOM mutation and is not `HTMLTemplateElement.content`. A caller that
manually adds an ordinary child to an associated template host creates an
ordinary active-tree child; the public API cannot convert that child into
template contents or detach, replace, or mutate the actual contents
association. Strict AE10 parser-created templates have no parser-created
ordinary host children.

The feature-gated, deliberately unstable `html::internal` interface is the
only cross-crate template-fragment boundary. It exposes read-only fragment ID,
kind, children, and host-association inspection, plus two controlled
constructors: one for an ordinary element and one that creates a canonical
template host and its typed contents together. It exposes neither a generic
fragment constructor nor fragment/association mutation. Fragment identity and
child mutation remain owned inside `html`; the only cross-crate renumbering
entry point is a `test-harness`-gated whole-model legacy test transformation.
`#[doc(hidden)]` is not used as an access-control claim.

The controlled template constructor is a generic/legacy validated
materialization boundary, not a strict-parser provenance marker. It always
creates one canonical `template` host and exactly one typed `TemplateContents`
fragment as a single recursive value; it cannot create a detached fragment, a
non-template host, or a wrong fragment kind. Its `ordinary_children` input
preserves structurally valid generic or legacy host children, which remain
ordinary active-tree children rather than fragment contents. Strict HTML5
parser materialization supplies an empty ordinary-child vector, and the
parser-output validator—not the tag name or this constructor—enforces that
strict guarantee.

In the recursive model, physical ownership through the typed element slot is
the authoritative association. The fragment does not redundantly store its
host ID, so changing the element ID cannot stale a fragment back-reference.
The Browser arena, HTML live tree, and generic patch validator store nodes as
independent records and therefore maintain explicit `host -> contents` and
`contents -> host` keys. One owning operation establishes, validates, removes,
clears, and rolls those two fields back as one semantic association.

Every independently stored representation also carries an explicit fragment
classification. The live tree, generic validator, Browser `DomStore`, strict
invariant model, and test applier distinguish
`DocumentFragment(TemplateContents)` (or the equivalent typed record) rather
than giving a generic fragment an implicit template-only meaning.
Materialization validates that classification before constructing the recursive
`DocumentFragmentNode`.

Checkable node-model invariants are:

- a supported parser-created template has exactly one contents root;
- a contents root has exactly one host while live, and a host owns at most one
  contents root;
- the host is an ordinary element in the ordinary document tree;
- the host-to-contents association is not a parent/child edge;
- the contents root has no ordinary parent and cannot be moved by ordinary
  child operations;
- fragment descendants use normal, ordered child edges and stable identity;
- parser-created direct children are not inserted on an accepted template
  host;
- full-model order is host, contents root and its descendants, then ordinary
  host children;
- ordinary-tree traversal visits the host and ordinary children only.

`Node::id()` and receiver-only `Node::set_id()` continue to address ordinary
nodes. The typed fragment has its own crate-owned ID accessor and mutation used
only by controlled legacy/test renumbering. Full-model missing-ID assignment, lookup, snapshot
traversal, and diff identity collection cross the association explicitly and
detect duplicate IDs across ordinary nodes, fragment roots, descendants, and
nested templates. Ordinary lookup does not cross it.

## Patch protocol and lifecycle

`DomPatch::CreateTemplateContents { host, contents }` is the only operation
that creates a template contents root. It atomically validates the existing
canonical `template` element host and a fresh contents key, creates the typed
fragment, and establishes both arena association fields. There is no detached
fragment create operation, association setter, or ordinary append between host
and fragment.

Generic structural validation owns key and endpoint validity, canonical host
kind/name, duplicate and re-association rejection, ordinary-parent
restrictions, combined association/child graph cycles, removal ownership,
reachability, clear/reset behavior, and batch atomicity. It intentionally
allows manually constructed or historical unassociated elements named
`template`; element name alone is not parser provenance.

The HTML5 parser-output validator additionally owns AE10 provenance and state:
every accepted template start has one association, uses no ordinary direct
parser children, agrees with the open-element/template-mode state while open,
and is never exposed half-associated. These checks run at completed-token
boundaries before patches may drain. EOF must leave no supported open template
context or template-mode entry.

Production parser-output validation is incremental and transition-local. Tokens
that do not mutate template state take an O(1) epoch fast path. Accepted starts
validate their bounded token patch suffix and newly pushed SOE, AFE, and
template-mode tops. Mode replacement validates only the current owner, narrow
mode, and explicit general-mode conversion. Close and each EOF unwind validate
the closed key, popped owner, one marker-clear proof, one-entry depth decrease,
and reset result. None of these production checks scans the complete SOE, AFE,
template stack, or historical host set. The complete order-sensitive
host/stack/marker audit is reserved for tests, fuzzing, explicit
parser-invariant builds, and diagnostics in those builds.

The template-state epoch and accepted-template count use checked arithmetic.
Overflow is an engine invariant error preflighted before structural mutation;
neither identity can wrap and accidentally reuse an O(1) validation checkpoint.

Lifecycle rules are:

- duplicate association and re-association are rejected;
- `AppendChild` and `InsertBefore` reject a contents root as an ordinary child;
- direct removal of a hosted contents root is rejected;
- removing a template host removes its contents root and fragment subtree;
- removing an ancestor recursively removes every associated fragment subgraph;
- `Clear` removes nodes and associations and resets the structural baseline;
- strict batch application is copy/validate/commit, so rejection exposes no
  partial mutation;
- materialization follows associations explicitly;
- legacy diff/reset traverses fragment identity explicitly; changing the
  association under a surviving host forces `Clear` plus rebuild, never live
  re-association.

## Atomic template-start transition

An accepted start is one parser transaction: resolve/canonicalize attributes,
compute the final adjusted insertion location, create host and contents keys,
emit host/association/ordinary insertion patches, update the live tree, push
the stack of open elements, insert an active-formatting marker, push the owner-
aware template mode, update frameset state, and enter `InTemplate`.

Before commit, the parser preflights document bootstrap, attributes, two-node
capacity, open-element depth, AFE/template/provenance storage, two contiguous
patch keys including overflow, patch/live-tree storage, association validity,
and combined graph validity. It also reserves the exact final insertion
parent's child vector before either key advances or any structural patch is
applied. That reservation accounts for append versus insert-before,
same-parent attachment, foster parenting, fragment parents, and checked
arithmetic. Commit therefore cannot allocate while attaching the host.
Reservation failures are typed: invalid parents, invalid `before` relationships,
invalid existing children, and arithmetic overflow are engine invariants, while
only allocation/resource denial records the resource-limit parse error.
`with_structural_mutation` merely groups this preflighted commit and is not
claimed as rollback. A deterministic test reservation-failure seam proves that
rejection leaves patches, live/materialized tree, key allocator, counters, SOE,
AFE, template modes, insertion mode, frameset state, text-coalescing state, and
associations unchanged.

## Template parser state and dispatch

`InsertionMode::InTemplate` and an explicit stack of owner-aware
`TemplateModeEntry { owner: PatchKey, mode: TemplateInsertionMode }` are parser
state. `TemplateInsertionMode` contains only `InTemplate`, `InBody`, `InTable`,
`InColumnGroup`, `InTableBody`, and `InRow`, and converts explicitly into the
general mode. `Initial`, `BeforeHead`, `Text`, `AfterBody`, and all other
general-only modes are unrepresentable in the template stack.
The stack starts empty. An accepted start pushes `InTemplate`; supported table,
column-group, table-body, or row tokens replace the top owner entry, switch the
active mode, and reprocess the same token. Normal close or EOF pops one owner
entry. A saved head-element pointer supports the pinned `AfterHead` delegation.

Shared template dispatch is reached from `InHead`, `AfterHead`, `InBody`, the
supported table-family modes, and `InTemplate`; per-mode modules do not
duplicate the algorithms. The `InTemplate` token categories follow the pinned
rules: characters, comments, and supported head-related tokens delegate to
`InBody`/`InHead` behavior as specified; doctype is a parse error and ignored;
table/column/body/row starts replace the template mode and reprocess; other
starts replace with `InBody`; template end tags use shared close; EOF uses
template recovery.

Adjusted insertion-location calculation redirects insertions whose target is a
template host to its contents root. That applies to elements, text, comments,
reconstructed formatting elements, foster-parented nodes, and supported AAA
moves. Foster parenting chooses an active template boundary when it is above
the relevant table, preserving AE8 table behavior outside templates.

Reset-insertion-mode scans the SOE from the current node downward. On reaching
the active template boundary it uses the current owner-matched template mode
and does not infer body, table, or select state from nodes outside that
boundary. AE9b remains the current `InBody`-based select-family subset; AE10
does not introduce `InSelect` or `InSelectInTable`.

## Closing, AFE markers, and EOF

For a matching template end tag the parser verifies template scope, generates
supported implied end tags, pops SOE through the template, calls the existing
`clear_to_last_marker()` exactly once, pops one owner-matched template mode,
and resets insertion mode. An unmatched end tag records a deterministic parse
error and is ignored.

AFE markers carry typed diagnostic metadata: formatting boundary, caption,
table cell, or template, plus an optional owning patch key. Marker kind and
owner participate in snapshots, digests, cycle identity, and invariant
diagnostics only. Every marker remains the same stopping boundary; metadata
does not change the pinned last-marker algorithm. The parser does not search
for the marker associated with the closing template and does not repeatedly
clear markers.
Consequently malformed table/cell/template input can leave an older diagnostic
marker after a newer marker is cleared; this is not repaired with an invented
cleanup algorithm. The pinned case
`<table><thead><template><td></template></table>` is regression evidence for
this exact behavior.

At EOF, pending table text is flushed first. A dedicated recovery loop records
the template EOF parse error, unwinds exactly one innermost open template,
clears through the last AFE marker once, pops one template mode, resets the
mode, and verifies that open-template depth decreased by exactly one. It uses
O(1) auxiliary recovery memory. Counters covering depth 16, depth 256, and
nested table/template contexts prove aggregate linear work: one close, one
owner scan, and one reset per template; every owner-scan step corresponds to an
SOE entry removed; EOF adds no scope scans; and total reset scan steps are
linear in template depth. The supported EOF path is therefore O(template depth
plus SOE entries unwound) time and supports configured parser depth rather than
an incidental dispatch count.

## Reprocessing and progress

There is no fixed same-token dispatch budget. Reprocessing separates a compact,
deterministic fingerprint, exact semantic equality, and a bounded progress
measure. The fingerprint selects a collision bucket only. Each bucket stores
exact states containing modes, exact SOE keys, exact typed AFE diagnostics,
exact template owner/modes, bounded key/node state, parser pointers, frameset
state, pending table text, and other same-token state. A cycle is rejected only
when exact equality succeeds; forced-collision tests prove distinct states are
retained. Patch-vector length and emitted patch history are deliberately
excluded, so patch emission or idempotent mutation cannot hide an otherwise
equal state.

For ordinary same-token reprocessing, constructing one exact state is O(S) time
and memory where S is current bounded parser-state size. R distinct retained
states use O(R*S) memory; lookup is O(S) per candidate in the selected collision
bucket, with a worst-case O(R*S) lookup under total fingerprint collision. Both
R and S are bounded by finite modes and configured parser limits. EOF performs
its separate depth-decreasing loop before retaining a cycle state, so it does
not store one exact snapshot per open template and keeps O(1) auxiliary recovery
memory. Template state remains present in snapshots, progress witnesses,
deterministic debug output, fuzz digests, schema versions, and
mutation-sensitivity tests.

Coordinated parser invariants are:

- no template-mode entry exists without a corresponding supported open
  template context;
- every open parser-created template has a valid typed association;
- template-mode owners exactly match open template keys in nesting order;
- the top mode belongs to the innermost open template;
- accepted starts add one AFE marker and close/EOF uses exactly one last-marker
  clear per template algorithm step;
- normal close, malformed close, resource rejection, EOF, and completed token
  validation cannot leave SOE, associations, template modes, AFE diagnostics,
  or active insertion mode in a half-committed state.

## Traversal and inertness

Parser/debug snapshots and full-model diff/materialization expose the host,
typed association, fragment identity, ordered contents, and nested templates.
The recursive model has one centralized safe full-model preorder visitor used
by identity collection, diff preparation, snapshots, complete audits,
performance counters, and lookup; missing-ID assignment uses the same order
with a separately documented mutable traversal safety proof. The visitor
reports fragment roots as typed entries and association containers, never as
ordinary children.
Ordinary document axes expose the host but do not cross the association. CSS
selector/style traversal, stylesheet/resource/base/metadata discovery,
visible-text collection, and form-control initialization use those ordinary
axes and therefore do not activate contents.

The template host itself is suppressed in Layout through the typed
parser-created association before principal-box generation. Browser retained
render identity traversal assigns identity to neither the host nor contents.
Paint receives no host or contents artifact and contains no template tag-name
special case. Contents remain stored and debuggable; inertness is not achieved
by discarding them or by adding fragments to generic `children()` traversal.

## Preserved interactions and exclusions

AE10 preserves AE8 table modes and foster parenting, AE9a form-pointer
behavior (including the pinned form-in-template exception), AE9b's current
`InBody` select-family subset, and the existing supported AFE/AAA behavior.
It retires generic-element template insertion, foster parenting to the host,
and the old table fallback only where the pinned template path applies.

No claim is made for full template APIs, declarative shadow DOM, shadow roots,
custom-element reactions, scripts or resource loading in contents, live DOM
mutation, navigation/event loops, a UA stylesheet system, full select modes,
or rendered template contents.
