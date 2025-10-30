mod page;
mod view;

use std::sync::atomic::{
    AtomicU64, 
    Ordering
};
use url::Url;
use page::PageState;
use bus::CoreCommand;
use std::sync::mpsc;
use view::NavigationAction;
use egui::{
    Context,
};
use app_api::{
    UiApp,
    RepaintHandle,
};

use html::{
    Node,
    dom_utils::collect_stylesheet_hrefs,
};

use css::{
    parse_color,
};
use bus::CoreEvent;
use core_types::{
    ResourceKind,
    SessionId,
};


pub struct BrowserApp {
    session_id: SessionId,

    url: String,
    history: Vec<String>,
    history_index: usize,

    loading: bool,
    last_status: Option<String>,

    dom_outline: Vec<String>,
    page: PageState,

    repaint: Option<RepaintHandle>,

    cmd_tx: Option<mpsc::Sender<CoreCommand>>,

    nav_gen: u64,
}

impl BrowserApp {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let sid = NEXT.fetch_add(1, Ordering::Relaxed);

        Self {
            session_id: sid,
            url: String::new(),
            history: Vec::new(),
            history_index: 0,

            loading: false,
            last_status: None,

            dom_outline: Vec::new(),
            page: PageState::new(),

            repaint: None,

            cmd_tx: None,
            nav_gen: 0,
        }
    }

    pub fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
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

    fn navigate_to_new(&mut self, url: String) {
        let url = self.normalize_url(&url);
        self.url = url.clone();

        // record to history (truncate forward branch)
        self.history.truncate(self.history_index + 1);
        self.history.push(url.clone());
        self.history_index = self.history.len() - 1;

        self.start_fetch(url);
    }

    fn load_current(&mut self, url: String) {
        // do NOT touch history; just fetch the given URL
        self.url = url.clone();
        self.start_fetch(url);
    }

    fn start_fetch(&mut self, url: String) {
        // Cancel previous nav (if any)
        if self.nav_gen > 0 {
            self.send_cmd(CoreCommand::CancelRequest { session_id: self.session_id, request_id: self.nav_gen });
        }

        // Bump generation and kick HTML stream
        self.nav_gen = self.nav_gen.wrapping_add(1);
        let request_id = self.nav_gen;

        self.loading = true;
        self.last_status = Some(format!("Fetching {url} …"));
        self.dom_outline.clear();

        self.send_cmd(CoreCommand::FetchStream {
            session_id: self.session_id,
            request_id,
            url,
            kind: ResourceKind::Html,
        });

        if let Some(repaint) = &self.repaint { repaint.request_now(); }
    }

    fn go_back(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    fn go_forward(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let url = self.history[self.history_index].clone();
            self.load_current(url);
        }
    }

    fn refresh(&mut self) {
        if let Some(url) = self.history.get(self.history_index).cloned() {
            self.load_current(url);
        }
    }

    fn inherited_color(node: &Node, ancestors: &[Node]) -> (u8, u8, u8, u8) {
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

impl UiApp for BrowserApp {
    fn ui(&mut self, ctx: &Context) {
        let nav_action = view::top_bar(ctx, self);

        match nav_action {
            NavigationAction::Navigate(url) => self.navigate_to_new(url),
            NavigationAction::Back => self.go_back(),
            NavigationAction::Forward => self.go_forward(),
            NavigationAction::Refresh => self.refresh(),
            NavigationAction::None => {}
        }

        view::content(
            ctx,
            &self.page,
            self.last_status.as_ref(),
            self.loading,
        )
    }

    fn on_core_event(&mut self, evt: CoreEvent) {
        let sid = self.session_id;
        let current = self.nav_gen;

        match evt {
            // Networking (HTML): drive parser runtime
            CoreEvent::NetworkStart { session_id, request_id, kind: ResourceKind::Html, url, .. } if session_id == sid &&request_id == current => {
                self.page.start_nav(&url);
                self.loading = true;
                self.last_status = Some(format!("Started HTML stream: {}", url));
                self.send_cmd(CoreCommand::ParseHtmlStart { session_id: sid, request_id });
                self.poke_redraw();
            }
            CoreEvent::NetworkChunk { session_id, request_id, kind: ResourceKind::Html, bytes, .. } if session_id == sid && request_id == current => {
                self.send_cmd(CoreCommand::ParseHtmlChunk { session_id: sid, request_id, bytes });
            }
            CoreEvent::NetworkDone { session_id, request_id, kind: ResourceKind::Html, url } if session_id == sid && request_id == current => {
                self.send_cmd(CoreCommand::ParseHtmlDone { session_id, request_id });
                self.last_status = Some(format!("Loaded HTML: {}", url));
                self.poke_redraw();
            }
            CoreEvent::NetworkError { session_id, request_id, kind: ResourceKind::Html, url, error } if session_id == sid && request_id == current => {
                self.loading = false;
                self.last_status = Some(format!("Network error on {url}: {error}"));
                self.poke_redraw();
            }

            // Parser → apply DOM snapshot; then kick CSS fetches
            CoreEvent::DomUpdate { session_id, request_id, dom } if session_id == sid && request_id == current => {
                self.page.dom = Some(dom);
                self.page.update_visible_text_cache();

                // discover and fetch stylesheets (once per nav)
                if let (Some(dom_ref), Some(base)) = (self.page.dom.as_ref(), self.page.base_url.as_ref()) {
                    let mut hrefs = Vec::new();
                    collect_stylesheet_hrefs(dom_ref, &mut hrefs);
                    if let Ok(base_url) = Url::parse(base) {
                        for h in hrefs {
                            if let Ok(abs) = base_url.join(&h) {
                                let href = abs.to_string();
                                if self.page.register_css(&href) {
                                    self.send_cmd(CoreCommand::FetchStream {
                                        session_id: sid,
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

            // Networking (CSS) → forward bytes to CSS runtime
            CoreEvent::NetworkChunk { session_id, request_id, kind: ResourceKind::Css, url, bytes } if session_id == sid && request_id == current => {
                self.send_cmd(CoreCommand::CssChunk { session_id: sid, request_id, url, bytes });
            }
            CoreEvent::NetworkDone {session_id, request_id, kind: ResourceKind::Css, url } if session_id == sid && request_id == current => {
                self.send_cmd(CoreCommand::CssDone { session_id: sid, request_id, url });
            }
            CoreEvent::NetworkError { session_id, request_id, kind: ResourceKind::Css, url, error } if session_id == sid && request_id == current => {
                // treat as done to unblock
                self.send_cmd(CoreCommand::CssDone { session_id: sid, request_id, url: url.clone() });
                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(format!("Stylesheet error on {url}: {error} ({} remaining)", remaining));
                self.poke_redraw();
            }

            // CSS runtime → apply incremental blocks
            CoreEvent::CssParsedBlock { session_id, request_id, url: _, css_block } if session_id == sid &&request_id == current => {
                self.page.apply_css_block(&css_block);
                self.dom_outline = self.page.outline(200); // optional / debug
                self.poke_redraw();
            }
            CoreEvent::CssSheetDone { session_id, request_id, url } if session_id == sid && request_id == current => {
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

            _ => {} // stale or irrelevant
        }
    }

    fn set_repaint_handle(&mut self, repaint: RepaintHandle) {
        self.repaint = Some(repaint);
    }

    fn set_bus_sender(&mut self, tx: mpsc::Sender<CoreCommand>) {
        self.cmd_tx = Some(tx);
    }
}
