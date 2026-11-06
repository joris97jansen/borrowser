use super::super::Tab;
use super::support::*;
use crate::page::{HtmlReferenceOutcome, HtmlResourceReference};
use addressing::ResourceKind as K;
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind as TransportKind;
use html::{DomPatch, PatchKey};

fn resolved(record: &HtmlResourceReference) -> &addressing::ResourceAddress {
    let HtmlReferenceOutcome::Resolved { address, .. } = &record.outcome else {
        panic!("expected a resolved occurrence")
    };
    assert_eq!(record.kind, address.kind());
    address
}

#[test]
fn html_resource_inventory_is_observational() {
    let (tab, commands) = addressing_tab(concat!(
        "<!doctype html><link id=sheet rel=stylesheet href='site.css#sheet'>",
        "<body><img id=image src=image.png><a href=next.html>x</a><form action=submit></form>",
        "<audio><source src=sound.ogg></audio><iframe src=frame.html></iframe>",
        "<script src=app.js></script>",
    ));
    // Production publication already performs ordinary image/stylesheet loading.
    // Account for it before observing the new inventory, rather than disabling it.
    let ordinary: Vec<_> = commands.try_iter().collect();
    assert_eq!(ordinary.len(), 2);
    for kind in [TransportKind::Css, TransportKind::Image] {
        assert_eq!(
            ordinary
                .iter()
                .filter(|cmd| matches!(cmd,
                    CoreCommand::FetchStream { kind: actual, .. } if actual.as_str() == kind.as_str()
                ))
                .count(),
            1
        );
    }
    let addresses = tab.page.addresses.clone();
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let loading = tab.loading;
    let status = tab.last_status.clone();
    let load = format!("{:?}", tab.document_load);
    let generations = tab.page.style_generations();
    let pipeline = tab.page.render_pipeline_debug_snapshot();
    let retained = tab.page.retained_render_state_debug_snapshot();
    let pending = format!("{:?}", tab.pending_render_work);
    let history: Vec<_> = tab
        .history()
        .iter()
        .map(|entry| entry.url().to_owned())
        .collect();
    let slots = tab.page.active_external_stylesheet_slots();
    let issued: Vec<_> = tab
        .stylesheet_loads
        .iter()
        .map(|(id, load)| {
            (
                *id,
                load.request_id,
                load.issued_key.clone(),
                format!("{load:?}"),
            )
        })
        .collect();
    let image = element_id(tab.page.dom.as_deref().unwrap(), "image");
    let selected = crate::page::selected_image_source(&tab.page.image_sources, image, "image.png");
    let image_state = format!(
        "{:?}",
        tab.resources
            .image_state_by_url(selected.as_deref().unwrap())
    );
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    assert!(snapshot.contains("HtmlResourceReferences {"));
    for _ in 0..3 {
        let records = tab.page.html_resource_references().unwrap();
        assert_eq!(records.len(), 7);
        assert_eq!(
            records[0].element,
            element_id(tab.page.dom.as_deref().unwrap(), "sheet")
        );
        assert_eq!(records[1].element, image);
        assert_eq!(records[6].kind, K::Script);
        assert_eq!(resolved(&records[0]).url().fragment(), Some("sheet"));
        assert_eq!(snapshot, tab.document_address_debug_snapshot().unwrap());
        assert!(commands.try_recv().is_err());
        assert_eq!(tab.page.addresses, addresses);
        assert_eq!(tab.dom_handle, handle);
        assert_eq!(tab.dom_version, version);
        assert_eq!(tab.loading, loading);
        assert_eq!(tab.last_status, status);
        assert_eq!(format!("{:?}", tab.document_load), load);
        assert_eq!(tab.page.style_generations(), generations);
        assert_eq!(tab.page.render_pipeline_debug_snapshot(), pipeline);
        assert_eq!(tab.page.retained_render_state_debug_snapshot(), retained);
        assert_eq!(format!("{:?}", tab.pending_render_work), pending);
        assert_eq!(
            tab.history()
                .iter()
                .map(|entry| entry.url().to_owned())
                .collect::<Vec<_>>(),
            history
        );
        assert_eq!(tab.page.active_external_stylesheet_slots(), slots);
        assert_eq!(tab.stylesheet_loads.len(), issued.len());
        for (id, request, key, debug) in &issued {
            let load = &tab.stylesheet_loads[id];
            assert_eq!(load.request_id, *request);
            assert_eq!(&load.issued_key, key);
            assert_eq!(&format!("{load:?}"), debug);
        }
        assert_eq!(
            crate::page::selected_image_source(&tab.page.image_sources, image, "image.png"),
            selected
        );
        assert_eq!(
            format!(
                "{:?}",
                tab.resources
                    .image_state_by_url(selected.as_deref().unwrap())
            ),
            image_state
        );
    }
}

#[test]
fn html_resource_current_base_inventory_preserves_selected_loader_associations() {
    let (mut tab, commands) = addressing_tab(concat!(
        "<!doctype html><base id=b href='/old/'>",
        "<link rel=stylesheet href=site.css><body><img id=image src=image.png>",
        "<form action=''></form><form action=' '></form><a href=''>x</a>",
    ));
    assert_eq!(commands.try_iter().count(), 2);
    let dom = tab.page.dom.as_deref().unwrap();
    let base = element_id(dom, "b");
    let image = element_id(dom, "image");
    let origin = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    let selected_image =
        crate::page::selected_image_source(&tab.page.image_sources, image, "image.png");
    let (slot, key) = tab
        .stylesheet_loads
        .iter()
        .map(|(slot, load)| (*slot, load.issued_key.clone()))
        .next()
        .unwrap();
    let before = tab.page.html_resource_references().unwrap();
    assert_eq!(
        resolved(&before[0]).url().as_str(),
        "https://example.test/old/site.css"
    );
    assert_eq!(resolved(&before[2]).url().as_str(), ADDRESS_URL);
    assert_eq!(
        resolved(&before[3]).url().as_str(),
        "https://example.test/old/"
    );
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            base,
            &[("id", "b"), ("href", "https://cdn.test/new/#base")],
        )],
    )
    .unwrap();
    assert!(commands.try_recv().is_err());
    let records = tab.page.html_resource_references().unwrap();
    assert_eq!(records.len(), 5);
    assert_eq!(
        resolved(&records[0]).url().as_str(),
        "https://cdn.test/new/site.css"
    );
    assert_eq!(
        resolved(&records[1]).url().as_str(),
        "https://cdn.test/new/image.png"
    );
    assert_eq!(resolved(&records[2]).url().as_str(), ADDRESS_URL);
    assert_eq!(
        resolved(&records[3]).url().as_str(),
        "https://cdn.test/new/"
    );
    assert_eq!(
        resolved(&records[4]).url().as_str(),
        "https://cdn.test/new/"
    );
    for (old, current) in before.iter().zip(&records) {
        assert_eq!(old.element, current.element);
    }
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, image, "image.png"),
        selected_image
    );
    assert_eq!(tab.stylesheet_loads[&slot].issued_key, key);
    assert_eq!(
        tab.page.active_external_stylesheet_slots(),
        std::collections::HashSet::from([slot])
    );
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
}

#[test]
fn html_resource_opaque_and_invalid_base_outcomes_use_committed_state() {
    let (tab, commands) = addressing_tab(concat!(
        "<!doctype html><base href='about:blank'>",
        "<a href=relative>x</a><a href='https://h.test/absolute#part'>x</a>",
        "<form action=''></form>",
    ));
    let records = tab.page.html_resource_references().unwrap();
    let HtmlReferenceOutcome::Failed(failure) = &records[0].outcome else {
        panic!("opaque active base cannot resolve a relative path")
    };
    assert_eq!(
        failure.kind,
        addressing::UrlFailureKind::InvalidInput(
            addressing::UrlParseError::RelativeAgainstOpaqueBase
        )
    );
    assert_eq!(failure.base.as_ref().unwrap().as_str(), "about:blank");
    assert_eq!(
        resolved(&records[1]).url().as_str(),
        "https://h.test/absolute#part"
    );
    assert_eq!(resolved(&records[2]).url().as_str(), ADDRESS_URL);
    assert!(commands.try_recv().is_err());

    let (tab, _) = addressing_tab(
        "<!doctype html><base href='http://['><base href='/ignored/'><a href=relative>x</a>",
    );
    let records = tab.page.html_resource_references().unwrap();
    assert_eq!(
        resolved(&records[0]).url().as_str(),
        "https://example.test/dir/relative"
    );
}

#[test]
fn html_resource_rejected_and_stale_publications_keep_the_committed_observation() {
    let (mut tab, commands) =
        addressing_tab("<!doctype html><base id=b href='/committed/'><a href=target>x</a>");
    assert!(commands.try_recv().is_err());
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    let output =
        html::parse_document("<!doctype html><a href='/stale'>x</a>", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen - 1,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert_eq!(tab.document_address_debug_snapshot().unwrap(), snapshot);
    assert!(
        publish_addressing_patches(
            &mut tab,
            vec![
                attributes(base, &[("href", "/rejected/")]),
                DomPatch::AppendText {
                    key: PatchKey(99999),
                    text: "invalid".into()
                },
            ]
        )
        .is_err()
    );
    assert_eq!(tab.document_address_debug_snapshot().unwrap(), snapshot);
    let records = tab.page.html_resource_references().unwrap();
    assert_eq!(
        resolved(&records[0]).url().as_str(),
        "https://example.test/committed/target"
    );
}

#[test]
fn html_resource_replacement_clear_and_navigation_have_no_retained_inventory() {
    let (mut tab, _commands) = addressing_tab("<!doctype html><a href=old>x</a>");
    let output =
        html::parse_document("<!doctype html><a href=new>x</a>", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert_eq!(
        resolved(&tab.page.html_resource_references().unwrap()[0])
            .url()
            .as_str(),
        "https://example.test/dir/new"
    );
    let output =
        html::parse_document("<!doctype html><p>No references", Default::default()).unwrap();
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
    assert!(tab.page.html_resource_references().unwrap().is_empty());
    tab.navigate_to_new("https://other.test/new".into());
    assert!(tab.page.html_resource_references().is_none());
    assert!(tab.document_address_debug_snapshot().is_none());
    assert!(Tab::new(99).document_address_debug_snapshot().is_none());
}

#[test]
fn html_resource_debug_literal_labels_and_outcomes_are_deterministic() {
    let markup = "<a href='/one?q#part'>x</a><img src=''><a href='http://['>bad</a>";
    let (tab, _) = addressing_tab(&format!("<!doctype html>{markup}"));
    let (other, _) = addressing_tab(&format!(
        "<!doctype html><!--allocate a different key-->{markup}"
    ));
    let records = tab.page.html_resource_references().unwrap();
    let other_records = other.page.html_resource_references().unwrap();
    assert_ne!(records[0].element, other_records[0].element);
    let url = r#"BrowserUrl { scheme: "https", username: "", password: None, host_kind: Some("domain"), port: None, path: "<redacted>", query: Some("<redacted>"), fragment: Some("<redacted>"), authority: true, opaque_path: false }"#;
    let expected = format!(
        "HtmlResourceReferences {{\n    [0] source=element[0] attribute=\"href\" kind=Hyperlink outcome=Resolved {{ address: ResourceAddress {{ url: {url}, kind: Hyperlink }}, diagnostics: [] }}\n        retrieval_scheme_capability=Supported navigation_scheme_capability=Supported\n    [1] source=element[1] attribute=\"src\" kind=Image outcome=EmptyReference\n    [2] source=element[2] attribute=\"href\" kind=Hyperlink outcome=Failed(UrlFailure {{ kind: InvalidInput(InvalidIpv6Address), diagnostics: [], base: Some({url}) }})\n}}"
    );
    assert_eq!(
        tab.page.html_resource_address_debug_snapshot().unwrap(),
        expected
    );
    assert_eq!(
        other.page.html_resource_address_debug_snapshot().unwrap(),
        expected
    );
    assert!(
        tab.document_address_debug_snapshot()
            .unwrap()
            .contains(&expected)
    );
}

#[test]
fn html_resource_debug_redacts_private_valid_and_invalid_contents() {
    let (tab, commands) = addressing_tab(concat!(
        "<!doctype html><a id=SECRET_ID href='https://SECRET_USER:SECRET_PASSWORD@private.test:8443/private-path?private-query#private-fragment'>x</a>",
        "<a href='http://[SECRET_INVALID'>bad</a>",
        "<script src='secret-scheme:SECRET_PAYLOAD#SECRET_FRAGMENT'></script>",
        "<script type=SECRET_TYPE src=SECRET_IGNORED></script>",
    ));
    assert!(commands.try_recv().is_err());
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    let records = tab.page.html_resource_references().unwrap();
    assert_eq!(records.len(), 3);
    let nested: Vec<_> = records
        .iter()
        .map(|r| format!("{:#?}", Some(&r.outcome)))
        .collect();
    for output in std::iter::once(&snapshot).chain(&nested) {
        for private in [
            "SECRET",
            "private.test",
            "8443",
            "private-path",
            "private-query",
            "private-fragment",
            "secret-scheme",
        ] {
            assert!(!output.contains(private), "{output}");
        }
    }
    assert_eq!(
        resolved(&records[0]).url().as_str(),
        "https://SECRET_USER:SECRET_PASSWORD@private.test:8443/private-path?private-query#private-fragment"
    );
    assert!(commands.try_recv().is_err());
}
