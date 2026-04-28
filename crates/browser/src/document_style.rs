use core_types::StylesheetSlotId;
use css::{ParseOptions, StylesheetParse, parse_stylesheet_with_options};
use html::Node;
use url::Url;

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
pub(crate) struct StylesheetFetch {
    pub(crate) slot_id: StylesheetSlotId,
    pub(crate) url: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DocumentStyleSet {
    next_slot_id: u64,
    slots: Vec<StylesheetSlot>,
    loaded_stylesheets: Vec<StylesheetParse>,
}

impl DocumentStyleSet {
    pub(crate) fn clear(&mut self) {
        self.next_slot_id = 0;
        self.slots.clear();
        self.loaded_stylesheets.clear();
    }

    pub(crate) fn reconcile_from_dom(
        &mut self,
        dom: &Node,
        base_url: Option<&str>,
    ) -> Vec<StylesheetFetch> {
        let mut discovered = Vec::new();
        collect_stylesheet_inputs(dom, base_url, &mut discovered);

        let old_slots = std::mem::take(&mut self.slots);
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
        fetches
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

    pub(crate) fn mark_external_done(&mut self, slot_id: StylesheetSlotId) {
        if let Some(slot) = self.slot_mut(slot_id)
            && matches!(slot.state, StylesheetSlotState::Pending)
        {
            slot.state = StylesheetSlotState::Failed;
        }
        self.rebuild_loaded_stylesheets();
    }

    pub(crate) fn mark_external_failed(&mut self, slot_id: StylesheetSlotId) {
        if let Some(slot) = self.slot_mut(slot_id) {
            slot.state = StylesheetSlotState::Failed;
        }
        self.rebuild_loaded_stylesheets();
    }

    pub(crate) fn mark_external_aborted(&mut self, slot_id: StylesheetSlotId) {
        if let Some(slot) = self.slot_mut(slot_id) {
            slot.state = StylesheetSlotState::Aborted;
        }
        self.rebuild_loaded_stylesheets();
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
    }
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
        Node::Element { name, children, .. } => {
            if name.as_ref() == "link"
                && node.attr_has_token("rel", "stylesheet")
                && let Some(href) = node.attr("href")
                && let Some(url) = resolve_url(base_url, href)
            {
                out.push(StylesheetSlotKey::External(url));
            } else if name.as_ref() == "style" {
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
        Node::Text { .. } | Node::Comment { .. } => {}
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
