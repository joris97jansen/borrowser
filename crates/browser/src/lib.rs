use egui::{
    Context,
    TopBottomPanel,
    Key,
    CentralPanel,
    ScrollArea,
    Color32,
    Stroke,
    Rounding,
    Frame,
};
use std::collections::HashSet;
use url::Url;
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
use css::{
    parse_stylesheet,
    attach_styles,
    is_css,
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
    base_url: Option<String>,
    css_pending: HashSet<String>,
    css_bundle: String,
    dom: Option<Node>,
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
            base_url: None,
            css_pending: HashSet::new(),
            css_bundle: String::new(),
            dom: None,
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

    fn collect_style_texts(node: &Node, out: &mut String) {
        match node {
            Node::Element { name, children, .. } if name.eq_ignore_ascii_case("style") => {
                for c in children {
                    if let Node::Text { text } = c {
                        out.push_str(text);
                        out.push('\n');
                    }
                }
            }
            Node::Element { children, .. } | Node::Document { children, .. } => {
                for c in children {
                    Self::collect_style_texts(c, out);
                }
            }
            _ => {}
        }
    }

    fn collect_stylesheet_hrefs(node: &Node, out: &mut Vec<String>) {
        if let Node::Element { name, attributes, .. } = node {
            if name.eq_ignore_ascii_case("link") {
                let mut is_stylesheet = false;
                let mut href: Option<&str> = None;
                for (k, v) in attributes {
                    let key = k.as_str();
                    if key.eq_ignore_ascii_case("rel") {
                        if let Some(val) = v.as_deref() {
                            if val.split_whitespace().any(|t| t.eq_ignore_ascii_case("stylesheet")) {
                                is_stylesheet = true;
                            }
                        }
                    } else if key.eq_ignore_ascii_case("href") {
                        href = v.as_deref();
                    }
                }
                if is_stylesheet {
                    if let Some(h) = href {
                        out.push(h.to_string());
                    }
                }
            }
            if let Node::Element { children, .. } | Node::Document { children, .. } = node {
                for c in children {
                    Self::collect_stylesheet_hrefs(c, out);
                }
            }
        }
    }

    fn first_styles(style: &[(String, String)]) -> String {
        style.iter()
            .take(3)
            .map(|(k, v)| format!(r#"{k}: {v};"#))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn outline_from_dom(root: &Node, cap: usize) -> Vec<String> {
        fn walk(node: &Node, depth: usize, out: &mut Vec<String>, left: &mut usize) {
            if *left == 0 { return; }
            *left -= 1;
            let indent = "  ".repeat(depth);
            match node {
                Node::Document { doctype, children } => {
                    if let Some(dt) = doctype {
                        out.push(format!("{indent}<!DOCTYPE {dt}>"));
                    } else {
                        out.push(format!("{indent}#document"));
                    }
                    for c in children { walk(c, depth+1, out, left); }
                }
                Node::Element { name, attributes, children, style } => {
                    let id = attributes.iter().find(|(k,_)| k=="id").and_then(|(_,v)| v.as_deref()).unwrap_or("");
                    let class = attributes.iter().find(|(k,_)| k=="class").and_then(|(_,v)| v.as_deref()).unwrap_or("");
                    // NOTE: call through the type, not `Self` (fixes E0401):
                    let styl = BrowserApp::first_styles(style);
                    let mut line = format!("{indent}<{name}");
                    if !id.is_empty()   { line.push_str(&format!(r#" id="{id}""#)); }
                    if !class.is_empty(){ line.push_str(&format!(r#" class="{class}""#)); }
                    line.push('>');
                    if !styl.is_empty() { line.push_str(&format!("  /* {styl} */")); }
                    out.push(line);
                    for c in children { walk(c, depth+1, out, left); }
                }
                Node::Text { text } => {
                    let t = text.replace('\n', " ").trim().to_string();
                    if !t.is_empty() {
                        let show = if t.len() > 40 { format!("{}…",&t[..40]) } else { t };
                        out.push(format!("{indent}\"{show}\""));
                    }
                }
                Node::Comment { text } => {
                    let t = text.replace('\n', " ");
                    let show = if t.len() > 40 { format!("{}…",&t[..40]) } else { t };
                    out.push(format!("{indent}<!-- {show} -->"));
                }
            }
        }
        let mut out = Vec::new();
        let mut left = cap;
        walk(root, 0, &mut out, &mut left);
        out
    }

    fn style_get<'a>(attributes: &[(String, Option<String>)], style: &'a [(String, String)], name: &str) -> Option<&'a str> {
        // (inline already merged into style earlier via attach_styles)
        style.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
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
                    self.css_pending.clear();
                    self.css_bundle.clear();
                    self.dom = None;
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
                            Token::StartTag { name, attributes, self_closing, style } => {
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
            if let Some(dom_ref) = self.dom.as_ref() {
                ui.separator();
                ui.heading("Page preview (early):");

                let bg = Self::page_background(dom_ref).unwrap_or((255, 255, 255, 255));
                let bg_ui = Color32::from_rgba_unmultiplied(bg.0, bg.1, bg.2, bg.3);

                let mut text = String::new();
                let mut ancestors = Vec::new();
                Self::collect_visible_text(dom_ref, &mut ancestors, &mut text);

                let color = Self::inherited_color(dom_ref, &[]);
                let fg_egui = Color32::from_rgba_unmultiplied(color.0, color.1, color.2, color.3);

                Frame::none()
                    .fill(bg_ui)
                    .stroke(Stroke::NONE)
                    .rounding(Rounding::same(4))
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
            self.base_url = Some(result.url.clone());
            self.css_pending.clear();
            self.css_bundle.clear();

            let tokens = tokenize(&result.body);
            let mut dom = build_dom(&tokens);
            self.dom = Some(dom);

            let mut css_text = String::new();
            if let Some(dom_ref) = self.dom.as_ref() {
                Self::collect_style_texts(dom_ref, &mut css_text);
            }

            if let Some(dom_mut) = self.dom.as_mut() {
                let sheet = parse_stylesheet(&css_text);
                attach_styles(dom_mut, &sheet);
            }

            let mut hrefs = Vec::new();
            Self::collect_stylesheet_hrefs(
                self.dom.as_ref().unwrap(),
                &mut hrefs
            );

            if let Some(base) = &self.base_url {
                if let Ok(base_url) = Url::parse(base) {
                    for href in hrefs {
                        if let Ok(abs) = base_url.join(&href) {
                            let abs = abs.to_string();
                            if self.css_pending.insert(abs.clone()) {
                                if let Some(callback) = self.net_callback.as_ref().cloned() {
                                    fetch_text(abs, callback);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(dom_ref) = self.dom.as_ref() {
                self.dom_outline.clear();
                let mut lines: Vec<String> = Vec::new();
                let mut limit = 200usize;
                self.dom_outline = BrowserApp::outline_from_dom(dom_ref, 200);
            }

            self.tokens_preview = tokens.into_iter().take(40).collect();

            if let Some(dom_ref) = self.dom.as_ref() {
                self.dom_outline.clear();
                let mut lines: Vec<String> = Vec::new();
                let mut limit = 200usize;
                self.dom_outline = BrowserApp::outline_from_dom(dom_ref, 200);
            }

            if let Some(dom_ref) = self.dom.as_ref() {
                self.dom_outline = Self::outline_from_dom(dom_ref, 200);
            }
        }

        if is_css(&result.content_type) || self.css_pending.contains(&result.requested_url) {
            self.css_pending.remove(&result.requested_url);

            if !result.body.is_empty() {
                self.css_bundle.push_str(&result.body);
                self.css_bundle.push('\n');
            }

            if let Some(dom) = self.dom.as_mut() {
                let mut inline = String::new();
                Self::collect_style_texts(dom, &mut inline);
                let sheet_inline = parse_stylesheet(&inline);
                let sheet_ext    = parse_stylesheet(&self.css_bundle);

                attach_styles(dom, &sheet_inline);
                attach_styles(dom, &sheet_ext);

                if let Some(dom_ref) = self.dom.as_ref() {
                    self.dom_outline.clear();
                    let mut lines: Vec<String> = Vec::new();
                    let mut limit = 200usize;
                    self.dom_outline = BrowserApp::outline_from_dom(dom_ref, 200);
                }
            }

            if let Some(dom_ref) = self.dom.as_ref() {
                self.dom_outline = Self::outline_from_dom(dom_ref, 200);
            }

            let remaining = self.css_pending.len();
            self.last_status = Some(format!("Loaded stylesheet: {} ({} remaining)", result.url, remaining));
            return;
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
