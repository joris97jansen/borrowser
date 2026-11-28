use super::*;
use std::sync::mpsc;
pub(super) fn chunk(slot: u64, bytes: &[u8]) -> CoreCommand {
    CoreCommand::CssChunk {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: StylesheetSlotId(slot),
        url: format!("fixture-{slot}"),
        bytes: bytes.to_vec(),
        decision: decoding::select_stylesheet_decoding(bytes, None, None).unwrap(),
    }
}
pub(super) fn done(slot: u64) -> CoreCommand {
    CoreCommand::CssDone {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: StylesheetSlotId(slot),
        url: format!("fixture-{slot}"),
    }
}
fn execute(commands: Vec<CoreCommand>, refuse: bool) -> Vec<CoreEvent> {
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    for c in commands {
        tx.send(c).unwrap();
    }
    drop(tx);
    allocation_tests::REFUSE_SESSION.with(|v| v.set(refuse));
    run(rx, events);
    received.into_iter().collect()
}
#[test]
fn aj8_a1_production_handoff_is_one_complete_body_then_done() {
    for body in [b"".as_slice(), b"p{color:red}", b"\xef\xbb\xbf"] {
        let events = execute(vec![chunk(1, body), done(1)], false);
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0],CoreEvent::CssDecodedBlock {decoded,..} if decoded.text().is_ok())
        );
        assert!(matches!(&events[1], CoreEvent::CssSheetDone { .. }));
    }
}
#[test]
fn aj8_a1_initial_runtime_storage_refusal_has_no_tombstone() {
    let events = execute(vec![chunk(1, b"x"), done(1), chunk(2, b""), done(2)], true);
    assert_eq!(events.len(), 3);
    assert!(matches!(
        &events[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::SessionReservation,
            ..
        }
    ));
    assert!(matches!(
        &events[1],
        CoreEvent::CssDecodedBlock {
            stylesheet_slot_id: StylesheetSlotId(2),
            ..
        }
    ));
    assert!(matches!(
        &events[2],
        CoreEvent::CssSheetDone {
            stylesheet_slot_id: StylesheetSlotId(2),
            ..
        }
    ));
}
#[test]
fn aj8_a1_complete_input_bounds_duplicates_abort_and_channels() {
    let duplicate = execute(vec![chunk(1, b"a"), chunk(1, b"b"), done(1)], false);
    assert_eq!(duplicate.len(), 1);
    assert!(matches!(
        &duplicate[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::DuplicateChunk,
            ..
        }
    ));
    let abort = CoreCommand::CssAbort {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: StylesheetSlotId(1),
        url: "fixture-1".into(),
    };
    assert!(execute(vec![chunk(1, b"\xe2"), abort, done(1)], false).is_empty());
    let mut wrong = done(1);
    if let CoreCommand::CssDone { url, .. } = &mut wrong {
        *url = "wrong".into();
    }
    let events = execute(vec![chunk(1, b"x"), wrong], false);
    assert!(matches!(
        &events[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::CommandProvenance,
            ..
        }
    ));
    assert!(execute(vec![done(99)], false).is_empty());
    let mut short = chunk(1, b"xx");
    if let CoreCommand::CssChunk { bytes, .. } = &mut short {
        bytes.pop();
    }
    assert!(matches!(
        &execute(vec![short, done(1)], false)[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::InputLength,
            ..
        }
    ));
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    drop(received);
    tx.send(chunk(1, b"x")).unwrap();
    tx.send(done(1)).unwrap();
    drop(tx);
    run(rx, events);
}

#[test]
fn aj8_a1_bounds_context_silent_cleanup_and_duplicate_done() {
    let mut excess = chunk(1, b"x");
    if let CoreCommand::CssChunk { bytes, .. } = &mut excess {
        bytes.push(b'x');
    }
    assert!(matches!(
        &execute(vec![excess, done(1)], false)[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::InputLength,
            ..
        }
    ));
    let over = chunk(1, &vec![b'x'; tools::common::MAX_STYLESHEET_BYTES + 1]);
    assert!(matches!(
        &execute(vec![over, done(1)], false)[0],
        CoreEvent::CssRuntimeFailed {
            failure: CssRuntimeFailure::InputBound,
            ..
        }
    ));
    assert!(
        execute(vec![chunk(1, b"\xe2")], false).is_empty(),
        "closed command input cannot fabricate EOF"
    );
    let abort = CoreCommand::CssAbort {
        tab_id: 1,
        request_id: 7,
        stylesheet_slot_id: StylesheetSlotId(1),
        url: "wrong".into(),
    };
    let events = execute(vec![chunk(1, b"x"), abort, done(1), done(1)], false);
    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0],CoreEvent::CssDecodedBlock{decoded,..}if decoded.text()==Ok("x")));
}
