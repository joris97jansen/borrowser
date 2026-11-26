use super::*;
use crate::page::stylesheet_discovery::{StylesheetHrefOutcome, StylesheetSource};
use crate::tab::parsed_document_observation::{
    ParsedDocumentObservation, ParsedDocumentPhase, ParsedTreeScope,
};

#[test]
fn aj6_preview_final_and_replacement() {
    let (mut tab, commands) = parsed_document::loaded(
        b"<!doctype html><base href='/assets/'><link rel=stylesheet href=a.css><style>x</style>",
        1,
    );
    let _: Vec<_> = commands.try_iter().collect();
    let view = ParsedDocumentObservation::for_test(&tab);
    assert_eq!(view.phase(), ParsedDocumentPhase::Parsed);
    assert_eq!(view.tree_scope(), ParsedTreeScope::Final);
    let inventory = view.stylesheet_discovery().unwrap().unwrap();
    let StylesheetSource::Link(link) = inventory.records()[0].source() else {
        panic!("link")
    };
    let StylesheetHrefOutcome::Resolved { address, .. } = link.href().outcome() else {
        panic!("resolved")
    };
    assert_eq!(address.url().as_str(), "https://final.test/assets/a.css");
    let owner = inventory.records()[0].source_node();
    let old_handle = view.document_handle();
    drop(inventory);
    let before = tab.stylesheet_discovery_debug_snapshot().unwrap();
    let request = submit(&mut tab, &commands, "https://replacement.test/page");
    let view = ParsedDocumentObservation::for_test(&tab);
    assert!(view.navigation_pending());
    assert_eq!(view.phase(), ParsedDocumentPhase::Parsed);
    assert_eq!(
        view.stylesheet_discovery().unwrap().unwrap().records()[0].source_node(),
        owner
    );
    tab.fail_document(Failure::Parser(bus::HtmlParseFailure::InputClosed));
    let view = ParsedDocumentObservation::for_test(&tab);
    assert_eq!(view.tree_scope(), ParsedTreeScope::RetainedPreview);
    assert_eq!(
        view.stylesheet_discovery()
            .unwrap()
            .unwrap()
            .records()
            .len(),
        2
    );
    assert_ne!(tab.stylesheet_discovery_debug_snapshot().unwrap(), before);
    let _: Vec<_> = commands.try_iter().collect();
    let mut events = fetch(
        &request,
        network(
            &request,
            reply(
                200,
                &[b"text/html"],
                &[],
                b"<!doctype html><link rel=stylesheet href=new.css><style>new</style>",
                "https://replacement.test/page",
            ),
        ),
    )
    .into_iter();
    let start = events.next().unwrap();
    assert!(matches!(start, NetEvent::Start { .. }));
    apply(&mut tab, start);
    let view = ParsedDocumentObservation::for_test(&tab);
    assert!(view.stylesheet_discovery().unwrap().is_none());
    assert_eq!(
        view.address_scope(),
        crate::tab::parsed_document_observation::ParsedAddressScope::Prepared
    );
    for event in events {
        apply(&mut tab, event);
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    let resources = finish_parser(&mut tab, &commands, &parser, &received);
    // Shut down the actual runtime before asserting the new semantic result.
    drop(parser);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    let view = ParsedDocumentObservation::for_test(&tab);
    assert_eq!(view.phase(), ParsedDocumentPhase::Parsed);
    assert_eq!(view.tree_scope(), ParsedTreeScope::Final);
    assert_ne!(view.document_handle(), old_handle);
    assert_eq!(
        view.active_base().unwrap().as_str(),
        "https://replacement.test/page"
    );
    let inventory = view.stylesheet_discovery().unwrap().unwrap();
    assert_eq!(inventory.records().len(), 2);
    assert_eq!(inventory.dependencies().count(), 2);
    let StylesheetSource::Link(link) = inventory.records()[0].source() else {
        panic!("new link")
    };
    let StylesheetHrefOutcome::Resolved { address, .. } = link.href().outcome() else {
        panic!("new address")
    };
    assert_eq!(address.url().as_str(), "https://replacement.test/new.css");
    let StylesheetSource::StyleBlock(block) = inventory.records()[1].source() else {
        panic!("new style")
    };
    assert_eq!(block.content().text_chunks().collect::<Vec<_>>(), ["new"]);
    assert_eq!(inventory.records()[0].dependency_order().unwrap().get(), 0);
    assert_eq!(inventory.records()[1].dependency_order().unwrap().get(), 1);
    assert_eq!(
        resources
            .iter()
            .filter(|c| matches!(
                c,
                CoreCommand::FetchStream {
                    kind: Route::Css,
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn aj6_streaming_preview_and_final_identity() {
    let source =
        b"<!doctype html><head><style>p{color:red}</style></head><body><p style='x'>first</p>";
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, "https://example.test/page");
    let mut done = None;
    for event in fetch(
        &request,
        network(
            &request,
            reply(
                200,
                &[b"text/html"],
                &[],
                source,
                "https://example.test/page",
            ),
        ),
    ) {
        if matches!(event, NetEvent::Done { .. }) {
            done = Some(event);
        } else {
            apply(&mut tab, event);
        }
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime_with_policy(
        rx,
        events,
        runtime_parse::PreviewPolicy {
            tick: Duration::ZERO,
            byte_threshold: Some(1),
            ..Default::default()
        },
    );
    let mut resources = Vec::new();
    route_parser(&commands, &parser, &mut resources);
    let owner = loop {
        let event = received.recv_timeout(Duration::from_secs(10)).unwrap();
        let publication = matches!(event, CoreEvent::DomPatchUpdate { .. });
        tab.on_core_event(event).unwrap();
        route_parser(&commands, &parser, &mut resources);
        if publication {
            let view = ParsedDocumentObservation::for_test(&tab);
            if let Some(inventory) = view.stylesheet_discovery().unwrap()
                && let Some(record) = inventory.dependencies().next()
            {
                assert_eq!(view.tree_scope(), ParsedTreeScope::Preview);
                break record.source_node();
            }
        }
    };
    apply(&mut tab, done.unwrap());
    finish_parser(&mut tab, &commands, &parser, &received);
    let view = ParsedDocumentObservation::for_test(&tab);
    assert_eq!(view.tree_scope(), ParsedTreeScope::Final);
    assert_eq!(
        view.stylesheet_discovery().unwrap().unwrap().records()[0].source_node(),
        owner
    );
    drop(parser);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}
