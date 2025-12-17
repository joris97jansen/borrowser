use crate::input_store::InputValueStore;
use css::{Stylesheet, attach_styles, parse_stylesheet};
use html::{
    Node,
    dom_utils::{collect_style_texts, element_path_key, outline_from_dom},
    head::{HeadMetadata, extract_head_metadata},
};
use std::collections::HashSet;

pub struct PageState {
    pub base_url: Option<String>,
    pub dom: Option<Node>,
    pub head: HeadMetadata,

    pub visible_text_cache: String,

    pub input_values: InputValueStore,

    css_pending: HashSet<String>,
    css_sheet: Stylesheet,
}

impl PageState {
    pub fn new() -> Self {
        Self {
            base_url: None,
            dom: None,
            head: HeadMetadata::default(),
            visible_text_cache: String::new(),
            input_values: InputValueStore::new(),
            css_pending: HashSet::new(),
            css_sheet: Stylesheet { rules: Vec::new() },
        }
    }

    // Clear all state for new navigation
    pub fn start_nav(&mut self, final_url: &str) {
        self.base_url = Some(final_url.to_string());
        self.dom = None;
        self.head = HeadMetadata::default();
        self.visible_text_cache.clear();
        self.input_values.clear();
        self.css_pending.clear();
        self.css_sheet.rules.clear();
    }

    pub fn update_head_metadata(&mut self) {
        if let Some(dom) = self.dom.as_ref() {
            self.head = extract_head_metadata(dom);
        } else {
            self.head = HeadMetadata::default();
        }
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
            let mut ancestors = Vec::new();
            html::dom_utils::collect_visible_text(
                dom,
                &mut ancestors,
                &mut self.visible_text_cache,
            );
        }
    }

    pub fn apply_inline_style_blocks(&mut self) {
        if let Some(dom_mut) = self.dom.as_mut() {
            let mut css_text = String::new();
            collect_style_texts(dom_mut, &mut css_text);

            if !css_text.trim().is_empty() {
                let parsed = parse_stylesheet(&css_text);
                self.css_sheet.rules.extend(parsed.rules.into_iter());
            }

            // Apply all known stylesheets + inline style="" attrs
            attach_styles(dom_mut, &self.css_sheet);
        }
    }

    pub fn seed_input_values_from_dom(&mut self) {
        // Take an immutable reference to the DOM first
        let dom = match self.dom.as_ref() {
            Some(d) => d,
            None => return,
        };

        fn get_attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
            match node {
                Node::Element { attributes, .. } => {
                    for (k, v) in attributes {
                        if k.eq_ignore_ascii_case(name) {
                            return v.as_deref();
                        }
                    }
                    None
                }
                _ => None,
            }
        }

        fn is_text_input(node: &Node) -> bool {
            match node {
                Node::Element {
                    name, attributes, ..
                } if name.eq_ignore_ascii_case("input") => {
                    let mut ty: Option<&str> = None;
                    for (k, v) in attributes {
                        if k.eq_ignore_ascii_case("type") {
                            ty = v.as_deref();
                        }
                    }
                    ty.map(|t| t.eq_ignore_ascii_case("text")).unwrap_or(true)
                }
                _ => false,
            }
        }

        fn input_key(root: &Node, node: &Node) -> Option<String> {
            // Prefer id-based key
            if let Some(id) = get_attr(node, "id") {
                let id = id.trim();
                if !id.is_empty() {
                    return Some(format!("id:{id}"));
                }
            }

            // Fallback: path-based key
            html::dom_utils::element_path_key(root, node as *const Node)
                .map(|p| format!("path:{p}"))
        }

        fn walk(store: &mut InputValueStore, root: &Node, node: &Node) {
            if is_text_input(node) {
                if let Some(key) = input_key(root, node) {
                    let initial = get_attr(node, "value").unwrap_or("").to_string();
                    store.ensure_initial(key, initial);
                }
            }

            match node {
                Node::Document { children, .. } | Node::Element { children, .. } => {
                    for c in children {
                        walk(store, root, c);
                    }
                }
                _ => {}
            }
        }

        // Only mutably borrow the store, not all of self
        walk(&mut self.input_values, dom, dom);
    }
}
