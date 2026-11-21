use super::execute::{self, Message};
use super::*;
use crate::{fixtures, model::*, report, run_fixture};
use std::{
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

fn fixture(stem: &str) -> Fixture {
    *fixtures::fixture(&format!("networking/fetch/{stem}"))
}
fn declared(stem: &str) -> NetworkingInput {
    match fixture(stem).input {
        FixtureInput::Networking(i) => i,
        _ => panic!("networking input"),
    }
}
fn modified(stem: &str, edit: impl FnOnce(&mut serde_json::Value)) -> Fixture {
    let original = fixture(stem);
    let input = declared(stem);
    let mut json: serde_json::Value = serde_json::from_str(input.declaration).unwrap();
    edit(&mut json);
    Fixture {
        input: FixtureInput::Networking(NetworkingInput {
            declaration: Box::leak(serde_json::to_string(&json).unwrap().into_boxed_str()),
        }),
        ..original
    }
}
fn request() -> net::FetchRequest {
    net::FetchRequest::try_new(
        1,
        ResourceAddress::new(
            BrowserUrl::parse("https://fixture.invalid/data")
                .unwrap()
                .url,
            addressing::ResourceKind::Document,
        ),
        net::RequestMethod::Get,
        net::HttpHeaders::new(),
        None,
        None,
    )
    .unwrap()
}
fn state() -> execute::Collector {
    let r = request();
    execute::Collector::new(
        RequestObservation::project(&r, Route::Html).unwrap(),
        1,
        r.legacy_issued_url(core_types::ResourceKind::Html).unwrap(),
        bounds::ROW_STORAGE,
    )
}
fn cancellation() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[test]
fn ai9_required_matrix_executes_independent_expectations() {
    let selected = fixtures::select_suite(Some(fixtures::Suite::Networking), None).unwrap();
    assert_eq!(selected.len(), 18);
    let mut rows = 0;
    let mut passed = 0;
    let mut unsupported = 0;
    for f in selected {
        if let FixtureKind::Runnable { .. } = f.kind {
            assert!(matches!(
                f.independent().unwrap().disposition,
                Expectation::Match
            ));
            let FixtureInput::Networking(input) = f.input else {
                panic!("wrong family")
            };
            let NetworkingCase::Fetch { rows: r } = input.case().unwrap() else {
                panic!("deferred required case")
            };
            rows += r.len();
            let evaluated = report::evaluate(f, run_fixture(f));
            assert_eq!(evaluated.status(), "PASS", "{}", f.id.0);
            passed += 1;
        } else {
            assert_eq!(report::evaluate(f, run_fixture(f)).status(), "UNSUPPORTED");
            unsupported += 1;
        }
    }
    assert_eq!((passed, rows, unsupported), (16, 36, 2));
}
#[test]
fn ai9_construction_and_dispatch_rejections_have_no_worker_evidence() {
    let input = declared("unsupported-scheme");
    let Expected::Networking(json) = fixture("unsupported-scheme")
        .independent()
        .unwrap()
        .expected
    else {
        panic!()
    };
    let observed = super::execute(input, json).unwrap();
    assert!(matches!(
        observed.rows[0].terminal,
        Terminal::ConstructionRejected {
            error: Admission::UnsupportedScheme
        }
    ));
    assert_eq!(
        observed.rows[0].request,
        Available::Unavailable("request_not_constructed".into())
    );
    assert!(matches!(
        observed.rows[1].terminal,
        Terminal::DispatchRejected {
            error: Admission::UnsupportedScheme
        }
    ));
    for r in &observed.rows[..2] {
        assert!(r.acquisitions.is_empty());
        assert_eq!(r.start, Available::Inapplicable);
    }
    assert!(matches!(
        observed.rows[2].terminal,
        Terminal::Error {
            error: NetworkError::Redirect(RedirectReason::UnsupportedScheme),
            ..
        }
    ));
    assert_eq!(observed.rows[2].acquisitions.len(), 1);
}
#[test]
fn ai9_precancelled_dispatch_closes_with_one_cancelled_terminal() {
    let NetworkingCase::Fetch { rows } = declared("response-200").case().unwrap() else {
        panic!()
    };
    let n = network(&rows[0], true).unwrap();
    let r = execute::run(
        n,
        request(),
        Route::Html,
        Arc::new(AtomicBool::new(true)),
        0,
    )
    .unwrap();
    assert!(r.acquisitions.is_empty());
    assert_eq!(r.start, Available::Inapplicable);
    assert!(matches!(
        r.terminal,
        Terminal::Error {
            error: NetworkError::Cancelled,
            ..
        }
    ));
}
#[test]
fn ai9_full_queue_failure_releases_inflight_sender_without_retry_or_panic() {
    let token = cancellation();
    let (ch, endpoint, observer) = execute::channels(token.clone());
    drop(observer);
    let latch = ch.delivery.clone();
    for _ in 0..bounds::QUEUE {
        endpoint
            .data
            .try_send(Message::Entered(
                RequestObservation::project(&request(), Route::Html).unwrap(),
            ))
            .unwrap();
    }
    assert!(matches!(
        endpoint.data.try_send(Message::Entered(
            RequestObservation::project(&request(), Route::Html).unwrap()
        )),
        Err(mpsc::TrySendError::Full(_))
    ));
    let (started_tx, started_rx) = mpsc::sync_channel(0);
    let (finished_tx, finished_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        endpoint.send(Ok(Message::Entered(
            RequestObservation::project(&request(), Route::Html).unwrap(),
        )));
        drop(endpoint);
        finished_tx.send(()).unwrap();
    });
    started_rx.recv().unwrap();
    let failure = execute::collect(ch, state(), Instant::now()).unwrap_err();
    assert_eq!(failure.primary.code, "networking.deadline");
    assert_eq!(failure.closure, ProducerClosure::Closed);
    finished_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert!(token.load(Ordering::Acquire));
    assert_eq!(latch.load(Ordering::Acquire), 2);
}
#[test]
fn ai9_receiver_disconnect_is_latched_once_and_requests_cancellation() {
    let token = cancellation();
    let (ch, endpoint, observer) = execute::channels(token.clone());
    let latch = ch.delivery.clone();
    drop(ch.data);
    drop(observer);
    endpoint.send(Ok(Message::Entered(
        RequestObservation::project(&request(), Route::Html).unwrap(),
    )));
    assert_eq!(latch.load(Ordering::Acquire), 2);
    assert!(token.load(Ordering::Acquire));
    drop(endpoint);
    assert_eq!(
        execute::closure_until(&ch.alive, Instant::now()),
        ProducerClosure::Closed
    );
}
#[test]
fn ai9_ready_records_cannot_bypass_absolute_deadline() {
    let (ch, endpoint, observer) = execute::channels(cancellation());
    for _ in 0..8 {
        endpoint
            .data
            .try_send(Message::Entered(
                RequestObservation::project(&request(), Route::Html).unwrap(),
            ))
            .unwrap();
    }
    drop(endpoint);
    drop(observer);
    let failure = execute::collect(ch, state(), Instant::now()).unwrap_err();
    assert_eq!(failure.primary.code, "networking.deadline");
    assert_eq!(failure.closure, ProducerClosure::Closed);
}
#[test]
fn ai9_missing_return_or_terminal_reports_error_with_closed_producers() {
    for missing_return in [true, false] {
        let (ch, endpoint, observer) = execute::channels(cancellation());
        if missing_return {
            endpoint.send(Ok(Message::Entered(
                RequestObservation::project(&request(), Route::Html).unwrap(),
            )));
        }
        drop(endpoint);
        drop(observer);
        let failure =
            execute::collect(ch, state(), Instant::now() + Duration::from_secs(5)).unwrap_err();
        assert_eq!(failure.closure, ProducerClosure::Closed);
        assert!(failure.primary.detail.contains(if missing_return {
            "acquisition return"
        } else {
            "terminal"
        }));
    }
}
#[test]
fn ai9_retained_producer_has_honest_unresolved_shutdown() {
    let token = cancellation();
    let (ch, endpoint, observer) = execute::channels(token.clone());
    drop(observer);
    assert_eq!(
        execute::closure_until(&ch.alive, Instant::now()),
        ProducerClosure::Unresolved
    );
    // This test owns the retained endpoint: no detached thread or worker is
    // left behind after exercising the collector's bounded failure path.
    let failure = execute::collect(ch, state(), Instant::now()).unwrap_err();
    assert_eq!(failure.primary.code, "networking.deadline");
    assert_eq!(failure.closure, ProducerClosure::Unresolved);
    assert!(token.load(Ordering::Acquire));
    drop(endpoint);
}
#[test]
fn ai9_callback_record_excess_is_error_not_silent_missing_evidence() {
    let (ch, endpoint, observer) = execute::channels(cancellation());
    endpoint.send(Ok(Message::Chunk {
        request_id: 1,
        url: "x".into(),
        bytes: vec![0; bounds::RECORD + 1],
    }));
    drop(endpoint);
    drop(observer);
    let failure =
        execute::collect(ch, state(), Instant::now() + Duration::from_secs(5)).unwrap_err();
    assert!(failure.primary.detail.contains("delivery failed"));
    assert_eq!(failure.closure, ProducerClosure::Closed);
}
#[test]
fn ai9_drain_data_before_accepting_lifetime_closure() {
    let (ch, endpoint, observer) = execute::channels(cancellation());
    endpoint.send(Ok(Message::Returned(AcquisitionResult::MissingRoute)));
    drop(endpoint);
    drop(observer);
    assert_eq!(
        execute::closure_until(&ch.alive, Instant::now()),
        ProducerClosure::Closed
    );
    let failure =
        execute::collect(ch, state(), Instant::now() + Duration::from_secs(5)).unwrap_err();
    assert!(failure.primary.detail.contains("return without entry"));
}
#[test]
fn ai9_input_and_expected_sequence_and_byte_budgets_are_strict() {
    let f = modified("response-200", |v| {
        let r = v["networking"]["rows"][0].clone();
        v["networking"]["rows"] = serde_json::json!(vec![r; 33]);
    });
    let FixtureInput::Networking(i) = f.input else {
        panic!()
    };
    assert!(i.case().is_err());
    let input = NetworkingInput {
        declaration: Box::leak(" ".repeat(bounds::DECLARATION + 1).into_boxed_str()),
    };
    assert!(input.case().is_err());
    let case = declared("response-200").case().unwrap();
    assert!(expected(&" ".repeat(bounds::EXPECTATION + 1), &case).is_err());
    let Expected::Networking(json) = fixture("response-200").independent().unwrap().expected else {
        panic!()
    };
    let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
    value["rows"][0]["unexpected"] = true.into();
    assert!(expected(&value.to_string(), &case).is_err());
}
#[test]
fn ai9_interior_body_change_changes_incremental_digest_without_full_buffering() {
    let mut a = BodyAccumulator::new();
    let mut b = BodyAccumulator::new();
    for i in 0..128 {
        a.update(&[7; 32]).unwrap();
        let mut bytes = [7; 32];
        if i == 64 {
            bytes[16] = 8;
        }
        b.update(&bytes).unwrap();
    }
    let (a, b) = (a.finish(), b.finish());
    let (
        Body::Summary {
            length: la,
            prefix: pa,
            suffix: sa,
            sha256: ha,
        },
        Body::Summary {
            length: lb,
            prefix: pb,
            suffix: sb,
            sha256: hb,
        },
    ) = (a, b)
    else {
        panic!()
    };
    assert_eq!((la, pa, sa), (lb, pb, sb));
    assert_ne!(ha, hb);
}
#[test]
fn ai9_changed_input_fails_unchanged_expectation_and_known_failure_never_hides_error() {
    for (disposition, want, exit) in [
        (Expectation::Match, "FAIL", 1),
        (
            Expectation::KnownFailure {
                reason: "controlled test-only mismatch",
            },
            "XFAIL",
            0,
        ),
    ] {
        let mut changed = modified("response-200", |v| {
            v["networking"]["rows"][0]["routes"][0]["outcome"]["status"] = 201.into()
        });
        let independent = changed.independent().unwrap();
        changed.kind = FixtureKind::Runnable {
            checks: Checks::Independent(IndependentExpectation {
                disposition,
                ..independent
            }),
        };
        let evaluated = report::evaluate(&changed, run_fixture(&changed));
        assert_eq!(evaluated.status(), want);
        assert_eq!(report::exit_code(&[evaluated]), exit);
    }
    let mut stale = fixture("response-200");
    let independent = stale.independent().unwrap();
    stale.kind = FixtureKind::Runnable {
        checks: Checks::Independent(IndependentExpectation {
            disposition: Expectation::KnownFailure {
                reason: "test-only stale mismatch",
            },
            ..independent
        }),
    };
    assert_eq!(
        report::evaluate(&stale, run_fixture(&stale)).status(),
        "XPASS"
    );
}

#[test]
fn ai9_evidence_malformed_expected_and_actual_are_errors_before_classification() {
    let original = fixture("response-200");
    let independent = original.independent().unwrap();
    let Expected::Networking(json) = independent.expected else {
        panic!()
    };
    let case = declared("response-200").case().unwrap();
    let Execution::NetworkingObserved(actual) = run_fixture(&original) else {
        panic!()
    };
    for mutation in [
        "start",
        "unknown_start",
        "request",
        "unknown_request",
        "acquisitions",
        "acquired_response",
        "terminal_response",
        "delivered_bytes",
        "final_url",
        "error_progress",
        "error_response",
    ] {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        let row = &mut value["rows"][0];
        match mutation {
            "start" => row["start"] = serde_json::json!({"state":"inapplicable"}),
            "unknown_start" => {
                row["start"] = serde_json::json!({"state":"unavailable","value":"not observed"})
            }
            "request" => row["request"] = serde_json::json!({"state":"inapplicable"}),
            "unknown_request" => {
                row["request"] = serde_json::json!({"state":"unavailable","value":"not observed"})
            }
            "acquisitions" => row["acquisitions"] = serde_json::json!([]),
            "acquired_response" => {
                row["acquisitions"][0]["result"] = serde_json::json!({"kind":"missing_route"})
            }
            "terminal_response" => row["terminal"]["response"]["status"] = 201.into(),
            "delivered_bytes" => row["terminal"]["progress"]["delivered_bytes"] = 0.into(),
            "final_url" => row["selected_final_url"] = serde_json::json!({"state":"inapplicable"}),
            "error_progress" | "error_response" => {
                let response = row["terminal"]["response"].clone();
                let mut progress = row["terminal"]["progress"].clone();
                progress["delivered_bytes"] = 0.into();
                row["body"] = serde_json::json!({"kind":"bytes","bytes":[]});
                row["terminal"] = serde_json::json!({"kind":"error","error":{"kind":"read"},
                    "response":{"state":"present","value":response},
                    "progress":{"state":"present","value":progress}});
                row["terminal"][if mutation == "error_progress" {
                    "progress"
                } else {
                    "response"
                }] = serde_json::json!({"state":"unavailable","value":"not observed"});
            }
            _ => unreachable!(),
        }
        let malformed_json = Box::leak(value.to_string().into_boxed_str());
        // All mutations deserialize: required evidence validation, rather than
        // JSON syntax rejection or an injected Execution::Error, must detect them.
        let malformed_actual: NetworkingObservation = serde_json::from_str(malformed_json).unwrap();
        assert!(expected(malformed_json, &case).is_err(), "{mutation}");
        for disposition in [
            Expectation::Match,
            Expectation::KnownFailure {
                reason: "controlled evidence review",
            },
        ] {
            let mut valid_fixture = original;
            valid_fixture.id = TestId("networking/fetch/evidence-review");
            valid_fixture.kind = FixtureKind::Runnable {
                checks: Checks::Independent(IndependentExpectation {
                    disposition,
                    ..independent
                }),
            };
            let mut malformed_fixture = valid_fixture;
            malformed_fixture.kind = FixtureKind::Runnable {
                checks: Checks::Independent(IndependentExpectation {
                    expected: Expected::Networking(malformed_json),
                    disposition,
                }),
            };
            // Execution intake and reporting each enforce the expectation contract.
            for execution in [
                run_fixture(&malformed_fixture),
                Execution::NetworkingObserved(actual.clone()),
            ] {
                let report = report::evaluate(&malformed_fixture, execution);
                assert_eq!(report.status(), "ERROR", "expected {mutation}");
                assert_eq!(report::exit_code(&[report]), 2);
            }
            let report = report::evaluate(
                &valid_fixture,
                Execution::NetworkingObserved(malformed_actual.clone()),
            );
            assert_eq!(report.status(), "ERROR", "actual {mutation}");
            assert_eq!(report.independent_status(), Some("ERROR"));
            assert_eq!(report::exit_code(&[report]), 2);
        }
    }
}

#[test]
fn ai9_evidence_closed_zero_byte_post_start_error_requires_progress() {
    let f = fixture("response-200");
    let Expected::Networking(json) = f.independent().unwrap().expected else {
        panic!()
    };
    let authored = expected(json, &declared("response-200").case().unwrap()).unwrap();
    let row = &authored.rows[0];
    let Available::Present(start) = &row.start else {
        panic!()
    };
    let progress = Progress {
        route: Route::Html,
        delivered_bytes: 0,
        limit_bytes: 10 * 1024 * 1024,
    };
    for present in [false, true] {
        let (ch, endpoint, observer) = execute::channels(cancellation());
        endpoint.send(Ok(Message::Entered(row.acquisitions[0].request.clone())));
        endpoint.send(Ok(Message::Returned(row.acquisitions[0].result.clone())));
        endpoint.send(Ok(Message::Start {
            request_id: 1,
            response: start.clone(),
        }));
        endpoint.send(Ok(Message::Error {
            request_id: 1,
            url: request()
                .legacy_issued_url(core_types::ResourceKind::Html)
                .unwrap(),
            error: NetworkError::Read,
            response: Available::Present(start.clone()),
            progress: if present {
                Available::Present(progress.clone())
            } else {
                Available::Unavailable("not observed".into())
            },
        }));
        drop(endpoint);
        drop(observer);
        let result = execute::collect(ch, state(), Instant::now() + Duration::from_secs(5));
        if present {
            let observed = result.unwrap();
            observed.validate().unwrap();
            assert_eq!(observed.body, Body::Bytes { bytes: vec![] });
            assert!(
                matches!(&observed.terminal, Terminal::Error { progress: Available::Present(p), .. } if p.delivered_bytes == 0)
            );
            // Complete but different behavior is an ordinary mismatch, not ERROR.
            let evaluated = report::evaluate(
                &f,
                Execution::NetworkingObserved(NetworkingObservation {
                    rows: vec![observed],
                }),
            );
            assert_eq!(evaluated.status(), "FAIL");
            assert_eq!(report::exit_code(&[evaluated]), 1);
        } else {
            let failure = result.unwrap_err();
            assert!(failure.primary.detail.contains("progress"));
            assert_eq!(failure.closure, ProducerClosure::Closed);
            let evaluated = report::evaluate(&f, Execution::NetworkingError(failure));
            assert_eq!(evaluated.status(), "ERROR");
            assert_eq!(report::exit_code(&[evaluated]), 2);
        }
    }
}

#[test]
fn ai9_evidence_pre_start_failures_preserve_conditional_absence() {
    // Acquired metadata can be rejected before Start without body progress.
    let input = modified("response-200", |v| {
        v["networking"]["rows"][0]["routes"][0]["outcome"]["reported_response_url"] =
            serde_json::Value::Null
    });
    let Execution::NetworkingObserved(observed) = run_fixture(&input) else {
        panic!()
    };
    observed.validate(1).unwrap();
    assert_eq!(observed.rows[0].start, Available::Inapplicable);
    assert!(matches!(
        &observed.rows[0].terminal,
        Terminal::Error {
            response: Available::Present(_),
            progress: Available::Unavailable(_),
            ..
        }
    ));
    for name in [
        "network-error",
        "redirect-target-failure",
        "unsupported-scheme",
    ] {
        let f = fixture(name);
        let evaluated = report::evaluate(&f, run_fixture(&f));
        assert_eq!(evaluated.status(), "PASS", "{name}");
    }
}

#[test]
fn ai9_evidence_local_known_failure_reports_explain_xfail_and_xpass() {
    for (status, want, exit) in [(201, "XFAIL", 0), (200, "XPASS", 1)] {
        let mut f = modified("response-200", |v| {
            v["networking"]["rows"][0]["routes"][0]["outcome"]["status"] = status.into()
        });
        let independent = f.independent().unwrap();
        f.id = TestId("networking/fetch/evidence-review");
        f.kind = FixtureKind::Runnable {
            checks: Checks::Independent(IndependentExpectation {
                disposition: Expectation::KnownFailure {
                    reason: "controlled HTTP status mismatch",
                },
                ..independent
            }),
        };
        let evaluated = report::evaluate(&f, run_fixture(&f));
        assert_eq!(evaluated.status(), want);
        let mut out = Vec::new();
        report::write_report(&[evaluated], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("known failure: controlled HTTP status mismatch"),
            "{text}"
        );
        assert_eq!(
            report::exit_code(&[report::evaluate(&f, run_fixture(&f))]),
            exit
        );
    }
}
#[test]
fn ai9_capped_serialization_counts_encoded_bytes_and_never_accepts_prefix() {
    use std::io::Write;
    let value = vec![255u8; 64];
    let exact = serde_json::to_vec(&value).unwrap().len();
    assert_eq!(bounds::json(&value, exact).unwrap().len(), exact);
    assert!(bounds::json(&value, exact - 1).is_err());
    for cap in [bounds::ROW_JSON, bounds::DIAGNOSTIC, bounds::REPORT] {
        let mut out = bounds::Buffer::new(cap);
        let chunk = vec![0; 1024];
        for _ in 0..cap / 1024 {
            out.write_all(&chunk).unwrap();
        }
        assert!(out.write_all(&[0]).is_err());
        assert_eq!(out.finish().len(), cap);
    }
}

#[test]
fn ai9_unresolved_shutdown_stops_selection_and_preserves_primary_error() {
    let a = fixture("response-200");
    let b = fixture("response-404");
    let mut entered = 0;
    let error = crate::evaluate_selected(&[&a, &b], |_| {
        entered += 1;
        Execution::NetworkingError(Failure {
            primary: HarnessError::new("networking.deadline", "primary deadline sentinel"),
            closure: ProducerClosure::Unresolved,
        })
    })
    .err()
    .unwrap();
    assert_eq!(entered, 1);
    assert_eq!(error.code, "networking.shutdown");
    assert!(error.detail.contains("primary deadline sentinel"));
    assert!(error.detail.contains("worker_termination=unverified"));
}
#[test]
fn ai9_terminal_does_not_conceal_queued_late_records() {
    let Expected::Networking(json) = fixture("response-200").independent().unwrap().expected else {
        panic!()
    };
    let mut observed = expected(json, &declared("response-200").case().unwrap()).unwrap();
    let row = observed.rows.remove(0);
    let Available::Present(r) = row.request else {
        panic!()
    };
    let Available::Present(start) = row.start else {
        panic!()
    };
    let Terminal::Done { response, progress } = row.terminal else {
        panic!()
    };
    let (ch, endpoint, observer) = execute::channels(cancellation());
    endpoint.send(Ok(Message::Entered(r.clone())));
    endpoint.send(Ok(Message::Returned(row.acquisitions[0].result.clone())));
    endpoint.send(Ok(Message::Start {
        request_id: 1,
        response: start,
    }));
    endpoint.send(Ok(Message::Chunk {
        request_id: 1,
        url: r.original_url.clone(),
        bytes: vec![0, 255],
    }));
    endpoint.send(Ok(Message::Done {
        request_id: 1,
        response,
        progress,
    }));
    endpoint.send(Ok(Message::Entered(r)));
    drop(endpoint);
    drop(observer);
    assert_eq!(
        execute::closure_until(&ch.alive, Instant::now()),
        ProducerClosure::Closed
    );
    let error = execute::collect(ch, state(), Instant::now() + Duration::from_secs(5)).unwrap_err();
    assert!(error.primary.detail.contains("after terminal"));
}

#[test]
fn ai9_interior_byte_change_fails_independent_unchanged_summary_expectation() {
    // SHA-256 for exactly 4096 bytes of 0x07, independently computed from that
    // literal byte pattern. No captured result is used to author the expectation.
    let expected_hash = "c9ac7b0624824f844f6c7f3d50fab9741a8914e878467e8daaedca143a34d90b";
    let original = fixture("response-200");
    let Expected::Networking(json) = original.independent().unwrap().expected else {
        panic!()
    };
    let mut expected: serde_json::Value = serde_json::from_str(json).unwrap();
    expected["rows"][0]["body"] = serde_json::json!({"kind":"summary","length":4096,"sha256":expected_hash,"prefix":vec![7;32],"suffix":vec![7;32]});
    expected["rows"][0]["terminal"]["progress"]["delivered_bytes"] = 4096.into();
    let independent = IndependentExpectation {
        expected: Expected::Networking(Box::leak(expected.to_string().into_boxed_str())),
        disposition: Expectation::Match,
    };
    let mut input = modified("response-200", |v| {
        v["networking"]["rows"][0]["routes"][0]["outcome"]["body"] =
            serde_json::json!({"kind":"bytes","bytes":vec![7;4096]})
    });
    input.kind = FixtureKind::Runnable {
        checks: Checks::Independent(independent),
    };
    assert_eq!(
        report::evaluate(&input, run_fixture(&input)).status(),
        "PASS"
    );
    let FixtureInput::Networking(declaration) = input.input else {
        panic!()
    };
    let mut value: serde_json::Value = serde_json::from_str(declaration.declaration).unwrap();
    value["networking"]["rows"][0]["routes"][0]["outcome"]["body"]["bytes"][2048] = 8.into();
    input.input = FixtureInput::Networking(NetworkingInput {
        declaration: Box::leak(value.to_string().into_boxed_str()),
    });
    assert_eq!(
        report::evaluate(&input, run_fixture(&input)).status(),
        "FAIL"
    );
}

#[test]
fn ai9_retained_observation_and_diagnostic_excess_are_harness_errors() {
    let Expected::Networking(json) = fixture("response-200").independent().unwrap().expected else {
        panic!()
    };
    let mut value = expected(json, &declared("response-200").case().unwrap()).unwrap();
    let large_headers = (0..8)
        .map(|_| Header("x".into(), vec![128; 4096]))
        .collect::<Vec<_>>();
    let AcquisitionResult::Response(response) = &mut value.rows[0].acquisitions[0].result else {
        panic!()
    };
    response.headers = large_headers;
    let attempt = value.rows[0].acquisitions[0].clone();
    value.rows[0].acquisitions = vec![attempt; 64];
    assert!(
        value.rows[0]
            .validate()
            .unwrap_err()
            .detail
            .contains("storage budget")
    );
    let left = expected(json, &declared("response-200").case().unwrap()).unwrap();
    let mut right = left.clone();
    right.rows[0].selected_final_url = Available::Present("x".repeat(bounds::DIAGNOSTIC + 1));
    assert!(
        differences(&left, &right)
            .unwrap_err()
            .detail
            .contains("diagnostic budget")
    );
}

#[test]
fn ai9_expected_root_and_terminal_records_charge_owned_unavailable_reasons() {
    let Expected::Networking(json) = fixture("response-200").independent().unwrap().expected else {
        panic!()
    };
    let original = expected(json, &declared("response-200").case().unwrap()).unwrap();
    let mut value = original.clone();
    let Available::Present(request) = &mut value.rows[0].request else {
        panic!()
    };
    request.current_url = "x".repeat(bounds::RECORD);
    assert!(
        value.rows[0]
            .validate()
            .unwrap_err()
            .detail
            .contains("record budget")
    );

    let mut value = original;
    value.rows[0].terminal = Terminal::Error {
        error: NetworkError::Cancelled,
        response: Available::Unavailable("no_acquired_response".into()),
        progress: Available::Unavailable(String::new()),
    };
    let before = value.rows[0].storage();
    let reason = "x".repeat(bounds::RECORD);
    let capacity = reason.capacity();
    let Terminal::Error { progress, .. } = &mut value.rows[0].terminal else {
        panic!()
    };
    *progress = Available::Unavailable(reason);
    assert_eq!(value.rows[0].storage(), before + capacity);
    assert!(
        value.rows[0]
            .validate()
            .unwrap_err()
            .detail
            .contains("record budget")
    );
}

#[test]
fn ai9_single_changed_acquisition_in_long_chain_is_a_precise_failure_not_diagnostic_overflow() {
    let f = fixture("redirect-loops");
    let Expected::Networking(json) = f.independent().unwrap().expected else {
        panic!()
    };
    let mut actual = expected(json, &declared("redirect-loops").case().unwrap()).unwrap();
    actual.rows[0].acquisitions[10].request.headers[0].1[0] = b'!';
    let evaluated = report::evaluate(&f, Execution::NetworkingObserved(actual));
    assert_eq!(evaluated.status(), "FAIL");
    let mut text = Vec::new();
    report::write_report(&[evaluated], &mut text).unwrap();
    assert!(
        String::from_utf8(text)
            .unwrap()
            .contains("/rows[0]/acquisitions[10]/request/headers[0]/value[0]")
    );
}
#[test]
fn ai9_diagnostic_difference_count_excess_is_error() {
    let Expected::Networking(json) = fixture("response-200").independent().unwrap().expected else {
        panic!()
    };
    let mut left = expected(json, &declared("response-200").case().unwrap()).unwrap();
    left.rows[0].body = Body::Bytes { bytes: vec![0; 64] };
    let mut right = left.clone();
    right.rows[0].body = Body::Bytes { bytes: vec![1; 64] };
    assert!(
        differences(&left, &right)
            .unwrap_err()
            .detail
            .contains("difference count budget")
    );
}
