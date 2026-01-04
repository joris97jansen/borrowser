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

See [ROADMAP.md](ROADMAP.md) for the full plan. Current focus areas:

1.  **Border Support** (borders + border-radius basics)
    
    -   inputs/buttons look awful without borders; also helps with debugging layout boxes.
        
2.  **CSS Unit Support** (em/rem/% etc.)
    
3.  **Layout Caching & Dirty Flags** (avoid rebuilding style/layout every frame)

4.  **Debug Overlays** (box outlines, line boxes, etc.)

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
- Supports `file://` URLs for local pages and examples
- Streaming images (PNG/JPEG) with async decode + egui textures

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
- Replaced elements: `<img>`, `<input type="text|checkbox|radio">`, `<textarea>`, `<button>` (basic behavior)
- Scrollable viewport with proper page background selection

### **Architecture**
- Fully session-aware message bus (CoreCommand / CoreEvent)
- Separate runtimes for:
  - Networking  
  - HTML parsing  
  - CSS parsing  
- Navigation toolbar widgets live in `gfx::ui::toolbar` (input: `core_types::BrowserInput`, output: intent)
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
- Rust toolchain pinned in `rust-toolchain.toml` (currently `1.92.0`)
- A GPU that supports `wgpu` (almost all modern machines)

Run in release mode for a smooth experience:

```bash
cargo run --release
```

Then try a local example in the URL bar:

- `file://examples/href.html`
- `file://examples/image.html`
