use super::Tab;
use crate::page::{RestyleTrigger, StyleRecalcKind};
use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion, NetworkResponseInfo, ResourceKind};
use css::{StyledNode, build_style_tree_with_stylesheets};
use html::{DomPatch, HtmlParseOptions, Node, PatchKey, internal::Id, parse_document};
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

#[test]
fn dom_patch_attribute_change_triggers_restyle_through_computed_cache() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 19;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(190);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 19,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
    });

    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::DocumentReplaced)
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    let after_initial = tab.page.style_generations();
    assert!(!tab.page.style_dirty());

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 19,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![(Arc::from("class"), Some("hot".to_string()))],
        }],
    });

    assert!(
        tab.page.style_dirty(),
        "attribute mutation must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::AttributesChanged)
    );
    assert_eq!(tab.page.style_generations().dom, after_initial.dom + 1);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 1,
        }),
        "attribute mutation on the last element should reuse the computed prefix"
    );
    assert!(
        !tab.page.style_dirty(),
        "style cache should be clean after recomputation"
    );
}

#[test]
fn dom_patch_node_insertion_triggers_restyle_for_inserted_subtree() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 20;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(200);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 20,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("span { color: blue; }", None),
    });

    assert!(
        current_element_color_optional(&mut tab, "span").is_none(),
        "initial document has no span"
    );

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 20,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![
            DomPatch::CreateElement {
                key: PatchKey(9),
                name: Arc::from("span"),
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(10),
                text: "Inserted".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(9),
                child: PatchKey(10),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(9),
            },
        ],
    });

    assert!(
        tab.page.style_dirty(),
        "node insertion must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TreeMutated)
    );
    assert_eq!(
        current_element_color(&mut tab, "span"),
        (0, 0, 255, 255),
        "inserted element should receive computed style from existing stylesheet"
    );
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 5 }),
        "structural mutations must not use suffix reuse while selector ids can shift"
    );
}

#[test]
fn dom_patch_node_removal_triggers_restyle_and_removes_styled_node() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 21;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(210);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 21,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 21,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::RemoveNode { key: PatchKey(7) }],
    });

    assert!(
        tab.page.style_dirty(),
        "node removal must mark style dirty before restyle"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TreeMutated)
    );
    assert!(
        current_element_color_optional(&mut tab, "p").is_none(),
        "removed element must not remain in the rebuilt styled tree"
    );
}

#[test]
fn dom_patch_style_text_change_reconciles_stylesheet_slot_and_restyles() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 22;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(220);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 22,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetText {
            key: PatchKey(5),
            text: "p { color: blue; }".to_string(),
        }],
    });

    let after = tab.page.style_generations();
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TextMutated)
    );
    assert_eq!(
        after.style_inputs, before.style_inputs,
        "style text changes should invalidate through stylesheet generation"
    );
    assert_eq!(
        after.stylesheets,
        before.stylesheets + 1,
        "style text mutation must update the document stylesheet generation"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
}

#[test]
fn dom_patch_attribute_change_incrementally_restyles_following_sibling_suffix() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 26;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(260);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: two_paragraph_patch_document(".hot ~ p { color: blue; } p { color: black; }"),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 26,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![(Arc::from("class"), Some("hot".to_string()))],
        }],
    });

    {
        let styled = tab
            .page
            .build_style_tree()
            .expect("style tree should build")
            .expect("document should be styled");
        assert_eq!(
            find_styled_node_id(&styled, Id(7))
                .expect("first paragraph")
                .style
                .color(),
            (0, 0, 0, 255)
        );
        assert_eq!(
            find_styled_node_id(&styled, Id(9))
                .expect("second paragraph")
                .style
                .color(),
            (0, 0, 255, 255),
            "suffix restyle must include following siblings affected by sibling selectors"
        );
    }
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 2,
        }),
        "first paragraph mutation should reuse html/head/style/body and recompute both paragraphs"
    );
}

#[test]
fn queued_attribute_mutations_merge_to_earliest_dirty_suffix() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 27;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(270);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: two_paragraph_patch_document(
            ".hot { color: red; } .cool { color: blue; } p { color: black; }",
        ),
    });

    assert_eq!(current_element_color_by_id(&mut tab, Id(7)), (0, 0, 0, 255));
    assert_eq!(current_element_color_by_id(&mut tab, Id(9)), (0, 0, 0, 255));

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![(Arc::from("class"), Some("hot".to_string()))],
        }],
    });
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 27,
        handle,
        from: DomVersion(2),
        to: DomVersion(3),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(9),
            attributes: vec![(Arc::from("class"), Some("cool".to_string()))],
        }],
    });

    {
        let styled = tab
            .page
            .build_style_tree()
            .expect("style tree should build")
            .expect("document should be styled");
        assert_eq!(
            find_styled_node_id(&styled, Id(7))
                .expect("first paragraph")
                .style
                .color(),
            (255, 0, 0, 255),
            "first queued attribute mutation must not be lost"
        );
        assert_eq!(
            find_styled_node_id(&styled, Id(9))
                .expect("second paragraph")
                .style
                .color(),
            (0, 0, 255, 255),
            "second queued attribute mutation must also apply"
        );
    }
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::IncrementalSuffix {
            reused_prefix_len: 4,
            recomputed_len: 2,
        }),
        "merged pending suffix must start at the earliest queued dirty element"
    );
}

#[test]
fn attribute_mutation_without_existing_style_cache_falls_back_to_full_recompute() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 28;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(280);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document(".hot { color: red; } p { color: black; }", Some("p")),
    });

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 28,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetAttributes {
            key: PatchKey(7),
            attributes: vec![(Arc::from("class"), Some("hot".to_string()))],
        }],
    });

    assert_eq!(
        current_element_color_by_id(&mut tab, Id(7)),
        (255, 0, 0, 255)
    );
    assert_eq!(
        tab.page.last_style_recalc(),
        Some(StyleRecalcKind::Full { elements: 5 }),
        "partial suffix reuse requires a validated previous style cache"
    );
}

#[test]
fn dom_patch_normal_text_change_dirties_layout_but_reuses_computed_style() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 23;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(230);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    tab.page.clear_layout_dirty_for_tests();
    let before = tab.page.style_generations();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 23,
        handle,
        from: DomVersion(1),
        to: DomVersion(2),
        patches: vec![DomPatch::SetText {
            key: PatchKey(8),
            text: "Goodbye".to_string(),
        }],
    });

    let after = tab.page.style_generations();
    assert_eq!(
        tab.page.last_restyle_trigger(),
        Some(RestyleTrigger::TextMutated)
    );
    assert_eq!(after.dom, before.dom + 1);
    assert_eq!(
        after.style_inputs, before.style_inputs,
        "normal text changes must not invalidate selector/cascade inputs"
    );
    assert_eq!(
        after.stylesheets, before.stylesheets,
        "normal text changes must not reconcile a new stylesheet set"
    );
    assert!(
        !tab.page.style_dirty(),
        "normal text changes should reuse cached computed style"
    );
    assert!(
        tab.page.layout_dirty(),
        "normal text changes still require downstream layout work"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}

#[test]
fn empty_dom_patch_batch_does_not_trigger_restyle() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 25;
    tab.page.start_nav("https://example.com/index.html");
    let handle = DomHandle(250);

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 25,
        handle,
        from: DomVersion::INITIAL,
        to: DomVersion(1),
        patches: initial_patch_document("p { color: red; }", Some("p")),
    });

    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
    let before = tab.page.style_generations();
    let previous_trigger = tab.page.last_restyle_trigger();

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 25,
        handle,
        from: DomVersion(1),
        to: DomVersion(1),
        patches: Vec::new(),
    });

    assert_eq!(
        tab.page.style_generations(),
        before,
        "empty patch batches must not advance DOM or style generations"
    );
    assert_eq!(
        tab.page.last_restyle_trigger(),
        previous_trigger,
        "empty patch batches must not record a synthetic restyle trigger"
    );
    assert!(
        !tab.page.style_dirty(),
        "empty patch batches must not invalidate cached computed style"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}

#[test]
fn external_stylesheet_arrival_invalidates_cached_computed_style() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 24;
    tab.page.start_nav("https://example.com/index.html");

    let output = parse_document(
        "<!doctype html><html><head>\
         <link rel=\"stylesheet\" href=\"site.css\">\
         </head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 24,
        dom: Box::new(output.document),
    });

    let (slot_id, url) = rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot_id),
                url,
                kind: ResourceKind::Css,
                ..
            } if url.ends_with("/site.css") => Some((slot_id, url)),
            _ => None,
        })
        .expect("site.css fetch");

    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 0, 255));
    let before = tab.page.style_generations();
    assert!(!tab.page.style_dirty());

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 24,
        stylesheet_slot_id: slot_id,
        url,
        css_block: "p { color: red; }".to_string(),
    });

    let after = tab.page.style_generations();
    assert_eq!(after.stylesheets, before.stylesheets + 1);
    assert!(
        tab.page.style_dirty(),
        "stylesheet arrival must invalidate cached computed style"
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(!tab.page.style_dirty());
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

fn current_element_color(tab: &mut Tab, name: &str) -> (u8, u8, u8, u8) {
    current_element_color_optional(tab, name).expect("styled element should exist")
}

fn current_element_color_optional(tab: &mut Tab, name: &str) -> Option<(u8, u8, u8, u8)> {
    let styled = tab
        .page
        .build_style_tree()
        .expect("style tree should build")?;
    find_styled_element(&styled, name).map(|node| node.style.color())
}

fn current_element_color_by_id(tab: &mut Tab, id: Id) -> (u8, u8, u8, u8) {
    let styled = tab
        .page
        .build_style_tree()
        .expect("style tree should build")
        .expect("document should be styled");
    find_styled_node_id(&styled, id)
        .map(|node| node.style.color())
        .expect("styled node should exist")
}

fn find_styled_node_id<'a>(node: &'a StyledNode<'a>, want: Id) -> Option<&'a StyledNode<'a>> {
    if node.node_id == want {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_node_id(child, want))
}

fn initial_patch_document(style_text: &str, body_element: Option<&str>) -> Vec<DomPatch> {
    let mut patches = vec![
        DomPatch::Clear,
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("head"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("style"),
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
            name: Arc::from("body"),
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
                name: Arc::from(name),
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

fn two_paragraph_patch_document(style_text: &str) -> Vec<DomPatch> {
    let mut patches = initial_patch_document(style_text, None);
    patches.extend([
        DomPatch::CreateElement {
            key: PatchKey(7),
            name: Arc::from("p"),
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
            name: Arc::from("p"),
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
