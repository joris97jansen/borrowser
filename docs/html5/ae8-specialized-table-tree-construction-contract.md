# AE8 Specialized Table Tree Construction Contract

Last updated: 2026-07-11
Scope: `crates/html/src/html5/tree_builder`
Status: normative contract for AE8 supported static table tree construction

Related contracts:
- [`docs/html5/html5-core-v0.md`](html5-core-v0.md)
- [`docs/html5/spec-matrix-treebuilder.md`](spec-matrix-treebuilder.md)
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/invariants.md`](invariants.md)
- [`docs/html5/tables-foster-quirks-contract.md`](tables-foster-quirks-contract.md)
- [`docs/html5/afe-aaa-contract.md`](afe-aaa-contract.md)

## Purpose

AE8 promotes Borrowser's supported table tree-construction subset from
robust generic handling to explicit parser-owned table-family insertion modes.

The contract is for parser-created DOM construction only. It defines how
supported table-related tokens mutate the stack of open elements, active
formatting markers, pending table-character state, insertion locations, and
`DomPatch` output.

AE8 is declared-scope support, not full WHATWG table parsing conformance.

## Ownership Boundary

HTML/parser owns:

- table-family insertion modes;
- table-specific stack-of-open-elements operations;
- implied table wrapper construction;
- pending table-character-token buffering and flushing;
- foster-parent insertion-location selection;
- malformed table recovery;
- deterministic tree-builder parse-error diagnostics;
- parser-created DOM patches and live-tree state.

CSS, Layout, Paint, browser/runtime, and accessibility code consume the
parser-created DOM shape. They do not repair table parentage, infer parser
state, choose foster parents, or reinterpret malformed table recovery.

## Supported Insertion Modes

AE8 supports these explicit insertion modes:

- `InTable`
- `InTableText`
- `InCaption`
- `InColumnGroup`
- `InTableBody`
- `InRow`
- `InCell`

The modes are reached from the existing document/body flow through `InBody`
start-tag handling for `table`.

`InTableText` is a temporary table mode. Entering it creates one coherent
pending table-text state containing the original table-family mode that
delegated character-token processing and the character-token buffer. When a
non-character token or EOF is seen, that state is taken atomically, the pending
character run is flushed, the recorded mode is restored, and the current token
is reprocessed in that mode.

An `InTableText` state without pending table-text state is an internal
tree-builder invariant violation, not malformed HTML recovery.
Entering `InTableText` while pending table-text state already exists is also an
internal invariant violation; it must not replace or discard the active state.

Closing a nested table resets the supported insertion mode from current SOE
context (`InCell`, row, row-group, caption, colgroup, table, head, or body)
rather than unconditionally selecting InBody. This is stack-derived and does
not add remembered return-mode state.

## Supported Table Elements

AE8 supports parser-created construction for:

- `table`
- `caption`
- `colgroup`
- `col`
- `tbody`
- `thead`
- `tfoot`
- `tr`
- `td`
- `th`

These are normal parser-created DOM elements with ordinary `PatchKey`
identity, first-wins attribute handling, live-tree state, and structural
patches.

AE9a adds the supported full-document `InTable` special paths for `form` and
hidden `input`. The parser owns form-pointer state: InTable form insertion sets
the pointer only after successful normal insertion, then removes that exact
current stack entry without DOM removal. Hidden input is a parser-created void
element. Template-specific branches remain deferred with deterministic fallback.

## Implied Element Construction

Supported implied table construction is parser-owned and uses the normal
element insertion path:

- `tr` directly in table context synthesizes an implied `tbody` and reprocesses
  the `tr` in `InTableBody`.
- `td` or `th` without an open row synthesizes an implied `tbody` where needed,
  then an implied `tr`, and reprocesses until the cell is inserted in `InRow`.
- `col` in table context synthesizes an implied `colgroup` and reprocesses the
  `col` in `InColumnGroup`.

Synthesized wrappers are real DOM nodes. They are not snapshot-only nodes,
layout boxes, or post-parse repairs.

## Stack And Scope Invariants

The stack of open elements is parser state, not the DOM tree.

AE8 preserves these distinct operations:

- table-scope checks inspect whether a tag is visible before a table boundary;
- table-context clearing pops parser stack entries until `html`, `table`, or
  `template`;
- table-body-context clearing pops until `tbody`, `thead`, `tfoot`, `html`, or
  `template`;
- table-row-context clearing pops until `tr`, `html`, or `template`.

Popping the stack of open elements never deletes already-created DOM nodes and
does not emit `RemoveNode`.

## Cell Handling And AFE Markers

Entering `td` or `th`:

- inserts the cell through the normal parser-created element path;
- pushes the cell on the stack of open elements;
- pushes an active-formatting marker;
- switches to `InCell`.

Closing a cell, whether explicit or parser-recovery-driven:

- finds the current table cell in table scope;
- performs the supported implied-end-tag stack cleanup by closing through the
  table-scope path;
- clears active formatting entries back to the last marker;
- switches to `InRow`.

AE8 does not introduce layout cell concepts into the parser.

## Pending Table Characters

`PendingTableTextState` is parser-owned state. It stores the original
table-family insertion mode plus `PendingTableCharacterTokens`, which stores
source-ordered owned chunks and whether any buffered character is non-space.

Lifecycle:

1. entering `InTableText` creates the complete pending table-text state
   atomically;
2. character tokens append resolved text chunks only to active pending
   table-text state;
3. a non-character token or EOF takes the complete state and flushes its buffer
   before reprocessing;
4. all-space runs are inserted in the table context;
5. runs containing non-space characters record a parse error and are inserted
   through body-style processing with foster parenting enabled;
6. the return mode and pending buffer are clear together after the flush;
7. parser finalization through EOF must leave no pending table-character state.

Whole-input and chunked-input runs must produce equivalent DOM shape, patch
semantics, parse-error output, and final parser state for supported table-text
cases.

## Foster Parenting

Foster parenting is an adjusted insertion-location decision made before text or
element insertion.

The insertion location is represented by:

```rust
struct InsertionLocation {
    parent: PatchKey,
    before: Option<PatchKey>,
}
```

`before = None` means append to `parent`; `before = Some(key)` means emit
`InsertBefore { parent, child, before: key }`.

For supported foster-parenting cases:

1. compute anchors from the stack of open elements;
2. if a template is above the relevant table, append inside that template;
3. otherwise find the last open `table`;
4. if the table has a live DOM parent, insert immediately before the table;
5. if the table has no live DOM parent, append to the element immediately above
   the table on the stack;
6. if no table anchor is present, fall back to the current insertion location.

Foster parenting applies to supported non-space table text and generic
non-table content routed through the `InTable` "anything else" branch.

AE9b clarifies adjusted-location use within that delegated body processing:
foster placement applies only while foster parenting is enabled and the
current insertion target is `table`, `tbody`, `tfoot`, `thead`, or `tr`. Once
delegated insertion makes an ordinary element such as `select` current, later
ordinary descendants use that current element. This is why a fostered select
is inserted before the table while its option remains its child.

The parser must not implement foster parenting by append-then-move,
insert-then-reparent, DOM rebuilding, runtime correction, layout ancestry, or
snapshot-only repair.

## Reprocessing

AE8 uses the existing bounded iterative dispatch loop.

Insertion-mode handlers return either:

- `Done`, meaning the token has been consumed; or
- `Reprocess(mode)`, meaning the same token is handled again in `mode`.

Reprocessing is intra-token, explicit, and bounded. Recursive redispatch is out
of contract.

## Supported Malformed Recovery

AE8 supports deterministic parser-owned recovery for representative malformed
table structures:

- cells directly under `table`;
- rows directly under `table`;
- row-group transitions while a row or cell is open;
- row starts while another row or cell remains open;
- cell starts while another cell remains open;
- table end tags that require unwinding cell, row, and row-group contexts;
- non-table text or elements in table context through foster parenting;
- nested `table` start tags in the supported recovery subset.

Unsupported or deferred table interactions must remain deterministic, preserve
parser invariants, and stay documented as outside the supported subset.

## Patch And Identity Rules

AE8 uses the existing `DomPatch` surface:

- `CreateElement`
- `CreateText`
- `CreateComment`
- `AppendChild`
- `InsertBefore`
- `AppendText`

No new move opcode is introduced. `AppendChild` and `InsertBefore` remain the
identity-preserving structural insertion/move surface for already-created
children. `RemoveNode` is not a legal temporary detach primitive for table
recovery or foster parenting.

The first structural insertion for a foster-parented newly created node must
already target the final foster parent and, where applicable, the final
`before` anchor.

## Observability

AE8 behavior is covered by:

- targeted tree-builder unit tests;
- DOM golden snapshots under `crates/html/tests/fixtures/html5/tables/dom`;
- patch golden snapshots under
  `crates/html/tests/fixtures/html5/tables/patches`;
- whole-input and chunked-input parity checks;
- deterministic tree-builder parse-error snapshot lines where fixtures opt in.

Parse-error output is an internal regression/debug surface, not a public
runtime rendering API.

## Non-Goals

AE8 does not implement:

- table layout;
- CSS table formatting;
- intrinsic table sizing;
- column width calculation;
- border collapsing;
- table painting;
- accessibility table semantics;
- interactive DOM mutation;
- form behavior;
- select-specific insertion modes (historical select modes are not current
  deferred work; AE9b handles select-family tokens through InBody);
- template insertion modes;
- SVG or MathML parsing;
- JavaScript;
- resource loading;
- full DOM APIs;
- unrelated expansion of the adoption agency algorithm.
