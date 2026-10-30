use super::support::*;
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkResponseInfo, ResourceKind, StylesheetSlotId};
use html::{DomPatch, PatchKey};

fn selected(tab: &super::super::Tab, owner: html::internal::Id, source: &str) -> Option<String> {
    crate::page::selected_image_source(&tab.page.image_sources, owner, source)
}
fn requests(
    rx: &std::sync::mpsc::Receiver<CoreCommand>,
    kind: ResourceKind,
) -> Vec<(Option<StylesheetSlotId>, String)> {
    rx.try_iter()
        .filter_map(|command| match command {
            CoreCommand::FetchStream {
                kind: actual,
                stylesheet_slot_id,
                url,
                ..
            } if kind.as_str() == actual.as_str() => Some((stylesheet_slot_id, url)),
            _ => None,
        })
        .collect()
}

#[test]
fn resource_associations_images_retain_selective_owners_and_retire_removed_nodes() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=b href='/old/'><body id=body><div id=container><img id=a src=same.png></div><img id=failed src='http://['>",
    );
    let dom = tab.page.dom.as_deref().unwrap();
    let base = element_id(dom, "b");
    let image = element_id(dom, "a");
    let failed = element_id(dom, "failed");
    let body = element_id(dom, "body");
    let container = element_id(dom, "container");
    assert_eq!(requests(&rx, ResourceKind::Image).len(), 1);
    assert_eq!(
        selected(&tab, image, "same.png").as_deref(),
        Some("https://example.test/old/same.png")
    );
    publish_addressing_patches(
        &mut tab,
        vec![
            attributes(base, &[("id", "b"), ("href", "/new/")]),
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey::from_id(image),
            },
        ],
    )
    .unwrap();
    assert!(requests(&rx, ResourceKind::Image).is_empty());
    assert_eq!(
        selected(&tab, image, "same.png").as_deref(),
        Some("https://example.test/old/same.png")
    );
    assert_eq!(selected(&tab, failed, "http://["), None);
    assert_eq!(selected(&tab, image, "mismatched.png"), None);
    assert_eq!(selected(&tab, html::internal::Id(999), "same.png"), None);
    publish_addressing_patches(
        &mut tab,
        vec![
            DomPatch::CreateElement {
                key: PatchKey(900),
                name: html::internal::html_name("img"),
                attributes: vec![html::internal::unqualified_attribute("src", "same.png")],
            },
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey(900),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        requests(&rx, ResourceKind::Image)[0].1,
        "https://example.test/new/same.png"
    );
    assert_eq!(
        selected(&tab, html::internal::Id(900), "same.png").as_deref(),
        Some("https://example.test/new/same.png")
    );
    publish_addressing_patches(
        &mut tab,
        vec![attributes(image, &[("id", "a"), ("src", "changed.png")])],
    )
    .unwrap();
    assert_eq!(
        requests(&rx, ResourceKind::Image)[0].1,
        "https://example.test/new/changed.png"
    );
    tab.resources.on_network_error(
        "https://example.test/new/changed.png",
        "fixture failure".into(),
    );
    publish_addressing_patches(
        &mut tab,
        vec![attributes(base, &[("id", "b"), ("href", "/latest/")])],
    )
    .unwrap();
    assert_eq!(
        selected(&tab, image, "changed.png").as_deref(),
        Some("https://example.test/new/changed.png")
    );
    assert!(requests(&rx, ResourceKind::Image).is_empty());
    publish_addressing_patches(
        &mut tab,
        vec![
            DomPatch::RemoveNode {
                key: PatchKey::from_id(image),
            },
            DomPatch::RemoveNode {
                key: PatchKey::from_id(container),
            },
            attributes(failed, &[("id", "failed")]),
        ],
    )
    .unwrap();
    assert_eq!(tab.page.image_sources.len(), 1);
    assert_eq!(selected(&tab, image, "changed.png"), None);
    // Failed resolution is a retained association, rather than permission to
    // try the same source again whenever a base changes.
    assert!(!tab.page.image_sources.contains_key(&failed));
}

#[test]
fn resource_associations_removed_images_do_not_accumulate_and_recreated_owners_use_current_base() {
    let (mut tab, rx) = addressing_tab("<!doctype html><body id=body>");
    let body = element_id(tab.page.dom.as_deref().unwrap(), "body");
    for key in 100..140 {
        publish_addressing_patches(
            &mut tab,
            vec![
                DomPatch::CreateElement {
                    key: PatchKey(key),
                    name: html::internal::html_name("img"),
                    attributes: vec![html::internal::unqualified_attribute("src", "same.png")],
                },
                DomPatch::AppendChild {
                    parent: PatchKey::from_id(body),
                    child: PatchKey(key),
                },
            ],
        )
        .unwrap();
        assert_eq!(tab.page.image_sources.len(), 1);
        publish_addressing_patches(&mut tab, vec![DomPatch::RemoveNode { key: PatchKey(key) }])
            .unwrap();
        assert!(tab.page.image_sources.is_empty());
    }
    assert_eq!(
        requests(&rx, ResourceKind::Image).len(),
        1,
        "existing cache identity deduplicates equal URLs across owners"
    );
}

#[test]
fn resource_associations_stylesheet_moves_media_source_changes_and_retired_responses() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=b href='/old/'><link id=a rel=stylesheet href=same.css><link id=c rel=stylesheet href=same.css><body id=body>",
    );
    let dom = tab.page.dom.as_deref().unwrap();
    let base = element_id(dom, "b");
    let a = element_id(dom, "a");
    let c = element_id(dom, "c");
    let body = element_id(dom, "body");
    let fetched = requests(&rx, ResourceKind::Css);
    assert_eq!(fetched.len(), 2);
    let first = fetched[0].0.unwrap();
    let second = fetched[1].0.unwrap();
    assert_ne!(first, second);
    tab.on_css_network_start(
        first,
        NetworkResponseInfo {
            requested_url: fetched[0].1.clone(),
            final_url: fetched[0].1.clone(),
            status_code: Some(200),
            content_type: Some("text/css".into()),
        },
    );
    tab.on_css_decoded_block(first, "p{color:red}".into());
    publish_addressing_patches(
        &mut tab,
        vec![
            attributes(base, &[("id", "b"), ("href", "/new/")]),
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey::from_id(a),
            },
            attributes(
                c,
                &[
                    ("id", "c"),
                    ("rel", "stylesheet"),
                    ("href", "same.css"),
                    ("media", "screen"),
                ],
            ),
        ],
    )
    .unwrap();
    assert!(requests(&rx, ResourceKind::Css).is_empty());
    assert!(tab.page.has_external_stylesheet_slot(first));
    assert!(tab.page.has_external_stylesheet_slot(second));
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            a,
            &[("id", "a"), ("rel", "stylesheet"), ("href", "changed.css")],
        )],
    )
    .unwrap();
    let changed = requests(&rx, ResourceKind::Css);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].1, "https://example.test/new/changed.css");
    assert_ne!(changed[0].0, Some(first));
    assert!(!tab.page.has_external_stylesheet_slot(first));
    assert!(!tab.stylesheet_loads.contains_key(&first));
    tab.on_css_network_start(
        first,
        NetworkResponseInfo {
            requested_url: fetched[0].1.clone(),
            final_url: fetched[0].1.clone(),
            status_code: Some(200),
            content_type: None,
        },
    );
    tab.on_css_decoded_block(first, "p{color:blue}".into());
    assert!(!tab.stylesheet_loads.contains_key(&first));
    assert!(tab.page.css_stylesheets().is_empty());
    publish_addressing_patches(
        &mut tab,
        vec![
            DomPatch::RemoveNode {
                key: PatchKey::from_id(c),
            },
            attributes(a, &[("id", "a"), ("href", "changed.css")]),
        ],
    )
    .unwrap();
    assert!(tab.page.active_external_stylesheet_slots().is_empty());
}

#[test]
fn resource_associations_clear_and_replacement_never_reuse_outstanding_slots() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base href='/old/'><link rel=stylesheet href=same.css><img src=same.png>",
    );
    let old = requests(&rx, ResourceKind::Css)[0].0.unwrap();
    let output = html::parse_document(
        "<!doctype html><base href='/new/'><link rel=stylesheet href=same.css><img src=same.png>",
        Default::default(),
    )
    .unwrap();
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
    let new = requests(&rx, ResourceKind::Css)[0].0.unwrap();
    assert_ne!(old, new);
    assert_eq!(tab.page.image_sources.len(), 1);
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: old,
        url: "https://example.test/old/same.css".into(),
        css_block: "p{color:red}".into(),
    })
    .unwrap();
    assert!(tab.page.css_stylesheets().is_empty());
    let output = html::parse_document(
        "<!doctype html><link rel=stylesheet href=same.css>",
        Default::default(),
    )
    .unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: 7,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    let replaced = requests(&rx, ResourceKind::Css)[0].0.unwrap();
    assert_ne!(new, replaced);
    assert!(tab.page.image_sources.is_empty());
    tab.on_css_decoded_block(new, "p{color:blue}".into());
    assert!(tab.page.css_stylesheets().is_empty());
}

#[test]
fn resource_associations_failed_stylesheet_does_not_retry_on_base_only_change() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=b href='about:blank'><link rel=stylesheet href=relative.css>",
    );
    assert!(requests(&rx, ResourceKind::Css).is_empty());
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            base,
            &[("id", "b"), ("href", "https://other.test/")],
        )],
    )
    .unwrap();
    assert!(requests(&rx, ResourceKind::Css).is_empty());
    assert!(tab.page.active_external_stylesheet_slots().is_empty());
}

#[test]
fn resource_associations_retirement_aborts_partial_css_and_late_events_cannot_restart_it() {
    let (mut tab, rx) =
        addressing_tab("<!doctype html><link id=sheet rel=stylesheet href=site.css>");
    let fetched = requests(&rx, ResourceKind::Css);
    let slot = fetched[0].0.unwrap();
    let url = fetched[0].1.clone();
    let owner = element_id(tab.page.dom.as_deref().unwrap(), "sheet");
    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(200),
        content_type: Some("text/css".into()),
    };
    tab.on_css_network_start(slot, response.clone());
    tab.on_css_network_chunk(slot, url.clone(), b"p{".to_vec(), 7);
    assert!(
        matches!(rx.try_recv().unwrap(),CoreCommand::CssChunk{stylesheet_slot_id,..} if stylesheet_slot_id==slot)
    );
    publish_addressing_patches(
        &mut tab,
        vec![DomPatch::RemoveNode {
            key: PatchKey::from_id(owner),
        }],
    )
    .unwrap();
    assert!(
        matches!(rx.try_recv().unwrap(),CoreCommand::CssAbort{stylesheet_slot_id,request_id:7,url:aborted,..} if stylesheet_slot_id==slot && aborted==url)
    );
    assert!(rx.try_recv().is_err());
    assert!(tab.stylesheet_loads.is_empty());
    tab.on_css_network_start(slot, response.clone());
    tab.on_css_network_chunk(slot, url.clone(), b"color:red}".to_vec(), 7);
    tab.on_css_network_done(slot, response, 12, 7);
    assert!(rx.try_recv().is_err());
    assert!(tab.stylesheet_loads.is_empty());
}

#[test]
fn resource_associations_recreated_sources_use_new_owners_and_current_base() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=b href='/old/'><link id=sheet rel=stylesheet href=same.css><body id=body><img id=image src=same.png>",
    );
    let fetched = requests(&rx, ResourceKind::Css);
    let old_slot = fetched[0].0.unwrap();
    let dom = tab.page.dom.as_deref().unwrap();
    let base = element_id(dom, "b");
    let body = element_id(dom, "body");
    let sheet = element_id(dom, "sheet");
    let image = element_id(dom, "image");
    publish_addressing_patches(
        &mut tab,
        vec![
            DomPatch::RemoveNode {
                key: PatchKey::from_id(sheet),
            },
            DomPatch::RemoveNode {
                key: PatchKey::from_id(image),
            },
            attributes(base, &[("id", "b"), ("href", "/new/")]),
            DomPatch::CreateElement {
                key: PatchKey(900),
                name: html::internal::html_name("img"),
                attributes: vec![html::internal::unqualified_attribute("src", "same.png")],
            },
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey(900),
            },
            DomPatch::CreateElement {
                key: PatchKey(901),
                name: html::internal::html_name("link"),
                attributes: vec![
                    html::internal::unqualified_attribute("rel", "stylesheet"),
                    html::internal::unqualified_attribute("href", "same.css"),
                ],
            },
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey(901),
            },
        ],
    )
    .unwrap();
    let commands: Vec<_> = rx.try_iter().collect();
    assert!(commands.iter().any(|command|matches!(command,CoreCommand::FetchStream{stylesheet_slot_id:Some(slot),url,kind:ResourceKind::Css,..} if *slot!=old_slot && url=="https://example.test/new/same.css")));
    assert!(commands.iter().any(|command|matches!(command,CoreCommand::FetchStream{url,kind:ResourceKind::Image,..} if url=="https://example.test/new/same.png")));
    assert!(!tab.page.image_sources.contains_key(&image));
    assert_eq!(tab.page.image_sources.len(), 1);
}

#[test]
fn resource_associations_reference_before_base_obeys_publication_discovery_boundary() {
    let (tab, rx) = addressing_tab(
        "<!doctype html><body><img id=early src=early.png><base href='/assets/'><img src=later.png>",
    );
    let fetched = requests(&rx, ResourceKind::Image);
    assert_eq!(
        fetched
            .iter()
            .map(|(_, url)| url.as_str())
            .collect::<Vec<_>>(),
        vec![
            "https://example.test/assets/early.png",
            "https://example.test/assets/later.png"
        ]
    );
    let early = element_id(tab.page.dom.as_deref().unwrap(), "early");
    assert_eq!(
        selected(&tab, early, "early.png").as_deref(),
        Some("https://example.test/assets/early.png")
    );
    let (mut tab, rx) = addressing_tab("<!doctype html><body id=body><img id=early src=early.png>");
    assert_eq!(
        requests(&rx, ResourceKind::Image)[0].1,
        "https://example.test/dir/early.png"
    );
    let body = element_id(tab.page.dom.as_deref().unwrap(), "body");
    let early = element_id(tab.page.dom.as_deref().unwrap(), "early");
    publish_addressing_patches(
        &mut tab,
        vec![
            DomPatch::CreateElement {
                key: PatchKey(900),
                name: html::internal::html_name("base"),
                attributes: vec![html::internal::unqualified_attribute("href", "/assets/")],
            },
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey(900),
            },
            DomPatch::CreateElement {
                key: PatchKey(901),
                name: html::internal::html_name("img"),
                attributes: vec![html::internal::unqualified_attribute("src", "later.png")],
            },
            DomPatch::AppendChild {
                parent: PatchKey::from_id(body),
                child: PatchKey(901),
            },
        ],
    )
    .unwrap();
    assert_eq!(
        requests(&rx, ResourceKind::Image),
        vec![(None, "https://example.test/assets/later.png".into())]
    );
    assert_eq!(
        selected(&tab, early, "early.png").as_deref(),
        Some("https://example.test/dir/early.png")
    );
}

#[test]
fn resource_associations_streamed_css_uses_one_live_record_lookup_per_chunk() {
    for attachments in [64, 2048] {
        let mut source = String::from("<!doctype html>");
        for i in 0..attachments {
            source.push_str(&format!("<link id=s{i} rel=stylesheet href=s{i}.css>"));
        }
        let (mut tab, rx) = addressing_tab(&source);
        let fetched = requests(&rx, ResourceKind::Css);
        let (slot, url) = fetched.last().unwrap();
        let slot = slot.unwrap();
        let response = NetworkResponseInfo {
            requested_url: url.clone(),
            final_url: url.clone(),
            status_code: Some(200),
            content_type: Some("text/css".into()),
        };
        tab.on_css_network_start(slot, response);
        let before = tab.page.stylesheet_slot_lookup_count();
        for _ in 0..4096 {
            tab.on_css_network_chunk(slot, url.clone(), b"p{}".to_vec(), 7);
        }
        assert_eq!(
            tab.page.stylesheet_slot_lookup_count() - before,
            4096,
            "attachments={attachments}"
        );
        assert_eq!(
            rx.try_iter()
                .filter(|cmd| matches!(cmd, CoreCommand::CssChunk { .. }))
                .count(),
            4096
        );
        let owner = element_id(
            tab.page.dom.as_deref().unwrap(),
            &format!("s{}", attachments - 1),
        );
        publish_addressing_patches(
            &mut tab,
            vec![DomPatch::RemoveNode {
                key: PatchKey::from_id(owner),
            }],
        )
        .unwrap();
        rx.try_iter().for_each(drop);
        let before = tab.page.stylesheet_slot_lookup_count();
        for _ in 0..4096 {
            tab.on_css_network_chunk(slot, url.clone(), b"late".to_vec(), 7);
        }
        assert_eq!(tab.page.stylesheet_slot_lookup_count() - before, 4096);
        assert!(
            rx.try_recv().is_err(),
            "retired chunks are rejected before forwarding"
        );
    }
}

fn publish_network_document(tab: &mut super::super::Tab, url: &str, markup: &str) {
    let response = NetworkResponseInfo {
        requested_url: url.into(),
        final_url: url.into(),
        status_code: Some(200),
        content_type: Some("text/html".into()),
    };
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
    })
    .unwrap();
    let output = html::parse_document(markup, Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
}

fn css_response(url: &str) -> NetworkResponseInfo {
    NetworkResponseInfo {
        requested_url: url.into(),
        final_url: url.into(),
        status_code: Some(200),
        content_type: Some("text/css".into()),
    }
}

fn stylesheet_fetch(commands: &[CoreCommand]) -> (StylesheetSlotId, String) {
    let fetches: Vec<_> = commands
        .iter()
        .filter_map(|command| match command {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                url,
                kind: ResourceKind::Css,
                ..
            } => Some((*slot, url.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(fetches.len(), 1, "commands: {commands:?}");
    fetches.into_iter().next().unwrap()
}

fn feed_partial_stylesheet(
    tab: &mut super::super::Tab,
    rx: &std::sync::mpsc::Receiver<CoreCommand>,
    runtime: &std::sync::mpsc::Sender<CoreCommand>,
    slot: StylesheetSlotId,
    url: &str,
) {
    let request_id = tab.nav_gen;
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: css_response(url),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.into(),
        bytes: b"p{color:red".to_vec(),
    })
    .unwrap();
    let chunk = rx
        .try_recv()
        .expect("Browser forwards the partial CSS chunk");
    assert!(
        matches!(&chunk, CoreCommand::CssChunk { request_id: id, stylesheet_slot_id, .. } if *id == request_id && *stylesheet_slot_id == slot)
    );
    runtime.send(chunk).unwrap();
}

#[test]
fn resource_associations_runtime_retires_partial_css_on_document_lifecycle_boundaries() {
    use std::sync::mpsc;
    use std::time::Duration;

    for boundary in [
        "navigation",
        "refresh",
        "back",
        "forward",
        "clear",
        "replacement",
    ] {
        let (mut tab, rx) = addressing_tab("<!doctype html><link rel=stylesheet href=old.css>");
        let (runtime_tx, runtime_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        runtime_css::start_css_runtime(runtime_rx, events_tx);
        let mut commands: Vec<_> = rx.try_iter().collect();

        if matches!(boundary, "back" | "forward") {
            let second = "https://example.test/second";
            tab.navigate_to_new(second.into());
            rx.try_iter().for_each(drop);
            publish_network_document(
                &mut tab,
                second,
                "<!doctype html><link rel=stylesheet href=old.css>",
            );
            commands = rx.try_iter().collect();
        }
        if boundary == "forward" {
            tab.go_back();
            rx.try_iter().for_each(drop);
            publish_network_document(
                &mut tab,
                ADDRESS_URL,
                "<!doctype html><link rel=stylesheet href=old.css>",
            );
            commands = rx.try_iter().collect();
        }
        let (old_slot, old_url) = stylesheet_fetch(&commands);
        let old_generation = tab.nav_gen;
        feed_partial_stylesheet(&mut tab, &rx, &runtime_tx, old_slot, &old_url);

        match boundary {
            "navigation" => tab.navigate_to_new("https://example.test/third".into()),
            "refresh" => tab.refresh(),
            "back" => tab.go_back(),
            "forward" => tab.go_forward(),
            "clear" => {
                let output = html::parse_document(
                    "<!doctype html><link rel=stylesheet href=new.css>",
                    Default::default(),
                )
                .unwrap();
                publish_addressing_patches(
                    &mut tab,
                    std::iter::once(DomPatch::Clear)
                        .chain(
                            output
                                .patches
                                .into_iter()
                                .filter(|patch| !matches!(patch, DomPatch::Clear)),
                        )
                        .collect(),
                )
                .unwrap();
            }
            "replacement" => {
                let old_handle = tab.dom_handle;
                let output = html::parse_document(
                    "<!doctype html><link rel=stylesheet href=new.css>",
                    Default::default(),
                )
                .unwrap();
                tab.on_core_event(CoreEvent::DomPatchUpdate {
                    tab_id: tab.tab_id,
                    request_id: old_generation,
                    publication: no_quirks_patch_publication_from_output(output),
                })
                .unwrap();
                assert_ne!(tab.dom_handle, old_handle);
            }
            _ => unreachable!(),
        }
        let commands: Vec<_> = rx.try_iter().collect();
        let aborts: Vec<_> = commands
            .iter()
            .enumerate()
            .filter(|(_, command)| matches!(command, CoreCommand::CssAbort { request_id, stylesheet_slot_id, url, .. } if *request_id == old_generation && *stylesheet_slot_id == old_slot && url == &old_url))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(
            aborts.len(),
            1,
            "boundary={boundary}, commands={commands:?}"
        );
        if !matches!(boundary, "clear" | "replacement") {
            let fetch = commands
                .iter()
                .position(|command| {
                    matches!(
                        command,
                        CoreCommand::FetchStream {
                            kind: ResourceKind::Html,
                            ..
                        }
                    )
                })
                .unwrap();
            assert!(aborts[0] < fetch, "retire before new document fetch");
        } else {
            assert_eq!(tab.nav_gen, old_generation);
            let fetch = commands
                .iter()
                .position(|command| {
                    matches!(
                        command,
                        CoreCommand::FetchStream {
                            kind: ResourceKind::Css,
                            ..
                        }
                    )
                })
                .expect("replacement discovers new stylesheet");
            assert!(
                aborts[0] < fetch,
                "retire before replacement stylesheet fetch"
            );
        }
        let mut next_fetch = None;
        let mut new_stylesheet = None;
        for command in commands {
            match command {
                command @ CoreCommand::CssAbort { .. } => runtime_tx.send(command).unwrap(),
                CoreCommand::FetchStream {
                    url,
                    kind: ResourceKind::Html,
                    ..
                } => next_fetch = Some(url),
                CoreCommand::FetchStream {
                    stylesheet_slot_id: Some(slot),
                    url,
                    kind: ResourceKind::Css,
                    ..
                } => new_stylesheet = Some((slot, url)),
                _ => {}
            }
        }

        // FIFO CssDone is a deterministic barrier: a retained partial buffer
        // would first emit CssDecodedBlock, whereas an aborted key emits only
        // CssSheetDone. No sleeps or timeout-as-success checks are needed.
        runtime_tx
            .send(CoreCommand::CssDone {
                tab_id: tab.tab_id,
                request_id: old_generation,
                stylesheet_slot_id: old_slot,
                url: old_url.clone(),
            })
            .unwrap();
        let old_done = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("CSS runtime barrier");
        assert!(
            matches!(&old_done, CoreEvent::CssSheetDone { request_id, stylesheet_slot_id, .. } if *request_id == old_generation && *stylesheet_slot_id == old_slot),
            "boundary={boundary}, event={old_done:?}"
        );
        tab.on_core_event(old_done).unwrap();

        if let Some(url) = next_fetch {
            publish_network_document(
                &mut tab,
                &url,
                "<!doctype html><link rel=stylesheet href=new.css>",
            );
            new_stylesheet = Some(stylesheet_fetch(&rx.try_iter().collect::<Vec<_>>()));
        }
        let (new_slot, new_url) = new_stylesheet.expect("new document stylesheet request");
        assert_ne!(old_slot, new_slot, "slot identities remain monotonic");

        // Both stale generation and retired-slot events are ignored, including
        // Clear and new-handle replacement retain the generation but replace
        // the attachment.
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: old_generation,
            stylesheet_slot_id: Some(old_slot),
            kind: ResourceKind::Css,
            response: css_response(&old_url),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: old_generation,
            stylesheet_slot_id: Some(old_slot),
            kind: ResourceKind::Css,
            url: old_url.clone(),
            bytes: b"late}".to_vec(),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: tab.tab_id,
            request_id: old_generation,
            stylesheet_slot_id: Some(old_slot),
            kind: ResourceKind::Css,
            response: css_response(&old_url),
            bytes_received: 5,
        })
        .unwrap();
        assert!(
            rx.try_recv().is_err(),
            "late retired chunks must not be forwarded"
        );

        let current_generation = tab.nav_gen;
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: current_generation,
            stylesheet_slot_id: Some(new_slot),
            kind: ResourceKind::Css,
            response: css_response(&new_url),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: current_generation,
            stylesheet_slot_id: Some(new_slot),
            kind: ResourceKind::Css,
            url: new_url.clone(),
            bytes: b"p{color:green}".to_vec(),
        })
        .unwrap();
        runtime_tx.send(rx.try_recv().unwrap()).unwrap();
        // Repeated old cleanup is idempotent and cannot remove the new key.
        runtime_tx
            .send(CoreCommand::CssAbort {
                tab_id: tab.tab_id,
                request_id: old_generation,
                stylesheet_slot_id: old_slot,
                url: old_url,
            })
            .unwrap();
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: tab.tab_id,
            request_id: current_generation,
            stylesheet_slot_id: Some(new_slot),
            kind: ResourceKind::Css,
            response: css_response(&new_url),
            bytes_received: 14,
        })
        .unwrap();
        runtime_tx.send(rx.try_recv().unwrap()).unwrap();
        let decoded = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("decoded CSS");
        assert!(
            matches!(&decoded, CoreEvent::CssDecodedBlock { request_id, stylesheet_slot_id, css_block, .. } if *request_id == current_generation && *stylesheet_slot_id == new_slot && css_block == "p{color:green}")
        );
        tab.on_core_event(decoded).unwrap();
        let done = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("stylesheet done");
        assert!(
            matches!(&done, CoreEvent::CssSheetDone { request_id, stylesheet_slot_id, .. } if *request_id == current_generation && *stylesheet_slot_id == new_slot)
        );
        tab.on_core_event(done).unwrap();
        assert_eq!(tab.page.css_stylesheets().len(), 1, "boundary={boundary}");
    }
}

#[test]
fn resource_associations_parser_failure_retires_runtime_buffer_but_keeps_preview_styles() {
    use std::sync::mpsc;
    use std::time::Duration;

    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base href='/assets/'><style>p{color:blue}</style><link rel=stylesheet href=loaded.css><link rel=stylesheet href=old.css><p>preview</p>",
    );
    let (runtime_tx, runtime_rx) = mpsc::channel();
    let (events_tx, events_rx) = mpsc::channel();
    runtime_css::start_css_runtime(runtime_rx, events_tx);
    let fetched = requests(&rx, ResourceKind::Css);
    assert_eq!(fetched.len(), 2);
    let (loaded_slot, loaded_url) = (fetched[0].0.unwrap(), fetched[0].1.clone());
    let (slot, url) = (fetched[1].0.unwrap(), fetched[1].1.clone());
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let base = tab.page.active_base_url().unwrap().clone();
    feed_partial_stylesheet(&mut tab, &rx, &runtime_tx, loaded_slot, &loaded_url);
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(loaded_slot),
        kind: ResourceKind::Css,
        url: loaded_url.clone(),
        bytes: b"}".to_vec(),
    })
    .unwrap();
    runtime_tx.send(rx.try_recv().unwrap()).unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(loaded_slot),
        kind: ResourceKind::Css,
        response: css_response(&loaded_url),
        bytes_received: 12,
    })
    .unwrap();
    runtime_tx.send(rx.try_recv().unwrap()).unwrap();
    let loaded_block = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("loaded CSS decoded");
    assert!(
        matches!(&loaded_block, CoreEvent::CssDecodedBlock { stylesheet_slot_id, css_block, .. } if *stylesheet_slot_id == loaded_slot && css_block == "p{color:red}")
    );
    tab.on_core_event(loaded_block).unwrap();
    let loaded_done = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("loaded CSS complete");
    assert!(
        matches!(&loaded_done, CoreEvent::CssSheetDone { stylesheet_slot_id, .. } if *stylesheet_slot_id == loaded_slot)
    );
    tab.on_core_event(loaded_done).unwrap();
    assert_eq!(tab.page.css_stylesheets().len(), 2);
    feed_partial_stylesheet(&mut tab, &rx, &runtime_tx, slot, &url);

    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: generation,
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(tab.dom_handle, handle);
    assert!(tab.page.dom.is_some());
    assert_eq!(tab.page.active_base_url(), Some(&base));
    assert_eq!(
        tab.page.css_stylesheets().len(),
        2,
        "loaded inline and external styles stay with the preview"
    );

    let commands: Vec<_> = rx.try_iter().collect();
    assert!(commands.iter().any(|command| matches!(command, CoreCommand::CancelRequest { request_id, .. } if *request_id == generation)));
    let aborts = commands.iter().filter(|command| matches!(command, CoreCommand::CssAbort { request_id, stylesheet_slot_id, url: aborted, .. } if *request_id == generation && *stylesheet_slot_id == slot && aborted == &url)).count();
    assert_eq!(
        aborts, 1,
        "partial attachment must be retired on cancellation"
    );
    assert!(commands.iter().all(|command| !matches!(command, CoreCommand::CssAbort { stylesheet_slot_id, .. } if *stylesheet_slot_id == loaded_slot)), "completed attachment must not be cancelled");
    for command in commands {
        if matches!(command, CoreCommand::CssAbort { .. }) {
            runtime_tx.send(command).unwrap();
        }
    }
    runtime_tx
        .send(CoreCommand::CssDone {
            tab_id: tab.tab_id,
            request_id: generation,
            stylesheet_slot_id: slot,
            url: url.clone(),
        })
        .unwrap();
    let barrier = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("CSS runtime barrier");
    assert!(
        matches!(&barrier, CoreEvent::CssSheetDone { request_id, stylesheet_slot_id, .. } if *request_id == generation && *stylesheet_slot_id == slot),
        "old partial CSS must not produce a decoded block: {barrier:?}"
    );
    tab.on_core_event(barrier).unwrap();

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: css_response(&url),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"late}".to_vec(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: css_response(&url),
        bytes_received: 5,
    })
    .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "late CSS events cannot recreate a cancelled buffer"
    );
    assert_eq!(tab.page.css_stylesheets().len(), 2);
}

#[test]
fn resource_associations_completed_css_queued_before_parser_failure_still_applies() {
    use std::sync::mpsc;
    use std::time::Duration;

    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base href='/assets/'><link rel=stylesheet href=site.css><p>preview</p>",
    );
    let (runtime_tx, runtime_rx) = mpsc::channel();
    let (events_tx, events_rx) = mpsc::channel();
    runtime_css::start_css_runtime(runtime_rx, events_tx);
    let (slot, url) = stylesheet_fetch(&rx.try_iter().collect::<Vec<_>>());
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let base = tab.page.active_base_url().unwrap().clone();
    feed_partial_stylesheet(&mut tab, &rx, &runtime_tx, slot, &url);
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"}".to_vec(),
    })
    .unwrap();
    runtime_tx.send(rx.try_recv().unwrap()).unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: css_response(&url),
        bytes_received: 12,
    })
    .unwrap();
    let css_done = rx.try_recv().expect("completed transfer queues CssDone");
    assert!(
        matches!(&css_done, CoreCommand::CssDone { request_id, stylesheet_slot_id, .. } if *request_id == generation && *stylesheet_slot_id == slot)
    );
    runtime_tx.send(css_done).unwrap();

    // The worker's FIFO makes completion deterministic. Do not deliver its
    // decoded events to Browser until after the parser failure is committed.
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: generation,
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    let cancellation: Vec<_> = rx.try_iter().collect();
    assert!(cancellation.iter().any(|command| matches!(command,
        CoreCommand::CancelRequest { request_id, .. } if *request_id == generation
    )));
    assert!(
        cancellation.iter().all(|command| !matches!(command,
            CoreCommand::CssAbort { request_id, stylesheet_slot_id, .. }
                if *request_id == generation && *stylesheet_slot_id == slot
        )),
        "a completed transfer must not be aborted before its queued decode"
    );
    assert_eq!(tab.dom_handle, handle);
    assert!(tab.page.dom.is_some());
    assert_eq!(tab.page.active_base_url(), Some(&base));
    assert!(tab.page.css_stylesheets().is_empty());

    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"late".to_vec(),
    })
    .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "new network chunks after failure remain rejected"
    );

    let decoded = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("queued CSS decode");
    assert!(
        matches!(&decoded, CoreEvent::CssDecodedBlock { request_id, stylesheet_slot_id, css_block, .. }
        if *request_id == generation && *stylesheet_slot_id == slot && css_block == "p{color:red}")
    );
    tab.on_core_event(decoded).unwrap();
    let done = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("queued CSS completion");
    assert!(
        matches!(&done, CoreEvent::CssSheetDone { request_id, stylesheet_slot_id, .. }
        if *request_id == generation && *stylesheet_slot_id == slot)
    );
    tab.on_core_event(done).unwrap();
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.page.active_base_url(), Some(&base));
}

#[test]
fn resource_associations_rejected_css_queued_before_parser_failure_settles_without_style() {
    use std::sync::mpsc;
    use std::time::Duration;

    let (mut tab, rx) =
        addressing_tab("<!doctype html><link rel=stylesheet href=site.css><p>preview</p>");
    let (runtime_tx, runtime_rx) = mpsc::channel();
    let (events_tx, events_rx) = mpsc::channel();
    runtime_css::start_css_runtime(runtime_rx, events_tx);
    let (slot, url) = stylesheet_fetch(&rx.try_iter().collect::<Vec<_>>());
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let response = NetworkResponseInfo {
        content_type: Some("application/javascript".into()),
        ..css_response(&url)
    };
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: response.clone(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"p{color:red}".to_vec(),
    })
    .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "rejected content type forwards no bytes"
    );
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: generation,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response,
        bytes_received: 12,
    })
    .unwrap();
    let css_done = rx.try_recv().expect("completion queues CssDone");
    assert!(
        matches!(&css_done, CoreCommand::CssDone { request_id, stylesheet_slot_id, .. } if *request_id == generation && *stylesheet_slot_id == slot)
    );
    runtime_tx.send(css_done).unwrap();

    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: generation,
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    let cancellation: Vec<_> = rx.try_iter().collect();
    assert!(cancellation.iter().any(|command| matches!(command,
        CoreCommand::CancelRequest { request_id, .. } if *request_id == generation
    )));
    assert!(
        cancellation.iter().all(|command| !matches!(command,
            CoreCommand::CssAbort { request_id, stylesheet_slot_id, .. }
                if *request_id == generation && *stylesheet_slot_id == slot
        )),
        "a transfer whose completion is queued must not be aborted"
    );

    // No bytes reached the runtime, so the FIFO completion event must be
    // CssSheetDone directly; a decoded block would be an erroneous stylesheet.
    let done = events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("queued CSS completion");
    assert!(
        matches!(&done, CoreEvent::CssSheetDone { request_id, stylesheet_slot_id, .. }
        if *request_id == generation && *stylesheet_slot_id == slot)
    );
    tab.on_core_event(done).unwrap();
    assert_eq!(tab.dom_handle, handle);
    assert!(tab.page.dom.is_some());
    assert!(tab.page.css_stylesheets().is_empty());
    assert_eq!(tab.page.pending_count(), 0);
    assert!(
        !tab.page.has_external_stylesheet_slot(slot),
        "rejected stylesheet settles as failed"
    );
}
