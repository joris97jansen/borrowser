use super::super::Tab;
use super::support::no_quirks_patch_publication_from_output;
use crate::rendering::{RenderInvalidationEntryPoint, render_invalidation_request};
use bus::CoreEvent;
use core_types::{NetworkResponseInfo, ResourceKind};
use egui::Context;
use html::{HtmlParseOptions, parse_document};

#[test]
fn redirected_document_response_updates_tab_base_url_and_status() {
    let mut tab = Tab::new(1);
    let requested = "https://example.com".to_string();
    tab.navigate_to_new(requested.clone());
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
    })
    .unwrap();

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
    })
    .unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 1,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();

    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: 1,
        result: Ok(bus::HtmlParseCompletion {
            handle: tab.dom_handle.unwrap(),
            version: tab.dom_version,
            document_mode: tab.page.document_mode.unwrap(),
        }),
    })
    .unwrap();
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

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 32,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();

    let ctx = Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        tab.ui_content(ctx).unwrap();
    });

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
