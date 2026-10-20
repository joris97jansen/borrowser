use std::process::Command;

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
            assert!(
                matches!(output.status.code(), Some(0 | 3)),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(report["schema"], "borrowser.cross-engine-phases.v1");
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
            assert!(
                cases
                    .iter()
                    .all(|c| c["status"] == "equal_for_requested_scope"
                        || c["status"] == "incomplete")
            );
            assert_eq!(report["summary"]["different_fields"], 0);
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
                    "ag5-static-phases-v1"
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
