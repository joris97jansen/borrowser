use super::*;
use crate::{
    fixtures::FIXTURES,
    model::{Expectation, TestId},
};
use serde_json::{Value, json};
use std::{cell::RefCell, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    CaptureStarted(TestId),
    CaptureCleaned(TestId),
    StandaloneReleased,
    WorkerStarted,
}
thread_local! { static EVENTS: RefCell<Option<Vec<Event>>> = const { RefCell::new(None) }; }
pub(crate) fn record(event: Event) {
    EVENTS.with(|events| {
        if let Some(events) = events.borrow_mut().as_mut() {
            events.push(event);
        }
    });
}
fn trace() -> Vec<Event> {
    EVENTS.with(|events| events.borrow().clone().unwrap())
}
fn capture(color: [u8; 3]) -> chromium::ChromiumCapture {
    chromium::tests::controlled_capture(CanvasColor(color))
}
fn case(left: [u8; 3], right: [u8; 3]) -> report::Case {
    report::Case::observed(&FIXTURES[1], CanvasColor(left), capture(right))
}
fn publish_bytes(result: Result<report::Report, Failure>) -> (u8, Vec<u8>, String) {
    let mut out = vec![];
    let mut diagnostics = vec![];
    let code = publish(result, &mut out, &mut diagnostics).unwrap();
    (code, out, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn imported_capture_uses_the_existing_projection_and_no_operational_metadata() {
    let fixtures: Vec<_> = FIXTURES.iter().collect();
    let inputs: Vec<_> = fixtures
        .iter()
        .map(|f| {
            let FixtureKind::Runnable { html, .. } = f.kind else {
                panic!("runnable fixture required")
            };
            CaptureInput {
                id: f.id,
                source: f.source,
                html,
            }
        })
        .collect();
    let colors = [[18, 52, 86], [52, 86, 120]];
    let observations: Vec<_> = inputs.iter().zip(colors).map(|(input,color)| {
        let mut browser=serde_json::to_value(capture(color).identity).unwrap();
        browser["platform"]=json!("linux-x86_64");
        json!({"test_id":input.id.0,"source":input.source,"canvas_color_rgb8":color,"browser":browser})
    }).collect();
    let wire=serde_json::to_vec(&json!({"schema":"borrowser.chromium-canvas.v1","document_url":crate::environment::URL,
        "viewport":[640,480],"device_scale_factor":1,"sample_pixel":[32,32],"observations":observations})).unwrap();
    let first = publish_bytes(compare_captured(
        &fixtures,
        chromium::ingest::decode(wire.as_slice(), &inputs).unwrap(),
    ));
    let second = publish_bytes(compare_captured(
        &fixtures,
        chromium::ingest::decode(wire.as_slice(), &inputs).unwrap(),
    ));
    assert_eq!(first, second);
    assert_eq!(first.0, 0);
    assert!(first.2.is_empty());
    let report: Value = serde_json::from_slice(&first.1).unwrap();
    assert_eq!(report["summary"]["equal_fields"], 2);
    let text = String::from_utf8(first.1).unwrap();
    for field in ["run_id", "stack_id", "instance_id", "cleanup", "elapsed"] {
        assert!(!text.contains(field));
    }
}

#[test]
fn unit_exact_channels_and_stable_serialization() {
    for (right, status, channels) in [
        ([52, 86, 120], "equal", ["equal", "equal", "equal"]),
        ([53, 86, 120], "different", ["different", "equal", "equal"]),
        ([52, 87, 120], "different", ["equal", "different", "equal"]),
        ([52, 86, 121], "different", ["equal", "equal", "different"]),
        (
            [53, 87, 121],
            "different",
            ["different", "different", "different"],
        ),
    ] {
        let result = || publish_bytes(Ok(report::Report::new(vec![case([52, 86, 120], right)])));
        let first = result();
        assert_eq!(first.0, u8::from(status == "different"));
        assert!(first.2.is_empty());
        assert_eq!(first, result());
        let report: Value = serde_json::from_slice(&first.1).unwrap();
        let field = &report["cases"][0]["fields"][0];
        assert_eq!(field["id"], "canvas_color_rgb8");
        assert_eq!(field["status"], status);
        assert_eq!(field["borrowser"], json!([52, 86, 120]));
        assert_eq!(field["chromium"], json!(right));
        for (index, name) in ["red", "green", "blue"].iter().enumerate() {
            assert_eq!(field["channels"][index]["channel"], *name);
            assert_eq!(field["channels"][index]["status"], channels[index]);
        }
    }
}

#[test]
fn unit_unsupported_skip_and_empty_never_equal() {
    let observed = CanvasObservation::Observed(CanvasColor([0, 0, 0]));
    let unsupported = CanvasObservation::Unsupported {
        reason: "explicit unsupported classification",
    };
    for (left, right) in [
        (observed, unsupported),
        (unsupported, observed),
        (unsupported, unsupported),
    ] {
        assert_eq!(compare(left, right), FieldStatus::Unsupported);
        let field = serde_json::to_value(report::Field::new(left, right)).unwrap();
        assert_eq!(field["status"], "unsupported");
        assert_eq!(field["channels"], json!([]));
    }
    let unsupported_fixture = Fixture {
        kind: FixtureKind::Unsupported {
            reason: "unsupported fixture",
        },
        ..FIXTURES[0]
    };
    let skipped_fixture = Fixture {
        kind: FixtureKind::Skipped {
            reason: "explicit skip",
        },
        ..FIXTURES[1]
    };
    let (code, bytes, diagnostic) = publish_bytes(execute_selected(
        &[&unsupported_fixture, &skipped_fixture],
        None,
    ));
    assert_eq!(code, 3);
    assert!(diagnostic.is_empty());
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["summary"]["equal_fields"], 0);
    assert_eq!(report["summary"]["unsupported_fields"], 1);
    assert_eq!(report["summary"]["skipped_cases"], 1);
    assert_eq!(
        report["cases"][0]["fields"][0]["borrowser"]["unsupported"],
        "unsupported fixture"
    );
    assert_eq!(report["cases"][1]["skipped"], "explicit skip");
    assert_eq!(report::Report::new(vec![]).exit_code(), 3);
    assert!(execute_selected(&[], None).is_err());
    assert!(validate_selection(&[&FIXTURES[0], &FIXTURES[0]]).is_err());
}

#[test]
fn unit_complete_correspondence_required() {
    let inputs = [
        CaptureInput {
            id: FIXTURES[0].id,
            source: FIXTURES[0].source,
            html: b"",
        },
        CaptureInput {
            id: FIXTURES[1].id,
            source: FIXTURES[1].source,
            html: b"",
        },
    ];
    let captures = || {
        inputs
            .iter()
            .map(|input| CapturedFixture {
                id: input.id,
                source: input.source.into(),
                capture: capture([0, 0, 0]),
            })
            .collect::<Vec<_>>()
    };
    assert!(validate_captures(&inputs, &captures()).is_ok());
    assert!(validate_captures(&inputs, &[]).is_err());
    assert!(validate_captures(&inputs, &captures()[..1]).is_err());
    let mut wrong = captures();
    wrong.reverse();
    assert!(validate_captures(&inputs, &wrong).is_err());
    wrong = captures();
    wrong[1].id = wrong[0].id;
    assert!(validate_captures(&inputs, &wrong).is_err());
    wrong = captures();
    wrong[1].source = "wrong source".into();
    assert!(validate_captures(&inputs, &wrong).is_err());
    wrong = captures();
    wrong.push(CapturedFixture {
        id: TestId("extra"),
        source: "extra".into(),
        capture: capture([0, 0, 0]),
    });
    assert!(validate_captures(&inputs, &wrong).is_err());
}

#[test]
fn unit_expectations_are_independent_and_precedence_is_explicit() {
    for (expectation, observed, status, exit) in [
        (Expectation::Match, [1, 2, 3], "FAIL", 1),
        (
            Expectation::KnownFailure { reason: "known" },
            [1, 2, 3],
            "XFAIL",
            0,
        ),
        (
            Expectation::KnownFailure { reason: "stale" },
            [52, 86, 120],
            "XPASS",
            1,
        ),
    ] {
        let FixtureKind::Runnable { html, expected, .. } = FIXTURES[1].kind else {
            unreachable!()
        };
        let fixture = Fixture {
            kind: FixtureKind::Runnable {
                html,
                expected,
                expectation,
            },
            ..FIXTURES[1]
        };
        let report = report::Report::new(vec![report::Case::observed(
            &fixture,
            CanvasColor(observed),
            capture(observed),
        )]);
        assert_eq!(report.exit_code(), exit);
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["cases"][0]["fields"][0]["status"], "equal");
        assert_eq!(json["cases"][0]["borrowser_expectation"]["status"], status);
        assert_eq!(
            json["cases"][0]["borrowser_expectation"]["expected_rgb8"],
            json!([52, 86, 120])
        );
    }
    let cases = vec![
        case([52, 86, 120], [0, 0, 0]),
        report::Case::skipped(&FIXTURES[0], "skip"),
    ];
    assert_eq!(report::Report::new(cases).exit_code(), 1);
    let cases = vec![
        case([52, 86, 120], [52, 86, 120]),
        report::Case::skipped(&FIXTURES[0], "skip"),
    ];
    assert_eq!(report::Report::new(cases).exit_code(), 3);
}

#[test]
fn unit_errors_have_empty_output_and_preserve_primary_and_cleanup() {
    for code in [
        "parser.execution",
        "capture.missing",
        "capture.overlap",
        "required-data",
        "configuration",
    ] {
        let result = publish_bytes(Err(
            HarnessError::new(code, "required observation failed").into()
        ));
        assert_eq!(result.0, 2);
        assert!(result.1.is_empty());
        assert!(result.2.contains(code));
    }
    let mut diagnostics = vec![];
    let failure = chromium::configuration("original primary");
    failure.write_diagnostics(&mut diagnostics).unwrap();
    assert!(
        String::from_utf8(diagnostics)
            .unwrap()
            .contains("original primary")
    );
    // The actual failure contains both categories; it is not a field outcome.
    let failure = chromium::tests::controlled_comparison_failure();
    let result = publish_bytes(Err(failure.into()));
    assert_eq!(result.0, 2);
    assert!(result.1.is_empty());
    assert!(result.2.contains("original primary"));
    assert!(result.2.contains("chromium.cleanup"));
}

struct FailingWriter {
    bytes: Vec<u8>,
    limit: usize,
    fail_flush: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() == self.limit {
            return Err(io::Error::other("injected write failure"));
        }
        let count = bytes.len().min(self.limit - self.bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn unit_writer_errors_include_partial_output_and_flush() {
    for (limit, fail_flush) in [(0, false), (23, false), (usize::MAX, true)] {
        let mut writer = FailingWriter {
            bytes: vec![],
            limit,
            fail_flush,
        };
        let mut diagnostics = vec![];
        let result = report::Report::new(vec![case([52, 86, 120], [52, 86, 120])]);
        assert_eq!(
            publish(Ok(result), &mut writer, &mut diagnostics).unwrap(),
            2
        );
        assert!(
            String::from_utf8(diagnostics)
                .unwrap()
                .contains("comparison.write")
        );
        if fail_flush {
            assert!(writer.bytes.ends_with(b"\n"));
        } else {
            assert_eq!(writer.bytes.len(), limit);
        }
    }
}

#[test]
fn unit_diagnostic_errors_propagate_without_publishing() {
    for limit in [0, 17] {
        for failure in [
            Failure::from(HarnessError::new("required-data", "missing observation")),
            Failure::from(chromium::tests::controlled_comparison_failure()),
        ] {
            let mut out = vec![];
            let mut diagnostics = FailingWriter {
                bytes: vec![],
                limit,
                fail_flush: false,
            };
            let error = publish(Err(failure), &mut out, &mut diagnostics).unwrap_err();
            assert_eq!(error.to_string(), "injected write failure");
            assert!(out.is_empty());
            assert_eq!(diagnostics.bytes.len(), limit);
            assert!(b"ERROR ".starts_with(&diagnostics.bytes[..limit.min(6)]));
        }
    }
}

#[test]
fn unit_publication_and_diagnostic_errors_preserve_partial_output() {
    for (limit, fail_flush) in [(0, false), (23, false), (usize::MAX, true)] {
        for diagnostic_limit in [0, 17] {
            let mut writer = FailingWriter {
                bytes: vec![],
                limit,
                fail_flush,
            };
            let mut diagnostics = FailingWriter {
                bytes: vec![],
                limit: diagnostic_limit,
                fail_flush: false,
            };
            let result = report::Report::new(vec![case([52, 86, 120], [52, 86, 120])]);
            let error = publish(Ok(result), &mut writer, &mut diagnostics).unwrap_err();
            assert_eq!(error.to_string(), "injected write failure");
            assert_eq!(diagnostics.bytes.len(), diagnostic_limit);
            if fail_flush {
                assert!(writer.bytes.ends_with(b"\n"));
                assert!(serde_json::from_slice::<Value>(&writer.bytes).is_ok());
            } else {
                assert_eq!(writer.bytes.len(), limit);
            }
        }
    }
}

#[test]
#[ignore = "requires pinned Chromium; outer wrapper launches isolated composition scenarios"]
fn real_chromium_comparison_composition() {
    std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit real test requires Chromium");
    for scenario in [
        "multiple",
        "css",
        "later-capture-failure",
        "borrowser-failure",
        "cleanup-failure",
        "cancel",
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "comparison::tests::isolated_case",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("AG3_ISOLATED_CASE", scenario)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{scenario}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        eprintln!("AG3 isolated scenario {scenario}: passed");
    }
}

#[test]
#[ignore = "private isolated entry; real_chromium_comparison_composition invokes it"]
fn isolated_case() {
    let scenario = std::env::var("AG3_ISOLATED_CASE").expect("private isolated scenario");
    let executable = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit real test requires Chromium");
    EVENTS.with(|events| *events.borrow_mut() = Some(vec![]));
    let mut fixtures = FIXTURES.to_vec();
    match scenario.as_str() {
        "multiple" => {},
        "css" => {
            let FixtureKind::Runnable { html, expected, expectation } = fixtures[1].kind else { unreachable!() };
            let changed = std::str::from_utf8(html).unwrap().replace("#345678", "#2468ac");
            fixtures[1].kind = FixtureKind::Runnable { html: Box::leak(changed.into_bytes().into_boxed_slice()), expected, expectation };
        },
        "later-capture-failure" => fixtures[1].kind = FixtureKind::Runnable { html: b"<!doctype html><link rel=canonical href=/inert>", expected: CanvasColor([0,0,0]), expectation: Expectation::Match },
        "borrowser-failure" => fixtures[1].kind = FixtureKind::Runnable { html: b"<!doctype html><html><head><style>html,body{margin:0;padding:0;height:0}html{background-color:#123456}div{height:100px;background-color:red}</style></head><body><div></div></body></html>", expected: CanvasColor([0,0,0]), expectation: Expectation::Match },
        "cleanup-failure" | "cancel" => chromium::tests::arm_comparison_cleanup_failure(scenario == "cancel"),
        _ => panic!("unknown scenario"),
    }
    let selected: Vec<_> = fixtures.iter().collect();
    let result = execute_selected(&selected, Some(Path::new(&executable)));
    if scenario == "cleanup-failure" {
        chromium::tests::assert_comparison_cleanup_failure(
            result
                .as_ref()
                .err()
                .expect("cleanup failure")
                .chromium_failure(),
        );
    }
    let (code, bytes, diagnostics) = publish_bytes(result);
    let events = trace();
    match scenario.as_str() {
        "multiple" | "css" | "borrowser-failure" => {
            assert_eq!(
                events,
                vec![
                    Event::CaptureStarted(fixtures[0].id),
                    Event::CaptureCleaned(fixtures[0].id),
                    Event::CaptureStarted(fixtures[1].id),
                    Event::CaptureCleaned(fixtures[1].id),
                    Event::StandaloneReleased,
                    Event::WorkerStarted,
                    Event::WorkerStarted
                ]
            );
            if scenario == "borrowser-failure" {
                assert_eq!(code, 2);
                assert!(bytes.is_empty());
                assert!(diagnostics.contains("capture.overlap"));
            } else {
                assert_eq!(code, u8::from(scenario == "css"));
                assert!(diagnostics.is_empty());
                let json: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(json["summary"]["equal_fields"], 2);
                let expected = if scenario == "css" {
                    [36, 104, 172]
                } else {
                    [52, 86, 120]
                };
                for engine in ["borrowser", "chromium"] {
                    assert_eq!(
                        json["cases"][1]["captures"][engine]["canvas_color_rgb8"],
                        json!(expected)
                    );
                }
                assert_eq!(
                    json["cases"][1]["borrowser_expectation"]["status"],
                    if scenario == "css" { "FAIL" } else { "PASS" }
                );
            }
        }
        "later-capture-failure" => {
            assert_eq!(code, 2);
            assert!(bytes.is_empty());
            assert!(diagnostics.contains("chromium.navigation"));
            assert_eq!(
                events,
                vec![
                    Event::CaptureStarted(fixtures[0].id),
                    Event::CaptureCleaned(fixtures[0].id),
                    Event::CaptureStarted(fixtures[1].id)
                ]
            );
        }
        "cleanup-failure" | "cancel" => {
            assert_eq!(code, 2);
            assert!(bytes.is_empty());
            assert_eq!(events, vec![Event::CaptureStarted(fixtures[0].id)]);
            assert!(diagnostics.contains(if scenario == "cancel" {
                "chromium.cancelled"
            } else {
                "chromium.cleanup"
            }));
            if scenario == "cleanup-failure" {
                chromium::tests::recover_comparison_cleanup_failure();
            }
        }
        _ => unreachable!(),
    }
}
impl Failure {
    fn chromium_failure(&self) -> &chromium::Failure {
        match self {
            Self::Chromium(failure) => failure,
            _ => panic!("expected Chromium failure"),
        }
    }
}
