use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind};

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ParseOpts, QualName, parse_document};

pub(crate) const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_NODES: usize = 131_072;
const MAX_TOTAL_ATTRIBUTES: usize = 131_072;
const MAX_ATTRIBUTES_PER_ELEMENT: usize = 4_096;
const MAX_RETAINED_METADATA_BYTES: usize = 1024 * 1024;
const MAX_PARSE_ERRORS: usize = 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HtmlElementMetadata {
    pub name: String,
    pub attributes: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedHtmlMetadata {
    pub elements: Vec<HtmlElementMetadata>,
    pub parse_error_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlMetadataError {
    DocumentTooLarge,
    InvalidUtf8,
    NodeLimit,
    AttributeLimit,
    RetainedMetadataLimit,
    ParseErrorLimit,
}

#[derive(Debug)]
struct ParserLimitAbort(HtmlMetadataError);

#[derive(Debug)]
enum NodeData {
    Document,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: usize,
    },
    Text(StrTendril),
    Other,
}
#[derive(Debug)]
struct Node {
    parent: Option<usize>,
    children: Vec<usize>,
    data: NodeData,
}
#[derive(Debug)]
struct State {
    nodes: Vec<Node>,
    attributes: usize,
    metadata_bytes: usize,
    parse_errors: usize,
}

struct MetadataSink {
    state: RefCell<State>,
    next_line: Cell<u64>,
}

impl MetadataSink {
    fn new() -> Self {
        Self {
            state: RefCell::new(State {
                nodes: vec![Node {
                    parent: None,
                    children: Vec::new(),
                    data: NodeData::Document,
                }],
                attributes: 0,
                metadata_bytes: 0,
                parse_errors: 0,
            }),
            next_line: Cell::new(1),
        }
    }
    fn allocate(&self, data: NodeData) -> usize {
        let mut state = self.state.borrow_mut();
        if state.nodes.len() >= MAX_NODES {
            panic_any(ParserLimitAbort(HtmlMetadataError::NodeLimit))
        }
        let id = state.nodes.len();
        state.nodes.push(Node {
            parent: None,
            children: Vec::new(),
            data,
        });
        id
    }
    fn detach(state: &mut State, target: usize) {
        if let Some(parent) = state.nodes[target].parent.take() {
            state.nodes[parent]
                .children
                .retain(|child| *child != target)
        }
    }
    fn append_node(&self, parent: usize, child: usize) {
        let mut state = self.state.borrow_mut();
        Self::detach(&mut state, child);
        state.nodes[child].parent = Some(parent);
        state.nodes[parent].children.push(child);
    }
    fn append_text(&self, parent: usize, text: StrTendril, before: Option<usize>) {
        let mut state = self.state.borrow_mut();
        let insertion = before
            .and_then(|sibling| {
                state.nodes[parent]
                    .children
                    .iter()
                    .position(|child| *child == sibling)
            })
            .unwrap_or(state.nodes[parent].children.len());
        if insertion > 0 {
            let previous = state.nodes[parent].children[insertion - 1];
            if let NodeData::Text(existing) = &mut state.nodes[previous].data {
                existing.push_tendril(&text);
                return;
            }
        }
        if state.nodes.len() >= MAX_NODES {
            panic_any(ParserLimitAbort(HtmlMetadataError::NodeLimit))
        }
        let id = state.nodes.len();
        state.nodes.push(Node {
            parent: Some(parent),
            children: Vec::new(),
            data: NodeData::Text(text),
        });
        state.nodes[parent].children.insert(insertion, id);
    }
    fn parent_of(&self, node: usize) -> Option<usize> {
        self.state.borrow().nodes[node].parent
    }
}

impl TreeSink for MetadataSink {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = Ref<'a, QualName>;
    fn finish(self) -> Self {
        self
    }
    fn parse_error(&self, _: Cow<'static, str>) {
        let mut state = self.state.borrow_mut();
        state.parse_errors += 1;
        if state.parse_errors > MAX_PARSE_ERRORS {
            panic_any(ParserLimitAbort(HtmlMetadataError::ParseErrorLimit))
        }
    }
    fn get_document(&self) -> usize {
        0
    }
    fn elem_name<'a>(&'a self, target: &'a usize) -> Self::ElemName<'a> {
        Ref::map(self.state.borrow(), |state| {
            match &state.nodes[*target].data {
                NodeData::Element { name, .. } => name,
                _ => panic!("tree builder requested an element name for a non-element"),
            }
        })
    }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _: ElementFlags) -> usize {
        if attrs.len() > MAX_ATTRIBUTES_PER_ELEMENT {
            panic_any(ParserLimitAbort(HtmlMetadataError::AttributeLimit))
        }
        {
            let mut state = self.state.borrow_mut();
            state.attributes = state.attributes.saturating_add(attrs.len());
            if state.attributes > MAX_TOTAL_ATTRIBUTES {
                panic_any(ParserLimitAbort(HtmlMetadataError::AttributeLimit))
            }
            let added = name.local.len()
                + attrs
                    .iter()
                    .map(|attr| attr.name.local.len() + attr.value.len())
                    .sum::<usize>();
            state.metadata_bytes = state.metadata_bytes.saturating_add(added);
            if state.metadata_bytes > MAX_RETAINED_METADATA_BYTES {
                panic_any(ParserLimitAbort(HtmlMetadataError::RetainedMetadataLimit))
            }
        }
        let template_contents = self.allocate(NodeData::Other);
        self.allocate(NodeData::Element {
            name,
            attrs,
            template_contents,
        })
    }
    fn create_comment(&self, _: StrTendril) -> usize {
        self.allocate(NodeData::Other)
    }
    fn create_pi(&self, _: StrTendril, _: StrTendril) -> usize {
        self.allocate(NodeData::Other)
    }
    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        match child {
            NodeOrText::AppendNode(node) => self.append_node(*parent, node),
            NodeOrText::AppendText(text) => self.append_text(*parent, text, None),
        }
    }
    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        if self.parent_of(*element).is_some() {
            self.append_before_sibling(element, child)
        } else {
            self.append(prev_element, child)
        }
    }
    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {
        let node = self.allocate(NodeData::Other);
        self.append_node(0, node)
    }
    fn get_template_contents(&self, target: &usize) -> usize {
        match &self.state.borrow().nodes[*target].data {
            NodeData::Element {
                template_contents, ..
            } => *template_contents,
            _ => panic!("template contents requested for non-element"),
        }
    }
    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }
    fn set_quirks_mode(&self, _: QuirksMode) {}
    fn append_before_sibling(&self, sibling: &usize, new_node: NodeOrText<usize>) {
        let mut state = self.state.borrow_mut();
        let parent = state.nodes[*sibling]
            .parent
            .expect("tree builder promised sibling parent");
        match new_node {
            NodeOrText::AppendNode(node) => {
                Self::detach(&mut state, node);
                let index = state.nodes[parent]
                    .children
                    .iter()
                    .position(|child| child == sibling)
                    .expect("sibling missing from parent");
                state.nodes[node].parent = Some(parent);
                state.nodes[parent].children.insert(index, node)
            }
            NodeOrText::AppendText(text) => {
                drop(state);
                self.append_text(parent, text, Some(*sibling))
            }
        }
    }
    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        let mut state = self.state.borrow_mut();
        let missing = {
            let NodeData::Element {
                attrs: existing, ..
            } = &state.nodes[*target].data
            else {
                panic!("tree builder promised element")
            };
            attrs
                .into_iter()
                .filter(|attr| !existing.iter().any(|current| current.name == attr.name))
                .collect::<Vec<_>>()
        };
        let existing_len = match &state.nodes[*target].data {
            NodeData::Element { attrs, .. } => attrs.len(),
            _ => unreachable!(),
        };
        if existing_len + missing.len() > MAX_ATTRIBUTES_PER_ELEMENT
            || state.attributes + missing.len() > MAX_TOTAL_ATTRIBUTES
        {
            panic_any(ParserLimitAbort(HtmlMetadataError::AttributeLimit))
        }
        let added = missing
            .iter()
            .map(|attr| attr.name.local.len() + attr.value.len())
            .sum::<usize>();
        if state
            .metadata_bytes
            .checked_add(added)
            .is_none_or(|total| total > MAX_RETAINED_METADATA_BYTES)
        {
            panic_any(ParserLimitAbort(HtmlMetadataError::RetainedMetadataLimit))
        }
        state.attributes += missing.len();
        state.metadata_bytes += added;
        let NodeData::Element {
            attrs: existing, ..
        } = &mut state.nodes[*target].data
        else {
            unreachable!()
        };
        existing.extend(missing)
    }
    fn remove_from_parent(&self, target: &usize) {
        Self::detach(&mut self.state.borrow_mut(), *target)
    }
    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let children = std::mem::take(&mut self.state.borrow_mut().nodes[*node].children);
        for child in children {
            self.append_node(*new_parent, child)
        }
    }
    fn set_current_line(&self, line_number: u64) {
        self.next_line.set(line_number)
    }
    fn is_mathml_annotation_xml_integration_point(&self, handle: &usize) -> bool {
        let state = self.state.borrow();
        let NodeData::Element { name, attrs, .. } = &state.nodes[*handle].data else {
            return false;
        };
        name.ns.as_ref() == "http://www.w3.org/1998/Math/MathML"
            && name.local.as_ref() == "annotation-xml"
            && attrs.iter().any(|attribute| {
                attribute.name.ns.as_ref().is_empty()
                    && attribute.name.local.as_ref() == "encoding"
                    && (attribute.value.eq_ignore_ascii_case("text/html")
                        || attribute
                            .value
                            .eq_ignore_ascii_case("application/xhtml+xml"))
            })
    }
    fn allow_declarative_shadow_roots(&self, _intended_parent: &usize) -> bool {
        false
    }
}

pub(crate) fn parse_html_metadata(bytes: &[u8]) -> Result<ParsedHtmlMetadata, HtmlMetadataError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(HtmlMetadataError::DocumentTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| HtmlMetadataError::InvalidUtf8)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        parse_document(MetadataSink::new(), ParseOpts::default()).one(text)
    }));
    let sink = match result {
        Ok(sink) => sink,
        Err(payload) => match payload.downcast::<ParserLimitAbort>() {
            Ok(limit) => return Err(limit.0),
            Err(payload) => resume_unwind(payload),
        },
    };
    let state = sink.state.into_inner();
    let mut elements = Vec::new();
    for node in state.nodes {
        if let NodeData::Element { name, attrs, .. } = node.data {
            let mut attributes = attrs
                .into_iter()
                .map(|attr| (attr.name.local.to_string(), attr.value.to_string()))
                .collect::<Vec<_>>();
            attributes.sort();
            elements.push(HtmlElementMetadata {
                name: name.local.to_string(),
                attributes,
            });
        }
    }
    Ok(ParsedHtmlMetadata {
        elements,
        parse_error_count: state.parse_errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scripts(input: &str) -> Vec<String> {
        parse_html_metadata(input.as_bytes())
            .unwrap()
            .elements
            .into_iter()
            .filter(|e| e.name == "script")
            .filter_map(|e| {
                e.attributes
                    .into_iter()
                    .find(|(k, _)| k == "src")
                    .map(|(_, v)| v)
            })
            .collect()
    }
    #[test]
    fn script_data_markup_is_not_an_element() {
        assert_eq!(
            scripts("<script>const x = '<script src=\"/resources/testdriver.js\">'</script>"),
            Vec::<String>::new()
        )
    }
    #[test]
    fn raw_text_does_not_create_metadata() {
        assert_eq!(
            scripts("<style>.x{content:'<script src=/resources/testharness.js>'}</style>"),
            Vec::<String>::new()
        )
    }
    #[test]
    fn actual_scripts_and_attribute_forms_are_normalized() {
        assert_eq!(
            scripts(
                "<ScRiPt SrC=/resources/testharness.js></sCrIpT><script src='/resources/testdriver.js'></script>"
            ),
            vec!["/resources/testharness.js", "/resources/testdriver.js"]
        )
    }
    #[test]
    fn malformed_but_tokenizable_metadata_is_retained() {
        let parsed = parse_html_metadata(b"<link REL=match href='ref.html'><p broken=").unwrap();
        assert!(parsed.elements.iter().any(|e| e.name == "link"))
    }

    #[test]
    fn mathml_html_integration_points_preserve_tree_builder_state() {
        assert_eq!(
            scripts(
                "<math><annotation-xml encoding='text/html'><script src=/resources/testharness.js></script></annotation-xml></math>"
            ),
            vec!["/resources/testharness.js"]
        );
    }
    #[test]
    fn limits_are_typed() {
        assert_eq!(
            parse_html_metadata(&vec![b'a'; MAX_DOCUMENT_BYTES + 1]),
            Err(HtmlMetadataError::DocumentTooLarge)
        )
    }
}
