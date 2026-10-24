use std::process::Command;

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"));
    command.arg("--chromium");
    command
}

#[test]
fn explicit_capture_never_succeeds_without_configuration_or_a_browser() {
    for args in [
        vec![],
        vec!["--chromium-executable", "/nonexistent/borrowser-cft"],
    ] {
        let output = command()
            .env_remove("BORROWSER_CHROMIUM_EXECUTABLE")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("ERROR chromium."));
    }
}

#[test]
#[ignore = "requires BORROWSER_CHROMIUM_EXECUTABLE naming the pinned real browser"]
fn real_chromium_cli_has_stable_independent_observations() {
    std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit real test requires Chromium");
    let mut previous = None;
    for iteration in 0..3 {
        let output = command().output().unwrap();
        assert!(
            output.status.success(),
            "independent CLI iteration={iteration} exit={:?}; stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schema"], "borrowser.chromium-phases.v2");
        assert_eq!(report["viewport"], serde_json::json!([640, 480]));
        assert_eq!(report["sample_pixel"], serde_json::json!([32, 32]));
        let observations = report["observations"].as_array().unwrap();
        assert!(!observations.is_empty());
        assert!(observations.windows(2).all(|pair| {
            let key = |row: &serde_json::Value| {
                (
                    row["descriptor"]["case_id"].as_str().unwrap().to_owned(),
                    if row["descriptor"]["role"] == "test" {
                        0
                    } else {
                        1
                    },
                )
            };
            key(&pair[0]) < key(&pair[1])
        }));
        for (id, color) in [
            ("canvas/root", [18, 52, 86]),
            ("canvas/cascade", [52, 86, 120]),
        ] {
            let case = observations
                .iter()
                .find(|c| c["descriptor"]["case_id"] == id)
                .unwrap();
            assert_eq!(case["canvas_color_rgb8"], serde_json::json!(color));
        }
        let ids = [
            "canvas/root",
            "canvas/cascade",
            "dom/structure",
            "dom/template-namespaces",
            "style/cascade-inheritance",
            "layout/block-boxes",
        ];
        for (index, id) in ids.into_iter().enumerate() {
            let observation = observations
                .iter()
                .find(|o| o["descriptor"]["case_id"] == id)
                .unwrap();
            if index < 2 {
                assert!(observation["inspection"].is_null());
            } else {
                assert!(observation["canvas_color_rgb8"].is_null());
                let inspection = &observation["inspection"];
                assert_eq!(inspection["dom"].is_object(), index < 4);
                let styles = inspection["styles"].as_array().unwrap();
                let layout = inspection["layout"].as_array().unwrap();
                assert_eq!(
                    styles
                        .iter()
                        .map(|s| s["target"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    if index == 4 {
                        vec!["parent", "child", "hidden"]
                    } else {
                        vec![]
                    }
                );
                assert!(
                    styles
                        .iter()
                        .all(|s| s["values"].as_array().unwrap().len() == 4)
                );
                assert_eq!(
                    layout
                        .iter()
                        .map(|l| l["target"].as_str().unwrap())
                        .collect::<Vec<_>>(),
                    if index == 5 {
                        vec!["container", "child", "sibling", "hidden", "zero", "missing"]
                    } else {
                        vec![]
                    }
                );
            }
            assert_eq!(
                observation["browser"]["capture_profile"],
                "ag7-static-references-v1"
            );
            assert!(observation.get("expected").is_none());
            assert!(observation.get("status").is_none());
        }
        if let Some(previous) = previous {
            assert_eq!(output.stdout, previous);
        }
        previous = Some(output.stdout);
    }
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn failed_native_cli_retains_capture_and_cleanup_evidence() {
    let runtime = tempfile::Builder::new().prefix("c-").tempdir().unwrap();
    // The test executable rejects Chromium's flags and exits. This exercises
    // the normal CLI's failed native capture, without a script or a real browser.
    let executable = std::env::current_exe().unwrap();
    let output = command()
        .arg("canvas/root")
        .arg("--chromium-executable")
        .arg(executable)
        .env("TMPDIR", runtime.path())
        .output()
        .unwrap();
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{diagnostic}");
    assert!(output.stdout.is_empty());
    for required in [
        "AG2 capture failure:",
        "native_check=",
        "root_at_failure=",
        "response_pipe_at_failure=",
        "AG2 cleanup:",
        "owned_terminated: true",
        "root_reaped=true",
        "artifacts: \"removed\"",
        "errors=[]",
        "id=\"canvas/root\"",
        "role=Test",
        "Borrowser workers not started",
        "ERROR chromium.",
    ] {
        assert!(
            diagnostic.contains(required),
            "missing {required}: {diagnostic}"
        );
    }
    assert!(std::fs::read_dir(runtime.path()).unwrap().next().is_none());
    assert!(diagnostic.len() < 8192);
}

#[test]
#[cfg(target_os = "macos")]
fn macos_execution_rejected_before_normal_cli_launch() {
    use std::os::unix::fs::PermissionsExt;
    let runtime = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let executable = input.path().join("test-owned-executable");
    let marker = input.path().join("executed");
    // A test-owned sentinel, never the installed browser. Its marker proves
    // rejection even when the supplied executable would otherwise be runnable.
    std::fs::write(
        &executable,
        format!(
            "#!/bin/sh\n/usr/bin/touch '{}'\nexit 99\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    for configuration in ["existing", "missing", "unset"] {
        if configuration == "missing" {
            std::fs::remove_file(&executable).unwrap();
        }
        for mode in ["--chromium", "--compare"] {
            let mut cli = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"));
            cli.args([mode, "canvas/root"])
                .env_remove("BORROWSER_CHROMIUM_EXECUTABLE")
                .env("TMPDIR", runtime.path());
            if configuration != "unset" {
                cli.arg("--chromium-executable").arg(&executable);
            }
            let output = cli.env("TMPDIR", runtime.path()).output().unwrap();
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(2), "{diagnostic}");
            assert!(output.stdout.is_empty());
            assert!(diagnostic.contains("chromium.platform"), "{diagnostic}");
            assert!(diagnostic.contains("macOS Chromium execution is temporarily unsupported"));
            assert!(diagnostic.contains("ownership cleanup completeness is not proven"));
            assert!(!diagnostic.contains("process ownership:"));
            assert!(!diagnostic.contains("AG2 capture failure:"));
            assert!(!diagnostic.contains("AG2 cleanup:"));
            assert!(!marker.exists(), "attempted executable was launched");
            assert!(std::fs::read_dir(runtime.path()).unwrap().next().is_none());
        }
    }
}

#[test]
fn ag8_capture_resolves_suites_before_entering_the_adapter() {
    for (args, origin) in [
        (vec!["--suite", "ci", "wpt/rgb-alias-p6"], "harness"),
        (
            vec![
                "--suite",
                "ci",
                "--chromium-executable",
                "/nonexistent/borrowser-cft",
            ],
            "chromium",
        ),
    ] {
        let output = command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains(&format!(
            "RUN_ERROR scope=invocation errors=1 origin={origin}"
        )));
        assert!(text.contains("case_results=not_published"));
    }
}
