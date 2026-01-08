use crate::page::PageState;
use crate::resources::ResourceManager;
use app_api::RepaintHandle;
use bus::{CoreCommand, CoreEvent};
use gfx::input::InteractionState;
use std::sync::mpsc;

use html::{Node, dom_utils::assign_node_ids};

use core_types::{RequestId, ResourceKind, TabId};
use css::parse_color;

mod discovery;
mod nav;
mod ui;

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

    // -- Event Handling ---
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        match evt {
            // HTML networking → parser aansturen
            CoreEvent::NetworkStart {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_start(url, request_id);
            }

            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url: _,
                bytes,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_chunk(bytes, request_id);
            }

            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_done(url, request_id);
            }

            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Html,
                url,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_html_network_error(url, error);
            }

            // Parser → DOM snapshot + CSS discovery
            CoreEvent::DomUpdate {
                tab_id,
                request_id,
                dom,
            } if self.is_current(tab_id, request_id) => {
                self.on_dom_update(dom, request_id);
            }

            // CSS streaming → CSS runtime
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_chunk(url, bytes, request_id);
            }
            CoreEvent::NetworkChunk {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                bytes,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_chunk(url, bytes);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_done(url, request_id);
            }
            CoreEvent::NetworkDone {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_done(url);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Css,
                url,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_network_error(url, error, request_id);
            }
            CoreEvent::NetworkError {
                tab_id,
                request_id,
                kind: ResourceKind::Image,
                url,
                error,
            } if self.is_current(tab_id, request_id) => {
                self.on_image_network_error(url, error);
            }

            // CSS runtime → apply styles
            CoreEvent::CssParsedBlock {
                tab_id,
                request_id,
                css_block,
                ..
            } if self.is_current(tab_id, request_id) => {
                self.on_css_parsed_block(css_block);
            }
            CoreEvent::CssSheetDone {
                tab_id,
                request_id,
                url,
            } if self.is_current(tab_id, request_id) => {
                self.on_css_sheet_done(url);
            }

            _ => {}
        }
    }

    fn is_current(&self, tab_id: TabId, request_id: RequestId) -> bool {
        tab_id == self.tab_id && request_id == self.nav_gen
    }

    fn on_html_network_start(&mut self, url: String, request_id: RequestId) {
        self.page.start_nav(&url);
        self.loading = true;
        self.last_status = Some(format!("Started HTML stream: {url}"));
        self.send_cmd(CoreCommand::ParseHtmlStart {
            tab_id: self.tab_id,
            request_id,
        });
        self.poke_redraw();
    }

    fn on_html_network_chunk(&mut self, bytes: Vec<u8>, request_id: RequestId) {
        self.send_cmd(CoreCommand::ParseHtmlChunk {
            tab_id: self.tab_id,
            request_id,
            bytes,
        });
    }

    fn on_html_network_done(&mut self, url: String, request_id: RequestId) {
        self.send_cmd(CoreCommand::ParseHtmlDone {
            tab_id: self.tab_id,
            request_id,
        });
        self.last_status = Some(format!("Loaded HTML: {url}"));
        self.poke_redraw();
    }

    fn on_html_network_error(&mut self, url: String, error: String) {
        self.loading = false;
        self.last_status = Some(format!("Network error on {url}: {error}"));
        self.poke_redraw();
    }

    fn on_dom_update(&mut self, mut dom: Node, request_id: RequestId) {
        assign_node_ids(&mut dom);
        self.page.dom = Some(dom);
        self.page.update_head_metadata();
        self.page.apply_inline_style_blocks();
        self.page.seed_input_values_from_dom();
        self.page.update_visible_text_cache();

        self.discover_resources(request_id);

        let pending = self.page.pending_count();
        self.loading = pending > 0;
        if pending > 0 {
            self.last_status = Some(format!("Loaded HTML • fetching {pending} stylesheet(s)…"));
        }
        self.poke_redraw();
    }

    fn on_css_network_chunk(&mut self, url: String, bytes: Vec<u8>, request_id: RequestId) {
        self.send_cmd(CoreCommand::CssChunk {
            tab_id: self.tab_id,
            request_id,
            url,
            bytes,
        });
    }

    fn on_image_network_chunk(&mut self, url: String, bytes: Vec<u8>) {
        self.resources.on_network_chunk(&url, &bytes);
    }

    fn on_css_network_done(&mut self, url: String, request_id: RequestId) {
        self.send_cmd(CoreCommand::CssDone {
            tab_id: self.tab_id,
            request_id,
            url,
        });
    }

    fn on_image_network_done(&mut self, url: String) {
        self.resources.on_network_done(&url, self.repaint.clone());
    }

    fn on_css_network_error(&mut self, url: String, error: String, request_id: RequestId) {
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

    fn on_image_network_error(&mut self, url: String, error: String) {
        self.resources.on_network_error(&url, error);
    }

    fn on_css_parsed_block(&mut self, css_block: String) {
        self.page.apply_css_block(&css_block);
        self.poke_redraw();
    }

    fn on_css_sheet_done(&mut self, url: String) {
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
