# Borrowser 🦀🌐

A learning project: building a **web browser in Rust**, from scratch, with a focus on 
understanding every piece of the stack: windowing, rendering, event loops, UI, and networking.

---

## 🙋 Why "Borrowser"?

It’s a **Rust learning experiment**, think “Borrow checker” + “Browser” = Borrowser. 🦀

P.S. nothing borrowed from Chormium *wink*.

---

## ✨ Current State

Right now the browser can:

- Open a desktop window (via [winit](https://github.com/rust-windowing/winit)).
- Render a GUI (via [egui](https://github.com/emilk/egui), [egui-wgpu](https://github.com/emilk/egui/tree/master/crates/egui-wgpu)).
- Show a simple **URL bar** and a "Go" button.
- Fetch a web page on demand (via [reqwest](https://docs.rs/reqwest)).
- Display status + the first 500 characters of the page as a preview.

It’s very early, and the goal is to learn the fundamentals step by step.

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
Coming Soon:
├── css
├── html
└── js
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

## 🧭 Proxy & Event Architecture (at a glance)

```

+-------------------+                 +--------------------------+
|   Browser App     |                 |        net crate         |
|  (UiApp impl)     |                 |  (threaded HTTP fetch)   |
|-------------------|                 |--------------------------|
| - url             |   fetch_text()  | reqwest::blocking::get() |
| - loading         | ───────────────▶|  build FetchResult       |
| - last_status     |                 |  cb(FetchResult)         |
| - last_preview    |◀─────────────── |                          |
| - net_cb: Option  |    (callback)   +--------------------------+
+---------┬---------+
│ set_net_callback(NetCallback)
│ (installed by platform at startup)
│
│                     EventLoopProxy<UserEvent>
│                  (single proxy, cloned as needed)
│
+---------▼---------+   send_event(UserEvent::NetResult)    +-------------------+
|     Platform      |◀──────────────────────────────────────|   net callback    |
|  (winit + gfx)    |                                       | (closure created  |
|-------------------|         request_redraw()              |  in platform)     |
| - EventLoop       |──────────────────────────────────────▶+-------------------+
| - EventLoopProxy  |
| - UserEvent enum  |
|   • Tick          |      +------------------------------+
|   • NetResult     |      |   UserEvent handling         |
|-------------------|      |------------------------------|
| on user_event:    |      | NetResult(result) =>         |
|   app.on_net_result(result)  (on main thread)            |
| on Redraw:        |      | Tick => window.request_redraw |
|   gfx::Renderer.render()    (approx. 60Hz)               |
+-------------------+      +------------------------------+

```

**Legend**
- **Single proxy:** One `EventLoopProxy<UserEvent>` created once in `run_with`, then `clone()`d wherever needed.
- **Callback:** The net callback is a closure that captures the proxy and does `proxy.send_event(UserEvent::NetResult(result))`.
- **Threading:** HTTP runs on a background thread in `net`; UI & event dispatch run on the main thread.

**Key contracts**
- `UiApp::set_net_callback(NetCallback)`: platform installs the proxy-backed callback into the app.
- `UiApp::on_net_result(FetchResult)`: platform delivers results to the app on the main thread.
- `net::fetch_text(url, cb)`: background fetch; calls `cb(FetchResult)` when done.
- `UserEvent`: enum carrying cross-thread messages (`Tick`, `NetResult`).

**Why this shape?**
- Keeps **networking** off the UI thread.
- Centralizes cross-thread delivery through **one proxy** → simpler lifecycle, predictable ordering.
- Keeps the **app** ignorant of windowing details and the **platform** ignorant of HTTP details.

---

## 🚀 Running

Requirements:

* Rust (latest stable)

```bash
cargo run
```

This will:

* Open a window titled **Borrowser**,
* Show the URL bar,
* Fetch `https://example.com` (default),
* Display the status and snippet.
* You can adjust the URL for any other URL.

---

## 📚 Next Steps

* [ ] Add **request IDs** so stale results are ignored.
* [ ] Show **loading spinner** properly in UI.
* [ ] Improve error handling and cancellation.
* [ ] Render actual HTML (right now it’s just text).
* [ ] Explore history, tabs, etc.

---
