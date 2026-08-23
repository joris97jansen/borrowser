use core_types::StylesheetSlotId;
use css::{
    ParseOptions, StylesheetCollectionInput, StylesheetCollectionInputBuildError,
    StylesheetConditionInput, StylesheetOrder, StylesheetParse, StylesheetSourceId,
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
enum StylesheetSlotSource {
    Inline(String),
    External(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiscoveredStylesheet {
    source: StylesheetSlotSource,
    media: Option<String>,
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
    source: StylesheetSlotSource,
    media: Option<String>,
    state: StylesheetSlotState,
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
    ) -> StylesheetReconcileResult {
        let mut discovered = Vec::new();
        collect_stylesheet_inputs(dom, base_url, &mut discovered);

        let old_slots = std::mem::take(&mut self.slots);
        let changed = old_slots.len() != discovered.len()
            || old_slots.iter().zip(&discovered).any(|(slot, discovered)| {
                slot.source != discovered.source || slot.media != discovered.media
            });
        let mut used = vec![false; old_slots.len()];
        let mut fetches = Vec::new();
        let mut new_slots = Vec::with_capacity(discovered.len());

        for discovered in discovered {
            if let Some((index, old)) = old_slots
                .iter()
                .enumerate()
                .find(|(index, slot)| !used[*index] && slot.source == discovered.source)
            {
                used[index] = true;
                let mut retained = old.clone();
                retained.media = discovered.media;
                new_slots.push(retained);
                continue;
            }

            let id = self.allocate_slot_id();
            let state = match &discovered.source {
                StylesheetSlotSource::Inline(text) => StylesheetSlotState::Loaded(
                    parse_stylesheet_with_options(text, &ParseOptions::stylesheet()),
                ),
                StylesheetSlotSource::External(url) => {
                    fetches.push(StylesheetFetch {
                        slot_id: id,
                        url: url.clone(),
                    });
                    StylesheetSlotState::Pending
                }
            };

            new_slots.push(StylesheetSlot {
                id,
                source: discovered.source,
                media: discovered.media,
                state,
            });
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
            source: StylesheetSlotSource::External(url.to_string()),
            media: None,
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
        if !matches!(slot.source, StylesheetSlotSource::External(_)) {
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

    pub(crate) fn stylesheet_collection_inputs(
        &self,
    ) -> Result<Vec<StylesheetCollectionInput<'_>>, StylesheetCollectionInputBuildError> {
        let mut inputs = Vec::new();
        let input_capacity = self.slots.len().checked_add(1).ok_or(
            css::SourceCoordinateError::CounterExhausted {
                coordinate: "browser-stylesheet-input-count",
            },
        )?;
        inputs
            .try_reserve(input_capacity)
            .map_err(|_| StylesheetCollectionInputBuildError::Reservation)?;
        inputs.push(StylesheetCollectionInput::user_agent_for_namespace(
            StylesheetSourceId::built_in_user_agent(),
            StylesheetOrder::new(0),
            minimal_ua_stylesheet_parse(),
            html::ElementNamespace::Html,
        ));

        for (slot_index, slot) in self.slots.iter().enumerate() {
            let StylesheetSlotState::Loaded(stylesheet) = &slot.state else {
                continue;
            };
            let order_index =
                slot_index
                    .checked_add(1)
                    .ok_or(css::SourceCoordinateError::CounterExhausted {
                        coordinate: "browser-stylesheet-order",
                    })?;
            let order = StylesheetOrder::from_usize(order_index)?;
            let source_id = StylesheetSourceId::from_browser_slot(slot.id.0)?;
            inputs.push(StylesheetCollectionInput::author(
                source_id,
                order,
                stylesheet,
                StylesheetConditionInput::from_optional_raw_media(slot.media.as_deref()),
            ));
        }
        Ok(inputs)
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

fn minimal_ua_stylesheet_parse() -> &'static StylesheetParse {
    static MINIMAL_UA_STYLESHEET_PARSE: OnceLock<StylesheetParse> = OnceLock::new();

    MINIMAL_UA_STYLESHEET_PARSE.get_or_init(|| {
        parse_stylesheet_with_options(MINIMAL_UA_STYLESHEET, &ParseOptions::stylesheet())
    })
}

fn collect_stylesheet_inputs(
    node: &Node,
    base_url: Option<&str>,
    out: &mut Vec<DiscoveredStylesheet>,
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
                out.push(DiscoveredStylesheet {
                    source: StylesheetSlotSource::External(url),
                    media: node.attr("media").map(str::to_string),
                });
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
                out.push(DiscoveredStylesheet {
                    source: StylesheetSlotSource::Inline(text),
                    media: node.attr("media").map(str::to_string),
                });
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
mod af5_tests {
    use super::*;
    use html::{HtmlParseOptions, parse_document};

    fn parsed(source: &str) -> html::ParseOutput {
        parse_document(source, HtmlParseOptions::default()).expect("HTML parses")
    }

    #[test]
    fn css_handoff_preserves_sparse_document_order_and_source_identity() {
        let output = parsed(concat!(
            "<!doctype html><html><head>",
            "<link rel=stylesheet href=a.css>",
            "<style>p { color: red; }</style>",
            "</head><body><p></p></body></html>"
        ));
        let mut set = DocumentStyleSet::default();
        let reconcile = set.reconcile_from_dom(&output.document, Some("https://example.com/"));
        let external = reconcile.fetches[0].slot_id;

        let first = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(first.len(), 2, "pending slot remains Browser-owned");
        assert_eq!(first[0].order().get(), 0);
        assert_eq!(
            first[1].order().get(),
            2,
            "available slots are not compacted"
        );
        let inline_source = first[1].source_id();

        let second = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(second[1].source_id(), inline_source);

        assert!(set.install_external_stylesheet(external, "p { color: blue; }"));
        let complete = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            complete
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(complete[2].source_id(), inline_source);
        assert_ne!(complete[1].source_id(), complete[2].source_id());
    }

    #[test]
    fn exact_media_metadata_changes_reconciliation_and_is_not_parsed_by_browser() {
        let screen = parsed(
            "<!doctype html><html><head><style media='screen'>p { color: red; }</style></head><body></body></html>",
        );
        let print = parsed(
            "<!doctype html><html><head><style media='print'>p { color: red; }</style></head><body></body></html>",
        );
        let mut set = DocumentStyleSet::default();
        assert!(set.reconcile_from_dom(&screen.document, None).changed);
        let inputs = set.stylesheet_collection_inputs().unwrap();
        let source_id = inputs[1].source_id();
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("screen")
        );
        assert!(set.reconcile_from_dom(&print.document, None).changed);
        let inputs = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(inputs[1].source_id(), source_id);
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("print")
        );
    }

    #[test]
    fn loaded_external_media_change_preserves_slot_parse_and_source_without_refetch() {
        let screen = parsed(
            "<!doctype html><html><head><link rel=stylesheet href=a.css media=screen></head><body></body></html>",
        );
        let print = parsed(
            "<!doctype html><html><head><link rel=stylesheet href=a.css media=print></head><body></body></html>",
        );
        let mut set = DocumentStyleSet::default();
        let first = set.reconcile_from_dom(&screen.document, Some("https://example.com/"));
        assert_eq!(first.fetches.len(), 1);
        let slot_id = first.fetches[0].slot_id;
        assert!(set.install_external_stylesheet(slot_id, "p { color: red; }"));
        let source_id = set.stylesheet_collection_inputs().unwrap()[1].source_id();

        let changed = set.reconcile_from_dom(&print.document, Some("https://example.com/"));
        assert!(changed.changed);
        assert!(
            changed.fetches.is_empty(),
            "media-only change must not refetch"
        );
        assert_eq!(set.stylesheets().len(), 1, "loaded parse remains available");
        let inputs = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(inputs[1].source_id(), source_id);
        assert_eq!(
            inputs[1].condition(),
            StylesheetConditionInput::RawMedia("print")
        );
    }

    #[test]
    fn duplicate_urls_keep_distinct_source_ids_and_completion_does_not_define_order() {
        let output = parsed(concat!(
            "<!doctype html><html><head>",
            "<link rel=stylesheet href=same.css>",
            "<link rel=stylesheet href=same.css>",
            "</head><body></body></html>",
        ));
        let mut set = DocumentStyleSet::default();
        let reconcile = set.reconcile_from_dom(&output.document, Some("https://example.com/"));
        assert_eq!(reconcile.fetches.len(), 2);
        let first = reconcile.fetches[0].slot_id;
        let second = reconcile.fetches[1].slot_id;
        assert_ne!(first, second);

        assert!(set.install_external_stylesheet(second, "p { color: blue; }"));
        let one_available = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            one_available
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 2]
        );

        assert!(set.install_external_stylesheet(first, "p { color: red; }"));
        let all_available = set.stylesheet_collection_inputs().unwrap();
        assert_eq!(
            all_available
                .iter()
                .map(|input| input.order().get())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_ne!(all_available[1].source_id(), all_available[2].source_id());
    }

    #[test]
    fn historical_style_scoped_attribute_is_ignored_and_contents_remain_global_author_css() {
        let output = parsed(concat!(
            "<!doctype html><html><body>",
            "<section><style scoped>p { color: red; }</style><p id=inside></p></section>",
            "<p id=outside></p>",
            "</body></html>",
        ));
        let mut set = DocumentStyleSet::default();
        assert!(set.reconcile_from_dom(&output.document, None).changed);
        let inputs = set
            .stylesheet_collection_inputs()
            .expect("ordinary style input remains representable");
        assert_eq!(inputs.len(), 2, "UA plus one ordinary author stylesheet");

        let diagnostic = css::rule_collection_diagnostic(
            &output.document,
            css::SelectorMatchingEnvironment::new(output.document_mode),
            &inputs,
            &css::StyleResolutionLimits::default(),
            css::RuleCollectionDiagnosticLimits::default(),
        );
        assert!(!diagnostic.to_debug_snapshot().contains("skipped-at-scope"));

        let resolved = css::try_resolve_document_styles_from_cascade_inputs_with_limits(
            &output.document,
            css::SelectorMatchingEnvironment::new(output.document_mode),
            &inputs,
            &css::StyleResolutionLimits::default(),
        )
        .expect("ordinary global author stylesheet resolves");
        let paragraph_colors = resolved
            .entries()
            .iter()
            .filter(|entry| entry.element_name() == "p")
            .map(|entry| {
                entry
                    .style()
                    .get(css::CascadePropertyId::Color)
                    .expect("both paragraphs receive a color")
                    .source()
            })
            .collect::<Vec<_>>();
        assert_eq!(paragraph_colors.len(), 2);
        assert!(paragraph_colors.iter().all(|source| matches!(
            source,
            css::ResolvedValueSource::Winner(winner)
                if winner.value.to_css_text().as_deref() == Some("red")
        )));
    }
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
