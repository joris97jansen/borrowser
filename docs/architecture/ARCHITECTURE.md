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
- **CSS stylesheet runtime:** Stylesheet transport/assembly + parse job execution

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

````

Each runtime operates independently:

- The **network runtime** streams raw bytes.
- The **HTML parser** builds DOM fragments incrementally.
- The **CSS stylesheet runtime** assembles and dispatches stylesheet parse jobs in parallel.
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
3. After each incremental update, a `DomUpdate` event is emitted.

`DomUpdate` is the legacy snapshot path. A patch-based stream also exists:
`DomPatchUpdate { handle, from, to, patches }` carries incremental DOM mutations
for a specific document handle and version range. The patch event is defined but
not applied yet; the snapshot path remains the default until the patch model and
applier are finalized.

DOM nodes are simple, ergonomic Rust enums:

```rust
enum Node {
    Document { children: Vec<Node> },
    Element { name, attributes, style, children },
    Text { text },
    Comment { text },
}
````

Inline style attributes are stored in the node but do not affect layout until the CSS cascade applies.
The current shipped path still writes a compatibility declaration vector into
`Node::style`; Milestone R is defining the long-term structured resolved-style
contract that replaces that DOM-attached bridge.

### Node IDs

Each `Node` has an `Id`. `Id(0)` means "unset". IDs are assigned during DOM construction by the
HTML builder and are deterministic for a given DOM snapshot (depth-first pre-order traversal with
children visited in source order). IDs are not guaranteed stable across different DOM builds or
parse runs if the tree shape changes.

---

# 🎨 CSS Engine

CSS is processed in four phases:

### 1. **Syntax Parsing**

`runtime_css` currently assembles decoded stylesheet text and hands it to the
`css::syntax` layer.

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

The shipped browser view path uses `build_style_tree_with_stylesheets(...)`,
which consumes structured stylesheet model output, resolves cascade winners,
normalizes property values, applies inheritance/defaults, validates element
identity, and returns a `StyledNode` tree without writing to
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

---

# 📐 Layout Engine

Borrowser has a hybrid layout engine:

* **Block layout** for structural elements (`div`, `p`, `html`, `body`, etc.)
* **Inline layout** for text and inline elements (`span`, `a`, `em`, …)

Layout computes a **LayoutBox tree** containing the geometry of each box.

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
struct LayoutBox<'a> {
    kind: BoxKind,
    style: &'a ComputedStyle,
    node: &'a StyledNode<'a>,
    rect: Rectangle,
    children: Vec<LayoutBox<'a>>,
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

Each frame follows:

```
1. Receive CoreEvents from runtimes.
2. If DOM/CSS changed:
    - rebuild style tree
    - rebuild layout tree
    - refine layout using inline layout
3. Paint the layout tree inside a scrollable viewport.
```

Later, this will be optimized so steps 2 only run on DOM/CSS changes.

---

# 🪢 Concurrency Model

Borrowser strictly isolates responsibilities:

| Thread           | Responsibility                |
| ---------------- | ----------------------------- |
| **Main thread**  | UI, layout, rendering         |
| `runtime_net`    | HTTP streaming                |
| `runtime_parse`  | DOM building                  |
| `runtime_css`    | CSS stylesheet assembly + parse job execution |
| winit event loop | Dispatches CoreEvents to tabs |

All communication is message-driven, no shared state.

---

# 📦 Page State

Each tab maintains:

```rust
struct PageState {
    dom: Option<Node>,
    css_stylesheets: Vec<StylesheetParse>, // engine-facing model parse artifacts in stylesheet insertion order
    head: HeadMetadata,
    visible_text_cache: String,

    // later:
    styled_root: Option<StyledNode>,
    layout_root: Option<LayoutBox>,
}
```

Compatibility projection still exists during migration, but it now happens
from structured `ResolvedDocumentStyle` output inside the cascade boundary
rather than as the core style-resolution result.

The remaining `Node::style` declaration vector is likewise a migration-only
cascade bridge and is not the intended long-term style-resolution contract.
The page-state shape is the storage boundary for persistent style/layout trees
and change-scoped recomputation; those caches remain derived state from the DOM,
stylesheet model, viewport, and runtime style pipeline.

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
