Here’s a clean starting point for your repo’s **README.md** that explains what you have now, why, and how things connect. It’s written for someone new to the codebase but curious about the architecture.

---

```markdown
# Borrowser 🦀🌐

A learning project: building a desktop **web browser in Rust**, from scratch, with a focus on 
understanding every piece of the stack — windowing, rendering, event loops, UI, and networking.

---

## ✨ Current State

Right now the browser can:

- Open a desktop window (via [winit](https://github.com/rust-windowing/winit)).
- Render a GUI (via [egui](https://github.com/emilk/egui), [egui-wgpu](https://github.com/emilk/egui/tree/master/crates/egui-wgpu)).
- Show a simple **URL bar** and a "Go" button.
- Fetch a web page on demand (via [reqwest](https://docs.rs/reqwest)).
- Display status + the first 500 characters of the page as a preview.

It’s very early — the goal is to learn the fundamentals step by step.

---

## 🏗️ Architecture

The project is organized into **crates** (sub-packages):

```

src/main.rs
crates/
├── app_api     # Shared traits and types between platform and apps
├── browser     # The "BrowserApp" implementation (UI + state)
├── gfx         # Rendering layer (egui + wgpu glue)
├── net         # Networking crate (fetch_text + FetchResult)
└── platform    # Platform integration: window, event loop, proxy

````

### Core flow

1. **`platform`** creates the main window & event loop (`winit`).
2. **`app_api`** defines a `UiApp` trait:
   - `ui(&mut self, &egui::Context)` – draw the UI
   - `set_net_callback(NetCallback)` – store a callback for async results
   - `on_net_result(FetchResult)` – update state when fetches complete
3. **`browser`** implements `UiApp` with a `BrowserApp`:
   - Renders the URL bar & status
   - Triggers `net::fetch_text` when the user clicks "Go"
4. **`net`** performs the blocking HTTP fetch in a background thread.
   - Builds a `FetchResult` with status, bytes, snippet, or error.
   - Invokes the callback provided by `platform`.
5. **`platform`** receives results on the main thread via an **`EventLoopProxy<UserEvent>`**.
   - Forwards them back into the app with `on_net_result`.
6. **`gfx`** handles rendering every frame using `egui` + `wgpu`.

---

## 🔄 Event Flow Example

```text
[User presses Go]
   ↓
BrowserApp → net::fetch_text(url, cb)
   ↓ (threaded HTTP request)
net crate calls cb(FetchResult)
   ↓
platform::UserEvent::NetResult(FetchResult)
   ↓
PlatformApp forwards to app.on_net_result(result)
   ↓
BrowserApp updates status / preview
   ↓
gfx::Renderer draws updated UI
````

---

## 🚀 Running

Requirements:

* Rust (latest stable)
* System libraries for wgpu (Vulkan/Metal/DirectX backend available)

```bash
cargo run
```

This will:

* Open a window titled **Borrowser**,
* Show the URL bar,
* Fetch `https://example.com` (default),
* Display the status and snippet.

---

## 📚 Next Steps

* [ ] Add **request IDs** so stale results are ignored.
* [ ] Show **loading spinner** properly in UI.
* [ ] Improve error handling and cancellation.
* [ ] Render actual HTML (right now it’s just text).
* [ ] Explore history, tabs, etc.

---

## 🙋 Why "Borrowser"?

Because this is not a production browser — it’s a **Rust learning experiment**.
Think “Borrow checker” + “Browser” = Borrowser. 🦀

---

```

---

Would you like me to also include a **short architecture diagram** (ASCII style with boxes/arrows) in the README to make the proxy/event flow even clearer?
```
