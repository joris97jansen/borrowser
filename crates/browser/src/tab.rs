use url::Url;
use crate::page::PageState;
use bus::{
    CoreCommand,
    CoreEvent,
};
use std::sync::mpsc;
use crate::view::{
    content,
};
use egui::{
    Context,
};
use app_api::{
    RepaintHandle,
};

use html::{
    Node,
    dom_utils::collect_stylesheet_hrefs,
};

use css::{
    parse_color,
};
use core_types::{
    ResourceKind,
    TabId,
    RequestId,
};


pub struct Tab {
    pub tab_id: TabId,

    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub nav_gen: RequestId,

    loading: bool,
    last_status: Option<String>,

    page: PageState,
    repaint: Option<RepaintHandle>,
    cmd_tx: Option<mpsc::Sender<CoreCommand>>,
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
            repaint: None,
            cmd_tx: None,
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
        content(ctx, &self.page, self.last_status.as_ref(), self.loading);
    }

    // -- Event Handling ---
    pub fn on_core_event(&mut self, evt: CoreEvent) {
        let current = self.nav_gen;
        match evt {
            // HTML networking → parser aansturen
            CoreEvent::NetworkStart { tab_id, request_id, kind: ResourceKind::Html, url, .. }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.page.start_nav(&url);
                self.loading = true;
                self.last_status = Some(format!("Started HTML stream: {url}"));
                self.send_cmd(CoreCommand::ParseHtmlStart { tab_id: self.tab_id, request_id });
                self.poke_redraw();
            }

            CoreEvent::NetworkChunk { tab_id, request_id, kind: ResourceKind::Html, bytes, .. }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.send_cmd(CoreCommand::ParseHtmlChunk { tab_id: self.tab_id, request_id, bytes });
            }

            CoreEvent::NetworkDone { tab_id, request_id, kind: ResourceKind::Html, url }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.send_cmd(CoreCommand::ParseHtmlDone { tab_id: self.tab_id, request_id });
                self.last_status = Some(format!("Loaded HTML: {url}"));
                self.poke_redraw();
            }

            CoreEvent::NetworkError { tab_id, request_id, kind: ResourceKind::Html, url, error }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.loading = false;
                self.last_status = Some(format!("Network error on {url}: {error}"));
                self.poke_redraw();
            }

            // Parser → DOM snapshot + CSS discovery
            CoreEvent::DomUpdate { tab_id, request_id, dom }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.page.dom = Some(dom);
                self.page.update_visible_text_cache();

                // stylesheets detecteren en fetchen
                if let (Some(dom_ref), Some(base)) = (self.page.dom.as_ref(), self.page.base_url.as_ref()) {
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

                let pending = self.page.pending_count();
                self.loading = pending > 0;
                if pending > 0 {
                    self.last_status = Some(format!("Loaded HTML • fetching {pending} stylesheet(s)…"));
                }
                self.poke_redraw();
            }

            // CSS streaming → CSS runtime
            CoreEvent::NetworkChunk { tab_id, request_id, kind: ResourceKind::Css, url, bytes }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.send_cmd(CoreCommand::CssChunk { tab_id: self.tab_id, request_id, url, bytes });
            }
            CoreEvent::NetworkDone { tab_id, request_id, kind: ResourceKind::Css, url }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.send_cmd(CoreCommand::CssDone { tab_id: self.tab_id, request_id, url });
            }
            CoreEvent::NetworkError { tab_id, request_id, kind: ResourceKind::Css, url, error }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.send_cmd(CoreCommand::CssDone { tab_id: self.tab_id, request_id, url: url.clone() });
                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(format!("Stylesheet error on {url}: {error} ({} remaining)", remaining));
                self.poke_redraw();
            }

            // CSS runtime → apply styles
            CoreEvent::CssParsedBlock { tab_id, request_id, css_block, .. }
                if tab_id == self.tab_id && request_id == current =>
            {
                self.page.apply_css_block(&css_block);
                self.poke_redraw();
            }
            CoreEvent::CssSheetDone { tab_id, request_id, url }
                if tab_id == self.tab_id && request_id == current =>
            {
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
        let url = self.normalize_url(&url);
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
            self.send_cmd(CoreCommand::CancelRequest { tab_id: self.tab_id, request_id: self.nav_gen });
        }
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
        self.url = url.clone();
        self.start_fetch(url);
    }

    fn normalize_url(&mut self, url: &String) -> String {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            // TODO: return error?
            return "http://example.com".into();
        }
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return format!("https://{trimmed}");
        }
        trimmed.into()
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
            if let Node::Element { attributes: _, style, .. } = node {
                if let Some(v) = style.iter().find(|(k, _)| k.eq_ignore_ascii_case("color")).map(|(_, v)| v) {
                    return parse_color(v);
                }
            }
            None
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
            if let Node::Element { name, style, .. } = node {
                if name.eq_ignore_ascii_case(want) {
                    if let Some(v) = style.iter().find(|(k, _)| k.eq_ignore_ascii_case("background-color")).map(|(_, v)| v) {
                        return parse_color(v);
                    }
                }
            }
            None
        }
        if let Node::Document { children, .. } = dom {
            for c in children {
                if let Some(c1) = from_element(c, "html") {
                    return Some(c1);
                }
                if let Node::Element { children: html_kids, .. } = c {
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

    pub fn collect_visible_text<'a>(node: &'a Node, ancestors: &mut Vec<&'a Node>, out: &mut String) {
        match node {
            Node::Text { text } => {
                let t = text.trim();
                if !t.is_empty() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(t);
                }
            }
            Node::Element{ name, children, .. } => {
                if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
                    return; // skip
                }
                ancestors.push(node);
                for c in children {
                    Self::collect_visible_text(c, ancestors, out);
                }
                ancestors.pop();

                match &name.to_ascii_lowercase()[..] {
                    "p" | "div" | "section" | "article" | "header" | "footer"
                    | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" => {
                        out.push_str("\n\n");
                    }
                    _ => {}
                }
            }
            Node::Document { children, .. } => {
                for c in children {
                    Self::collect_visible_text(c, ancestors, out);
                }
            }
            _ => {}
        }
    }
}
