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
        assert_eq!(tab.page.fallback_base_url().unwrap().as_str(), ADDRESS_URL);
        assert_eq!(
            tab.page.active_base_url().unwrap().as_str(),
            expected,
            "{markup}"
        );
    }
}

#[test]
fn document_addressing_ordered_changes_and_frozen_lifetime() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=first href='http://['><base id=second href='../second/'>",
    );
    let first = element_id(tab.page.dom.as_deref().unwrap(), "first");
    let second = element_id(tab.page.dom.as_deref().unwrap(), "second");
    tab.activate_link("https://example.test/dir/page?q=1#new");
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
        request_id: 6,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert_eq!(tab.page.addresses, before);
    // A fresh current document is required after the intentional fatal rejection.
    let (mut tab, _) = addressing_tab("<!doctype html><base href='/old/'>");
    let output = html::parse_document("<!doctype html><p>replacement", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: 7,
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
        tab.page.start_nav(ADDRESS_URL);
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
                    request_id: 0,
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
                request_id: 0,
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
    let generation = tab.nav_gen;
    tab.url = "editable address bar text".into();
    tab.activate_link("#one");
    tab.activate_link("#two");
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("initial"));
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    assert_eq!(tab.nav_gen, generation);
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
        tab.activate_link(href);
        assert_eq!(tab.nav_gen, generation);
        assert!(rx.try_recv().is_err());
    }
    let (mut tab, rx) = addressing_tab("<!doctype html><base href='https://other.test/different'>");
    tab.activate_link("#fragment");
    assert_eq!(tab.nav_gen, 8);
    assert!(tab.page.document_url().is_none());
    assert!(rx.try_iter().any(|cmd| matches!(cmd, CoreCommand::FetchStream { url, kind: ResourceKind::Html, .. } if url == "https://other.test/different#fragment")));
    let (mut tab, rx) = addressing_tab("<!doctype html><base href='../assets/'>");
    tab.activate_link("child");
    assert!(rx.try_iter().any(|cmd| matches!(cmd, CoreCommand::FetchStream { url, .. } if url == "https://example.test/assets/child")));
    let mut tab = Tab::new(1);
    tab.activate_link("relative");
    tab.activate_link("https://example.test/absolute");
    tab.activate_link("custom:opaque");
    assert_eq!(tab.nav_gen, 0);
    assert!(tab.page.document_url().is_none());
    tab.navigate_to_new("example.test".into());
    assert_eq!(tab.url, "https://example.test");
}

fn addressing_tab_at(
    document_url: &str,
    base_href: &str,
) -> (Tab, std::sync::mpsc::Receiver<CoreCommand>) {
    addressing_tab_with_url(
        &format!(
            "<!doctype html><base href='{base_href}'><img id=old-image src=old.png><link rel=stylesheet href=old.css>"
        ),
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
        assert_eq!(old_slots.len(), 1);
        let generation = tab.nav_gen;
        tab.activate_link(href);
        assert_eq!(tab.nav_gen, generation + 1, "base={base} href={href:?}");
        assert_eq!(tab.url, destination);
        assert_eq!(
            tab.history.last().map(|entry| entry.url()),
            Some(destination)
        );
        assert!(tab.page.document_url().is_none());
        assert!(tab.page.active_base_url().is_none());
        assert!(tab.page.dom.is_none());
        assert!(tab.page.image_sources.is_empty());
        assert!(tab.page.active_external_stylesheet_slots().is_empty());
        assert!(rx.try_iter().any(|command| matches!(command,
            CoreCommand::FetchStream { request_id, url, kind: ResourceKind::Html, .. }
                if request_id == generation + 1 && url == destination
        )));

        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: generation + 1,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: core_types::NetworkResponseInfo {
                requested_url: destination.into(),
                final_url: destination.into(),
                status_code: Some(200),
                content_type: Some("text/html".into()),
            },
        })
        .unwrap();
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
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let frozen = tab.page.active_base_url().unwrap().clone();
    let old_image = element_id(tab.page.dom.as_deref().unwrap(), "old-image");
    let image = crate::page::selected_image_source(&tab.page.image_sources, old_image, "old.png");
    let slots = tab.page.active_external_stylesheet_slots();
    assert!(image.is_some());
    assert_eq!(slots.len(), 1);

    tab.activate_link("#one");
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.activate_link("#");
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some(""));
    assert_eq!(
        tab.history.last().map(|entry| entry.url()),
        Some("https://old:secret@example.test/dir/page?q=1#")
    );
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some(""));

    assert_eq!(tab.nav_gen, generation);
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
        tab.navigate_to_new("https://example.test/requested".into());
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: core_types::NetworkResponseInfo {
                requested_url: "https://example.test/requested".into(),
                final_url: final_url.into(),
                status_code: Some(200),
                content_type: None,
            },
        })
        .unwrap();
        if final_url.is_empty() {
            assert_eq!(
                tab.page.document_url().unwrap().as_str(),
                "https://example.test/requested"
            );
        } else {
            assert!(tab.page.document_url().is_none());
            assert_eq!(
                tab.document_load.parse_status,
                super::super::state::DocumentParseStatus::Failed
            );
        }
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
fn document_addressing_identity_version_and_mode_rejections_preserve_all_address_state() {
    for rejection in ["identity", "version", "mode"] {
        let (mut tab, rx) = addressing_tab(
            "<!doctype html><base id=b href='/original/'><img id=i src=image.png><link rel=stylesheet href=site.css>",
        );
        rx.try_iter().for_each(drop);
        let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
        let image = element_id(tab.page.dom.as_deref().unwrap(), "i");
        let before = tab.page.addresses.clone();
        let version = tab.dom_version;
        let image_before =
            crate::page::selected_image_source(&tab.page.image_sources, image, "image.png");
        let slots = tab.page.active_external_stylesheet_slots();
        let mut publication = no_quirks_patch_publication(
            tab.dom_handle.unwrap(),
            version,
            version.next(),
            vec![attributes(base, &[("href", "/rejected/")])],
        );
        let result = match rejection {
            "identity" => tab.commit_document_publication_with_forced_identity_failure_for_tests(
                publication,
                tab.nav_gen,
                PatchKey::from_id(base),
            ),
            "version" => {
                let bus::DocumentPublicationPayload::Patch { from, .. } = &mut publication.payload;
                *from = DomVersion::INITIAL;
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: 1,
                    request_id: 7,
                    publication,
                })
            }
            _ => {
                publication.document_mode = html::DocumentMode::Quirks;
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: 1,
                    request_id: 7,
                    publication,
                })
            }
        };
        assert!(result.is_err());
        assert_eq!(tab.page.addresses, before);
        assert_eq!(tab.dom_version, version);
        assert_eq!(
            crate::page::selected_image_source(&tab.page.image_sources, image, "image.png"),
            image_before
        );
        assert_eq!(tab.page.active_external_stylesheet_slots(), slots);
        assert!(
            rx.try_iter()
                .all(|command| !matches!(command, CoreCommand::FetchStream { .. }))
        );
    }
}

#[test]
fn document_addressing_history_preserves_valid_and_fallback_frozen_bases() {
    for href in ["", "http://[", "/assets/"] {
        let (mut tab, rx) = addressing_tab(&format!("<!doctype html><base href='{href}'>"));
        let frozen = tab.page.active_base_url().unwrap().clone();
        tab.activate_link("https://example.test/dir/page?q=1#one");
        tab.activate_link("https://example.test/dir/page?q=1#two");
        tab.go_back();
        assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
        tab.go_forward();
        assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
        assert_eq!(tab.page.active_base_url(), Some(&frozen));
        assert_eq!(tab.nav_gen, 7);
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn document_addressing_production_pointer_activation_resolves_authored_href_in_browser() {
    let (mut tab, rx) =
        addressing_tab("<!doctype html><base href='/assets/'><body><a id=link href=child>next</a>");
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
                    let hit =
                        layout::hit_test::hit_test(layout.root(), (x as f32, y as f32), &measurer)?;
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
    assert!(rx.try_iter().any(|command|matches!(command,CoreCommand::FetchStream{url,kind:ResourceKind::Html,..} if url=="https://example.test/assets/child")));
}
