use crate::html5::shared::{
    EventPosition, PATH_WORK, ParseErrorCode, ParserInputCompletion, ParserRecoveryAction,
    ParserStage, PositionUnavailableReason, TreeConstructionParseErrorCode, WhatwgParseErrorCode,
};
use crate::{DocumentMode, HtmlParseOptions, HtmlParser};

fn run(
    source: &[u8],
    chunk: usize,
    runtime: bool,
) -> (
    crate::ParseOutput,
    Option<crate::html5::shared::HtmlParserDiagnosticReport>,
) {
    let mut options = HtmlParseOptions::default();
    options.error_policy.track = false;
    options.error_policy.max_stored = 0;
    let mut parser = if runtime {
        HtmlParser::new_with_runtime_diagnostics(options)
    } else {
        HtmlParser::new(options)
    }
    .unwrap();
    for part in source.chunks(chunk.max(1)) {
        parser.push_bytes(part).unwrap();
        parser.pump().unwrap();
    }
    parser.finish().unwrap();
    let report = parser.take_runtime_diagnostics();
    assert!(parser.take_runtime_diagnostics().is_none());
    (parser.into_output().unwrap(), report)
}

#[test]
fn aj5_canonical_diagnostics() {
    let (output, report) = run(b"<div a='first' a='second'>", 1, true);
    let report = report.unwrap();
    assert_eq!(report.total(), 2);
    assert_eq!(report.retained(), 2);
    assert_eq!(report.dropped(), 0);
    assert!(report.counts_exact());
    assert_eq!(report.completion(), ParserInputCompletion::Finalized);
    assert_eq!(report.normalized_input_bytes(), 26);
    assert_eq!(output.document_mode, DocumentMode::Quirks);
    let events = report.events();
    assert_eq!(
        events[0].code,
        ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute)
    );
    assert_eq!(events[0].stage, ParserStage::Tokenizer);
    assert_eq!(
        events[0].recovery,
        Some(ParserRecoveryAction::DropDuplicateAttribute)
    );
    let EventPosition::Known(position) = &events[0].position else {
        panic!("known duplicate position")
    };
    assert_eq!(
        (
            position.normalized.utf8_byte_offset,
            position.normalized.line.get(),
            position.normalized.column.get()
        ),
        (15, 1, 16)
    );
    assert_eq!(
        events[1].code,
        ParseErrorCode::TreeConstruction(
            TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken
        )
    );
    assert_eq!(
        events[1].position,
        EventPosition::Unavailable(PositionUnavailableReason::ParserDidNotProvidePosition)
    );
    assert_eq!(
        events[1].recovery,
        Some(ParserRecoveryAction::ReprocessToken)
    );
}

#[test]
fn aj5_runtime_capture_uses_counted_decoder_and_ordinary_token_drain() {
    PATH_WORK.with(|w| w.set([0; 4]));
    let (_, r) = run(b"<!doctype html><p>\xff\xe2\x82", 1, true);
    let r = r.unwrap();
    assert_eq!(r.completion(), ParserInputCompletion::Finalized);
    PATH_WORK.with(|w| {
        let v = w.get();
        assert!(v[0] > 0);
        assert!(v[2] > 0);
        assert_eq!(v[1], 0);
        assert_eq!(v[3], 0);
    });
}

#[test]
fn aj5_diagnostics_preserve_parser_semantics() {
    for source in [
        b"<!doctype html><p>document</p>".as_slice(),
        b"<div>",
        b"<div a='first' a='second'>",
        b"<!doctype html><template><p>x</template><svg><g/></svg>",
        b"\xef\xbb\xbf<!doctype html><p>x",
        b"<!doctype html><p>\xff\xe2\x82",
    ] {
        for chunk in [1, 2, 7, source.len()] {
            let (a, _) = run(source, chunk, false);
            let (b, r) = run(source, chunk, true);
            assert_eq!(a.document_mode, b.document_mode);
            assert_eq!(a.patches, b.patches);
            assert_eq!(a.counters, b.counters);
            assert_eq!(r.unwrap().total(), b.counters.parse_errors);
        }
    }
}

#[test]
fn aj5_multibyte_decoder_and_crlf_positions_match_all_partitions() {
    let source = "<!doctype html>\r\né<div a='first' a='second'></div>".as_bytes();
    let (_, reference) = run(source, source.len(), true);
    let reference = reference.unwrap();
    assert_eq!(reference.normalized_input_bytes(), 50);
    let EventPosition::Known(p) = &reference.events()[0].position else {
        panic!("known position")
    };
    assert_eq!(
        (
            p.normalized.utf8_byte_offset,
            p.normalized.line.get(),
            p.normalized.column.get()
        ),
        (33, 2, 17)
    );
    for split in 0..=source.len() {
        let mut parser = HtmlParser::new_with_runtime_diagnostics(Default::default()).unwrap();
        for part in [&source[..split], &source[split..]] {
            parser.push_bytes(part).unwrap();
            parser.pump().unwrap();
        }
        parser.finish().unwrap();
        let r = parser.take_runtime_diagnostics().unwrap();
        assert_eq!(r.events(), reference.events());
        assert_eq!(r.total(), reference.total());
        assert_eq!(
            r.normalized_input_bytes(),
            reference.normalized_input_bytes()
        );
    }
}

#[test]
fn aj5_large_clean_and_late_diagnostic_input() {
    for size in [64 * 1024, 1024 * 1024] {
        let mut source = String::from("<!doctype html><p>");
        source.extend(std::iter::repeat_n('\n', size));
        source.push_str("</p>");
        let (_, r) = run(source.as_bytes(), 1024, true);
        let r = r.unwrap();
        assert_eq!(r.total(), 0);
        source.push_str("<div a='first' a='second'></div>");
        let (_, r) = run(source.as_bytes(), 1024, true);
        let r = r.unwrap();
        assert_eq!(r.total(), 1);
        let EventPosition::Known(p) = &r.events()[0].position else {
            panic!("known late error")
        };
        assert_eq!(p.normalized.line.get(), size as u64 + 1);
    }
}

#[test]
fn aj5_decoder_eof_does_not_certify_failed_parser_finalization() {
    crate::html5::shared::FAIL_AFTER_DECODER_EOF.with(|f| f.set(true));
    let mut parser = HtmlParser::new_with_runtime_diagnostics(Default::default()).unwrap();
    parser.push_bytes(b"<p>\xe2").unwrap();
    assert!(parser.finish().is_err());
    assert!(parser.decode_report().eof_processed);
    assert_eq!(
        parser.take_runtime_diagnostics().unwrap().completion(),
        ParserInputCompletion::PrefixOnly
    );
}

#[test]
fn aj5_finalized_report_survives_mirror_poisoning() {
    let mut parser = HtmlParser::new_with_runtime_diagnostics(Default::default()).unwrap();
    parser.push_bytes(b"<p a=x a=y>").unwrap();
    parser.finish().unwrap();
    assert!(
        parser
            .apply_patches(&[crate::DomPatch::AppendChild {
                parent: crate::PatchKey(u32::MAX),
                child: crate::PatchKey(u32::MAX - 1)
            }])
            .is_err()
    );
    let r = parser.take_runtime_diagnostics().unwrap();
    assert_eq!(r.completion(), ParserInputCompletion::Finalized);
    assert_eq!(r.total(), 2);
}

#[cfg(feature = "parser-failure-injection")]
#[test]
fn aj5_fatal_parser_retains_prefix_diagnostics() {
    use crate::html5::shared::{ParserFailureInjection, ParserReservationSite};
    let injection = ParserFailureInjection::new(
        ParserReservationSite::TemplateChildStorage,
        std::num::NonZeroU64::MIN,
    );
    let mut p = HtmlParser::new_with_runtime_diagnostics_and_failure_injection(
        Default::default(),
        injection,
    )
    .unwrap();
    p.push_bytes(b"<template a=x a=y>").unwrap();
    assert!(p.pump().is_err());
    let r = p.take_runtime_diagnostics().unwrap();
    assert_eq!(r.completion(), ParserInputCompletion::PrefixOnly);
    assert!(r.total() > 0);
    assert!(p.finish().is_err());
}

#[cfg(feature = "parser-conformance")]
#[test]
fn aj5_conformance_recorder_and_zero_cap_requests_remain_isolated() {
    use crate::html5::shared::{ParserObservationConfig, SurfaceCaptureRequest};
    for tokens in [true, false] {
        PATH_WORK.with(|w| w.set([0; 4]));
        let request = SurfaceCaptureRequest::Capture { capacity: 0 };
        let config = if tokens {
            ParserObservationConfig {
                tokens: request,
                ..Default::default()
            }
        } else {
            ParserObservationConfig {
                implementation_diagnostics: request,
                ..Default::default()
            }
        };
        let mut p = HtmlParser::new_with_observations(Default::default(), config).unwrap();
        assert!(p.take_runtime_diagnostics().is_none());
        p.push_bytes(b"<!doctype html><p>\xff\xe2").unwrap();
        p.finish().unwrap();
        assert_eq!(p.decode_report().replacements.total(), 2);
        assert_eq!(p.counters().decode_errors, 2);
        let capture = p.take_observations_for_conformance().unwrap().unwrap();
        if tokens {
            assert!(capture.tokens.dropped > 0);
            PATH_WORK.with(|w| {
                let v = w.get();
                assert!(v[0] > 0);
                assert_eq!(v[1], 0);
                assert!(v[3] > 0);
            });
        } else {
            assert_eq!(capture.implementation_diagnostics.dropped, 2);
            PATH_WORK.with(|w| {
                let v = w.get();
                assert!(v[1] > 0);
                assert_eq!(v[0], 0);
                assert_eq!(v[3], 0);
            });
        }
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn aj5_production_and_conformance_canonical_events_are_identical() {
    use crate::html5::shared::{ParserObservationConfig, SurfaceCaptureRequest};
    for source in [
        "<div a='first' a='second'>",
        "<!doctype html>\r\né<div a='first' a='second'></div>",
        "<template><table>x</table></template>",
    ] {
        let (_, runtime) = run(source.as_bytes(), 1, true);
        let mut p = HtmlParser::new_with_observations(
            Default::default(),
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 128 },
                ..Default::default()
            },
        )
        .unwrap();
        for b in source.as_bytes().chunks(1) {
            p.push_bytes(b).unwrap();
            p.pump().unwrap();
        }
        p.finish().unwrap();
        let capture = p.take_observations_for_conformance().unwrap().unwrap();
        assert_eq!(runtime.unwrap().events(), capture.parse_errors.items);
    }
}

#[test]
fn aj5_capture_refusal_or_conversion_failure_does_not_fail_or_change_parser_output() {
    use crate::html5::shared::DiagnosticCaptureStatus;
    let source = b"<div a='first' a='second'>";
    let (control, _) = run(source, 1, false);
    for fault in [
        (true, false, None),
        (false, true, None),
        (false, false, Some(0)),
    ] {
        crate::html5::shared::NEXT_CAPTURE_FAILURE.with(|v| v.set(fault));
        let (output, report) = run(source, 1, true);
        let report = report.unwrap();
        assert_eq!(report.completion(), ParserInputCompletion::Finalized);
        assert!(matches!(
            report.capture(),
            DiagnosticCaptureStatus::Incomplete { .. }
        ));
        assert_eq!(report.total(), 2);
        assert_eq!(report.total(), report.retained() as u64 + report.dropped());
        assert!(report.counts_exact());
        assert_eq!(control.patches, output.patches);
        assert_eq!(control.counters, output.counters);
        assert_eq!(control.document_mode, output.document_mode);
    }
}

#[test]
fn aj5_actual_parser_storage_scan_and_observation_work_are_bounded() {
    use crate::html5::shared::LAST_WORK;
    for size in [65536, 1048576] {
        for kind in ["clean", "late", "heavy"] {
            let mut source = String::from("<!doctype html><body>");
            if kind == "heavy" {
                let group = format!("<div>{}</div>", "<span a=x a=y></span>".repeat(32));
                while source.len() + group.len() <= size {
                    source.push_str(&group);
                }
            }
            let tail = if kind == "late" { "<p a=x a=y>" } else { "" };
            source.extend(std::iter::repeat_n('\n', size - source.len() - tail.len()));
            source.push_str(tail);
            for width in [1, 1024] {
                PATH_WORK.with(|w| w.set([0; 4]));
                let (_, report) = run(source.as_bytes(), width, true);
                let report = report.unwrap();
                LAST_WORK.with(|work| {
                    let w = work.borrow();
                    let rows = report.retained();
                    let max = report.events().iter().filter_map(|event| match &event.position {
                        EventPosition::Known(position) => Some(position.normalized.utf8_byte_offset as usize),
                        EventPosition::Unavailable(_) => None,
                    }).max();
                    assert_eq!(rows, (report.total() as usize).min(128));
                    assert_eq!(w.pending_reservations, usize::from(rows != 0));
                    assert_eq!(w.output_reservations, usize::from(rows != 0));
                    assert!(w.high_water <= 128);
                    assert!(!w.position_index_present);
                    assert_eq!((w.position_checkpoints, w.unrelated_surfaces_requested), (0, 0));
                    assert!(w.peak_heap_bytes <= (128 + rows) * std::mem::size_of::<crate::html5::shared::ParseErrorEvent>());
                    assert_eq!(w.scans, usize::from(max.is_some()));
                    assert_eq!(w.scalar_visits, source[..max.unwrap_or(0)].chars().count());
                    if kind == "clean" {assert_eq!((w.peak_heap_bytes, w.scans, w.scalar_visits), (0, 0, 0));}
                    eprintln!("AJ5_PARSER_WORK size={size} kind={kind} width={width} retained={rows} work={w:?}");
                });
                PATH_WORK.with(|w| {
                    let counts = w.get();
                    assert!(counts[0] > 0 && counts[2] > 0);
                    assert_eq!((counts[1], counts[3]), (0, 0));
                });
            }
        }
    }
}
