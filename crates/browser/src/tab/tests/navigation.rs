use super::super::Tab;
use crate::rendering::{RenderInvalidationEntryPoint, render_invalidation_request};
use bus::CoreEvent;
use core_types::{NetworkResponseInfo, ResourceKind};
use egui::Context;
use html::{HtmlParseOptions, parse_document};

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
fn starting_new_navigation_clears_pending_render_work_and_last_trace() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 32;
    tab.page.start_nav("https://example.com/");

    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 32,
        dom: Box::new(output.document),
    });

    let ctx = Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| tab.ui_content(ctx));

    assert!(tab.pending_render_work.is_empty());
    assert!(
        tab.last_render_trace.is_some(),
        "a completed frame should retain the last orchestration trace"
    );

    tab.request_render_work(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    assert!(!tab.pending_render_work.is_empty());

    tab.navigate_to_new("next.example.com/page".to_string());

    assert!(tab.pending_render_work.is_empty());
    assert!(tab.last_render_trace.is_none());
}
