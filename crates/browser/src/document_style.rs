use core_types::StylesheetSlotId;
use css::{
    CascadeOrigin, ParseOptions, StylesheetCascadeInput, StylesheetParse,
    parse_stylesheet_with_options,
};
use html::Node;
use std::sync::OnceLock;
use url::Url;

const MINIMAL_UA_STYLESHEET: &str = r#"
html, body, div, p, section, article, header, footer, main, nav, aside,
h1, h2, h3, h4, h5, h6, ul, ol, menu, form, table, thead, tbody, tfoot,
tr, td, th, blockquote, pre, address, figure, figcaption, dl, dt, dd {
    display: block;
}

li {
    display: list-item;
}

input, button, textarea {
    display: inline-block;
}

head, title, meta, link, style, script {
    display: none;
}
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
enum StylesheetSlotKey {
    Inline(String),
    External(String),
}

#[derive(Clone, Debug)]
enum StylesheetSlotState {
    Pending,
    Loaded(StylesheetParse),
    Failed,
    Aborted,
}

#[derive(Clone, Debug)]
struct StylesheetSlot {
    id: StylesheetSlotId,
    key: StylesheetSlotKey,
    state: StylesheetSlotState,
}

#[derive(Clone, Debug)]
struct CascadedStylesheet {
    origin: CascadeOrigin,
    stylesheet: StylesheetParse,
}

#[derive(Clone, Debug)]
pub(crate) struct StylesheetFetch {
    pub(crate) slot_id: StylesheetSlotId,
    pub(crate) url: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct StylesheetReconcileResult {
    pub(crate) fetches: Vec<StylesheetFetch>,
    pub(crate) changed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentStyleSet {
    next_slot_id: u64,
    slots: Vec<StylesheetSlot>,
    loaded_stylesheets: Vec<StylesheetParse>,
    cascade_stylesheets: Vec<CascadedStylesheet>,
}

impl DocumentStyleSet {
    pub(crate) fn clear(&mut self) {
        self.next_slot_id = 0;
        self.slots.clear();
        self.loaded_stylesheets.clear();
        self.rebuild_cascade_stylesheets();
    }

    pub(crate) fn reconcile_from_dom(
        &mut self,
        dom: &Node,
        base_url: Option<&str>,
    ) -> StylesheetReconcileResult {
        let mut discovered = Vec::new();
        collect_stylesheet_inputs(dom, base_url, &mut discovered);

        let old_slots = std::mem::take(&mut self.slots);
        let changed = old_slots.len() != discovered.len()
            || old_slots
                .iter()
                .zip(&discovered)
                .any(|(slot, key)| &slot.key != key);
        let mut used = vec![false; old_slots.len()];
        let mut fetches = Vec::new();
        let mut new_slots = Vec::with_capacity(discovered.len());

        for key in discovered {
            if let Some((index, old)) = old_slots
                .iter()
                .enumerate()
                .find(|(index, slot)| !used[*index] && slot.key == key)
            {
                used[index] = true;
                new_slots.push(old.clone());
                continue;
            }

            let id = self.allocate_slot_id();
            let state = match &key {
                StylesheetSlotKey::Inline(text) => StylesheetSlotState::Loaded(
                    parse_stylesheet_with_options(text, &ParseOptions::stylesheet()),
                ),
                StylesheetSlotKey::External(url) => {
                    fetches.push(StylesheetFetch {
                        slot_id: id,
                        url: url.clone(),
                    });
                    StylesheetSlotState::Pending
                }
            };

            new_slots.push(StylesheetSlot { id, key, state });
        }

        self.slots = new_slots;
        self.rebuild_loaded_stylesheets();
        StylesheetReconcileResult { fetches, changed }
    }

    #[cfg(test)]
    pub(crate) fn register_external_for_tests(&mut self, url: &str) -> StylesheetSlotId {
        let id = self.allocate_slot_id();
        self.slots.push(StylesheetSlot {
            id,
            key: StylesheetSlotKey::External(url.to_string()),
            state: StylesheetSlotState::Pending,
        });
        id
    }

    pub(crate) fn install_external_stylesheet(
        &mut self,
        slot_id: StylesheetSlotId,
        css_text: &str,
    ) -> bool {
        let Some(slot) = self.slot_mut(slot_id) else {
            return false;
        };
        if !matches!(slot.key, StylesheetSlotKey::External(_)) {
            return false;
        }
        if !matches!(
            slot.state,
            StylesheetSlotState::Pending | StylesheetSlotState::Loaded(_)
        ) {
            return false;
        }
        slot.state = StylesheetSlotState::Loaded(parse_stylesheet_with_options(
            css_text,
            &ParseOptions::stylesheet(),
        ));
        self.rebuild_loaded_stylesheets();
        true
    }

    pub(crate) fn mark_external_done(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id)
            && matches!(slot.state, StylesheetSlotState::Pending)
        {
            slot.state = StylesheetSlotState::Failed;
            return false;
        }
        false
    }

    pub(crate) fn mark_external_failed(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id) {
            let had_loaded_style = matches!(slot.state, StylesheetSlotState::Loaded(_));
            slot.state = StylesheetSlotState::Failed;
            if had_loaded_style {
                self.rebuild_loaded_stylesheets();
                return true;
            }
        }
        false
    }

    pub(crate) fn mark_external_aborted(&mut self, slot_id: StylesheetSlotId) -> bool {
        if let Some(slot) = self.slot_mut(slot_id) {
            let had_loaded_style = matches!(slot.state, StylesheetSlotState::Loaded(_));
            slot.state = StylesheetSlotState::Aborted;
            if had_loaded_style {
                self.rebuild_loaded_stylesheets();
                return true;
            }
        }
        false
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| matches!(slot.state, StylesheetSlotState::Pending))
            .count()
    }

    pub(crate) fn stylesheets(&self) -> &[StylesheetParse] {
        &self.loaded_stylesheets
    }

    pub(crate) fn cascade_stylesheet_inputs(&self) -> Vec<StylesheetCascadeInput<'_>> {
        // Cascade source indexes are indexes into this runtime input list, not
        // indexes into `PageState::css_stylesheets()`. The runtime list includes
        // built-in UA stylesheets; authored stylesheet reporting intentionally
        // does not.
        self.cascade_stylesheets
            .iter()
            .map(|entry| match entry.origin {
                CascadeOrigin::UserAgent => StylesheetCascadeInput::user_agent_for_namespace(
                    &entry.stylesheet,
                    html::ElementNamespace::Html,
                ),
                CascadeOrigin::User | CascadeOrigin::Author => {
                    StylesheetCascadeInput::new(entry.origin, &entry.stylesheet)
                }
            })
            .collect()
    }

    fn allocate_slot_id(&mut self) -> StylesheetSlotId {
        self.next_slot_id = self
            .next_slot_id
            .checked_add(1)
            .expect("stylesheet slot id exhausted for document");
        StylesheetSlotId(self.next_slot_id)
    }

    fn slot_mut(&mut self, slot_id: StylesheetSlotId) -> Option<&mut StylesheetSlot> {
        self.slots.iter_mut().find(|slot| slot.id == slot_id)
    }

    fn rebuild_loaded_stylesheets(&mut self) {
        self.loaded_stylesheets.clear();
        self.loaded_stylesheets
            .extend(self.slots.iter().filter_map(|slot| match &slot.state {
                StylesheetSlotState::Loaded(stylesheet) => Some(stylesheet.clone()),
                StylesheetSlotState::Pending
                | StylesheetSlotState::Failed
                | StylesheetSlotState::Aborted => None,
            }));
        self.rebuild_cascade_stylesheets();
    }

    fn rebuild_cascade_stylesheets(&mut self) {
        self.cascade_stylesheets.clear();
        self.cascade_stylesheets.push(CascadedStylesheet {
            origin: CascadeOrigin::UserAgent,
            stylesheet: minimal_ua_stylesheet_parse(),
        });
        self.cascade_stylesheets
            .extend(
                self.loaded_stylesheets
                    .iter()
                    .cloned()
                    .map(|stylesheet| CascadedStylesheet {
                        origin: CascadeOrigin::Author,
                        stylesheet,
                    }),
            );
    }
}

impl Default for DocumentStyleSet {
    fn default() -> Self {
        let mut set = Self {
            next_slot_id: 0,
            slots: Vec::new(),
            loaded_stylesheets: Vec::new(),
            cascade_stylesheets: Vec::new(),
        };
        set.rebuild_cascade_stylesheets();
        set
    }
}

fn minimal_ua_stylesheet_parse() -> StylesheetParse {
    static MINIMAL_UA_STYLESHEET_PARSE: OnceLock<StylesheetParse> = OnceLock::new();

    MINIMAL_UA_STYLESHEET_PARSE
        .get_or_init(|| {
            parse_stylesheet_with_options(MINIMAL_UA_STYLESHEET, &ParseOptions::stylesheet())
        })
        .clone()
}

fn collect_stylesheet_inputs(
    node: &Node,
    base_url: Option<&str>,
    out: &mut Vec<StylesheetSlotKey>,
) {
    match node {
        Node::Document { children, .. } => {
            for child in children {
                collect_stylesheet_inputs(child, base_url, out);
            }
        }
        Node::Element { element } => {
            let name = element.name();
            let children = element.children();
            if element.namespace() == html::ElementNamespace::Html
                && name == "link"
                && node.attr_has_token("rel", "stylesheet")
                && let Some(href) = node.attr("href")
                && let Some(url) = resolve_url(base_url, href)
            {
                out.push(StylesheetSlotKey::External(url));
            } else if element.namespace() == html::ElementNamespace::Html && name == "style" {
                let mut text = String::new();
                for child in children {
                    if let Node::Text {
                        text: child_text, ..
                    } = child
                    {
                        text.push_str(child_text);
                    }
                }
                out.push(StylesheetSlotKey::Inline(text));
            }

            for child in children {
                collect_stylesheet_inputs(child, base_url, out);
            }
        }
        Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. }
        | Node::DocumentType { .. } => {}
    }
}

fn resolve_url(base_url: Option<&str>, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let base = Url::parse(base_url?).ok()?;
    base.join(href).ok().map(|url| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::DocumentStyleSet;
    use html::internal::Id;

    fn element(
        id: u32,
        namespace: html::ElementNamespace,
        local_name: &str,
        attributes: Vec<(&str, &str)>,
        children: Vec<html::Node>,
    ) -> html::Node {
        html::internal::node_element_from_parts(
            Id(id),
            html::internal::expanded_name(namespace, local_name),
            attributes
                .into_iter()
                .map(|(name, value)| html::internal::unqualified_attribute(name, value))
                .collect(),
            Vec::new(),
            children,
        )
    }

    #[test]
    fn foreign_style_and_link_lookalikes_are_not_document_stylesheet_inputs() {
        let dom = html::Node::Document {
            id: Id(1),
            doctype: None,
            children: vec![
                element(
                    2,
                    html::ElementNamespace::Svg,
                    "style",
                    Vec::new(),
                    vec![html::Node::Text {
                        id: Id(3),
                        text: "p { color: red; }".to_string(),
                    }],
                ),
                element(
                    4,
                    html::ElementNamespace::MathMl,
                    "link",
                    vec![("rel", "stylesheet"), ("href", "foreign.css")],
                    Vec::new(),
                ),
            ],
        };
        let mut set = DocumentStyleSet::default();
        let result = set.reconcile_from_dom(&dom, Some("https://example.test/"));
        assert!(result.fetches.is_empty());
        assert!(set.stylesheets().is_empty());
    }
}
