use super::super::{
    Tab,
    parsed_document_observation::{
        ParsedDocumentObservation, ParsedDocumentPhase, ParsedTreeScope,
    },
};
use super::support::*;
use crate::page::stylesheet_discovery::*;
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;

fn view(tab: &Tab) -> ParsedDocumentObservation<'_> {
    ParsedDocumentObservation::for_test(tab)
}
fn source_address<'a>(record: &'a StylesheetOccurrence<'_>) -> &'a str {
    let StylesheetSource::Link(link) = record.source() else {
        panic!("link")
    };
    let StylesheetHrefOutcome::Resolved { address, .. } = link.href().outcome() else {
        panic!("resolved")
    };
    address.url().as_str()
}
fn stamp(tab: &Tab) -> String {
    // Controlled test evidence only; never a default diagnostic API.
    let mut slots = tab
        .page
        .active_external_stylesheet_slots()
        .into_iter()
        .collect::<Vec<_>>();
    slots.sort_unstable();
    format!(
        "{:?}",
        (
            (
                tab.dom_handle,
                tab.dom_version,
                tab.active_document_request_id(),
                tab.navigation_pending(),
                tab.loading
            ),
            &tab.document_load,
            &tab.stylesheet_loads,
            slots,
            tab.page.style_generations(),
            &tab.pending_render_work,
            tab.page.retained_render_state_debug_snapshot(),
            tab.page
                .css_stylesheets()
                .iter()
                .map(|p| p as *const _)
                .collect::<Vec<_>>(),
            tab.page.document_address_debug_snapshot(),
        )
    )
}
#[test]
fn aj6_committed_state_matrix() {
    for mask in 0u8..16 {
        let (mut tab, commands) = addressing_tab("<!doctype html><style></style>");
        let _: Vec<_> = commands.try_iter().collect();
        if mask & 8 == 0 {
            tab.page.dom = None;
        }
        if mask & 4 == 0 {
            tab.dom_handle = None;
        }
        if mask & 2 == 0 {
            tab.page.document_mode = None;
        }
        if mask & 1 == 0 {
            tab.page.addresses = None;
        }
        let result = view(&tab).stylesheet_discovery();
        match mask {
            0 | 1 => assert!(matches!(result, Ok(None)), "absent/prepared {mask}"),
            15 => assert_eq!(result.unwrap().unwrap().records().len(), 1),
            _ => assert!(
                matches!(
                    result,
                    Err(StylesheetDiscoveryError::CommittedStateInconsistent)
                ),
                "inconsistent {mask}"
            ),
        }
        let output = tab.stylesheet_discovery_debug_snapshot().unwrap();
        match mask {
            0 | 1 => {
                assert!(
                    output.contains("semantic_complete=not-applicable formatting_complete=true")
                );
                assert!(
                    output.contains("occurrences=0/not-applicable diagnostics=0/not-applicable")
                );
                assert!(output.contains("diagnostics_omitted: not-applicable"));
                if mask == 1 {
                    assert!(output.contains("address=Prepared"));
                }
            }
            15 => {
                assert!(output.contains("semantic_complete=true formatting_complete=true"));
                assert!(output.contains("occurrences=1/1 diagnostics=0/Some(0)"));
            }
            _ => {
                assert!(output.contains("CommittedStateInconsistent"));
                assert!(output.contains("semantic_complete=false formatting_complete=true"));
                assert!(output.contains("occurrences=0/unavailable diagnostics=0/unavailable"));
                assert!(output.contains("diagnostics_omitted: unavailable"));
                assert!(!output.contains("occurrences=0/0"));
            }
        }
        assert!(commands.try_recv().is_err());
    }
    let (tab, commands) = addressing_tab("<!doctype html><p>empty</p>");
    let _: Vec<_> = commands.try_iter().collect();
    assert!(view(&Tab::new(1)).stylesheet_discovery().unwrap().is_none());
    assert!(
        view(&tab)
            .stylesheet_discovery()
            .unwrap()
            .unwrap()
            .records()
            .is_empty()
    );
    assert!(
        tab.stylesheet_discovery_debug_snapshot()
            .unwrap()
            .contains("semantic_complete=true")
    );
}
#[test]
fn aj6_lifecycle_and_coherence_are_separate() {
    let (mut tab, commands) = addressing_tab("<!doctype html><style></style>");
    let _: Vec<_> = commands.try_iter().collect();
    tab.document_load.body_completion =
        Some(net::BodyProgress::try_new(ResourceKind::Css, 0).unwrap());
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::InvalidEvidence);
    assert_eq!(view(&tab).tree_scope(), ParsedTreeScope::Preview);
    assert_eq!(
        view(&tab)
            .stylesheet_discovery()
            .unwrap()
            .unwrap()
            .records()
            .len(),
        1
    );
    let output = tab.stylesheet_discovery_debug_snapshot().unwrap();
    assert!(output.contains("phase=InvalidEvidence tree=Preview"));
    assert!(output.contains("semantic_complete=true"));
    tab.fail_document(super::super::state::DocumentLoadFailure::Parser(
        bus::HtmlParseFailure::InputClosed,
    ));
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Failed);
    assert!(view(&tab).stylesheet_discovery().unwrap().is_some());
    tab.dom_handle = None;
    assert!(matches!(
        view(&tab).stylesheet_discovery(),
        Err(StylesheetDiscoveryError::CommittedStateInconsistent)
    ));
}
#[test]
fn aj6_base_resolution() {
    for (base, expected) in [
        ("", "https://final.test/docs/a.css"),
        ("<base href='/assets/'>", "https://final.test/assets/a.css"),
        (
            "<base href='http://['><base href='/later/'>",
            "https://final.test/docs/a.css",
        ),
    ] {
        let (tab, commands) = addressing_tab_with_response_url(
            &format!("<!doctype html>{base}<link rel=stylesheet href=a.css>"),
            "https://start.test/request",
            "https://final.test/docs/page.html",
        );
        let _: Vec<_> = commands.try_iter().collect();
        let observation = view(&tab);
        let inventory = observation.stylesheet_discovery().unwrap().unwrap();
        assert_eq!(source_address(&inventory.records()[0]), expected);
        assert!(commands.try_recv().is_err());
    }
}
#[test]
fn aj6_current_base_and_retained_attachment() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><base id=b href='/old/'><link id=s rel=stylesheet href=a.css>",
    );
    let issued = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                ..
            } => Some((slot, request.original_url().as_str().to_owned())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(issued.len(), 1);
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    let before_slots = tab.page.active_external_stylesheet_slots();
    let before_loads = format!("{:?}", tab.stylesheet_loads);
    publish_addressing_patches(
        &mut tab,
        vec![attributes(base, &[("id", "b"), ("href", "/new/")])],
    )
    .unwrap();
    assert!(commands.try_recv().is_err());
    assert_eq!(tab.page.active_external_stylesheet_slots(), before_slots);
    assert_eq!(format!("{:?}", tab.stylesheet_loads), before_loads);
    let inventory = view(&tab).stylesheet_discovery().unwrap().unwrap();
    assert_eq!(
        source_address(&inventory.records()[0]),
        "https://example.test/new/a.css"
    );
    assert_eq!(issued[0].1, "https://example.test/old/a.css");
    assert!(commands.try_recv().is_err());
}
#[test]
fn aj6_observation_is_not_runtime_eligibility() {
    // Production preserves Unicode rel membership. AJ6 observes authored facts;
    // AJ8 independently excludes alternate/disabled acquisition.
    let (tab, commands) = addressing_tab(
        "<!doctype html><link rel='alternate&#160;stylesheet' href=legacy.css><link rel=stylesheet href=' '><link rel=stylesheet disabled media=screen integrity=private href=other.css>",
    );
    let issued = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                request,
                kind: ResourceKind::Css,
                ..
            } => Some(request.original_url().as_str().to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        issued.is_empty(),
        "AJ8 excludes alternate and disabled attachments before acquisition"
    );
    let inventory = view(&tab).stylesheet_discovery().unwrap().unwrap();
    assert!(inventory.records()[0].dependency_order().is_none());
    assert_eq!(inventory.dependencies().count(), 2);
    assert_eq!(
        source_address(&inventory.records()[1]),
        "https://example.test/dir/page?q=1"
    );
    assert!(commands.try_recv().is_err());
}
#[test]
fn aj6_observation_purity() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><link rel=stylesheet href=a.css><style>p{color:red}</style><p style='color:blue'>text</p>",
    );
    let issued = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                kind: ResourceKind::Css,
                ..
            } => Some((slot, request.original_url().as_str().to_owned())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(issued.len(), 1);
    let before = stamp(&tab);
    let snapshot = tab.stylesheet_discovery_debug_snapshot().unwrap();
    for _ in 0..3 {
        assert_eq!(
            view(&tab)
                .stylesheet_discovery()
                .unwrap()
                .unwrap()
                .records()
                .len(),
            3
        );
        assert_eq!(tab.stylesheet_discovery_debug_snapshot().unwrap(), snapshot);
        assert_eq!(stamp(&tab), before);
        assert!(commands.try_recv().is_err());
    }
    complete_css(&mut tab, issued[0].0, &issued[0].1, "p{color:green}");
    let _: Vec<_> = commands.try_iter().collect();
    let installed = stamp(&tab);
    let _inventory = view(&tab).stylesheet_discovery().unwrap().unwrap();
    assert_eq!(stamp(&tab), installed);
    assert!(commands.try_recv().is_err());
}
#[test]
fn aj6_reverse_completion_preserves_authored_discovery() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><link id=a rel=stylesheet href=a.css><link id=b rel=stylesheet href=b.css>",
    );
    let owners = [
        element_id(tab.page.dom.as_deref().unwrap(), "a"),
        element_id(tab.page.dom.as_deref().unwrap(), "b"),
    ];
    let issued = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                kind: ResourceKind::Css,
                ..
            } => Some((slot, request.original_url().as_str().to_owned())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(issued.len(), 2);
    assert_eq!(issued[0].1, "https://example.test/dir/a.css");
    assert_eq!(issued[1].1, "https://example.test/dir/b.css");
    assert_ne!(issued[0].0, issued[1].0);
    let observe = |tab: &Tab| {
        let before = stamp(tab);
        let observation = view(tab);
        let inventory = observation.stylesheet_discovery().unwrap().unwrap();
        assert_eq!(inventory.records().len(), 2);
        for (i, (record, expected)) in inventory
            .dependencies()
            .zip([
                "https://example.test/dir/a.css",
                "https://example.test/dir/b.css",
            ])
            .enumerate()
        {
            assert_eq!(record.occurrence_index().get(), i as u32);
            assert_eq!(record.dependency_order().unwrap().get(), i as u32);
            assert_eq!(record.source_node(), owners[i]);
            assert_eq!(source_address(record), expected);
        }
        assert_eq!(inventory.dependencies().count(), 2);
        let _redacted = tab.stylesheet_discovery_debug_snapshot().unwrap();
        assert_eq!(stamp(tab), before);
        assert!(commands.try_recv().is_err());
    };
    assert_eq!(tab.page.pending_count(), 2);
    observe(&tab);
    complete_css(&mut tab, issued[1].0, &issued[1].1, "p{color:blue}");
    let _: Vec<_> = commands.try_iter().collect();
    assert_eq!(tab.page.pending_count(), 1);
    observe(&tab);
    complete_css(&mut tab, issued[0].0, &issued[0].1, "p{color:red}");
    let _: Vec<_> = commands.try_iter().collect();
    assert_eq!(tab.page.pending_count(), 0);
    observe(&tab);
}
#[test]
fn aj6_debug_snapshot_and_redaction() {
    let (tab, commands) = addressing_tab(
        "<!doctype html><link rel=help href=PRIVATE_OTHER><link rel='stylesheet alternate' href='https://USER:PASS@PRIVATE_HOST:8443/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT' type=PRIVATE_TYPE title=PRIVATE_TITLE disabled=false media=PRIVATE_MEDIA crossorigin=PRIVATE_CORS integrity=PRIVATE_INTEGRITY referrerpolicy=PRIVATE_POLICY><style type=other title=PRIVATE_TITLE>PRIVATE_CSS</style><p style='PRIVATE_ATTRIBUTE'></p>",
    );
    let _: Vec<_> = commands.try_iter().collect();
    let snapshot = tab.stylesheet_discovery_debug_snapshot().unwrap();
    for secret in ["PRIVATE_", "USER", "PASS", "8443"] {
        assert!(!snapshot.contains(secret), "redaction failed: {snapshot}");
    }
    assert_eq!(
        snapshot,
        include_str!("fixtures/aj6-stylesheet-discovery.snap")
    );
    assert!(commands.try_recv().is_err());
}
#[test]
fn aj6_allocation_and_format_limits() {
    use super::parsed_document_observation::{
        allocation_work, refuse_allocation_after, refuse_next_allocation,
    };
    let (tab, commands) = addressing_tab("<!doctype html><style>x</style><p style='x'></p>");
    let _: Vec<_> = commands.try_iter().collect();
    let failed = refuse_next_allocation(|| view(&tab).stylesheet_discovery());
    assert!(matches!(
        failed,
        Err(StylesheetDiscoveryError::ReservationFailed(
            StylesheetDiscoveryStorage::TraversalFrames
        ))
    ));
    assert!(matches!(
        refuse_next_allocation(|| tab.stylesheet_discovery_debug_snapshot()),
        Err(StylesheetDiscoverySnapshotError::OutputAllocation)
    ));
    let failed = refuse_allocation_after(1, || view(&tab).stylesheet_discovery());
    assert!(matches!(
        failed,
        Err(StylesheetDiscoveryError::ReservationFailed(
            StylesheetDiscoveryStorage::Occurrences
        ))
    ));
    let (many, commands) =
        addressing_tab(&format!("<!doctype html>{}", "<p style=''></p>".repeat(40)));
    let _: Vec<_> = commands.try_iter().collect();
    let failed = refuse_allocation_after(2, || view(&many).stylesheet_discovery());
    assert!(matches!(
        failed,
        Err(StylesheetDiscoveryError::ReservationFailed(
            StylesheetDiscoveryStorage::Occurrences
        ))
    ));
    assert_eq!(
        crate::page::stylesheet_discovery::WORK.with(|w| w.get().records),
        16
    );
    let (deep, commands) = addressing_tab(&format!(
        "<!doctype html>{}<p></p>{}",
        "<div>".repeat(24),
        "</div>".repeat(24)
    ));
    let _: Vec<_> = commands.try_iter().collect();
    let failed = refuse_allocation_after(1, || view(&deep).stylesheet_discovery());
    assert!(matches!(
        failed,
        Err(StylesheetDiscoveryError::ReservationFailed(
            StylesheetDiscoveryStorage::TraversalFrames
        ))
    ));
    assert!(view(&tab).stylesheet_discovery().unwrap().is_some());
    assert!(view(&many).stylesheet_discovery().unwrap().is_some());
    assert!(view(&deep).stylesheet_discovery().unwrap().is_some());
    // Keep each attribute below the existing 16-KiB tokenizer cap. The block
    // text is independently large; actual retained lengths are asserted before
    // measuring. A missing href isolates borrowing from delegated AH costs.
    let (small, small_commands) = addressing_tab(
        "<!doctype html><link rel=stylesheet integrity=x crossorigin=x referrerpolicy=x><style>x</style><p style=x></p>",
    );
    let policy = "p".repeat(12 * 1024);
    let attribute = "x".repeat(12 * 1024);
    let (large, large_commands) = addressing_tab(&format!(
        "<!doctype html><link rel=stylesheet integrity='{policy}' crossorigin='{policy}' referrerpolicy='{policy}'><style>{}</style><p style='{attribute}'></p>",
        "x".repeat(100_000),
    ));
    let _: Vec<_> = small_commands.try_iter().collect();
    let _: Vec<_> = large_commands.try_iter().collect();
    for (fixture, css_len, attribute_len, policy_len) in
        [(&small, 1, 1, 1), (&large, 100_000, 12 * 1024, 12 * 1024)]
    {
        let inventory = view(fixture).stylesheet_discovery().unwrap().unwrap();
        assert_eq!(inventory.records().len(), 3);
        let StylesheetSource::Link(link) = inventory.records()[0].source() else {
            panic!("link")
        };
        assert!(matches!(
            link.href().outcome(),
            StylesheetHrefOutcome::Missing
        ));
        for fact in [
            link.policies().integrity(),
            link.policies().crossorigin(),
            link.policies().referrerpolicy(),
        ] {
            assert_eq!(fact.authored().unwrap().len(), policy_len);
        }
        let StylesheetSource::StyleBlock(block) = inventory.records()[1].source() else {
            panic!("block")
        };
        assert_eq!(
            block.content().text_chunks().map(str::len).sum::<usize>(),
            css_len
        );
        let StylesheetSource::StyleAttribute { value } = inventory.records()[2].source() else {
            panic!("attribute")
        };
        assert_eq!(value.len(), attribute_len);
    }
    let (_, small_work) = allocation_work(|| view(&small).stylesheet_discovery().unwrap());
    let (_, large_work) = allocation_work(|| view(&large).stylesheet_discovery().unwrap());
    assert_eq!(small_work, large_work);
    eprintln!(
        "AJ6 retained bytes: small css=1 attribute=1 policy=1x3; large css=100000 attribute=12288 policy=12288x3; allocation work [allocations, requested bytes, reallocations]: small={small_work:?} large={large_work:?}"
    );
    assert!(small_commands.try_recv().is_err());
    assert!(large_commands.try_recv().is_err());
    let (_, snapshot_work) = allocation_work(|| tab.stylesheet_discovery_debug_snapshot().unwrap());
    assert!(snapshot_work[1] >= 8192);
    let empty = Tab::new(9);
    assert!(
        empty
            .stylesheet_discovery_debug_snapshot()
            .unwrap()
            .contains("semantic_complete=not-applicable")
    );
}
#[test]
fn aj6_allocation_guards_restore_after_unwind() {
    use super::parsed_document_observation::{
        allocation_work, refuse_allocation_after, refuse_next_allocation,
    };
    for delayed in [false, true] {
        // Allocate the panic payload before refusal and skip the panic hook.
        let payload: Box<dyn std::any::Any + Send> = Box::new("controlled unwind");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> () {
            if delayed {
                // Leave a pending countdown for Drop to clear. Unwinding itself
                // allocates, so it must not be the allocation we refuse.
                refuse_allocation_after(100, || -> () { std::panic::resume_unwind(payload) })
            } else {
                refuse_next_allocation(|| -> () {
                    let mut refused = Vec::<u8>::new();
                    assert!(refused.try_reserve_exact(8).is_err());
                    std::panic::resume_unwind(payload)
                })
            }
        }));
        assert!(result.is_err());
        for _ in 0..101 {
            let mut probe = Vec::<u8>::new();
            assert!(
                probe.try_reserve_exact(8).is_ok(),
                "refusal must be disarmed on unwind"
            );
            std::hint::black_box(probe);
        }
    }
    let payload: Box<dyn std::any::Any + Send> = Box::new("allocation counter unwind");
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            allocation_work(|| -> () { std::panic::resume_unwind(payload) })
        }))
        .is_err()
    );
    let (_, counts) = allocation_work(|| {
        let mut probe = Vec::<u8>::new();
        probe.try_reserve_exact(8).unwrap();
        std::hint::black_box(probe);
    });
    assert_eq!(counts, [1, 8, 0]);
}
#[cfg(feature = "conformance-observation")]
#[test]
fn aj6_feature_observation_scope() {
    let (tab, commands) = addressing_tab("<!doctype html><style>x</style><p style='y'></p>");
    let _: Vec<_> = commands.try_iter().collect();
    let observation = tab.observe_parsed_document();
    let inventory = observation.stylesheet_discovery().unwrap().unwrap();
    assert!(observation.document_handle().is_some());
    assert!(observation.document_version().is_some());
    assert_eq!(inventory.dependencies().count(), 1);
    assert_eq!(inventory.style_attributes().count(), 1);
    assert!(commands.try_recv().is_err());
}
#[test]
fn aj6_namespace_template_and_recovery() {
    let (mut tab, commands) = addressing_tab(
        "<!doctype html><template style='host'><link rel=stylesheet href=inert.css></template><svg style='svg'><link rel=stylesheet href=foreign.css/><foreignObject><link rel=stylesheet href=html.css></foreignObject></svg><table style='table'><div style='div'>x</div></table>",
    );
    let issued = commands
        .try_iter()
        .filter(|c| {
            matches!(
                c,
                CoreCommand::FetchStream {
                    kind: ResourceKind::Css,
                    ..
                }
            )
        })
        .count();
    assert_eq!(issued, 1);
    let observation = view(&tab);
    let inventory = observation.stylesheet_discovery().unwrap().unwrap();
    assert_eq!(inventory.dependencies().count(), 1);
    assert_eq!(
        inventory
            .style_attributes()
            .map(|r| match r.source() {
                StylesheetSource::StyleAttribute { value } => *value,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>(),
        ["host", "svg", "div", "table"]
    );
    drop(inventory);
    // A stale publication must not change the current observation.
    let before = tab.stylesheet_discovery_debug_snapshot().unwrap();
    let parsed =
        html::parse_document("<!doctype html><style>stale</style>", Default::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: tab.active_document_request_id().unwrap() + 1,
        publication: no_quirks_patch_publication_from_output(parsed),
    })
    .unwrap();
    assert_eq!(tab.stylesheet_discovery_debug_snapshot().unwrap(), before);
    assert!(commands.try_recv().is_err());
    // Unlike the stale generation above, a malformed current publication is
    // validated and rejected. Failure lifecycle metadata may change; the
    // committed semantic inventory and addressing must remain authoritative.
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let addresses = tab.page.addresses.clone();
    let owners = view(&tab)
        .stylesheet_discovery()
        .unwrap()
        .unwrap()
        .records()
        .iter()
        .map(StylesheetOccurrence::source_node)
        .collect::<Vec<_>>();
    let publication = no_quirks_patch_publication(
        handle.unwrap(),
        core_types::DomVersion::INITIAL,
        version.next(),
        vec![],
    );
    let generation = tab.active_document_request_id().unwrap();
    assert!(
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: tab.tab_id,
            request_id: generation,
            publication,
        })
        .is_err()
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.addresses, addresses);
    let inventory = view(&tab).stylesheet_discovery().unwrap().unwrap();
    assert_eq!(
        inventory
            .records()
            .iter()
            .map(StylesheetOccurrence::source_node)
            .collect::<Vec<_>>(),
        owners
    );
    assert_eq!(inventory.dependencies().count(), 1);
    assert_eq!(
        source_address(inventory.dependencies().next().unwrap()),
        "https://example.test/dir/html.css"
    );
    assert!(
        commands
            .try_iter()
            .all(|c| !matches!(c, CoreCommand::FetchStream { .. }))
    );
}
