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
* Show a **URL bar** with back, forward, and refresh buttons
* Keep a simple **navigation history** and loading indicator
* Fetch and **stream HTML** incrementally
* Parse HTML into a DOM tree on a background thread
* Detect and stream **external stylesheets** in parallel
* Parse and apply both inline and external CSS
* Display visible text and page background color
* Communicate between components through a **message bus**

It’s already structured like a small real browser, with clear boundaries between UI, networking, and parsing.

---

## 🏗️ Architecture Overview

Borrowser is split into modular crates, each with a focused responsibility:

```
src/main.rs
crates/
├── app_api       # Shared traits, types, and the CoreCommand/CoreEvent bus definitions
├── browser       # The BrowserApp (UI logic, navigation, DOM + CSS state)
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

* **Commands (CoreCommand)** — from the BrowserApp to the runtimes
* **Events (CoreEvent)** — from the runtimes back to the BrowserApp

This means the UI never talks to threads directly; it only sends commands through the bus.

### 3. The Runtimes

Each runtime has its own thread and purpose:

* **runtime-net** — downloads HTML or CSS streams over HTTP
* **runtime-parse** — builds DOM trees and emits `DomUpdate` snapshots
* **runtime-css** — parses CSS blocks and emits parsed rules

They all share the same event bus, so they can work concurrently and independently.

### 4. The BrowserApp (UI)

Implements the `UiApp` trait. It:

* Sends `CoreCommand::FetchStream` when the user navigates
* Receives `CoreEvent::{DomUpdate, CssParsedBlock, CssSheetDone}`
* Updates its in-memory DOM and style sheet state
* Requests repaints through a lightweight `RepaintHandle`

### 5. Rendering

The `gfx` crate renders each frame with egui.
Only the main thread draws; all heavy work happens elsewhere.

---

## 🔄 Data Flow Example

```text
[User enters URL and presses Enter]
   ↓
BrowserApp sends CoreCommand::FetchStream(url)
   ↓
Bus routes it to runtime-net
   ↓
runtime-net streams bytes and emits CoreEvent::NetChunk
   ↓
Bus routes CoreEvent to runtime-parse
   ↓
runtime-parse builds DOM incrementally and emits CoreEvent::DomUpdate
   ↓
Bus routes it to the platform (main thread)
   ↓
Platform posts UserEvent::Core(event) via winit proxy
   ↓
BrowserApp.on_core_event(event) updates state + requests redraw
   ↓
gfx::Renderer draws new frame
```

Meanwhile, detected stylesheets trigger extra `FetchStream` commands handled by the same flow.

---

## 🧭 Event & Repaint System

```
+-------------------+         +--------------------+        +-------------------+
|    BrowserApp     |         |   Message Bus      |        |    Runtimes       |
|  (UI + state)     |◀──────▶| CoreCommand/CoreEvent |◀──▶ | net / parse / css |
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
| Calls app.on_core_event() |
| Requests window redraw    |
+---------------------------+
```

**Why this design?**

* Each runtime works independently (like Chrome’s process model)
* The main thread only handles UI and rendering
* Message passing keeps things simple and thread-safe
* Scales naturally to multi-tab or multi-runtime setups later

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
* Show a URL bar and simple navigation buttons
* Fetch and stream a web page
* Incrementally parse and display its text
* Apply inline and external CSS
* Update the UI smoothly as data arrives

---

## 📚 Next Steps

* [ ] Parallelize DOM parsing across subtrees
* [ ] Implement an “Inspect” panel for DOM + CSS
* [ ] Add caching and connection reuse
* [ ] Render inline images
* [ ] Basic layout engine (block & inline flow)
* [ ] Add simple JavaScript execution (sandboxed)
* [ ] Multi-tab support via multiple runtime groups

---

**Borrowser** is first and foremost a learning project, every line is meant to teach something about how browsers really work under the hood, one crate at a time.
