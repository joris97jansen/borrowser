mod page;
use page::PageState;
use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
    Color32,
    Stroke,
    CornerRadius,
    Frame,
};
use app_api::{
    UiApp,
    NetCallback,
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
    loading: bool,
    last_status: Option<String>,
    last_preview: String,
    net_callback: Option<NetCallback>,
    tokens_preview: Vec<Token>,
    dom_outline: Vec<String>,
    page: PageState
}

impl BrowserApp {
    pub fn new() -> Self {
        Self{
            url: "https://example.com".into(),
            loading: false,
            last_status: None,
            last_preview: String::new(),
            net_callback: None,
            tokens_preview: Vec::new(),
            dom_outline: Vec::new(),
            page: PageState::new(),
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
    fn ui(&mut self, context: &Context) {
        TopBottomPanel::top("topbar").show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL:");
                let response = ui.text_edit_singleline(&mut self.url);
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) || ui.button("Go").clicked() {
                    self.loading = true;
                    self.last_status = Some(format!("Fetching {}…", self.url));
                    self.last_preview.clear();
                    self.dom_outline.clear();
                    self.tokens_preview.clear();

                    if let Some(cb) = self.net_callback.as_ref().cloned() {
                        let url_str = self.url.clone();
                        let url = self.normalize_url(&url_str);
                        self.url = url.clone();
                        fetch_text(self.url.clone(), cb);
                    } else {
                        self.loading = false;
                        self.last_status = Some("No network callback set".into());
                    }
                }
            });
        });
        CentralPanel::default().show(context, |ui| {
            if !self.tokens_preview.is_empty() {
                ui.separator();
                ui.heading("HTML tokens (first 40:)");
                ScrollArea::vertical().max_height(200.0).id_salt("second").show(ui, |ui| {
                    for (i, t) in self.tokens_preview.iter().enumerate() {
                        match t {
                            Token::StartTag { name, attributes, self_closing, .. } => {
                                let mut parts = vec![name.clone()];
                                if !attributes.is_empty() {
                                    let attributes_str = attributes.iter()
                                        .map(|(k, v)| match v {
                                            Some(v) => format!(r#"{k}="{v}""#),
                                            None => k.clone(),
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    parts.push(attributes_str);
                                }
                                let slash = if *self_closing { " /" } else { "" };
                                ui.monospace(format!("{i:02}: <{}{}>", parts.join(" "), slash))
                            }
                            Token::EndTag(name) => ui.monospace(format!("{i:02}: </{}>", name)),
                            Token::Doctype(doctype) => ui.monospace(format!("{i:02}: <!DOCTYPE {}>", doctype)),
                            Token::Comment(comment) => ui.monospace(format!("{i:02}: <!-- {} -->", comment)),
                            Token::Text(text) => ui.monospace(format!("{i:02}: \"{}\"", text)),
                        };
                    }
                });
            }
            if !self.dom_outline.is_empty() {
                ui.separator();
                ui.heading("DOM Outline (first 200 lines):");
                ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for line in &self.dom_outline {
                        ui.monospace(line);
                    }
                });
            }
            if let Some(dom_ref) = self.page.dom.as_ref() {
                ui.separator();
                ui.heading("Page preview (early):");

                let bg = Self::page_background(dom_ref).unwrap_or((255, 255, 255, 255));
                let bg_ui = Color32::from_rgba_unmultiplied(bg.0, bg.1, bg.2, bg.3);

                let mut text = String::new();
                let mut ancestors = Vec::new();
                Self::collect_visible_text(dom_ref, &mut ancestors, &mut text);

                let color = Self::inherited_color(dom_ref, &[]);
                let fg_egui = Color32::from_rgba_unmultiplied(color.0, color.1, color.2, color.3);

                Frame::new()
                    .fill(bg_ui)
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::same(4))
                    .show(ui, |ui| {
                        ui.set_min_height(200.0);
                        ui.add_space(6.0);
                        ui.style_mut().visuals.override_text_color = Some(fg_egui);
                        ui.label(text);
                        ui.style_mut().visuals.override_text_color = None;
                        ui.add_space(6.0);
                    });
            }
            if self.loading { ui.label("⏳ Loading…"); }
            if let Some(s) = &self.last_status { ui.label(s); }
            if !self.last_preview.is_empty() {
                ui.separator();
                ui.label("Preview (first 500 chars):");
                ui.code(self.last_preview.clone());
            }
        });
    }

    fn set_net_callback(&mut self, callback: NetCallback) {
        self.net_callback = Some(callback);
    }

    fn on_net_result(&mut self, result: net::FetchResult) {
        self.loading = false;
        self.tokens_preview.clear();

        if is_html(&result.content_type) && !result.body.is_empty() {
            let callback = self.net_callback.as_ref().cloned().expect("net cb");
            self.page.ingest_html(&result.url, &result.body, move |href| {
                fetch_text(href, callback.clone());
            });

            let queued = self.page.pending_count();
            self.last_status = Some(if queued > 0 {
                format!("Loaded HTML • fetching {queued} stylesheet(s)…")
            } else {
                "Loaded HTML".to_string()
            });

            self.dom_outline = self.page.outline(200);
            self.loading = queued > 0; // keep spinner if CSS pending
            return;
        }

        if self.page.try_ingest_css(&result.requested_url, &result.content_type, &result.body) {
            self.dom_outline = self.page.outline(200);
            let remaining = self.page.pending_count();
            self.last_status = Some(if remaining > 0 {
                format!("Loaded stylesheet: {} ({} remaining)", result.url, remaining)
            } else {
                "All stylesheets loaded".to_string()
            });
            self.loading = remaining > 0;
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
        self.last_preview = result.snippet;
    }
}
