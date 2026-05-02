use crate::page::{PageState, RestyleHint};
use crate::rendering::*;
use html::{HtmlParseOptions, parse_document};

use super::support::*;

#[test]
fn render_invalidation_request_contracts_pin_runtime_entry_points() {
    let contracts = render_invalidation_request_contracts();
    assert_eq!(contracts.len(), 8);

    let attrs = render_invalidation_request(RenderInvalidationEntryPoint::DomAttributesChanged);
    assert_eq!(attrs.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        attrs.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomAttributesChanged)
    );
    assert_eq!(
        attrs.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        attrs.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );

    let text = render_invalidation_request(RenderInvalidationEntryPoint::DomTextChanged);
    assert_eq!(text.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(text.work.style, PhaseRerunSource::None);
    assert_eq!(
        text.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
    assert_eq!(
        text.work.frame_orchestration,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );

    let input = render_invalidation_request(RenderInvalidationEntryPoint::InputStateChanged);
    assert_eq!(input.requested_by, RenderingSubsystem::BrowserView);
    assert_eq!(input.work.style, PhaseRerunSource::None);
    assert_eq!(input.work.layout, PhaseRerunSource::None);
    assert_eq!(
        input.work.paint,
        PhaseRerunSource::Direct(RenderRebuildTrigger::InputStateChanged)
    );

    let resource = render_invalidation_request(RenderInvalidationEntryPoint::ResourceStateChanged);
    assert_eq!(resource.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        resource.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged)
    );
    assert_eq!(
        resource.work.paint,
        PhaseRerunSource::Direct(RenderRebuildTrigger::ResourceStateChanged)
    );
}

#[test]
fn render_invalidation_request_contracts_cover_each_entry_point_once() {
    let contracts = render_invalidation_request_contracts();
    let expected = [
        RenderInvalidationEntryPoint::DocumentReplaced,
        RenderInvalidationEntryPoint::DomStructureChanged,
        RenderInvalidationEntryPoint::DomAttributesChanged,
        RenderInvalidationEntryPoint::DomTextChanged,
        RenderInvalidationEntryPoint::StylesheetSetChanged,
        RenderInvalidationEntryPoint::ViewportChanged,
        RenderInvalidationEntryPoint::ResourceStateChanged,
        RenderInvalidationEntryPoint::InputStateChanged,
    ];

    for entry_point in expected {
        let count = contracts
            .iter()
            .filter(|contract| contract.entry_point == entry_point)
            .count();
        assert_eq!(
            count, 1,
            "entry point must have exactly one invalidation contract: {entry_point:?}"
        );
    }

    assert_eq!(contracts.len(), expected.len());
}

#[test]
fn direct_invalidation_phase_sources_align_with_phase_rebuild_triggers() {
    let phase_contracts = render_phase_contracts();

    for request in render_invalidation_request_contracts() {
        assert!(
            request.work.requests_redraw(),
            "every shipped invalidation entry point should request a frame: {:?}",
            request.entry_point
        );

        for (phase, source) in [
            (RenderingPhase::Style, request.work.style),
            (RenderingPhase::Layout, request.work.layout),
            (RenderingPhase::Paint, request.work.paint),
            (
                RenderingPhase::FrameOrchestration,
                request.work.frame_orchestration,
            ),
        ] {
            if let PhaseRerunSource::Direct(trigger) = source {
                let contract = phase_contracts
                    .iter()
                    .find(|contract| contract.phase == phase)
                    .expect("phase contract should exist");
                assert!(
                    contract.rebuild_triggers.contains(&trigger),
                    "direct invalidation trigger {trigger:?} must be listed on {phase:?}"
                );
            }
        }
    }
}

#[test]
fn pending_render_work_deduplicates_and_preserves_request_order() {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));

    assert_eq!(
        pending
            .requests()
            .iter()
            .map(|request| request.entry_point)
            .collect::<Vec<_>>(),
        vec![
            RenderInvalidationEntryPoint::DocumentReplaced,
            RenderInvalidationEntryPoint::ResourceStateChanged,
        ]
    );
}

#[test]
fn document_replacement_returns_explicit_full_pipeline_work_request() {
    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should work");
    let mut page = PageState::new();
    page.start_nav("https://example.com/index.html");

    let request = page.replace_dom(Box::new(output.document), RestyleHint::document_replaced());
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::DocumentReplaced
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        request.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomReplaced)
    );
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(
        request.work.frame_orchestration,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
}

#[test]
fn dom_text_mutation_returns_explicit_layout_and_paint_work_request() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    page.clear_layout_dirty_for_tests();

    let request = page.mark_dom_changed(RestyleHint::text_mutated());
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::DomTextChanged
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(request.work.style, PhaseRerunSource::None);
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(
        request.work.frame_orchestration,
        PhaseRerunSource::Direct(RenderRebuildTrigger::DomTextChanged)
    );
}

#[test]
fn stylesheet_reconcile_returns_explicit_style_invalidation_request() {
    let output = parse_document(
        "<!doctype html><html><head><link rel=\"stylesheet\" href=\"https://example.com/site.css\"></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should work");
    let mut page = PageState::new();
    page.start_nav("https://example.com/index.html");
    let _ = page.replace_dom(Box::new(output.document), RestyleHint::document_replaced());

    let outcome = page.reconcile_document_stylesheets();
    let request = outcome
        .render_invalidation
        .expect("stylesheet discovery should invalidate style inputs");
    assert_eq!(
        request.entry_point,
        RenderInvalidationEntryPoint::StylesheetSetChanged
    );
    assert_eq!(request.requested_by, RenderingSubsystem::BrowserRuntime);
    assert_eq!(
        request.work.style,
        PhaseRerunSource::Direct(RenderRebuildTrigger::StylesheetSetChanged)
    );
    assert_eq!(
        request.work.layout,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Style)
    );
    assert_eq!(
        request.work.paint,
        PhaseRerunSource::CascadedFrom(RenderingPhase::Layout)
    );
    assert_eq!(outcome.fetches.len(), 1);
}
