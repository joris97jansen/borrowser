#[path = "support/ag7.rs"]
mod ag7;
use std::process::Command;

fn authored_projection(projection: serde_json::Value) -> serde_json::Value {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("numeric.fixture.json"),
        br#"{"id":"test/numeric","expected":{"kind":"projection","path":"numeric.expected.json"}}"#,
    )
    .unwrap();
    std::fs::write(
        temp.path().join("numeric.expected.json"),
        serde_json::to_vec(&projection).unwrap(),
    )
    .unwrap();
    ag7::declarations(temp.path())["test/numeric"]["authored_expected"].clone()
}

fn font_projection(value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"dom":{"state":"not_requested"},"styles":[{
        "target":"probe","property":"font-size","value":{"state":"observed","value":{
            "state":"present","value":{"kind":"css_px","value":value}}}}],
        "layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}})
}

#[test]
fn ag7_authored_integer_css_pixels_accept_typed_decimal() {
    let authored = font_projection(serde_json::json!(20));
    let observed = font_projection(serde_json::json!(20.0));
    // Demonstrate why raw JSON equality is not AG5 typed numeric equality.
    assert_ne!(authored, observed);
    assert_eq!(authored_projection(authored), observed);
}

#[test]
fn ag7_authored_decimal_css_pixels_and_real_differences() {
    let expected = authored_projection(font_projection(serde_json::json!(20.0)));
    assert_eq!(expected, font_projection(serde_json::json!(20.0)));
    assert_ne!(expected, font_projection(serde_json::json!(21.0)));
    // Exact equality, including differences far smaller than a CSS pixel.
    assert_ne!(
        expected,
        font_projection(serde_json::json!(20.000000000000004))
    );
}

fn layout_projection(rect: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"dom":{"state":"not_requested"},"styles":[],
        "layout":{"state":"observed","value":[{"target":"probe",
            "value":{"state":"present","value":[rect]}}]},
        "canvas":{"state":"not_requested"}})
}

#[test]
fn ag7_authored_layout_coordinates_use_exact_typed_numbers() {
    use serde_json::json;
    let observed = layout_projection(json!({"left":0.0,"top":20.0,"right":40.0,"bottom":60.0}));
    for rect in [
        json!({"left":0,"top":20,"right":40,"bottom":60}),
        json!({"left":0.0,"top":20.0,"right":40.0,"bottom":60.0}),
    ] {
        let expected = authored_projection(layout_projection(rect));
        assert_eq!(expected, observed);
        for edge in ["left", "top", "right", "bottom"] {
            let mut different = observed.clone();
            different["layout"]["value"][0]["value"]["value"][0][edge] = json!(21.0);
            assert_ne!(expected, different, "{edge}");
        }
    }
}

#[test]
fn ag7_numeric_decoding_preserves_kinds_colors_and_correspondence() {
    use serde_json::json;
    let valid = font_projection(json!(20.0));
    for wrong in [
        json!({"kind":"keyword","value":"20"}),
        json!({"kind":"rgba8","value":[20,0,0,255]}),
    ] {
        let mut authored = font_projection(json!(20));
        authored["styles"][0]["value"]["value"]["value"] = wrong;
        assert_ne!(authored_projection(authored), valid);
    }
    let mut color = valid.clone();
    color["styles"][0]["property"] = json!("background-color");
    color["styles"][0]["value"]["value"]["value"] = json!({"kind":"rgba8","value":[20,0,0,255]});
    assert_eq!(authored_projection(color.clone()), color);
    for invalid in [
        json!({"kind":"css_px","value":20}),
        json!({"kind":"keyword","value":"red"}),
        json!({"kind":"rgba8","value":[20.0,0,0,255]}),
        json!({"kind":"rgba8","value":[256,0,0,255]}),
        json!({"kind":"rgba8","value":[20,0,0,128]}),
        json!({"kind":"rgba8","value":[20,0,0,0]}),
    ] {
        let mut authored = color.clone();
        authored["styles"][0]["value"]["value"]["value"] = invalid;
        assert_ne!(authored_projection(authored), color);
    }
    for pointer in [
        "/styles/0/target",
        "/styles/0/property",
        "/styles/0/value/state",
        "/dom/state",
    ] {
        let mut authored = valid.clone();
        *authored.pointer_mut(pointer).unwrap() = json!("changed");
        assert_ne!(authored_projection(authored), valid);
    }
    let mut ordered = valid.clone();
    ordered["styles"]
        .as_array_mut()
        .unwrap()
        .push(color["styles"][0].clone());
    let mut reversed = ordered.clone();
    reversed["styles"].as_array_mut().unwrap().reverse();
    assert_ne!(authored_projection(reversed), ordered);
    let mut extra = valid.clone();
    extra["styles"][0]["extra"] = json!(20);
    assert_ne!(authored_projection(extra), valid);
}

#[test]
fn ag7_numeric_decoding_rejects_malformed_numbers() {
    use serde_json::json;
    for invalid in [json!("20"), json!(null), json!(true), json!([20])] {
        assert!(
            std::panic::catch_unwind(|| authored_projection(font_projection(invalid.clone())))
                .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| authored_projection(layout_projection(
                json!({"left":0,"top":0,"right":invalid,"bottom":20})
            )))
            .is_err()
        );
    }
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"));
    command.arg("--compare");
    command
}

#[test]
fn comparison_configuration_errors_publish_nothing() {
    for args in [
        vec![],
        vec!["unknown"],
        vec!["canvas/root", "canvas/cascade"],
        vec!["--chromium-executable"],
        vec!["--unexpected"],
        vec!["--chromium-executable", "/nonexistent/borrowser-cft"],
    ] {
        let output = command()
            .env_remove("BORROWSER_CHROMIUM_EXECUTABLE")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("ERROR "));
    }
}

fn output_with_closed_stderr(mut command: Command) -> std::process::Output {
    let (reader, writer) = std::io::pipe().unwrap();
    // Close the only reader before spawning: the child can never successfully
    // write a diagnostic. No parent/child closure race or global signal change.
    drop(reader);
    command.stderr(writer).output().unwrap()
}

#[test]
fn comparison_closed_stderr_preserves_exit_two() {
    let mut command = command();
    command
        .env_remove("BORROWSER_CHROMIUM_EXECUTABLE")
        .arg("unknown");
    let output = output_with_closed_stderr(command);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn non_utf8_argument_closed_stderr_preserves_exit_two() {
    use std::os::unix::ffi::OsStringExt;
    let invalid = std::ffi::OsString::from_vec(vec![0xff]);
    let mut command = command();
    command.arg(invalid);
    let writable = command.output().unwrap();
    assert_eq!(writable.status.code(), Some(2));
    assert!(writable.stdout.is_empty());
    assert_eq!(
        writable.stderr,
        b"ERROR command: test identity must be UTF-8\n"
    );
    let failed = output_with_closed_stderr(command);
    assert_eq!(failed.status.code(), Some(2));
    assert!(failed.stdout.is_empty());
}

#[test]
#[ignore = "requires BORROWSER_CHROMIUM_EXECUTABLE naming the pinned real browser"]
fn real_chromium_combined_cli_is_stable_and_selects_exactly() {
    let executable = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit combined test requires Chromium");
    for selection in [Some("canvas/cascade"), None] {
        let mut previous = None;
        for iteration in 0..3 {
            let mut command = command();
            if iteration == 0 {
                // Explicit executable wins over an unusable environment value.
                command
                    .env(
                        "BORROWSER_CHROMIUM_EXECUTABLE",
                        "/nonexistent/borrowser-cft",
                    )
                    .arg("--chromium-executable")
                    .arg(&executable);
            }
            if let Some(id) = selection {
                command.arg(id);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(if selection.is_some() { 0 } else { 1 }),
                "selection={selection:?} iteration={iteration} status={} stdout={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(report["schema"], "borrowser.cross-engine-phases.v2");
            assert_eq!(
                report["environment"]["sample_pixel"],
                serde_json::json!([32, 32])
            );
            let cases = report["cases"].as_array().unwrap();
            let expected: &[(&str, [u8; 3])] = if selection.is_some() {
                &[("canvas/cascade", [52, 86, 120])]
            } else {
                &[
                    ("canvas/root", [18, 52, 86]),
                    ("canvas/cascade", [52, 86, 120]),
                ]
            };
            assert!(!cases.is_empty());
            if selection.is_some() {
                assert_eq!(cases.len(), 1);
            }
            assert!(
                cases
                    .windows(2)
                    .all(|pair| pair[0]["test_id"].as_str().unwrap()
                        < pair[1]["test_id"].as_str().unwrap())
            );
            if selection.is_none() {
                assert_general_catalogue(&report, output.status.code().unwrap());
            } else {
                assert_eq!(report["summary"]["different_fields"], 0);
                assert!(
                    cases
                        .iter()
                        .all(|c| c["status"] == "equal_for_requested_scope")
                );
            }
            let ids: &[&str] = if selection.is_some() {
                &["canvas/cascade"]
            } else {
                &[
                    "canvas/root",
                    "canvas/cascade",
                    "dom/structure",
                    "dom/template-namespaces",
                    "style/cascade-inheritance",
                    "layout/block-boxes",
                ]
            };
            let mut baseline_fields = 0;
            for id in ids {
                let case = cases.iter().find(|c| c["test_id"] == *id).unwrap();
                assert_eq!(case["borrowser_expectation"]["status"], "PASS");
                let fields = case["fields"].as_array().unwrap();
                baseline_fields += fields.len();
                assert_eq!(
                    fields
                        .iter()
                        .map(|f| f["id"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    expected_profile_fields(id)
                );
                assert!(fields.iter().all(|f| f["status"] == "equal"));
            }
            assert_eq!(baseline_fields, if selection.is_some() { 1 } else { 34 });
            for (id, color) in expected {
                let case = cases.iter().find(|c| c["test_id"] == *id).unwrap();
                for engine in ["borrowser", "chromium"] {
                    assert_eq!(
                        case["captures"][engine]["canvas_color_rgb8"],
                        serde_json::json!(color)
                    );
                    assert_eq!(case["fields"][0][engine], serde_json::json!(color));
                }
                assert_eq!(
                    case["captures"]["chromium"]["browser"]["capture_profile"],
                    "ag7-static-references-v1"
                );
                assert_eq!(case["fields"][0]["status"], "equal");
                assert_eq!(case["borrowser_expectation"]["status"], "PASS");
            }
            if let Some(previous) = previous {
                assert_eq!(output.stdout, previous);
            }
            previous = Some(output.stdout);
        }
    }
}

fn expected_profile_fields(id: &str) -> Vec<String> {
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
fn acceptance_rejects_an_additional_unrelated_difference_even_with_exit_one() {
    use serde_json::json;
    let field = || json!({"id":"style:#p6/background-color","status":"different"});
    let mut report = json!({"cases":ag7::RGB.map(|id|json!({"test_id":id,"fields":[field()],"reference_document":{"fields":[field()]}})),"summary":{"different_fields":4,"equal_fields":36},"exit":1});
    ag7::assert_exact_difference_set(&report);
    report["cases"].as_array_mut().unwrap().push(json!({"test_id":"canvas/cascade","fields":[{"id":"canvas_color_rgb8","status":"different","borrowser":[52,86,120],"chromium":[52,86,121],"channels":[{"channel":"red","status":"equal"},{"channel":"green","status":"equal"},{"channel":"blue","status":"different"}]}],"reference_document":null}));
    report["summary"]["different_fields"] = json!(5);
    report["summary"]["equal_fields"] = json!(35);
    assert_eq!(report["exit"], 1);
    assert!(std::panic::catch_unwind(|| ag7::assert_exact_difference_set(&report)).is_err());
}

#[test]
#[ignore = "requires pinned Chromium; each selected comparison is a fresh process"]
fn real_chromium_ag7_scoped_relationships() {
    for (id, expected_exit) in ag7::RGB
        .into_iter()
        .map(|id| (id, 1))
        .chain([(ag7::LOCAL, 0)])
        .chain(ag7::EXCLUDED.into_iter().map(|id| (id, 3)))
    {
        let output = command().arg(id).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{id}: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{id}: status={} stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["cases"].as_array().unwrap().len(), 1);
        assert_eq!(report["cases"][0]["test_id"], id);
        assert_eq!(
            report["summary"]["executed_documents"],
            if expected_exit == 3 { 0 } else { 2 }
        );
        assert_eq!(
            report["summary"]["different_fields"],
            if expected_exit == 1 { 2 } else { 0 }
        );
        eprintln!(
            "AG7 selected {id}: exit={expected_exit} summary={}",
            report["summary"]
        );
    }
    let output = command().output().unwrap();
    assert!(
        output.stderr.is_empty(),
        "catalogue: status={} stdout={} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_general_catalogue(&report, output.status.code().unwrap());
}

fn assert_general_catalogue(report: &serde_json::Value, exit: i32) {
    let identity = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
        .arg("--ag4-artifact-info")
        .output()
        .unwrap();
    assert!(identity.status.success());
    let identity: serde_json::Value = serde_json::from_slice(&identity.stdout).unwrap();
    let authored =
        ag7::declarations(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures"));
    ag7::assert_catalogue(report, exit, &identity["fixtures"], &authored);
}
