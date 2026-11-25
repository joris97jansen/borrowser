use bus::{CoreCommand, CoreEvent, HtmlParseFailure};
use core_types::DomHandle;
use html::internal::{DiagnosticCaptureStatus, ParserInputCompletion};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

fn terminal(events: &Receiver<CoreEvent>) -> CoreEvent {
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("bounded terminal wait");
        if matches!(event, CoreEvent::HtmlParseFinished { .. }) {
            return event;
        }
    }
}

#[test]
fn aj5_runtime_diagnostics_are_finalized_and_partition_independent() {
    let source = b"<div a='first' a='second'>";
    let mut expected = None;
    for width in [1, 2, 7, source.len()] {
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        crate::start_parse_runtime(rx, events);
        tx.send(CoreCommand::ParseHtmlStart {
            tab_id: 1,
            request_id: 1,
        })
        .unwrap();
        for bytes in source.chunks(width) {
            tx.send(CoreCommand::ParseHtmlChunk {
                tab_id: 1,
                request_id: 1,
                bytes: bytes.to_vec(),
            })
            .unwrap();
        }
        tx.send(CoreCommand::ParseHtmlDone {
            tab_id: 1,
            request_id: 1,
        })
        .unwrap();
        let CoreEvent::HtmlParseFinished {
            result,
            diagnostics: Some(report),
            decode_report: Some(decoder),
            ..
        } = terminal(&received)
        else {
            panic!("report");
        };
        assert!(result.is_ok());
        assert!(decoder.eof_processed);
        assert_eq!(report.completion(), ParserInputCompletion::Finalized);
        assert_eq!(*report.capture(), DiagnosticCaptureStatus::Complete);
        assert_eq!(
            (report.total(), report.retained(), report.dropped()),
            (2, 2, 0)
        );
        if let Some(events) = &expected {
            assert_eq!(events, report.events());
        } else {
            expected = Some(report.events().to_vec());
        }
        drop(tx);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(10)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}

#[test]
fn aj5_input_closure_is_prefix_only_and_cancellation_is_silent() {
    for cancel in [false, true] {
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        crate::start_parse_runtime(rx, events);
        tx.send(CoreCommand::ParseHtmlStart {
            tab_id: 1,
            request_id: 2,
        })
        .unwrap();
        tx.send(CoreCommand::ParseHtmlChunk {
            tab_id: 1,
            request_id: 2,
            bytes: b"<p a=x a=y>".to_vec(),
        })
        .unwrap();
        if cancel {
            tx.send(CoreCommand::CancelRequest {
                tab_id: 1,
                request_id: 2,
            })
            .unwrap();
        }
        drop(tx);
        if !cancel {
            let CoreEvent::HtmlParseFinished {
                result: Err(HtmlParseFailure::InputClosed),
                diagnostics: Some(report),
                decode_report: Some(decoder),
                ..
            } = terminal(&received)
            else {
                panic!("prefix terminal")
            };
            assert_eq!(report.completion(), ParserInputCompletion::PrefixOnly);
            assert!(!decoder.eof_processed);
            assert!(report.total() > 0);
        }
        while let Ok(event) = received.recv_timeout(Duration::from_secs(10)) {
            assert!(!matches!(event, CoreEvent::HtmlParseFinished { .. }));
        }
        assert!(matches!(
            received.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
}

#[test]
fn aj5_restart_retires_diagnostics_and_duplicate_done_is_silent() {
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    crate::start_parse_runtime(rx, events);
    tx.send(CoreCommand::ParseHtmlStart {
        tab_id: 1,
        request_id: 1,
    })
    .unwrap();
    tx.send(CoreCommand::ParseHtmlChunk {
        tab_id: 1,
        request_id: 1,
        bytes: b"<p a=x a=y>".to_vec(),
    })
    .unwrap();
    tx.send(CoreCommand::ParseHtmlStart {
        tab_id: 1,
        request_id: 1,
    })
    .unwrap();
    tx.send(CoreCommand::ParseHtmlChunk {
        tab_id: 1,
        request_id: 1,
        bytes: b"<!doctype html><p>clean".to_vec(),
    })
    .unwrap();
    for _ in 0..2 {
        tx.send(CoreCommand::ParseHtmlDone {
            tab_id: 1,
            request_id: 1,
        })
        .unwrap();
    }
    let CoreEvent::HtmlParseFinished {
        result: Ok(_),
        diagnostics: Some(report),
        ..
    } = terminal(&received)
    else {
        panic!("new report")
    };
    assert_eq!(report.total(), 0);
    drop(tx);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn aj5_known_closed_sink_does_not_extract_or_scan_diagnostics() {
    crate::state::DIAGNOSTIC_EXTRACTIONS.with(|n| n.set(0));
    let mut state = crate::state::RuntimeState::new(Instant::now(), 64, DomHandle(1)).unwrap();
    state.parser.push_bytes(b"<p a=x a=y>").unwrap();
    let (tx, rx) = mpsc::channel();
    drop(rx);
    crate::driver::handle_runtime_done(Box::new(state), &tx, 1, 1);
    crate::state::DIAGNOSTIC_EXTRACTIONS.with(|n| assert_eq!(n.get(), 0));
}

#[test]
fn aj5_finalized_parser_report_survives_later_internal_failure() {
    let mut state = crate::state::RuntimeState::new(Instant::now(), 64, DomHandle(1)).unwrap();
    state.parser.push_bytes(b"<p a=x a=y>").unwrap();
    state.document_mode = Some(html::DocumentMode::NoQuirks);
    let (tx, rx) = mpsc::channel();
    crate::driver::handle_runtime_done(Box::new(state), &tx, 1, 1);
    let CoreEvent::HtmlParseFinished {
        result: Err(HtmlParseFailure::DocumentModeChanged { .. }),
        diagnostics: Some(report),
        ..
    } = terminal(&rx)
    else {
        panic!("deliverable failure")
    };
    assert_eq!(report.completion(), ParserInputCompletion::Finalized);
    assert_eq!(report.total(), 2);
}

#[cfg(feature = "parser-failure-injection")]
#[test]
fn aj5_execution_failure_transports_prefix_diagnostics_once() {
    use html::ParserReservationSite;
    use html::internal::ParserFailureInjection;
    let injection = ParserFailureInjection::new(
        ParserReservationSite::TemplateChildStorage,
        std::num::NonZeroU64::MIN,
    );
    let mut state = crate::state::RuntimeState::new_with_failure_injection(
        Instant::now(),
        64,
        DomHandle(1),
        injection,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    assert!(crate::driver::handle_runtime_chunk(
        &mut state,
        b"<template a=x a=y>",
        &crate::PreviewPolicy::default(),
        Instant::now(),
        &tx,
        1,
        1
    ));
    let CoreEvent::HtmlParseFinished {
        result: Err(HtmlParseFailure::Execution(_)),
        diagnostics: Some(report),
        decode_report: Some(decoder),
        ..
    } = terminal(&rx)
    else {
        panic!("execution failure")
    };
    assert_eq!(report.completion(), ParserInputCompletion::PrefixOnly);
    assert!(report.total() > 0);
    assert!(!decoder.eof_processed);
    crate::driver::handle_runtime_done(Box::new(state), &tx, 1, 1);
    assert!(rx.try_recv().is_err());
}
