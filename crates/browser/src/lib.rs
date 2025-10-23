mod page;
mod view;

use page::PageState;
use view::NavigationAction;
use egui::{
    Context,
};
use app_api::{
    UiApp,
    NetCallback,
    RepaintHandle,
};
use net::{
    fetch_text,
};
use html::{
    Token,
    Node,
    is_html,
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
    net_callback: Option<NetCallback>,
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
            net_callback: None,
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
        self.loading = true;
        self.last_status = Some(format!("Fetching {} …", url));
        self.dom_outline.clear();

        self.poke_redraw();

        if let Some(callback) = self.net_callback.as_ref().cloned() {
            fetch_text(url, callback);
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

    fn style_get<'a>(attributes: &[(String, Option<String>)], style: &'a [(String, String)], name: &str) -> Option<&'a str> {
        // inline already merged into style earlier via attach_styles
        style.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn page_background(dom: &Node) -> Option<(u8, u8, u8, u8)> {
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

    fn collect_visible_text<'a>(node: &'a Node, ancestors: &mut Vec<&'a Node>, out: &mut String) {
        match node {
            Node::Text { text } => {
                if !text.trim().is_empty() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text.trim());
                }
            }
            Node::Element{ children, .. } | Node::Document { children, .. } => {
                if let Node::Element { name, .. } = node {
                    if name.eq_ignore_ascii_case("style") || name.eq_ignore_ascii_case("script") {
                        return; //skip
                    }
                }
                ancestors.push(node);
                for c in children {
                    Self::collect_visible_text(c, ancestors, out);
                }
                ancestors.pop();
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

    fn set_net_callback(&mut self, callback: NetCallback) {
        self.net_callback = Some(callback);
    }

    fn on_net_result(&mut self, result: net::FetchResult) {
        self.loading = false;

        if is_html(&result.content_type) && !result.body.is_empty() {
            let callback = self.net_callback.as_ref().cloned().expect("net cb");
            self.page.ingest_html(&result.url, &result.body, move |href| {
                fetch_text(href, callback.clone());
            });

            let queued = self.page.pending_count();

            self.dom_outline = self.page.outline(200);
            self.loading = queued > 0; // keep spinner if CSS pending
            self.last_status = Some(if queued > 0 {
                format!("Loaded HTML • fetching {queued} stylesheet(s)…")
            } else {
                "Loaded HTML".to_string()
            });

            self.poke_redraw();

            return;
        }

        if self.page.try_ingest_css(&result.requested_url, &result.content_type, &result.body) {
            self.dom_outline = self.page.outline(200);
            let remaining = self.page.pending_count();
            self.loading = remaining > 0;
            self.last_status = Some(if remaining > 0 {
                format!("Loaded stylesheet: {} ({} remaining)", result.url, remaining)
            } else {
                "All stylesheets loaded".to_string()
            });

            self.poke_redraw();

            return;
        }

        let content_type = result.content_type.clone().unwrap_or_else(|| "unknown".into());
        let meta = match(result.status, &result.error) {
            (Some(code), None) => format!(
                "OK {code} — {} bytes - {content_type} - {} ms - {}",
                result.bytes, result.duration_ms, result.url
            ),
            (Some(code), Some(err)) => format!(
                "OK {code} — {} bytes - {content_type} - {} ms - {} - Network error: {err}",
                result.bytes, result.duration_ms, result.url
                ),
            (None, Some(error)) => format!(
                "Network error: {} ms - {error}", result.duration_ms
            ),
            _ => "Unknown".to_string(),
        };

        self.last_status = Some(meta);
    }

    fn set_repaint_handle(&mut self, repaint: RepaintHandle) {
        self.repaint = Some(repaint);
    }
}
