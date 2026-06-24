use crate::input_state::DocumentInputState;
use crate::page::{PageState, RestyleHint};
use crate::rendering::*;
use crate::resources::ResourceManager;
use egui::{CentralPanel, Context, Pos2, RawInput, Rect, Vec2};

use super::support::*;

const DEFAULT_VIEWPORT_WIDTH: f32 = 640.0;
const DEFAULT_VIEWPORT_HEIGHT: f32 = 480.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuardCounters {
    style_reuse: u64,
    style_recompute: u64,
    style_discard: u64,
    layout_reuse: u64,
    layout_recompute: u64,
    layout_discard: u64,
    paint_reuse: u64,
    paint_recompute: u64,
    paint_discard: u64,
    retained_identities: usize,
    dirty_entries: usize,
}

impl GuardCounters {
    fn from_page(page: &PageState) -> Self {
        let snapshot = page.retained_render_state_debug_snapshot();
        Self {
            style_reuse: snapshot.style_artifacts.stats.reuse_count,
            style_recompute: snapshot.style_artifacts.stats.recompute_count,
            style_discard: snapshot.style_artifacts.stats.discard_count,
            layout_reuse: snapshot.layout_artifacts.stats.reuse_count,
            layout_recompute: snapshot.layout_artifacts.stats.recompute_count,
            layout_discard: snapshot.layout_artifacts.stats.discard_count,
            paint_reuse: snapshot.paint_artifacts.stats.reuse_count,
            paint_recompute: snapshot.paint_artifacts.stats.recompute_count,
            paint_discard: snapshot.paint_artifacts.stats.discard_count,
            retained_identities: snapshot.retained_identities.len(),
            dirty_entries: snapshot.dirty_state.entries.len(),
        }
    }
}

struct FrameHarness {
    input_state: DocumentInputState,
    resources: ResourceManager,
}

impl FrameHarness {
    fn new() -> Self {
        Self {
            input_state: DocumentInputState::new(),
            resources: ResourceManager::new(),
        }
    }

    fn execute_and_record(
        &mut self,
        page: &mut PageState,
        pending_work: PendingRenderWork,
        viewport_width: f32,
    ) -> RenderFrameExecutionTrace {
        let prepared = prepare_page_frame(page, pending_work)
            .expect("frame preparation should succeed")
            .expect("document should produce a frame");
        let ctx = Context::default();
        let mut prepared = Some(prepared);
        let mut outcome = None;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::new(0.0, 0.0),
                    Vec2::new(viewport_width, DEFAULT_VIEWPORT_HEIGHT),
                )),
                ..RawInput::default()
            },
            |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    outcome = Some(execute_prepared_page_frame(
                        ui,
                        prepared.take().expect("prepared frame should execute once"),
                        &mut self.input_state,
                        &self.resources,
                    ));
                });
            },
        );
        let mut outcome = outcome.expect("frame should execute");
        if let Some(result) = outcome.retained_layout_result.take() {
            page.record_layout_frame_result(result);
        }
        if let Some(result) = outcome.retained_paint_result.take() {
            page.record_paint_frame_result(result);
        }
        outcome.trace
    }
}

fn execute_initial_frame(page: &mut PageState) -> (FrameHarness, GuardCounters) {
    let mut harness = FrameHarness::new();
    harness.execute_and_record(page, PendingRenderWork::default(), DEFAULT_VIEWPORT_WIDTH);
    assert_clean_after_recorded_frame(page);
    let baseline = GuardCounters::from_page(page);
    assert!(baseline.style_recompute > 0);
    assert!(baseline.layout_recompute > 0);
    assert!(baseline.paint_recompute > 0);
    (harness, baseline)
}

fn assert_clean_after_recorded_frame(page: &PageState) {
    let snapshot = page.retained_render_state_debug_snapshot();
    assert!(
        snapshot.dirty_state.entries.is_empty(),
        "recorded frame should leave no retained dirty entries: {:?}",
        snapshot.dirty_state.entries
    );
    assert!(!snapshot.style_dirty);
    assert!(!snapshot.layout_dirty);
    assert!(!snapshot.paint_dirty);
    assert_eq!(
        snapshot.style_artifacts.state,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        snapshot.layout_artifacts.state,
        RenderArtifactState::RetainedFresh
    );
    assert_eq!(
        snapshot.paint_artifacts.state,
        RenderArtifactState::RetainedFresh
    );
}

fn empty_pending_work() -> PendingRenderWork {
    PendingRenderWork::default()
}

fn pending_work_for(entry_point: RenderInvalidationEntryPoint) -> PendingRenderWork {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(entry_point));
    pending
}

fn baseline_page(html: &str) -> (PageState, FrameHarness, GuardCounters) {
    let mut page = page_with_dom(html);
    let (harness, baseline) = execute_initial_frame(&mut page);
    (page, harness, baseline)
}

#[test]
fn ac9_initial_render_records_retained_work_baseline() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let before = GuardCounters::from_page(&page);
    let (_harness, baseline) = execute_initial_frame(&mut page);

    assert!(baseline.style_recompute > before.style_recompute);
    assert!(baseline.layout_recompute > before.layout_recompute);
    assert!(baseline.paint_recompute > before.paint_recompute);
    assert_eq!(baseline.dirty_entries, 0);
    assert!(page.retained_render_state_debug_snapshot().has_dom);
    assert_eq!(baseline.style_reuse, before.style_reuse);
    assert_eq!(baseline.layout_reuse, before.layout_reuse);
    assert_eq!(baseline.paint_reuse, before.paint_reuse);
}

#[test]
fn ac9_noop_repeated_render_reuses_retained_artifacts_without_recompute_growth() {
    let (mut page, mut harness, baseline) = baseline_page(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    let trace = harness.execute_and_record(&mut page, empty_pending_work(), DEFAULT_VIEWPORT_WIDTH);
    assert_clean_after_recorded_frame(&page);
    let after = GuardCounters::from_page(&page);

    assert_eq!(after.style_recompute, baseline.style_recompute);
    assert_eq!(after.layout_recompute, baseline.layout_recompute);
    assert_eq!(after.paint_recompute, baseline.paint_recompute);
    assert!(after.style_reuse > baseline.style_reuse);
    assert!(after.layout_reuse > baseline.layout_reuse);
    assert!(after.paint_reuse > baseline.paint_reuse);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
    assert_eq!(
        trace.layout.kind,
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    );
    assert_eq!(
        trace.paint.kind,
        RenderPhaseExecutionKind::MaterializedFromRetainedArtifacts
    );
}

#[test]
fn ac9_repeated_viewport_resize_does_not_restyle_or_grow_retained_state() {
    let (mut page, mut harness, baseline) = baseline_page(
        "<!doctype html><html><head><style>p { display: block; width: 100px; color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let widths = [560.0, 700.0, 600.0, 680.0, 640.0];

    for width in widths {
        let trace = harness.execute_and_record(&mut page, empty_pending_work(), width);
        assert_clean_after_recorded_frame(&page);
        assert!(
            trace
                .triggered_entry_points
                .contains(&RenderInvalidationEntryPoint::ViewportChanged)
        );
    }

    let after = GuardCounters::from_page(&page);
    assert_eq!(
        after.style_recompute, baseline.style_recompute,
        "viewport-only frames must not restyle by default"
    );
    assert!(after.style_reuse >= baseline.style_reuse + widths.len() as u64);
    assert!(after.layout_recompute <= baseline.layout_recompute + widths.len() as u64);
    assert!(after.paint_recompute <= baseline.paint_recompute + widths.len() as u64);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

#[test]
fn ac9_text_content_update_reuses_style_and_recomputes_layout_paint() {
    let (mut page, mut harness, baseline) = baseline_page(
        "<!doctype html><html><head><style>p { display: block; width: 100px; color: red; }</style></head><body><p>Hello</p></body></html>",
    );

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        "Hello",
        "Hello with more text",
    );
    page.mark_dom_changed_for_tests(RestyleHint::text_mutated());

    harness.execute_and_record(&mut page, empty_pending_work(), DEFAULT_VIEWPORT_WIDTH);
    assert_clean_after_recorded_frame(&page);
    let after = GuardCounters::from_page(&page);

    assert_eq!(after.style_recompute, baseline.style_recompute);
    assert!(after.style_reuse > baseline.style_reuse);
    assert!(after.layout_recompute > baseline.layout_recompute);
    assert!(after.paint_recompute > baseline.paint_recompute);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

#[test]
fn ac9_paint_only_style_update_reuses_layout_and_recomputes_paint() {
    let (mut page, mut harness, baseline) = baseline_page(
        "<!doctype html><html><head><style>.paint { background-color: red; }</style></head><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("page DOM should exist");
        set_first_element_attr(dom, "p", "class", Some("paint".to_string()))
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    harness.execute_and_record(&mut page, empty_pending_work(), DEFAULT_VIEWPORT_WIDTH);
    assert_clean_after_recorded_frame(&page);
    let after = GuardCounters::from_page(&page);

    assert!(after.style_recompute > baseline.style_recompute);
    assert_eq!(
        after.layout_recompute, baseline.layout_recompute,
        "CSS-owned paint-only impact classification should allow retained layout reuse"
    );
    assert!(after.layout_reuse > baseline.layout_reuse);
    assert!(after.paint_recompute > baseline.paint_recompute);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

#[test]
fn ac9_layout_affecting_style_update_recomputes_layout_and_paint() {
    let (mut page, mut harness, baseline) = baseline_page(
        "<!doctype html><html><body><p style=\"display: block; width: 100px;\">Hello</p></body></html>",
    );

    let dirty_id = {
        let dom = page.dom.as_deref_mut().expect("page DOM should exist");
        set_first_element_attr(
            dom,
            "p",
            "style",
            Some("display: block; width: 180px;".to_string()),
        )
    };
    page.mark_dom_changed_for_tests(RestyleHint::attributes_changed(vec![dirty_id]));

    harness.execute_and_record(&mut page, empty_pending_work(), DEFAULT_VIEWPORT_WIDTH);
    assert_clean_after_recorded_frame(&page);
    let after = GuardCounters::from_page(&page);

    assert!(after.style_recompute > baseline.style_recompute);
    assert!(after.layout_recompute > baseline.layout_recompute);
    assert!(after.paint_recompute > baseline.paint_recompute);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

#[test]
fn ac9_stylesheet_update_recomputes_style_with_bounded_downstream_work() {
    let initial_css = "p { display: block; width: 100px; color: red; }";
    let (mut page, mut harness, baseline) = baseline_page(&format!(
        "<!doctype html><html><head><style>{initial_css}</style></head><body><p>Hello</p></body></html>"
    ));

    replace_first_text(
        page.dom
            .as_deref_mut()
            .expect("page DOM should exist for mutation"),
        initial_css,
        "p { display: block; width: 140px; color: blue; }",
    );
    page.mark_dom_changed_for_tests(RestyleHint::text_mutated());
    let reconcile = page.reconcile_document_stylesheets();
    assert!(reconcile.render_invalidation.is_some());

    harness.execute_and_record(
        &mut page,
        pending_work_for(RenderInvalidationEntryPoint::StylesheetSetChanged),
        DEFAULT_VIEWPORT_WIDTH,
    );
    assert_clean_after_recorded_frame(&page);
    let after = GuardCounters::from_page(&page);

    assert!(after.style_recompute > baseline.style_recompute);
    assert!(after.layout_recompute > baseline.layout_recompute);
    assert!(after.paint_recompute > baseline.paint_recompute);
    assert!(after.style_discard >= baseline.style_discard);
    assert!(after.layout_discard >= baseline.layout_discard);
    assert!(after.paint_discard >= baseline.paint_discard);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

#[test]
fn ac9_representative_page_repeated_text_updates_have_bounded_resource_growth() {
    let (mut page, mut harness, baseline) = baseline_page(&representative_page_fixture());
    let mut from = "Open invoices: 14";
    let mut to = "Open invoices: 15";
    let updates = 8;

    for _ in 0..updates {
        replace_first_text(
            page.dom
                .as_deref_mut()
                .expect("page DOM should exist for mutation"),
            from,
            to,
        );
        page.mark_dom_changed_for_tests(RestyleHint::text_mutated());
        harness.execute_and_record(&mut page, empty_pending_work(), DEFAULT_VIEWPORT_WIDTH);
        assert_clean_after_recorded_frame(&page);
        std::mem::swap(&mut from, &mut to);
    }

    let after = GuardCounters::from_page(&page);
    assert_eq!(
        after.style_recompute, baseline.style_recompute,
        "representative text updates should not restyle in the current CSS model"
    );
    assert!(after.style_reuse >= baseline.style_reuse + updates);
    assert!(after.layout_recompute <= baseline.layout_recompute + updates);
    assert!(after.paint_recompute <= baseline.paint_recompute + updates);
    assert_eq!(after.retained_identities, baseline.retained_identities);
    assert_eq!(after.dirty_entries, 0);
}

fn representative_page_fixture() -> String {
    let cards = (0..12)
        .map(|index| {
            let status = if index % 3 == 0 {
                "warning"
            } else if index % 3 == 1 {
                "good"
            } else {
                "neutral"
            };
            format!(
                "<section class=\"card {status}\"><h2>Account {index}</h2><p>Open invoices: 14</p><p class=\"note\">Updated today</p></section>"
            )
        })
        .collect::<String>();

    format!(
        concat!(
            "<!doctype html><html><head><style>",
            "body {{ display: block; color: black; }}",
            ".dashboard {{ display: block; width: 480px; }}",
            ".card {{ display: block; width: 220px; background-color: white; color: black; }}",
            ".warning {{ background-color: red; }}",
            ".good {{ background-color: green; }}",
            ".neutral {{ background-color: blue; }}",
            ".note {{ color: gray; }}",
            "</style></head><body><main class=\"dashboard\">",
            "{cards}",
            "</main></body></html>"
        ),
        cards = cards
    )
}
