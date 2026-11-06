use super::super::{AddressingRequestState, CssSource, HtmlReferenceBase, Tab};
use super::support::*;
use bus::CoreCommand;
use core_types::ResourceKind;

#[test]
fn ah9_addressing_observation_borrows_committed_state_without_commands_or_raw_ids_in_debug() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><base href='../assets/'><style>p{background:url(a.svg)}</style><p style='cursor:url(b.png)'></p><form action=''></form><img src='same.png'><img src='same.png'>",
    );
    let issued: Vec<_> = commands.try_iter().collect();
    assert_eq!(issued.len(), 1);
    let CoreCommand::FetchStream {
        kind: ResourceKind::Image,
        request,
        stylesheet_slot_id: None,
        ..
    } = &issued[0]
    else {
        panic!("one deduplicated image command")
    };
    let request_id = request.request_id();
    let url = request.original_url().as_str();
    let before = tab.document_address_debug_snapshot();
    let generation = tab.nav_gen;
    let origin = {
        let view = tab.observe_document_addressing().unwrap().unwrap();
        assert_eq!(view.active_base().as_str(), "https://example.test/assets/");
        assert!(view.selected_base().is_some());
        assert_eq!(view.html_references().len(), 3);
        assert_eq!(
            view.html_references()[0].resolution_base(),
            HtmlReferenceBase::DocumentUrl
        );
        assert_eq!(view.css_sources().len(), 2);
        assert!(matches!(
            view.css_sources()[0].source(),
            CssSource::Stylesheet { .. }
        ));
        for source in view.css_sources() {
            assert!(!source.discovery().stats.hit_limit);
            for reference in &source.discovery().references {
                assert!(source.discovery().input.slice(reference.span).is_some());
            }
        }
        view.origin().clone()
    };
    assert_eq!(
        tab.observe_addressing_request(ResourceKind::Image, url, request_id, None),
        Some(AddressingRequestState::Pending)
    );
    assert_eq!(tab.nav_gen, generation);
    assert_eq!(tab.document_address_debug_snapshot(), before);
    assert!(commands.try_recv().is_err());
    assert!(origin.is_same_origin(tab.observe_document_addressing().unwrap().unwrap().origin()));
    tab.navigate_to_new("https://example.test/new-document".into());
    assert!(tab.observe_document_addressing().unwrap().is_none());
}
#[test]
fn ah9_file_document_observation_retains_opaque_identity() {
    let (mut tab, commands) = addressing_tab_with_url(
        "<!doctype html><base href='http://['>",
        "file:///fixture/page.html#old",
    );
    // Initial document fetch/parse-start commands belong to the existing helper.
    assert!(commands.try_recv().is_err());
    let origin = tab
        .observe_document_addressing()
        .unwrap()
        .unwrap()
        .origin()
        .clone();
    tab.navigate_to_new("file:///fixture/page.html#new".into());
    let view = tab.observe_document_addressing().unwrap().unwrap();
    assert!(origin.is_same_origin(view.origin()));
    assert!(!origin.is_same_origin(&view.document_url().url_origin()));
    assert_eq!(view.active_base().fragment(), Some("old"));
    assert_eq!(view.fallback_base().fragment(), Some("new"));
    assert!(commands.try_recv().is_err());
    assert!(Tab::new(9).observe_document_addressing().unwrap().is_none());
}
