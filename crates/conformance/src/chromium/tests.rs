use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::os::fd::FromRawFd;
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
))]
use std::process::Command;

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
    assert!(matches!(
        capture_html(b"", &config, &Cancellation::default())
            .unwrap_err()
            .primary,
        Some(Error::Configuration(_))
    ));
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
        let output = Command::new(std::env::current_exe().unwrap())
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
        assert!(
            unrelated.try_wait().unwrap().is_none(),
            "terminated unrelated sibling"
        );
        eprintln!("native scenario {scenario}: passed; unrelated sibling survived");
    }
    drop(unrelated.stdin.take());
    assert!(unrelated.wait().unwrap().success());
}
#[test]
#[ignore = "private isolated-process entry; native_process_and_pipe_contract invokes it"]
fn native_case() {
    let scenario = std::env::var("AG2_NATIVE_CASE").expect("private native scenario");
    let _standalone = process::Standalone::enter().unwrap();
    // Exercise FD3/4 collisions: launch must replace them and close all the
    // other ambient copies in the exec child without changing the parent.
    let occupied: Vec<_> = (0..16)
        .map(|_| std::fs::File::open("/dev/null").unwrap())
        .collect();
    let _keep_occupied = occupied;
    let cancel = Cancellation::default();
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
        assert!(child.finish(Duration::from_secs(2)).is_empty());
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
    if scenario == "unrelated" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).unwrap();
        return;
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

#[test]
#[ignore = "requires the configured pinned real Chromium; run with --test-threads=1"]
fn real_chromium_capture_and_scripts() {
    let path = std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE")
        .expect("explicit real Chromium test requires BORROWSER_CHROMIUM_EXECUTABLE");
    let _standalone = process::Standalone::enter().unwrap();
    let config = ChromiumConfig::new(PathBuf::from(path));
    let cancel = Cancellation::default();
    for fixture in crate::fixtures::FIXTURES {
        let FixtureKind::Runnable { html, expected, .. } = fixture.kind else {
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
            assert_eq!(result.color, expected);
            let serialized = serde_json::to_string(&(result.color.0, result.identity)).unwrap();
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
    let scripted = b"<!doctype html><html><head><style>html{background:#123456}body{margin:0}</style><script>document.documentElement.style.backgroundColor='#ff0000'</script></head><body></body></html>";
    eprintln!("AG2 fixture=script-suppression iteration=1/1");
    assert_eq!(
        capture_html(scripted, &config, &cancel).unwrap().color,
        CanvasColor([18, 52, 86])
    );
    let enabled = ChromiumConfig {
        scripts_disabled: false,
        ..config
    };
    eprintln!("AG2 fixture=script-positive-control iteration=1/1");
    assert_eq!(
        capture_html(scripted, &enabled, &cancel).unwrap().color,
        CanvasColor([255, 0, 0])
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
        assert!(child.finish(Duration::from_secs(5)).is_empty());
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
