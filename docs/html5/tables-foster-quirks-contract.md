# HTML5 Tables, Foster Parenting, and Quirks Contract (Milestone I)

Last updated: 2026-03-20  
Scope: `crates/html/src/html5/tree_builder` (`feature = "html5"`)  
Status: normative implementation contract for Milestone I; current repository code may still be partial

Related contracts:
- [`docs/html5/spec-matrix-treebuilder.md`](spec-matrix-treebuilder.md)
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/afe-aaa-contract.md`](afe-aaa-contract.md)
- [`docs/html5/adr/ADR-002-runtime-patch-move-semantics.md`](adr/ADR-002-runtime-patch-move-semantics.md)

This document defines Borrowser's implementation contract for HTML5 table-family
insertion modes, foster parenting, and tri-state document mode handling as they
apply to parser behavior in Milestone I.

It is the implementation target for landing table parsing in the current
HTML5 tree builder without weakening the existing `DomPatch`, identity, or
determinism contracts.

## Goals

- Implement the table-family tree-construction modes required for real-web
  compatibility.
- Define foster-parenting behavior as a direct `DomPatch`-emitting algorithm,
  not as a DOM-rebuild or post-pass repair step.
- Upgrade document mode handling from a boolean quirks flag to the spec-aligned
  tri-state model:
  - `NoQuirks`
  - `LimitedQuirks`
  - `Quirks`
- Preserve node identity, patch ordering, and SOE/AFE invariants under
  reparenting and implied table structure synthesis.

## Non-Goals

- This contract does not add template insertion-mode stack behavior.
- This contract does not add `InSelectInTable`.
- This contract does not add fragment-context-specific table bootstrap rules.
- This contract does not add foreign-content integration-point behavior inside
  table modes.
- This contract does not externalize document mode as a dedicated `DomPatch`
  event.
- This contract does not define CSS/layout quirks behavior; only parser-visible
  document-mode effects are in scope.

## Supported HTML Surface

Milestone I table-family element support is defined for the following HTML tags:

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

This milestone also covers misplaced content encountered while a table-family
mode is active, specifically:

- character tokens
- comments
- generic non-table HTML start tags
- generic non-table HTML end tags

The contract is HTML-namespace only. SVG, MathML, and template-specific table
interactions remain out of scope here.

## Supported Insertion Modes

Milestone I adds the following insertion modes to the supported parser surface:

- `InTable`
- `InTableText`
- `InCaption`
- `InColumnGroup`
- `InTableBody`
- `InRow`
- `InCell`

These modes are in scope together with the already-supported entry/exit modes
that reach them:

- `InBody`
- `Text` where reached from supported descendants inside table-family elements

### Required Entry/Exit Coverage

Milestone I support is not limited to holding these enum values. It includes the
spec-required transitions needed to reach and leave them deterministically:

- `InBody` start-tag handling for `table`
- `InTable` routing for `caption`, `colgroup`, `tbody`, `thead`, `tfoot`, `tr`,
  `td`, `th`, and table-misplaced content
- `InColumnGroup` handling for `col` and `colgroup`
- `InTableBody` handling for `tr`, `td`, and `th`
- `InRow` handling for `td`, `th`, `tr`, and row-group closure
- `InCell` handling for cell closure and body-style descendant processing
- return transitions back to the appropriate surrounding mode after closing
  caption/column-group/body/row/cell contexts

### Implied Table Structure Synthesis

Milestone I includes the spec-required implied wrapper creation needed for the
supported table surface. At minimum:

- missing row-group insertion when a `tr` is encountered directly from
  `InTable`
- missing row insertion when a `td` or `th` is encountered without an open `tr`
- reprocessing-driven synthesis rather than ad hoc fallback insertion

Synthesized elements are real DOM nodes and must follow the normal
`CreateElement` + structural-patch rules with fresh `PatchKey` allocation.

## Explicitly Deferred From Milestone I

The following remain deferred even after this contract lands:

- `InSelectInTable`
- template insertion modes and template table interactions
- fragment parsing that starts in a table-family context
- foreign-content parsing inside table-family modes
- any new patch variant for move/reparent operations

No implementation may silently treat these deferred areas as contractual by
reusing table-mode logic opportunistically.

## Foster Parenting Contract

Foster parenting is in scope for Milestone I and is required wherever the HTML
tree-construction algorithm routes table-misplaced content through the foster
parenting insertion location.

### Foster Target Selection

Borrowser must compute the foster-parenting target from the live SOE using the
HTML5 algorithm shape:

1. Find the last open `table` element on SOE.
2. If there is no open `table` element, insert into the current node as normal.
3. If that `table` has a DOM parent, insert immediately before the `table` in
   that parent.
4. Otherwise, insert into the element immediately above that `table` on SOE.

This target selection is identity- and stack-based. Name-only heuristics or
"nearest table-like ancestor" scans outside SOE are out of contract.

### Foster-Parented Token Classes

Milestone I requires foster-parenting behavior for:

- non-space character data flushed from `InTableText`
- generic start tags processed by the `InTable` "anything else" branch
- generic end tags processed by the `InTable` "anything else" branch
- any equivalent reprocessed body-flow token path entered with foster parenting
  enabled

Whitespace-only character runs handled by the dedicated table-text whitespace
path are not foster-parented unless the spec path for that token requires it.

### `InTableText` Buffering Contract

`InTableText` is not a streaming shortcut. It must preserve the spec buffering
behavior:

- "table-space characters" in this contract use the shared HTML space
  classification owned by
  [`crates/html/src/html5/tokenizer/scan.rs`](../../crates/html/src/html5/tokenizer/scan.rs)
  (`is_html_space` / `is_html_space_byte`)
- consecutive character tokens are buffered in source order
- buffering must be chunk-safe; whole input and chunked input must flush the
  same logical character sequence
- if the buffered run is all table-space characters, insert it using the
  table-text whitespace path
- if any buffered character is not a table-space character:
  - record a parse error
  - flush the buffered characters in source order through the foster-parenting
    path as if they were reprocessed via body-style insertion with foster
    parenting enabled

No implementation may emit patches for a partial buffered run and later revise
them after discovering a non-space character in the same logical run.

## Parse-Error Policy

Milestone I correctness is defined primarily by parser state, final DOM, and
`DomPatch` output, not by a public parse-error API surface.

Contractual policy:

- implementations must record the spec-required parse-error events internally
  where Borrowser already has parser error accounting machinery
- parse-error recording is normative internal bookkeeping, not best-effort
- Milestone I does not require a new public error surface or patch-level error
  event
- Milestone I tests may assert parser outputs and internal counters where such
  hooks already exist, but DOM/patch correctness remains the primary acceptance
  surface

At minimum, internal parse-error recording is required for the in-scope
table-mode misnesting and recovery paths that the spec marks as parse errors,
including:

- non-space `InTableText` flush through foster parenting
- conflicting caption/row-group/row/cell transitions that force implied close
  or mode-recovery behavior
- table-family end tags handled through the mode-specific error-recovery paths

## `DomPatch` Representation For Table Recovery And Moves

Milestone I does not introduce any new patch types. Table parsing and foster
parenting must use the existing structural contract:

- `CreateElement`
- `CreateText`
- `CreateComment`
- `AppendChild`
- `InsertBefore`
- existing `Set*` / `AppendText` operations where applicable

### Canonical Encoding Rules

- Newly created nodes must be created once, with their final `PatchKey`, before
  the first structural patch that references them.
- If the final foster target is known up front, the node must be inserted
  directly at that target.
- `InsertBefore { parent, child, before }` is the canonical encoding when the
  foster location is "before the table".
- `AppendChild { parent, child }` is the canonical encoding when the foster
  location is "inside the element immediately above the table on SOE" or the
  current node path resolves to append-at-end behavior.
- Identity-preserving reparent/reorder operations must use
  `AppendChild` / `InsertBefore` only.
- `RemoveNode` is never a legal temporary-detach primitive for foster
  parenting, table recovery, or any implied move.

### Parser Intent vs Runtime Materialization

- the parser is responsible for emitting structural patches that describe the
  intended final tree transition for the current algorithm step
- the parser must not depend on runtime-specific internal detach mechanics or
  any applier-local repair behavior
- the runtime/materializer is responsible for applying `AppendChild` and
  `InsertBefore` as identity-preserving moves when `child` is already parented,
  under the existing `DomPatch` and move-semantics contracts
- parser-side legality checks and runtime-side legality checks are complementary;
  neither layer may weaken the identity-preserving move contract

### Deterministic Move Rules

When a node already exists and the algorithm requires it to appear at a new
position:

- the node keeps its existing `PatchKey`
- unaffected siblings keep their relative order
- same-parent reorder and cross-parent reparent use the same canonical
  structural move encoding
- a no-op move to the already-correct position is permitted as a deterministic
  structural no-op, but implementations should avoid emitting redundant patches
  when the target position is already satisfied

### Text Coalescing Under Foster Parenting

Text coalescing remains parent-local and must use the actual foster parent, not
the table-context origin of the token:

- adjacent foster-parented text under the same resolved parent may coalesce via
  `AppendText`
- coalescing must not cross structural boundaries introduced by mode changes,
  implied table synthesis, or foster-target changes

## Quirks And Limited-Quirks Contract

Milestone I upgrades document mode from the current binary model to a tri-state
parser contract:

```rust
enum QuirksMode {
    NoQuirks,
    LimitedQuirks,
    Quirks,
}
```

### Ownership And Timing

- document mode is computed by the tree builder from the full DOCTYPE token
  payload:
  - `name`
  - `public_id`
  - `system_id`
  - `force_quirks`
- `force_quirks = true` always produces `Quirks`
- when `force_quirks = false`, the tree builder must still distinguish
  `NoQuirks` from `LimitedQuirks` using the spec doctype classifier; a
  boolean-only model is insufficient for Milestone I
- document mode becomes immutable once early bootstrap is left under the
  existing document-mode immutability contract
- document mode remains internal parser state; no `DomPatch` is emitted for it

### Quirks Classifier Source Of Truth

Milestone I requires one centralized DOCTYPE-to-document-mode classifier owned
by [`crates/html/src/html5/tree_builder/document.rs`](../../crates/html/src/html5/tree_builder/document.rs).

Contractual requirements:

- all `NoQuirks` / `LimitedQuirks` / `Quirks` decisions must flow through that
  single classifier
- no insertion-mode handler may hand-roll its own quirks detection logic from
  raw DOCTYPE fields
- the classifier must be a dedicated, reviewable implementation of the WHATWG
  DOCTYPE classification rules within Milestone I scope
- if Borrowser later adds a repo-local classifier table or generated data file,
  that artifact becomes part of this source-of-truth surface and must remain
  owned by `document.rs`
- focused classifier conformance tests must live with the tree-builder test
  surface that validates `document.rs` ownership, rather than being scattered
  across unrelated insertion-mode tests

### Parser Behaviors Affected In Milestone I

Milestone I deliberately keeps parser-visible document-mode effects narrow and
explicit.

In scope:

- `InBody` start tag `table` must honor the HTML5 quirks distinction for the
  implicit-`p` closure rule when a `p` element is in button scope:
  - `NoQuirks` and `LimitedQuirks` must close that `p` before inserting the
    `table`
  - `Quirks` must not force that `p` closure before inserting the `table`

Not otherwise distinguished in Milestone I:

- `LimitedQuirks` does not introduce a separate table-mode algorithm branch
  beyond the `table` start-tag behavior above
- for all other table-family branches in this milestone, `LimitedQuirks` is
  parser-equivalent to `NoQuirks`

This is intentional. The tri-state model is contractual now so later milestones
do not need to change the document-mode data model again.

## Required Recovery Cases

Milestone I does not inline full spec prose for every table-mode branch, but
the following recovery behaviors are mandatory and must be implemented
deterministically:

- entering a new `caption` while one is still open must resolve through the
  caption-closing recovery path before the new caption is inserted
- entering `tbody`, `thead`, or `tfoot` when a conflicting row-group is open
  must auto-close the current row-group through the mode-appropriate recovery
  path before switching
- entering `tr` while a cell is still open must auto-close the current cell
  before the row transition completes
- entering `td` or `th` while another cell is still open must auto-close the
  current cell before inserting the new cell
- closing a row-group while a row or cell is still open must resolve the inner
  structures first through the spec-aligned implied-close path

These cases are acceptance-critical because they exercise the interaction
between SOE mutation, AFE marker clearing, implied structure synthesis, and
identity-preserving patch emission.

## AFE And Cell-Boundary Interaction

Milestone I table-cell handling must integrate with the existing AFE contract.

Required interaction points:

- entering `td` or `th` inserts an AFE marker after the cell element is created
  and pushed
- closing a cell, whether explicit or implied by the spec cell-closing path,
  clears AFE back to the last marker
- cell-boundary AFE clearing must happen in the same token-bounded algorithm
  step as the corresponding SOE transition

This milestone does not relax any Milestone H AFE identity rules. Table-cell
markers extend them.

## Core Invariants

### Structural Invariants

- SOE entries refer only to already-created live element nodes.
- AFE entries refer only to live element nodes or markers.
- implied table synthesis never reuses an old `PatchKey`
- foster parenting never creates duplicate parentage
- document/document-root moves remain illegal

### Determinism Invariants

- whole-input, chunked-input, and seeded-fuzz chunk plans must converge to the
  same final DOM for in-scope table fixtures
- patch order for a fixed input and fixed chunk plan must be deterministic
- `InTableText` buffering must not depend on chunk boundaries
- chunk boundaries must not change whether foster parenting occurs for a given
  logical token sequence
- foster-target computation must depend only on current parser state and token
  stream, never on hash iteration or incidental container ordering

### Patch Invariants

- every `Create*` appears before the first structural/content reference to its
  key
- structural patches encode the final intended parent/order state; the tree
  builder must not emit a temporary wrong parent and then "fix it up" unless the
  spec algorithm genuinely requires a later identity-preserving move
- node moves preserve `PatchKey` identity
- deterministic reparenting must remain materializable by strict runtime
  appliers under the existing `DomPatch` and node-identity contracts

### Patch-Semantics Equivalence

For this milestone, "same patch semantics" across execution modes means:

- materializing the emitted patch batches must yield the same final tree shape
  and same required foster-parented/implied-synthesis structural relationships
- surviving logical nodes must preserve the same identity semantics required by
  the node-identity contract
- different chunk plans may change batch boundaries or the exact incremental
  patch grouping
- different chunk plans must not change the intended structural effect of the
  patch stream for any in-scope table case

For a fixed chunk plan, patch order and patch contents remain deterministic.

## Deferred-Behavior Fallback Rules

If Milestone I encounters a token/state combination that is still outside this
contract while table-family modes are otherwise active:

- the parser must remain deterministic
- the parser must preserve SOE/AFE/patch invariants
- the parser must not use `Clear` or full-document rebuild as recovery
- the unsupported path must be documented before being treated as contractual

Fail-safe behavior is still required; undocumented ad hoc recovery is not.

## Acceptance Obligations

Milestone I is not complete unless the implementation satisfies all of the
following:

- table-family fixtures covering supported tags and modes pass in:
  - whole-input execution
  - chunked-input execution
  - seeded fuzz-chunk execution
- move-heavy foster-parenting and reparenting scenarios must prove:
  - the same final DOM across whole/chunked/fuzzed execution
  - the same patch semantics across whole/chunked/fuzzed execution
  - deterministic patch ordering for a fixed chunk plan
  - no chunk-boundary-induced change in whether foster parenting happens
- patch-level golden fixtures prove deterministic foster-parenting and implied
  synthesis behavior
- strict materialization/runtime apply paths accept legal move-heavy patch
  streams produced by table recovery
- WPT table-construction coverage is enabled for the in-scope slice, with an
  explicit skip manifest for deferred areas

This document is the normative implementation target for those tests.
