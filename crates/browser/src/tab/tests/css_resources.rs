use super::super::Tab;
use super::support::*;
use crate::page::css_resources::{CssBase, CssReferenceOutcome as O, CssSource};
use addressing::{UrlDiagnostic as D, UrlFailureKind, UrlParseError as E};
use bus::CoreCommand;

fn fixture(final_url: &str, css: &str) -> (Tab, std::sync::mpsc::Receiver<CoreCommand>) {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><link id=sheet rel=stylesheet href=site.css><p id=p>hello</p>",
    );
    let (slot, url) = commands
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(slot),
                ..
            } => {
                let url = fetch_request
                    .legacy_issued_url(core_types::ResourceKind::Css)
                    .unwrap();
                Some((slot, url))
            }
            _ => None,
        })
        .unwrap();
    complete_css_with_response(
        &mut tab,
        slot,
        super::support::response_facts(url, final_url.into(), Some(200), Some("text/css".into())),
        css,
    );
    commands.try_iter().for_each(drop);
    (tab, commands)
}
fn outcomes(tab: &Tab) -> Vec<O> {
    tab.page
        .css_resource_references()
        .unwrap()
        .unwrap()
        .iter()
        .flat_map(|s| s.discovery.references.iter().map(|r| s.outcome(r)))
        .collect()
}
fn address(outcome: &O) -> &str {
    match outcome {
        O::Resolved { address, .. } => address.url().as_str(),
        _ => panic!("expected address, got {outcome:?}"),
    }
}
#[test]
fn css_resource_notice_truncation_preserves_addresses_and_observational_debug() {
    let mut options = css::ParseOptions::stylesheet();
    let notice_count = options.limits.max_diagnostics + 1;
    let mut css = String::from("p{");
    for index in 0..notice_count {
        // Unrelated to resource discovery; no custom property contains a URL.
        css.push_str(&format!("--accent{index}:red;"));
    }
    css.push_str("background:url(a.png),url(a.png)}");
    let (tab, commands) = fixture("https://cdn.test/css/main.css", &css);
    let parsed = &tab.page.css_stylesheets()[0];
    let original_stats = parsed.stats.clone();
    assert!(!original_stats.hit_limit);
    let generations = tab.page.style_generations();
    let mut sources = tab.page.css_resource_references().unwrap().unwrap();
    let source = &mut sources[0];

    options.limits.max_diagnostics = notice_count;
    source.discovery =
        css::resource_references::discover_stylesheet_resource_references(parsed, &options);
    assert_eq!(source.discovery.stats, original_stats);
    assert!(!source.discovery.feature_notices_truncated);
    let before = source
        .discovery
        .references
        .iter()
        .map(|reference| address(&source.outcome(reference)).to_owned())
        .collect::<Vec<_>>();

    options.limits.max_diagnostics = 0;
    source.discovery =
        css::resource_references::discover_stylesheet_resource_references(parsed, &options);
    assert_eq!(parsed.stats, original_stats);
    assert_eq!(source.discovery.stats, original_stats);
    assert!(source.discovery.feature_notices.is_empty());
    assert!(source.discovery.feature_notices_truncated);
    let after = source
        .discovery
        .references
        .iter()
        .map(|reference| address(&source.outcome(reference)).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(after, before);
    assert_eq!(
        after,
        ["https://cdn.test/css/a.png", "https://cdn.test/css/a.png"]
    );

    // The production observation's default budget also truncates notices, but
    // still visits and resolves both occurrences from the same retained parse.
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    assert!(snapshot.contains("incomplete=false feature_notices_truncated=true"));
    assert!(!snapshot.contains("outcome=IncompleteSource"));
    for _ in 0..3 {
        assert_eq!(snapshot, tab.document_address_debug_snapshot().unwrap());
        let sources = tab.page.css_resource_references().unwrap().unwrap();
        assert_eq!(sources[0].discovery.feature_notices.len(), notice_count - 1);
        assert_eq!(
            outcomes(&tab).iter().map(address).collect::<Vec<_>>(),
            after
        );
        assert_eq!(tab.page.css_stylesheets()[0].stats, original_stats);
        assert_eq!(tab.page.style_generations(), generations);
        assert!(commands.try_recv().is_err());
    }
}

#[test]
fn css_resource_response_base_is_distinct_from_request_identity() {
    let (tab, _) = fixture(
        "https://cdn.test/assets/main.css",
        "p{background-image:url(icons/a.svg),url(icons/a.svg);cursor:url(cursor.png);color:red}",
    );
    let sources = tab.page.css_resource_references().unwrap().unwrap();
    assert_eq!(sources.len(), 1);
    assert!(matches!(sources[0].source, CssSource::Stylesheet { .. }));
    let results = outcomes(&tab);
    assert_eq!(results.len(), 3);
    assert_eq!(address(&results[0]), "https://cdn.test/assets/icons/a.svg");
    assert_eq!(address(&results[1]), address(&results[0]));
    assert_eq!(address(&results[2]), "https://cdn.test/assets/cursor.png");
    assert_eq!(
        tab.stylesheet_loads
            .values()
            .next()
            .unwrap()
            .issued_key
            .as_str(),
        "https://example.test/dir/site.css"
    );
}
#[test]
fn css_resource_unavailable_bases_do_not_guess_and_keep_failures_separate() {
    let css = "p{background:url(https:icons/logo.svg),url('https:'),url(icons/logo.svg),url(https://cdn.test/a),url('https://[::1'),url('https://cdn.test/a%zz%yy')}";
    for base in ["", "https://source.test:bad/main.css"] {
        let (tab, _) = fixture(base, css);
        let sources = tab.page.css_resource_references().unwrap().unwrap();
        if base.is_empty() {
            assert!(matches!(sources[0].base, CssBase::Response(None)));
        } else {
            assert!(
                matches!(sources[0].base, CssBase::Response(Some(Err(f))) if f.kind == UrlFailureKind::InvalidInput(E::InvalidPort))
            );
        }
        let results = outcomes(&tab);
        assert!(results[..3].iter().all(|r| matches!(r, O::UnresolvedBase)));
        assert_eq!(address(&results[3]), "https://cdn.test/a");
        assert!(
            matches!(&results[4], O::Failed(f) if f.kind == UrlFailureKind::InvalidInput(E::InvalidIpv6Address) && f.base.is_none())
        );
        assert!(
            matches!(&results[5], O::Resolved { diagnostics, .. } if diagnostics == &[D::InvalidPercentEncoding, D::InvalidPercentEncoding])
        );
    }
    let (tab, _) = fixture("https://example.test/css/main.css", css);
    let results = outcomes(&tab);
    assert_eq!(
        address(&results[0]),
        "https://example.test/css/icons/logo.svg"
    );
    assert_eq!(address(&results[1]), "https://example.test/css/main.css");
}
#[test]
fn css_resource_shapes_and_blocking_contexts_have_deterministic_precedence() {
    let (tab, _) = fixture(
        "https://cdn.test/main.css",
        "p{--asset:url('#mask');mask:url(#m);background:url(''),url(icon.svg#part);width:url(bad\"x)} @import url(''); @unknown{ x:url(bad\"x) }",
    );
    let sources = tab.page.css_resource_references().unwrap().unwrap();
    let results = outcomes(&tab);
    assert!(matches!(results[0], O::BlockedContext));
    assert_eq!(
        sources[0].discovery.references[0].disposition.label(),
        "LocalFragment"
    );
    assert!(matches!(results[1], O::LocalFragment));
    assert!(matches!(results[2], O::Empty));
    assert_eq!(address(&results[3]), "https://cdn.test/icon.svg#part");
    assert!(matches!(results[4], O::InvalidCss));
    assert!(matches!(results[5], O::BlockedContext));
    assert!(matches!(results[6], O::BlockedContext));
    assert_eq!(
        sources[0].discovery.references[6].disposition.label(),
        "InvalidCss"
    );
    assert!(!sources[0].discovery.diagnostics.is_empty());
}
#[test]
fn css_resource_inline_observations_follow_committed_document_base() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><base id=b href=/old/><style>p{background:url(sheet.png)}</style><p id=p style='cursor:url(attr.png)'>hello</p><link rel=stylesheet href=site.css>",
    );
    let (slot, url) = commands
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(slot),
                ..
            } => {
                let url = fetch_request
                    .legacy_issued_url(core_types::ResourceKind::Css)
                    .unwrap();
                Some((slot, url))
            }
            _ => None,
        })
        .unwrap();
    complete_css_with_response(
        &mut tab,
        slot,
        super::support::response_facts(
            url.clone(),
            "https://cdn.test/css/main.css".into(),
            Some(200),
            Some("text/css".into()),
        ),
        "p{mask-image:url(external.svg)}",
    );
    commands.try_iter().for_each(drop);
    let before = outcomes(&tab);
    assert_eq!(
        before.iter().map(address).collect::<Vec<_>>(),
        [
            "https://example.test/old/sheet.png",
            "https://example.test/old/attr.png",
            "https://cdn.test/css/external.svg"
        ]
    );
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(base, &[("id", "b"), ("href", "/new/")])],
    )
    .unwrap();
    let after = outcomes(&tab);
    assert_eq!(
        after.iter().map(address).collect::<Vec<_>>(),
        [
            "https://example.test/new/sheet.png",
            "https://example.test/new/attr.png",
            "https://cdn.test/css/external.svg"
        ]
    );
    assert_eq!(tab.stylesheet_loads[&slot].issued_key.as_str(), url);
    assert!(commands.try_recv().is_err());
}
#[test]
fn css_resource_inventory_and_debug_are_observational_and_redacted() {
    let (mut tab, commands) = fixture(
        "https://SOURCE_USER:SOURCE_PASS@private-source.test:8443/secret/main.css?SECRET_QUERY#SECRET_FRAGMENT",
        "p{background:url('https://USER:PASS@private-target.test:8444/secret?SECRET_QUERY#SECRET_FRAGMENT');--SECRET:url(#SECRET);color:red}",
    );
    let before = (
        tab.dom_handle,
        tab.dom_version,
        tab.nav_gen,
        tab.loading,
        tab.page.style_generations(),
        format!("{:?}", tab.pending_render_work),
        tab.page.retained_render_state_debug_snapshot(),
    );
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    assert!(snapshot.contains("CssResourceReferences"));
    for secret in [
        "SOURCE_USER",
        "SOURCE_PASS",
        "private-source",
        "USER",
        "PASS",
        "private-target",
        "8443",
        "8444",
        "secret",
        "SECRET_QUERY",
        "SECRET_FRAGMENT",
        "SECRET",
    ] {
        assert!(!snapshot.contains(secret), "{secret}: {snapshot}");
    }
    for _ in 0..3 {
        assert_eq!(snapshot, tab.document_address_debug_snapshot().unwrap());
        assert_eq!(outcomes(&tab).len(), 2);
        assert!(commands.try_recv().is_err());
    }
    assert_eq!(
        before,
        (
            tab.dom_handle,
            tab.dom_version,
            tab.nav_gen,
            tab.loading,
            tab.page.style_generations(),
            format!("{:?}", tab.pending_render_work),
            tab.page.retained_render_state_debug_snapshot()
        )
    );
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
}
#[test]
fn css_resource_capabilities_and_unsupported_features_are_distinct() {
    let (tab, _) = fixture(
        "https://cdn.test/main.css",
        "p{background:url(data:image/png,payload),url(https://h.test/a);cursor:image-set(url(a) 1x);mask:var(--x,url(fallback))} @font-face{src:url(font)} @import 'theme.css';",
    );
    let results = outcomes(&tab);
    assert!(
        matches!(&results[0], O::Resolved { address,.. } if address.capability(addressing::ResourceOperation::Retrieval) == addressing::ResourceCapability::Deferred)
    );
    assert!(results[2..].iter().all(|r| matches!(r, O::BlockedContext)));
}

#[test]
fn css_resource_incomplete_source_blocks_resolution_without_erasing_context() {
    let mut css = "p{background:url(a);--x:url(#m)}".to_owned();
    for _ in 0..16_384 {
        css.push_str("p{color:red}");
    }
    let (tab, commands) = fixture("https://cdn.test/main.css", &css);
    let sources = tab.page.css_resource_references().unwrap().unwrap();
    assert!(sources[0].discovery.stats.hit_limit);
    #[cfg(feature = "conformance-observation")]
    assert!(
        tab.observe_document_addressing()
            .unwrap()
            .unwrap()
            .css_sources()[0]
            .discovery()
            .stats
            .hit_limit
    );
    assert_eq!(
        sources[0].discovery.references[1].disposition.label(),
        "LocalFragment"
    );
    assert!(!sources[0].discovery.references[1].contexts.is_empty());
    assert!(
        outcomes(&tab)
            .iter()
            .all(|r| matches!(r, O::IncompleteSource))
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn css_resource_attachment_retirement_does_not_reuse_the_old_base() {
    let (mut tab, commands) = fixture("https://old-cdn.test/css/main.css", "p{background:url(a)}");
    let old_slot = *tab.stylesheet_loads.keys().next().unwrap();
    let owner = element_id(tab.page.dom.as_deref().unwrap(), "sheet");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            owner,
            &[("id", "sheet"), ("rel", "stylesheet"), ("href", "new.css")],
        )],
    )
    .unwrap();
    let (new_slot, url) = commands
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(slot),
                ..
            } => {
                let url = fetch_request
                    .legacy_issued_url(core_types::ResourceKind::Css)
                    .unwrap();
                Some((slot, url))
            }
            _ => None,
        })
        .unwrap();
    assert_ne!(old_slot, new_slot);
    assert!(outcomes(&tab).is_empty());
    complete_css_with_response(
        &mut tab,
        new_slot,
        super::support::response_facts(
            url,
            "https://new-cdn.test/other/main.css".into(),
            Some(200),
            Some("text/css".into()),
        ),
        "p{background:url(a)}",
    );
    assert_eq!(address(&outcomes(&tab)[0]), "https://new-cdn.test/other/a");
    assert!(!tab.stylesheet_loads.contains_key(&old_slot));
}

#[test]
fn css_resource_debug_literal_snapshot() {
    let (tab, _) = addressing_tab("<!doctype html><p style='background:url(a.png)'>hello</p>");
    let snapshot = tab.page.css_resource_address_debug_snapshot().unwrap();
    assert_eq!(
        snapshot,
        include_str!("fixtures/ah8-css-resource-addresses.snap").trim_end()
    );
}

#[test]
fn css_resource_loaded_preview_keeps_response_base_after_document_failure() {
    let (mut tab, commands) = fixture(
        "https://cdn.test/preview/main.css",
        "p{background:url(icon.svg);color:red}",
    );
    let before = address(&outcomes(&tab)[0]).to_owned();
    tab.on_core_event(bus::CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    commands.try_iter().for_each(drop);
    assert_eq!(address(&outcomes(&tab)[0]), before);
    let snapshot = tab.document_address_debug_snapshot().unwrap();
    assert_eq!(snapshot, tab.document_address_debug_snapshot().unwrap());
    assert!(commands.try_recv().is_err());
}
