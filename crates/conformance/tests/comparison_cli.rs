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
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(report["schema"], "borrowser.cross-engine-canvas.v1");
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
            assert_eq!(cases.len(), expected.len());
            assert_eq!(report["summary"]["equal_fields"], cases.len());
            for (case, (id, color)) in cases.iter().zip(expected) {
                assert_eq!(case["test_id"], *id);
                for engine in ["borrowser", "chromium"] {
                    assert_eq!(
                        case["captures"][engine]["canvas_color_rgb8"],
                        serde_json::json!(color)
                    );
                    assert_eq!(case["fields"][0][engine], serde_json::json!(color));
                }
                assert_eq!(
                    case["captures"]["chromium"]["browser"]["capture_profile"],
                    "ag2-canvas-srgb-v2"
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
