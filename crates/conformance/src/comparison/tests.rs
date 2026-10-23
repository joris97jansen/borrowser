use super::*;
use crate::model::{Execution, Expectation, TestId};
use serde_json::{Value, json};
use std::{cell::RefCell, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    CaptureStarted(TestId, chromium::DocumentRole),
    CaptureCleaned(TestId, chromium::DocumentRole),
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
#[cfg(target_os = "macos")]
pub(crate) fn macos_platform_failure_blocks_workers(executable: &Path) {
    EVENTS.with(|events| *events.borrow_mut() = Some(vec![]));
    let fixture = crate::fixtures::fixture("canvas/root");
    let result = execute_selected(&[fixture], Some(executable));
    let (code, output, diagnostics) = publish_bytes(result);
    assert_eq!(code, 2);
    assert!(output.is_empty());
    assert!(diagnostics.contains("chromium.platform"), "{diagnostics}");
    assert!(diagnostics.contains("ownership cleanup completeness is not proven"));
    // No capture, standalone cleanup or worker event may precede rejection.
    assert!(trace().is_empty(), "{:?}", trace());
    EVENTS.with(|events| *events.borrow_mut() = None);
}
// Called only by the existing isolated native process fixture.
pub(crate) fn startup_failure_blocks_workers(executable: &Path, expected_code: &str) {
    EVENTS.with(|events| *events.borrow_mut() = Some(vec![]));
    let fixture = crate::fixtures::fixture("canvas/root");
    let failure = execute_selected(&[fixture], Some(executable))
        .err()
        .expect("startup termination must fail the required capture");
    let Failure::Chromium(ref chromium) = failure else {
        panic!("{failure:?}")
    };
    assert_eq!(chromium.primary.as_ref().unwrap().code(), expected_code);
    assert!(chromium.cleanup.is_empty(), "{chromium:?}");
    assert_eq!(
        trace(),
        vec![Event::CaptureStarted(
            fixture.id,
            chromium::DocumentRole::Test
        )]
    );
    let (code, out, diagnostics) = publish_bytes(Err(failure));
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(diagnostics.contains(&format!("chromium.{expected_code}")));
    eprintln!(
        "startup capture failed; no Borrowser workers; exit={code}; trace={:?}",
        trace()
    );
    EVENTS.with(|events| *events.borrow_mut() = None);
}
fn capture(color: [u8; 3]) -> chromium::ChromiumCapture {
    chromium::tests::controlled_capture(CanvasColor(color))
}
fn case(left: [u8; 3], right: [u8; 3]) -> report::Case {
    report::Case::observed(
        crate::fixtures::fixture("canvas/cascade"),
        CanvasColor(left).into(),
        capture(right),
    )
    .unwrap()
}
fn publish_bytes(result: Result<report::Report, Failure>) -> (u8, Vec<u8>, String) {
    let mut out = vec![];
    let mut diagnostics = vec![];
    let code = publish(result, &mut out, &mut diagnostics).unwrap();
    (code, out, String::from_utf8(diagnostics).unwrap())
}

#[test]
fn imported_capture_uses_the_existing_projection_and_no_operational_metadata() {
    let fixtures: Vec<_> = crate::fixtures::baseline()[..2].iter().collect();
    let inputs: Vec<_> = fixtures
        .iter()
        .map(|f| {
            let html = f.document.html;
            CaptureInput {
                role: crate::chromium::DocumentRole::Test,
                metadata_links: "[]",
                request: crate::projection::ObservationRequest::CANVAS,
                id: f.id,
                source: f.document.source,
                html,
            }
        })
        .collect();
    let colors = [[18, 52, 86], [52, 86, 120]];
    let observations: Vec<_> = inputs.iter().zip(colors).map(|(input,color)| {
        let mut browser=serde_json::to_value(capture(color).identity).unwrap();
        browser["platform"]=json!("linux-x86_64");
        json!({"descriptor":input.descriptor(),"metadata_links":[],"canvas_color_rgb8":color,"inspection":null,"browser":browser})
    }).collect();
    let wire=serde_json::to_vec(&json!({"schema":"borrowser.chromium-phases.v2","document_url":crate::environment::URL,
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
        ..*crate::fixtures::fixture("canvas/root")
    };
    let skipped_fixture = Fixture {
        kind: FixtureKind::Skipped {
            reason: "explicit skip",
        },
        ..*crate::fixtures::fixture("canvas/cascade")
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
    assert!(
        validate_selection(&[
            crate::fixtures::fixture("canvas/root"),
            crate::fixtures::fixture("canvas/root")
        ])
        .is_err()
    );
}

#[test]
fn unit_complete_correspondence_required() {
    let inputs = [
        CaptureInput {
            role: crate::chromium::DocumentRole::Test,
            metadata_links: "[]",
            request: crate::projection::ObservationRequest::CANVAS,
            id: crate::fixtures::fixture("canvas/root").id,
            source: crate::fixtures::fixture("canvas/root").document.source,
            html: b"",
        },
        CaptureInput {
            role: crate::chromium::DocumentRole::Test,
            metadata_links: "[]",
            request: crate::projection::ObservationRequest::CANVAS,
            id: crate::fixtures::fixture("canvas/cascade").id,
            source: crate::fixtures::fixture("canvas/cascade").document.source,
            html: b"",
        },
    ];
    let captures = || {
        inputs
            .iter()
            .map(|input| CapturedFixture {
                descriptor: input.descriptor(),
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
    wrong[1].descriptor.case_id = wrong[0].descriptor.case_id.clone();
    assert!(validate_captures(&inputs, &wrong).is_err());
    wrong = captures();
    wrong[1].descriptor.source_sha256 = "wrong hash".into();
    assert!(validate_captures(&inputs, &wrong).is_err());
    wrong = captures();
    wrong.push(CapturedFixture {
        descriptor: crate::chromium::CaptureDescriptor {
            case_id: "extra".into(),
            ..inputs[0].descriptor()
        },
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
        let html = (crate::fixtures::fixture("canvas/cascade")).document.html;
        let expected = (crate::fixtures::fixture("canvas/cascade"))
            .independent()
            .unwrap()
            .expected;
        let fixture = Fixture {
            document: crate::fixtures::fixture("canvas/cascade")
                .document
                .with_test_bytes(html),
            kind: FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected,
                    disposition: expectation,
                }),
            },
            ..*crate::fixtures::fixture("canvas/cascade")
        };
        let report = report::Report::new(vec![
            report::Case::observed(&fixture, CanvasColor(observed).into(), capture(observed))
                .unwrap(),
        ]);
        assert_eq!(report.exit_code(), exit);
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["cases"][0]["fields"][0]["status"], "equal");
        assert_eq!(json["cases"][0]["borrowser_expectation"]["status"], status);
        assert_eq!(
            json["cases"][0]["borrowser_expectation"]["expected"]["canvas"]["value"],
            json!([52, 86, 120])
        );
    }
    let cases = vec![
        case([52, 86, 120], [0, 0, 0]),
        report::Case::skipped(crate::fixtures::fixture("canvas/root"), "skip"),
    ];
    assert_eq!(report::Report::new(cases).exit_code(), 1);
    let cases = vec![
        case([52, 86, 120], [52, 86, 120]),
        report::Case::skipped(crate::fixtures::fixture("canvas/root"), "skip"),
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

#[test]
fn unit_ag7_exclusions_enter_neither_execution_engine() {
    EVENTS.with(|e| *e.borrow_mut() = Some(vec![]));
    let fixtures = [
        crate::fixtures::fixture("wpt/unsupported/first-child-js"),
        crate::fixtures::fixture("wpt/unsupported/testdriver"),
    ];
    for f in fixtures {
        assert!(matches!(
            crate::run_fixture(f),
            Execution::Unsupported { .. }
        ));
    }
    let report = execute_selected(&fixtures, None).unwrap();
    assert_eq!(report.exit_code(), 3);
    assert!(trace().is_empty());
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["summary"]["executed_documents"], 0);
    assert!(
        value["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["captures"].is_null() && c["relationships"].is_null())
    );
    EVENTS.with(|e| *e.borrow_mut() = None);
}

fn style_capture(target: &str, background: &str) -> chromium::ChromiumCapture {
    let mut c = capture([0, 0, 0]);
    c.color = None;
    c.inspection=Some(serde_json::from_value(json!({"dom":null,"styles":[{"target":target,"values":["rgb(0, 0, 0)",background,"16px","block"]}],"layout":[],"inner_width":640,"inner_height":480,"layout_width":640})).unwrap());
    c
}

#[test]
fn unit_ag7_reference_descriptor_binds_bytes_role_request_and_order() {
    let fixture = crate::fixtures::fixture("wpt/rgb-comments-p6");
    let mut inputs = chromium::inputs(&[fixture]);
    assert_eq!(inputs.len(), 2);
    let captures = || {
        inputs
            .iter()
            .map(|i| {
                let mut capture = style_capture("p6", "rgb(10, 150, 50)");
                capture.metadata_links = serde_json::from_str(i.metadata_links).unwrap();
                CapturedFixture {
                    descriptor: i.descriptor(),
                    capture,
                }
            })
            .collect::<Vec<_>>()
    };
    assert!(validate_captures(&inputs, &captures()).is_ok());
    for fault in 0..7 {
        let mut bad = captures();
        match fault {
            0 => {
                bad.pop();
            }
            1 => bad.reverse(),
            2 => bad[1].descriptor = bad[0].descriptor.clone(),
            3 => bad[1].descriptor.source_sha256 = "0".repeat(64),
            4 => {
                bad[1].descriptor.request =
                    crate::projection::ObservationRequest::CANVAS.descriptor()
            }
            5 => bad[1].capture.metadata_links.clear(),
            6 => bad.extend(captures()),
            _ => unreachable!(),
        }
        assert!(validate_captures(&inputs, &bad).is_err(), "{fault}");
    }
    let original = captures();
    let before = inputs[1].descriptor();
    inputs[1].html = b"<!doctype html><p>test-owned changed reference</p>";
    assert_ne!(inputs[1].descriptor().source_sha256, before.source_sha256);
    assert!(validate_captures(&inputs, &original).is_err());
    inputs[1].html = fixture.reference().unwrap().1.html;
    inputs[1].request = crate::projection::ObservationRequest::CANVAS;
    assert_ne!(inputs[1].descriptor().request, before.request);
    assert!(validate_captures(&inputs, &original).is_err());
}

#[test]
fn unit_ag7_relationships_never_hide_independent_or_cross_engine_failures() {
    use crate::{
        model::{Checks, Expected, IndependentExpectation, RelationshipKind},
        relationship::tests::color,
    };
    let mut fixture = *crate::fixtures::fixture("wpt-style/background-color-mismatch");
    let capture_b = |rgba| crate::projection::BorrowserCapture {
        observations: color("probe", rgba),
        layout_viewport_width: None,
        layout_artifact_source_width: None,
    };
    let (_, reference) = fixture.reference().unwrap();
    let expected = Box::leak(
        serde_json::to_string(&color("probe", [0, 128, 0, 255]))
            .unwrap()
            .into_boxed_str(),
    );
    fixture.kind = FixtureKind::Runnable {
        checks: Checks::Reference {
            kind: RelationshipKind::Mismatch,
            reference,
            independent: Some(IndependentExpectation {
                expected: Expected::Projection(expected),
                disposition: Expectation::KnownFailure {
                    reason: "test-owned independent mismatch",
                },
            }),
        },
    };
    let case = report::Case::observed_pair(
        &fixture,
        capture_b([255, 0, 0, 255]),
        style_capture("probe", "rgb(255, 0, 0)"),
        Some((
            capture_b([255, 0, 0, 255]),
            style_capture("probe", "rgb(255, 0, 0)"),
        )),
    )
    .unwrap();
    let report = report::Report::new(vec![case]);
    assert_eq!(report.exit_code(), 1);
    let v = serde_json::to_value(report).unwrap();
    assert_eq!(v["cases"][0]["borrowser_expectation"]["status"], "XFAIL");
    assert_eq!(v["summary"]["different_fields"], 0);
    assert_eq!(v["summary"]["relationship_failures"], 2);
    // Both mismatch relationships hold, but both same-document comparisons differ.
    fixture.kind = FixtureKind::Runnable {
        checks: Checks::Reference {
            kind: RelationshipKind::Mismatch,
            reference,
            independent: None,
        },
    };
    let case = report::Case::observed_pair(
        &fixture,
        capture_b([0, 128, 0, 255]),
        style_capture("probe", "rgb(0, 0, 255)"),
        Some((
            capture_b([255, 0, 0, 255]),
            style_capture("probe", "rgb(255, 255, 0)"),
        )),
    )
    .unwrap();
    let report = report::Report::new(vec![case]);
    assert_eq!(report.exit_code(), 1);
    let v = serde_json::to_value(report).unwrap();
    assert_eq!(v["summary"]["different_fields"], 2);
    assert_eq!(v["summary"]["relationship_failures"], 0);
    fixture.upstream = crate::fixtures::fixture("wpt/rgb-comments-p6").upstream;
    let case = report::Case::observed_pair(
        &fixture,
        capture_b([0, 128, 0, 255]),
        style_capture("probe", "rgb(0, 128, 0)"),
        Some((
            capture_b([255, 0, 0, 255]),
            style_capture("probe", "rgb(255, 0, 0)"),
        )),
    )
    .unwrap();
    let report = report::Report::new(vec![case]);
    assert_eq!(report.exit_code(), 3);
    let v = serde_json::to_value(report).unwrap();
    assert_eq!(v["summary"]["different_fields"], 0);
    assert_eq!(v["summary"]["runnable_upstream_incomplete_cases"], 1);
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
        "reference",
        "reference-capture-failure",
        "css",
        "later-capture-failure",
        "second-shutdown-cancel",
        "second-shutdown-expired",
        "second-shutdown-delayed",
        "borrowser-failure-delayed",
        "second-cleanup-failure",
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
            "{scenario}: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // Intentional harness failures can make the child test pass. Preserve
        // their primary/cleanup evidence as well as unexpected child failures.
        if !output.stderr.is_empty() {
            eprintln!(
                "AG3 isolated scenario {scenario}: status={} stdout={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
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
    let mut fixtures = crate::fixtures::baseline()[..2].to_vec();
    match scenario.as_str() {
        "multiple" => {}
        "second-shutdown-delayed" => chromium::tests::arm_second_shutdown_read_pause(),
        "reference" | "reference-capture-failure" => {
            let mut fixture = *crate::fixtures::fixture("wpt-style/background-color-mismatch");
            if scenario == "reference-capture-failure" {
                let (kind, reference) = fixture.reference().unwrap();
                fixture.kind = FixtureKind::Runnable {
                    checks: crate::model::Checks::Reference {
                        kind,
                        reference: reference
                            .with_test_bytes(b"<!doctype html><link rel=canonical href=/inert>"),
                        independent: None,
                    },
                };
            }
            fixtures = vec![fixture];
        }
        "css" => {
            let html = (fixtures[1]).document.html;
            let expected = (fixtures[1]).independent().unwrap().expected;
            let expectation = (fixtures[1]).independent().unwrap().disposition;
            let changed = std::str::from_utf8(html)
                .unwrap()
                .replace("#345678", "#2468ac");
            {
                fixtures[1].document = fixtures[1]
                    .document
                    .with_test_bytes(Box::leak(changed.into_bytes().into_boxed_slice()));
                fixtures[1].kind = FixtureKind::Runnable {
                    checks: crate::model::Checks::Independent(
                        crate::model::IndependentExpectation {
                            expected,
                            disposition: expectation,
                        },
                    ),
                };
            };
        }
        "later-capture-failure" => {
            fixtures[1].document = fixtures[1]
                .document
                .with_test_bytes(b"<!doctype html><link rel=canonical href=/inert>");
            fixtures[1].kind = FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected: crate::model::Expected::Canvas(CanvasColor([0, 0, 0])),
                    disposition: Expectation::Match,
                }),
            };
        }
        "borrowser-failure" | "borrowser-failure-delayed" => {
            if scenario == "borrowser-failure-delayed" {
                chromium::tests::arm_second_shutdown_read_pause();
            }
            fixtures[1].document=fixtures[1].document.with_test_bytes(b"<!doctype html><html><head><style>html,body{margin:0;padding:0;height:0}html{background-color:#123456}div{height:100px;background-color:red}</style></head><body><div></div></body></html>");
            fixtures[1].kind = FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected: crate::model::Expected::Canvas(CanvasColor([0, 0, 0])),
                    disposition: Expectation::Match,
                }),
            };
        }
        "second-shutdown-cancel" => chromium::tests::arm_shutdown_ack_cancellation(2),
        "second-shutdown-expired" => chromium::tests::arm_second_shutdown_expiry(),
        "cleanup-failure" | "second-cleanup-failure" | "cancel" => {
            chromium::tests::arm_comparison_cleanup_failure(
                scenario == "cancel",
                if scenario == "second-cleanup-failure" {
                    2
                } else {
                    1
                },
            )
        }
        _ => panic!("unknown scenario"),
    }
    let selected: Vec<_> = fixtures.iter().collect();
    let result = execute_selected(&selected, Some(Path::new(&executable)));
    if scenario == "second-shutdown-expired" {
        chromium::tests::assert_comparison_shutdown_timeout(
            result
                .as_ref()
                .err()
                .expect("capture budget expired")
                .chromium_failure(),
        );
        chromium::tests::assert_shutdown_cancellation_consumed();
    }
    if matches!(
        scenario.as_str(),
        "cleanup-failure" | "second-cleanup-failure"
    ) {
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
        "reference" | "reference-capture-failure" => {
            let mut expected = vec![
                Event::CaptureStarted(fixtures[0].id, chromium::DocumentRole::Test),
                Event::CaptureCleaned(fixtures[0].id, chromium::DocumentRole::Test),
                Event::CaptureStarted(fixtures[0].id, chromium::DocumentRole::Reference),
            ];
            if scenario == "reference" {
                expected.extend([
                    Event::CaptureCleaned(fixtures[0].id, chromium::DocumentRole::Reference),
                    Event::StandaloneReleased,
                    Event::WorkerStarted,
                    Event::WorkerStarted,
                ]);
                assert_eq!(code, 0);
                assert!(diagnostics.is_empty());
                let report: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(report["summary"]["executed_documents"], 2);
                assert_eq!(report["cases"].as_array().unwrap().len(), 1);
                assert_eq!(
                    report["cases"][0]["relationships"]["borrowser"]["status"],
                    "satisfied"
                );
            } else {
                assert_eq!(code, 2);
                assert!(bytes.is_empty());
                assert!(!diagnostics.is_empty());
            }
            assert_eq!(events, expected);
        }
        "multiple"
        | "css"
        | "borrowser-failure"
        | "second-shutdown-delayed"
        | "borrowser-failure-delayed" => {
            chromium::tests::assert_shutdown_cancellation_consumed();
            assert_eq!(
                events,
                vec![
                    Event::CaptureStarted(fixtures[0].id, chromium::DocumentRole::Test),
                    Event::CaptureCleaned(fixtures[0].id, chromium::DocumentRole::Test),
                    Event::CaptureStarted(fixtures[1].id, chromium::DocumentRole::Test),
                    Event::CaptureCleaned(fixtures[1].id, chromium::DocumentRole::Test),
                    Event::StandaloneReleased,
                    Event::WorkerStarted,
                    Event::WorkerStarted
                ],
                "scenario={scenario} exit={code} diagnostics={diagnostics}"
            );
            if matches!(
                scenario.as_str(),
                "borrowser-failure" | "borrowser-failure-delayed"
            ) {
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
        "later-capture-failure"
        | "second-shutdown-cancel"
        | "second-shutdown-expired"
        | "second-cleanup-failure" => {
            assert_eq!(code, 2);
            assert!(bytes.is_empty());
            assert!(diagnostics.contains(match scenario.as_str() {
                "second-shutdown-cancel" => "chromium.cancelled",
                "second-shutdown-expired" => "chromium.timeout",
                "second-cleanup-failure" => "chromium.cleanup",
                _ => "chromium.navigation",
            }));
            if scenario == "second-shutdown-cancel" {
                chromium::tests::assert_shutdown_cancellation_consumed();
            }
            if scenario == "second-cleanup-failure" {
                chromium::tests::recover_comparison_cleanup_failure();
            }
            assert_eq!(
                events,
                vec![
                    Event::CaptureStarted(fixtures[0].id, chromium::DocumentRole::Test),
                    Event::CaptureCleaned(fixtures[0].id, chromium::DocumentRole::Test),
                    Event::CaptureStarted(fixtures[1].id, chromium::DocumentRole::Test)
                ]
            );
        }
        "cleanup-failure" | "cancel" => {
            assert_eq!(code, 2);
            assert!(bytes.is_empty());
            assert_eq!(
                events,
                vec![Event::CaptureStarted(
                    fixtures[0].id,
                    chromium::DocumentRole::Test
                )]
            );
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

#[test]
fn unit_phase_partial_coverage_never_reports_full_pass() {
    use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};
    let request = ObservationRequest {
        styles: &[
            StyleField {
                target: "parent",
                property: Request::Compare(StyleProperty::Color),
                name: "color",
            },
            StyleField {
                target: "parent",
                property: Request::Unsupported {
                    reason: "outside selected AG5 surface",
                },
                name: "line-height",
            },
        ],
        ..ObservationRequest::NONE
    };
    let fixture = Fixture {
        request,
        document: crate::fixtures::fixture("style/cascade-inheritance")
            .document
            .with_test_bytes(b"<!doctype html><div id=parent style='color:#123456'></div>"),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: crate::model::Expected::Projection(
                    r#"{"dom":{"state":"not_requested"},"styles":[{"target":"parent","property":"color","value":{"state":"observed","value":{"state":"present","value":{"kind":"rgba8","value":[18,52,86,255]}}}},{"target":"parent","property":"line-height","value":{"state":"unsupported","value":"outside selected AG5 surface"}}],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
                ),
                disposition: Expectation::Match,
            }),
        },
        ..*crate::fixtures::fixture("style/cascade-inheritance")
    };
    let html = (fixture).document.html;
    let actual = execute::execute_fixture(html, request).unwrap();
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    reference.inspection=Some(serde_json::from_value(json!({"dom":null,"styles":[{"target":"parent","values":["rgb(18, 52, 86)","rgba(0, 0, 0, 0)","16px","block"]}],"layout":[],"inner_width":640,"inner_height":480,"layout_width":640})).unwrap());
    let report = report::Report::new(vec![
        report::Case::observed(&fixture, actual, reference).unwrap(),
    ]);
    assert_eq!(report.exit_code(), 3);
    let wire = serde_json::to_value(report).unwrap();
    assert_eq!(wire["cases"][0]["status"], "incomplete");
    assert_eq!(
        wire["cases"][0]["borrowser_expectation"]["status"],
        "INCOMPLETE"
    );
    assert_eq!(wire["summary"]["equal_fields"], 1);
    assert_eq!(wire["summary"]["unsupported_fields"], 1);
}

#[test]
fn unit_phase_layout_difference_is_localized_and_missing_capture_is_error() {
    let fixture = crate::fixtures::fixture("layout/block-boxes");
    let html = (fixture).document.html;
    let actual = execute::execute_fixture(html, fixture.request).unwrap();
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    reference.inspection = Some(
        serde_json::from_value(json!({"dom":null,"styles":[],"layout":[
        {"target":"container","boxes":[{"left":0,"top":0,"right":170.125,"bottom":74}]},
        {"target":"child","boxes":[{"left":5,"top":5,"right":37,"bottom":21}]},
        {"target":"sibling","boxes":[{"left":0,"top":74,"right":80,"bottom":86}]},
        {"target":"hidden","boxes":[]},
        {"target":"zero","boxes":[{"left":0,"top":86,"right":0,"bottom":86}]},
        {"target":"missing","boxes":null}],"inner_width":640,"inner_height":480,"layout_width":640}))
        .unwrap(),
    );
    let report = report::Report::new(vec![
        report::Case::observed(fixture, actual.clone(), reference).unwrap(),
    ]);
    assert_eq!(report.exit_code(), 1);
    let wire = serde_json::to_value(report).unwrap();
    assert_eq!(wire["summary"]["different_fields"], 1);
    assert_eq!(
        wire["cases"][0]["fields"][2]["id"],
        "layout:#container/border-box[0]/right"
    );
    assert_eq!(wire["cases"][0]["fields"][2]["status"], "different");
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    assert!(report::Case::observed(fixture, actual.clone(), reference).is_err());
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    reference.inspection=Some(serde_json::from_value(json!({"dom":null,"styles":[],"layout":(["container","child","sibling","hidden","zero","missing"].map(|target|json!({"target":target,"boxes":null}))),"inner_width":640,"inner_height":480,"layout_width":640})).unwrap());
    let case = report::Case::observed(fixture, actual, reference).unwrap();
    let wire = serde_json::to_value(case).unwrap();
    assert_eq!(wire["status"], "different");
    assert_eq!(wire["fields"][0]["id"], "layout:#container/boxes");
}

#[test]
fn unit_unrequested_and_unsupported_families_are_not_canvas_equality() {
    let fixture = Fixture {
        request: crate::projection::ObservationRequest::NONE,
        document: crate::fixtures::fixture("dom/structure")
            .document
            .with_test_bytes(b"<!doctype html><div></div>"),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: crate::model::Expected::Projection(
                    r#"{"dom":{"state":"not_requested"},"styles":[],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
                ),
                disposition: Expectation::Match,
            }),
        },
        ..*crate::fixtures::fixture("dom/structure")
    };
    let html = (fixture).document.html;
    let actual = execute::execute_fixture(html, fixture.request).unwrap();
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    let report = report::Report::new(vec![
        report::Case::observed(&fixture, actual, reference).unwrap(),
    ]);
    assert_eq!(report.exit_code(), 3);
    let no_fields =
        serde_json::to_value(report::Case::unsupported(&fixture, "explicit exclusion")).unwrap();
    assert_eq!(no_fields["coverage"]["reason"], "explicit exclusion");
    let unsupported = report::Case::unsupported(
        crate::fixtures::fixture("dom/structure"),
        "explicitly excluded DOM scenario",
    );
    let wire = serde_json::to_value(unsupported).unwrap();
    assert_eq!(wire["fields"][0]["id"], "dom");
    assert_eq!(wire["fields"][0]["status"], "unsupported");
}

fn style_reference() -> chromium::ChromiumCapture {
    let mut reference = capture([0, 0, 0]);
    reference.color = None;
    reference.inspection = Some(
        serde_json::from_value(json!({"dom":null,"styles":[
        {"target":"parent","values":["rgb(18, 52, 86)","rgb(171, 205, 239)","20px","block"]},
        {"target":"child","values":["rgb(52, 86, 120)","rgba(0, 0, 0, 0)","20px","inline"]},
        {"target":"hidden","values":["rgb(18, 52, 86)","rgba(0, 0, 0, 0)","20px","none"]}
    ],"layout":[],"inner_width":640,"inner_height":480,"layout_width":640}))
        .unwrap(),
    );
    reference
}

#[test]
fn unit_phase_required_records_cannot_be_truncated_duplicated_or_misidentified() {
    use crate::projection::{Observation, TargetValue};
    for index in [4, 5] {
        let mut fixture = crate::fixtures::baseline()[index];
        let html = (fixture).document.html;
        let expected = (fixture).independent().unwrap().expected;
        {
            fixture.document = fixture.document.with_test_bytes(html);
            fixture.kind = FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected,
                    disposition: Expectation::KnownFailure {
                        reason: "must not hide malformed data",
                    },
                }),
            };
        };
        let actual = execute::execute_fixture(html, fixture.request).unwrap();
        let mut reference = style_reference();
        if index == 5 {
            reference.inspection = Some(
                serde_json::from_value(json!({"dom":null,"styles":[],"layout":[
                {"target":"container","boxes":[{"left":0,"top":0,"right":170,"bottom":74}]},
                {"target":"child","boxes":[{"left":5,"top":5,"right":37,"bottom":21}]},
                {"target":"sibling","boxes":[{"left":0,"top":74,"right":80,"bottom":86}]},
                {"target":"hidden","boxes":[]},
                {"target":"zero","boxes":[{"left":0,"top":86,"right":0,"bottom":86}]},
                {"target":"missing","boxes":null}],"inner_width":640,"inner_height":480,"layout_width":640}))
                .unwrap(),
            );
        }
        let inspection = reference.inspection;
        let reference = || {
            let mut capture = style_reference();
            capture.inspection = inspection.clone();
            capture
        };
        assert!(report::Case::observed(&fixture, actual.clone(), reference()).is_ok());
        for mutation in [
            "missing",
            "extra",
            "duplicate",
            "target",
            "property",
            "order",
            "unrequested",
            "unsupported",
            "extra_family",
        ] {
            let mut malformed = actual.clone();
            if mutation == "extra_family" {
                malformed.observations.canvas = Observation::Observed(CanvasColor([0, 0, 0]));
            } else if index == 4 {
                let records = &mut malformed.observations.styles;
                match mutation {
                    "missing" => {
                        records.pop();
                    }
                    "extra" => records.push(records[0].clone()),
                    "duplicate" => records[1] = records[0].clone(),
                    "target" => records[0].target = "wrong".into(),
                    "property" => records[0].property = "wrong".into(),
                    "order" => records.swap(0, 1),
                    "unrequested" => records[0].value = Observation::NotRequested,
                    "unsupported" => {
                        records[0].value = Observation::Unsupported("unexpected".into())
                    }
                    _ => unreachable!(),
                }
            } else {
                let Observation::Observed(records) = &mut malformed.observations.layout else {
                    unreachable!()
                };
                match mutation {
                    "missing" => {
                        records.pop();
                    }
                    "extra" => records.push(records[0].clone()),
                    "duplicate" => records[1] = records[0].clone(),
                    "target" | "property" => records[0].target = "wrong".into(),
                    "order" => records.swap(0, 1),
                    "unrequested" => malformed.observations.layout = Observation::NotRequested,
                    "unsupported" => {
                        malformed.observations.layout =
                            Observation::Unsupported("unexpected".into())
                    }
                    _ => unreachable!(),
                }
            }
            let result = report::Case::observed(&fixture, malformed.clone(), reference());
            assert!(
                matches!(result, Err(ref e) if e.code == "observation.required_data"),
                "fixture {index}, {mutation} must not become XFAIL plus cross-engine success"
            );
            let local = crate::report::evaluate(&fixture, Execution::Observed(malformed));
            assert_eq!(local.status(), "ERROR", "fixture {index}, {mutation}");
            assert_eq!(crate::report::exit_code(&[local]), 2);
        }
        // Record completeness does not constrain observed content cardinality.
        let mut different = actual;
        if index == 4 {
            different.observations.styles[0].value =
                Observation::Observed(TargetValue::MissingNode);
        } else {
            let Observation::Observed(records) = &mut different.observations.layout else {
                unreachable!()
            };
            let TargetValue::Present(boxes) = &mut records[0].value else {
                unreachable!()
            };
            boxes.push(boxes[0].clone());
        }
        let case = report::Case::observed(&fixture, different, reference()).unwrap();
        assert_eq!(serde_json::to_value(case).unwrap()["status"], "different");
    }
}

pub(crate) fn expected_profile_fields(id: &str) -> Vec<String> {
    match id {
        "canvas/root" | "canvas/cascade" => vec!["canvas_color_rgb8".into()],
        "dom/structure" | "dom/template-namespaces" => vec!["dom".into()],
        "style/cascade-inheritance" => ["parent", "child", "hidden"]
            .into_iter()
            .flat_map(|id| {
                ["color", "background-color", "font-size", "display"]
                    .map(|p| format!("style:#{id}/{p}"))
            })
            .collect(),
        "layout/block-boxes" => ["container", "child", "sibling", "hidden", "zero", "missing"]
            .into_iter()
            .flat_map(|id| {
                if matches!(id, "hidden" | "missing") {
                    vec![format!("layout:#{id}/boxes")]
                } else {
                    ["left", "top", "right", "bottom"]
                        .map(|edge| format!("layout:#{id}/border-box[0]/{edge}"))
                        .to_vec()
                }
            })
            .collect(),
        _ => panic!("unexpected profile fixture {id}"),
    }
}

#[test]
fn unit_phase_single_style_property_difference_retains_all_twelve_fields() {
    let fixture = crate::fixtures::fixture("style/cascade-inheritance");
    let html = (fixture).document.html;
    let actual = execute::execute_fixture(html, fixture.request).unwrap();
    for differs in [false, true] {
        let mut reference = style_reference();
        if differs {
            reference.inspection.as_mut().unwrap().styles[0]
                .values
                .as_mut()
                .unwrap()[0] = "rgb(18, 52, 87)".into();
        }
        let report = report::Report::new(vec![
            report::Case::observed(fixture, actual.clone(), reference).unwrap(),
        ]);
        assert_eq!(report.exit_code(), u8::from(differs));
        let wire = serde_json::to_value(report).unwrap();
        assert_eq!(
            wire["summary"]["equal_fields"],
            if differs { 11 } else { 12 }
        );
        assert_eq!(wire["summary"]["different_fields"], usize::from(differs));
        assert_eq!(wire["summary"]["unsupported_fields"], 0);
        let fields = wire["cases"][0]["fields"].as_array().unwrap();
        assert_eq!(
            fields
                .iter()
                .map(|f| f["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_profile_fields(fixture.id.0)
        );
        assert_eq!(
            fields[0]["status"],
            if differs { "different" } else { "equal" }
        );
        assert_eq!(wire["cases"][0]["borrowser_expectation"]["status"], "PASS");
        if differs {
            assert_eq!(
                fields[0]["differences"][0]["path"],
                "style:#parent/color/value/value/value[2]"
            );
        }
    }
}

#[test]
fn unit_phase_dom_common_comparison_preserves_meaningful_structure() {
    // Native facts authored independently for the existing dom/structure input.
    fn node(kind: u8, name: &str, value: &str, children: Vec<Value>) -> Value {
        json!({"node_type":kind,"name":name,"namespace":if kind == 1 {"http://www.w3.org/1999/xhtml"} else {""},"value":value,"public_id":"","system_id":"","attributes":[],"children":children,"template":null})
    }
    let mut div = node(
        1,
        "div",
        "",
        vec![
            node(3, "", "A & B", vec![]),
            node(1, "span", "", vec![node(3, "", " x ", vec![])]),
            node(8, "", "note", vec![]),
        ],
    );
    div["attributes"] = json!([
        {"namespace":"","prefix":"","name":"z","value":"last"},
        {"namespace":"","prefix":"","name":"a","value":"first"},
        {"namespace":"","prefix":"","name":"id","value":"target"}
    ]);
    let dom = node(
        9,
        "",
        "",
        vec![
            node(10, "html", "", vec![]),
            node(
                1,
                "html",
                "",
                vec![node(1, "head", "", vec![]), node(1, "body", "", vec![div])],
            ),
        ],
    );
    let fixture = crate::fixtures::fixture("dom/structure");
    let html = (fixture).document.html;
    let actual = execute::execute_fixture(html, fixture.request).unwrap();
    for mutation in [
        "none",
        "attribute_order",
        "attribute",
        "text",
        "namespace",
        "name",
        "child_order",
        "child_count",
        "template",
    ] {
        let mut dom = dom.clone();
        let div = &mut dom["children"][1]["children"][1]["children"][0];
        match mutation {
            "none" => {}
            "attribute_order" => div["attributes"].as_array_mut().unwrap().reverse(),
            "attribute" => div["attributes"][0]["value"] = json!("changed"),
            "text" => div["children"][0]["value"] = json!("A & B "),
            "namespace" => div["namespace"] = json!("http://www.w3.org/2000/svg"),
            "name" => div["name"] = json!("section"),
            "child_order" => div["children"].as_array_mut().unwrap().swap(0, 2),
            "child_count" => {
                div["children"].as_array_mut().unwrap().pop();
            }
            "template" => {
                div["name"] = json!("template");
                div["template"] = node(11, "", "", div["children"].as_array().unwrap().clone());
                div["children"] = json!([]);
            }
            _ => unreachable!(),
        }
        let mut reference = capture([0, 0, 0]);
        reference.color = None;
        reference.inspection = Some(
            serde_json::from_value(json!({"dom":dom,"styles":[],"layout":[],"inner_width":640,"inner_height":480,"layout_width":640}))
                .unwrap(),
        );
        let report = report::Report::new(vec![
            report::Case::observed(fixture, actual.clone(), reference).unwrap(),
        ]);
        let differs = !matches!(mutation, "none" | "attribute_order");
        assert_eq!(report.exit_code(), u8::from(differs), "{mutation}");
        let wire = serde_json::to_value(report).unwrap();
        assert_eq!(
            wire["summary"]["different_fields"],
            usize::from(differs),
            "{mutation}"
        );
        assert_eq!(wire["summary"]["equal_fields"], usize::from(!differs));
        assert_eq!(wire["summary"]["unsupported_fields"], 0);
        assert_eq!(wire["cases"][0]["fields"].as_array().unwrap().len(), 1);
        assert_eq!(wire["cases"][0]["fields"][0]["id"], "dom");
        assert_eq!(wire["cases"][0]["borrowser_expectation"]["status"], "PASS");
        if differs {
            assert!(
                !wire["cases"][0]["fields"][0]["differences"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
