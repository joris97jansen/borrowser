use super::Tab;
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkResponseInfo, ResourceKind};
use css::{StyledNode, build_style_tree_with_stylesheets};
use html::{HtmlParseOptions, Node, internal::Id, parse_document};
use std::sync::Arc;
use std::sync::mpsc;

#[test]
fn redirected_document_response_updates_tab_base_url_and_status() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 1;
    let requested = "https://example.com".to_string();
    let final_url = "https://example.com/landing".to_string();
    let response = NetworkResponseInfo {
        requested_url: requested,
        final_url: final_url.clone(),
        status_code: Some(200),
        content_type: Some("text/html; charset=utf-8".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 1,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response.clone(),
    });

    let output = parse_document(
        "<!doctype html><title>Example Domain</title><h1>Example Domain</h1>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: 1,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
        bytes_received: 63,
    });
    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 1,
        dom: Box::new(output.document),
    });

    assert_eq!(tab.page.base_url.as_deref(), Some(final_url.as_str()));
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Document parsed • HTTP 200"),
        "expected structured document status, got {:?}",
        tab.last_status
    );
}

#[test]
fn css_with_html_content_type_is_not_forwarded_to_css_runtime() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 7;

    let url = "https://example.com/site.css".to_string();
    let slot_id = tab.page.register_css(&url);

    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(404),
        content_type: Some("text/html".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response: response.clone(),
    });
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"<html>not css</html>".to_vec(),
    });
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
        bytes_received: 20,
    });

    let queued = rx.try_iter().collect::<Vec<_>>();
    assert!(
        queued
            .iter()
            .all(|cmd| !matches!(cmd, CoreCommand::CssChunk { .. })),
        "unexpected CSS chunks queued for HTML response: {queued:?}"
    );
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
        ),
        "expected CssDone to clear pending stylesheet state"
    );

    tab.on_css_sheet_done(slot_id, url);
    assert_eq!(tab.page.pending_count(), 0);
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Stylesheet ignored"),
        "expected ignored stylesheet status, got {:?}",
        tab.last_status
    );
}

#[test]
fn resource_limit_document_error_surfaces_status_and_stops_loading() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 3;

    let response = NetworkResponseInfo {
        requested_url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        status_code: Some(200),
        content_type: Some("text/html".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 3,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 3,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: "https://example.com".to_string(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "html response exceeded byte limit of 10485760 bytes".to_string(),
    });

    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Resource limit loading document"),
        "expected resource-limit document status, got {:?}",
        tab.last_status
    );
    assert!(!tab.loading, "document load should stop after limit error");
}

#[test]
fn stylesheet_resource_limit_aborts_partial_css_and_clears_pending_state() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 9;

    let url = "https://example.com/site.css".to_string();
    let slot_id = tab.page.register_css(&url);

    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(200),
        content_type: Some("text/css".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"body { color: red; }".to_vec(),
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit of 2097152 bytes".to_string(),
    });

    let queued = rx.try_iter().collect::<Vec<_>>();
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssChunk { url: chunk_url, .. } if chunk_url == &url)
        ),
        "expected partial CSS chunks to be buffered before the limit-triggered abort"
    );
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssAbort { url: abort_url, .. } if abort_url == &url)
        ),
        "expected CssAbort to discard buffered stylesheet state"
    );
    assert!(
        queued.iter().all(
            |cmd| !matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
        ),
        "unexpected CssDone on stylesheet limit failure: {queued:?}"
    );
    assert_eq!(tab.page.pending_count(), 0);
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Resource limit loading stylesheet"),
        "expected stylesheet limit status, got {:?}",
        tab.last_status
    );
}

#[test]
fn inline_styles_are_attached_and_computed_during_initial_document_load() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 11;
    tab.page.start_nav("https://example.com/");

    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 11,
        dom: Box::new(output.document),
    });

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(p.style.color(), (255, 0, 0, 255));
    assert!(
        find_dom_element(dom, "p")
            .and_then(|node| match node {
                Node::Element { style, .. } => Some(style),
                _ => None,
            })
            .is_some_and(Vec::is_empty),
        "structured runtime style resolution must not write legacy Node::style"
    );
}

#[test]
fn split_text_style_element_is_concatenated_without_synthetic_newlines() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 15;
    tab.page.start_nav("https://example.com/");
    let dom = Box::new(Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![Node::Element {
            id: Id(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![
                Node::Element {
                    id: Id(3),
                    name: Arc::from("head"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Element {
                        id: Id(4),
                        name: Arc::from("style"),
                        attributes: Vec::new(),
                        style: Vec::new(),
                        children: vec![
                            Node::Text {
                                id: Id(5),
                                text: "p { co".to_string(),
                            },
                            Node::Text {
                                id: Id(6),
                                text: "lor: red; }".to_string(),
                            },
                        ],
                    }],
                },
                Node::Element {
                    id: Id(7),
                    name: Arc::from("body"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Element {
                        id: Id(8),
                        name: Arc::from("p"),
                        attributes: Vec::new(),
                        style: Vec::new(),
                        children: vec![Node::Text {
                            id: Id(9),
                            text: "Hello".to_string(),
                        }],
                    }],
                },
            ],
        }],
    });

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 15,
        dom,
    });

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(
        p.style.color(),
        (255, 0, 0, 255),
        "style text node boundaries must not inject CSS tokenization whitespace"
    );
}

#[test]
fn repeated_dom_updates_reconcile_inline_styles_without_duplicate_slots() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 14;
    tab.page.start_nav("https://example.com/");
    let html = "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>";

    for _ in 0..2 {
        let output = parse_document(html, HtmlParseOptions::default()).expect("parse succeeds");
        tab.on_core_event(CoreEvent::DomUpdate {
            tab_id: tab.tab_id,
            request_id: 14,
            dom: Box::new(output.document),
        });
    }

    assert_eq!(
        tab.page.css_stylesheets().len(),
        1,
        "repeated equivalent DOM updates must reconcile inline stylesheet slots"
    );

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(p.style.color(), (255, 0, 0, 255));
}

#[test]
fn external_stylesheets_keep_document_order_when_network_arrives_out_of_order() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 12;
    tab.page.start_nav("https://example.com/index.html");

    let output = parse_document(
        "<!doctype html><html><head>\
         <link rel=\"stylesheet\" href=\"a.css\">\
         <style>p { color: red; }</style>\
         <link rel=\"stylesheet\" href=\"b.css\">\
         </head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 12,
        dom: Box::new(output.document),
    });

    let queued = rx.try_iter().collect::<Vec<_>>();
    let mut css_fetches = queued
        .iter()
        .filter_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot_id),
                url,
                kind: ResourceKind::Css,
                ..
            } => Some((*slot_id, url.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    css_fetches.sort_by_key(|(_, url)| url.clone());

    let (a_slot, a_url) = css_fetches
        .iter()
        .find(|(_, url)| url.ends_with("/a.css"))
        .cloned()
        .expect("a.css fetch");
    let (b_slot, b_url) = css_fetches
        .iter()
        .find(|(_, url)| url.ends_with("/b.css"))
        .cloned()
        .expect("b.css fetch");

    assert_ne!(a_slot, b_slot, "distinct stylesheet slots are required");

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 12,
        stylesheet_slot_id: b_slot,
        url: b_url.clone(),
        css_block: "p { color: blue; }".to_string(),
    });
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 12,
        stylesheet_slot_id: a_slot,
        url: a_url.clone(),
        css_block: "p { color: green; }".to_string(),
    });

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(
        p.style.color(),
        (0, 0, 255, 255),
        "b.css must win because document order is a.css, inline style, b.css"
    );
}

#[test]
fn duplicate_same_url_stylesheets_keep_distinct_document_slots() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 13;
    tab.page.start_nav("https://example.com/index.html");

    let output = parse_document(
        "<!doctype html><html><head>\
         <link rel=\"stylesheet\" href=\"theme.css\">\
         <link rel=\"stylesheet\" href=\"theme.css\">\
         </head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 13,
        dom: Box::new(output.document),
    });

    let slots = rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot_id),
                url,
                kind: ResourceKind::Css,
                ..
            } if url.ends_with("/theme.css") => Some(slot_id),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(slots.len(), 2, "duplicate links create duplicate slots");
    assert_ne!(slots[0], slots[1], "duplicate URL slots must not collapse");
}

#[test]
fn duplicate_same_url_stylesheets_participate_as_separate_cascade_slots() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 16;
    tab.page.start_nav("https://example.com/index.html");

    let output = parse_document(
        "<!doctype html><html><head>\
         <link rel=\"stylesheet\" href=\"theme.css\">\
         <link rel=\"stylesheet\" href=\"theme.css\">\
         </head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 16,
        dom: Box::new(output.document),
    });

    let slots = rx
        .try_iter()
        .filter_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot_id),
                url,
                kind: ResourceKind::Css,
                ..
            } if url.ends_with("/theme.css") => Some((slot_id, url)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(slots.len(), 2, "duplicate links create duplicate slots");
    let (first_slot, url) = slots[0].clone();
    let (second_slot, _) = slots[1].clone();

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 16,
        stylesheet_slot_id: first_slot,
        url: url.clone(),
        css_block: "p { color: red; }".to_string(),
    });
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 16,
        stylesheet_slot_id: second_slot,
        url,
        css_block: "p { color: blue; }".to_string(),
    });

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(
        p.style.color(),
        (0, 0, 255, 255),
        "second same-URL document slot must independently win by source order"
    );
}

#[test]
fn decoded_css_for_removed_stylesheet_slot_is_ignored() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 17;
    tab.page.start_nav("https://example.com/index.html");

    let with_link = parse_document(
        "<!doctype html><html><head>\
         <link rel=\"stylesheet\" href=\"removed.css\">\
         </head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");
    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 17,
        dom: Box::new(with_link.document),
    });

    let (removed_slot, removed_url) = rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot_id),
                url,
                kind: ResourceKind::Css,
                ..
            } if url.ends_with("/removed.css") => Some((slot_id, url)),
            _ => None,
        })
        .expect("removed.css fetch");

    let without_link = parse_document(
        "<!doctype html><html><head></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");
    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 17,
        dom: Box::new(without_link.document),
    });

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 17,
        stylesheet_slot_id: removed_slot,
        url: removed_url,
        css_block: "p { color: red; }".to_string(),
    });

    assert!(
        tab.page.css_stylesheets().is_empty(),
        "decoded CSS for removed slots must not attach to the active style set"
    );

    let dom = tab.page.dom.as_deref().expect("dom installed");
    let styled = build_style_tree_with_stylesheets(dom, tab.page.css_stylesheets())
        .expect("structured style tree should build");
    let p = find_styled_element(&styled, "p").expect("p styled node");

    assert_eq!(p.style.color(), (0, 0, 0, 255));
}

#[test]
fn decoded_css_for_aborted_stylesheet_slot_is_ignored() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 18;
    tab.page.start_nav("https://example.com/index.html");

    let url = "https://example.com/aborted.css".to_string();
    let slot_id = tab.page.register_css(&url);
    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(200),
        content_type: Some("text/css".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit".to_string(),
    });

    assert!(
        rx.try_iter().any(
            |cmd| matches!(cmd, CoreCommand::CssAbort { stylesheet_slot_id: abort_slot, .. } if abort_slot == slot_id)
        ),
        "network failure should abort the css runtime buffer"
    );

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: slot_id,
        url,
        css_block: "p { color: red; }".to_string(),
    });

    assert!(
        tab.page.css_stylesheets().is_empty(),
        "late decoded CSS for an aborted slot must not attach"
    );
}

fn find_styled_element<'a>(node: &'a StyledNode<'a>, want: &str) -> Option<&'a StyledNode<'a>> {
    if let Node::Element { name, .. } = node.node
        && name.as_ref() == want
    {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_element(child, want))
}

fn find_dom_element<'a>(node: &'a Node, want: &str) -> Option<&'a Node> {
    match node {
        Node::Element { name, children, .. } => {
            if name.as_ref() == want {
                return Some(node);
            }
            children
                .iter()
                .find_map(|child| find_dom_element(child, want))
        }
        Node::Document { children, .. } => children
            .iter()
            .find_map(|child| find_dom_element(child, want)),
        Node::Text { .. } | Node::Comment { .. } => None,
    }
}
