use super::super::Tab;
use super::support::{current_element_color, find_dom_element, find_styled_element};
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;
use css::build_style_tree_with_stylesheets;
use html::{HtmlParseOptions, Node, internal::Id, parse_document};
use std::sync::mpsc;

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
                Node::Element { element } => Some(element.style()),
                _ => None,
            })
            .is_some_and(<[_]>::is_empty),
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
        children: vec![html::internal::node_element_from_parts(
            Id(2),
            html::internal::html_name("html"),
            Vec::new(),
            Vec::new(),
            vec![
                html::internal::node_element_from_parts(
                    Id(3),
                    html::internal::html_name("head"),
                    Vec::new(),
                    Vec::new(),
                    vec![html::internal::node_element_from_parts(
                        Id(4),
                        html::internal::html_name("style"),
                        Vec::new(),
                        Vec::new(),
                        vec![
                            Node::Text {
                                id: Id(5),
                                text: "p { co".to_string(),
                            },
                            Node::Text {
                                id: Id(6),
                                text: "lor: red; }".to_string(),
                            },
                        ],
                    )],
                ),
                html::internal::node_element_from_parts(
                    Id(7),
                    html::internal::html_name("body"),
                    Vec::new(),
                    Vec::new(),
                    vec![html::internal::node_element_from_parts(
                        Id(8),
                        html::internal::html_name("p"),
                        Vec::new(),
                        Vec::new(),
                        vec![Node::Text {
                            id: Id(9),
                            text: "Hello".to_string(),
                        }],
                    )],
                ),
            ],
        )],
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
