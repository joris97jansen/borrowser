use super::super::Tab;
use super::support::no_quirks_patch_publication_from_output;
use crate::rendering::{RenderInvalidationEntryPoint, render_invalidation_request};
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};
use egui::Context;
use html::{HtmlParseOptions, parse_document};

fn history_test_tab() -> (Tab, std::sync::mpsc::Receiver<CoreCommand>) {
    let mut tab = Tab::new(1);
    let (tx, rx) = std::sync::mpsc::channel();
    tab.set_bus_sender(tx);
    (tab, rx)
}

fn history_expect_fetch(
    rx: &std::sync::mpsc::Receiver<CoreCommand>,
    generation: u64,
    expected_url: &str,
) {
    let commands: Vec<_> = rx.try_iter().collect();
    assert!(
        commands.iter().any(|command| matches!(
            command,
            CoreCommand::FetchStream { request_id, url, kind: ResourceKind::Html, .. }
                if *request_id == generation && url == expected_url
        )),
        "missing document request for {expected_url}: {commands:?}"
    );
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

fn history_accept_document(tab: &mut Tab, requested: &str, final_url: &str, markup: &str) {
    let request_id = tab.nav_gen;
    let response = NetworkResponseInfo {
        requested_url: requested.into(),
        final_url: final_url.into(),
        status_code: Some(200),
        content_type: Some("text/html".into()),
    };
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response.clone(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
        bytes_received: markup.len(),
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
        tab_id: tab.tab_id,
        request_id,
        result: Ok(bus::HtmlParseCompletion {
            handle: tab.dom_handle.unwrap(),
            version: tab.dom_version,
            document_mode: tab.page.document_mode.unwrap(),
        }),
    })
    .unwrap();
}

#[test]
fn navigation_history_back_from_fragment_free_replacement_never_relabels_new_document() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page#old";
    let b = "https://example.test/page";
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(
        &mut tab,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#base-a'><img id=old src=a.png><link rel=stylesheet href=a.css>",
    );
    let old_image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "old");
    let old_slots = tab.page.active_external_stylesheet_slots();
    assert_eq!(old_slots.len(), 1);
    let old_handle = tab.dom_handle;
    rx.try_iter().for_each(drop);

    tab.activate_link("");
    let b_request = tab.nav_gen;
    history_expect_fetch(&rx, b_request, b);
    assert!(tab.page.document_url().is_none());
    history_accept_document(
        &mut tab,
        b,
        b,
        "<!doctype html><base href='https://example.test/page#base-b'><img id=new src=b.png><link rel=stylesheet href=b.css>",
    );
    assert_ne!(tab.dom_handle, old_handle);
    assert_eq!(tab.page.document_url().unwrap().as_str(), b);
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, old_image, "a.png"),
        None
    );
    assert!(old_slots.is_disjoint(&tab.page.active_external_stylesheet_slots()));
    rx.try_iter().for_each(drop);

    tab.go_back();
    assert_eq!(tab.nav_gen, b_request + 1);
    history_expect_fetch(&rx, tab.nav_gen, a);
    assert!(
        tab.page.document_url().is_none(),
        "B must not be relabeled as A"
    );
    assert!(tab.page.dom.is_none());
    assert!(tab.page.image_sources.is_empty());
    assert!(tab.page.active_external_stylesheet_slots().is_empty());

    history_accept_document(
        &mut tab,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#base-restored'><img id=restored src=restored.png>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "restored");
    rx.try_iter().for_each(drop);
    tab.go_forward();
    history_expect_fetch(&rx, tab.nav_gen, b);
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
}

#[test]
fn navigation_history_fragments_share_document_and_refresh_repopulates_the_group() {
    let (mut tab, rx) = history_test_tab();
    let root = "https://example.test/page";
    tab.navigate_to_new(root.into());
    history_expect_fetch(&rx, tab.nav_gen, root);
    history_accept_document(
        &mut tab,
        root,
        root,
        "<!doctype html><base href='https://example.test/page#frozen'><img id=old src=a.png><link rel=stylesheet href=a.css>",
    );
    rx.try_iter().for_each(drop);
    let original_generation = tab.nav_gen;
    let original_handle = tab.dom_handle;
    let original_frozen = tab.page.active_base_url().unwrap().clone();
    let original_image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "old");
    let original_source =
        crate::page::selected_image_source(&tab.page.image_sources, original_image, "a.png");
    let original_slots = tab.page.active_external_stylesheet_slots();
    assert!(original_source.is_some());
    assert_eq!(original_slots.len(), 1);

    tab.activate_link("#one");
    tab.activate_link("#two");
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward();
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(tab.nav_gen, original_generation);
    assert_eq!(tab.dom_handle, original_handle);
    assert_eq!(tab.page.active_base_url(), Some(&original_frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, original_image, "a.png"),
        original_source
    );
    assert_eq!(tab.page.active_external_stylesheet_slots(), original_slots);
    history_expect_no_document_fetch(&rx);

    tab.refresh();
    let refreshed = "https://example.test/page#two";
    assert_eq!(tab.nav_gen, original_generation + 1);
    history_expect_fetch(&rx, tab.nav_gen, refreshed);
    assert!(tab.page.dom.is_none());
    history_accept_document(
        &mut tab,
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
    let generation = tab.nav_gen;
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward();
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(tab.nav_gen, generation);
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
    tab.navigate_to_new(url.into());
    history_expect_fetch(&rx, tab.nav_gen, url);
    history_accept_document(&mut tab, url, url, "<!doctype html><p id=first>first</p>");
    let first = tab.dom_handle;
    rx.try_iter().for_each(drop);
    tab.navigate_to_new(url.into());
    history_expect_fetch(&rx, tab.nav_gen, url);
    history_accept_document(&mut tab, url, url, "<!doctype html><p id=second>second</p>");
    assert_ne!(tab.dom_handle, first);
    rx.try_iter().for_each(drop);
    let generation = tab.nav_gen;
    tab.go_back();
    assert_eq!(tab.nav_gen, generation + 1);
    history_expect_fetch(&rx, tab.nav_gen, url);
    assert!(tab.page.dom.is_none());
    assert!(tab.page.document_url().is_none());
    history_accept_document(
        &mut tab,
        url,
        url,
        "<!doctype html><p id=restored>restored</p>",
    );
    rx.try_iter().for_each(drop);
    let generation = tab.nav_gen;
    tab.go_forward();
    assert_eq!(tab.nav_gen, generation + 1);
    history_expect_fetch(&rx, tab.nav_gen, url);
    assert!(tab.page.dom.is_none());
}

#[test]
fn navigation_history_repopulated_entry_rejoins_its_other_fragment_entries() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(
        &mut tab,
        a,
        a,
        "<!doctype html><base href='https://example.test/page#frozen'><img id=old src=old.png>",
    );
    rx.try_iter().for_each(drop);
    tab.activate_link("#one");
    tab.activate_link("#two");
    history_expect_no_document_fetch(&rx);

    tab.navigate_to_new(b.into());
    history_expect_fetch(&rx, tab.nav_gen, b);
    history_accept_document(&mut tab, b, b, "<!doctype html><p id=b>B</p>");
    rx.try_iter().for_each(drop);
    tab.go_back();
    history_expect_fetch(&rx, tab.nav_gen, a_two);
    assert!(tab.page.dom.is_none());
    history_accept_document(
        &mut tab,
        a_two,
        a_two,
        "<!doctype html><base href='https://example.test/page#new-frozen'><img id=restored src=new.png>",
    );
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let frozen = tab.page.active_base_url().unwrap().clone();
    let restored = super::support::element_id(tab.page.dom.as_deref().unwrap(), "restored");
    let source = crate::page::selected_image_source(&tab.page.image_sources, restored, "new.png");
    assert!(source.is_some());
    rx.try_iter().for_each(drop);

    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    tab.go_forward();
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_two);
    assert_eq!(tab.nav_gen, generation);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.page.active_base_url(), Some(&frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, restored, "new.png"),
        source
    );
    history_expect_no_document_fetch(&rx);
    tab.go_forward();
    history_expect_fetch(&rx, tab.nav_gen, b);
    assert!(tab.page.dom.is_none());
}

#[test]
fn navigation_history_redirected_repopulation_does_not_reassociate_old_fragment_entries() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    let redirected = "https://different.test/new";
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(&mut tab, a, a, "<!doctype html><p id=old>A</p>");
    rx.try_iter().for_each(drop);
    tab.activate_link("#one");
    tab.activate_link("#two");
    history_expect_no_document_fetch(&rx);
    tab.navigate_to_new(b.into());
    history_expect_fetch(&rx, tab.nav_gen, b);
    history_accept_document(&mut tab, b, b, "<!doctype html><p id=b>B</p>");
    rx.try_iter().for_each(drop);
    tab.go_back();
    history_expect_fetch(&rx, tab.nav_gen, a_two);
    history_accept_document(
        &mut tab,
        a_two,
        redirected,
        "<!doctype html><p id=redirect>redirect</p>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), redirected);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "redirect");
    rx.try_iter().for_each(drop);
    let generation = tab.nav_gen;
    tab.go_back();
    assert_eq!(tab.nav_gen, generation + 1);
    history_expect_fetch(&rx, tab.nav_gen, a_one);
    assert!(
        tab.page.document_url().is_none(),
        "redirected replacement is not A"
    );
    assert!(tab.page.dom.is_none());
}

#[test]
fn navigation_history_repopulation_preserves_requested_fragment_when_response_omits_it() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/page";
    let a_one = "https://example.test/page#one";
    let a_two = "https://example.test/page#two";
    let b = "https://example.test/other";
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(&mut tab, a, a, "<!doctype html><p id=first>A</p>");
    rx.try_iter().for_each(drop);
    tab.activate_link("#one");
    tab.activate_link("#two");
    history_expect_no_document_fetch(&rx);
    tab.navigate_to_new(b.into());
    history_expect_fetch(&rx, tab.nav_gen, b);
    history_accept_document(&mut tab, b, b, "<!doctype html><p id=b>B</p>");
    rx.try_iter().for_each(drop);

    tab.go_back();
    history_expect_fetch(&rx, tab.nav_gen, a_two);
    history_accept_document(
        &mut tab,
        a_two,
        a,
        "<!doctype html><base href='https://example.test/page#restored'><img id=restored src=restored.png>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), a);
    assert_eq!(tab.history[tab.history_index].url(), a_two);
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let frozen = tab.page.active_base_url().unwrap().clone();
    rx.try_iter().for_each(drop);

    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().as_str(), a_two);
    assert_eq!(tab.history[tab.history_index].url(), a_two);
    assert_eq!(tab.nav_gen, generation);
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
        tab.navigate_to_new(a.into());
        history_expect_fetch(&rx, tab.nav_gen, a);
        history_accept_document(
            &mut tab,
            a,
            a,
            "<!doctype html><base href='https://example.test/page#frozen'><img id=a src=a.png>",
        );
        rx.try_iter().for_each(drop);
        let generation = tab.nav_gen;
        let handle = tab.dom_handle;
        let frozen = tab.page.active_base_url().unwrap().clone();
        tab.activate_link("#one");
        tab.activate_link("#two");
        tab.go_back();
        assert_eq!(tab.page.document_url().unwrap().as_str(), a_one);
        assert_eq!(tab.history_index(), 1);
        assert_eq!(tab.history()[2].url(), a_two);
        history_expect_no_document_fetch(&rx);

        if cross_document {
            tab.navigate_to_new(b.into());
            assert_eq!(tab.nav_gen, generation + 1);
            history_expect_fetch(&rx, tab.nav_gen, b);
            assert!(tab.page.document_url().is_none());
            history_accept_document(&mut tab, b, b, "<!doctype html><p id=b>B</p>");
            rx.try_iter().for_each(drop);
            assert_eq!(
                tab.history()
                    .iter()
                    .map(|entry| entry.url())
                    .collect::<Vec<_>>(),
                vec![a, a_one, b]
            );
            let b_generation = tab.nav_gen;
            let b_handle = tab.dom_handle;
            tab.go_forward();
            assert_eq!(tab.nav_gen, b_generation);
            assert_eq!(tab.dom_handle, b_handle);
            assert_eq!(tab.page.document_url().unwrap().as_str(), b);
            history_expect_no_document_fetch(&rx);

            tab.go_back();
            assert_eq!(tab.nav_gen, b_generation + 1);
            history_expect_fetch(&rx, tab.nav_gen, a_one);
            assert!(tab.page.dom.is_none());
        } else {
            tab.activate_link("#new");
            assert_eq!(
                tab.history()
                    .iter()
                    .map(|entry| entry.url())
                    .collect::<Vec<_>>(),
                vec![a, a_one, a_new]
            );
            tab.go_forward();
            assert_eq!(tab.nav_gen, generation);
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
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(&mut tab, a, a, "<!doctype html><p id=a>A</p>");
    rx.try_iter().for_each(drop);
    tab.navigate_to_new(b.into());
    let cancelled = tab.nav_gen;
    history_expect_fetch(&rx, cancelled, b);
    tab.go_back();
    assert_eq!(tab.nav_gen, cancelled + 1);
    history_expect_fetch(&rx, tab.nav_gen, a);
    assert!(tab.page.document_url().is_none());
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: cancelled,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: NetworkResponseInfo {
            requested_url: b.into(),
            final_url: b.into(),
            status_code: Some(200),
            content_type: Some("text/html".into()),
        },
    })
    .unwrap();
    assert!(
        tab.page.document_url().is_none(),
        "stale response cannot install B"
    );
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: a.into(),
        error_kind: NetworkErrorKind::Cancelled,
        status_code: None,
        error: "synthetic failure".into(),
    })
    .unwrap();
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
    rx.try_iter().for_each(drop);
    let failed = tab.nav_gen;
    tab.go_forward();
    assert_eq!(tab.nav_gen, failed + 1);
    history_expect_fetch(&rx, tab.nav_gen, b);
    history_accept_document(
        &mut tab,
        b,
        redirected,
        "<!doctype html><p id=redirect>redirect</p>",
    );
    assert_eq!(tab.page.document_url().unwrap().as_str(), redirected);
    super::support::element_id(tab.page.dom.as_deref().unwrap(), "redirect");
    rx.try_iter().for_each(drop);
    let generation = tab.nav_gen;
    tab.go_back();
    assert_eq!(tab.nav_gen, generation + 1);
    history_expect_fetch(&rx, tab.nav_gen, a);
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
}

#[test]
fn navigation_history_does_not_reuse_a_response_without_a_published_document() {
    for fail_before_back in [false, true] {
        let (mut tab, rx) = history_test_tab();
        let root = "https://example.test/page";
        let one = "https://example.test/page#one";
        let two = "https://example.test/page#two";
        tab.navigate_to_new(root.into());
        history_expect_fetch(&rx, tab.nav_gen, root);
        history_accept_document(&mut tab, root, root, "<!doctype html><p>original</p>");
        rx.try_iter().for_each(drop);
        tab.activate_link("#one");
        tab.activate_link("#two");
        history_expect_no_document_fetch(&rx);

        tab.refresh();
        let request_id = tab.nav_gen;
        history_expect_fetch(&rx, request_id, two);
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: NetworkResponseInfo {
                requested_url: two.into(),
                final_url: two.into(),
                status_code: Some(200),
                content_type: Some("text/html".into()),
            },
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
                status_code: None,
                error: "synthetic failure".into(),
            })
            .unwrap();
        }
        rx.try_iter().for_each(drop);
        tab.go_back();
        assert_eq!(tab.nav_gen, request_id + 1, "failed={fail_before_back}");
        history_expect_fetch(&rx, tab.nav_gen, one);
        assert!(tab.page.dom.is_none());
        assert!(tab.page.document_url().is_none());
    }
}

#[test]
fn navigation_history_rejects_current_response_with_wrong_requested_url() {
    let (mut tab, rx) = history_test_tab();
    let a = "https://example.test/a";
    let b = "https://example.test/b";
    tab.navigate_to_new(a.into());
    history_expect_fetch(&rx, tab.nav_gen, a);
    history_accept_document(&mut tab, a, a, "<!doctype html><p id=a>A</p>");
    rx.try_iter().for_each(drop);

    tab.navigate_to_new(b.into());
    let request_id = tab.nav_gen;
    history_expect_fetch(&rx, request_id, b);
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: NetworkResponseInfo {
            requested_url: "https://example.test/not-b".into(),
            final_url: b.into(),
            status_code: Some(200),
            content_type: Some("text/html".into()),
        },
    })
    .unwrap();
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
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
        response: NetworkResponseInfo {
            requested_url: b.into(),
            final_url: b.into(),
            status_code: Some(200),
            content_type: Some("text/html".into()),
        },
    })
    .unwrap();
    assert!(tab.page.document_url().is_none());
    rx.try_iter().for_each(drop);
    tab.go_back();
    assert_eq!(tab.nav_gen, request_id + 1);
    history_expect_fetch(&rx, tab.nav_gen, a);
    assert!(tab.page.document_url().is_none());
}

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
