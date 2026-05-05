use super::super::Tab;
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

    tab.on_core_event(CoreEvent::DomUpdate {
        tab_id: tab.tab_id,
        request_id: 31,
        dom: Box::new(output.document),
    });

    assert_eq!(
        tab.pending_render_work
            .requests()
            .iter()
            .map(|request| request.entry_point)
            .collect::<Vec<_>>(),
        vec![
            RenderInvalidationEntryPoint::StylesheetSetChanged,
            RenderInvalidationEntryPoint::DocumentReplaced,
        ]
    );

    let ctx = Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| tab.ui_content(ctx));

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
            RenderRebuildTrigger::DomReplaced,
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
