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
    for _ in 0..3 {
        let output = command().output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["schema"], "borrowser.chromium-canvas.v1");
        assert_eq!(report["viewport"], serde_json::json!([640, 480]));
        assert_eq!(report["sample_pixel"], serde_json::json!([32, 32]));
        let observations = report["observations"].as_array().unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(
            observations[0]["canvas_color_rgb8"],
            serde_json::json!([18, 52, 86])
        );
        assert_eq!(
            observations[1]["canvas_color_rgb8"],
            serde_json::json!([52, 86, 120])
        );
        for observation in observations {
            assert_eq!(
                observation["browser"]["capture_profile"],
                "ag2-canvas-srgb-v2"
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
