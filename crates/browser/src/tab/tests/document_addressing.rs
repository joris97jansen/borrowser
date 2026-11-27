use super::super::Tab;
use super::support::*;
use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion, ResourceKind};
use html::{DomPatch, PatchKey};

#[test]
fn document_addressing_static_selection_matrix() {
    for (markup, expected) in [
        ("<p>No base", ADDRESS_URL),
        ("<base href='https://cdn.test/a/'>", "https://cdn.test/a/"),
        ("<base href='../assets/'>", "https://example.test/assets/"),
        ("<base href>", "https://example.test/dir/page?q=1"),
        ("<base href=' \t '>", "https://example.test/dir/page?q=1"),
        ("<base href='http://['><base href='/later/'>", ADDRESS_URL),
        (
            "<base href='data:text/plain,x'><base href='/later/'>",
            ADDRESS_URL,
        ),
        ("<base href='javascript:x()'>", ADDRESS_URL),
        (
            "<base target='_blank'><base href='/first/'><base href='/last/'>",
            "https://example.test/first/",
        ),
        (
            "<base href='/first/' href='/last/'>",
            "https://example.test/first/",
        ),
        (
            "<body><div><base href='/body/'></div>",
            "https://example.test/body/",
        ),
        (
            "<template><base href='/inert/'></template><base href='/active/'>",
            "https://example.test/active/",
        ),
        (
            "<body><svg><base href='/svg/'/></svg><math><base href='/math/'/></math><base href='/html/'>",
            "https://example.test/html/",
        ),
        (
            "<body><svg><foreignObject><base href='/integrated/'></foreignObject></svg>",
            "https://example.test/integrated/",
        ),
        ("<base href=' /a%zz '>", "https://example.test/a%zz"),
    ] {
        let (tab, _) = addressing_tab(&format!("<!doctype html>{markup}"));
        assert_eq!(tab.page.document_url().unwrap().as_str(), ADDRESS_URL);
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &addressing::BrowserUrl::parse(ADDRESS_URL)
                .unwrap()
                .url
                .url_origin()
        );
        assert_eq!(tab.page.fallback_base_url().unwrap().as_str(), ADDRESS_URL);
        assert_eq!(
            tab.page.active_base_url().unwrap().as_str(),
            expected,
            "{markup}"
        );
    }
}

#[test]
fn document_addressing_file_origin_survives_base_clear_handle_and_rejected_publications() {
    let file = "file:///tmp/ah5-document.html";
    let (mut tab, rx) = addressing_tab_with_url(
        "<!doctype html><base id=b href='https://other.test/'>",
        file,
    );
    let retained = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    assert!(retained.is_opaque());
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(base, &[("href", "https://different.test/")])],
    )
    .unwrap();
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &retained
    );
    let parsed = html::parse_document(
        "<!doctype html><base href='https://replacement.test/'>",
        Default::default(),
    )
    .unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        publication: no_quirks_patch_publication_from_output(parsed),
    })
    .unwrap();
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &retained
    );
    let parsed = html::parse_document("<!doctype html><p>clear", Default::default()).unwrap();
    publish_addressing_patches(
        &mut tab,
        std::iter::once(DomPatch::Clear)
            .chain(
                parsed
                    .patches
                    .into_iter()
                    .filter(|patch| !matches!(patch, DomPatch::Clear)),
            )
            .collect(),
    )
    .unwrap();
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &retained
    );
    let before = tab.page.addresses.clone();
    let parsed = html::parse_document("<!doctype html><p>stale", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture") - 1,
        publication: no_quirks_patch_publication_from_output(parsed),
    })
    .unwrap();
    assert_eq!(tab.page.addresses, before);
    assert!(
        publish_addressing_patches(
            &mut tab,
            vec![DomPatch::AppendText {
                key: PatchKey(99999),
                text: "invalid".into()
            }]
        )
        .is_err()
    );
    assert_eq!(tab.page.addresses, before);
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &retained
    );
    assert_eq!(retained.serialize(), "null");
    assert!(
        tab.page
            .document_address_debug_snapshot()
            .unwrap()
            .contains("document_origin: Origin { kind: \"opaque\" }")
    );
    rx.try_iter().for_each(drop);
}

#[test]
fn document_addressing_unsupported_response_never_partially_accepts() {
    for unsupported in [
        "ftp://host/a",
        "ws://host/a",
        "wss://host/a",
        "about:blank?x#f",
        "about:srcdoc",
        "data:text/html,secret",
        "blob:https://host/id",
        "javascript:secret",
        "custom-private:secret",
    ] {
        for transition in ["new", "refresh", "traversal"] {
            let (mut tab, rx) = addressing_tab_with_url(
                "<!doctype html><p>previous",
                "file:///tmp/ah5-admission.html",
            );
            if transition == "traversal" {
                let id = super::support::admit_document(&mut tab, "https://request.test/other");
                let output =
                    html::parse_document("<!doctype html><p>other", Default::default()).unwrap();
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: 1,
                    request_id: id,
                    publication: super::support::no_quirks_patch_publication_from_output(output),
                })
                .unwrap();
            }
            let addresses = tab.page.addresses.clone();
            let pointer = tab.page.dom.as_deref().unwrap() as *const html::Node;
            let identity = tab.active_document_request_id();
            let index = tab.history_index;
            let len = tab.history.len();
            let association = tab.history[index].document_state.clone().unwrap();
            match transition {
                "new" => tab
                    .navigate_to_new("https://request.test/new".into())
                    .unwrap(),
                "refresh" => tab.refresh().unwrap(),
                _ => tab.go_back().unwrap(),
            }
            let request = tab.navigation_attempt.as_ref().unwrap().request.clone();
            assert_eq!(tab.page.addresses, addresses);
            rx.try_iter().for_each(drop);
            let response = super::support::response_facts(
                request.address().kind(),
                request.original_url().as_str().into(),
                unsupported.into(),
                if request.original_url().scheme() == "file" {
                    None
                } else {
                    Some(200)
                },
                Some("text/html".into()),
            );
            tab.on_core_event(CoreEvent::NetworkStart {
                tab_id: 1,
                request_id: request.request_id(),
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                response,
            })
            .unwrap();
            assert!(matches!(
                tab.navigation_attempt.as_ref().unwrap().terminal,
                Some(Err(
                    super::super::state::DocumentLoadFailure::UnsupportedDocumentCreation { .. }
                ))
            ));
            assert_eq!(tab.active_document_request_id(), identity);
            assert_eq!(tab.page.addresses, addresses);
            assert_eq!(
                tab.page.dom.as_deref().unwrap() as *const html::Node,
                pointer
            );
            assert_eq!(tab.history_index, index);
            assert_eq!(tab.history.len(), len);
            assert!(
                tab.history[index]
                    .document_state
                    .as_ref()
                    .unwrap()
                    .is_same(&association)
            );
            assert!(
                !rx.try_iter()
                    .any(|c| matches!(c, CoreCommand::ParseHtmlStart { .. }))
            );
        }
    }
}

#[test]
fn document_addressing_ordered_changes_and_frozen_lifetime() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=first href='http://['><base id=second href='../second/'>",
    );
    let first = element_id(tab.page.dom.as_deref().unwrap(), "first");
    let second = element_id(tab.page.dom.as_deref().unwrap(), "second");
    tab.activate_link("https://example.test/dir/page?q=1#new")
        .unwrap();
    assert_eq!(
        tab.page.fallback_base_url().unwrap().fragment(),
        Some("new")
    );
    assert_eq!(tab.page.active_base_url().unwrap().as_str(), ADDRESS_URL);
    assert!(
        rx.try_iter()
            .all(|cmd| !matches!(cmd, CoreCommand::FetchStream { .. }))
    );
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            first,
            &[
                ("id", "first"),
                ("href", "http://["),
                ("title", "unrelated"),
            ],
        )],
    )
    .unwrap();
    assert_eq!(tab.page.active_base_url().unwrap().as_str(), ADDRESS_URL);
    publish_addressing_patches(
        &mut tab,
        vec![
            attributes(first, &[("id", "first"), ("href", "../one/")]),
            attributes(first, &[("id", "first"), ("href", "../two/")]),
        ],
    )
    .unwrap();
    assert_eq!(
        tab.page.active_base_url().unwrap().as_str(),
        "https://example.test/two/"
    );
    publish_addressing_patches(
        &mut tab,
        vec![DomPatch::RemoveNode {
            key: PatchKey::from_id(first),
        }],
    )
    .unwrap();
    assert_eq!(
        tab.page.active_base_url().unwrap().as_str(),
        "https://example.test/second/"
    );
    publish_addressing_patches(&mut tab, vec![attributes(second, &[("id", "second")])]).unwrap();
    assert_eq!(tab.page.active_base_url(), tab.page.fallback_base_url());
}

#[test]
fn document_addressing_rejection_stale_events_and_replacement() {
    let (mut tab, _) = addressing_tab("<!doctype html><base id=b href='/original/'><img src=x>");
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    let before = tab.page.addresses.clone();
    let version = tab.dom_version;
    let bindings = tab.page.image_sources.len();
    assert!(
        publish_addressing_patches(
            &mut tab,
            vec![
                attributes(base, &[("href", "/rejected/")]),
                DomPatch::AppendText {
                    key: PatchKey(99999),
                    text: "invalid".into()
                }
            ]
        )
        .is_err()
    );
    assert_eq!(tab.page.addresses, before);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.image_sources.len(), bindings);
    let output =
        html::parse_document("<!doctype html><base href='/stale/'>", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: 0,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert_eq!(tab.page.addresses, before);
    // A fresh current document is required after the intentional fatal rejection.
    let (mut tab, _) = addressing_tab("<!doctype html><base href='/old/'>");
    let output = html::parse_document("<!doctype html><p>replacement", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().unwrap(),
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert_eq!(tab.page.active_base_url().unwrap().as_str(), ADDRESS_URL);
    let output =
        html::parse_document("<!doctype html><base href='/clear/'>", Default::default()).unwrap();
    publish_addressing_patches(
        &mut tab,
        std::iter::once(DomPatch::Clear)
            .chain(
                output
                    .patches
                    .into_iter()
                    .filter(|p| !matches!(p, DomPatch::Clear)),
            )
            .collect(),
    )
    .unwrap();
    assert_eq!(
        tab.page.active_base_url().unwrap().as_str(),
        "https://example.test/clear/"
    );
}

#[test]
fn document_addressing_chunking_qualifies_final_state_and_ordered_facts() {
    let source = "<!doctype html><head><base target=_blank><template><base href='/inert/'></template></head><body><p>é&amp;x</p><base href='../caf&#233;/'><base href='/later/'>";
    for chunk in [source.len(), 1, 2, 7, 19] {
        let mut parser = html::HtmlParser::new(Default::default()).unwrap();
        let mut tab = Tab::new(1);
        super::support::admit_document(&mut tab, ADDRESS_URL);
        let mut pending = Vec::new();
        let mut from = DomVersion::INITIAL;
        for bytes in source.as_bytes().chunks(chunk) {
            parser.push_bytes(bytes).unwrap();
            parser.pump().unwrap();
            pending.extend(parser.take_patches().unwrap());
            if let Some(mode) = parser.selected_document_mode().unwrap()
                && !pending.is_empty()
            {
                let publication = bus::DocumentPublication {
                    handle: DomHandle(77),
                    document_mode: mode,
                    payload: bus::DocumentPublicationPayload::Patch {
                        from,
                        to: from.next(),
                        patches: std::mem::take(&mut pending),
                    },
                };
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: 1,
                    request_id: tab.active_document_request_id().unwrap(),
                    publication,
                })
                .unwrap();
                from = from.next();
            }
        }
        parser.finish().unwrap();
        pending.extend(parser.take_patches().unwrap());
        if !pending.is_empty() {
            let publication =
                no_quirks_patch_publication(DomHandle(77), from, from.next(), pending);
            tab.on_core_event(CoreEvent::DomPatchUpdate {
                tab_id: 1,
                request_id: tab.active_document_request_id().unwrap(),
                publication,
            })
            .unwrap();
        }
        assert_eq!(
            tab.page.active_base_url().unwrap().as_str(),
            "https://example.test/caf%C3%A9/",
            "chunk={chunk}"
        );
    }
}

#[test]
fn document_addressing_hyperlinks_and_history_are_separate_from_address_bar_guessing() {
    let (mut tab, rx) = addressing_tab("<!doctype html><p>document");
    let generation = tab.active_document_request_id().expect("admitted fixture");
    tab.url = "editable address bar text".into();
    tab.activate_link("#one").unwrap();
    tab.activate_link("#two").unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("initial"));
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        generation
    );
    assert!(
        rx.try_iter()
            .all(|cmd| !matches!(cmd, CoreCommand::FetchStream { .. }))
    );
    for href in [
        "data:text/plain,x",
        "javascript:x()",
        "mailto:x@example.test",
        "ftp://example.test/x",
        "custom:opaque",
        "http://[",
    ] {
        assert!(tab.activate_link(href).is_err());
        assert_eq!(
            tab.active_document_request_id().expect("admitted fixture"),
            generation
        );
        assert!(rx.try_recv().is_err());
    }
    let (mut tab, rx) = addressing_tab("<!doctype html><base href='https://other.test/different'>");
    let active = tab.active_document_request_id();
    tab.activate_link("#fragment").unwrap();
    assert_eq!(tab.active_document_request_id(), active);
    assert_eq!(super::support::pending_id(&tab), active.unwrap() + 1);
    assert!(tab.page.document_url().is_some());
    assert!(rx.try_iter().any(|cmd| matches!(cmd, CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Html, .. } if fetch_request.legacy_issued_url(ResourceKind::Html).unwrap() == "https://other.test/different#fragment")));
    let (mut tab, rx) = addressing_tab("<!doctype html><base href='../assets/'>");
    tab.activate_link("child").unwrap();
    assert!(rx.try_iter().any(|cmd| matches!(cmd, CoreCommand::FetchStream { request: fetch_request, .. } if fetch_request.legacy_issued_url(ResourceKind::Html).unwrap() == "https://example.test/assets/child")));
    let mut tab = Tab::new(1);
    assert!(tab.activate_link("relative").is_err());
    assert!(tab.activate_link("https://example.test/absolute").is_err());
    assert!(tab.activate_link("custom:opaque").is_err());
    assert_eq!(tab.active_document_request_id(), None);
    assert!(tab.page.document_url().is_none());
    let (tx, rx) = std::sync::mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new("example.test".into()).unwrap();
    assert_eq!(tab.url, "https://example.test/");
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::FetchStream { .. }
    ));
}

fn addressing_tab_at(
    document_url: &str,
    base_href: &str,
) -> (Tab, std::sync::mpsc::Receiver<CoreCommand>) {
    addressing_tab_with_response_url(
        &format!(
            "<!doctype html><base href='{base_href}'><img id=old-image src=old.png><link rel=stylesheet href=old.css>"
        ),
        "https://initial.test/navigation",
        document_url,
    )
}

#[test]
fn document_addressing_fragment_cross_document_navigation_replaces_address_and_associations() {
    let credential_url = "https://old:secret@example.test/dir/page?q=1#initial";
    for (current, base, href, destination) in [
        (
            credential_url,
            "https://new:other@example.test/dir/page?q=1",
            "#next",
            "https://new:other@example.test/dir/page?q=1#next",
        ),
        (
            credential_url,
            "https://new:secret@example.test/dir/page?q=1",
            "#next",
            "https://new:secret@example.test/dir/page?q=1#next",
        ),
        (
            credential_url,
            "https://old:other@example.test/dir/page?q=1",
            "#next",
            "https://old:other@example.test/dir/page?q=1#next",
        ),
        (
            ADDRESS_URL,
            "https://new:other@example.test/dir/page?q=1",
            "#next",
            "https://new:other@example.test/dir/page?q=1#next",
        ),
        (
            ADDRESS_URL,
            "https://example.test/other/page?q=1",
            "#next",
            "https://example.test/other/page?q=1#next",
        ),
        (
            ADDRESS_URL,
            "https://other.test/dir/page?q=1",
            "#next",
            "https://other.test/dir/page?q=1#next",
        ),
        (
            ADDRESS_URL,
            "https://example.test/dir/page?q=2",
            "#next",
            "https://example.test/dir/page?q=2#next",
        ),
        (
            ADDRESS_URL,
            "https://example.test:8443/dir/page?q=1",
            "#next",
            "https://example.test:8443/dir/page?q=1#next",
        ),
        (
            ADDRESS_URL,
            "https://example.test/dir/page?q=1#base-fragment",
            "",
            "https://example.test/dir/page?q=1",
        ),
    ] {
        let (mut tab, rx) = addressing_tab_at(current, base);
        rx.try_iter().for_each(drop);
        let old_image = element_id(tab.page.dom.as_deref().unwrap(), "old-image");
        assert!(
            crate::page::selected_image_source(&tab.page.image_sources, old_image, "old.png")
                .is_some()
        );
        let old_slots = tab.page.active_external_stylesheet_slots();
        let credential_target = addressing::BrowserUrl::parse(destination).unwrap().url;
        let has_credentials =
            !credential_target.username().is_empty() || credential_target.password().is_some();
        let generation = tab.active_document_request_id().expect("admitted fixture");
        if has_credentials {
            let addresses = tab.page.addresses.clone();
            let history = tab
                .history
                .iter()
                .map(|e| e.url().to_owned())
                .collect::<Vec<_>>();
            let visible = tab.url.clone();
            assert!(matches!(
                tab.activate_link(href),
                Err(super::super::NavigationError::Request(
                    net::RequestAdmissionError::UrlCredentialsUnsupported
                ))
            ));
            assert_eq!(tab.active_document_request_id(), Some(generation));
            assert_eq!(tab.page.addresses, addresses);
            assert_eq!(
                tab.history
                    .iter()
                    .map(|e| e.url().to_owned())
                    .collect::<Vec<_>>(),
                history
            );
            assert_eq!(tab.url, visible);
            assert!(rx.try_recv().is_err());
            continue;
        }
        assert_eq!(old_slots.len(), 1);
        tab.activate_link(href).unwrap();
        assert_eq!(tab.active_document_request_id(), Some(generation));
        assert_eq!(super::support::pending_id(&tab), generation + 1);
        assert_eq!(tab.url, destination);
        assert_ne!(
            tab.history.last().map(|entry| entry.url()),
            Some(destination)
        );
        assert!(tab.page.document_url().is_some());
        assert!(tab.page.active_base_url().is_some());
        assert!(tab.page.dom.is_some());
        assert!(!tab.page.image_sources.is_empty());
        assert_eq!(tab.page.active_external_stylesheet_slots(), old_slots);
        let issued = rx
            .try_iter()
            .find_map(|command| match command {
                CoreCommand::FetchStream {
                    request,
                    kind: ResourceKind::Html,
                    ..
                } => Some(*request),
                _ => None,
            })
            .expect("issued hyperlink request");
        assert_eq!(issued.request_id(), generation + 1);
        assert_eq!(
            issued.legacy_issued_url(ResourceKind::Html).unwrap(),
            destination
        );
        assert_eq!(issued.address().kind(), addressing::ResourceKind::Hyperlink);
        let mut headers = net::HttpHeaders::new();
        headers.append("content-type", b"text/html").unwrap();
        let response = net::NetworkResponseInfo::http(
            issued.address(),
            issued.legacy_issued_url(ResourceKind::Html).unwrap(),
            Some(destination.into()),
            net::HttpStatus::try_from(200).unwrap(),
            headers,
        )
        .unwrap();

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: generation + 1,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response,
        })
        .unwrap();
        assert_eq!(
            tab.document_load.response.as_ref().unwrap().resource_kind(),
            addressing::ResourceKind::Hyperlink
        );
        assert_eq!(tab.page.document_url().unwrap().as_str(), destination);
        assert_eq!(tab.page.fallback_base_url().unwrap().as_str(), destination);
        assert_eq!(tab.page.active_base_url().unwrap().as_str(), destination);
        assert!(tab.page.dom.is_none());
        assert!(tab.page.image_sources.is_empty());
        assert!(tab.page.active_external_stylesheet_slots().is_empty());

        let output = html::parse_document(
            "<!doctype html><img id=new-image src=new.png><link rel=stylesheet href=new.css>",
            Default::default(),
        )
        .unwrap();
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: generation + 1,
            publication: no_quirks_patch_publication_from_output(output),
        })
        .unwrap();
        assert_eq!(tab.page.document_url().unwrap().as_str(), destination);
        assert_eq!(
            tab.document_load.response.as_ref().unwrap().resource_kind(),
            addressing::ResourceKind::Hyperlink
        );
        assert_eq!(tab.page.active_base_url().unwrap().as_str(), destination);
        assert_eq!(
            crate::page::selected_image_source(&tab.page.image_sources, old_image, "old.png"),
            None,
            "old authored association cannot survive replacement"
        );
        let new_image = element_id(tab.page.dom.as_deref().unwrap(), "new-image");
        assert!(
            crate::page::selected_image_source(&tab.page.image_sources, new_image, "new.png")
                .is_some()
        );
        let new_slots = tab.page.active_external_stylesheet_slots();
        assert_eq!(new_slots.len(), 1);
        assert!(old_slots.is_disjoint(&new_slots));
    }
}

#[test]
fn document_addressing_fragment_navigation_preserves_frozen_base_and_owner_associations() {
    let current = "https://old:secret@example.test/dir/page?q=1";
    // Host case and an explicit default port normalize to the current URL.
    let base = "https://old:secret@EXAMPLE.TEST:443/dir/page?q=1#frozen";
    let (mut tab, rx) = addressing_tab_at(current, base);
    rx.try_iter().for_each(drop);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let frozen = tab.page.active_base_url().unwrap().clone();
    let old_image = element_id(tab.page.dom.as_deref().unwrap(), "old-image");
    let image = crate::page::selected_image_source(&tab.page.image_sources, old_image, "old.png");
    let slots = tab.page.active_external_stylesheet_slots();
    assert!(image.is_some());
    assert_eq!(
        slots.len(),
        1,
        "attachment association is retained even when Fetch rejects it"
    );
    assert!(tab.stylesheet_loads.is_empty());
    assert_eq!(tab.page.pending_count(), 0);

    tab.activate_link("#one").unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.activate_link("#").unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some(""));
    assert_eq!(
        tab.history.last().map(|entry| entry.url()),
        Some("https://old:secret@example.test/dir/page?q=1#")
    );
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_forward().unwrap();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some(""));

    assert_eq!(
        tab.active_document_request_id().expect("admitted fixture"),
        generation
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.active_base_url(), Some(&frozen));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, old_image, "old.png"),
        image
    );
    assert_eq!(tab.page.active_external_stylesheet_slots(), slots);
    assert!(rx.try_iter().all(|command| !matches!(
        command,
        CoreCommand::FetchStream {
            kind: ResourceKind::Html,
            ..
        }
    )));
}

#[test]
fn document_addressing_response_validation_and_no_synthetic_base() {
    for final_url in ["", "http://["] {
        let mut tab = Tab::new(1);
        let (tx, _commands) = std::sync::mpsc::channel();
        tab.set_bus_sender(tx);
        tab.navigate_to_new("https://example.test/requested".into())
            .unwrap();
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: super::support::pending_id(&tab),
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: super::support::response_facts(
                addressing::ResourceKind::Document,
                "https://example.test/requested".into(),
                final_url.into(),
                Some(200),
                None,
            ),
        })
        .unwrap();
        // AI3 removes the old empty-final-URL fallback: absent provenance must
        // not synthesize a document address from the request.
        assert!(tab.page.document_url().is_none());
        assert!(matches!(
            &tab.navigation_attempt.as_ref().unwrap().terminal,
            Some(Err(_))
        ));
    }
}

#[test]
fn document_addressing_debug_is_structural_and_deterministic() {
    let (tab, _) = addressing_tab(
        "<!doctype html><base href='https://user:secret-password@secret.test:8443/private?q=secret#hidden'>",
    );
    let snapshot = tab.page.document_address_debug_snapshot().unwrap();
    assert_eq!(
        snapshot,
        tab.page.document_address_debug_snapshot().unwrap()
    );
    assert!(snapshot.contains("outcome: Resolved"));
    for secret in [
        "secret-password",
        "secret.test",
        "private",
        "q=secret",
        "hidden",
        "8443",
    ] {
        assert!(!snapshot.contains(secret), "{snapshot}");
    }
}

#[test]
fn document_addressing_publication_rejections_preserve_committed_state_and_scope_cleanup() {
    use super::super::state::{DocumentParseStatus, StylesheetLoadPhase};

    for rejection in ["identity", "version", "mode", "replacement", "stale"] {
        let (mut tab, rx) = addressing_tab(concat!(
            "<!doctype html><base id=b href='/original/'><img id=i src=image.png>",
            "<style>p{color:blue}</style>",
            "<link rel=stylesheet href='loaded.css#complete'>",
            "<link rel=stylesheet href='site.css?a=1&a=2#pending'><p>preview</p>",
        ));
        let fetched: Vec<_> = rx
            .try_iter()
            .filter_map(|command| match command {
                CoreCommand::FetchStream {
                    request: fetch_request,
                    stylesheet_slot_id: Some(slot),
                    kind: ResourceKind::Css,
                    ..
                } => {
                    let request_id = fetch_request.request_id();
                    let url = fetch_request.legacy_issued_url(ResourceKind::Css).unwrap();
                    Some((slot, request_id, url))
                }
                _ => None,
            })
            .collect();
        assert_eq!(fetched.len(), 2);
        let (loaded, generation, loaded_url) = fetched[0].clone();
        let (unfinished, unfinished_generation, issued_url) = fetched[1].clone();
        assert_eq!(
            generation,
            tab.active_document_request_id().expect("admitted fixture")
        );
        assert_eq!(unfinished_generation, generation);
        assert_eq!(issued_url, "https://example.test/original/site.css?a=1&a=2");
        complete_css(&mut tab, loaded, &loaded_url, "p{color:red}");
        rx.try_iter().for_each(drop);
        let response = net::NetworkResponseInfo::http(
            tab.page.stylesheet_address(unfinished).unwrap(),
            issued_url.clone(),
            Some(issued_url.clone()),
            net::HttpStatus::try_from(200).unwrap(),
            net::HttpHeaders::new(),
        )
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: generation,
            stylesheet_slot_id: Some(unfinished),
            kind: ResourceKind::Css,
            response: response.clone(),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: generation,
            stylesheet_slot_id: Some(unfinished),
            kind: ResourceKind::Css,
            url: issued_url.clone(),
            bytes: b"p{color:".to_vec(),
        })
        .unwrap();
        // AJ7 buffers raw acquisition until authenticated completion.

        assert!(rx.try_recv().is_err());
        assert_eq!(tab.page.pending_count(), 1);
        assert_eq!(tab.page.css_stylesheets().len(), 2);
        assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));

        let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
        let image = element_id(tab.page.dom.as_deref().unwrap(), "i");
        let before = tab.page.addresses.clone();
        let handle = tab.dom_handle.unwrap();
        let version = tab.dom_version;
        let dom_before = format!("{:?}", (&tab.page.dom, tab.dom_store.get_current(handle)));
        let status_before = tab.last_status.clone();
        let image_before =
            crate::page::selected_image_source(&tab.page.image_sources, image, "image.png");
        let slots = tab.page.active_external_stylesheet_slots();
        let mut publication = no_quirks_patch_publication(
            handle,
            version,
            version.next(),
            vec![attributes(base, &[("href", "/rejected/")])],
        );
        let result = match rejection {
            "identity" => tab.commit_document_publication_with_forced_identity_failure_for_tests(
                publication,
                tab.active_document_request_id().expect("admitted fixture"),
                PatchKey::from_id(base),
            ),
            "version" => {
                let bus::DocumentPublicationPayload::Patch { from, .. } = &mut publication.payload;
                *from = DomVersion::INITIAL;
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: tab.tab_id,
                    request_id: generation,
                    publication,
                })
            }
            "replacement" => {
                let output = html::parse_document(
                    "<!doctype html><base href='/rejected/'><link rel=stylesheet href=speculative.css><p>replacement</p>",
                    Default::default(),
                )
                .unwrap();
                publication = no_quirks_patch_publication_from_output(output);
                let bus::DocumentPublicationPayload::Patch { patches, .. } =
                    &mut publication.payload;
                patches.push(DomPatch::RemoveNode {
                    key: PatchKey(u32::MAX),
                });
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: tab.tab_id,
                    request_id: generation,
                    publication,
                })
            }
            _ => {
                publication.document_mode = html::DocumentMode::Quirks;
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: tab.tab_id,
                    request_id: if rejection == "stale" {
                        generation - 1
                    } else {
                        generation
                    },
                    publication,
                })
            }
        };
        if rejection == "stale" {
            assert!(
                result.is_ok(),
                "stale publications are ignored before validation"
            );
        } else {
            assert!(result.is_err(), "{rejection}");
        }
        assert_eq!(tab.page.addresses, before);
        assert_eq!(tab.page.document_url().unwrap().as_str(), ADDRESS_URL);
        assert_eq!(
            tab.page.active_base_url().unwrap().as_str(),
            "https://example.test/original/"
        );
        assert_eq!(tab.dom_handle, Some(handle));
        assert_eq!(tab.dom_version, version);
        assert_eq!(
            format!("{:?}", (&tab.page.dom, tab.dom_store.get_current(handle))),
            dom_before
        );
        assert_eq!(
            crate::page::selected_image_source(&tab.page.image_sources, image, "image.png"),
            image_before
        );
        assert_eq!(tab.page.active_external_stylesheet_slots(), slots);
        assert_eq!(tab.page.css_stylesheets().len(), 2);
        assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
        let commands: Vec<_> = rx.try_iter().collect();
        assert!(
            commands
                .iter()
                .all(|command| !matches!(command, CoreCommand::FetchStream { .. }))
        );

        // The direct staging probe never invokes the terminal failure wrapper;
        // an old-generation event never reaches either staging or that wrapper.
        if matches!(rejection, "identity" | "stale") {
            assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
            assert_eq!(tab.last_status, status_before);
            assert_eq!(tab.page.pending_count(), 1);
            assert_eq!(tab.stylesheet_loads.len(), 1);
            assert!(matches!(
                tab.stylesheet_loads[&unfinished].phase,
                StylesheetLoadPhase::Fetching(_)
            ));
            assert_eq!(tab.stylesheet_loads[&unfinished].request_id, generation);
            assert_eq!(tab.stylesheet_loads[&unfinished].issued_key(), issued_url);
            assert!(commands.is_empty());
            continue;
        }

        assert!(matches!(
            &tab.document_load.parse_status,
            DocumentParseStatus::Failed(_)
        ));
        assert!(!tab.loading);
        assert_eq!(tab.page.pending_count(), 0);
        assert!(!tab.stylesheet_loads.contains_key(&unfinished));
        assert!(tab.stylesheet_loads.is_empty());
        assert_eq!(
            tab.page.stylesheet_installation_result(loaded),
            Some(Ok(()))
        );
        // Only unfinished work is retired. Speculative replacement retirement
        // would also abort the completed request and remove its preview style.
        assert_eq!(commands.len(), 1, "{rejection}: {commands:?}");
        assert!(
            commands
                .iter()
                .all(|command| !matches!(command, CoreCommand::CssAbort { .. }))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, CoreCommand::CancelRequest {
            tab_id, request_id,
        } if *tab_id == tab.tab_id && *request_id == generation))
        );

        for late in [
            CoreEvent::NetworkStart {
                tab_id: tab.tab_id,
                request_id: generation,
                stylesheet_slot_id: Some(unfinished),
                kind: ResourceKind::Css,
                response: response.clone(),
            },
            CoreEvent::NetworkChunk {
                tab_id: tab.tab_id,
                request_id: generation,
                stylesheet_slot_id: Some(unfinished),
                kind: ResourceKind::Css,
                url: issued_url.clone(),
                bytes: b"green}".to_vec(),
            },
            CoreEvent::NetworkDone {
                tab_id: tab.tab_id,
                request_id: generation,
                stylesheet_slot_id: Some(unfinished),
                kind: ResourceKind::Css,
                response,
                body: net::BodyProgress::try_new(ResourceKind::Css, 14).unwrap(),
            },
            CoreEvent::CssDecodedBlock {
                tab_id: tab.tab_id,
                request_id: generation,
                stylesheet_slot_id: unfinished,
                url: issued_url.clone(),
                css_block: "p{color:green}".into(),
            },
            CoreEvent::CssSheetDone {
                tab_id: tab.tab_id,
                request_id: generation,
                stylesheet_slot_id: unfinished,
                url: issued_url,
            },
        ] {
            tab.on_core_event(late).unwrap();
        }
        assert!(!tab.stylesheet_loads.contains_key(&unfinished));
        assert_eq!(tab.page.pending_count(), 0);
        assert_eq!(tab.page.css_stylesheets().len(), 2);
        assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
        assert!(rx.try_recv().is_err(), "retired traffic must remain inert");
    }
}

#[test]
fn document_addressing_history_preserves_valid_and_fallback_frozen_bases() {
    for href in ["", "http://[", "/assets/"] {
        let (mut tab, rx) = addressing_tab(&format!("<!doctype html><base href='{href}'>"));
        let frozen = tab.page.active_base_url().unwrap().clone();
        tab.activate_link("https://example.test/dir/page?q=1#one")
            .unwrap();
        tab.activate_link("https://example.test/dir/page?q=1#two")
            .unwrap();
        tab.go_back().unwrap();
        assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
        tab.go_forward().unwrap();
        assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
        assert_eq!(tab.page.active_base_url(), Some(&frozen));
        assert_eq!(tab.active_document_request_id(), Some(1));
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn document_addressing_production_pointer_activation_resolves_authored_href_in_browser() {
    for (href, rejected) in [("child", false), ("custom:opaque", true)] {
        let (mut tab, rx) = addressing_tab(&format!(
            "<!doctype html><base href='/assets/'><body><a id=link href='{href}'>next</a>"
        ));
        let active_id = tab.active_document_request_id();
        let active_status = tab.last_status.clone();
        let ctx = egui::Context::default();
        let raw = |events| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 400.0),
            )),
            events,
            ..Default::default()
        };
        for _ in 0..2 {
            let _ = ctx.run(raw(vec![]), |ctx| {
                tab.ui_content(ctx).unwrap();
            });
        }
        let link_id = element_id(tab.page.dom.as_deref().unwrap(), "link");
        let pos = {
            let retained = tab.page.retained_layout_artifact().unwrap().clone();
            let style = tab.page.build_style_phase_output().unwrap().unwrap();
            let layout = retained.materialize(style.root()).unwrap();
            let measurer = gfx::text_measurer::EguiTextMeasurer::new(&ctx);
            (0..100)
                .find_map(|y| {
                    (0..200).find_map(|x| {
                        let hit = layout::hit_test::hit_test(
                            layout.root(),
                            (x as f32, y as f32),
                            &measurer,
                        )?;
                        (hit.node_id == link_id && hit.kind == layout::HitKind::Link)
                            .then_some(egui::pos2(x as f32, y as f32))
                    })
                })
                .expect("production layout exposes a link hit region")
        };
        for pressed in [true, false] {
            let _ = ctx.run(
                raw(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]),
                |ctx| {
                    tab.ui_content(ctx).unwrap();
                },
            );
        }
        if rejected {
            assert_eq!(
                tab.visible_load_status(),
                Some(format!(
                    "Navigation submission: Fetch request rejected: UnsupportedScheme • {}",
                    active_status.as_deref().unwrap()
                ))
            );
            assert_eq!(tab.last_status, active_status);
            assert_eq!(tab.active_document_request_id(), active_id);
            assert!(tab.document_load.failure().is_none());
            assert!(rx.try_recv().is_err());
        } else {
            assert!(rx.try_iter().any(|command|matches!(command,CoreCommand::FetchStream { request: fetch_request, kind:ResourceKind::Html,..} if fetch_request.legacy_issued_url(ResourceKind::Html).unwrap()=="https://example.test/assets/child")));
        }
    }
}
