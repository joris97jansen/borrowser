use super::super::{FrameObservationError, PageFrameStatus, Tab};
use super::support::no_quirks_patch_publication_from_output;
use bus::{CoreEvent, HtmlParseCompletion, HtmlParseFailure};
use core_types::DomVersion;
fn published() -> (
    Tab,
    HtmlParseCompletion,
    std::sync::mpsc::Receiver<bus::CoreCommand>,
) {
    let mut tab = Tab::new(1);
    let (sender, commands) = std::sync::mpsc::channel();
    tab.set_bus_sender(sender);
    tab.navigate_to_new("https://example.com/ag5".into())
        .unwrap();
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: super::support::pending_id(&tab),
        stylesheet_slot_id: None,
        kind: core_types::ResourceKind::Html,
        response: super::support::response_facts(
            addressing::ResourceKind::Document,
            tab.url.clone(),
            tab.url.clone(),
            Some(200),
            Some("text/html".into()),
        ),
    })
    .unwrap();
    let markup = "<!doctype html><div id='x' style='color:#123456;width:32px;height:16px'></div>";
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        stylesheet_slot_id: None,
        kind: core_types::ResourceKind::Html,
        url: tab
            .document_load
            .response
            .as_ref()
            .unwrap()
            .requested_url()
            .into(),
        bytes: markup.as_bytes().to_vec(),
    })
    .unwrap();
    let parsed = html::parse_document(markup, html::HtmlParseOptions::default()).unwrap();
    let publication = no_quirks_patch_publication_from_output(parsed);
    let completion = HtmlParseCompletion {
        handle: publication.handle,
        version: DomVersion(1),
        document_mode: publication.document_mode,
    };
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        publication,
    })
    .unwrap();
    (tab, completion, commands)
}
fn completed() -> Tab {
    let (mut tab, completion, _commands) = published();
    finish(&mut tab, completion).unwrap();
    tab
}
fn finish(
    tab: &mut Tab,
    completion: HtmlParseCompletion,
) -> Result<(), bus::DocumentPublicationFailure> {
    super::support::complete_document_body(tab);
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: Some(html::internal::HtmlDecodeReport {
            eof_processed: true,
            replacements: Default::default(),
        }),
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
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
    let (mut tab, mut completion, _commands) = published();
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
    let (mut tab, _, _commands) = published();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: None,
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
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
    let (mut tab, completion, _commands) = published();
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
            request_id: tab.active_document_request_id().expect("admitted fixture"),
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
    let (sender, _commands) = std::sync::mpsc::channel();
    tab.set_bus_sender(sender);
    tab.navigate_to_new("https://example.com/other".into())
        .unwrap();
    assert!(tab.page.accepts_observed_artifacts(&draft));
    assert!(tab.render_observed_frame(&ctx, input()).is_ok());
    super::support::admit_pending(&mut tab);
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
