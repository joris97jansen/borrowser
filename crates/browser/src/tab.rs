use crate::interactions::InteractionState;
use crate::page::PageState;
use crate::resources::ResourceManager;
use crate::view::content;
use app_api::RepaintHandle;
use bus::{CoreCommand, CoreEvent};
use egui::Context;
use std::sync::mpsc;
use url::Url;

use html::{
    Node,
    dom_utils::{assign_node_ids, collect_img_srcs, collect_stylesheet_hrefs},
};

use core_types::{RequestId, ResourceKind, TabId};
use css::parse_color;

pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub nav_gen: RequestId,

    loading: bool,
    last_status: Option<String>,

    page: PageState,
    resources: ResourceManager,
    repaint: Option<RepaintHandle>,
    cmd_tx: Option<mpsc::Sender<CoreCommand>>,
    interaction: InteractionState,
}

impl Tab {
    pub fn new(tab_id: TabId) -> Self {
        Self {
            tab_id,
            url: String::new(),
            history: Vec::new(),
            history_index: 0,
            nav_gen: 0,
            loading: false,
            last_status: None,
            page: PageState::new(),
            resources: ResourceManager::new(),
            repaint: None,
            cmd_tx: None,
            interaction: InteractionState::default(),
        }
    }

    // -- Setup Methods ---
    pub fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
    }

    pub fn set_repaint_handle(&mut self, h: RepaintHandle) {
        self.repaint = Some(h);
    }

    pub fn ui_content(&mut self, ctx: &Context) {
        // Drain completed decode jobs and upload textures before painting.
        if self.resources.pump(ctx) {
            self.poke_redraw();
        }

        if let Some(action) = content(
            ctx,
            &mut self.page,
            &mut self.interaction,
            &self.resources,
            self.last_status.as_ref(),
            self.loading,
        ) {
            match action {
                crate::view::PageAction::Navigate(url) => self.navigate_to_new(url),
            }
        }
    }

    // -- Event Handling ---
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        let current = self.nav_gen;
        match evt {
            // HTML networking → parser aansturen
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
                ..
            } if tab_id == self.tab_id && request_id == current => {
                self.page.start_nav(&url);
                self.loading = true;
                self.last_status = Some(format!("Started HTML stream: {url}"));
                self.send_cmd(CoreCommand::ParseHtmlStart {
                    tab_id: self.tab_id,
                    request_id,
                });
                self.poke_redraw();
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                bytes,
                ..
            } if tab_id == self.tab_id && request_id == current => {
                self.send_cmd(CoreCommand::ParseHtmlChunk {
                    tab_id: self.tab_id,
                    request_id,
                    bytes,
                });
            }

            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
            } if tab_id == self.tab_id && request_id == current => {
                self.send_cmd(CoreCommand::ParseHtmlDone {
                    tab_id: self.tab_id,
                    request_id,
                });
                self.last_status = Some(format!("Loaded HTML: {url}"));
                self.poke_redraw();
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
                error,
            } if tab_id == self.tab_id && request_id == current => {
                self.loading = false;
                self.last_status = Some(format!("Network error on {url}: {error}"));
                self.poke_redraw();
            }

            // Parser → DOM snapshot + CSS discovery
            CoreEvent::DomUpdate {
                tab_id,
                request_id,
                dom,
            } if tab_id == self.tab_id && request_id == current => {
                let mut dom = dom;
                assign_node_ids(&mut dom);
                self.page.dom = Some(dom);
                self.page.update_head_metadata();
                self.page.apply_inline_style_blocks();
                self.page.seed_input_values_from_dom();
                self.page.update_visible_text_cache();

                // stylesheets detecteren en fetchen
                if let (Some(dom_ref), Some(base)) =
                    (self.page.dom.as_ref(), self.page.base_url.as_ref())
                {
                    let mut hrefs = Vec::new();
                    collect_stylesheet_hrefs(dom_ref, &mut hrefs);
                    if let Ok(base_url) = Url::parse(base) {
                        for h in hrefs {
                            if let Ok(abs) = base_url.join(&h) {
                                let href = abs.to_string();
                                if self.page.register_css(&href) {
                                    self.send_cmd(CoreCommand::FetchStream {
                                        tab_id: self.tab_id,
                                        request_id: current,
                                        url: href,
                                        kind: ResourceKind::Css,
                                    });
                                }
                            }
                        }
                    }
                }

                // images detecteren en fetchen (media pipeline)
                if let (Some(dom_ref), Some(base)) =
                    (self.page.dom.as_ref(), self.page.base_url.as_ref())
                {
                    let mut srcs = Vec::new();
                    collect_img_srcs(dom_ref, &mut srcs);

                    let cmd_tx = self.cmd_tx.clone();
                    let tab_id = self.tab_id;
                    let request_id = current;

                    if let Ok(base_url) = Url::parse(base) {
                        for src in srcs {
                            if let Ok(abs) = base_url.join(&src) {
                                let url = abs.to_string();
                                self.resources.request_image(url, |url| {
                                    if let Some(tx) = &cmd_tx {
                                        let _ = tx.send(CoreCommand::FetchStream {
                                            tab_id,
                                            request_id,
                                            url,
                                            kind: ResourceKind::Image,
                                        });
                                    }
                                });
                            }
                        }
                    }
                }

                let pending = self.page.pending_count();
                self.loading = pending > 0;
                if pending > 0 {
                    self.last_status =
                        Some(format!("Loaded HTML • fetching {pending} stylesheet(s)…"));
                }
                self.poke_redraw();
            }

            // CSS streaming → CSS runtime
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                bytes,
            } if tab_id == self.tab_id && request_id == current => {
                self.send_cmd(CoreCommand::CssChunk {
                    tab_id: self.tab_id,
                    request_id,
                    url,
                    bytes,
                });
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if tab_id == self.tab_id && request_id == current => {
                self.resources.on_network_chunk(&url, &bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
            } if tab_id == self.tab_id && request_id == current => {
                self.send_cmd(CoreCommand::CssDone {
                    tab_id: self.tab_id,
                    request_id,
                    url,
                });
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
            } if tab_id == self.tab_id && request_id == current => {
                self.resources.on_network_done(&url, self.repaint.clone());
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                error,
            } if tab_id == self.tab_id && request_id == current => {
                self.send_cmd(CoreCommand::CssDone {
                    tab_id: self.tab_id,
                    request_id,
                    url: url.clone(),
                });
                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(format!(
                    "Stylesheet error on {url}: {error} ({} remaining)",
                    remaining
                ));
                self.poke_redraw();
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                error,
            } if tab_id == self.tab_id && request_id == current => {
                self.resources.on_network_error(&url, error);
            }

            // CSS runtime → apply styles
            CoreEvent::CssParsedBlock {
                tab_id,
                request_id,
                css_block,
                ..
            } if tab_id == self.tab_id && request_id == current => {
                self.page.apply_css_block(&css_block);
                self.poke_redraw();
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                url,
            } if tab_id == self.tab_id && request_id == current => {
                self.page.mark_css_done(&url);
                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(if remaining > 0 {
                    format!("Stylesheet loaded ({} remaining)", remaining)
                } else {
                    "All stylesheets loaded".to_string()
                });
                self.poke_redraw();
            }

            _ => {}
        }
    }

    // -- Navigation Methods ---
    pub fn navigate_to_new(&mut self, url: String) {
        let url = match self.normalize_url(&url) {
            Ok(url) => url,
            Err(err) => {
                self.loading = false;
                self.last_status = Some(err.to_string());
                self.poke_redraw();
                return;
            }
        };
        self.url = url.clone();

        // record to history (truncate forward branch)
        self.history.truncate(self.history_index + 1);
        self.history.push(url.clone());
        self.history_index = self.history.len() - 1;

        self.start_fetch(url);
    }

    pub fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    pub fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    pub fn refresh(&mut self) {
        if let Some(url) = self.history.get(self.history_index).cloned() {
            self.load_current(url);
        }
    }

    // -- Internal Helpers ---
    fn start_fetch(&mut self, url: String) {
        if self.nav_gen > 0 {
            self.send_cmd(CoreCommand::CancelRequest {
                tab_id: self.tab_id,
                request_id: self.nav_gen,
            });
        }
        self.interaction.clear_for_navigation();
        self.nav_gen = self.nav_gen.wrapping_add(1);
        let request_id = self.nav_gen;

        self.loading = true;
        self.last_status = Some(format!("Fetching {url} …"));
        self.page.start_nav(&url);

        self.send_cmd(CoreCommand::FetchStream {
            tab_id: self.tab_id,
            request_id,
            url,
            kind: ResourceKind::Html,
        });
        self.poke_redraw();
    }

    fn load_current(&mut self, url: String) {
        // do NOT touch history; just fetch the given URL
        self.interaction.clear_for_navigation();
        self.url = url.clone();
        self.start_fetch(url);
    }

    fn normalize_url(&mut self, url: &str) -> Result<String, &'static str> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err("Cannot navigate to an empty URL");
        }

        // Already a full URL with scheme we support
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("file://")
        {
            return Ok(trimmed.into());
        }

        // For everything else, keep your old "guess https" behavior
        Ok(format!("https://{trimmed}"))
    }

    /// Derive a human-friendly label from the URL:
    /// - for http/https: "host — last/path/segment"
    /// - for file://: just the file name
    /// - otherwise: empty string if parse fails
    fn url_label(&self) -> String {
        if self.url.is_empty() {
            return String::new();
        }

        if let Ok(url) = url::Url::parse(&self.url) {
            // file:// → show file name only
            let file_name = if url.scheme() == "file" {
                url.to_file_path().ok().and_then(|path| {
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
            } else {
                None
            };

            if let Some(name) = file_name {
                return name;
            }

            // http/https → "host — last-segment"
            let mut label = String::new();

            if let Some(host) = url.host_str() {
                label.push_str(host);
            }

            if let Some(last_seg) = url
                .path_segments()
                .and_then(|mut segs| segs.rfind(|s| !s.is_empty()))
            {
                if !label.is_empty() {
                    label.push_str(" — ");
                }
                label.push_str(last_seg);
            }

            return label;
        }

        // Fallback: nothing nice we can format
        String::new()
    }

    pub fn display_title(&self) -> String {
        // 1) Prefer <title> from head
        if let Some(title) = self.page.head.title.as_ref() {
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                return elide_end(trimmed, 30);
            }
        }

        // 2) If still loading and we have a URL, show a loading label
        if self.loading && !self.url.is_empty() {
            let core = self.url_label();
            if core.is_empty() {
                return "Loading…".to_string();
            } else {
                // keep total length modest
                return format!("Loading… — {}", elide_end(&core, 24));
            }
        }

        // 3) No title, not loading → fall back to URL-based label
        let url_label = self.url_label();
        if !url_label.is_empty() {
            return elide_end(&url_label, 30);
        }

        // 4) Absolute last fallback:
        if !self.url.is_empty() {
            return elide_end(&self.url, 30);
        }

        "New Tab".to_string()
    }

    fn send_cmd(&self, cmd: CoreCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }

    fn poke_redraw(&self) {
        if let Some(repaint) = &self.repaint {
            repaint.request_now();
        }
    }

    pub fn inherited_color(node: &Node, ancestors: &[Node]) -> (u8, u8, u8, u8) {
        fn find_on(node: &Node) -> Option<(u8, u8, u8, u8)> {
            match node {
                Node::Element { style, .. } => style
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("color"))
                    .and_then(|(_, v)| parse_color(v)),
                _ => None,
            }
        }
        if let Some(c) = find_on(node) {
            return c;
        }
        for a in ancestors {
            if let Some(c) = find_on(a) {
                return c;
            }
        }
        (0, 0, 0, 255) // default black
    }

    pub fn page_background(dom: &Node) -> Option<(u8, u8, u8, u8)> {
        fn from_element(node: &Node, want: &str) -> Option<(u8, u8, u8, u8)> {
            match node {
                Node::Element { name, style, .. } if name.eq_ignore_ascii_case(want) => style
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("background-color"))
                    .and_then(|(_, v)| parse_color(v)),
                _ => None,
            }
        }
        if let Node::Document { children, .. } = dom {
            for c in children {
                if let Some(c1) = from_element(c, "html") {
                    return Some(c1);
                }
                if let Node::Element {
                    children: html_kids,
                    ..
                } = c
                {
                    for k in html_kids {
                        if let Some(c2) = from_element(k, "body") {
                            return Some(c2);
                        }
                    }
                }
            }
        }
        None
    }
}

fn elide_end(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_owned();
    }

    let keep = max_chars.saturating_sub(1);
    let mut s: String = chars[..keep].iter().collect();
    s.push('…');
    s
}
