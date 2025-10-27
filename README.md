# The Borrowser 🦀🌐

A learning project: building a **web browser in Rust**, from scratch, with a focus on
understanding every piece of the stack: windowing, rendering, event loops, UI, and networking.

---

## 🙋 Why "Borrowser"?

Think “Borrow checker” + “Browser” = Borrowser. 🦀

P.S. nothing borrowed from Chromium *wink*.

---

## ✨ Current State

Right now the browser can:

* Open a desktop window (via [winit](https://github.com/rust-windowing/winit))
* Render a GUI (via [egui](https://github.com/emilk/egui), [egui-wgpu](https://github.com/emilk/egui/tree/master/crates/egui-wgpu))
* Show a **URL bar** with back, forward, and refresh buttons
* Fetch and **stream HTML** (via [ureq](https://github.com/algesten/ureq))
* Parse HTML into a DOM tree, incrementally as chunks arrive
* Detect and fetch **external stylesheets** concurrently
* Parse and apply inline and external CSS
* Display visible text and page background color
* Keep a simple **navigation history** and loading indicator

It’s early, but the foundations are solid and realistic for building a real browser.

---

## 🏗️ Architecture

Borrowser is split into modular crates:

```
src/main.rs
crates/
├── app_api     # Shared traits and types between platform and apps
├── browser     # The BrowserApp implementation (UI, state, DOM, CSS)
├── css         # CSS parsing and style attachment
├── gfx         # Rendering layer (egui + wgpu glue)
├── html        # HTML tokenizer and DOM builder
├── net         # Streaming HTTP fetcher (ureq-based)
└── platform    # Platform integration: window, event loop, repaint proxy
```

### Core flow

1. **`platform`** creates the window and event loop (via `winit`)
2. **`app_api`** defines the `UiApp` trait:

   * `ui(&mut self, &egui::Context)` draws the UI
   * `set_net_stream_callback(NetStreamCallback)` installs a network event handler
   * `on_net_stream(NetEvent)` handles streaming updates
3. **`browser`** implements `UiApp` with `BrowserApp`:

   * Handles navigation, history, and rendering
   * Streams HTML and CSS through the `net` crate
   * Updates the DOM incrementally and attaches styles
4. **`net`** streams data over HTTP in background threads

   * Emits `NetEvent::{Start, Chunk, Done, Error}` events
   * Each event is sent back to the main thread via a proxy
5. **`platform`** forwards `NetEvent` messages to the app and triggers repaints
6. **`gfx`** renders everything using `egui` on top of `wgpu`

---

## 🔄 Streaming Flow Example

```text
[User enters URL and presses Enter]
   ↓
BrowserApp → net::fetch_text_stream(url, callback)
   ↓ (background thread)
net crate reads HTTP response in chunks
   ↓
cb(NetEvent::Start)
cb(NetEvent::Chunk)
cb(NetEvent::Chunk)
cb(NetEvent::Done)
   ↓
platform::UserEvent::NetStream(NetEvent)
   ↓
PlatformApp forwards to BrowserApp.on_net_stream(event)
   ↓
BrowserApp updates DOM and repaints incrementally
   ↓
gfx::Renderer draws updated frame
```

The same pattern applies to **CSS streams**: each stylesheet URL is registered, streamed, and applied as soon as it completes.

---

## 🧭 Event & Repaint Architecture

```
+--------------------+                 +--------------------------+
|   BrowserApp       |                 |        net crate         |
|  (UiApp impl)      |                 |  (background streaming)  |
|--------------------|                 |--------------------------|
| - url              |  fetch_stream() |  ureq::get().into_reader |
| - dom              | ───────────────▶|  emit NetEvent::*        |
| - loading          |                 |  cb(NetEvent)            |
| - css_pending      |◀─────────────── |                          |
| - repaint_handle   |                 +--------------------------+
+---------┬----------+
│ set_net_stream_callback(cb)
│ (installed by platform)
│
│                    EventLoopProxy<UserEvent>
│                 (used by all background threads)
│
+---------▼----------+   send_event(NetStream)   +-------------------+
|     Platform       |◀──────────────────────────|   network thread  |
|  (winit + gfx)     |                          | (closure proxy)   |
|--------------------|                          +-------------------+
| on NetStream:      |
|   app.on_net_stream(event)                    |
| on Repaint:        |                          |
|   window.request_redraw()                     |
+--------------------+
```

### Key contracts

* `UiApp::set_net_stream_callback(cb)` — installs the callback
* `UiApp::on_net_stream(event)` — receives streamed HTML/CSS events
* `net::fetch_stream(url, kind, cb)` — starts a streaming fetch
* `UserEvent::NetStream(NetEvent)` — message type for cross-thread delivery
* `RepaintHandle` — lightweight handle to request redraws safely

### Why this design?

* Keeps networking fully off the main thread
* Uses a **single proxy** to post events thread-safely
* Decouples UI from networking, windowing, and rendering
* Enables smooth incremental updates (streamed HTML and CSS)

---

## 🚀 Running

Requirements:

* Rust (latest stable)

```bash
cargo run
```

Borrowser will:

* Open a desktop window titled **Borrowser**
* Display a URL bar with back, forward, and refresh
* Fetch `https://example.com` by default
* Stream and render its HTML and CSS
* Display visible text and background color
* Show loading state in the status bar

---

## 📚 Next Steps

* [ ] Incremental CSS parsing while streaming
* [ ] Add request IDs for safe navigation cancellation
* [ ] Implement a DOM inspector view (for learning/debugging)
* [ ] Support basic layout and box model
* [ ] Add JavaScript execution sandbox
* [ ] Introduce caching, cookies, and persistent sessions
* [ ] Optimize redraw frequency and GPU usage

---

**Borrowser** is first and foremost a learning project, every line is meant to teach something about how browsers actually work under the hood.
