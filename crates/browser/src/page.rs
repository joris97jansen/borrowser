use std::collections::HashSet;
use std::time::{
    Instant,
    Duration,
};
use url::Url;
use html::{
    Node,
    tokenize,
    build_dom,
};
use html::dom_utils::{
    collect_style_texts, collect_stylesheet_hrefs
};
use css::{
    parse_stylesheet,
    attach_styles,
};

const MAX_HTML_BUFFER_SIZE: usize = 10 * 1024 * 1024; // 10 MB

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Node>,
    // css_pending: HashSet<String>,
    // css_bundle: String,
    pub html_buffer: String,
    last_parse: Option<Instant>,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            // css_pending: HashSet::new(),
            // css_bundle: String::new(),
            html_buffer: String::new(),
            last_parse: None,
        }
    }

    pub fn reset_for(&mut self, url: &str) {
        self.base_url = Some(url.to_string());
        self.dom = None;
        self.html_buffer.clear();
        self.last_parse = None;
    }

    pub fn ingest_html_start(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.html_buffer.clear();
        self.dom = None;
        self.last_parse = None;
    }

    pub fn ingest_html_chunk(&mut self, chunk: &[u8]) {
        if self.html_buffer.len() >= MAX_HTML_BUFFER_SIZE {
            return;
        }

        self.html_buffer.push_str(&String::from_utf8_lossy(chunk));

        if self.html_buffer.len() > MAX_HTML_BUFFER_SIZE {
            self.html_buffer.truncate(MAX_HTML_BUFFER_SIZE);
        }
    }

    pub fn should_parse_now(&mut self) -> bool {
        const MIN_INTERVAL: Duration = Duration::from_millis(180);
        let now = Instant::now();
        match self.last_parse {
            None => {
                self.last_parse = Some(now);
                true
            }
            Some(t) if now.duration_since(t) >= MIN_INTERVAL => {
                self.last_parse = Some(now);
                true
            }
            _ => false,
        }
    }

    pub fn parse_now_and_attach(&mut self) {
        let tokens = tokenize(&self.html_buffer);
        let mut dom = build_dom(&tokens);

        let mut inline_css = String::new();
        collect_style_texts(&dom, &mut inline_css);
        attach_styles(&mut dom, &parse_stylesheet(&inline_css));

        self.dom = Some(dom);
    }

    pub fn ingest_html_done(&mut self) {
        self.parse_now_and_attach();
    }

    pub fn ingest_html(&mut self, final_url: &str, body: &str, net_callback: impl Fn(String) + 'static + Clone) {
        self.base_url = Some(final_url.to_string());

        let tokens = tokenize(body);
        self.dom = Some(build_dom(&tokens));

        let mut inline_css = String::new();
        if let Some(dom_ref) = self.dom.as_ref() {
            collect_style_texts(dom_ref, &mut inline_css);
        }
        if let Some(dom_mut) = self.dom.as_mut() {
            attach_styles(dom_mut, &parse_stylesheet(&inline_css));
        }

        if let (Some(dom_ref), Some(base)) = (self.dom.as_ref(), self.base_url.as_ref()) {
            let mut hrefs = Vec::new();
            collect_stylesheet_hrefs(dom_ref, &mut hrefs);
            if let Ok(base_url) = Url::parse(base) {
                for h in hrefs {
                    if let Ok(abs) = base_url.join(&h) {
                        let href = abs.to_string();
                        // if self.css_pending.insert(href.clone()) {
                        //     net_callback(href);
                        // }
                    }
                }
            }
        }
    }

    pub fn try_ingest_css(
        &mut self,
        requested_url: &str,
        content_type: &Option<String>,
        body: &str,
    ) -> bool {
        let ct_is_css = content_type
            .as_deref()
            .map(|s| s.to_ascii_lowercase().contains("text/css"))
            .unwrap_or(false);

        // if self.css_pending.contains(requested_url) || ct_is_css {
        //     self.ingest_css(requested_url, body);
        //     true
        // } else {
        //     false
        // }
        false
    }

    fn ingest_css(&mut self, requested_url: &str, body: &str) {
        // self.css_pending.remove(requested_url);
        // if !body.is_empty() {
        //     self.css_bundle.push_str(body);
        //     self.css_bundle.push('\n');
        // }

        // if let Some(dom_mut) = self.dom.as_mut() {
        //     let mut inline_css = String::new();
        //     collect_style_texts(dom_mut, &mut inline_css);
        //     let sheet_inline = parse_stylesheet(&inline_css);
        //     let sheet_ext    = parse_stylesheet(&self.css_bundle);
        //     attach_styles(dom_mut, &sheet_inline);
        //     attach_styles(dom_mut, &sheet_ext);
        // }
    }

    pub fn pending_count(&self) -> usize {
        // self.css_pending.len()
        0
    }

    pub fn outline(&self, cap: usize) -> Vec<String> {
        if let Some(dom_ref) = self.dom.as_ref() {
            outline_from_dom(dom_ref, cap)
        } else {
            Vec::new()
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
                let styl = first_styles(style);
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
