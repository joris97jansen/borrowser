# Borrowser Roadmap
*A minimal browser engine built with clarity, correctness, and extensibility in mind.*

This roadmap reflects the current state of the Borrowser engine and the planned direction for layout, styling, rendering, and architecture.  
Completed features are marked, ongoing tasks use `[ ]`, and future structural improvements are clearly broken down.

---

# 📌 Upcoming Work

## A. Layout Engine Enhancements

### A1. Display Property Support
Introduce `display` as a first-class computed property.

- [✅] Add `Display` enum to `ComputedStyle` (`Block`, `Inline`, `InlineBlock`, `ListItem`, …)
- [ ✅] Assign default display per element (span=inline, div=block, etc.)
- [✅] Replace `is_inline_element_name` with computed `display`
- [ ] Add support for:
  - [ ] `display: inline-block`
  - [ ] `display: list-item` (including marker)
- [ ] Parse `display` from CSS rules

---

### A2. Unify Layout Passes
Currently:
- First pass: `layout_block_subtree`  
- Second pass: `recompute_block_heights` (authoritative)

Goal:
- [ ] Collapse into a single consistent layout pipeline  
- [ ] Inline layout is integrated at the correct stage  
- [ ] Reduce duplication of geometry logic  
- [ ] Make the layout engine easier to reason about and extend

---

### A3. Width & Size Properties
Support explicit sizing, enabling more realistic layouts.

- [ ] Parse and support:
  - [ ] `width`
  - [ ] `min-width`
  - [ ] `max-width`
- [ ] Content width shrink-to-fit for inline-block elements
- [ ] Percentage widths (long-term)

---

### A4. Border Support  
Add missing part of the CSS box model.

- [ ] Parse `border-width`, `border-color`, `border-style`
- [ ] Paint border around border box  
- [ ] Adjust layout to include border thickness  
- [ ] Respect border-radius (much later)

---

## B. Debugging & Visualization Tools

### B1. Layout Debug Overlays
Developer-only visualization:

- [ ] Draw border boxes
- [ ] Draw padding area (semi-transparent)
- [ ] Draw margins
- [ ] Draw line boxes
- [ ] Toggle overlays from UI (`D` or debug menu)

---

## C. Performance & Architecture

### C1. Cache Style & Layout Trees
Move toward a real browser-style incremental pipeline.

- [ ] Add `style_root: StyledNode` to `PageState`
- [ ] Add `layout_root: LayoutBox` to `PageState`
- [ ] Compute style/layout **only when needed**
  - [ ] When DOM changes
  - [ ] When CSS changes
  - [ ] When viewport changes
- [ ] Otherwise reuse cached trees

This will eliminate 80–90% of current per-frame work.

---

### C2. Dirty Flags & Incremental Layout

- [ ] Add boolean flags:
  - `style_dirty`
  - `layout_dirty`
- [ ] Automatic propagation:
  - DOM change → style_dirty
  - style_dirty → layout_dirty
- [ ] Only recompute affected parts (future enhancement)
- [ ] Possibly adopt a node-based “layout dirty bits” system

---

### C3. Resize-Aware Layout

- [ ] Detect viewport size changes in renderer
- [ ] Mark layout as dirty only on size change
- [ ] Eventually support media queries (future)

---

## D. CSS & Text Engine Improvements

### D1. More CSS Properties
- [ ] `white-space: pre`, `nowrap`, `pre-wrap`
- [ ] `font-weight`
- [ ] `font-style`
- [ ] Color inheritance improvements
- [ ] Background positioning (long-term)

---

### D2. Advanced Text Features (Long-term)
- [ ] Caret positioning
- [ ] Selection highlight
- [ ] Bidirectional text (very far future)
- [ ] Ligatures / shaping (requires HarfBuzz or rustybuzz)

---

## E. Browser UX Polish

### E1. Metadata & Tabs
- [ ] Display HTTP status code in tab UI
- [ ] Show favicon (simple `<link rel="icon">` parsing)
- [ ] Optional “unsaved changes” marker for forms
- [ ] Improve title fallback logic

---

# 🌱 Philosophy of the Roadmap

Borrowser is intentionally built with:

- Clean layering (DOM → Style → Layout → Paint)
- Real browser concepts (box model, inline formatting context)
- A focus on **correctness over shortcuts**
- Small, iterative, understandable steps

This roadmap helps maintain that direction without overwhelming the project.

---

# 🧭 Contributing / Updating

Whenever a feature is completed:

1. Check the corresponding checkbox  
2. Link to the PR or commit  
3. Add new TODOs as the engine grows  

The roadmap lives in the repo so it evolves *with* the engine.



After networking, CSS, and rendering, the next major areas to revisit are:

DOM / Web Platform Runtime
The DOM needs to become a real runtime-owned platform surface, not just a parsed tree.
This means:

DOM mutation model
event dispatch
live document state
script-facing APIs later
JavaScript Integration
Once parsing, styling, and rendering are stronger, the next huge gap is JS driving the page.
That means:

event loop/task model
timers/microtasks
DOM bindings
script-triggered style/layout invalidation
Security / Origin / Policy Model
To be production-grade, the browser needs clear policy boundaries:

origin model
same-origin rules
file:// policy
localhost/local resource rules
subresource restrictions
sandboxing direction
Text / Fonts / Shaping
This is a very big “professional polish” area:

proper font fallback
shaping
metrics consistency
selection/caret correctness
international text handling
Forms, Editing, and Interaction Fidelity
If you want real websites to feel right:

input controls
selection/editing behavior
keyboard focus/navigation
form submission/state
textarea/content-editing quality
Images / Media / Asset Pipeline
More professional decoding and lifecycle handling:

image formats and decode policy
font loading
media pipeline later
cache/reuse/invalidation
Testing / Standards / WPT Expansion
Professional engines live or die by regression discipline:

more WPT coverage
real-page regression suites
perf/allocation guards
runtime invariants and observability
Accessibility
Eventually essential for “professional browser engine” status:

accessible tree
roles/names/states
focus semantics
screen-reader integration direction
If I had to prioritize the next tracks after those milestones, I would go:

DOM/runtime ownership
JavaScript/event loop
Security/origin model
Text/fonts/editing
Accessibility and broader platform fidelity