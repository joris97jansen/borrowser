# The Borrowser 🦀🌐  
*A web browser engine written from scratch in Rust.*

Borrowser is a learning-focused project that builds a real browser stack—HTML parsing, CSS cascade, layout, rendering, networking, and desktop UI—from the ground up.  
The goal: deeply understand *every* part of a browser engine, while keeping code clean, modular, and production-quality.

---

## 🙋 Why “Borrowser”?

Think **Borrow Checker** + **Browser** → **Borrowser**.  
A full browser engine built with Rust’s safety and clarity, nothing borrowed from Chromium 😉.

---
## Next steps

1.  **HTML Replaced Elements & Form Controls** (Phase 1: `<img>`, `<a>`, `<input type=text>`)
    
2.  **Border Support** (borders + border-radius basics)
    
    -   inputs/buttons look awful without borders; also helps with debugging layout boxes.
        
3.  **CSS Color Support** (to reduce “everything looks wrong” quickly)
    
4.  **CSS Unit Support** (em/rem/% etc.)
    
5.  **Inline Formatting Polishing** (baseline/vertical-align, better line-height behavior, etc.)
---

## ✨ Current Capabilities

Borrowser currently supports:

### **Browser Shell**
- Native desktop window (via `winit`)
- Custom tab strip with navigation (Back / Forward / Refresh / New Tab)
- Independent per-tab session state and navigation history
- URL bar with proper navigation handling

### **Networking**
- Streaming HTML over HTTP
- Parallel streaming of external CSS files

### **HTML & CSS**
- HTML tokenizer + DOM tree builder
- CSS parser: selectors, specificity, inline styles
- Cascade + computed styles (inheritance + defaults)
- Incremental DOM/CSS updates via multi-threaded runtimes

### **Layout & Rendering**
- Styled tree construction
- Block layout engine (CSS box model, margins, padding)
- Inline layout engine with:
  - whitespace collapsing  
  - word-wrapping  
  - line boxes + fragments  
- Painting backgrounds + text via `egui` + `wgpu`
- Scrollable viewport with proper page background selection

### **Architecture**
- Fully session-aware message bus (CoreCommand / CoreEvent)
- Separate runtimes for:
  - Networking  
  - HTML parsing  
  - CSS parsing  
- Thread-safe, highly modular design

---

## 🧩 Documentation

Borrowser is built to be understood, not black magic.

- 📘 **[Architecture Overview](ARCHITECTURE.md)**  
  Deep dive into every subsystem: DOM, CSS cascade, layout, runtimes, rendering pipeline, message bus, and threading model.

- 🗺️ **[Project Roadmap](ROADMAP.md)**  
  The long-term vision, planned features, and sequencing of future work.

---

## 🚀 Running Borrowser

Requirements:
- Latest stable Rust
- A GPU that supports `wgpu` (almost all modern machines)

Run in release mode for a smooth experience:

```bash
cargo run --release
