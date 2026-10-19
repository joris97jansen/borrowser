use super::super::{FrameObservationError, PageFrameStatus, Tab};
use super::support::no_quirks_patch_publication_from_output;
use bus::{CoreEvent, HtmlParseCompletion, HtmlParseFailure};
use core_types::DomVersion;
fn published() -> (Tab, HtmlParseCompletion) {
    let mut tab = Tab::new(1);
    tab.navigate_to_new("https://example.com/ag5".into());
    let parsed = html::parse_document(
        "<!doctype html><div id='x' style='color:#123456;width:32px;height:16px'></div>",
        html::HtmlParseOptions::default(),
    )
    .unwrap();
    let publication = no_quirks_patch_publication_from_output(parsed);
    let completion = HtmlParseCompletion {
        handle: publication.handle,
        version: DomVersion(1),
        document_mode: publication.document_mode,
    };
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.nav_gen,
        publication,
    })
    .unwrap();
    (tab, completion)
}
fn completed() -> Tab {
    let (mut tab, completion) = published();
    finish(&mut tab, completion).unwrap();
    tab
}
fn finish(
    tab: &mut Tab,
    completion: HtmlParseCompletion,
) -> Result<(), bus::DocumentPublicationFailure> {
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Ok(completion),
    })
}
fn input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640.0, 480.0),
        )),
        ..Default::default()
    }
}
#[test]
fn observation_requires_committed_publication_and_valid_completion() {
    let ctx = egui::Context::default();
    let (mut tab, mut completion) = published();
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
    completion.version = DomVersion(2);
    assert!(finish(&mut tab, completion).is_err());
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
    let (mut tab, _) = published();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Err(HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
    // A correctly parsed tree with no Browser commit is never authoritative.
    let mut tab = Tab::new(1);
    tab.page.dom = Some(Box::new(
        html::parse_document(
            "<!doctype html><div>correct parser output</div>",
            html::HtmlParseOptions::default(),
        )
        .unwrap()
        .document,
    ));
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
}
#[test]
fn observation_publication_failure_latches_even_with_existing_dom() {
    let (mut tab, completion) = published();
    let parsed = html::parse_document(
        "<!doctype html><div>replacement</div>",
        html::HtmlParseOptions::default(),
    )
    .unwrap();
    let mut publication = no_quirks_patch_publication_from_output(parsed);
    publication.handle = completion.handle;
    // This second patch incorrectly starts at INITIAL, so commit must fail.
    assert!(
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: tab.nav_gen,
            publication
        })
        .is_err()
    );
    assert!(finish(&mut tab, completion).is_err());
    assert!(matches!(
        tab.render_observed_frame(&egui::Context::default(), input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
}
#[test]
fn observation_uses_actual_frame_and_reuses_without_extra_computation() {
    let mut tab = completed();
    let ctx = egui::Context::default();
    let frame = tab.render_observed_frame(&ctx, input()).unwrap();
    let document_id = frame.document().id();
    assert!(!frame.element_styles().is_empty());
    let key = frame.layout().key();
    assert_eq!(frame.layout_viewport_width(), 640.0);
    assert_eq!(frame.layout().source_viewport_width(), 640.0);
    drop(frame);
    assert_eq!(document_id, tab.page.dom.as_deref().unwrap().id());
    let first = tab.page.render_pipeline_debug_snapshot();
    let frame = tab.render_observed_frame(&ctx, input()).unwrap();
    assert_eq!(frame.layout().key(), key);
    drop(frame);
    let second = tab.page.render_pipeline_debug_snapshot();
    assert_eq!(
        first.layout_artifacts.stats.recompute_count,
        second.layout_artifacts.stats.recompute_count
    );
    assert_eq!(
        first.style_artifacts.stats.recompute_count,
        second.style_artifacts.stats.recompute_count
    );
    assert_eq!(
        first.paint_artifacts.stats.recompute_count,
        second.paint_artifacts.stats.recompute_count
    );
}
#[test]
fn observation_rejects_stale_or_mismatched_artifacts() {
    let mut tab = completed();
    let ctx = egui::Context::default();
    let mut draft = None;
    ctx.begin_pass(input());
    assert_eq!(
        tab.ui_content_inner(&ctx, Some(&mut draft)).unwrap(),
        PageFrameStatus::Rendered
    );
    let _ = ctx.end_pass();
    let mut draft = draft.unwrap();
    assert!(tab.page.accepts_observed_artifacts(&draft));
    draft.style_key.style_input_generation += 1;
    assert!(!tab.page.accepts_observed_artifacts(&draft));
    draft.style_key.style_input_generation -= 1;
    draft.paint_key.paint_input_generation += 1;
    assert!(!tab.page.accepts_observed_artifacts(&draft));
    draft.paint_key.paint_input_generation -= 1;
    let other = completed();
    assert!(!other.page.accepts_observed_artifacts(&draft));
    tab.navigate_to_new("https://example.com/other".into());
    assert!(!tab.page.accepts_observed_artifacts(&draft));
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::IncompleteDocument)
    ));
}
#[test]
fn observation_discard_cannot_produce_an_accepted_frame() {
    let mut tab = completed();
    let ctx = egui::Context::default();
    ctx.on_end_pass(
        "AG5 discard regression",
        std::sync::Arc::new(|ctx| ctx.request_discard("test discard")),
    );
    assert!(matches!(
        tab.render_observed_frame(&ctx, input()),
        Err(FrameObservationError::Discarded)
    ));
}

#[test]
fn observation_followup_or_missing_frame_rejects_even_complete_artifacts() {
    let mut tab = completed();
    let ctx = egui::Context::default();
    let frame = tab.render_observed_frame(&ctx, input()).unwrap();
    for (status, pending) in [
        (PageFrameStatus::FollowupRequired, false),
        (PageFrameStatus::NoDocument, false),
        (PageFrameStatus::Rendered, true),
    ] {
        assert_eq!(
            super::super::observation::validate_frame_output(status, pending, frame.output()),
            Err(FrameObservationError::FrameNotAccepted)
        );
    }
}
