use std::collections::{
    HashSet,
};
use html::{
    Node,
};
use css::{
    parse_stylesheet,
    attach_styles,
    Stylesheet,
};

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Node>,


    pub visible_text_cache: String,

    css_pending: HashSet<String>,
    css_sheet: Stylesheet,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            visible_text_cache: String::new(),
            css_pending: HashSet::new(),
            css_sheet: Stylesheet { rules: Vec::new() },
        }
    }

    // Clear all state for new navigation
    pub fn start_nav(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.dom = None;
        self.visible_text_cache.clear();
        self.css_pending.clear();
        self.css_sheet.rules.clear();
    }


    // --- CSS ---
    pub fn register_css(&mut self, absolute_url: &str) -> bool {
        self.css_pending.insert(absolute_url.to_string())
    }

    pub fn apply_css_block(&mut self, block: &str) {
        let parsed = parse_stylesheet(block);
        self.css_sheet.rules.extend(parsed.rules.into_iter());
        if let Some(dom_mut) = self.dom.as_mut() {
            attach_styles(dom_mut, &self.css_sheet);
        }
    }

    pub fn mark_css_done(&mut self, url: &str) {
        self.css_pending.remove(url);
    }

    pub fn pending_count(&self) -> usize {
        self.css_pending.len()
    }

    pub fn outline(&self, cap: usize) -> Vec<String> {
        if let Some(dom_ref) = self.dom.as_ref() {
            outline_from_dom(dom_ref, cap)
        } else {
            Vec::new()
        }
    }

    pub fn update_visible_text_cache(&mut self) {
        self.visible_text_cache.clear();
        if let Some(dom) = self.dom.as_ref() {
            fn collect(node: &Node, out: &mut String) {
                match node {
                    Node::Text { text } => {
                        let t = text.trim();
                        if !t.is_empty() {
                            if !out.is_empty() { out.push(' '); }
                            out.push_str(t);
                        }
                    }
                    Node::Element { name, children, .. } => {
                        if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
                            return;
                        }
                        for c in children { collect(c, out); }
                        match &name.to_ascii_lowercase()[..] {
                            "p" | "div" | "section" | "article" | "header" | "footer"
                            | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" => {
                                out.push_str("\n\n");
                            }
                            _ => {}
                        }
                    }
                    Node::Document { children, .. } => {
                        for c in children { collect(c, out); }
                    }
                    _ => {}
                }
            }
            collect(dom, &mut self.visible_text_cache);
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
