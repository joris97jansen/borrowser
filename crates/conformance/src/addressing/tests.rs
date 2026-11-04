use super::*;
use crate::{fixtures, model::*, report, run_fixture};
use serde_json::{Value, json};

fn fixture(stem: &str) -> Fixture {
    *fixtures::fixture(&format!("addressing/{stem}"))
}
fn input(stem: &str) -> AddressingInput {
    match fixture(stem).input {
        FixtureInput::Addressing(input) => input,
        _ => panic!("addressing fixture"),
    }
}
fn modified(stem: &str, edit: impl FnOnce(&mut Value)) -> AddressingInput {
    let original = input(stem);
    let mut json: Value = serde_json::from_str(original.declaration).unwrap();
    edit(&mut json);
    AddressingInput {
        declaration: Box::leak(serde_json::to_string(&json).unwrap().into_boxed_str()),
        ..original
    }
}
fn expectation_fixture(
    stem: &str,
    edit: impl FnOnce(&mut Value),
    disposition: Expectation,
) -> Fixture {
    let original = fixture(stem);
    let Expected::Addressing(json) = original.independent().unwrap().expected else {
        panic!("addressing expected")
    };
    let mut expected: Value = serde_json::from_str(json).unwrap();
    edit(&mut expected);
    Fixture {
        kind: FixtureKind::Runnable {
            checks: Checks::Independent(IndependentExpectation {
                expected: Expected::Addressing(Box::leak(
                    serde_json::to_string(&expected).unwrap().into_boxed_str(),
                )),
                disposition,
            }),
        },
        ..original
    }
}
#[test]
fn ah9_required_matrix_has_local_execution_and_independent_snapshots() {
    let selected = fixtures::select_suite(Some(fixtures::Suite::Addressing), None).unwrap();
    assert_eq!(selected.len(), 24);
    let mut pass = 0;
    let mut unsupported = 0;
    for fixture in selected {
        let report = report::evaluate(fixture, run_fixture(fixture));
        if fixture.id.0.starts_with("addressing/unsupported-") {
            assert!(
                matches!(fixture.kind, FixtureKind::Unsupported { reason } if !reason.trim().is_empty())
            );
            assert_eq!(report.status(), "UNSUPPORTED");
            unsupported += 1;
        } else {
            assert!(matches!(fixture.kind, FixtureKind::Runnable { .. }));
            assert_eq!(report.status(), "PASS", "{}", fixture.id.0);
            pass += 1;
        }
    }
    assert_eq!((pass, unsupported), (22, 2));
}
#[test]
fn ah9_wrong_expectation_fails_with_field_difference_and_known_failure_never_hides_error() {
    for (change, disposition, status, exit) in [
        (true, Expectation::Match, "FAIL", 1),
        (
            true,
            Expectation::KnownFailure {
                reason: "test-only genuine mismatch",
            },
            "XFAIL",
            0,
        ),
        (
            false,
            Expectation::KnownFailure {
                reason: "test-only stale mismatch",
            },
            "XPASS",
            1,
        ),
    ] {
        let fixture = expectation_fixture(
            "origins-opaque",
            |value| {
                if change {
                    value["rows"][0]["same_origin"] = json!(true);
                }
            },
            disposition,
        );
        let evaluated = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(evaluated.status(), status);
        assert_eq!(report::exit_code(std::slice::from_ref(&evaluated)), exit);
        let mut output = vec![];
        report::write_report(&[evaluated], &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        if change {
            assert!(
                output.contains("difference: /rows[0]/same_origin expected true, actual false"),
                "{output}"
            );
        }
        let error = report::evaluate(
            &fixture,
            Execution::Error(HarnessError::new(
                "addressing.transport",
                "unexpected command",
            )),
        );
        assert_eq!(error.status(), "ERROR");
        assert_eq!(report::exit_code(&[error]), 2);
    }
}
#[test]
fn ah9_empty_partial_wrong_family_and_missing_origin_observations_are_errors() {
    for edit in [
        (|v: &mut Value| v["rows"] = json!([])) as fn(&mut Value),
        |v| {
            v["rows"].as_array_mut().unwrap().pop();
        },
        |v| v["kind"] = json!("url"),
        |v| {
            v["rows"][0]["left"]["outcome"]["url"]
                .as_object_mut()
                .unwrap()
                .remove("query");
        },
        |v| {
            v["rows"][0]["left"]["outcome"] =
                json!({"kind":"failed","diagnostics":[],"failure":"invalid_port"})
        },
    ] {
        let fixture = expectation_fixture(
            "origins-opaque",
            edit,
            Expectation::KnownFailure {
                reason: "cannot conceal missing evidence",
            },
        );
        let evaluated = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(evaluated.status(), "ERROR");
        assert_eq!(report::exit_code(&[evaluated]), 2);
    }
    let fixture = fixture("origins-opaque");
    let error = report::evaluate(
        &fixture,
        Execution::AddressingObserved(AddressingObservation::Origins(vec![])),
    );
    assert_eq!(error.status(), "ERROR");
    let empty = modified("url-normalization", |v| v["addressing"]["rows"] = json!([]));
    assert_eq!(execute(empty).unwrap_err().code, "addressing.input");
}
#[test]
fn ah9_resource_manifest_is_checked_in_both_directions_and_counts_requests_not_occurrences() {
    let actual = execute(input("html-resources")).unwrap();
    let AddressingObservation::Documents(rows) = actual else {
        panic!("document")
    };
    assert_eq!(rows[0].requests.len(), 3); // Two CSS attachments, one shared image request.
    assert_eq!(rows[0].snapshots[0].html.len(), 10); // Authored duplicates remain separate.
    for edit in [
        (|v: &mut Value| {
            v["addressing"]["rows"][0]["requests"]
                .as_array_mut()
                .unwrap()
                .remove(0);
        }) as fn(&mut Value),
        |v| {
            let r = v["addressing"]["rows"][0]["requests"][2].clone();
            v["addressing"]["rows"][0]["requests"]
                .as_array_mut()
                .unwrap()
                .push(r);
        },
        |v| {
            v["addressing"]["rows"][0]["requests"][0]["url"] =
                json!("https://missing.test/never.css")
        },
    ] {
        assert_eq!(
            execute(modified("html-resources", edit)).unwrap_err().code,
            "addressing.transport"
        );
    }
    let wrong = modified("css-external", |v| {
        v["addressing"]["rows"][0]["requests"][0]["response"]["css"] = json!("missing.css")
    });
    assert_eq!(execute(wrong).unwrap_err().code, "addressing.asset");
}
#[test]
fn ah9_missing_resource_observation_cannot_be_known_failure_and_phase_denial_is_preserved() {
    let fixture = expectation_fixture(
        "scheme-capabilities",
        |v| v["rows"][0]["outcome"]["resource"] = json!({"state":"inapplicable"}),
        Expectation::KnownFailure {
            reason: "missing address is not a mismatch",
        },
    );
    assert_eq!(
        report::evaluate(&fixture, run_fixture(&fixture)).status(),
        "ERROR"
    );
    for html in [
        "<!doctype html><img src='https://example.test/a.png'>",
        "<!doctype html><link rel=stylesheet href='https://example.test/a.css'>",
    ] {
        let error = crate::execute::execute_fixture(
            html.as_bytes(),
            crate::projection::ObservationRequest::CANVAS,
        )
        .unwrap_err();
        assert_eq!(error.code, "execution.subresource");
    }
}
#[test]
fn ah9_exact_snapshots_do_not_change_private_production_debug() {
    let url = BrowserUrl::parse(
        "https://private-user:private-pass@private.test:8443/private?q=secret#secret",
    )
    .unwrap()
    .url;
    let exact = parsed(&url);
    assert!(exact.serialization.contains("private-pass"));
    let debug = format!("{url:?} {:?}", url.url_origin());
    for secret in [
        "private-user",
        "private-pass",
        "private.test",
        "8443",
        "secret",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn ah9_unavailable_cannot_replace_inapplicable_or_available_required_fields() {
    for (stem, path) in [
        ("url-normalization", vec!["rows", "0", "base"]),
        (
            "document-default",
            vec!["rows", "0", "retained_origin_same"],
        ),
        (
            "css-inline",
            vec![
                "rows",
                "0",
                "snapshots",
                "0",
                "css",
                "0",
                "references",
                "0",
                "source",
                "location",
            ],
        ),
    ] {
        let fixture = expectation_fixture(
            stem,
            |value| {
                let mut field = value;
                for component in path {
                    field = if let Ok(index) = component.parse::<usize>() {
                        &mut field[index]
                    } else {
                        &mut field[component]
                    };
                }
                *field =
                    json!({"state":"unavailable","value":"missing evidence cannot satisfy XFAIL"});
            },
            Expectation::KnownFailure {
                reason: "invalid evidence",
            },
        );
        assert_eq!(
            report::evaluate(&fixture, run_fixture(&fixture)).status(),
            "ERROR"
        );
    }
}
