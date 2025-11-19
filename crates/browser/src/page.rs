use std::collections::{
    HashSet,
};
use html::{
    Node,
    dom_utils::outline_from_dom,
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
            let mut ancestors = Vec::new();
            html::dom_utils::collect_visible_text(dom, &mut ancestors, &mut self.visible_text_cache);
        }
    }
}
