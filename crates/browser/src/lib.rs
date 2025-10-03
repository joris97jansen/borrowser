use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
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
    tokenize,
    is_html,
    build_dom,
};

pub struct BrowserApp {
    url: String,
    loading: bool,
    last_status: Option<String>,
    last_preview: String,
    net_callback: Option<NetCallback>,
    tokens_preview: Vec<Token>,
    dom_outline: Vec<String>,
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
                            Token::StartTag { name, attributes, self_closing } => {
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
        self.net_callback = Some(callback);    }

    fn on_net_result(&mut self, result: net::FetchResult) {
        self.loading = false;
        self.tokens_preview.clear();
        if is_html(&result.content_type) && !result.body.is_empty() {
            let tokens = tokenize(&result.body);
            let dom = build_dom(&tokens);

            self.tokens_preview = tokens.into_iter().take(40).collect();

            let mut lines = Vec::new();
            fn walk(node: &Node, depth: usize, out: &mut Vec<String>, limit: &mut usize) {
                if *limit == 0 {
                    return;
                }
                *limit -= 1;

                let indent = "  ".repeat(depth);
                match node {
                    Node::Document { doctype, children } => {
                        if let Some(dt) = doctype {
                            out.push(format!("{indent}<!DOCTYPE {dt}>"));
                        } else {
                            out.push(format!("{indent}#document"));
                        }
                        for c in children {
                            walk(c, depth + 1, out, limit);
                        }
                    }
                    Node::Element { name, attributes, children } => {
                        let attributes_str = if attributes.is_empty() {
                            String::new()
                        } else {
                            let a = attributes.iter().map(|(k, v)| match v {
                                Some(v) => format!(r#"{k}="{v}""#),
                                None => k.clone(),
                            }).collect::<Vec<_>>().join(" ");
                            format!(" {}", a)
                        };
                        out.push(format!("{indent}<{}{}>", name, attributes_str));
                        for c in children { walk(c, depth + 1, out, limit); }
                    }
                    Node::Text{ text } => {
                        let t = text.replace('\n', " ").trim().to_string();
                        if !t.is_empty() {
                            let show = if t.len() > 40 { &t[..40] } else { &t };
                            out.push(format!("{indent}\"{show}"));
                        }
                    }
                    Node::Comment{ text } => {
                        let t = text.replace('\n', " ");
                        let show = if t.len() > 40 { &t[..40] } else { &t };
                        out.push(format!("{indent}<!-- {show} -->"));
                    }
                }
            }
            let mut limit = 200; // cap out to keep UI snappy
            walk(&dom, 0, &mut lines, &mut limit);
            self.dom_outline = lines;
        }
        let content_type = result.content_type.clone().unwrap_or_else(|| "unknown".into());
        let meta = match(result.status, &result.error) {
            (Some(code), None) => format!(
                "OK {code} — {} bytes - {content_type} - {} ms - {}",
                result.bytes, result.duration_ms, result.url
            ),
            (Some(code), Some(err)) => format!(
                "OK {code} — {} bytes - {content_type} - {} ms - {}",
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
