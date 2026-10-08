use super::super::Tab;
use super::support::no_quirks_patch_publication_from_output;
use crate::rendering::{
    RenderInvalidationEntryPoint, RenderPhaseExecutionKind, RenderRebuildTrigger, RenderingPhase,
};
use bus::CoreEvent;
use egui::Context;
use html::{HtmlParseOptions, parse_document};

#[test]
fn ui_content_consumes_pending_render_work_through_explicit_orchestration_path() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 31;
    tab.page.start_nav("https://example.com/");

    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 31,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();

    assert_eq!(
        tab.pending_render_work
            .requests()
            .iter()
            .map(|request| request.entry_point())
            .collect::<Vec<_>>(),
        vec![
            RenderInvalidationEntryPoint::StylesheetSetChanged,
            RenderInvalidationEntryPoint::DocumentReplaced,
            RenderInvalidationEntryPoint::DomStructureChanged,
            RenderInvalidationEntryPoint::DomPublicationStyleInvalidated,
        ]
    );

    let ctx = Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        tab.ui_content(ctx).unwrap();
    });

    assert!(tab.pending_render_work.is_empty());
    let trace = tab
        .last_render_trace
        .as_ref()
        .expect("ui frame should store an orchestration trace");
    assert!(
        trace
            .triggered_entry_points
            .contains(&RenderInvalidationEntryPoint::DocumentReplaced)
    );
    assert_eq!(trace.style.kind, RenderPhaseExecutionKind::Requested);
    assert_eq!(
        trace.style.direct_triggers,
        vec![
            RenderRebuildTrigger::StylesheetSetChanged,
            RenderRebuildTrigger::DomPublicationStyleInvalidated,
        ]
    );
    assert_eq!(trace.layout.kind, RenderPhaseExecutionKind::Requested);
    assert!(trace.layout.cascaded_from.contains(&RenderingPhase::Style));
    assert_eq!(
        trace.semantic_phase_order,
        vec![
            RenderingPhase::Style,
            RenderingPhase::Layout,
            RenderingPhase::Paint,
        ]
    );
}

#[test]
fn frame_result_exposes_absent_document_and_style_failure() {
    use crate::tab::PageFrameStatus;
    let ctx = Context::default();
    let mut tab = Tab::new(1);
    ctx.begin_pass(egui::RawInput::default());
    assert_eq!(tab.ui_content(&ctx).unwrap(), PageFrameStatus::NoDocument);
    let _ = ctx.end_pass();
    // Deliberately violate the production publication invariant in this private
    // Browser regression test; no such initialization surface is exported.
    tab.page.dom = Some(Box::new(
        html::parse_document("<p>ok", HtmlParseOptions::default())
            .unwrap()
            .document,
    ));
    ctx.begin_pass(egui::RawInput::default());
    assert!(matches!(
        tab.ui_content(&ctx),
        Err(css::ComputedStyleResolutionError::MissingMatchingEnvironment)
    ));
    let _ = ctx.end_pass();
    assert!(tab.last_render_trace.is_none());
}
