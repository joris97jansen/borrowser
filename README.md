# The core architecture (first principles) according to ChatGPT, let's stick it in the README for later reference.

**1) Process & concurrency model**

* Start single-process with clear boundaries; evolve to multi-process (per-tab renderer) later.
* Use **Tokio** for async I/O and **message-passing** between subsystems (e.g., `flume`/`crossbeam-channel`).
* Add structured logs & tracing early: **tracing** + **tracing-subscriber**.

**2) Crate/workspace layout**

* `borrowser/` (Cargo workspace)

  * `browser/` – application shell (UI, tabs, navigation, profile, settings)
  * `net/` – loader: URL, fetch, cache, cookies, compression, TLS
  * `html/` – HTML tokenizer + tree builder (DOM)
  * `css/` – CSS parser + style system (cascade, inheritance, layout data)
  * `layout/` – block/inline layout, line breaking
  * `gfx/` – painting + compositing (GPU pipeline)
  * `js/` – JS engine & DOM bindings (starts minimal)
  * `platform/` – windowing, input, clipboard, fonts, file dialogs
  * `tools/` – devtools hooks, test harness

This keeps boundaries clean and lets us swap parts as we grow.

**3) Platform & UI**

* Windowing/input: **winit** (or **tao** if you want native menus).
* Rendering surface: **wgpu** (modern, cross-platform GPU).
* Text shaping: **rustybuzz** + **swash**/**font-kit**.
* Accessibility (soon): **accesskit**.
* Don’t use Tauri/WebView—we’re building our own engine.

**4) Networking stack**

* URLs: **url** (WHATWG-compatible).
* HTTP/1.1: **hyper**; HTTP/2: **h2**; HTTP/3/QUIC later via **quiche**.
* TLS: **rustls**.
* Compression: **flate2** (gzip/deflate), **brotli**, **zstd**.
* Cache + cookies: start with **sled** or **sqlite** (via **rusqlite**).

**5) Parsing & model trees**

* HTML5 parsing: **html5ever** (battle-tested, Servo heritage).
* DOM: our own Rust types; events, mutation, node lifetimes.
* CSS: **cssparser** + **selectors**; build CSSOM + computed style.

**6) Layout, painting, compositing**

* Phase 1: Block & inline layout (no floats, no flex/grid yet).
* Line breaking, whitespace collapsing, basic box model (margin/border/padding).
* Painting: generate display list -> **wgpu** to draw; consider **webrender** later for advanced compositing.
* Images: **image** (PNG/JPEG), add WebP/**libwebp-sys2**, AVIF later.

**7) JavaScript & DOM bindings (after HTML/CSS MVP)**

* Start with **Boa** (pure-Rust JS engine) for tight integration.

  * Alt: **rquickjs** (QuickJS bindings) for faster bring-up.
  * V8 via **rusty\_v8** is powerful but heavier to embed.
* Implement a small DOM surface: `Document`, `Element`, `Node`, `Window`, events, timers, `fetch`.
* Event loop: HTML spec’s **task queues** (micro/macro tasks) integrated with Tokio.

**8) Storage & state**

* LocalStorage & Cookies first; IndexedDB later.
* Profile directory per user (cache, cookies, history, settings).

**9) Security & sandboxing**

* MVP runs in-process; plan IPC boundary (Cap’n Proto or flatbuffers) for renderer processes later.
* Strict URL/Origin policy from day one. No dangerous file:// privileges in early builds.

**10) Testing from day 1**

* Unit tests per crate.
* Integrate **Web Platform Tests (WPT)** early (start with subsets for HTML parsing, DOM, CSS selectors).
* Golden tests for layout (DOM -> layout tree -> display list snapshots).

---

# Milestone roadmap (make it fun & shippable)

**M0 – Bootstrap (1–2 weeks)**

* Cargo workspace; tracing/logging; config & profile dirs.
* `platform`: window with GPU surface via wgpu; simple event loop.
* `net`: GET over HTTPS (rustls + hyper), gzip/brotli, charset via **encoding\_rs**.

**M1 – “Hello, HTML” (2–3 weeks)**

* `html`: tokenize+parse to DOM (html5ever).
* Minimal CSS: parse inline `<style>` + `<link>` (no cascade quirks).
* `layout`: block layout for paragraphs/divs; inline text; line breaking.
* `gfx`: paint text and simple backgrounds/borders.
* Navigate to a URL, render static pages (no JS). History/back/forward, reload.

**M2 – Real CSS (3–5 weeks)**

* CSS cascade/inheritance, specificity, selectors, `display:block/inline`.
* Box model correctness; fonts, text metrics; images `<img>`.
* Basic stylesheet fetch & same-origin checks; cache stylesheets.

**M3 – JS + DOM Core (4–6 weeks)**

* Integrate **Boa**; global `window` with timers (`setTimeout`, microtasks).
* DOM bindings for core nodes, events, DOM mutation, querySelector.
* Script execution ordering (parser-blocking), `defer`, `async` basics.

**M4 – Interactivity & perf**

* Input events, link clicks, form submission, simple navigation.
* Incremental layout/paint on DOM mutations.
* Start async fetch API (subset) and CORS.
* Add HTTP/2, start cache eviction policy.

**M5 – Hardening & UX**

* Tabbed UI; crash shielding for renderer (prep for multi-process).
* Settings pane; basic devtools hooks (DOM tree inspector, console log bridge).
* A11y initial support.

After M5 you can chase Flexbox, position, overflow/scrolling, fonts & emoji, WebGL via wgpu, Service Workers, etc.

---

# Key design choices to lock now (so we don’t rewrite later)

* **Async runtime:** Tokio.
* **GPU path:** wgpu now; consider **webrender** if we want the Servo compositor later.
* **HTML/CSS stack:** html5ever + cssparser/selectors (Servo ecosystem).
* **JS engine:** Boa for Rust-native start; keep an abstraction so we could swap.
* **Data store:** sqlite (history, cookies, cache index); sled optional for key-value.
* **IPC & multiprocess:** design as if tabs are separate “renderer” components even before we split processes.

---

# Developer experience

* Hot-reload for UI (feature flag), fast end-to-end example pages in `crates/tools/site-fixtures`.
* `tracing` spans around parse/layout/paint to see time breakdowns.
* `cargo xtask` pattern for dev tasks (run WPT subset, build fixtures, package).

---

# First deliverable we can build today

* Open a window.
* Type a URL.
* Fetch the page over HTTPS.
* Parse HTML into DOM.
* Run block/inline layout.
* Paint text & backgrounds to the GPU.
* Scroll wheel support.

That single vertical slice proves our skeleton: **net → parse → style → layout → paint → present**. Everything else gets layered on.

