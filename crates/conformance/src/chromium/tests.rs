use super::*;
#[test]
fn termination_event_stderr_diagnostic_is_bounded_and_escaped() {
    let mut output = vec![];
    write_chromium_stderr(b"line\n\x1b\xff", &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.starts_with("AG2 Chromium stderr (retained_bytes=7, lossy_utf8, escaped): "));
    assert!(text.contains("line\\n\\u{1b}�"));
    assert_eq!(text.lines().count(), 1);
    let mut bytes = vec![0; 65_536];
    bytes.extend_from_slice(b"NOT_RETAINED");
    let mut output = vec![];
    write_chromium_stderr(&bytes, &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("retained_bytes=65536"));
    assert!(!text.contains("NOT_RETAINED"));
    assert!(text.len() <= 6 * 65_536 + 128);
}
use base64::{Engine, engine::general_purpose::STANDARD};
use std::os::fd::FromRawFd;
use std::path::Path;
use std::process::Command;

fn viewport_metrics() -> Value {
    json!({
        "cssLayoutViewport":{"pageX":0,"pageY":0,"clientWidth":640,"clientHeight":480},
        "cssVisualViewport":{"offsetX":0,"offsetY":0,"pageX":0,"pageY":0,"clientWidth":640,"clientHeight":480,"scale":1,"zoom":1},
        "cssContentSize":{"x":0,"y":0,"width":640,"height":480}
    })
}

#[test]
fn viewport_accepts_observed_linux_dom_scrollbar_dimensions() {
    // Run 39366788841/1, dom/template-namespaces: no scroll or zoom.
    let mut metrics = viewport_metrics();
    metrics["cssLayoutViewport"]["clientWidth"] = json!(625);
    metrics["cssVisualViewport"]["clientWidth"] = json!(625);
    metrics["cssContentSize"] = json!({"x":0,"y":0,"width":625,"height":647});
    for fixture in &crate::fixtures::baseline()[2..] {
        assert_eq!(validate_viewport(&metrics, fixture.request).unwrap(), 625.0);
    }
    // Any requested canvas keeps the original full client-size requirement.
    for request in [
        crate::projection::ObservationRequest::CANVAS,
        crate::projection::ObservationRequest {
            canvas: crate::projection::Request::Compare(()),
            ..crate::fixtures::fixture("dom/template-namespaces").request
        },
    ] {
        assert!(validate_viewport(&metrics, request).is_err());
    }
}

#[test]
fn viewport_native_dimensions_are_exact_bounded_and_not_a_scrollbar_constant() {
    let request = crate::fixtures::fixture("layout/block-boxes").request;
    for width in [640, 637, 625, 320] {
        let mut metrics = viewport_metrics();
        for object in ["cssLayoutViewport", "cssVisualViewport"] {
            metrics[object]["clientWidth"] = json!(width);
            metrics[object]["clientHeight"] = json!(465);
        }
        assert_eq!(
            validate_viewport(&metrics, request).unwrap(),
            f64::from(width)
        );
    }
    let mut metrics = viewport_metrics();
    metrics["cssVisualViewport"]["clientWidth"] = json!(639);
    assert!(
        validate_viewport(&metrics, request).is_err(),
        "inconsistent native viewports"
    );
}

#[test]
fn viewport_rejects_invalid_dimensions_scroll_offsets_scale_and_malformed_data() {
    let request = crate::fixtures::fixture("dom/template-namespaces").request;
    for (object, field, values) in [
        (
            "cssLayoutViewport",
            "clientWidth",
            vec![0.0, -1.0, 641.0, 625.5],
        ),
        ("cssLayoutViewport", "clientHeight", vec![0.0, -1.0, 481.0]),
        ("cssLayoutViewport", "pageX", vec![-1.0, 1.0]),
        ("cssLayoutViewport", "pageY", vec![-1.0, 1.0]),
        ("cssVisualViewport", "clientWidth", vec![0.0, -1.0, 641.0]),
        ("cssVisualViewport", "clientHeight", vec![0.0, -1.0, 481.0]),
        ("cssVisualViewport", "pageX", vec![-1.0, 1.0]),
        ("cssVisualViewport", "pageY", vec![-1.0, 1.0]),
        ("cssVisualViewport", "offsetX", vec![-0.25, 0.25]),
        ("cssVisualViewport", "offsetY", vec![-0.25, 0.25]),
        ("cssVisualViewport", "scale", vec![0.0, 0.5, 2.0]),
        ("cssVisualViewport", "zoom", vec![0.0, 0.5, 2.0]),
    ] {
        for value in
            values
                .into_iter()
                .map(|n| json!(n))
                .chain([Value::Null, json!("0"), json!(false)])
        {
            let mut metrics = viewport_metrics();
            metrics[object][field] = value;
            assert!(
                validate_viewport(&metrics, request).is_err(),
                "{object}.{field}: {metrics}"
            );
        }
        let mut metrics = viewport_metrics();
        metrics[object].as_object_mut().unwrap().remove(field);
        assert_eq!(
            validate_viewport(&metrics, request).is_ok(),
            field == "zoom",
            "{object}.{field}"
        );
    }
}

#[test]
fn viewport_failure_diagnostic_attributes_fixture_metrics_and_cleanup() {
    let mut metrics = viewport_metrics();
    metrics["cssLayoutViewport"]["clientWidth"] = json!(625);
    metrics["cssVisualViewport"]["clientWidth"] = json!(625);
    metrics["cssContentSize"]["height"] = json!(900);
    let fixture = crate::fixtures::fixture("dom/template-namespaces");
    let input = CaptureInput {
        role: crate::chromium::DocumentRole::Test,
        metadata_links: "[]",
        id: fixture.id,
        source: fixture.document.source,
        html: b"must never appear in diagnostics",
        request: fixture.request,
    };
    let failure = attribute_capture_failure(
        Failure {
            context: None,
            primary: Some(
                validate_viewport(&metrics, crate::projection::ObservationRequest::CANVAS)
                    .unwrap_err(),
            ),
            cleanup: vec![error::CleanupError::Timeout],
        },
        &input,
    );
    let mut out = vec![];
    failure.write_diagnostics(&mut out).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.starts_with("ERROR chromium.capture: unexpected viewport dimensions or scroll;"));
    assert!(out.contains("\"id\":\"dom/template-namespaces\""));
    assert!(
        out.contains("\"source\":\"crates/conformance/fixtures/dom-template-namespaces.html\"")
    );
    assert!(out.contains("\"families\":[\"dom\"]"));
    assert!(
        out.contains("\"actual\":625,\"expected\":640,\"field\":\"cssLayoutViewport.clientWidth\"")
    );
    assert!(out.contains("\"field\":\"cssVisualViewport.clientWidth\""));
    assert!(out.contains("\"height\":900"));
    assert!(out.contains("\"offsetX\":0"));
    assert!(out.contains("\"scale\":1"));
    assert!(out.contains("ERROR chromium.cleanup:"));
    assert!(!out.contains("must never appear"));
}

#[test]
fn viewport_failure_diagnostic_distinguishes_missing_malformed_and_mismatch() {
    let mut metrics = viewport_metrics();
    metrics["cssLayoutViewport"]
        .as_object_mut()
        .unwrap()
        .remove("clientWidth");
    metrics["cssVisualViewport"]["clientHeight"] = json!("x".repeat(100_000));
    metrics["cssVisualViewport"]["pageY"] = json!(1);
    let error = validate_viewport(&metrics, crate::projection::ObservationRequest::CANVAS)
        .unwrap_err()
        .to_string();
    let evidence: Value = serde_json::from_str(error.split_once("; viewport=").unwrap().1).unwrap();
    let rejected = evidence["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 3);
    assert_eq!(rejected[0]["reason"], "missing");
    assert_eq!(rejected[0]["actual"], json!({"missing":true}));
    assert_eq!(rejected[1]["reason"], "malformed");
    assert_eq!(rejected[1]["actual"], json!({"malformed":"string"}));
    assert_eq!(rejected[2]["reason"], "numeric_mismatch");
    assert!(error.len() < 4096);
    metrics["cssLayoutViewport"] = json!([]);
    assert!(
        validate_viewport(&metrics, crate::projection::ObservationRequest::CANVAS)
            .unwrap_err()
            .to_string()
            .len()
            < 4096
    );
}

#[test]
fn viewport_canvas_keeps_full_size_and_zero_scroll() {
    validate_viewport(
        &viewport_metrics(),
        crate::projection::ObservationRequest::CANVAS,
    )
    .unwrap();
    for (object, field, value) in [
        ("cssLayoutViewport", "clientWidth", 625),
        ("cssVisualViewport", "clientHeight", 465),
        ("cssLayoutViewport", "pageX", 1),
        ("cssVisualViewport", "pageY", 1),
        ("cssVisualViewport", "scale", 2),
    ] {
        let mut metrics = viewport_metrics();
        metrics[object][field] = json!(value);
        assert!(
            validate_viewport(&metrics, crate::projection::ObservationRequest::CANVAS).is_err(),
            "{object}.{field}"
        );
    }
    // Content extent is evidence, not an additional capture predicate.
    let mut metrics = viewport_metrics();
    metrics["cssContentSize"]["height"] = json!(900);
    validate_viewport(&metrics, crate::projection::ObservationRequest::CANVAS).unwrap();
}

fn png_bytes(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
    }
    bytes
}
#[test]
fn screenshot_samples_exact_pixel_and_rejects_alpha() {
    let mut pixels = vec![0_u8; WIDTH as usize * HEIGHT as usize * 4];
    let offset = (32 * WIDTH as usize + 32) * 4;
    pixels[offset..offset + 4].copy_from_slice(&[18, 52, 86, 255]);
    let encoded = STANDARD.encode(png_bytes(WIDTH, HEIGHT, png::ColorType::Rgba, &pixels));
    assert_eq!(
        screenshot::sample(&encoded).unwrap(),
        CanvasColor([18, 52, 86])
    );
    pixels[offset + 3] = 254;
    assert!(matches!(
        screenshot::sample(&STANDARD.encode(png_bytes(
            WIDTH,
            HEIGHT,
            png::ColorType::Rgba,
            &pixels
        ))),
        Err(Error::Capture(_))
    ));
}
#[test]
fn screenshot_rejects_malformed_wrong_size_and_non_rgb() {
    for encoded in [
        "!".into(),
        STANDARD.encode(b"not PNG"),
        STANDARD.encode(png_bytes(1, 1, png::ColorType::Rgb, &[1, 2, 3])),
        STANDARD.encode(png_bytes(
            WIDTH,
            HEIGHT,
            png::ColorType::Grayscale,
            &vec![0; WIDTH as usize * HEIGHT as usize],
        )),
        "A".repeat(3 * 1024 * 1024),
    ] {
        assert!(matches!(
            screenshot::sample(&encoded),
            Err(Error::Capture(_))
        ));
    }
    let mut bytes = png_bytes(
        WIDTH,
        HEIGHT,
        png::ColorType::Rgb,
        &vec![0; WIDTH as usize * HEIGHT as usize * 3],
    );
    bytes.truncate(bytes.len() - 4);
    assert!(screenshot::sample(&STANDARD.encode(bytes)).is_err());
}
#[test]
fn screenshot_rejects_conflicting_color_metadata_without_normalization() {
    for (gamma, duplicate, accepted) in [
        (45455_u32, false, true),
        (50000, false, false),
        (45455, true, false),
    ] {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, WIDTH, HEIGHT);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_chunk(png::chunk::gAMA, &gamma.to_be_bytes())
                .unwrap();
            if duplicate {
                writer.write_chunk(png::chunk::sRGB, &[0]).unwrap();
            }
            writer
                .write_image_data(&vec![52; WIDTH as usize * HEIGHT as usize * 3])
                .unwrap();
        }
        let result = screenshot::sample(&STANDARD.encode(&bytes));
        if accepted {
            assert_eq!(result.unwrap(), CanvasColor([52, 52, 52]));
        } else {
            assert!(matches!(result, Err(Error::Capture(_))));
        }
    }
}
#[test]
fn exact_identity_is_required() {
    let pin: Value = serde_json::from_str(include_str!("../../chromium-reference.json")).unwrap();
    let mut version = json!({"product":format!("Chrome/{}",pin["version"].as_str().unwrap()),
        "revision":pin["revision"],"protocolVersion":pin["protocol_version"]});
    let actual = identity(version.clone()).unwrap();
    assert_eq!(actual.capture_profile, "ag7-static-references-v1");
    assert_eq!(
        actual,
        serde_json::from_str::<BrowserIdentity>(&serde_json::to_string(&actual).unwrap()).unwrap()
    );
    for field in ["product", "revision", "protocolVersion"] {
        let saved = version[field].clone();
        version[field] = json!("different");
        assert!(matches!(
            identity(version.clone()),
            Err(Error::IncompatibleBrowser(_))
        ));
        version[field] = saved;
    }
}
#[test]
fn missing_executable_and_cancelled_start_do_not_observe() {
    let config = ChromiumConfig::new(PathBuf::from("/nonexistent/borrowser-cft"));
    let failure = capture_html(b"", &config, &Cancellation::default()).unwrap_err();
    if cfg!(target_os = "macos") {
        assert!(
            matches!(failure.primary, Some(Error::UnsupportedPlatform(ref s))
            if s.contains("ownership cleanup completeness is not proven"))
        );
    } else {
        assert!(matches!(failure.primary, Some(Error::Configuration(_))));
    }
    assert!(failure.cleanup.is_empty());
    let cancel = Cancellation::default();
    cancel.cancel();
    let error = process::OwnedChromium::launch(
        &std::env::current_exe().unwrap(),
        &[],
        Instant::now() + Duration::from_secs(1),
        &cancel,
    )
    .err()
    .unwrap();
    assert!(matches!(error.primary, Some(Error::Cancelled)));
    assert!(error.cleanup.is_empty());
}

// Native tests are explicit subprocesses: subreaper/handlers/fork belong only
// to the standalone path, never to the multithreaded outer Rust test runner.
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_process_and_pipe_contract() {
    run_native_cases(&[
        "exec-error",
        "eof",
        "malformed",
        "oversized",
        "hang",
        "detached",
        "early-exit",
        "cancel",
        "truncated",
        "wrong-id",
        "wrong-session",
        "fragmented",
        "drop",
    ]);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_lifecycle_regressions() {
    run_native_cases(&[
        "partial-descriptor",
        "partial-mask",
        "partial-status",
        "partial-cancel",
        "partial-drop",
        "discovery-runtime",
        "cleanup-discovery",
        "cleanup-verification",
        "cleanup-reap",
    ]);
    #[cfg(target_os = "linux")]
    run_native_cases(&["adopted-reap"]);
    #[cfg(target_os = "macos")]
    run_native_cases(&["arguments-transient", "arguments-persistent"]);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_macos_bsd_ownership_states() {
    run_native_cases(&["bsd-ownership"]);
}
#[test]
fn linux_temporary_socket_path_uses_bytes_and_reserves_nul() {
    use std::os::unix::ffi::OsStrExt;
    let suffix = "/org.chromium.Chromium.XXXXXX/SingletonSocket";
    let longest = format!("/{}", "a".repeat(107 - suffix.len() - 1));
    assert_eq!(longest.len() + suffix.len(), 107);
    assert!(process::validate_linux_temporary_path(Path::new(&longest)).is_ok());
    for invalid in [
        format!("{longest}a"),
        format!("{longest}é"),
        "relative".into(),
        "/tmp/\0bad".into(),
    ] {
        assert!(matches!(
            process::validate_linux_temporary_path(Path::new(&invalid)),
            Err(Error::Configuration(_))
        ));
    }
    // Equal character counts can cross the boundary with a multibyte pathname.
    let unicode = longest.replacen('a', "é", 1);
    assert_eq!(unicode.chars().count(), longest.chars().count());
    assert_eq!(unicode.len() + suffix.len(), 108);
    assert!(process::validate_linux_temporary_path(Path::new(&unicode)).is_err());
    let ci = Path::new("/home/runner/work/_temp/ag2-profiles/ag2-XXXXXX/tmp");
    assert!(process::validate_linux_temporary_path(ci).is_ok());
    assert_eq!(ci.as_os_str().as_bytes().len() + suffix.len(), 96);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_temporary_artifact_containment() {
    run_native_cases(&[
        "temp-clean",
        "temp-exit",
        "temp-cancel",
        "temp-incomplete",
        "temp-remove-failure",
    ]);
    #[cfg(target_os = "linux")]
    run_native_cases(&["temp-path-too-long"]);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_prefork_artifact_cleanup() {
    run_native_cases(&["prefork-clean", "prefork-remove-failure"]);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_cleanup_diagnostic_failures() {
    run_native_cases(&[
        "diagnostic-finish-writable",
        "diagnostic-finish-closed",
        "diagnostic-drop-writable",
        "diagnostic-drop-closed",
        "diagnostic-unwind-writable",
        "diagnostic-unwind-closed",
    ]);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_capture_failure_diagnostics() {
    run_native_cases(&[
        "capture-timeout",
        "capture-send-timeout",
        "capture-discovery-timeout",
        "capture-disconnected",
        "capture-exited",
    ]);
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_startup_exit_event_classification() {
    run_native_cases(&["startup-exit-events", "capture-exited"]);
    // The composition capture boundary rejects macOS before any startup event.
    // Protocol/owner helper regressions above remain platform evidence only.
    #[cfg(target_os = "linux")]
    run_native_cases(&["startup-comparison-failure"]);
}
#[test]
#[cfg(target_os = "macos")]
fn macos_execution_rejected_before_allocation_and_workers() {
    let runtime = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "chromium::tests::macos_support_boundary_child",
            "--ignored",
            "--nocapture",
        ])
        .env("TMPDIR", runtime.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_dir(runtime.path()).unwrap().next().is_none());
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "isolated support-boundary assertion; invoked by its parent test"]
fn macos_support_boundary_child() {
    use process::fault::{self, Point};
    // No Standalone::enter: rejection must precede native permission checks too.
    assert_no_children();
    fault::set(Point::Allocation);
    let config = ChromiumConfig::new(PathBuf::from("/nonexistent/borrowser-cft"));
    let failure = capture_html(b"", &config, &Cancellation::default()).unwrap_err();
    assert!(
        matches!(failure.primary, Some(Error::UnsupportedPlatform(ref s))
        if s.contains("macOS Chromium execution is temporarily unsupported"))
    );
    assert!(failure.cleanup.is_empty());
    crate::comparison::tests::macos_platform_failure_blocks_workers(&config.executable);
    assert!(
        fault::take(Point::Allocation),
        "reached Chromium allocation"
    );
    assert_no_children();
    assert!(
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_resource_completion_and_shutdown() {
    run_native_cases(&[
        "resource-rejected-abort",
        "resource-missing-ack",
        "resource-missing-terminal",
        "resource-shutdown-valid",
        "resource-shutdown-incomplete",
        "resource-shutdown-after-ack",
        "resource-shutdown-invalidated",
        "resource-shutdown-root-invalidated",
        "resource-shutdown-root-clean",
        "resource-shutdown-buffered-deadline",
        "resource-shutdown-partial-timeout",
        "resource-shutdown-no-response",
        "resource-shutdown-ack-only",
        "resource-shutdown-capture-budget",
        "resource-shutdown-truncated",
        "resource-shutdown-clean",
        "resource-shutdown-invalidated-after-ack",
    ]);
}
#[test]
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn native_shutdown_uses_capture_deadline() {
    run_native_cases(&[
        "resource-shutdown-delayed-valid",
        "resource-shutdown-delayed-malformed",
        "resource-shutdown-delayed-truncated",
        "resource-shutdown-delayed-policy",
        "resource-shutdown-delayed-never",
        "resource-shutdown-cancel-after-ack",
    ]);
}
#[test]
#[cfg(target_os = "macos")]
fn persistent_argument_eio_requires_activation() {
    use process::fault::{self, Point};
    fault::reset_persistent_argument_eio_activations();
    fault::set(Point::ArgumentsUnavailablePersistent);
    // An armed but unreached injection must fail the same assertion used by
    // the native regression, even if some other operation exhausts its budget.
    let assertion = std::panic::catch_unwind(|| fault::assert_persistent_argument_eio_since(0));
    assert!(assertion.is_err());
    assert!(fault::take(Point::ArgumentsUnavailablePersistent));
    fault::assert_consumed();
}
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
fn run_native_cases(scenarios: &[&str]) {
    // A sibling of each isolated capture process is outside its ownership,
    // despite using exactly the same executable as the browser test helper.
    let mut unrelated = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "chromium::tests::browser_helper", "--ignored"])
        .env("AG2_NATIVE_CASE", "unrelated")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    for scenario in scenarios {
        let mut command = Command::new(std::env::current_exe().unwrap());
        if *scenario == "bsd-ownership" {
            command.env("AG2_UNRELATED_PID", unrelated.id().to_string());
        }
        // Set only the isolated process's environment, never that of the outer
        // multithreaded test harness. Pre-fork cases
        // inspect their entire parent, which must not contain parallel tests.
        let isolated_parent = if scenario.starts_with("prefork-") {
            // Nine additional pathname bytes fit the pinned Linux socket bound
            // under the existing CI runtime directory.
            let parent = tempfile::Builder::new().prefix("p-").tempdir().unwrap();
            command.env("TMPDIR", parent.path());
            Some(parent)
        } else if *scenario == "temp-path-too-long" {
            let parent = tempfile::tempdir().unwrap();
            let long = parent.path().join("x".repeat(80));
            std::fs::create_dir(&long).unwrap();
            command.env("TMPDIR", long);
            Some(parent)
        } else {
            None
        };
        let output = command
            .args([
                "--exact",
                "chromium::tests::native_case",
                "--ignored",
                "--nocapture",
            ])
            .env("AG2_NATIVE_CASE", scenario)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{scenario}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if *scenario == "startup-comparison-failure" {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let snapshot = stderr.find("AG2 capture failure: BrowserExited").unwrap();
            let cleanup = stderr.find("AG2 cleanup:").unwrap();
            let retained = stderr.find("AG2 Chromium stderr (").unwrap();
            assert!(snapshot < cleanup && cleanup < retained, "{stderr}");
            assert!(stderr.contains("exit_source=Inspector.targetCrashed session=Capture"));
            assert!(stderr.contains("controlled startup stderr\\n"));
            assert!(stderr.contains("owned_terminated: true"));
            assert!(stderr.contains("root_reaped=true"));
            assert!(stderr.contains("errors=[]"));
            assert!(stderr.contains("startup capture failed; no Borrowser workers; exit=2"));
            eprint!("{stderr}");
        }
        if scenario.starts_with("resource-shutdown-")
            || matches!(
                *scenario,
                "bsd-ownership" | "startup-exit-events" | "capture-exited"
            )
        {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        if *scenario == "cleanup-reap" {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("cleanup-reap: armed after helper startup"),
                "{stderr}"
            );
            assert!(
                stderr.contains("cleanup-reap: injected fault consumed"),
                "{stderr}"
            );
        }
        if scenario.starts_with("diagnostic-") {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let writable = scenario.ends_with("-writable");
            assert_eq!(
                stderr
                    .matches("Chromium cleanup incomplete; artifacts: ")
                    .count(),
                usize::from(writable),
                "{scenario}: {stderr}"
            );
            assert_eq!(
                stderr.matches("Chromium emergency cleanup: owned process termination could not be verified before deadline").count(),
                usize::from(writable && !scenario.starts_with("diagnostic-finish-")),
                "{scenario}: {stderr}"
            );
            assert!(!String::from_utf8_lossy(&output.stdout).contains("Chromium cleanup"));
        }
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "terminated unrelated sibling"
        );
        eprintln!("native scenario {scenario}: passed; unrelated sibling survived");
        drop(isolated_parent);
    }
    drop(unrelated.stdin.take());
    assert!(unrelated.wait().unwrap().success());
}
#[test]
#[ignore = "private isolated-process entry; native_process_and_pipe_contract invokes it"]
fn native_case() {
    let scenario = std::env::var("AG2_NATIVE_CASE").expect("private native scenario");
    let _standalone = process::Standalone::enter().unwrap();
    #[cfg(target_os = "macos")]
    if scenario == "bsd-ownership" {
        process::OwnedChromium::test_macos_bsd_ownership_states();
        assert_no_children();
        return;
    }
    // Exercise FD3/4 collisions: launch must replace them and close all the
    // other ambient copies in the exec child without changing the parent.
    let occupied: Vec<_> = (0..16)
        .map(|_| std::fs::File::open("/dev/null").unwrap())
        .collect();
    let _keep_occupied = occupied;
    let cancel = Cancellation::default();
    let parent_tmpdir = std::env::var_os("TMPDIR");
    let exe = if scenario == "exec-error" {
        PathBuf::from("/nonexistent/ag2")
    } else {
        std::env::current_exe().unwrap()
    };
    let args = [
        "--exact",
        "chromium::tests::browser_helper",
        "--ignored",
        "--nocapture",
        "--",
    ]
    .map(str::to_owned);
    let start = Instant::now();
    if scenario == "startup-comparison-failure" {
        #[cfg(target_os = "macos")]
        let bundle = bundle_fixture::Bundle::new();
        #[cfg(target_os = "macos")]
        bundle.use_test_executable();
        #[cfg(target_os = "macos")]
        let exe = bundle.executable.clone();
        crate::comparison::tests::startup_failure_blocks_workers(&exe, "browser-exited");
        assert_no_children();
        return;
    }
    if scenario.starts_with("prefork-") {
        use process::fault::{self, Point};
        use std::os::unix::fs::PermissionsExt;
        let parent = std::env::temp_dir();
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
        let sentinel = parent.join("unrelated");
        std::fs::write(&sentinel, b"preserve").unwrap();
        fault::set(if scenario == "prefork-clean" {
            Point::PreFork
        } else {
            Point::PreForkArtifacts
        });
        let failure =
            process::OwnedChromium::launch(&exe, &args, start + Duration::from_secs(2), &cancel)
                .err()
                .expect("pre-fork failure must not become successful launch");
        // Consumption proves initialization created the root and its evidence
        // file before returning the exact same primary error in both cases.
        fault::assert_consumed();
        assert!(matches!(
            failure.primary,
            Some(Error::Launch {
                stage: "injected pre-fork initialization",
                errno: libc::EIO,
            })
        ));
        assert_no_children();
        let remaining: Vec<_> = std::fs::read_dir(&parent)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path != &sentinel)
            .collect();
        if scenario == "prefork-clean" {
            assert!(failure.cleanup.is_empty(), "{failure:?}");
            assert!(
                remaining.is_empty(),
                "private directory survived: {remaining:?}"
            );
        } else {
            assert_eq!(remaining.len(), 1, "{remaining:?}");
            let artifacts = &remaining[0];
            let [error::CleanupError::Artifacts(error)] = failure.cleanup.as_slice() else {
                panic!("expected independent artifact error: {failure:?}");
            };
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            assert!(
                error.to_string().contains(artifacts.to_str().unwrap()),
                "{error}"
            );
            assert_eq!(
                std::fs::read(artifacts.join("tmp/retained")).unwrap(),
                b"pre-fork evidence"
            );
            // Recovery is test-only, after proving the failed removal and path
            // diagnostic. Production reports the retained, possibly partial tree.
            std::fs::set_permissions(
                artifacts.join("tmp"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            std::fs::remove_dir_all(artifacts).unwrap();
            assert!(!artifacts.exists());
        }
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserve");
        assert_eq!(std::env::var_os("TMPDIR"), parent_tmpdir);
        assert_no_children();
        return;
    }
    if scenario.starts_with("partial-") {
        let fd_directory = if cfg!(target_os = "linux") {
            "/proc/self/fd"
        } else {
            "/dev/fd"
        };
        let descriptor_count = || std::fs::read_dir(fd_directory).unwrap().count();
        let original_descriptors = descriptor_count();
        use process::fault::{self, Point};
        let point = match scenario.as_str() {
            "partial-descriptor" => Point::Descriptor,
            "partial-mask" => Point::MaskRestored,
            "partial-status" => Point::LaunchStatus,
            "partial-cancel" => Point::CancelStartup,
            "partial-drop" => Point::DropPartial,
            _ => unreachable!(),
        };
        fault::set(point);
        let launch = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            process::OwnedChromium::launch(&exe, &args, start + Duration::from_secs(2), &cancel)
        }));
        fault::assert_consumed();
        if scenario == "partial-drop" {
            assert!(launch.is_err());
        } else {
            let failure = launch.unwrap().err().expect("injected startup failure");
            if scenario == "partial-cancel" {
                assert!(matches!(failure.primary, Some(Error::Cancelled)));
            } else {
                assert!(matches!(failure.primary, Some(Error::Launch { .. })));
            }
            assert!(failure.cleanup.is_empty(), "{failure:?}");
        }
        assert!(
            start.elapsed() < Duration::from_secs(6),
            "silent stderr blocked cleanup"
        );
        assert_no_children();
        assert_eq!(
            descriptor_count(),
            original_descriptors,
            "partial launch leaked a descriptor"
        );
        return;
    }
    let result =
        process::OwnedChromium::launch(&exe, &args, start + Duration::from_secs(2), &cancel);
    if scenario == "temp-path-too-long" {
        let failure = result
            .err()
            .expect("excessive socket path must fail before fork");
        assert!(
            matches!(failure.primary, Some(Error::Configuration(ref message)) if message.contains("singleton socket pathname"))
        );
        assert!(failure.cleanup.is_empty());
        assert_eq!(std::fs::read_dir(std::env::temp_dir()).unwrap().count(), 0);
        assert_eq!(std::env::var_os("TMPDIR"), parent_tmpdir);
        assert_no_children();
        return;
    }
    if scenario == "exec-error" {
        let failure = result.err().expect("exec must fail");
        assert!(matches!(failure.primary, Some(Error::Launch { .. })));
        assert!(failure.cleanup.is_empty(), "{failure:?}");
        return;
    }
    let mut child = match result {
        Ok(child) => child,
        Err(failure) if scenario == "early-exit" => {
            assert!(failure.cleanup.is_empty(), "{failure:?}");
            return;
        }
        Err(failure) => panic!("{failure:?}"),
    };
    let artifacts = child.artifact_path().to_path_buf();
    if scenario == "startup-exit-events" {
        cdp::Cdp::new(&mut child, &cancel)
            .call(
                "Browser.getVersion",
                json!({}),
                Instant::now() + Duration::from_secs(2),
                Phase::Startup,
            )
            .unwrap();
        let cases = [
            (
                "attached-detach",
                json!({"method":"Inspector.detached","sessionId":"fixture-session","params":{"reason":"target_closed"}}),
                "exit",
            ),
            (
                "attached-crash",
                json!({"method":"Inspector.targetCrashed","sessionId":"fixture-session","params":{}}),
                "exit",
            ),
            (
                "browser-target-crash",
                json!({"method":"Target.targetCrashed","params":{"targetId":"fixture-target","status":"crashed","errorCode":1}}),
                "exit",
            ),
            (
                "malformed-target-crash",
                json!({"method":"Target.targetCrashed","params":{}}),
                "malformed",
            ),
            (
                "unrelated-target-crash",
                json!({"method":"Target.targetCrashed","params":{"targetId":"other-target","status":"crashed","errorCode":1}}),
                "not-exit",
            ),
            (
                "foreign-session-crash",
                json!({"method":"Inspector.targetCrashed","sessionId":"other-session","params":{}}),
                "not-exit",
            ),
            (
                "malformed-event-envelope",
                json!({"method":"Inspector.targetCrashed","sessionId":"fixture-session","params":null}),
                "malformed",
            ),
            (
                "ordinary-lifecycle-event",
                json!({"method":"Page.lifecycleEvent","sessionId":"fixture-session","params":{"frameId":"frame","loaderId":"loader","name":"init","timestamp":1}}),
                "ok",
            ),
        ];
        let mut violations = vec![];
        for (label, event, expected) in cases {
            let mut cdp = cdp::Cdp::new(&mut child, &cancel);
            let result = cdp.startup_exit_event_regression(event);
            let correct = match expected {
                "exit" => matches!(result, Err(Error::BrowserExited)),
                "not-exit" => result.as_ref().is_ok_and(|value| *value == json!({})),
                "malformed" => matches!(
                    result,
                    Err(Error::Protocol(error::ProtocolError::Malformed(_)))
                ),
                "ok" => result.is_ok(),
                _ => unreachable!(),
            };
            eprintln!(
                "controlled exit event {label}: expected={expected}; result={result:?}; {}",
                cdp.failure_diagnostic()
            );
            let diagnostic = cdp.failure_diagnostic();
            let expected_source = match label {
                "attached-detach" => {
                    "exit_source=Inspector.detached session=Capture reason=\"target_closed\""
                }
                "attached-crash" => "exit_source=Inspector.targetCrashed session=Capture",
                "browser-target-crash" => {
                    "exit_source=Target.targetCrashed session=Absent target_matches=true status=\"crashed\" error_code=1"
                }
                _ => "exit_source=none",
            };
            assert!(diagnostic.contains(expected_source), "{diagnostic}");
            assert!(diagnostic.len() < 4096);
            if !correct {
                violations.push(label);
            }
        }
        for scenario in [
            "non-live-at-inspector-enable",
            "pending-page-response",
            "after-page-response",
            "normal",
            "foreign",
        ] {
            cdp::Cdp::new(&mut child, &cancel).startup_crash_diagnostic_regression(scenario);
        }
        let cleanup = child.finish(Duration::from_secs(2));
        eprintln!(
            "controlled exit events cleanup: {cleanup:?}; {}; artifacts_exist={}",
            child.cleanup_diagnostic(),
            artifacts.exists()
        );
        assert!(cleanup.is_empty(), "{cleanup:?}");
        assert!(
            child
                .cleanup_diagnostic()
                .contains("owned_terminated: true")
        );
        assert!(child.cleanup_diagnostic().contains("root_reaped=true"));
        assert!(!artifacts.exists());
        assert_no_children();
        assert!(
            violations.is_empty(),
            "exit-event classification violations: {violations:?}"
        );
        return;
    }
    if scenario.starts_with("diagnostic-") {
        cleanup_diagnostic_case(child, &scenario);
        return;
    }
    if scenario.starts_with("temp-") {
        let sibling = tempfile::tempdir().unwrap();
        let sentinel = sibling.path().join("unrelated");
        std::fs::write(&sentinel, b"preserve").unwrap();
        let response = cdp::Cdp::new(&mut child, &cancel)
            .call(
                "Browser.getVersion",
                json!({}),
                Instant::now() + Duration::from_secs(2),
                Phase::Startup,
            )
            .unwrap();
        let temporary = artifacts.canonicalize().unwrap().join("tmp");
        assert_eq!(response["tmpdir"], temporary.to_str().unwrap());
        assert_eq!(response["tmpdir_entries"], 1);
        assert_eq!(std::env::var_os("TMPDIR"), parent_tmpdir);
        let created = temporary.join("child/nested/evidence.txt");
        assert_eq!(std::fs::read(&created).unwrap(), b"child temporary data");
        assert_eq!(
            std::fs::read(temporary.join("child/descendant.txt")).unwrap(),
            b"inherited"
        );
        assert!(
            std::fs::symlink_metadata(temporary.join("child/nested/link"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        if scenario == "temp-cancel" {
            cancel.cancel();
            assert!(matches!(
                child.test_discover(Instant::now() + Duration::from_secs(1), &cancel),
                Err(Error::Cancelled)
            ));
        } else if scenario == "temp-exit" {
            child.kill_root().unwrap();
        }
        if scenario == "temp-incomplete" {
            process::fault::set(process::fault::Point::Discovery);
            let errors = child.finish(Duration::from_millis(100));
            process::fault::assert_consumed();
            assert!(
                errors
                    .iter()
                    .any(|e| matches!(e, error::CleanupError::Timeout)),
                "{errors:?}"
            );
            assert!(
                created.exists(),
                "incomplete verification removed child artifacts"
            );
            assert!(!child.finish(Duration::from_secs(1)).is_empty());
            assert!(child.recover_test_children().is_empty());
            // Test-only recovery occurs after the deliberately failed cleanup;
            // production retains the entire tree and reports the failure.
            std::fs::remove_dir_all(&artifacts).unwrap();
        } else if scenario == "temp-remove-failure" {
            use std::os::unix::fs::PermissionsExt;
            let nested = created.parent().unwrap();
            std::fs::set_permissions(nested, std::fs::Permissions::from_mode(0o500)).unwrap();
            let errors = child.finish(Duration::from_secs(2));
            assert!(
                errors
                    .iter()
                    .any(|e| matches!(e, error::CleanupError::Artifacts(_))),
                "{errors:?}"
            );
            assert!(created.exists());
            assert!(!child.finish(Duration::from_secs(1)).is_empty());
            assert_no_children();
            std::fs::set_permissions(nested, std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::remove_dir_all(&artifacts).unwrap();
        } else {
            let errors = child.finish(Duration::from_secs(2));
            assert!(errors.is_empty(), "{errors:?}");
        }
        assert!(!artifacts.exists());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserve");
        assert_eq!(std::env::var_os("TMPDIR"), parent_tmpdir);
        assert_no_children();
        return;
    }
    if scenario.starts_with("resource-") {
        let result = cdp::Cdp::new(&mut child, &cancel).resource_regression(&scenario);
        let errors = child.finish(Duration::from_secs(2));
        assert!(errors.is_empty(), "{errors:?}");
        let diagnostic = child.cleanup_diagnostic();
        assert!(
            diagnostic.contains("owned_terminated: true"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("root_reaped=true"), "{diagnostic}");
        assert!(
            diagnostic.contains("artifacts: \"removed\""),
            "{diagnostic}"
        );
        eprintln!("AG7 controlled {scenario} cleanup: {diagnostic}");
        assert!(!artifacts.exists());
        assert_no_children();

        match scenario.as_str() {
            "resource-shutdown-valid"
            | "resource-shutdown-clean"
            | "resource-shutdown-root-clean"
            | "resource-shutdown-delayed-valid" => result.unwrap(),
            "resource-shutdown-buffered-deadline"
            | "resource-shutdown-partial-timeout"
            | "resource-shutdown-no-response"
            | "resource-shutdown-ack-only"
            | "resource-shutdown-capture-budget"
            | "resource-shutdown-delayed-never" => {
                assert!(
                    matches!(result, Err(Error::Timeout(Phase::Shutdown))),
                    "{result:?}"
                );
            }
            "resource-shutdown-cancel-after-ack" => {
                assert!(matches!(result, Err(Error::Cancelled)), "{result:?}")
            }
            "resource-shutdown-delayed-malformed" => assert!(
                matches!(
                    result,
                    Err(Error::Protocol(error::ProtocolError::Malformed(_)))
                ),
                "{result:?}"
            ),
            "resource-shutdown-truncated" | "resource-shutdown-delayed-truncated" => assert!(
                matches!(
                    result,
                    Err(Error::Protocol(error::ProtocolError::Truncated))
                ),
                "{result:?}"
            ),
            "resource-rejected-abort" => assert!(
                matches!(result, Err(Error::Protocol(error::ProtocolError::Remote { ref method, .. })) if method == "Fetch.failRequest"),
                "{result:?}"
            ),
            "resource-missing-ack" | "resource-missing-terminal" => assert!(
                matches!(result, Err(Error::Timeout(Phase::Capture))),
                "{result:?}"
            ),
            "resource-shutdown-incomplete"
            | "resource-shutdown-after-ack"
            | "resource-shutdown-invalidated"
            | "resource-shutdown-invalidated-after-ack"
            | "resource-shutdown-root-invalidated"
            | "resource-shutdown-delayed-policy" => {
                assert!(matches!(result, Err(Error::Navigation(_))), "{result:?}")
            }
            _ => unreachable!(),
        }
        return;
    }
    if scenario.starts_with("capture-") {
        let mut cdp = cdp::Cdp::new(&mut child, &cancel);
        cdp.call(
            "DOM.getDocument",
            json!({}),
            Instant::now() + Duration::from_secs(2),
            Phase::Capture,
        )
        .unwrap();
        if scenario == "capture-exited" {
            cdp.child.kill_root().unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while cdp.child.failure_observation().0 != process::RootObservation::Exited {
                assert!(Instant::now() < deadline, "killed helper did not exit");
                std::thread::yield_now();
            }
        }
        if scenario == "capture-discovery-timeout" {
            process::fault::set(process::fault::Point::Discovery);
        }
        let params = if scenario == "capture-send-timeout" {
            json!({"not_a_real_screenshot": "PRIVATE_PAYLOAD".repeat(65536)})
        } else {
            json!({})
        };
        let error = cdp
            .call(
                "Page.captureScreenshot",
                params,
                Instant::now() + Duration::from_millis(300),
                Phase::Capture,
            )
            .unwrap_err();
        let diagnostic = cdp.failure_diagnostic();
        assert!(
            diagnostic.contains("operation: \"Page.captureScreenshot\""),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("command_id: Some(2)"), "{diagnostic}");
        assert!(
            diagnostic.contains("completed: [\"DOM.getDocument\"]"),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("PRIVATE_PAYLOAD"));
        assert!(diagnostic.len() < 4096);
        match scenario.as_str() {
            "capture-exited" => {
                assert!(matches!(error, Error::BrowserExited));
                assert!(
                    diagnostic.contains("exit_source=native root termination"),
                    "{diagnostic}"
                );
                assert!(
                    diagnostic.contains("root_at_failure=Exited"),
                    "{diagnostic}"
                );
            }
            "capture-disconnected" => {
                assert!(matches!(error, Error::Protocol(error::ProtocolError::Eof)));
                assert!(
                    diagnostic.contains("response_pipe_at_failure=HangupOrError"),
                    "{diagnostic}"
                );
                assert!(
                    diagnostic.contains("root_at_failure=NoExitObserved"),
                    "{diagnostic}"
                );
            }
            _ => {
                assert!(matches!(error, Error::Timeout(Phase::Capture)));
                assert!(
                    diagnostic.contains("root_at_failure=NoExitObserved"),
                    "{diagnostic}"
                );
                assert!(
                    diagnostic.contains("response_pipe_at_failure=OpenEmpty"),
                    "{diagnostic}"
                );
                if scenario == "capture-discovery-timeout" {
                    process::fault::assert_consumed();
                    assert!(
                        diagnostic.contains("native_check=OwnershipDiscovery"),
                        "{diagnostic}"
                    );
                } else {
                    let activity = if scenario == "capture-send-timeout" {
                        "waiting for writable command pipe"
                    } else {
                        "waiting for response"
                    };
                    assert!(diagnostic.contains(activity), "{diagnostic}");
                }
            }
        }
        drop(cdp);
        let cleanup = child.finish(Duration::from_secs(2));
        assert!(cleanup.is_empty(), "{cleanup:?}");
        let cleanup_diagnostic = child.cleanup_diagnostic();
        assert!(cleanup_diagnostic.contains("owned_terminated: true"));
        assert!(cleanup_diagnostic.contains("root_reaped=true"));
        if scenario == "capture-exited" {
            eprintln!(
                "controlled native root exit: {diagnostic}; cleanup={cleanup:?}; {cleanup_diagnostic}"
            );
        }
        assert!(!artifacts.exists());
        assert_no_children();
        return;
    }
    if scenario == "discovery-runtime"
        || scenario.starts_with("cleanup-")
        || scenario.starts_with("arguments-")
        || scenario == "adopted-reap"
    {
        // First establish the helper's actual pipe and retained native identity.
        cdp::Cdp::new(&mut child, &cancel)
            .call(
                "Browser.getVersion",
                json!({}),
                Instant::now() + Duration::from_secs(2),
                Phase::Startup,
            )
            .unwrap();
        use process::fault::{self, Point};
        let point = match scenario.as_str() {
            "discovery-runtime" | "cleanup-discovery" => Point::Discovery,
            "cleanup-verification" => Point::Verification,
            "cleanup-reap" => Point::Reap,
            #[cfg(target_os = "macos")]
            "arguments-transient" => Point::ArgumentsUnavailable,
            #[cfg(target_os = "macos")]
            "arguments-persistent" => Point::ArgumentsUnavailablePersistent,
            #[cfg(target_os = "linux")]
            "adopted-reap" => Point::AdoptedReap,
            _ => unreachable!(),
        };
        #[cfg(target_os = "macos")]
        if scenario == "arguments-persistent" {
            fault::reset_persistent_argument_eio_activations();
        }
        fault::set(point);
        if scenario == "cleanup-reap" {
            eprintln!("cleanup-reap: armed after helper startup");
        }
        if scenario == "arguments-transient" {
            // An argument-memory copy failure leaves discovery incomplete;
            // a fresh full scan must recover before cleanup can succeed.
            child
                .test_discover(Instant::now() + Duration::from_secs(1), &cancel)
                .unwrap();
            fault::assert_consumed();
            assert!(child.finish(Duration::from_secs(2)).is_empty());
            assert!(!artifacts.exists());
        } else if scenario == "discovery-runtime" {
            // Force the next ownership check, independent of its scan throttle.
            let error = child
                .test_discover(Instant::now() + Duration::from_millis(100), &cancel)
                .unwrap_err();
            fault::assert_consumed();
            assert!(matches!(error, Error::Timeout(Phase::Capture)));
            assert!(child.finish(Duration::from_secs(2)).is_empty());
            assert!(!artifacts.exists());
        } else {
            #[cfg(target_os = "macos")]
            let previous_activations = if scenario == "arguments-persistent" {
                // Force one scan, then require another activation during
                // cleanup itself. No timing-dependent minimum retry count.
                child
                    .test_discover(Instant::now() + Duration::from_secs(1), &cancel)
                    .unwrap();
                fault::assert_persistent_argument_eio_since(0)
            } else {
                0
            };
            let deadline_start = Instant::now();
            let errors = child.finish(Duration::from_secs(1));
            #[cfg(target_os = "macos")]
            if scenario == "arguments-persistent" {
                fault::assert_persistent_argument_eio_since(previous_activations);
                assert!(fault::take(Point::ArgumentsUnavailablePersistent));
            }
            fault::assert_consumed();
            if scenario == "cleanup-reap" {
                eprintln!("cleanup-reap: injected fault consumed");
            }
            assert!(
                errors
                    .iter()
                    .any(|e| matches!(e, error::CleanupError::Timeout)),
                "{errors:?}"
            );
            assert!(deadline_start.elapsed() < Duration::from_secs(2));
            assert!(artifacts.exists(), "unverified cleanup discarded artifacts");
            assert!(
                !child.finish(Duration::from_secs(1)).is_empty(),
                "failed cleanup became success on a second call"
            );
            // The test must clean up its deliberately interrupted fixture with
            // retained ownership; production never retries using unchecked PIDs.
            assert!(child.recover_test_children().is_empty());
            std::fs::remove_dir_all(&artifacts).unwrap();
        }
        assert_no_children();
        return;
    }
    if matches!(
        scenario.as_str(),
        "eof"
            | "malformed"
            | "oversized"
            | "hang"
            | "cancel"
            | "truncated"
            | "wrong-id"
            | "wrong-session"
    ) {
        if scenario == "cancel" {
            cancel.cancel();
        }
        let mut cdp = cdp::Cdp::new(&mut child, &cancel);
        let budget = if scenario == "oversized" {
            Duration::from_secs(3)
        } else {
            Duration::from_millis(300)
        };
        let error = cdp
            .call(
                "Browser.getVersion",
                json!({}),
                Instant::now() + budget,
                Phase::Startup,
            )
            .unwrap_err();
        match scenario.as_str() {
            "cancel" => assert!(matches!(error, Error::Cancelled)),
            "hang" => assert!(matches!(error, Error::Timeout(Phase::Startup))),
            "eof" => assert!(matches!(error, Error::Protocol(error::ProtocolError::Eof))),
            "malformed" => assert!(matches!(
                error,
                Error::Protocol(error::ProtocolError::Malformed(_))
            )),
            "oversized" => assert!(matches!(
                error,
                Error::Protocol(error::ProtocolError::Oversized)
            )),
            "truncated" => assert!(matches!(
                error,
                Error::Protocol(error::ProtocolError::Truncated)
            )),
            "wrong-id" | "wrong-session" => assert!(matches!(
                error,
                Error::Protocol(error::ProtocolError::UnexpectedResponse)
            )),
            _ => unreachable!(),
        }
    } else {
        // Readiness is the helper's pipe record, not a scheduling sleep.
        let mut cdp = cdp::Cdp::new(&mut child, &cancel);
        let result = cdp.call(
            "Browser.getVersion",
            json!({}),
            Instant::now() + Duration::from_secs(2),
            Phase::Startup,
        );
        if scenario != "early-exit" {
            assert!(result.is_ok(), "{result:?}");
        }
    }
    if scenario == "drop" {
        drop(child);
        assert!(
            !artifacts.exists(),
            "Drop cleanup did not verify termination/remove artifacts"
        );
        return;
    }
    let errors = child.finish(Duration::from_secs(2));
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!artifacts.exists());
    assert!(start.elapsed() < Duration::from_secs(6));
}

// Called only through native_case in its isolated process. stderr is redirected
// after startup and restored before assertions or libtest's result reporting.
fn cleanup_diagnostic_case(mut child: OwnedChromium, scenario: &str) {
    use process::fault::{self, CleanupEvent, Point};
    use std::os::fd::{AsRawFd, OwnedFd};
    let cancel = Cancellation::default();
    cdp::Cdp::new(&mut child, &cancel)
        .call(
            "Browser.getVersion",
            json!({}),
            Instant::now() + Duration::from_secs(2),
            Phase::Startup,
        )
        .unwrap();
    let artifacts = child.artifact_path().to_owned();
    let evidence = artifacts.join("retained-evidence");
    std::fs::write(&evidence, b"retain even if stderr fails").unwrap();
    let unrelated = tempfile::tempdir().unwrap();
    let sentinel = unrelated.path().join("unrelated");
    std::fs::write(&sentinel, b"preserve").unwrap();

    let closed = scenario.ends_with("-closed");
    let saved_stderr = if closed {
        // This process has no other test. Never alter the outer runner's FDs,
        // signal dispositions or panic hook. The only pipe reader closes first.
        let saved = unsafe { libc::fcntl(2, libc::F_DUPFD_CLOEXEC, 5) };
        assert!(saved >= 0);
        let saved = unsafe { OwnedFd::from_raw_fd(saved) };
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        assert_eq!(unsafe { libc::dup2(writer.as_raw_fd(), 2) }, 2);
        Some(saved)
    } else {
        None
    };
    fault::begin_cleanup_trace();
    // Expire only after real termination and reaping. Unlike a dir.close()
    // failure, this leaves directory ownership for the retained-path branch.
    fault::set(Point::AfterReap);
    let finishing = scenario.starts_with("diagnostic-finish-");
    let unwinding = scenario.starts_with("diagnostic-unwind-");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        if finishing {
            let errors = child.finish(Duration::from_secs(2));
            drop(child);
            Some(errors)
        } else if unwinding {
            let _owned = child;
            // Test an already-existing unwind without invoking a diagnostic
            // panic hook first and masking the production Drop branch.
            std::panic::resume_unwind(Box::new("original native test unwind"));
        } else {
            drop(child);
            None
        }
    }));
    if let Some(saved) = saved_stderr {
        assert_eq!(unsafe { libc::dup2(saved.as_raw_fd(), 2) }, 2);
    }
    if finishing {
        assert_cleanup_failure_diagnostic(scenario, result.as_ref().unwrap().as_ref().unwrap());
    }
    fault::assert_consumed();
    let mut expected = vec![
        CleanupEvent::Started,
        CleanupEvent::RetainedDiagnostic { failed: closed },
        CleanupEvent::Finished,
    ];
    if !finishing {
        expected.push(CleanupEvent::EmergencyDiagnostic { failed: closed });
    }
    assert_eq!(fault::take_cleanup_trace(), expected);
    if unwinding {
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"original native test unwind")
        );
    } else if finishing {
        let errors = result.unwrap().unwrap();
        assert!(
            matches!(errors.as_slice(), [error::CleanupError::Timeout]),
            "{errors:?}"
        );
    } else {
        assert!(result.unwrap().is_none());
    }
    assert_eq!(
        std::fs::read(&evidence).unwrap(),
        b"retain even if stderr fails"
    );
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserve");
    // After proving errors, retained evidence and a single finished attempt,
    // recover only this recorded directory. Native reaping preceded the fault.
    assert_no_children();
    std::fs::remove_dir_all(&artifacts).unwrap();
    assert!(!artifacts.exists());
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserve");
}

// Shared by the real lifecycle assertion and its negative diagnostic regression.
// It receives only already-completed errors, so reporting cannot attempt cleanup.
fn assert_lifecycle_cleanup(scenario: &str, errors: &[error::CleanupError]) {
    assert!(errors.is_empty(), "scenario={scenario}: cleanup={errors:?}");
}

fn assert_cleanup_failure_diagnostic(scenario: &str, errors: &[error::CleanupError]) {
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_lifecycle_cleanup(scenario, errors);
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<String>().unwrap(),
        &format!("scenario={scenario}: cleanup={errors:?}")
    );
}

#[test]
fn lifecycle_cleanup_diagnostic_retains_scenario_and_entire_error_vector() {
    let errors = vec![
        error::CleanupError::Identity("discover read /proc/42/stat: os error 13".into()),
        error::CleanupError::Reap("waitpid: os error 10".into()),
        error::CleanupError::Timeout,
    ];
    for scenario in ["cancel", "timeout", "exit"] {
        assert_cleanup_failure_diagnostic(scenario, &errors);
        assert_lifecycle_cleanup(scenario, &[]);
    }
}

#[test]
fn cleanup_reap_assertion_rejects_an_unreached_fault() {
    use process::fault::{self, Point};
    fault::set(Point::Reap);
    assert!(std::panic::catch_unwind(fault::assert_consumed).is_err());
    assert!(fault::take(Point::Reap));
    fault::assert_consumed();
}

fn assert_no_children() {
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
#[ignore = "private process fixture; only native_case invokes this"]
fn browser_helper() {
    use std::io::Read;
    let scenario = std::env::var("AG2_NATIVE_CASE").unwrap();
    if scenario == "temp-descendant" {
        std::fs::write(
            std::env::temp_dir().join("child/descendant.txt"),
            b"inherited",
        )
        .unwrap();
        return;
    }
    if scenario == "unrelated" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        return;
    }
    #[cfg(target_os = "macos")]
    if scenario == "bsd-ownership" {
        use std::io::Write;
        use std::os::unix::process::CommandExt;
        let mut input = unsafe { std::fs::File::from_raw_fd(3) };
        let mut output = unsafe { std::fs::File::from_raw_fd(4) };
        let after_exec = std::env::var_os("AG2_BSD_EXEC_ACK").is_some();
        output
            .write_all(if after_exec { b"E" } else { b"R" })
            .unwrap();
        let mut command = [0];
        match input.read(&mut command).unwrap() {
            0 => return, // ordinary owned cleanup closed the command writer
            1 if command[0] == b'Q' => std::process::exit(0),
            1 if command[0] == b'X' && !after_exec => {
                let error = Command::new(std::env::current_exe().unwrap())
                    .args(std::env::args_os().skip(1))
                    .env("AG2_BSD_EXEC_ACK", "1")
                    .exec();
                panic!("controlled exec failed: {error}");
            }
            _ => panic!("unexpected ownership probe command: {command:?}"),
        }
    }
    for (fd, direction) in [(3, libc::O_RDONLY), (4, libc::O_WRONLY)] {
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_ACCMODE,
            direction
        );
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
    }
    unsafe {
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
    }
    if scenario.starts_with("partial-") {
        // Keep stderr open and silent, including after the command pipe closes.
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
    if scenario == "detached" || scenario == "early-exit" || scenario == "adopted-reap" {
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            unsafe {
                libc::setsid();
                libc::close(3);
                libc::close(4);
            }
            loop {
                std::thread::park_timeout(Duration::from_secs(1));
            }
        }
        if scenario == "early-exit" {
            std::process::exit(0);
        }
    }
    let mut read = unsafe { std::fs::File::from_raw_fd(3) };
    let mut request = Vec::new();
    loop {
        let mut byte = [0];
        read.read_exact(&mut byte).unwrap();
        if byte[0] == 0 {
            break;
        }
        request.push(byte[0]);
    }
    let mut write = unsafe { std::fs::File::from_raw_fd(4) };
    if scenario == "startup-comparison-failure" {
        let pin: Value =
            serde_json::from_str(include_str!("../../chromium-reference.json")).unwrap();
        for method in [
            "Browser.getVersion",
            "Target.createTarget",
            "Target.setDiscoverTargets",
            "Target.attachToTarget",
            "Page.enable",
        ] {
            let command: Value = serde_json::from_slice(&request).unwrap();
            assert_eq!(command["method"], method);
            if method == "Page.enable" {
                assert_eq!(command["sessionId"], "fixture-session");
                std::io::stderr()
                    .write_all(b"controlled startup stderr\n")
                    .unwrap();
                write.write_all(format!("{}\0", json!({"method":"Inspector.targetCrashed","sessionId":"fixture-session","params":{}})).as_bytes()).unwrap();
                // Remain waitable for ordinary owned cleanup; no forced exit
                // supplies the capture failure or a successful capture boundary.
                let mut remaining = vec![];
                read.read_to_end(&mut remaining).unwrap();
                return;
            }
            let result = match method {
                "Browser.getVersion" => {
                    json!({"product":format!("Chrome/{}",pin["version"].as_str().unwrap()),"revision":pin["revision"],"protocolVersion":pin["protocol_version"]})
                }
                "Target.createTarget" => json!({"targetId":"fixture-target"}),
                "Target.attachToTarget" => json!({"sessionId":"fixture-session"}),
                _ => json!({}),
            };
            write
                .write_all(format!("{}\0", json!({"id":command["id"],"result":result})).as_bytes())
                .unwrap();
            request.clear();
            loop {
                let mut byte = [0];
                read.read_exact(&mut byte).unwrap();
                if byte[0] == 0 {
                    break;
                }
                request.push(byte[0]);
            }
        }
        unreachable!();
    }
    if scenario.starts_with("temp-") {
        let temporary = std::env::temp_dir();
        let crashes = PathBuf::from(std::env::var_os("BREAKPAD_DUMP_LOCATION").unwrap());
        assert_eq!(temporary, crashes.parent().unwrap().join("tmp"));
        let nested = temporary.join("child/nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("evidence.txt"), b"child temporary data").unwrap();
        std::os::unix::fs::symlink("evidence.txt", nested.join("link")).unwrap();
        assert!(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "chromium::tests::browser_helper", "--ignored"])
                .env("AG2_NATIVE_CASE", "temp-descendant")
                .status()
                .unwrap()
                .success()
        );
        let value: Value = serde_json::from_slice(&request).unwrap();
        let entries = std::env::vars_os()
            .filter(|(key, _)| key == "TMPDIR")
            .count();
        write
            .write_all(
                format!(
                    "{}\0",
                    json!({"id":value["id"],"result":{"tmpdir":temporary,"tmpdir_entries":entries}})
                )
                .as_bytes(),
            )
            .unwrap();
        // Graceful command EOF, cancellation, and forced root exit all leave
        // artifact removal to the owner's post-verification cleanup.
        let mut byte = [0];
        assert_eq!(read.read(&mut byte).unwrap(), 0);
        return;
    }
    if scenario.starts_with("resource-") {
        resource_protocol_helper(&scenario, &request, &mut read, &mut write);
        drop(write);
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
    match scenario.as_str() {
        "eof" => drop(write),
        "malformed" => {
            write.write_all(b"garbage\0").unwrap();
        }
        "truncated" => {
            write.write_all(b"{\"id\":").unwrap();
            drop(write);
        }
        "wrong-id" => {
            write.write_all(b"{\"id\":999,\"result\":{}}\0").unwrap();
        }
        "wrong-session" => {
            write
                .write_all(b"{\"id\":1,\"sessionId\":\"other\",\"result\":{}}\0")
                .unwrap();
        }
        "oversized" => {
            let _ = write.write_all(&vec![b'x'; 4 * 1024 * 1024]);
        }
        "hang" | "cancel" => {}
        _ => {
            let value: Value = serde_json::from_slice(&request).unwrap();
            let response = format!("{}\0", json!({"id":value["id"],"result":{}}));
            let chunk = if scenario == "fragmented" {
                1
            } else {
                response.len()
            };
            for part in response.as_bytes().chunks(chunk) {
                write.write_all(part).unwrap();
            }
            if scenario == "capture-disconnected" {
                // Close after the second command was delivered, so the failure
                // unambiguously belongs to that capture command.
                loop {
                    let mut byte = [0];
                    read.read_exact(&mut byte).unwrap();
                    if byte[0] == 0 {
                        break;
                    }
                }
                drop(write);
            }
        }
    }
    loop {
        std::thread::park_timeout(Duration::from_secs(1));
    }
}

// Fixed scripted responses exercise the real pipe, dispatcher and shutdown path.
// This helper neither inspects nor authorizes process identities.
fn resource_protocol_helper(
    scenario: &str,
    request: &[u8],
    read: &mut std::fs::File,
    write: &mut std::fs::File,
) {
    use std::io::Read;
    fn emit(write: &mut std::fs::File, value: Value) {
        write
            .write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
        write.write_all(&[0]).unwrap();
    }
    fn event(write: &mut std::fs::File, method: &str, params: Value) {
        emit(
            write,
            json!({"sessionId":"fixture-session","method":method,"params":params}),
        );
    }
    let command: Value = serde_json::from_slice(request).unwrap();
    if matches!(
        scenario,
        "resource-shutdown-root-invalidated"
            | "resource-shutdown-root-clean"
            | "resource-shutdown-buffered-deadline"
    ) {
        assert_eq!(command["method"], "Page.getLayoutMetrics");
        emit(
            write,
            json!({"id":command["id"],"sessionId":"fixture-session","result":{}}),
        );
        if scenario != "resource-shutdown-root-clean" {
            event(write, "DOM.documentUpdated", json!({}));
        }
        return;
    }
    let shutdown = scenario.starts_with("resource-shutdown-");
    assert_eq!(
        command["method"],
        if shutdown {
            "Browser.close"
        } else {
            "Page.getLayoutMetrics"
        }
    );
    if shutdown {
        assert!(command.get("sessionId").is_none());
    }
    let mut response = json!({"id":command["id"],"result":{}});
    if !shutdown {
        response["sessionId"] = json!("fixture-session");
    }
    if scenario.starts_with("resource-shutdown-delayed-") {
        use std::os::fd::AsRawFd;
        emit(write, response);
        // A deliberate protocol-completion boundary counterexample, not a
        // readiness sleep. Keep both directions open for longer than the former
        // 250 ms subdeadline. Command EOF means the owner already failed and
        // entered cleanup; return without manufacturing late success evidence.
        let release = Instant::now() + Duration::from_millis(300);
        loop {
            let remaining = release.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let mut fd = libc::pollfd {
                fd: read.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut fd, 1, remaining.as_millis().max(1) as i32) };
            if rc > 0 {
                return;
            }
            assert!(
                rc == 0
                    || std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            );
        }
        match scenario {
            "resource-shutdown-delayed-valid" => event(
                write,
                "Inspector.detached",
                json!({"reason":"target_closed"}),
            ),
            "resource-shutdown-delayed-never" => {
                event(
                    write,
                    "Inspector.detached",
                    json!({"reason":"target_closed"}),
                );
                let mut byte = [0];
                assert_eq!(read.read(&mut byte).unwrap(), 0);
            }
            "resource-shutdown-delayed-malformed" => write.write_all(b"invalid-json\0").unwrap(),
            "resource-shutdown-delayed-truncated" => write.write_all(b"{\"method\":").unwrap(),
            "resource-shutdown-delayed-policy" => event(write, "DOM.documentUpdated", json!({})),
            _ => unreachable!(),
        }
        return;
    }
    if matches!(
        scenario,
        "resource-shutdown-no-response"
            | "resource-shutdown-ack-only"
            | "resource-shutdown-capture-budget"
            | "resource-shutdown-cancel-after-ack"
    ) {
        if scenario != "resource-shutdown-no-response" {
            emit(write, response);
        }
        // Controlled pipe synchronization: finish() closes the command pipe.
        // No readiness sleep, delayed retry or independent timer in the helper.
        let mut byte = [0];
        assert_eq!(read.read(&mut byte).unwrap(), 0);
        return;
    }
    if matches!(
        scenario,
        "resource-shutdown-partial-timeout" | "resource-shutdown-truncated"
    ) {
        emit(write, response);
        // No NUL delimiter: neither an acknowledgment nor previously complete
        // navigation evidence may excuse this unfinished incoming frame.
        write
            .write_all(
                br#"{"sessionId":"fixture-session","method":"DOM.documentUpdated","params":{}}"#,
            )
            .unwrap();
        if scenario == "resource-shutdown-partial-timeout" {
            loop {
                std::thread::park_timeout(Duration::from_secs(1));
            }
        }
        return;
    }
    if scenario == "resource-shutdown-clean" {
        emit(write, response);
        return;
    }
    if scenario == "resource-shutdown-after-ack" {
        emit(write, response);
        event(
            write,
            "Network.requestWillBeSent",
            json!({"frameId":"f","type":"Image","requestId":"authored","request":{"url":"https://external.invalid/image.png","method":"GET"}}),
        );
        return;
    }
    if matches!(
        scenario,
        "resource-shutdown-invalidated" | "resource-shutdown-invalidated-after-ack"
    ) {
        if scenario == "resource-shutdown-invalidated-after-ack" {
            emit(write, response);
        }
        event(write, "DOM.documentUpdated", json!({}));
        return;
    }
    event(
        write,
        "Network.requestWillBeSent",
        json!({"frameId":"f","type":"Other","requestId":"icon","initiator":{"type":"other"},"request":{"url":"https://borrowser.invalid/favicon.ico","method":"GET"}}),
    );
    event(
        write,
        "Fetch.requestPaused",
        json!({"frameId":"f","resourceType":"Other","requestId":"icon-fetch","networkId":"icon","request":{"url":"https://borrowser.invalid/favicon.ico","method":"GET"}}),
    );
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        read.read_exact(&mut byte).unwrap();
        if byte[0] == 0 {
            break;
        }
        bytes.push(byte[0]);
        assert!(bytes.len() < 1024);
    }
    let abort: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(abort["method"], "Fetch.failRequest");
    assert_eq!(abort["sessionId"], "fixture-session");
    assert_eq!(
        abort["params"],
        json!({"requestId":"icon-fetch","errorReason":"BlockedByClient"})
    );
    if scenario == "resource-rejected-abort" {
        emit(
            write,
            json!({"id":abort["id"],"sessionId":"fixture-session","error":{"code":-32000,"message":"injected abort rejection"}}),
        );
        return;
    }
    if scenario != "resource-missing-ack" {
        emit(
            write,
            json!({"id":abort["id"],"sessionId":"fixture-session","result":{}}),
        );
    }
    if !matches!(
        scenario,
        "resource-missing-terminal" | "resource-shutdown-incomplete"
    ) {
        event(
            write,
            "Network.loadingFailed",
            json!({"requestId":"icon","type":"Other","errorText":"net::ERR_BLOCKED_BY_CLIENT.Inspector"}),
        );
    }
    emit(write, response);
    if !shutdown {
        // Keep the pipe open for the missing-evidence timeout cases.
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
}

// The only script-enabled input: its script changes color, never resource or
// DOM structure. A snapshot cannot authorize arbitrary create/remove-icon scripts.
pub(super) const SCRIPT_POSITIVE_CONTROL: &[u8] = b"<!doctype html><html><head><style>html{background:#123456}body{margin:0}</style><script>document.documentElement.style.backgroundColor='#ff0000'</script></head><body></body></html>";
pub(super) fn require_script_positive_control(html: &[u8]) -> Result<(), Error> {
    if html != SCRIPT_POSITIVE_CONTROL {
        return Err(Error::Configuration(
            "script-enabled test requires the exact color positive control".into(),
        ));
    }
    Ok(())
}
#[test]
fn script_positive_control_rejects_untrusted_input_before_launch() {
    require_script_positive_control(SCRIPT_POSITIVE_CONTROL).unwrap();
    let mut altered = SCRIPT_POSITIVE_CONTROL.to_vec();
    altered.extend_from_slice(b"<script>let i=document.createElement('link');i.rel='icon';i.href='/favicon.ico';document.head.append(i);i.remove()</script>");
    let config = ChromiumConfig {
        scripts_disabled: false,
        ..ChromiumConfig::new(PathBuf::from("/nonexistent"))
    };
    let failure = capture_html(&altered, &config, &Cancellation::default()).unwrap_err();
    assert!(
        matches!(failure.primary, Some(Error::Configuration(ref s)) if s.contains("exact color positive control"))
    );
    assert!(failure.cleanup.is_empty());
}

#[test]
#[ignore = "requires the configured pinned real Chromium; run with --test-threads=1"]
fn real_chromium_capture_and_scripts() {
    let path = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit real Chromium test requires BORROWSER_CHROMIUM_EXECUTABLE");
    let _standalone = process::Standalone::enter().unwrap();
    let config = ChromiumConfig::new(PathBuf::from(path));
    let cancel = Cancellation::default();
    for fixture in &crate::fixtures::baseline()[..2] {
        let html = (fixture).document.html;
        let crate::model::Expected::Canvas(expected) = (fixture).independent().unwrap().expected
        else {
            unreachable!()
        };
        let mut previous = None;
        for iteration in 1..=3 {
            eprintln!(
                "AG2 fixture={} iteration={iteration}/3 starting",
                fixture.id.0
            );
            let result = capture_html(html, &config, &cancel).unwrap_or_else(|failure| {
                panic!(
                    "fixture={} iteration={iteration}/3: {failure:?}",
                    fixture.id.0
                )
            });
            assert_eq!(result.color, Some(expected));
            assert!(
                result.default_favicon_aborted,
                "qualified ordinary target did not exercise default favicon interception"
            );
            assert_eq!(result.identity.capture_profile, "ag7-static-references-v1");
            eprintln!(
                "AG2 fixture={} iteration={iteration}/3 screenshot=640x480 rgb8={:?} profile={} default_favicon=correlated-aborted-acknowledged-terminal cleanup=verified",
                fixture.id.0,
                result.color.unwrap().0,
                result.identity.capture_profile
            );
            let serialized =
                serde_json::to_string(&(result.color.unwrap().0, result.identity)).unwrap();
            if let Some(previous) = previous {
                assert_eq!(serialized, previous);
            }
            previous = Some(serialized);
            eprintln!(
                "AG2 fixture={} iteration={iteration}/3 passed",
                fixture.id.0
            );
        }
    }
    // Markup lookalikes are not live link elements. The script is disabled;
    // comments and raw text are interpreted by Chromium, never scanned as HTML.
    for (index, suffix) in [
        b"<!-- <link rel=icon href=/favicon.ico> -->".as_slice(),
        b"<script>const text='<link rel=icon href=/favicon.ico>';</script>",
    ]
    .into_iter()
    .enumerate()
    {
        let html = (crate::fixtures::fixture("canvas/root")).document.html;
        let crate::model::Expected::Canvas(expected) = (crate::fixtures::fixture("canvas/root"))
            .independent()
            .unwrap()
            .expected
        else {
            unreachable!()
        };
        let mut input = html.to_vec();
        input.extend_from_slice(suffix);
        eprintln!("AG2 fixture=markup-lookalike-{index} iteration=1/1");
        assert_eq!(
            capture_html(&input, &config, &cancel).unwrap().color,
            Some(expected)
        );
    }
    let scripted = SCRIPT_POSITIVE_CONTROL;
    eprintln!("AG2 fixture=script-suppression iteration=1/1");
    assert_eq!(
        capture_html(scripted, &config, &cancel).unwrap().color,
        Some(CanvasColor([18, 52, 86]))
    );
    let enabled = ChromiumConfig {
        scripts_disabled: false,
        ..config
    };
    eprintln!("AG2 fixture=script-positive-control iteration=1/1");
    assert_eq!(
        capture_html(scripted, &enabled, &cancel).unwrap().color,
        Some(CanvasColor([255, 0, 0]))
    );
    let config = ChromiumConfig {
        scripts_disabled: true,
        ..enabled
    };
    for (index, html) in [
        b"<!doctype html><link rel=stylesheet href=https://external.invalid/style.css>".as_slice(),
        b"<!doctype html><img src=https://external.invalid/image.png>",
        b"<!doctype html><meta http-equiv=refresh content='0;url=https://external.invalid/'>",
        b"<!doctype html><meta http-equiv=refresh content='10;url=https://external.invalid/'>",
        b"<!doctype html><iframe src=https://external.invalid/>",
        b"<!doctype html><link rel=icon href=/favicon.ico>",
        b"<!doctype html><LINK REL=ICON HREF=/favicon.ico>",
        b"<!doctype html><link rel='shortcut ICON' href=/favicon.ico>",
        b"<!doctype html><link rel='&#105;con' href=/favicon.ico>",
        b"<!doctype html><link rel='shortcut&#9;IcOn' href=/favicon.ico>",
        b"<!doctype html><body><p><link rel=icon href=/favicon.ico>",
        b"<!doctype html><link rel=canonical href=/inert>",
        b"<!doctype html><link>",
    ]
    .into_iter()
    .enumerate()
    {
        eprintln!("AG2 fixture=unexpected-resource-{index} iteration=1/1");
        let failure = capture_html(html, &config, &cancel).unwrap_err();
        assert!(
            matches!(failure.primary, Some(Error::Navigation(_))),
            "{failure:?}"
        );
        assert!(failure.cleanup.is_empty(), "{failure:?}");
    }
}

#[test]
#[ignore = "requires pinned Chromium and native lifecycle permissions; run with --test-threads=1"]
fn real_chromium_lifecycle_and_topology() {
    require_execution_platform().expect("real Chromium requires a supported execution platform");
    let executable = PathBuf::from(
        std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
            .expect("explicit real test requires Chromium"),
    );
    let _standalone = process::Standalone::enter().unwrap();
    for scenario in ["cancel", "timeout", "exit"] {
        let cancel = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut child = OwnedChromium::launch(&executable, &flags(), deadline, &cancel).unwrap();
        let artifacts = child.artifact_path().to_path_buf();
        {
            let mut cdp = cdp::Cdp::new(&mut child, &cancel);
            identity(
                cdp.call("Browser.getVersion", json!({}), deadline, Phase::Startup)
                    .unwrap(),
            )
            .unwrap();
            let target = cdp
                .call(
                    "Target.createTarget",
                    json!({"url":"about:blank"}),
                    deadline,
                    Phase::Startup,
                )
                .unwrap();
            if cfg!(target_os = "linux") {
                use std::io::Read;
                let attached = cdp
                    .call(
                        "Target.attachToTarget",
                        json!({"targetId":target["targetId"],"flatten":true}),
                        deadline,
                        Phase::Startup,
                    )
                    .unwrap();
                cdp.session = Some(cdp::string(&attached, "sessionId").unwrap().into());
                // A renderer command establishes that the blank target has a
                // live renderer; no sleeps or page JavaScript are needed.
                cdp.call("Page.getLayoutMetrics", json!({}), deadline, Phase::Startup)
                    .unwrap();
                cdp.session = None;
                let processes = cdp
                    .call(
                        "SystemInfo.getProcessInfo",
                        json!({}),
                        deadline,
                        Phase::Startup,
                    )
                    .unwrap();
                let mut renderers = 0;
                for process in processes["processInfo"].as_array().unwrap() {
                    if process["type"] != "renderer" {
                        continue;
                    }
                    cancel.check(deadline, Phase::Startup).unwrap();
                    let pid = sandbox_renderer_pid(process)
                        .expect("CDP renderer ID must be a positive native integer PID");
                    // Read-only diagnostic identities never authorize signals.
                    // The existing native owner remains responsible for cleanup.
                    let mut status = String::new();
                    std::fs::File::open(format!("/proc/{pid}/status"))
                        .unwrap()
                        .take(65537)
                        .read_to_string(&mut status)
                        .unwrap();
                    cancel.check(deadline, Phase::Startup).unwrap();
                    assert!(status.len() <= 65536);
                    let field = |name| {
                        status
                            .lines()
                            .find_map(|line| line.strip_prefix(name))
                            .unwrap()
                            .trim()
                    };
                    assert_eq!(field("NoNewPrivs:"), "1", "{status}");
                    assert_eq!(field("Seccomp:"), "2", "{status}");
                    assert!(field("NSpid:").split_whitespace().count() > 1, "{status}");
                    eprintln!(
                        "Linux renderer {pid}: NoNewPrivs={} Seccomp={} NSpid={}",
                        field("NoNewPrivs:"),
                        field("Seccomp:"),
                        field("NSpid:")
                    );
                    renderers += 1;
                }
                assert!(renderers > 0, "no renderer sandbox evidence: {processes}");
            }
        }
        #[cfg(target_os = "macos")]
        {
            let topology = child.topology().unwrap();
            eprintln!("macOS owned topology (executable, detached session): {topology:?}");
            assert!(
                topology
                    .iter()
                    .any(|(exe, detached)| exe == "chrome_crashpad_handler" && *detached)
            );
        }
        #[cfg(target_os = "linux")]
        {
            // Exercise the actual pinned singleton socket, not a name-based
            // cleanup exception. This path conveys no process signal authority.
            let socket = std::fs::read_link(artifacts.join("profile/SingletonSocket")).unwrap();
            assert!(
                socket.starts_with(artifacts.canonicalize().unwrap().join("tmp")),
                "{socket:?}"
            );
            assert!(socket.exists());
        }
        if scenario == "cancel" {
            cancel.cancel();
        }
        if scenario == "exit" {
            child.kill_root().unwrap();
        }
        let deadline = if scenario == "timeout" {
            Instant::now()
        } else {
            deadline
        };
        let error = cdp::Cdp::new(&mut child, &cancel)
            .call("Browser.getVersion", json!({}), deadline, Phase::Capture)
            .unwrap_err();
        match scenario {
            "cancel" => assert!(matches!(error, Error::Cancelled)),
            "timeout" => assert!(matches!(error, Error::Timeout(Phase::Capture))),
            "exit" => assert!(matches!(
                error,
                Error::BrowserExited
                    | Error::Protocol(error::ProtocolError::Eof)
                    | Error::Io { .. }
            )),
            _ => unreachable!(),
        }
        let cleanup = child.finish(Duration::from_secs(5));
        assert_lifecycle_cleanup(scenario, &cleanup);
        assert!(!artifacts.exists());
    }
}

// Read-only sandbox evidence; this decode confers no signaling authority.
fn sandbox_renderer_pid(process: &Value) -> Option<libc::pid_t> {
    libc::pid_t::try_from(process.get("id")?.as_i64()?)
        .ok()
        .filter(|pid| *pid > 0)
}

#[test]
fn sandbox_renderer_pid_requires_positive_native_integer() {
    assert_eq!(sandbox_renderer_pid(&json!({"id":1})), Some(1));
    assert_eq!(
        sandbox_renderer_pid(&json!({"id":libc::pid_t::MAX})),
        Some(libc::pid_t::MAX)
    );
    for process in [
        json!({}),
        json!({"id":null}),
        json!({"id":"123"}),
        json!({"id":true}),
        json!({"id":[]}),
        json!({"id":1.0}),
        json!({"id":1.5}),
        json!({"id":0}),
        json!({"id":-1}),
        json!({"id":i64::from(libc::pid_t::MAX) + 1}),
        json!({"id":u64::MAX}),
    ] {
        assert_eq!(sandbox_renderer_pid(&process), None, "{process}");
    }
}

// AG3-only controls, available exclusively in an isolated test process.
thread_local! {
    static SHUTDOWN_ACKS_BEFORE_EXPIRY: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SHUTDOWN_ACKS_BEFORE_PAUSE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SHUTDOWN_ACKS_BEFORE_CANCEL: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPARISON_CLEANUP_COUNTDOWN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static COMPARISON_CLEANUP_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static COMPARISON_RETAINED_PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}
pub(crate) fn controlled_capture(color: CanvasColor) -> ChromiumCapture {
    let pin: Value = serde_json::from_str(include_str!("../../chromium-reference.json")).unwrap();
    ChromiumCapture {
        color: Some(color),
        inspection: None,
        identity: identity(json!({
            "product": format!("Chrome/{}", pin["version"].as_str().unwrap()),
            "revision": pin["revision"], "protocolVersion": pin["protocol_version"],
        }))
        .unwrap(),
        metadata_links: vec![],
        default_favicon_aborted: false,
    }
}
pub(crate) fn arm_shutdown_ack_cancellation(count: usize) {
    assert!(count > 0);
    SHUTDOWN_ACKS_BEFORE_CANCEL.with(|state| assert_eq!(state.replace(count), 0));
}
pub(crate) fn arm_second_shutdown_read_pause() {
    SHUTDOWN_ACKS_BEFORE_PAUSE.with(|state| assert_eq!(state.replace(2), 0));
}
pub(crate) fn arm_second_shutdown_expiry() {
    SHUTDOWN_ACKS_BEFORE_EXPIRY.with(|state| assert_eq!(state.replace(2), 0));
}
pub(super) fn shutdown_acknowledged(
    cancel: &Cancellation,
    started: Instant,
    capture_deadline: Instant,
) {
    SHUTDOWN_ACKS_BEFORE_EXPIRY.with(|state| {
        let count = state.get();
        if count > 0 {
            state.set(count - 1);
            if count == 1 {
                // Exhaust this capture's actual absolute budget. Do not alter
                // its deadline or consume/close either protocol direction.
                cancel.check(capture_deadline, Phase::Shutdown).unwrap();
                process::fault::set(process::fault::Point::ShutdownRead);
                process::fault::expire(process::fault::Point::ShutdownRead, capture_deadline);
                process::fault::assert_consumed();
                cancel.check_cancelled().unwrap();
                assert!(Instant::now() >= capture_deadline);
                eprintln!(
                    "AG7 second shutdown absolute budget exhausted; command writer unchanged"
                );
            }
        }
    });
    SHUTDOWN_ACKS_BEFORE_PAUSE.with(|state| {
        let count = state.get();
        if count > 0 {
            state.set(count - 1);
            if count == 1 {
                // Model a scheduler/native-operation stall after the second
                // acknowledgement. Preserve all incoming bytes and the clock;
                // the production deadline checks must still decide the result.
                process::fault::set(process::fault::Point::ShutdownRead);
                process::fault::expire(process::fault::Point::ShutdownRead, started + Duration::from_millis(300));
                process::fault::assert_consumed();
                eprintln!("AG7 second shutdown read pause consumed; elapsed={:?}; command writer unchanged", started.elapsed());
            }
        }
    });
    SHUTDOWN_ACKS_BEFORE_CANCEL.with(|state| {
        let count = state.get();
        if count > 0 {
            state.set(count - 1);
            if count == 1 {
                cancel.cancel();
            }
        }
    });
}
pub(crate) fn assert_shutdown_cancellation_consumed() {
    SHUTDOWN_ACKS_BEFORE_EXPIRY.with(|state| assert_eq!(state.get(), 0));
    SHUTDOWN_ACKS_BEFORE_PAUSE.with(|state| assert_eq!(state.get(), 0));
    SHUTDOWN_ACKS_BEFORE_CANCEL.with(|state| assert_eq!(state.get(), 0));
}
pub(crate) fn arm_comparison_cleanup_failure(cancel: bool, capture: usize) {
    assert!(capture > 0);
    COMPARISON_CLEANUP_COUNTDOWN.with(|state| assert_eq!(state.replace(capture), 0));
    COMPARISON_CLEANUP_FAULT.with(|fault| {
        assert_eq!(fault.replace(if cancel { 2 } else { 1 }), 0);
    });
}
pub(super) fn comparison_before_cleanup(child: &OwnedChromium, cancel: &Cancellation) {
    use std::os::unix::fs::PermissionsExt;
    if COMPARISON_CLEANUP_COUNTDOWN.with(|state| {
        let count = state.get();
        if count > 0 {
            state.set(count - 1);
        }
        count != 1
    }) {
        return;
    }
    match COMPARISON_CLEANUP_FAULT.with(|fault| fault.replace(0)) {
        0 => {}
        1 => {
            let root = child.artifact_path().to_owned();
            let locked = root.join("ag3-removal");
            std::fs::create_dir(&locked).unwrap();
            std::fs::write(locked.join("retained"), b"cleanup must fail").unwrap();
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
            COMPARISON_RETAINED_PATH.with(|path| *path.borrow_mut() = Some(root));
        }
        2 => cancel.cancel(),
        _ => unreachable!(),
    }
}
pub(crate) fn recover_comparison_cleanup_failure() {
    use std::os::unix::fs::PermissionsExt;
    COMPARISON_CLEANUP_FAULT.with(|fault| assert_eq!(fault.get(), 0, "fault never activated"));
    let root = COMPARISON_RETAINED_PATH
        .with(|path| path.borrow_mut().take())
        .unwrap();
    assert!(root.join("ag3-removal/retained").exists());
    assert_no_children();
    std::fs::set_permissions(
        root.join("ag3-removal"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}
pub(crate) fn controlled_comparison_failure() -> Failure {
    Failure {
        context: None,
        primary: Some(Error::Capture("original primary".into())),
        cleanup: vec![error::CleanupError::Identity("verification failed".into())],
    }
}
pub(crate) fn assert_comparison_cleanup_failure(failure: &Failure) {
    assert!(
        failure.primary.is_none(),
        "capture should have succeeded: {failure:?}"
    );
    assert!(
        failure
            .cleanup
            .iter()
            .any(|e| matches!(e, error::CleanupError::Artifacts(_))),
        "{failure:?}"
    );
}

pub(crate) fn assert_comparison_shutdown_timeout(failure: &Failure) {
    assert!(
        matches!(failure.primary, Some(Error::Timeout(Phase::Shutdown))),
        "{failure:?}"
    );
    assert!(failure.cleanup.is_empty(), "{failure:?}");
    assert_no_children();
}

// The fixed collector's thrown Error message is the discriminator already
// carried by Runtime.evaluate. Earlier capture failures cannot satisfy it.
fn is_inspection_rejection(failure: &Failure, reason: &str) -> bool {
    let Some(Error::Capture(detail)) = &failure.primary else {
        return false;
    };
    let Some(exception) = detail.strip_prefix("native inspection failed: ") else {
        return false;
    };
    let Ok(exception) = serde_json::from_str::<Value>(exception) else {
        return false;
    };
    failure.cleanup.is_empty()
        && exception["exception"]["description"]
            .as_str()
            .and_then(|description| description.lines().next())
            == Some(format!("Error: {reason}").as_str())
}

#[test]
fn inspection_negative_control_rejects_unrelated_capture_failures() {
    let expected = "inspection string limit";
    for message in [
        "unexpected viewport dimensions or scroll".to_string(),
        "native inspection failed: null".to_string(),
        format!(
            "native inspection failed: {}",
            json!({"exception":{"description":"Error: inspection tree limit\n at node"}})
        ),
    ] {
        let failure = Failure {
            context: None,
            primary: Some(Error::Capture(message)),
            cleanup: vec![],
        };
        assert!(!is_inspection_rejection(&failure, expected));
    }
    let mut failure = Failure {
        context: None,
        primary: Some(Error::Capture(format!(
            "native inspection failed: {}",
            json!({"exception":{"description":"Error: inspection string limit\n at text"}})
        ))),
        cleanup: vec![],
    };
    assert!(is_inspection_rejection(&failure, expected));
    failure.cleanup.push(error::CleanupError::Timeout);
    assert!(!is_inspection_rejection(&failure, expected));
}

#[test]
#[ignore = "requires pinned Chromium; standalone; page scripts must remain disabled"]
fn real_chromium_ag5_read_only_inspection() {
    let path = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").expect("pinned browser required");
    let _standalone = process::Standalone::enter().unwrap();
    let config = ChromiumConfig::new(PathBuf::from(path));
    let request = crate::projection::ObservationRequest {
        dom: crate::projection::Request::Compare(()),
        styles: &[crate::projection::StyleField {
            target: "target",
            property: crate::projection::Request::Compare(crate::projection::StyleProperty::Color),
            name: "color",
        }],
        layout: crate::projection::Request::Compare(&["target"]),
        ..crate::projection::ObservationRequest::NONE
    };
    // If page scripts execute, they replace both the inspected color and width.
    let html = br#"<!doctype html><html><head><style>
      html,body {margin:0} #target {color:#123456;width:32px;height:16px}
      </style></head><body><div id="target"></div><script>
      document.getElementById('target').style.cssText='color:red;width:64px';
      </script></body></html>"#;
    let capture = capture_requested_html(html, request, &config, &Cancellation::default()).unwrap();
    assert!(capture.color.is_none());
    let observed = capture.inspection.as_ref().unwrap();
    assert!(observed.dom.is_some());
    assert_eq!(
        (observed.inner_width, observed.inner_height),
        (640.0, 480.0)
    );
    assert_eq!(observed.layout_width, 640.0);
    assert_eq!(
        observed.styles[0].values.as_ref().unwrap()[0],
        "rgb(18, 52, 86)"
    );
    assert_eq!(observed.layout[0].boxes.as_ref().unwrap()[0].right, 32.0);
    crate::projection::chromium(&capture, request).unwrap();
    // These are capture errors, never truncated successful observations.
    // Inert template contents still exercise the complete DOM collector but
    // cannot introduce scrollbars before the intended bounds check.
    for (reason, bad) in [
        (
            "inspection string limit",
            format!("<!doctype html><template>{}</template>", "x".repeat(8193)),
        ),
        (
            "inspection tree limit",
            format!(
                "<!doctype html><template>{}</template>",
                "<div></div>".repeat(129)
            ),
        ),
        (
            "duplicate target id",
            "<!doctype html><div id=target></div><div id=target></div>".into(),
        ),
    ] {
        let failure =
            capture_requested_html(bad.as_bytes(), request, &config, &Cancellation::default())
                .unwrap_err();
        assert!(
            is_inspection_rejection(&failure, reason),
            "{reason}: {failure:?}"
        );
        assert!(failure.cleanup.is_empty(), "{failure:?}");
        eprintln!("AG5 native inspection rejected {reason}; cleanup verified");
    }
}

#[path = "../../tests/support/ag7.rs"]
mod ag7_acceptance;

#[test]
#[ignore = "requires pinned Chromium, standalone process; all captures precede parser workers"]
fn real_chromium_ag5_profile() {
    let path = std::env::var("BORROWSER_CHROMIUM_EXECUTABLE").expect("pinned browser required");
    let mut out = vec![];
    let mut diagnostics = vec![];
    let code = crate::comparison::run(
        &["--chromium-executable".into(), path],
        &mut out,
        &mut diagnostics,
    )
    .unwrap();
    assert!(
        code == 1,
        "{}\n{}",
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&diagnostics)
    );
    let report: Value = serde_json::from_slice(&out).unwrap();
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), crate::fixtures::catalogue().unwrap().len());
    assert!(cases.windows(2).all(|pair| pair[0]["test_id"].as_str().unwrap() < pair[1]["test_id"].as_str().unwrap()));
    let selected: Vec<_> = crate::fixtures::catalogue().unwrap().iter().collect();
    let descriptors: Vec<_> = super::inputs(&selected)
        .iter()
        .map(|input| input.descriptor())
        .collect();
    ag7_acceptance::assert_catalogue(
        &report,
        i32::from(code),
        &json!(descriptors),
        &ag7_acceptance::declarations(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")),
    );

    let ids = [
        "canvas/root",
        "canvas/cascade",
        "dom/structure",
        "dom/template-namespaces",
        "style/cascade-inheritance",
        "layout/block-boxes",
    ];
    let mut baseline_fields = 0;
    for id in ids {
        let case = cases.iter().find(|case| case["test_id"] == id).unwrap();
        let fields = case["fields"].as_array().unwrap();
        baseline_fields += fields.len();
        assert_eq!(
            fields
                .iter()
                .map(|f| f["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            crate::comparison::tests::expected_profile_fields(id)
        );
        assert!(fields.iter().all(|f| f["status"] == "equal"));
        assert_eq!(case["status"], "equal_for_requested_scope");
        assert_eq!(case["borrowser_expectation"]["status"], "PASS");
        eprintln!(
            "AG5 {} {} fields={}",
            case["test_id"],
            case["status"],
            case["fields"].as_array().unwrap().len()
        );
    }
    assert_eq!(baseline_fields, 34);
}

#[test]
fn metadata_links_policy_rejects_mixed_attributes_namespaces_and_order() {
    let fixture = crate::fixtures::fixture("wpt/rgb-comments-p6");
    let links = fixture.document.links().unwrap();
    crate::metadata_links::validate(&links).unwrap();
    for change in 0..5 {
        let mut bad = links.clone();
        match change {
            0 => bad[0].namespace = "http://www.w3.org/2000/svg".into(),
            1 => {
                bad[0]
                    .attributes
                    .iter_mut()
                    .find(|a| a.local_name == "rel")
                    .unwrap()
                    .value = "author stylesheet".into()
            }
            2 => bad[0].attributes[0].namespace = "http://www.w3.org/1999/xlink".into(),
            3 => bad[0].attributes.reverse(),
            4 => bad[0].attributes[0].local_name = "onload".into(),
            _ => unreachable!(),
        }
        assert!(crate::metadata_links::validate(&bad).is_err(), "{change}");
    }
}

#[test]
#[ignore = "requires pinned Chromium, standalone process; fixed read-only metadata inspection"]
fn real_chromium_ag7_inert_links() {
    let path = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").expect("pinned Chromium required");
    let _standalone = process::Standalone::enter().unwrap();
    let config = ChromiumConfig::new(path.into());
    let cancel = Cancellation::default();
    for id in [
        "wpt/rgb-comments-p6",
        "wpt/rgb-alias-p6",
        "wpt-style/background-color-mismatch",
    ] {
        let fixture = crate::fixtures::fixture(id);
        for input in inputs(&[fixture]) {
            let links: Vec<crate::metadata_links::LinkFact> =
                serde_json::from_str(input.metadata_links).unwrap();
            let capture =
                capture_document(input.html, input.request, &links, &config, &cancel).unwrap();
            assert_eq!(capture.metadata_links, links);
            let projection = crate::projection::chromium(&capture, input.request).unwrap();
            assert_eq!(projection.styles.len(), 1);
            assert!(capture.color.is_none());
            eprintln!(
                "AG7 metadata {id} {:?}: links={} cleanup verified",
                input.role,
                capture.metadata_links.len()
            );
        }
    }
    use crate::metadata_links::{AttributeFact, LinkFact};
    let attr = |name: &str, value: &str| AttributeFact {
        namespace: String::new(),
        prefix: String::new(),
        local_name: name.into(),
        value: value.into(),
    };
    let expected = vec![LinkFact {
        namespace: crate::metadata_links::HTML.into(),
        local_name: "link".into(),
        attributes: vec![
            attr("href", "?a=1&b=2"),
            attr("rel", " HeLp "),
            attr("title", "A & B"),
        ],
    }];
    let html=b"<!doctype html><link title='A &amp; B' rel=' HeLp ' href='?a=1&amp;b=2'><div id=probe style='background-color:green'></div>";
    let request = crate::fixtures::fixture("wpt-style/background-color-mismatch").request;
    assert_eq!(
        capture_document(html, request, &expected, &config, &cancel)
            .unwrap()
            .metadata_links,
        expected
    );
    for relation in [
        "help stylesheet",
        "help preload",
        "help help",
        "canonical",
        "icon",
    ] {
        let bad = String::from_utf8(html.to_vec())
            .unwrap()
            .replace(" HeLp ", relation);
        let failure =
            capture_document(bad.as_bytes(), request, &expected, &config, &cancel).unwrap_err();
        assert!(failure.primary.is_some(), "{relation}");
        assert!(failure.cleanup.is_empty(), "{failure:?}");
    }
    let mut resource = html.to_vec();
    resource.extend_from_slice(b"<img src='https://borrowser.invalid/denied.png'>");
    let failure = capture_document(&resource, request, &expected, &config, &cancel).unwrap_err();
    assert!(failure.primary.is_some());
    assert!(failure.cleanup.is_empty());
}

#[test]
fn ag8_all_capture_failures_preserve_document_role_and_primary_cleanup_codes() {
    let fixture = crate::fixtures::fixture("wpt-style/background-color-mismatch");
    let inputs = inputs(&[fixture]);
    for primary in [
        Error::Configuration("invalid".into()),
        Error::Timeout(Phase::Navigation),
        Error::BrowserExited,
    ] {
        let expected_code = primary.code();
        let failure = attribute_capture_failure(
            Failure {
                context: None,
                primary: Some(primary),
                cleanup: vec![error::CleanupError::Timeout],
            },
            &inputs[1],
        );
        let mut diagnostics = vec![];
        failure.write_diagnostics(&mut diagnostics).unwrap();
        let text = String::from_utf8(diagnostics).unwrap();
        assert!(text.contains(&format!("ERROR chromium.{expected_code}:")));
        assert!(text.contains("ERROR chromium.cleanup:"));
        assert!(text.contains("role=Reference"));
        assert!(text.contains("wpt-style/background-color-mismatch"));
        assert!(text.contains("fixtures/wpt-style/red.html"));
        assert!(!text.contains("SUMMARY"));
    }
}
