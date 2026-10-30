use super::ShellApp;
use app_api::UiApp;
use bus::{CoreCommand, CoreEvent, DocumentPublication, DocumentPublicationPayload};
use core_types::{DomHandle, DomVersion, NetworkResponseInfo, ResourceKind, StylesheetSlotId};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

fn response(url: &str, content_type: &str) -> NetworkResponseInfo {
    NetworkResponseInfo {
        requested_url: url.into(),
        final_url: url.into(),
        status_code: Some(200),
        content_type: Some(content_type.into()),
    }
}

fn open_document(
    app: &mut ShellApp,
    rx: &Receiver<CoreCommand>,
    document_url: &str,
) -> (u64, u64, StylesheetSlotId, String) {
    let tab_id = app.tabs[app.active].tab_id;
    app.tabs[app.active].navigate_to_new(document_url.into());
    let request_id = app.tabs[app.active].nav_gen;
    assert!(rx.try_iter().any(|command| matches!(command,
        CoreCommand::FetchStream { tab_id: id, request_id: request, kind: ResourceKind::Html, .. }
            if id == tab_id && request == request_id
    )));
    app.on_core_event(CoreEvent::NetworkStart {
        tab_id,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response(document_url, "text/html"),
    });
    let output = html::parse_document(
        "<!doctype html><link rel=stylesheet href=site.css>",
        Default::default(),
    )
    .unwrap();
    app.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id,
        request_id,
        publication: DocumentPublication {
            handle: DomHandle(tab_id),
            document_mode: output.document_mode,
            payload: DocumentPublicationPayload::Patch {
                from: DomVersion::INITIAL,
                to: DomVersion(1),
                patches: output.patches,
            },
        },
    });
    let fetches: Vec<_> = rx
        .try_iter()
        .filter_map(|command| match command {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                url,
                kind: ResourceKind::Css,
                ..
            } => Some((slot, url)),
            _ => None,
        })
        .collect();
    assert_eq!(fetches.len(), 1);
    let (slot, url) = fetches.into_iter().next().unwrap();
    (tab_id, request_id, slot, url)
}

fn feed_chunk(
    app: &mut ShellApp,
    rx: &Receiver<CoreCommand>,
    runtime: &Sender<CoreCommand>,
    key: (u64, u64, StylesheetSlotId),
    url: &str,
    bytes: &[u8],
) {
    let (tab_id, request_id, slot) = key;
    app.on_core_event(CoreEvent::NetworkStart {
        tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: response(url, "text/css"),
    });
    app.on_core_event(CoreEvent::NetworkChunk {
        tab_id,
        request_id,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.into(),
        bytes: bytes.to_vec(),
    });
    let chunk = rx.try_recv().expect("Browser forwards CSS bytes");
    assert!(
        matches!(&chunk, CoreCommand::CssChunk { tab_id: id, request_id: request, stylesheet_slot_id, .. } if *id == tab_id && *request == request_id && *stylesheet_slot_id == slot)
    );
    runtime.send(chunk).unwrap();
}

#[test]
fn stylesheet_runtime_tab_close_discards_old_buffer_and_does_not_touch_new_tab() {
    for close_active in [false, true] {
        let mut app = ShellApp::new();
        let (cmd_tx, cmd_rx) = mpsc::channel();
        app.set_bus_sender(cmd_tx);
        let (runtime_tx, runtime_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        runtime_css::start_css_runtime(runtime_rx, events_tx);

        let (old_tab, old_generation, old_slot, old_url) =
            open_document(&mut app, &cmd_rx, "https://example.test/old/page");
        feed_chunk(
            &mut app,
            &cmd_rx,
            &runtime_tx,
            (old_tab, old_generation, old_slot),
            &old_url,
            b"p{color:red",
        );
        if close_active {
            app.close_active();
        } else {
            app.close_at(0);
        }
        let commands: Vec<_> = cmd_rx.try_iter().collect();
        let abort_index = commands.iter().position(|command| matches!(command, CoreCommand::CssAbort { tab_id, request_id, stylesheet_slot_id, url, .. } if *tab_id == old_tab && *request_id == old_generation && *stylesheet_slot_id == old_slot && url == &old_url)).expect("closed tab retires old partial stylesheet");
        let cancel_index = commands.iter().position(|command| matches!(command, CoreCommand::CancelRequest { tab_id, request_id } if *tab_id == old_tab && *request_id == old_generation)).expect("closed tab cancels old request");
        assert!(
            abort_index < cancel_index,
            "retirement precedes cancellation"
        );
        assert_eq!(commands.iter().filter(|command| matches!(command, CoreCommand::CssAbort { tab_id, request_id, stylesheet_slot_id, .. } if *tab_id == old_tab && *request_id == old_generation && *stylesheet_slot_id == old_slot)).count(), 1);
        for command in commands {
            if matches!(command, CoreCommand::CssAbort { .. }) {
                runtime_tx.send(command).unwrap();
            }
        }

        // CssDone is an ordered barrier: an old retained buffer would first
        // emit CssDecodedBlock; the removed key emits only CssSheetDone.
        runtime_tx
            .send(CoreCommand::CssDone {
                tab_id: old_tab,
                request_id: old_generation,
                stylesheet_slot_id: old_slot,
                url: old_url.clone(),
            })
            .unwrap();
        let old_done = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("old-key runtime barrier");
        assert!(
            matches!(&old_done, CoreEvent::CssSheetDone { tab_id, request_id, stylesheet_slot_id, .. } if *tab_id == old_tab && *request_id == old_generation && *stylesheet_slot_id == old_slot),
            "partial CSS survived closed tab: {old_done:?}"
        );
        app.on_core_event(old_done);
        app.on_core_event(CoreEvent::NetworkChunk {
            tab_id: old_tab,
            request_id: old_generation,
            stylesheet_slot_id: Some(old_slot),
            kind: ResourceKind::Css,
            url: old_url.clone(),
            bytes: b"late}".to_vec(),
        });
        assert!(
            cmd_rx.try_recv().is_err(),
            "late closed-tab events are dropped"
        );

        let (new_tab, generation, slot, url) =
            open_document(&mut app, &cmd_rx, "https://example.test/new/page");
        assert_ne!(old_tab, new_tab);
        feed_chunk(
            &mut app,
            &cmd_rx,
            &runtime_tx,
            (new_tab, generation, slot),
            &url,
            b"p{color:green}",
        );
        // Repeated old cleanup must not remove a different tab's live buffer,
        // even when generation and stylesheet slot values happen to be equal.
        runtime_tx
            .send(CoreCommand::CssAbort {
                tab_id: old_tab,
                request_id: old_generation,
                stylesheet_slot_id: old_slot,
                url: old_url,
            })
            .unwrap();
        app.on_core_event(CoreEvent::NetworkDone {
            tab_id: new_tab,
            request_id: generation,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            response: response(&url, "text/css"),
            bytes_received: 14,
        });
        runtime_tx
            .send(cmd_rx.try_recv().expect("Browser forwards CssDone"))
            .unwrap();
        let decoded = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("new stylesheet decoded");
        assert!(
            matches!(&decoded, CoreEvent::CssDecodedBlock { tab_id, request_id, stylesheet_slot_id, css_block, .. } if *tab_id == new_tab && *request_id == generation && *stylesheet_slot_id == slot && css_block == "p{color:green}")
        );
        app.on_core_event(decoded);
        let done = events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("new stylesheet done");
        assert!(
            matches!(&done, CoreEvent::CssSheetDone { tab_id, request_id, stylesheet_slot_id, .. } if *tab_id == new_tab && *request_id == generation && *stylesheet_slot_id == slot)
        );
        app.on_core_event(done);
    }
}
