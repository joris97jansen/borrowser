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
- **CSS parsing runtime:** Stylesheet parsing + cascade updates

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
├── runtime_css     # CSS parsing runtime thread
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
- The **CSS parser** parses stylesheets in parallel.
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

### Node IDs

Each `Node` has an `Id`. `Id(0)` means "unset". IDs are assigned on DOM updates via
`html::traverse::assign_node_ids` before the DOM is used by layout, hit-testing, or input state.
Assignment is deterministic for a given DOM snapshot: depth-first pre-order traversal with children
visited in source order. Existing non-zero IDs are preserved. IDs are not guaranteed stable across
different DOM builds or parse runs if the tree shape changes.

---

# 🎨 CSS Engine

CSS is processed in three phases:

### 1. **Parse Stylesheets**

`runtime_css` parses CSS blocks into:

* selectors (`div`, `#id`, `.class`)
* declarations (`color: red;`)
* rules with specificity and document order

### 2. **Cascade**

`attach_styles(dom, sheet)` walks the DOM and selects the winning declarations for each property using:

* selector matching
* specificity comparison
* cascading order
* inline styles (highest priority)

### 3. **Computed Styles**

Each element receives a final `ComputedStyle`:

```rust
struct ComputedStyle {
    color: rgba,
    background_color: rgba,
    font_size: Length,
    box_metrics: BoxMetrics,  // margin/padding per side
}
```

Computed styles are inherited appropriately (e.g., color, font-size).

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
| `runtime_css`    | CSS parsing                   |
| winit event loop | Dispatches CoreEvents to tabs |

All communication is message-driven, no shared state.

---

# 📦 Page State

Each tab maintains:

```rust
struct PageState {
    dom: Option<Node>,
    css_sheet: Stylesheet,
    head: HeadMetadata,
    visible_text_cache: String,

    // later:
    styled_root: Option<StyledNode>,
    layout_root: Option<LayoutBox>,
}
```

This will soon enable:

* persistent style/layout trees
* recompute only on DOM or CSS changes
* faster frame rendering

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
