# The Borrowser 🦀🌐

A learning project: building a **web browser in Rust**, from scratch, with a focus on
understanding every piece of the stack: windowing, rendering, event loops, UI, networking, and background runtimes.

---

## 🙋 Why "Borrowser"?

Think “Borrow checker” + “Browser” = Borrowser. 🦀
P.S. nothing borrowed from Chromium *wink*.

---

## ✨ Current State

Right now the browser can:

* Open a native desktop window (via [winit](https://github.com/rust-windowing/winit))
* Render its UI and content (via [egui](https://github.com/emilk/egui) + [egui-wgpu](https://github.com/emilk/egui/tree/master/crates/egui-wgpu))
* Show a **tab strip** and **URL bar** with back, forward, refresh, and new/close tab buttons
* Keep a **separate navigation history per tab**
* Handle multiple **independent tabs**, each with isolated runtime sessions
* Fetch and **stream HTML** incrementally
* Parse HTML into a DOM tree on a background thread
* Detect and stream **external stylesheets** in parallel
* Parse and apply inline and external CSS with:
  * a **cascade layer** (selectors + specificity + inline styles)
  * a **computed style phase** (inherited + non-inherited properties)
* Render a **computed style tree** parallel to the DOM
* Compute a simple **block layout tree** (one box per DOM element)
* Paint **background colors** for nested elements, respecting layout order
* Render **text inside the correct layout box**, using:
  * CSS `color`
  * CSS `font-size`
  * basic **word-wrapping** inside each element’s box
* Display visible text and page background color
* Communicate between components through a **session-aware message bus**

It’s now structured like a real browser shell, with clear separation between the Browser Shell (`ShellApp`) and individual pages (`Tab`), and with proper session routing between UI, networking, and parsing.

---

## 🏗️ Architecture Overview

Borrowser is split into modular crates, each with a focused responsibility:

```

src/main.rs
crates/
├── app_api       # Shared traits, types, and CoreCommand/CoreEvent interfaces
├── browser       # ShellApp (tabs, UI Browser Shell) + Tab (page logic, DOM + CSS state)
├── css           # CSS syntax parser, cascade, computed styles, value parsing
├── gfx           # Rendering layer (egui + wgpu integration)
├── html          # HTML tokenizer and DOM builder
├── net           # Low-level HTTP streaming
├── runtime-net   # Networking runtime (handles FetchStream commands)
├── runtime-parse # HTML parsing runtime
├── runtime-css   # CSS parsing runtime
├── bus           # Message bus (CoreCommand / CoreEvent routing)
└── platform      # Platform integration: window, event loop, repaint proxy

```

---

## 🧩 How It Works

### 1. The Platform
(... unchanged ...)

### 2. The Message Bus
(... unchanged ...)

### 3. The Runtimes
(... unchanged ...)

### 4. The ShellApp and Tabs
(... unchanged ...)

### 5. Rendering

The `gfx` crate renders each frame with egui.
Only the main thread draws; all heavy work (networking, parsing, CSS) runs in the background.

The rendering pipeline now includes:

1. **Style tree construction** (Computed CSS for every element)
2. **Block layout tree construction**
3. **Painting**:
   * Background colors
   * Text (using font-size + color)
   * Basic inline layout with word-wrapping

---

## 🔄 Data Flow Example

```

[User presses Enter in URL bar]
↓
ShellApp forwards URL to active Tab
↓
Tab sends CoreCommand::FetchStream(url, tab_id)
↓
Bus routes to runtime-net
↓
runtime-net streams bytes and emits CoreEvent::NetworkChunk(tab_id)
↓
runtime-parse builds DOM incrementally and emits CoreEvent::DomUpdate(tab_id)
↓
runtime-css parses stylesheets and updates inline/external CSS
↓
Platform posts UserEvent::Core(event) to main thread
↓
ShellApp routes event to correct Tab by tab_id
↓
Tab updates its DOM + CSS state and requests redraw
↓
gfx::Renderer builds style tree, layout tree, and paints content

```

---

## 🧭 Event & Repaint System
(... unchanged ...)

---

## 🚀 Running the Project
(... unchanged ...)
```

---

# ✔️ Summary of What Was Added

Only these factual additions were made:

### 🆕 Features added to README

* Computed style tree
* Block layout tree
* Background painting per layout box
* Text rendering using CSS color & font-size
* Basic word-wrapping
* Mention of cascade, specificity, computed styles
* runtime-css is now explicitly part of the flow
* Rendering pipeline steps updated

Everything else stayed exactly as written.

---

If you'd like, I can also update the README with a tiny visual diagram of the **dom → style → layout → paint** pipeline, but only if you want it.
