# AE1: HTML Parser And Parser-Created DOM Ownership Contract

Last updated: 2026-07-01
Status: Milestone AE architecture contract
Scope: `crates/html/src/html5`, parser-created DOM output, and downstream
consumer boundaries

This document defines the Milestone AE ownership boundary for Borrowser's HTML
parser, tree builder, parser-created DOM output, and consumers of that output.

AE1 is an architecture and contract issue. It does not implement new tokenizer
behavior, tree-construction behavior, DOM APIs, rendering behavior, JavaScript
behavior, resource loading, navigation behavior, or runtime lifecycle behavior.

## Related Code

- `crates/html/src/html5/shared/context.rs`
- `crates/html/src/html5/shared/token.rs`
- `crates/html/src/html5/shared/error.rs`
- `crates/html/src/html5/tokenizer/`
- `crates/html/src/html5/tree_builder/`
- `crates/html/src/html5/session/`
- `crates/html/src/dom_patch.rs`
- `crates/browser/src/dom_store/`
- `crates/browser/src/rendering/identity.rs`
- `crates/css/src/computed/`
- `crates/layout/src/`
- `crates/gfx/src/paint/`

## Related Contracts

- [`docs/adr/001-html5-parsing-architecture.md`](../adr/001-html5-parsing-architecture.md)
- [`docs/html5/html5-core-v0.md`](html5-core-v0.md)
- [`docs/html5/spec-matrix-tokenizer.md`](spec-matrix-tokenizer.md)
- [`docs/html5/spec-matrix-treebuilder.md`](spec-matrix-treebuilder.md)
- [`docs/html5/dompatch-contract.md`](dompatch-contract.md)
- [`docs/html5/node-identity-contract.md`](node-identity-contract.md)
- [`docs/html5/invariants.md`](invariants.md)
- [`docs/html5/rawtext-script-stability.md`](rawtext-script-stability.md)
- [`docs/css/u8-runtime-integration-contracts-extension-points.md`](../css/u8-runtime-integration-contracts-extension-points.md)
- [`docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`](../rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md)
- [`docs/rendering/ac2-retained-render-identities.md`](../rendering/ac2-retained-render-identities.md)

## Purpose

Milestone AE strengthens Borrowser's HTML input side. The parser must become
more browser-shaped without letting HTML parsing, DOM materialization, CSS,
layout, paint, and browser/runtime orchestration collapse into one subsystem.

The architectural handoff is:

```text
bytes or strings
  -> HTML parser input / preprocessing
  -> tokenizer states and typed tokens
  -> tree-construction state
  -> parser-created DOM semantics
  -> DomPatch stream or parser-owned DOM snapshot
  -> browser/runtime materialization and page state
  -> CSS style resolution
  -> Layout
  -> Paint
```

Each stage may consume the previous stage's documented output. No later stage
may reinterpret the internal semantics of an earlier stage.

## Ownership Matrix

| subsystem | owns | may consume | must not own or depend on |
| --- | --- | --- | --- |
| HTML/parser | tokenizer input preprocessing, `Html5Tokenizer`, tokenizer states, `Token`, `ParseError`, parse-error accounting, tree-construction state, `InsertionMode`, stack of open elements, active formatting elements where supported, parser-created DOM construction semantics, `DomPatch` emission semantics, parser debug output | decoded input, parser configuration, session policy | browser retained render identity allocation, CSS cascade, layout geometry, paint ordering, runtime navigation behavior |
| DOM/document model | parser-created document and node structure, node parent/child invariants, parser-created node identity within the parser output model | parser tree-construction operations and emitted patches | tokenizer states, insertion modes as runtime behavior, retained render IDs |
| Browser/runtime | document handle/version lifetime, `DomStore` materialization, page-owned active DOM lifetime, stylesheet discovery, invalidation/orchestration, retained render state lifetime | completed parser snapshots, `DomPatch` batches, materialized DOM, parser diagnostics for debug display where explicitly exposed | HTML tokenizer states, insertion modes, parse-error recovery rules, malformed-markup semantics |
| CSS | CSS parsing, selector matching, cascade, computed style, style-tree construction | DOM element names, attributes, relationships, text in `<style>` elements, document-order stylesheet slots | tokenizer states, insertion modes, parse errors, parser recovery internals, layout geometry, paint ordering |
| Layout | box generation, formatting behavior, layout structures, geometry | styled tree, computed style, explicit layout environment inputs | HTML parser behavior, malformed-markup recovery rules, CSS parsing/cascade, paint ordering |
| Paint | paint primitives, stacking, semantic paint ordering, current-frame paint output | layout output, runtime paint inputs, resource/input state | HTML parser behavior, malformed-markup recovery rules, CSS parsing/cascade, layout geometry ownership |

## HTML Parser Ownership

HTML/parser owns the complete parser semantics boundary:

- tokenizer input preprocessing and decoded input handoff policy;
- tokenizer state machines;
- emitted typed tokens;
- tokenizer-level normalization;
- recoverable parse errors and parser diagnostics;
- tree-construction state;
- insertion modes;
- stack of open elements;
- active formatting elements where supported;
- document-mode selection from parser inputs;
- parser-created DOM construction semantics;
- `DomPatch` emission semantics and parser debug output.

Existing Rust surfaces that carry this ownership include:

- `DocumentParseContext`
- `Html5Tokenizer`
- `Token`
- `ParseError`
- `Html5TreeBuilder`
- `InsertionMode`
- `OpenElementsStack`
- `ActiveFormattingList`
- `DocumentState`
- `Html5ParseSession`

`Html5ParseSession` orchestrates tokenizer and tree-builder pumping. It does
not transfer tokenizer or tree-construction semantics to browser/runtime code.

## Parser-Created DOM And DomPatch Output

The HTML tree builder owns parser-created DOM construction semantics. The
current runtime-facing output is `DomPatch` and `DomPatchBatch`; tests and
debug paths may also materialize parser-created DOM snapshots.

`DomPatch` is a parser output protocol. It records deterministic document/node
creation, structural edits, content edits, and reset boundaries. It does not
expose tokenizer states, insertion modes, SOE/AFE internals, or parse-error
recovery decisions as runtime semantics.

The DOM/document model owns node structure and invariants:

- a rooted document state where required by the patch contract;
- acyclic parent/child structure;
- at most one parent per node;
- explicit sibling order;
- deterministic node creation and movement semantics;
- parser-created node identity within its own identity domain.

## Browser Runtime Consumption Boundary

Browser/runtime consumes parser output. It may:

- receive completed DOM snapshots;
- apply non-empty `DomPatch` batches through `DomStore`;
- retain the active materialized DOM in `PageState`;
- derive restyle and render invalidation requests from documented DOM patch
  classes;
- reconcile document stylesheets from the active materialized DOM;
- use parser diagnostics in debug surfaces where explicitly exposed.

Browser/runtime must not:

- decide tokenizer state transitions;
- choose tree-builder insertion modes;
- implement malformed-markup recovery rules;
- interpret parse-error kinds as rendering semantics;
- allocate or reinterpret parser-created node identity as retained render
  identity.

`DomStore` is a materialization consumer. Its strict applier validates patch
protocol and materializes a runtime DOM view, but it does not own HTML parsing
semantics.

## CSS, Layout, And Paint Consumer Boundaries

CSS consumes the materialized DOM as style input. CSS may inspect element
names, attributes, relationships, and stylesheet text in `<style>` elements.
It must not depend on tokenizer states, insertion modes, parse-error counts,
parse-error kinds, or parser recovery internals.

Layout consumes styled/layout-ready structures. Layout must not parse HTML,
recover malformed markup, read insertion modes, or decide document-mode
effects that belong to HTML/parser or CSS.

Paint consumes layout output and runtime paint inputs. Paint must not infer
parser behavior, malformed-markup recovery, tokenizer state, insertion mode,
or DOM construction rules from paint order or visual output.

## Identity Domains

AE1 keeps three identity domains separate.

### `PatchKey`

`PatchKey` is the parser output identity used by `DomPatch` streams. It is
allocated by the HTML tree builder, is non-zero in emitted patches, and is not
reused within the active parser document baseline except where the patch
contract explicitly permits reuse after `Clear`.

### `html::internal::Id`

`html::internal::Id` is the materialized DOM node identity exposed by
`html::Node` and browser/runtime DOM views. Today, the `DomStore`
materialization bridge maps live `PatchKey(n)` to `Id(n)`, but that numeric
bridge is owned by DOM materialization. CSS, Layout, Paint, and retained
rendering code must not treat the numeric equality as parser ownership or as a
cross-document continuity guarantee.

### `RetainedRenderId`

`RetainedRenderId` is a browser/runtime-owned retained render identity for
render artifacts. It is allocated and reconciled inside the retained render
identity domain. It is not a DOM ID, not a `PatchKey`, not a layout `BoxId`,
not a paint operation index, and not a parser-created node identity.

Full document replacement starts a new retained render identity domain. A newly
parsed document may produce matching numeric `PatchKey` or `html::internal::Id`
values without proving retained render continuity.

## Document Mode Ownership

Document mode selection is owned by HTML tree construction at document scope.
The tokenizer emits DOCTYPE token fields, including `force_quirks`; the tree
builder/document parse state chooses document mode from those fields in the
supported scope documented by the HTML5 Core v0 and tree-builder matrix
contracts.

Document mode remains parser-owned state unless a future issue introduces a
dedicated, deterministic document-level signal. AE1 does not claim broad
document-mode conformance beyond already documented supported behavior.

## No-JavaScript Scope

Milestone AE is static HTML parsing and DOM construction work.

`<script>` may be represented as parser-created elements and text where the
current tokenizer/tree-builder scope supports it. That representation does not
imply JavaScript execution.

The following are explicit non-goals for AE1 and Milestone AE parser work
unless a later contract says otherwise:

- JavaScript execution;
- parser pauses caused by scripts;
- `document.write`;
- event loop behavior;
- timers, tasks, or microtasks;
- DOM bindings and full DOM APIs;
- event dispatch;
- custom elements;
- shadow DOM;
- resource loading;
- navigation behavior.

## Deferred Or Out-Of-Scope HTML Platform Work

AE1 defines ownership boundaries only. It does not promote any tokenizer state,
tree-builder algorithm, DOM API, parser recovery behavior, script behavior, or
resource behavior into supported status.

Feature support remains governed by:

- `docs/html5/html5-core-v0.md`
- `docs/html5/spec-matrix-tokenizer.md`
- `docs/html5/spec-matrix-treebuilder.md`
- `docs/html5/parser-parity-matrix.md`
- `docs/engine-feature-gap-tracker.md`

## Invariants

For AE1 and future AE implementation issues:

- HTML/parser owns parser semantics and parser debug output.
- Parser-created DOM construction semantics do not move into browser/runtime,
  CSS, Layout, or Paint.
- Browser/runtime consumes and materializes parser output without
  reinterpreting parser state.
- CSS consumes DOM/style inputs without depending on tokenizer or
  tree-builder internals.
- Layout and Paint consume typed downstream handoffs, not parser behavior.
- `PatchKey`, `html::internal::Id`, and `RetainedRenderId` remain separate
  identity domains.
- `<script>` tokenization/tree construction support does not imply script
  execution or parser pause behavior.
- Document mode remains parser-owned state in the current supported scope.
