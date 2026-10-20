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
        assert_eq!(report["schema"], "borrowser.chromium-phases.v1");
        assert_eq!(report["viewport"], serde_json::json!([640, 480]));
        assert_eq!(report["sample_pixel"], serde_json::json!([32, 32]));
        let observations = report["observations"].as_array().unwrap();
        assert!(!observations.is_empty());
        assert!(observations.windows(2).all(
            |pair| pair[0]["test_id"].as_str().unwrap() < pair[1]["test_id"].as_str().unwrap()
        ));
        for (id, color) in [
            ("canvas/root", [18, 52, 86]),
            ("canvas/cascade", [52, 86, 120]),
        ] {
            let case = observations.iter().find(|c| c["test_id"] == id).unwrap();
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
            let observation = observations.iter().find(|o| o["test_id"] == id).unwrap();
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
                "ag5-static-phases-v1"
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
