use super::super::Tab;
use bus::{DocumentPublication, DocumentPublicationPayload};
use core_types::{DomHandle, DomVersion};
use css::StyledNode;
use html::{DomPatch, Node, PatchKey, internal::Id};
use layout::TextMeasurer;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const ADDRESS_URL: &str = "https://example.test/dir/page?q=1#initial";

/// Add a real parser-publication attachment and let production discovery issue it.
pub(super) fn append_stylesheet_link(tab: &mut Tab, url: &str) -> core_types::StylesheetSlotId {
    let before: std::collections::HashSet<_> = tab.stylesheet_loads.keys().copied().collect();
    let parent = PatchKey::from_id(tab.page.dom.as_deref().expect("committed fixture").id());
    let key = PatchKey(900_000);
    publish_addressing_patches(
        tab,
        vec![
            DomPatch::CreateElement {
                key,
                name: html::internal::html_name("link"),
                attributes: vec![
                    html::internal::unqualified_attribute("rel", "stylesheet"),
                    html::internal::unqualified_attribute("href", url),
                ],
            },
            DomPatch::AppendChild { parent, child: key },
        ],
    )
    .unwrap();
    *tab.stylesheet_loads
        .keys()
        .find(|id| !before.contains(id))
        .expect("production dispatch")
}

/// Establish the actual network/runtime protocol for cascade/lifetime fixtures.
/// The slot must already have been issued by production discovery.
pub(super) fn complete_css(
    tab: &mut Tab,
    slot: core_types::StylesheetSlotId,
    url: &str,
    text: &str,
) {
    let response = response_facts(url.into(), url.into(), Some(200), Some("text/css".into()));
    complete_css_with_response(tab, slot, response, text);
}

pub(super) fn complete_css_with_response(
    tab: &mut Tab,
    slot: core_types::StylesheetSlotId,
    response: net::NetworkResponseInfo,
    text: &str,
) {
    use bus::CoreEvent;
    use core_types::ResourceKind;
    let request_url = response.requested_url().to_owned();
    let url = request_url.as_str();
    let tab_id = tab.tab_id;
    let request_id = tab.nav_gen;
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: response.clone(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.into(),
        bytes: text.as_bytes().to_vec(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response,
        body: net::BodyProgress::try_new(ResourceKind::Css, text.len()).unwrap(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id,
        request_id,
        stylesheet_slot_id: slot,
        url: url.into(),
        css_block: text.into(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::CssSheetDone {
        tab_id,
        request_id,
        stylesheet_slot_id: slot,
        url: url.into(),
    })
    .unwrap();
}

pub(super) fn addressing_tab(source: &str) -> (Tab, std::sync::mpsc::Receiver<bus::CoreCommand>) {
    addressing_tab_with_url(source, ADDRESS_URL)
}

pub(super) fn addressing_tab_with_url(
    source: &str,
    document_url: &str,
) -> (Tab, std::sync::mpsc::Receiver<bus::CoreCommand>) {
    addressing_tab_with_response_url(source, document_url, document_url)
}

/// Inject an explicit legacy final response, including the known backend's
/// credential-bearing redirect limitation. Initial issuance still uses normal
/// production admission; this helper never dispatches rejected initial URLs.
pub(super) fn addressing_tab_with_response_url(
    source: &str,
    requested_url: &str,
    document_url: &str,
) -> (Tab, std::sync::mpsc::Receiver<bus::CoreCommand>) {
    let mut tab = Tab::new(1);
    tab.nav_gen = 6;
    let (tx, rx) = std::sync::mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new(requested_url.into());
    tab.on_core_event(bus::CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: None,
        kind: core_types::ResourceKind::Html,
        response: response_facts(
            addressing::BrowserUrl::parse(requested_url)
                .unwrap()
                .url
                .as_str()
                .to_owned(),
            document_url.into(),
            if addressing::BrowserUrl::parse(requested_url)
                .unwrap()
                .url
                .scheme()
                == "file"
            {
                None
            } else {
                Some(200)
            },
            Some("text/html".into()),
        ),
    })
    .unwrap();
    rx.try_iter().for_each(drop);
    let output = html::parse_document(source, html::HtmlParseOptions::default()).unwrap();
    tab.on_core_event(bus::CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: 7,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    (tab, rx)
}

pub(super) fn publish_addressing_patches(
    tab: &mut Tab,
    patches: Vec<DomPatch>,
) -> Result<(), bus::DocumentPublicationFailure> {
    tab.on_core_event(bus::CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        publication: no_quirks_patch_publication(
            tab.dom_handle.unwrap(),
            tab.dom_version,
            tab.dom_version.next(),
            patches,
        ),
    })
}

pub(super) fn element_id(node: &Node, id: &str) -> Id {
    fn find(node: &Node, id: &str) -> Option<Id> {
        if node.attr("id") == Some(id) {
            return Some(node.id());
        }
        match node {
            Node::Document { children, .. } => children.iter().find_map(|node| find(node, id)),
            Node::Element { element } => element.children().iter().find_map(|node| find(node, id)),
            _ => None,
        }
    }
    find(node, id).expect("explicit fixture element")
}

pub(super) fn attributes(key: Id, attrs: &[(&str, &str)]) -> DomPatch {
    DomPatch::SetAttributes {
        key: PatchKey::from_id(key),
        attributes: attrs
            .iter()
            .map(|(name, value)| html::internal::unqualified_attribute(name, *value))
            .collect(),
    }
}

pub(super) fn page_matching_environment(tab: &Tab) -> css::SelectorMatchingEnvironment {
    css::SelectorMatchingEnvironment::new(
        tab.page
            .document_mode
            .expect("test document publication should retain document mode"),
    )
}

pub(super) fn no_quirks_patch_publication_from_output(
    output: html::ParseOutput,
) -> DocumentPublication {
    static TEST_HANDLE: AtomicU64 = AtomicU64::new(10_000);
    let handle = DomHandle(TEST_HANDLE.fetch_add(1, Ordering::Relaxed));
    DocumentPublication {
        handle,
        document_mode: output.document_mode,
        payload: DocumentPublicationPayload::Patch {
            from: DomVersion::INITIAL,
            to: DomVersion(1),
            patches: output.patches,
        },
    }
}

pub(super) fn no_quirks_patch_publication_from_dom(dom: Box<Node>) -> DocumentPublication {
    static TEST_HANDLE: AtomicU64 = AtomicU64::new(20_000);
    let handle = DomHandle(TEST_HANDLE.fetch_add(1, Ordering::Relaxed));
    let mut patches = vec![html::DomPatch::Clear];
    append_manual_dom_patches(&dom, None, &mut patches);
    DocumentPublication {
        handle,
        document_mode: html::DocumentMode::NoQuirks,
        payload: DocumentPublicationPayload::Patch {
            from: DomVersion::INITIAL,
            to: DomVersion(1),
            patches,
        },
    }
}

fn append_manual_dom_patches(node: &Node, parent: Option<PatchKey>, patches: &mut Vec<DomPatch>) {
    let key = PatchKey::from_id(node.id());
    match node {
        Node::Document {
            doctype, children, ..
        } => {
            patches.push(DomPatch::CreateDocument {
                key,
                doctype: doctype.clone(),
            });
            for child in children {
                append_manual_dom_patches(child, Some(key), patches);
            }
        }
        Node::Element { element } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: element.expanded_name().clone(),
                attributes: element.attributes().to_vec(),
            });
            if let Some(parent) = parent {
                patches.push(DomPatch::AppendChild { parent, child: key });
            }
            for child in element.children() {
                append_manual_dom_patches(child, Some(key), patches);
            }
        }
        Node::Text { text, .. } => {
            patches.push(DomPatch::CreateText {
                key,
                text: text.clone(),
            });
            let parent = parent.expect("manual text node must have a parent");
            patches.push(DomPatch::AppendChild { parent, child: key });
        }
        Node::Comment { text, .. } => {
            patches.push(DomPatch::CreateComment {
                key,
                text: text.clone(),
            });
            let parent = parent.expect("manual comment node must have a parent");
            patches.push(DomPatch::AppendChild { parent, child: key });
        }
        Node::DocumentType { .. } | Node::ProcessingInstruction { .. } => {
            panic!("manual publication helper does not support this node kind")
        }
    }
}

pub(super) fn no_quirks_patch_publication(
    handle: DomHandle,
    from: DomVersion,
    to: DomVersion,
    patches: Vec<DomPatch>,
) -> DocumentPublication {
    DocumentPublication {
        handle,
        document_mode: html::DocumentMode::NoQuirks,
        payload: DocumentPublicationPayload::Patch { from, to, patches },
    }
}

pub(super) fn find_styled_element<'a>(
    node: &'a StyledNode<'a>,
    want: &str,
) -> Option<&'a StyledNode<'a>> {
    if let Node::Element { element } = node.node
        && element.name() == want
    {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_element(child, want))
}

pub(super) struct FixedTextMeasurer;

impl TextMeasurer for FixedTextMeasurer {
    fn measure(&self, text: &str, _style: &css::ComputedStyle) -> f32 {
        text.chars().count() as f32 * 8.0
    }

    fn line_height(&self, _style: &css::ComputedStyle) -> f32 {
        16.0
    }
}

pub(super) fn current_element_color(tab: &mut Tab, name: &str) -> (u8, u8, u8, u8) {
    current_element_color_optional(tab, name).expect("styled element should exist")
}

pub(super) fn current_element_color_optional(
    tab: &mut Tab,
    name: &str,
) -> Option<(u8, u8, u8, u8)> {
    let style_output = tab
        .page
        .build_style_phase_output()
        .expect("style phase output should build")?;
    find_styled_element(style_output.root(), name).map(|node| node.style.color())
}

pub(super) fn current_element_color_by_id(tab: &mut Tab, id: Id) -> (u8, u8, u8, u8) {
    let style_output = tab
        .page
        .build_style_phase_output()
        .expect("style phase output should build")
        .expect("document should be styled");
    find_styled_node_id(style_output.root(), id)
        .map(|node| node.style.color())
        .expect("styled node should exist")
}

pub(super) fn find_styled_node_id<'a>(
    node: &'a StyledNode<'a>,
    want: Id,
) -> Option<&'a StyledNode<'a>> {
    if node.node_id == want {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_node_id(child, want))
}

pub(super) fn initial_patch_document(
    style_text: &str,
    body_element: Option<&str>,
) -> Vec<DomPatch> {
    let mut patches = vec![
        DomPatch::Clear,
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: html::internal::html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: html::internal::html_name("head"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: html::internal::html_name("style"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::CreateText {
            key: PatchKey(5),
            text: style_text.to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(5),
        },
        DomPatch::CreateElement {
            key: PatchKey(6),
            name: html::internal::html_name("body"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(6),
        },
    ];

    if let Some(name) = body_element {
        patches.extend([
            DomPatch::CreateElement {
                key: PatchKey(7),
                name: html::internal::html_name(name),
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(8),
                text: "Hello".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(8),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
        ]);
    }

    patches
}

pub(super) fn two_paragraph_patch_document(style_text: &str) -> Vec<DomPatch> {
    let mut patches = initial_patch_document(style_text, None);
    patches.extend([
        DomPatch::CreateElement {
            key: PatchKey(7),
            name: html::internal::html_name("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "First".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(7),
            child: PatchKey(8),
        },
        DomPatch::AppendChild {
            parent: PatchKey(6),
            child: PatchKey(7),
        },
        DomPatch::CreateElement {
            key: PatchKey(9),
            name: html::internal::html_name("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(10),
            text: "Second".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(9),
            child: PatchKey(10),
        },
        DomPatch::AppendChild {
            parent: PatchKey(6),
            child: PatchKey(9),
        },
    ]);
    patches
}

pub(super) fn find_dom_element<'a>(node: &'a Node, want: &str) -> Option<&'a Node> {
    match node {
        Node::Element { element } => {
            if element.name() == want {
                return Some(node);
            }
            element
                .children()
                .iter()
                .find_map(|child| find_dom_element(child, want))
        }
        Node::Document { children, .. } => children
            .iter()
            .find_map(|child| find_dom_element(child, want)),
        Node::Text { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. }
        | Node::DocumentType { .. } => None,
    }
}

/// Construct fixture facts through the same validated response APIs as production.
/// The explicit status distinguishes HTTP from local-resource facts.
#[track_caller]
pub(super) fn response_facts(
    requested: String,
    reported: String,
    status: Option<u16>,
    content_type: Option<String>,
) -> net::NetworkResponseInfo {
    let original = addressing::BrowserUrl::parse(&requested)
        .expect("fixture request URL")
        .url;
    // AH8's existing fixture convention uses an empty string for absent provenance.
    let reported = (!reported.is_empty()).then_some(reported);
    if let Some(code) = status {
        let mut headers = net::HttpHeaders::new();
        if let Some(value) = content_type {
            headers.append("content-type", value.as_bytes()).unwrap();
        }
        net::NetworkResponseInfo::http(
            original,
            requested,
            reported,
            net::HttpStatus::try_from(code).unwrap(),
            headers,
        )
        .expect("HTTP fixture facts")
    } else {
        net::NetworkResponseInfo::local_file(original, requested, reported, content_type)
            .expect("local fixture facts")
    }
}
