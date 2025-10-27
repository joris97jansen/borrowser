mod page;
mod view;

use page::PageState;
use view::NavigationAction;
use egui::{
    Context,
};
use app_api::{
    UiApp,
    RepaintHandle,
    NetStreamCallback,
};
use url::Url;
use net::{
    NetEvent,
    ResourceKind,
    fetch_stream,
};
use html::{
    Node,
};
use html::dom_utils::{
    collect_stylesheet_hrefs,
};
use css::{
    parse_color,
};

pub struct BrowserApp {
    url: String,
    history: Vec<String>,
    history_index: usize,
    loading: bool,
    last_status: Option<String>,
    net_stream_callback: Option<NetStreamCallback>,
    dom_outline: Vec<String>,
    page: PageState,
    repaint: Option<RepaintHandle>,
}

impl BrowserApp {
    pub fn new() -> Self {
        Self{
            url: String::new(),
            history: Vec::new(),
            history_index: 0,
            loading: false,
            last_status: None,
            net_stream_callback: None,
            dom_outline: Vec::new(),
            page: PageState::new(),
            repaint: None,
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
        self.page.reset_for(&url);

        self.loading = true;
        self.last_status = Some(format!("Fetching {} …", url));
        self.dom_outline.clear();

        self.poke_redraw();

        if let Some(callback) = self.net_stream_callback.as_ref().cloned() {
            println!("Starting streaming fetch for {}", url);
            fetch_stream(url, ResourceKind::Html, callback);
        } else {
            self.loading = false;
            self.last_status = Some("No network callback set".into());
        }
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

    fn set_net_stream_callback(&mut self, callback: NetStreamCallback) {
        self.net_stream_callback = Some(callback);
    }

    fn on_net_stream(&mut self, event: NetEvent) {
        print!("Received network stream event");
        match event {
            NetEvent::Start { kind: ResourceKind::Html, url, .. } => {
                self.page.ingest_html_start(&url);
                self.loading = true;
                self.last_status = Some(format!("Started HTML stream: {}", url));
                self.poke_redraw();
            }
            NetEvent::Chunk { kind: ResourceKind::Html, url: _, chunk } => {
                self.page.ingest_html_chunk(&chunk);
                if self.page.should_parse_now() {
                    self.page.parse_now_and_attach();
                    self.dom_outline = self.page.outline(200);
                    self.last_status = Some("Parsing HTML stream…".into());
                    self.poke_redraw();
                }
            }
            NetEvent::Done { kind: ResourceKind::Html, url } => {
                // finalize HTML
                self.page.ingest_html_done();
                self.dom_outline = self.page.outline(200);
                self.last_status = Some(format!("Loaded HTML: {}", url));

                if let (Some(dom_ref), Some(base)) = (self.page.dom.as_ref(), self.page.base_url.as_ref()) {
                    let mut hrefs = Vec::new();
                    collect_stylesheet_hrefs(dom_ref, &mut hrefs);
                    if let Ok(base_url) = Url::parse(base) {
                        if let Some(callback) = self.net_stream_callback.as_ref().clone() {
                            for h in hrefs {
                                if let Ok(abs) = base_url.join(&h) {
                                    let href = abs.to_string();
                                    if self.page.register_css(&href) {
                                        fetch_stream(href, ResourceKind::Css, callback.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                let pending = self.page.pending_count();
                self.loading = pending > 0;
                if pending > 0 {
                    self.last_status = Some(format!("Loaded HTML, fetching {pending} stylesheet(s)..."));
                }
                self.poke_redraw();
            }
            NetEvent::Error { kind: ResourceKind::Html, url, error } => {
                self.loading = false;
                self.last_status = Some(format!("Network error on {}: {}", url, error));
                self.poke_redraw();
            }
            NetEvent::Start { kind: ResourceKind::Css, url, .. } => {
                // already registered; status only
                self.last_status = Some(format!("Fetching stylesheets: {url}"));
                self.poke_redraw();
           }
            NetEvent::Chunk { kind: ResourceKind::Css, url, chunk } => {
                self.page.ingest_css_chunk(&url, &chunk);
                // phase 2: incremental parse
            }
            NetEvent::Done { kind: ResourceKind::Css, url } => {
                self.page.ingest_css_done(&url);
                self.dom_outline = self.page.outline(200);

                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(if remaining > 0 {
                    format!("Loaded stylesheet {}, {} remaining...", url, remaining)
                } else {
                    format!("All stylesheets loaded")
                });
                self.poke_redraw();
            }
            NetEvent::Error { kind: ResourceKind::Css, url, error } => {
                self.page.ingest_css_done(&url);
                let remaining = self.page.pending_count();
                self.loading = remaining > 0;
                self.last_status = Some(format!("Stylesheet error on {}: {} ({} remaining)", url, error, remaining));
                self.poke_redraw();
            }
        }
    }

    fn set_repaint_handle(&mut self, repaint: RepaintHandle) {
        self.repaint = Some(repaint);
    }
}
