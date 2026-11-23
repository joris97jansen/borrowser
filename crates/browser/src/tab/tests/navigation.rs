use super::super::Tab;
use super::support::no_quirks_patch_publication_from_output;
use crate::rendering::{RenderInvalidationEntryPoint, render_invalidation_request};
use addressing::ResourceKind as Purpose;
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;
use egui::Context;
use html::{HtmlParseOptions, parse_document};
use net::NetworkErrorKind;

fn history_test_tab() -> (Tab, std::sync::mpsc::Receiver<CoreCommand>) {
    let mut tab = Tab::new(1);
    let (tx, rx) = std::sync::mpsc::channel();
    tab.set_bus_sender(tx);
    (tab, rx)
}

#[test]
fn navigation_file_origin_is_retained_for_fragments_and_fresh_after_refetch() {
    let root = "file:///tmp/ah5-history.html";
    let (mut tab, rx) = history_test_tab();
    tab.navigate_to_new(root.into()).unwrap();
    history_accept_document(
        &mut tab,
        Purpose::Document,
        root,
        root,
        "<!doctype html><p>file",
    );
    let first = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    assert!(first.is_opaque());
    let (other, _) = super::support::addressing_tab_with_url("<!doctype html><p>independent", root);
    assert_ne!(
        other.page.addresses.as_ref().unwrap().document_origin(),
        &first
    );
    rx.try_iter().for_each(drop);
    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    tab.go_back().unwrap();
    tab.go_forward().unwrap();
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &first
    );
    history_expect_no_document_fetch(&rx);
    tab.refresh().unwrap();
    assert!(tab.page.addresses.is_some());
    let refreshed = "file:///tmp/ah5-history.html#two";
    history_accept_document(
        &mut tab,
        Purpose::Document,
        refreshed,
        root,
        "<!doctype html><p>reloaded",
    );
    let second = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    assert_ne!(second, first);
    assert_eq!(second.serialize(), first.serialize());
    rx.try_iter().for_each(drop);
    tab.go_back().unwrap();
    tab.go_back().unwrap();
    tab.go_forward().unwrap();
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &second
    );
    history_expect_no_document_fetch(&rx);
    // A distinct load retires the available file document. Traversal back
    // must refetch and determine a new origin.
    tab.navigate_to_new("https://other.test/".into()).unwrap();
    history_accept_document(
        &mut tab,
        Purpose::Document,
        "https://other.test/",
        "https://other.test/",
        "<!doctype html><p>other",
    );
    tab.go_back().unwrap();
    assert!(tab.page.addresses.is_some());
    history_accept_document(
        &mut tab,
        Purpose::Document,
        "file:///tmp/ah5-history.html#one",
        root,
        "<!doctype html><p>returned",
    );
    let third = tab.page.addresses.as_ref().unwrap().document_origin();
    assert_ne!(third, &first);
    assert_ne!(third, &second);
}

fn history_expect_fetch(
    rx: &std::sync::mpsc::Receiver<CoreCommand>,
    generation: u64,
    expected_url: &str,
) -> net::FetchRequest {
    let commands: Vec<_> = rx.try_iter().collect();
    commands
        .iter()
        .find_map(|command| match command {
            CoreCommand::FetchStream {
                request,
                kind: ResourceKind::Html,
                ..
            } if request.request_id() == generation
                && request.legacy_issued_url(ResourceKind::Html).unwrap() == expected_url =>
            {
                Some((**request).clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing document request for {expected_url}: {commands:?}"))
}

fn history_expect_no_document_fetch(rx: &std::sync::mpsc::Receiver<CoreCommand>) {
    assert!(rx.try_iter().all(|command| !matches!(
        command,
        CoreCommand::FetchStream {
            kind: ResourceKind::Html,
            ..
        }
    )));
}

fn history_accept_document(
    tab: &mut Tab,
    purpose: Purpose,
    requested: &str,
    final_url: &str,
    markup: &str,
) {
    let request_id = super::support::pending_id(tab);
    let response = super::support::response_facts(
        purpose,
        requested.into(),
        final_url.into(),
        if addressing::BrowserUrl::parse(requested)
            .unwrap()
            .url
            .scheme()
            == "file"
        {
            None
        } else {
            Some(200)
        },
        Some("text/html".into()),
    );
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response.clone(),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.response.as_ref().unwrap().resource_kind(),
        purpose
    );
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: requested.into(),
        bytes: markup.as_bytes().to_vec(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
        body: net::BodyProgress::try_new(ResourceKind::Html, markup.len()).unwrap(),
    })
    .unwrap();
    let output = parse_document(markup, HtmlParseOptions::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: Some(html::internal::HtmlDecodeReport {
            eof_processed: true,
            replacements: Default::default(),
        }),
        tab_id: tab.tab_id,
        request_id,
        result: Ok(bus::HtmlParseCompletion {
            handle: tab.dom_handle.unwrap(),
            version: tab.dom_version,
            document_mode: tab.page.document_mode.unwrap(),
        }),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.response.as_ref().unwrap().resource_kind(),
        purpose
    );
}

#[test]
fn navigation_history_back_from_fragment_free_replacement_never_relabels_new_document() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page#old";
    let b = "https://example.test/page";
    tab.navigate_to_new(a.into()).unwrap();
    let address_bar_request = history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    assert_eq!(address_bar_request.address().kind(), Purpose::Document);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#base-a'><img id=old src=a.png><link rel=stylesheet href=a.css>",
    );
    let old_image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "old");
    let old_slots = tab.page.active_external_stylesheet_slots();
    assert_eq!(old_slots.len(), 1);
    let old_handle = tab.dom_handle;
    rx.try_iter().for_each(drop);

    tab.activate_link("").unwrap();
    let b_request = super::support::pending_id(&tab);
    let link_request = history_expect_fetch(&rx, b_request, b);
    assert_eq!(link_request.address().kind(), Purpose::Hyperlink);
    assert!(tab.page.document_url().is_some());
    history_accept_document(
        &mut tab,
        link_request.address().kind(),
        b,
        b,
        "<!doctype html><base href='https://example.test/page#base-b'><img id=new src=b.png><link rel=stylesheet href=b.css>",
    );
    assert_eq!(
        tab.document_load.response.as_ref().unwrap().resource_kind(),
        Purpose::Hyperlink
    );
    assert_ne!(tab.dom_handle, old_handle);
    assert_eq!(tab.page.document_url().unwrap().as_str(), b);
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, old_image, "a.png"),
        None
    );
    assert!(old_slots.is_disjoint(&tab.page.active_external_stylesheet_slots()));
    rx.try_iter().for_each(drop);

    tab.go_back().unwrap();
    assert_eq!(super::support::pending_id(&tab), b_request + 1);
    let traversal_request = history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    assert_eq!(traversal_request.address().kind(), Purpose::Document);
    assert!(
        tab.page.document_url().unwrap().as_str() == b,
        "pending traversal must not relabel B as A"
    );
    assert!(tab.page.dom.is_some());
    assert!(!tab.page.image_sources.is_empty());
    assert!(!tab.page.active_external_stylesheet_slots().is_empty());

    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#base-restored'><img id=restored src=restored.png>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "restored");
    rx.try_iter().for_each(drop);
    tab.go_forward().unwrap();
    let forward_request = history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    assert_eq!(forward_request.address().kind(), Purpose::Document);
    assert!(tab.page.document_url().is_some());
    assert!(tab.page.dom.is_some());
}

#[test]
fn navigation_history_fragments_share_document_and_refresh_repopulates_the_group() {
    let (mut tab, rx) = history_test_tab();
    let root = "https://example.test/page";
    tab.navigate_to_new(root.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), root);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        root,
        root,
        "<!doctype html><base href='https://example.test/page#frozen'><img id=old src=a.png><link rel=stylesheet href=a.css>",
    );
    rx.try_iter().for_each(drop);
    let original_generation = tab.active_document_request_id().expect("admitted fixture");
    let original_handle = tab.dom_handle;
    let original_frozen = tab.page.active_base_url().unwrap().clone();
    let original_image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "old");
    let original_source =
        crate::page::selected_image_source(&tab.page.image_sources, original_image, "a.png");
    let original_slots = tab.page.active_external_stylesheet_slots();
    assert!(original_source.is_some());
    assert_eq!(original_slots.len(), 1);

    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward().unwrap();
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        original_generation
    );
    assert_eq!(tab.dom_handle, original_handle);
    assert_eq!(tab.page.active_base_url(), Some(&original_frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, original_image, "a.png"),
        original_source
    );
    assert_eq!(tab.page.active_external_stylesheet_slots(), original_slots);
    history_expect_no_document_fetch(&rx);

    tab.refresh().unwrap();
    let refreshed = "https://example.test/page#two";
    assert_eq!(super::support::pending_id(&tab), original_generation + 1);
    let refresh_request = history_expect_fetch(&rx, super::support::pending_id(&tab), refreshed);
    assert_eq!(refresh_request.address().kind(), Purpose::Document);
    assert!(tab.page.dom.is_some());
    history_accept_document(
        &mut tab,
        Purpose::Document,
        refreshed,
        refreshed,
        "<!doctype html><base href='https://example.test/page#new-frozen'><img id=new src=b.png><link rel=stylesheet href=b.css>",
    );
    let refreshed_handle = tab.dom_handle;
    let refreshed_frozen = tab.page.active_base_url().unwrap().clone();
    let refreshed_image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "new");
    let refreshed_source =
        crate::page::selected_image_source(&tab.page.image_sources, refreshed_image, "b.png");
    let refreshed_slots = tab.page.active_external_stylesheet_slots();
    assert!(refreshed_source.is_some());
    assert!(original_slots.is_disjoint(&refreshed_slots));
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward().unwrap();
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        generation
    );
    assert_eq!(tab.dom_handle, refreshed_handle);
    assert_eq!(tab.page.active_base_url(), Some(&refreshed_frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, refreshed_image, "b.png"),
        refreshed_source
    );
    assert_eq!(tab.page.active_external_stylesheet_slots(), refreshed_slots);
    history_expect_no_document_fetch(&rx);
}

#[test]
fn navigation_history_identical_urls_from_different_document_lifetimes_fetch_on_traversal() {
    let (mut tab, rx) = history_test_tab();
    let url = "https://example.test/page";
    tab.navigate_to_new(url.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), url);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        url,
        url,
        "<!doctype html><p id=first>first</p>",
    );
    let first = tab.dom_handle;
    rx.try_iter().for_each(drop);
    tab.navigate_to_new(url.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), url);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        url,
        url,
        "<!doctype html><p id=second>second</p>",
    );
    assert_ne!(tab.dom_handle, first);
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    tab.go_back().unwrap();
    assert_eq!(super::support::pending_id(&tab), generation + 1);
    history_expect_fetch(&rx, super::support::pending_id(&tab), url);
    assert!(tab.page.dom.is_some());
    assert!(tab.page.document_url().is_some());
    history_accept_document(
        &mut tab,
        Purpose::Document,
        url,
        url,
        "<!doctype html><p id=restored>restored</p>",
    );
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    tab.go_forward().unwrap();
    assert_eq!(super::support::pending_id(&tab), generation + 1);
    history_expect_fetch(&rx, super::support::pending_id(&tab), url);
    assert!(tab.page.dom.is_some());
}

#[test]
fn navigation_history_repopulated_entry_rejoins_its_other_fragment_entries() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    tab.navigate_to_new(a.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#frozen'><img id=old src=old.png>",
    );
    rx.try_iter().for_each(drop);
    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    history_expect_no_document_fetch(&rx);

    tab.navigate_to_new(b.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        b,
        b,
        "<!doctype html><p id=b>B</p>",
    );
    rx.try_iter().for_each(drop);
    tab.go_back().unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a_two);
    assert!(tab.page.dom.is_some());
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a_two,
        a_two,
        "<!doctype html><base href='https://example.test/page#new-frozen'><img id=restored src=new.png>",
    );
    let generation = tab.active_document_request_id().expect("admitted fixture");
    let handle = tab.dom_handle;
    let frozen = tab.page.active_base_url().unwrap().clone();
    let restored = super::support::element_id(tab.page.dom.as_deref().unwrap(), "restored");
    let source = crate::page::selected_image_source(&tab.page.image_sources, restored, "new.png");
    assert!(source.is_some());
    rx.try_iter().for_each(drop);

    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    tab.go_forward().unwrap();
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_two);
    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        generation
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.page.active_base_url(), Some(&frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, restored, "new.png"),
        source
    );
    history_expect_no_document_fetch(&rx);
    tab.go_forward().unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    assert!(tab.page.dom.is_some());
}

#[test]
fn navigation_history_redirected_repopulation_does_not_reassociate_old_fragment_entries() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    let redirected = "https://different.test/new";
    tab.navigate_to_new(a.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><p id=old>A</p>",
    );
    rx.try_iter().for_each(drop);
    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    history_expect_no_document_fetch(&rx);
    tab.navigate_to_new(b.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        b,
        b,
        "<!doctype html><p id=b>B</p>",
    );
    rx.try_iter().for_each(drop);
    tab.go_back().unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a_two);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a_two,
        redirected,
        "<!doctype html><p id=redirect>redirect</p>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), redirected);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "redirect");
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    tab.go_back().unwrap();
    assert_eq!(super::support::pending_id(&tab), generation + 1);
    history_expect_fetch(&rx, super::support::pending_id(&tab), a_one);
    assert!(
        tab.page.document_url().unwrap().as_str() == redirected,
        "pending traversal must retain the redirected document"
    );
    assert!(tab.page.dom.is_some());
}

#[test]
fn navigation_history_repopulation_preserves_requested_fragment_when_response_omits_it() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    tab.navigate_to_new(a.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><p id=first>A</p>",
    );
    rx.try_iter().for_each(drop);
    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    history_expect_no_document_fetch(&rx);
    tab.navigate_to_new(b.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        b,
        b,
        "<!doctype html><p id=b>B</p>",
    );
    rx.try_iter().for_each(drop);

    tab.go_back().unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a_two);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a_two,
        a,
        "<!doctype html><base href='https://example.test/page#restored'><img id=restored src=restored.png>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    assert_eq!(tab.history[tab.history_index].url(), a_two);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    let handle = tab.dom_handle;
    let frozen = tab.page.active_base_url().unwrap().clone();
    rx.try_iter().for_each(drop);

    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_two);
    assert_eq!(tab.history[tab.history_index].url(), a_two);
    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        generation
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.page.active_base_url(), Some(&frozen));
    history_expect_no_document_fetch(&rx);
}

#[test]
fn navigation_history_new_navigation_after_back_truncates_forward_entries() {
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let a_new = "https://example.test/page#new";
    let b = "https://example.test/other";

    for cross_document in [false, true] {
        let (mut tab, rx) = history_test_tab();
        tab.navigate_to_new(a.into()).unwrap();
        history_expect_fetch(&rx, super::support::pending_id(&tab), a);
        history_accept_document(
            &mut tab,
            Purpose::Document,
            a,
            a,
            "<!doctype html><base href='https://example.test/page#frozen'><img id=a src=a.png>",
        );
        rx.try_iter().for_each(drop);
        let generation = tab.active_document_request_id().expect("admitted fixture");
        let handle = tab.dom_handle;
        let frozen = tab.page.active_base_url().unwrap().clone();
        tab.activate_link("#one").unwrap();
        tab.activate_link("#two").unwrap();
        tab.go_back().unwrap();
        assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
        assert_eq!(tab.history_index(), 1);
        assert_eq!(tab.history()[2].url(), a_two);
        history_expect_no_document_fetch(&rx);

        if cross_document {
            tab.navigate_to_new(b.into()).unwrap();
            assert_eq!(super::support::pending_id(&tab), generation + 1);
            history_expect_fetch(&rx, super::support::pending_id(&tab), b);
            assert!(tab.page.document_url().is_some());
            history_accept_document(
                &mut tab,
                Purpose::Document,
                b,
                b,
                "<!doctype html><p id=b>B</p>",
            );
            rx.try_iter().for_each(drop);
            assert_eq!(
                tab.history()
                    .iter()
                    .map(|entry| entry.url())
                    .collect::<Vec<_>>(),
                vec![a, a_one, b]
            );
            let b_generation = tab.active_document_request_id().expect("admitted fixture");
            let b_handle = tab.dom_handle;
            tab.go_forward().unwrap();
            assert_eq!(
                tab.active_document_request_id().expect("admitted fixture"),
                b_generation
            );
            assert_eq!(tab.dom_handle, b_handle);
            assert_eq!(tab.page.document_url().unwrap().as_str(), b);
            history_expect_no_document_fetch(&rx);

            tab.go_back().unwrap();
            assert_eq!(super::support::pending_id(&tab), b_generation + 1);
            history_expect_fetch(&rx, super::support::pending_id(&tab), a_one);
            assert!(tab.page.dom.is_some());
        } else {
            tab.activate_link("#new").unwrap();
            assert_eq!(
                tab.history()
                    .iter()
                    .map(|entry| entry.url())
                    .collect::<Vec<_>>(),
                vec![a, a_one, a_new]
            );
            tab.go_forward().unwrap();
            assert_eq!(
                tab.active_document_request_id().expect("admitted fixture"),
                generation
            );
            assert_eq!(tab.dom_handle, handle);
            assert_eq!(tab.page.document_url().unwrap().as_str(), a_new);
            assert_eq!(tab.page.active_base_url(), Some(&frozen));
            history_expect_no_document_fetch(&rx);
        }
    }
}

#[test]
fn navigation_history_pending_failed_and_stale_responses_cannot_bind_the_wrong_document() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/a";
    let b = "https://example.test/b";
    let redirected = "https://other.test/final";
    for url in [a, b] {
        tab.navigate_to_new(url.into()).unwrap();
        history_expect_fetch(&rx, super::support::pending_id(&tab), url);
        history_accept_document(
            &mut tab,
            Purpose::Document,
            url,
            url,
            "<!doctype html><p>committed",
        );
        rx.try_iter().for_each(drop);
    }
    let active = tab.active_document_request_id();
    let handle = tab.dom_handle;
    tab.refresh().unwrap();
    let cancelled = super::support::pending_id(&tab);
    history_expect_fetch(&rx, cancelled, b);
    tab.go_back().unwrap();
    let traversal = super::support::pending_id(&tab);
    assert!(traversal > cancelled);
    history_expect_fetch(&rx, traversal, a);
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: cancelled,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: super::support::response_facts(
            Purpose::Document,
            b.into(),
            b.into(),
            Some(200),
            Some("text/html".into()),
        ),
    })
    .unwrap();
    assert_eq!(tab.active_document_request_id(), active);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.page.document_url().unwrap().as_str(), b);
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: 1,
        request_id: traversal,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: a.into(),
        error_kind: NetworkErrorKind::Cancelled,
        response: None,
        body: None,
        error: "synthetic failure".into(),
    })
    .unwrap();
    assert_eq!(tab.history_index(), 1);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.active_document_request_id(), active);
    rx.try_iter().for_each(drop);
    // Retry traversal: the failed attempt never committed its target index.
    tab.go_back().unwrap();
    assert!(super::support::pending_id(&tab) > traversal);
    history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        redirected,
        "<!doctype html><p id=redirect>redirect</p>",
    );
    assert_eq!(tab.history_index(), 0);
    assert_eq!(tab.page.document_url().unwrap().as_str(), redirected);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "redirect");
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id();
    tab.go_forward().unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), b);
    assert_eq!(tab.active_document_request_id(), generation);
    assert_eq!(tab.page.document_url().unwrap().as_str(), redirected);
}

#[test]
fn navigation_history_does_not_reuse_a_response_without_a_published_document() {
    for fail_before_back in [false, true] {
        let (mut tab, rx) = history_test_tab();
        let root = "https://example.test/page";
        let one = "https://example.test/page#one";
        let two = "https://example.test/page#two";
        tab.navigate_to_new(root.into()).unwrap();
        history_expect_fetch(&rx, super::support::pending_id(&tab), root);
        history_accept_document(
            &mut tab,
            Purpose::Document,
            root,
            root,
            "<!doctype html><p>original</p>",
        );
        rx.try_iter().for_each(drop);
        tab.activate_link("#one").unwrap();
        tab.activate_link("#two").unwrap();
        history_expect_no_document_fetch(&rx);

        tab.refresh().unwrap();
        let request_id = super::support::pending_id(&tab);
        history_expect_fetch(&rx, request_id, two);
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: super::support::response_facts(
                addressing::ResourceKind::Document,
                two.into(),
                two.into(),
                Some(200),
                Some("text/html".into()),
            ),
        })
        .unwrap();
        assert!(tab.page.dom.is_none());
        if fail_before_back {
            tab.on_core_event(CoreEvent::NetworkError {
                tab_id: 1,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                url: two.into(),
                error_kind: NetworkErrorKind::Cancelled,
                response: None,
                body: None,
                error: "synthetic failure".into(),
            })
            .unwrap();
        }
        rx.try_iter().for_each(drop);
        tab.go_back().unwrap();
        assert_eq!(
            super::support::pending_id(&tab),
            request_id + 1,
            "failed={fail_before_back}"
        );
        history_expect_fetch(&rx, super::support::pending_id(&tab), one);
        assert!(tab.page.dom.is_none());
        assert_eq!(tab.page.document_url().unwrap().as_str(), two);
    }
}

#[test]
fn navigation_history_rejects_current_response_with_wrong_requested_url() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/a";
    let b = "https://example.test/b";
    tab.navigate_to_new(a.into()).unwrap();
    history_expect_fetch(&rx, super::support::pending_id(&tab), a);
    history_accept_document(
        &mut tab,
        Purpose::Document,
        a,
        a,
        "<!doctype html><p id=a>A</p>",
    );
    rx.try_iter().for_each(drop);

    tab.navigate_to_new(b.into()).unwrap();
    let request_id = super::support::pending_id(&tab);
    history_expect_fetch(&rx, request_id, b);
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: super::support::response_facts(
            addressing::ResourceKind::Document,
            "https://example.test/not-b".into(),
            b.into(),
            Some(200),
            Some("text/html".into()),
        ),
    })
    .unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    assert!(tab.page.dom.is_some());
    let commands: Vec<_> = rx.try_iter().collect();
    assert!(commands.iter().any(|command| matches!(command,
        CoreCommand::CancelRequest { request_id: cancelled, .. } if *cancelled == request_id
    )));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, CoreCommand::ParseHtmlStart { .. }))
    );

    // A second response cannot revive the failed lifecycle or install B.
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: super::support::response_facts(
            addressing::ResourceKind::Document,
            b.into(),
            b.into(),
            Some(200),
            Some("text/html".into()),
        ),
    })
    .unwrap();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    rx.try_iter().for_each(drop);
    tab.go_back().unwrap();
    assert_eq!(tab.history_index(), 0);
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    assert!(
        tab.navigation_attempt
            .as_ref()
            .unwrap()
            .terminal
            .as_ref()
            .unwrap()
            .is_err()
    );
    history_expect_no_document_fetch(&rx);
}

#[test]
fn redirected_document_response_updates_tab_base_url_and_status() {
    let (mut tab, _commands) = history_test_tab();
    let requested = "https://example.com/".to_string();
    tab.navigate_to_new(requested.clone()).unwrap();
    let final_url = "https://example.com/landing".to_string();
    let response = super::support::response_facts(
        addressing::ResourceKind::Document,
        requested,
        final_url.clone(),
        Some(200),
        Some("text/html; charset=utf-8".to_string()),
    );

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 1,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response.clone(),
    })
    .unwrap();

    let markup = "<!doctype html><title>Example Domain</title><h1>Example Domain</h1>";
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 1,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: response.requested_url().into(),
        bytes: markup.as_bytes().to_vec(),
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
        body: net::BodyProgress::try_new(ResourceKind::Html, markup.len()).unwrap(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: 1,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();

    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: Some(html::internal::HtmlDecodeReport {
            eof_processed: true,
            replacements: Default::default(),
        }),
        tab_id: tab.tab_id,
        request_id: 1,
        result: Ok(bus::HtmlParseCompletion {
            handle: tab.dom_handle.unwrap(),
            version: tab.dom_version,
            document_mode: tab.page.document_mode.unwrap(),
        }),
    })
    .unwrap();
    assert_eq!(
        tab.page.active_base_url().map(|url| url.as_str()),
        Some(final_url.as_str())
    );
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
fn eligible_navigation_activation_clears_pending_render_work_and_last_trace() {
    let (mut tab, _commands) = history_test_tab();
    let request_id = super::support::admit_document(&mut tab, "https://example.com/");

    let output = parse_document(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
        HtmlParseOptions::default(),
    )
    .expect("parse should succeed");

    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id,
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

    tab.navigate_to_new("next.example.com/page".to_string())
        .unwrap();

    assert!(!tab.pending_render_work.is_empty());
    assert!(tab.last_render_trace.is_some());
    super::support::admit_pending(&mut tab);
    assert!(tab.pending_render_work.is_empty());
    assert!(tab.last_render_trace.is_none());
}
