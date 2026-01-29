```md
# The Borrowser 🦀🌐  
*A browser engine written from scratch in Rust.*

Borrowser is a learning-focused but **engine-grade** browser project that builds a real web stack from first principles:

- HTML parsing  
- CSS cascade and layout  
- Rendering and input handling  
- Networking and concurrency  

Everything is implemented explicitly, with correctness, clarity, and long-term architecture as the primary goals.

Borrowser is not a wrapper around an existing engine.  
It is built to understand — and eventually **own** — every layer of the rendering pipeline.

---

## 🙋 Why “Borrowser”?

**Borrowser = Borrow Checker + Browser**

The project is written in Rust and intentionally embraces Rust’s strengths:
- explicit ownership
- deterministic behavior
- safe concurrency
- clarity over cleverness

The name reflects the project’s original scope:  
**a browser engine, built properly, without shortcuts.**

---

## 🧱 What Borrowser is today

Today, Borrowser is **a standalone browser engine** with a custom desktop shell.

It follows a modern multi-runtime architecture:


```

HTML / CSS → Parsing → DOM → Style → Layout → Paint → GPU

```

### Current focus areas

Borrowser is primarily focused on:
- incremental HTML parsing
- CSS cascade correctness
- deterministic layout
- explicit rendering pipelines
- message-driven concurrency

The goal is not feature parity with Chromium, but **engine-grade foundations** that are:
- understandable
- testable
- extensible
- performance-conscious

---

## ✨ Current capabilities

### **Browser shell**
- Native desktop window (via `winit`)
- Custom tab strip and navigation
- Independent per-tab session state
- URL bar and navigation history

### **Networking**
- Streaming HTML over HTTP
- Parallel streaming of external CSS
- `file://` URL support for local pages
- Async image loading (PNG/JPEG)

### **HTML & CSS**
- Incremental HTML tokenizer and DOM builder
- CSS parsing (selectors, specificity, inline styles)
- Cascade and computed styles (inheritance + defaults)
- Multi-threaded parsing runtimes

### **Layout & rendering**
- Styled tree construction
- Block layout engine (CSS box model)
- Inline layout engine with:
  - whitespace collapsing
  - word wrapping
  - line boxes and fragments
- Custom painting using `egui` + `wgpu`
- Basic replaced elements (`img`, `input`, `textarea`, `button`)
- Scrollable viewport with correct background behavior

### **Architecture**
- Session-aware message bus (`CoreCommand` / `CoreEvent`)
- Dedicated runtimes for:
  - networking
  - HTML parsing
  - CSS parsing
- No shared mutable state across threads
- Strong crate boundaries and testability

---

## 🧭 Project direction: beyond a browser

Borrowser is intentionally presented **today** as a browser engine.

However, its architecture is designed with a broader goal in mind.

### Why?

Modern computing still largely follows this model:


```

Device → OS → Apps → Cloud / AI

```

This leads to:
- fragmented state
- fragile syncing
- app-owned data
- AI bolted on at the edges
- recovery as a special case

While browsers have evolved enormously, the **core model has not**.

---

## 🔮 Looking ahead: Continuum

Borrowser is being built as the **UI runtime foundation** for a future project called **Continuum**.

**Continuum** is an experimental concept for a **state-native, event-driven user operating system**, built on top of Borrowser.

The core idea is a different stack:


```

Event → State → UI / Cloud / AI

```

Where:
- state is canonical
- UI is a deterministic projection of state
- cloud replication is native, not “sync”
- AI observes events and proposes actions
- devices are temporary participants, not authorities

In this future architecture:
- **Borrowser remains the engine**
- **Continuum becomes the user OS built on top of it**

The current repository focuses entirely on the engine layer.  
OS-level work will begin only once the HTML/CSS runtime is sufficiently complete and stable.

Nothing in Borrowser’s design is accidental — it is built to support that future without rewrites.

---

## 🧪 Why build this?

Borrowser exists to deeply understand and explore:

- how browsers actually work
- how modern UI systems can be deterministic
- how event-driven state can simplify complexity
- how HTML/CSS can function as a general UI runtime

Continuum exists as the *logical extension* of those foundations.

This project prioritizes:
- correctness over shortcuts
- clarity over cleverness
- explicit state over hidden mutation
- architecture that can evolve over decades

---

## 🚧 Non-goals (for now)

- Competing with Chrome or Safari
- Full web-platform parity
- Writing a kernel
- Shipping a consumer OS

This is about **foundations**, not polish or hype.

---

## 📚 Documentation

- 📘 **Architecture** — deep dive into parsing, layout, rendering, and concurrency  
- 🧠 **ADRs** — design decisions and trade-offs, documented explicitly  
- 🗺️ **Roadmap** — planned milestones and long-term direction  

---

## 🚀 Running Borrowser

Requirements:
- Rust toolchain (pinned in `rust-toolchain.toml`)
- A GPU supported by `wgpu`

Run in release mode:

```bash
cargo run --release

```

Try local examples:

-   `file://examples/basic.html`
    
-   `file://examples/layout.html`
    

----------

## ✨ Final note

Borrowser is what this project **is today**.

Continuum is what this architecture is **intentionally moving toward**.

The goal is not to rush that transition —  
but to earn it by building the right foundations first.

```