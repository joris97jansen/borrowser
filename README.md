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
* Render its UI (via [egui](https://github.com/emilk/egui) + [egui-wgpu](https://github.com/emilk/egui/tree/master/crates/egui-wgpu))
* Show a **tab strip** and **URL bar** with back, forward, refresh, and new/close tab buttons
* Keep a **separate navigation history per tab**
* Handle multiple **independent tabs**, each with isolated runtime sessions
* Fetch and **stream HTML** incrementally
* Parse HTML into a DOM tree on a background thread
* Detect and stream **external stylesheets** in parallel
* Parse and apply both inline and external CSS
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
├── css           # CSS parsing and style attachment logic
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

Creates the window, sets up the **event loop**, and launches the background runtimes.
It owns an `EventLoopProxy<UserEvent>` that safely lets background threads send messages to the UI.

### 2. The Message Bus

Connects everything using two channels:

* **Commands (CoreCommand)** — from the ShellApp/Tabs to the runtimes
* **Events (CoreEvent)** — from the runtimes back to the ShellApp/Tabs

Each command and event includes a `session_id` (or `tab_id`), keeping communication fully isolated per tab.

### 3. The Runtimes

Each runtime has its own thread and purpose:

* **runtime-net** — downloads HTML or CSS streams over HTTP
* **runtime-parse** — builds DOM trees and emits `DomUpdate` snapshots
* **runtime-css** — parses CSS blocks and emits parsed rules

All share the same event bus, operating concurrently and independently.

### 4. The ShellApp and Tabs

* **ShellApp** implements the `UiApp` trait and manages the overall browser Browser Shell (tab strip, URL bar, navigation buttons).
* Each **Tab** owns its own `tab_id`, history, DOM, and CSS state, and communicates via the message bus.
* Tabs send `CoreCommand` messages when navigating and receive `CoreEvent` updates from runtimes.

### 5. Rendering

The `gfx` crate renders each frame with egui.
Only the main thread draws; all heavy work (networking, parsing, CSS) runs in the background.

---

## 🔄 Data Flow Example

```text
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
Platform posts UserEvent::Core(event) to main thread
   ↓
ShellApp routes event to correct Tab by tab_id
   ↓
Tab updates its DOM + CSS state and requests redraw
   ↓
gfx::Renderer draws the updated content
```

Each tab runs through this flow independently.

---

## 🧭 Event & Repaint System

```
+-------------------+         +--------------------+        +-------------------+
|     ShellApp      |         |   Message Bus      |        |    Runtimes       |
| (Browser Shell + tabs)   |◀──────▶| CoreCommand/CoreEvent |◀──▶ | net / parse / css |
+-------------------+         +--------------------+        +-------------------+
        │                              │                             │
        │        EventLoopProxy<UserEvent>                            │
        │─────────────────────────────────────────────────────────────▶
        │
        ▼
+---------------------------+
|       Platform            |
|  (winit + egui + gfx)     |
|---------------------------|
| Receives UserEvent::Core  |
| Routes to ShellApp/Tab    |
| Requests window redraw    |
+---------------------------+
```

**Why this design?**

* Each tab is fully isolated via its `session_id`
* Each runtime works independently
* The main thread only handles UI and rendering
* Message passing keeps everything thread-safe and modular
* Scales naturally to multi-tab and multi-runtime setups

---

## 🚀 Running the Project

Requirements:

* Rust (latest stable)

Run in release mode for full speed:

```bash
cargo run --release
```

Borrowser will:

* Open a desktop window titled **Borrowser**
* Show a tab strip, URL bar, and navigation buttons
* Support multiple independent tabs
* Fetch and stream web pages incrementally
* Parse and render DOM + CSS progressively
* Keep the UI smooth and responsive throughout

---

## 📚 Next Steps

* [ ] Implement a DevTools/Inspect panel for DOM + CSS per tab
* [ ] Add caching and connection reuse
* [ ] Render inline images
* [ ] Basic layout engine (block & inline flow)
* [ ] Add simple JavaScript execution (sandboxed)
* [ ] Expand to multi-runtime-per-tab isolation

---

**Borrowser** is a learning project at its core — every crate and component is designed to make you understand how real browsers work, one layer at a time.
