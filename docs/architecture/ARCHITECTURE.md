# Borrowser Architecture 🦀🏗️  
*A clean, modular browser engine written in Rust.*

This document describes the internal architecture of Borrowser:  
how HTML is parsed, how CSS is applied, how layout is computed,  
how rendering happens, and how components communicate across threads.

The goal is clarity and educational value,  every subsystem is designed to be easy to understand, explore, and improve.

---

# 🌐 High-Level Overview

Borrowser follows a modern browser architecture:

```

HTML/CSS → Parsing → DOM → Style → Layout → Paint → GPU

```

Work is split across dedicated threads:

- **Main thread:** UI, layout, rendering
- **Networking runtime:** Streaming HTML/CSS over HTTP
- **HTML parsing runtime:** Incremental DOM construction
- **CSS stylesheet runtime:** Stylesheet byte buffering, UTF-8 assembly, abort handling, and decoded stylesheet event emission

Communication happens through a **session-aware message bus**, allowing each tab to behave like an independent browser instance.

---

# 🧱 Architectural Layers

Borrowser is organized into modular crates, each with a focused role:

```

crates/
├── core_types      # Shared IDs/types (TabId, ResourceKind, BrowserInput, …)
├── tools           # Small shared helpers/constants
├── input_core      # UI-agnostic input state + editing semantics
├── app_api         # UI-facing traits + runtime/bus glue
│
├── html            # Tokenizer + DOM tree builder
├── css             # CSS parser, cascade, computed styles
├── layout          # Block + inline layout engine and box model
├── gfx             # egui + wgpu renderer + input/paint layer (text controls, caret, hit-testing)
│
├── net             # HTTP streaming client
├── runtime_net     # Network runtime thread
├── runtime_parse   # HTML parsing runtime thread
├── runtime_css     # CSS stylesheet runtime thread
│
├── bus             # Message bus for CoreCommand/CoreEvent
├── browser         # Tabs, navigation, page state
├── platform        # Window, event loop, system integration
└── js              # JavaScript runtime (WIP)

```

Each crate is intentionally small, isolated, and testable.

As a rule of thumb: `layout` stays UI/input-agnostic, while interactive behaviors (rendering + input routing for things like text controls) live in `gfx` (e.g. `<textarea>` caret/selection helpers in `gfx::textarea`).

---

# 📤 Message Bus (CoreCommand / CoreEvent)

At the heart of the engine is a **message-driven architecture**.

Every tab has a unique `tab_id`, and commands/events must carry it:

```

Tab → (CoreCommand) → runtime_net / runtime_parse / runtime_css
runtime → (CoreEvent) → Tab

```

Each runtime operates independently:

- The **network runtime** streams raw bytes.
- The **HTML parser** builds DOM fragments incrementally.
- The **CSS stylesheet runtime** buffers stylesheet bytes, assembles UTF-8 text, handles aborts, and emits decoded stylesheet blocks.
- Events are routed back to the main thread through winit’s event loop (`UserEvent::Core`).

This design guarantees:

- thread safety  
- predictable lifetimes  
- true tab isolation  
- no shared mutable state between threads  

The UI remains responsive even during heavy parsing or networking.

---

# 🌳 DOM Tree

HTML is parsed incrementally:

1. The tokenizer receives streamed bytes from the network.
2. The parser builds the tree node-by-node.
3. After incremental work, the parser emits either a legacy `DomUpdate`
   snapshot or a patch-based `DomPatchUpdate`.

`DomUpdate` is the legacy snapshot path. A patch-based stream also exists:
`DomPatchUpdate { handle, from, to, patches }` carries incremental DOM mutations
for a specific document handle and version range. Browser tabs apply patch
batches atomically through `DomStore`, classify non-empty batches into
`RestyleHint`s before materialization, and treat empty batches as no-ops for
style generations.

DOM nodes are simple, ergonomic Rust enums:

```rust
enum Node {
    Document { children: Vec<Node> },
    Element { name, attributes, style, children },
    Text { text },
    Comment { text },
}
```

Inline style attributes are stored in the node but do not affect layout until
the CSS cascade applies. Legacy compatibility APIs can still write a
compatibility declaration vector into `Node::style`; Milestones R and S define
the structured cascade and computed-style contracts that replace that
DOM-attached style bridge. Milestone U integrates those contracts into the
runtime path and isolates remaining browser-facing dependencies on
`Node::style`.

### Node IDs

Each `Node` has an `Id`. `Id(0)` means "unset". IDs are assigned during DOM construction by the
HTML builder and are deterministic for a given DOM snapshot (depth-first pre-order traversal with
children visited in source order). IDs are not guaranteed stable across different DOM builds or
parse runs if the tree shape changes.

---

# 🎨 CSS Engine

CSS is processed in four phases:

### 1. **Syntax Parsing**

`runtime_css` currently assembles decoded stylesheet text and emits it back to
the browser/page integration path. CSS parsing semantics remain owned by
`crates/css`; browser/page state owns stylesheet attachment order and lifetime.
A future parse-worker model may use another runtime thread as an execution host
for `crates/css`, but it must not move CSS semantic ownership into
`runtime_css`.

The syntax contract for Milestone N lives in:

* `docs/css/syntax-parser-contract.md`

The engine-facing CSS rule/value model contract for Milestone O lives in:

* `docs/css/o1-rule-value-model-architecture.md`

The cascade architecture contract for Milestone R lives in:

* `docs/css/r1-cascade-architecture-style-resolution-contract.md`

The stable cascade/style-resolution debug-output contract for Milestone R lives
in:

* `docs/css/r8-cascade-style-resolution-debug-output.md`

The Milestone R close-out and computed-style handoff contract lives in:

* `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

The Milestone S property system and computed-style runtime handoff contract
lives in:

* `docs/css/s9-property-system-computed-style-runtime-contract.md`

The Milestone T CSS hardening threat model and invariant contract lives in:

* `docs/css/t1-css-hardening-strategy-threat-model.md`

The implemented CSS hardening limits, fuzzing workflow, regression corpus, and
CI repro workflow live in:

* `docs/security/css-hardening.md`

The Milestone U runtime integration architecture and CSS pipeline ownership
contract lives in:

* `docs/css/u1-runtime-integration-architecture-css-pipeline-ownership.md`

The Milestone U close-out runtime integration contract and future extension
points live in:

* `docs/css/u8-runtime-integration-contracts-extension-points.md`

The syntax layer owns:

* tokenizer and parser entry points
* structured stylesheet parsing on top of tokens
* syntax-layer AST output plus explicit compatibility projection
* deterministic malformed-input recovery
* parser diagnostics and limit enforcement
* stable debug/snapshot output for regression tests

The shipped browser path now treats the engine-facing model parse result as the
default stylesheet product. Stylesheet text is parsed through the structured
syntax-layer entrypoints, converted into the Milestone O rule/value model, and
stored that way in page state. Compatibility projection still exists, but only
for legacy callers that have not moved to the structured model, cascade, and
computed-style contracts.

### 2. **Engine Rule/Value Model**

Milestone O defines the distinct engine-owned stylesheet/rule/declaration/value
layer between syntax parsing and cascade.

That layer:

* is built from structured `css::syntax` output
* owns long-lived stylesheet/rule/declaration/value storage
* replaces raw declaration-string handling as the permanent engine direction
* preserves canonical names plus source/debug metadata where needed
* remains separate from selector matching and cascade winner resolution

Compatibility adapters still exist for legacy callers, but they are not the
intended permanent handoff into cascade.

### 3. **Cascade**

The structured cascade path produces `ResolvedDocumentStyle` without mutating
DOM-attached declaration vectors. `attach_styles(dom, sheets)` remains a
compatibility projection for older callers, but it is not the browser view
handoff.

Milestone R's structured cascade path:

* consumes selector match outcomes rather than reparsing selector text
* resolves winners with explicit precedence keys
* models author stylesheet declarations, author inline declarations, and
  declaration-level `!important` ordering in the current scope
* owns the initial/default value table plus inheritance/default fill for the
  supported property subset
* produces deterministic style-resolution outputs independent of DOM mutation
* exposes stable cascade and resolved-style snapshots for regression triage

The current structured DOM-level cascade output is `ResolvedDocumentStyle`.
`attach_styles(dom, sheets)` remains only as a compatibility projection from
that output into `Node::style` for legacy consumers.
The primary debug surfaces are `cascade_evaluation_debug_snapshot(...)`,
`ResolvedStyle::to_debug_snapshot()`,
`ResolvedDocumentStyle::to_debug_snapshot()`, and
`resolve_document_styles_debug_snapshot(...)`.
The current document-level integration remains function-oriented. Any future
style-resolution session object or first-class inline declaration-list
entrypoint must preserve the structured cascade boundary.

### 4. **Computed Styles**

Milestone S defines the property-aware computed-style pipeline:

```
DOM + StylesheetParse[]
  -> ResolvedDocumentStyle
  -> ComputedDocumentStyle
  -> StyledNode tree
```

`ComputedStyle` is the final runtime CSS contract for the supported property
subset. It stores typed, normalized values; exposes private-field accessors and
deterministic property iteration; and is assembled through
`ComputedStyleBuilder` rather than ad hoc field mutation.

The shipped browser view path calls `PageState::build_style_phase_output()`.
Page state reuses or recomputes owned `ResolvedDocumentStyle` and
`ComputedDocumentStyle` artifacts, then calls
`build_style_tree_from_computed_styles(...)` to rebuild a borrow-backed
`StyledNode` view wrapped in `StylePhaseOutput` without writing to
`Node::Element::style`.

Downstream systems consume `ComputedStyle` or `StyledNode`. They do not parse
CSS text, inspect cascade winners, duplicate property metadata, or recover from
invalid supported declarations after computed-style assembly.

---

# 🖌️ Style Tree

From the DOM, we build a parallel **style tree**, where each node is a pair:

```
( DOM node, ComputedStyle, styled children[] )
```

This tree is what the layout engine consumes.

The normative rendering ownership and phase-boundary contract lives in:

* `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
* `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
* `docs/rendering/v3-retained-state-versus-rebuilt-state-ownership.md`
* `docs/rendering/v4-invalidation-and-rebuild-entry-points.md`
* `docs/rendering/v5-explicit-runtime-render-orchestration-path.md`
* `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
* `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`
* `docs/rendering/w1-box-tree-layout-model-contract.md`
* `docs/rendering/w2-structured-box-tree-data-structures.md`
* `docs/rendering/w3-display-to-box-generation-behavior.md`
* `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
* `docs/rendering/w5-containing-block-relationships.md`
* `docs/rendering/w6-block-formatting-context-foundations.md`
* `docs/rendering/w7-inline-formatting-context-foundations.md`
* `docs/rendering/w8-box-generation-formatting-debug-surfaces.md`
* `docs/rendering/w9-box-tree-invariants-extension-hooks.md`
* `docs/rendering/x1-sizing-architecture-flow-correctness-contract.md`
* `docs/rendering/x2-structured-size-resolution-model-inputs.md`

---

# 📐 Layout Engine

Borrowser has a hybrid layout engine:

* **Block layout** for structural elements (`div`, `p`, `html`, `body`, etc.)
* **Inline layout** for text and inline elements (`span`, `a`, `em`, …)

Layout generates a **BoxTree** from styled content, then projects it into the
current **LayoutBox** geometry structure consumed by paint and hit testing.
Milestones W1 through W9 define this as a distinct layout-owned model derived
from DOM and computed style, with explicit boundaries for display-to-box
generation, anonymous boxes, containing-block modeling, block formatting
contexts, inline formatting contexts, deterministic debug surfaces, and later
layout extension hooks.

---

## 1. Block Layout (vertical flow)

Block layout processes nodes top-to-bottom:

```
y cursor moves down
width inherited from parent
margins applied from box model
children placed inside padding box
```

Each `LayoutBox` contains:

```rust
struct LayoutBox<'style_tree, 'dom> {
    box_id: BoxId,
    kind: BoxKind,
    style: &'style_tree ComputedStyle,
    source: BoxSource<'style_tree, 'dom>,
    node: &'style_tree StyledNode<'dom>,
    rect: Rectangle,
    children: Vec<LayoutBox<'style_tree, 'dom>>,
    containing_block: Option<ContainingBlockId>,
    formatting_context: Option<FormattingContextId>,
}
```

Block layout computes:

* final width
* final height
* position of children
* handling margins & padding

The block model is now fully integrated with BoxMetrics.

---

## 2. Inline Layout (text + inline elements)

Inline layout is more sophisticated and replicates core browser behavior:

### ✔ Whitespace collapsing

`"a   b"` becomes `"a b"`

### ✔ Tokenization into:

* **Words**
* **Collapsible spaces**

### ✔ Fragmentation into **line boxes**:

Each line contains:

* a list of `LineFragment`
* actual positioned rects for each word/space
* style applied per fragment

### ✔ Inline Run Collection

Borrowser walks the subtree and collects inline content, stopping at block boundaries.

### ✔ TextMeasurer abstraction

Inline layout depends on a `TextMeasurer` trait:

```rust
trait TextMeasurer {
    fn measure(&self, text: &str, style: &ComputedStyle) -> f32;
    fn line_height(&self, style: &ComputedStyle) -> f32;
}
```

This keeps layout independent of any rendering backend (e.g., egui).

---

## 3. Inline + Block Integration

After inline layout produces line boxes:

* Their height determines the block’s content height.
* Padding is applied around them.
* Block children appear below inline content.
* Margins adjust spacing between sibling blocks.

This creates a clean unified flow layout.

---

# 🖼️ Painting

Rendering is done via `egui` + `wgpu`, but **painting logic is custom**:

### For each LayoutBox:

1. Paint background color
2. Paint its inline content (LineBoxes → LineFragments)
3. Recursively paint children

Painting uses only the geometry computed during layout.

`egui` is used for the **UI shell** (tab strip, URL bar, etc.) and as a **drawing API** for page painting.
It is not used as the page layout engine — layout comes from the `layout` crate.

---

# 🔄 Frame Pipeline

Each page-rendering frame now follows this explicit orchestration contract:

```
1. Receive `CoreEvent`s and update page-owned DOM/style/resource/input invalidation state.
2. Runtime invalidation enters as `RenderInvalidationRequest`.
3. `Tab::request_render_work(...)` queues work in `PendingRenderWork`.
4. `browser::view::content(...)` starts the next orchestrated frame attempt.
5. `browser::rendering::prepare_page_frame(...)` asks `PageState` for `StylePhaseOutput`.
6. `browser::rendering::execute_prepared_page_frame(...)` executes the viewport frame.
7. `gfx::viewport::execute_viewport_frame(...)` prepares the current layout and paint execution inputs.
8. `layout::layout_document(LayoutPhaseInput::from_style_output(...))` produces `LayoutPhaseOutput`.
9. `gfx::paint::paint_page(PaintPhaseInput::new(...), PaintArgs { ... })` emits immediate paint commands for the current frame.
10. `RenderFrameExecutionTrace` records the orchestration decision for the frame attempt.
```

`PageState::build_style_phase_output()` remains the page-owned style
preparation implementation. It reuses or recomputes retained
resolved/computed-style artifacts, then rebuilds a borrow-backed
`StylePhaseOutput` through `build_style_tree_from_computed_styles(...)`.
Layout and paint remain frame-local rebuilt artifacts until a later retained
layout or retained-paint hook is implemented explicitly.

Milestone U introduced page-owned DOM/style/stylesheet generations and a
`PageStyleCache` so clean frames can reuse computed style. Milestone V makes it
explicit that style artifacts are retained, while `StyledNode`, layout, and
paint outputs remain rebuilt derived state until later rendering milestones add
retained downstream caches. Milestone V6 adds deterministic debug snapshots for
`StylePhaseOutput`, `LayoutPhaseInput`, `LayoutPhaseOutput`,
`PaintPhaseInput`, and `RenderFrameExecutionTrace` so representative pipeline
flows can be asserted without pixel-diff tests. Milestone V7 closes the
milestone by documenting the invariant and extension-hook contract that future
layout, paint, invalidation, and retained-state work must extend deliberately.

---

# 🪢 Concurrency Model

Borrowser strictly isolates responsibilities:

| Thread           | Responsibility                |
| ---------------- | ----------------------------- |
| **Main thread**  | UI, layout, rendering         |
| `runtime_net`    | HTTP streaming                |
| `runtime_parse`  | DOM building                  |
| `runtime_css`    | CSS stylesheet byte buffering, UTF-8 assembly, abort handling, and decoded-block event emission |
| winit event loop | Dispatches CoreEvents to tabs |

All communication is message-driven, no shared state.

---

# 📦 Page State

Each tab maintains:

```rust
struct PageState {
    dom: Option<Node>,
    head: HeadMetadata,
    visible_text_cache: String,
    rendering: RetainedRenderState,

    // later derived caches:
    layout_cache: Option<PageLayoutCache>,
}

struct RetainedRenderState {
    document_styles: DocumentStyleSet, // document/source-ordered stylesheet slots
    generations: PageStyleGenerations,
    style_cache: Option<PageStyleCache>, // owned ResolvedDocumentStyle + ComputedDocumentStyle
    style_dirty: bool,
    layout_dirty: bool,
    pending_style_invalidation: Option<StyleInvalidationScope>,
}
```

Compatibility projection still exists during migration, but it now happens
from structured `ResolvedDocumentStyle` output inside the cascade boundary
rather than as the core style-resolution result.

The remaining `Node::style` declaration vector is likewise a migration-only
cascade bridge and is not the intended long-term style-resolution contract.
`PageState` is still the document lifecycle owner, but retained rendering state
is now grouped under an explicit `RetainedRenderState` sub-owner. That struct
is the storage boundary for document stylesheet order, style generations,
persistent owned style artifacts, and change-scoped recomputation.
`DocumentStyleSet` exposes loaded stylesheet artifacts in document/source
order; pending, failed, and aborted slots preserve cascade position without
contributing declarations.

Derived style/layout caches must respect Rust ownership boundaries. If
`StyledNode` or `LayoutBox` remain borrow-backed views over the DOM, they must
not be stored self-referentially inside `PageState`. Long-lived caches should
either store owned style artifacts keyed by stable node identity, use an arena
owned outside the borrowed view, or rebuild borrow-backed trees from owned DOM
and computed-style artifacts when needed.

---

# 🔮 Future Directions

See the full [ROADMAP.md](ROADMAP.md) for upcoming work:

* Inline-block & display model
* Borders, backgrounds, floats
* Page-level caching: no rebuild on every frame
* Debug overlay (paint lines, box outlines)
* More CSS properties (width, height, overflow, font, etc.)
* JavaScript runtime integration

---

# ⌨️ Input Subsystem Boundaries

The input subsystem is split across crates with explicit responsibilities:

- input_core: state + editing semantics, UI-agnostic, deterministic, heavily tested
- gfx::input: routing + focus policy + hit-test integration + caret positioning
- browser: owns lifecycle + navigation reset semantics + persistence rules

Allowed dependencies for input-related modules:

| crate | allowed dependencies |
| --- | --- |
| input_core | std (+ html if chosen) |
| gfx | layout, egui, input_core |
| browser | gfx, input_core |

Reviewer note: these boundaries are enforceable without cross-checking other
subsystems; keep input logic within the crates listed above and avoid adding
new dependencies that bypass this table.

---

# 🎯 Design Philosophy

Borrowser is built with four principles:

### **1. Clarity over cleverness**

Every subsystem should be readable and teachable.

### **2. Clean modular boundaries**

Each crate handles one job, no cross-contamination.

### **3. Real browser behavior**

Correctness first, shortcuts avoided unless explicitly temporary.

### **4. Extensible foundations**

Everything is designed so future features (layout modes, floats, transforms, JS) can plug in naturally.

---
