use super::*;
use std::process::Command;

#[test]
fn cleanup_phase_deadlines() {
    let start = Instant::now();
    let mut state = Cleanup::new(start, true);
    assert_eq!(state.boundary(), start + Duration::from_secs(2));
    assert_eq!(state.force_by, start + Duration::from_secs(3));
    assert_eq!(state.hard_end, start + Duration::from_secs(5));
    assert!(!state.end_natural(start + Duration::from_millis(1999), false));
    assert!(state.end_natural(start + Duration::from_secs(2), false));
    assert!(!state.end_natural(start + Duration::from_secs(2), true));
    state.term_dispatched(start + Duration::from_millis(2400));
    assert_eq!(state.boundary(), start + Duration::from_secs(3));
    assert!(!state.kill_due(start + Duration::from_millis(2999)));
    assert!(state.kill_due(start + Duration::from_secs(3)));
    assert_eq!(state.hard_end, start + Duration::from_secs(5));
    assert!(
        state
            .check_deadline(start + Duration::from_millis(4999))
            .is_ok()
    );
    assert!(
        state
            .check_deadline(start + Duration::from_secs(5))
            .is_err()
    );
    assert!(
        state
            .check_deadline(start + Duration::from_secs(6))
            .is_err()
    );

    let mut early = Cleanup::new(start, false);
    assert_eq!(early.phase, CleanupPhase::TermPending);
    early.term_dispatched(start + Duration::from_millis(10));
    assert_eq!(early.boundary(), start + Duration::from_millis(1010));
    let mut failed = Cleanup::new(start, true);
    assert!(failed.end_natural(start + Duration::from_millis(10), true));
    failed.term_dispatched(start + Duration::from_secs(4));
    assert!(failed.kill_due(start + Duration::from_secs(4)));
    assert_eq!(failed.hard_end, start + Duration::from_secs(5));
}

#[test]
fn cleanup_poll_boundaries() {
    let start = Instant::now();
    assert_eq!(poll_timeout(start, start + Duration::from_secs(2)), 20);
    assert_eq!(poll_timeout(start, start + Duration::from_millis(7)), 7);
    assert_eq!(poll_timeout(start, start + Duration::from_micros(999)), 0);
    assert_eq!(poll_timeout(start, start), 0);
    // Recompute after work/EINTR; an old wait must not cross the new boundary.
    let boundary = start + Duration::from_millis(20);
    assert_eq!(poll_timeout(start + Duration::from_millis(18), boundary), 2);
    assert_eq!(poll_timeout(start + Duration::from_millis(25), boundary), 0);
    // libc permits an empty set, but it must use exactly the same bounded wait.
    assert_eq!(
        unsafe { libc::poll(std::ptr::null_mut(), 0, poll_timeout(start, start)) },
        0
    );
}

#[test]
fn cleanup_observation_deadline_is_not_quiescence() {
    assert_eq!(
        native::group_running(unsafe { libc::getpgrp() }, Instant::now()).unwrap(),
        GroupObservation::DeadlineReached
    );
}

fn assert_authority_order(owner: &Owner) {
    assert_eq!(
        owner
            .timing
            .iter()
            .filter(|(name, _)| *name == "cleanup_started")
            .count(),
        1
    );
    for signal in ["term_sent", "kill_sent"] {
        assert!(
            owner
                .timing
                .iter()
                .filter(|(name, _)| *name == signal)
                .count()
                <= 1
        );
    }
    let event = |name| owner.timing.iter().find(|(n, _)| *n == name).unwrap().1;
    let confirmed = event("group_quiescence_confirmed");
    let signals_done = event("signaling_complete");
    let root_reaped = event("root_reaped");
    assert!(confirmed <= signals_done && signals_done <= root_reaped);
    for (name, at) in &owner.timing {
        if matches!(*name, "group_observation" | "term_sent" | "kill_sent") {
            assert!(
                *at <= confirmed && *at <= root_reaped,
                "post-ownership authority: {name}"
            );
        }
    }
    assert!(root_reaped <= event("descendant_reap_started"));
    assert!(event("descendants_reaped") <= event("owner_quiescent"));
    assert!(event("owner_quiescent") < event("cleanup_started") + CLEANUP_LIMIT);
}

// Each command owner runs in an isolated process: no test-harness child/reaper races.
#[test]
fn native_command_owner() {
    for mode in [
        "normal",
        "both",
        "overflow",
        "stderr-overflow",
        "timeout",
        "timeout-stderr",
        "pipe-descendant",
        "closed-descendant",
        "stdout-closed-descendant",
        "stderr-closed-descendant",
        "cancel",
        "second-cancel",
        "blocked-input",
        "closed-input",
        "exiting-input",
        "inspection-failure",
        "signal-failure",
        "aws-environment",
        "proxy-natural",
        "proxy-delayed",
        "proxy-survivor",
        "proxy-observation-deadline",
        "proxy-late-secret",
        "expired-native-reap",
        "poll-eintr",
        "reserved-root",
    ] {
        eprintln!("AG4_OWNER_SCENARIO start {mode}");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "aws::command::tests::isolated_owner",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("BORROWSER_AG4_COMMAND_SCENARIO", mode)
            .env("AWS_ACCESS_KEY_ID", "synthetic-access")
            .env("AWS_SECRET_ACCESS_KEY", "synthetic-secret")
            .env("AWS_SESSION_TOKEN", "synthetic-token")
            .env("AWS_ENDPOINT_URL", "http://127.0.0.1:1")
            .env("AWS_ENDPOINT_URL_CLOUDFORMATION", "http://127.0.0.1:2")
            .env("AWS_PROFILE", "unsupported-helper")
            .env("AWS_CLI_AUTO_PROMPT", "on")
            .env("AWS_CONFIG_FILE", "/untrusted/config")
            .env("AWS_SHARED_CREDENTIALS_FILE", "/untrusted/credentials")
            .env("AWS_WEB_IDENTITY_TOKEN_FILE", "/untrusted/token")
            .env("AWS_CONTAINER_CREDENTIALS_FULL_URI", "http://127.0.0.1:3")
            .env("HTTPS_PROXY", "http://127.0.0.1:4")
            .env("AWS_DATA_PATH", "/untrusted/models")
            .env_remove("AWS_CA_BUNDLE")
            .process_group(0)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                // This outer test guard reports a broken production bound rather
                // than allowing an ordinary CI job to wait forever.
                panic!("isolated command owner exceeded its bound: {mode}");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "scenario {mode}");
        eprintln!("AG4_OWNER_SCENARIO passed {mode}");
    }
}
#[test]
#[ignore = "only invoked by the isolated native-command wrapper"]
fn isolated_owner() {
    let Ok(mode) = std::env::var("BORROWSER_AG4_COMMAND_SCENARIO") else {
        return;
    };
    let mut owner = Owner::enter().unwrap();
    if mode == "expired-native-reap" {
        assert!(owner.native.reap(Instant::now()).is_err());
        owner.finish().unwrap();
        return;
    }
    if mode == "poll-eintr" {
        // Actual native empty-set poll interrupted on this thread. Repeated
        // directed signals avoid relying on a single sleep/startup race.
        let done = std::sync::atomic::AtomicBool::new(false);
        let target = unsafe { libc::pthread_self() } as usize;
        let end = Instant::now() + Duration::from_secs(1);
        let interrupted = std::thread::scope(|scope| {
            scope.spawn(|| {
                while !done.load(Ordering::Relaxed) && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(1));
                    unsafe { libc::pthread_kill(target as libc::pthread_t, libc::SIGINT) };
                }
            });
            let mut observed = false;
            while Instant::now() < end {
                let rc = unsafe {
                    libc::poll(std::ptr::null_mut(), 0, poll_timeout(Instant::now(), end))
                };
                if rc < 0 {
                    assert_eq!(
                        std::io::Error::last_os_error().kind(),
                        std::io::ErrorKind::Interrupted
                    );
                    observed = true;
                    break;
                }
            }
            done.store(true, Ordering::Relaxed);
            observed
        });
        assert!(interrupted);
        assert!(owner.finish().is_err());
        return;
    }
    if mode == "reserved-root" {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let end = Instant::now() + Duration::from_secs(1);
        while !exited(pid).unwrap() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(root_succeeded(pid).unwrap());
        assert_eq!(
            native::group_running(pid, end).unwrap(),
            GroupObservation::NoExecuting
        );
        assert!(
            exited(pid).unwrap(),
            "group observation must not reap the root"
        );
        assert!(child.try_wait().unwrap().unwrap().success());
        // No numeric identity checks follow this reap.
        owner.native.reap(end).unwrap();
        owner.finish().unwrap();
        return;
    }
    if mode == "proxy-timing" {
        proxy_cleanup_timing(owner);
        return;
    }
    if mode == "ssh-banner" {
        ssh_banner_timeout(owner);
        return;
    }
    if mode == "eice-cli-local" {
        eice_cli_local(owner);
        return;
    }
    if mode == "aws-environment" {
        aws_environment(owner);
        return;
    }
    if mode == "aws-model-library" || mode == "aws-model-cli" {
        aws_model_resolution(owner, &mode);
        return;
    }
    if mode == "cdk" {
        let python =
            std::env::var_os("BORROWSER_AG4_TEST_PYTHON").expect("hash-locked CDK Python required");
        let work = tempfile::tempdir().unwrap();
        let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aws/tests");
        let script = "import json,sys; sys.path.insert(0,sys.argv[1]); from test_stack import Infrastructure; print(json.dumps(Infrastructure().synth()))";
        let mut spec = Spec::new(
            python,
            vec!["-c".into(), script.into(), tests.into_os_string()],
        );
        spec.timeout = Duration::from_secs(60);
        spec.stdout_limit = 51200;
        spec.env.push((
            "JSII_RUNTIME_PACKAGE_CACHE_ROOT".into(),
            work.path().join("cache").into_os_string(),
        ));
        spec.env
            .push(("AWS_EC2_METADATA_DISABLED".into(), "true".into()));
        let result = owner.run(spec).unwrap();
        assert_eq!(
            result.code,
            0,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let template = crate::aws::template::json(&result.stdout).unwrap();
        crate::aws::template::binding(&template).unwrap();
        owner.finish().unwrap();
        return;
    }
    if mode == "inspection-failure" {
        FAIL_OBSERVATION.store(true, Ordering::Relaxed);
    }
    if mode == "proxy-observation-deadline" {
        EXHAUST_OBSERVATION_ONCE.store(true, Ordering::Relaxed);
    }
    if mode == "signal-failure" {
        FAIL_TERM_ONCE.store(true, Ordering::Relaxed);
    }
    let mut spec = Spec::new(
        "/bin/sh",
        [
            "-c",
            match mode.as_str() {
                "normal" | "inspection-failure" => "printf ok",
                "both" => {
                    "i=0; while [ $i -lt 1000 ]; do printf 'out'; printf 'err' >&2; i=$((i+1)); done"
                }
                "overflow" => "while :; do printf '012345678901234567890123456789'; done",
                "stderr-overflow" => {
                    "while :; do printf '012345678901234567890123456789' >&2; done"
                }
                "timeout" | "blocked-input" | "signal-failure" => {
                    "trap '' TERM; while :; do sleep 1; done"
                }
                // Input acceptance synchronizes closure; stopping instead of
                // naturally exiting leaves termination entirely to the owner.
                "closed-input" => {
                    "read -r line; exec 0<&-; printf 'input closed' >&2; kill -STOP $$"
                }
                "exiting-input" => "read -r line; exec 0<&-; printf 'input closed' >&2; exit 23",
                "timeout-stderr" => {
                    "printf 'bounded SSH context' >&2; trap '' TERM; while :; do sleep 1; done"
                }
                "pipe-descendant" => "sleep 30 & exit 0",
                "closed-descendant" => "sleep 30 </dev/null >/dev/null 2>&1 & exit 0",
                "stdout-closed-descendant" => "sleep 30 >/dev/null & exit 0",
                "stderr-closed-descendant" => "sleep 30 2>/dev/null & exit 0",
                "proxy-natural" => "(printf late-proxy >&2) & printf ok; exit 0",
                "proxy-delayed" => "(sleep 1.2; printf late-proxy >&2) & printf ok; exit 0",
                "proxy-survivor" | "proxy-observation-deadline" => {
                    "(trap '' TERM; sleep 30) & exit 0"
                }
                "proxy-late-secret" => {
                    "(sleep 0.2; printf 'banner failure synthe' >&2; sleep 0.05; printf 'tic-secret\nprocess-shutdown error\nX-Amz-Signature=AUTH_SENTINEL\nunindented-auth-continuation' >&2) & printf ok; exit 0"
                }
                "cancel" => "kill -INT $PPID; sleep 30",
                "second-cancel" => "kill -INT $PPID; kill -TERM $PPID; sleep 30",
                _ => panic!("unknown scenario"),
            },
        ],
    );
    spec.timeout = Duration::from_millis(300);
    spec.stdout_limit = 4096;
    if mode.starts_with("proxy-") {
        spec.eice_proxy = true;
        spec.failure_stderr_limit = 2048;
        crate::aws::config::AwsEnvironment::load()
            .unwrap()
            .apply(&mut spec);
    }
    if mode == "timeout-stderr" {
        spec.failure_stderr_limit = 2048;
    }
    if matches!(
        mode.as_str(),
        "blocked-input" | "closed-input" | "exiting-input"
    ) {
        spec.input = vec![b'\n'; 1024 * 1024];
    }
    let start = Instant::now();
    let result = owner.run(spec);
    assert!(start.elapsed() < Duration::from_secs(7));
    if result
        .as_ref()
        .map_or_else(|failure| failure.cleanup.is_none(), |_| true)
    {
        assert_authority_order(&owner);
    }
    if mode == "normal"
        || mode == "both"
        || matches!(
            mode.as_str(),
            "proxy-natural" | "proxy-delayed" | "proxy-late-secret"
        )
    {
        let out = result.unwrap();
        assert_eq!(out.code, 0);
        assert!(!out.stdout.is_empty());
        if mode.starts_with("proxy-") {
            let text = String::from_utf8(out.stderr).unwrap();
            if mode == "proxy-late-secret" {
                assert!(text.contains("banner failure"));
                assert!(text.contains("process-shutdown error"));
                assert!(!text.contains("synthetic-secret"));
                assert!(!text.contains("AUTH_SENTINEL"));
                assert!(!text.contains("unindented-auth-continuation"));
            } else {
                assert_eq!(text, "late-proxy");
            }
            eprintln!("{mode}: natural quiescence in {:?}", start.elapsed());
        }
        owner.finish().unwrap();
    } else {
        let failure = result.unwrap_err();
        if mode == "closed-input" || mode == "exiting-input" {
            assert!(
                failure.primary.contains("command input:")
                    || failure.primary == "command input closed"
                    || failure.primary == "command exited before accepting its complete input"
            );
        }
        // Natural exit can race even a complete running-group observation.
        // Darwin may then reject SIGTERM with EPERM. This is NOT successful
        // cleanup: preserve that error and prove subsequent commands refused.
        if mode == "exiting-input" && failure.cleanup.is_some() {
            let diagnostic = failure.cleanup.as_ref().unwrap();
            assert!(
                diagnostic.contains("SIGTERM") && diagnostic.contains("raw errno=Some(1)"),
                "{failure}"
            );
            let mut cleanup = Spec::new("/bin/sh", ["-c", "exit 0"]);
            cleanup.cleanup = true;
            assert!(owner.run(cleanup).unwrap_err().cleanup.is_some());
            assert!(owner.finish().is_err());
            eprintln!(
                "Natural exit raced group signaling; EPERM remains unresolved and poisoned owner as required"
            );
            return;
        }
        if mode == "timeout-stderr" {
            assert!(failure.primary.contains("bounded SSH context"));
        }
        if mode == "signal-failure" {
            let error = failure.cleanup.as_ref().unwrap();
            assert!(error.contains("SIGTERM") && error.contains("raw errno=Some(1)"));
            assert!(!FAIL_TERM_ONCE.load(Ordering::Relaxed));
            assert!(owner.poisoned);
            assert!(!owner.timing.iter().any(|(n, _)| *n == "owner_quiescent"));

            // The failed TERM remains an authority error even though the real
            // KILL and native reaping can still complete bounded safe cleanup.
            let event = |name| owner.timing.iter().find(|(n, _)| *n == name).unwrap().1;
            let confirmed = event("group_quiescence_confirmed");
            let root_reaped = event("root_reaped");
            assert!(event("term_sent") <= event("kill_sent"));
            assert!(event("kill_sent") <= confirmed);
            assert!(event("stdout_eof") <= confirmed && event("stderr_eof") <= confirmed);
            assert!(confirmed <= event("signaling_complete"));
            assert!(event("signaling_complete") <= root_reaped);
            assert!(root_reaped <= event("descendant_reap_started"));
            assert!(event("descendant_reap_started") <= event("descendants_reaped"));
            assert!(event("descendants_reaped") < event("cleanup_started") + CLEANUP_LIMIT);
            for name in ["cleanup_started", "term_sent", "kill_sent", "root_reaped"] {
                assert_eq!(owner.timing.iter().filter(|(n, _)| *n == name).count(), 1);
            }
            for (name, at) in &owner.timing {
                if matches!(*name, "group_observation" | "term_sent" | "kill_sent") {
                    assert!(*at <= confirmed && *at <= root_reaped);
                }
            }
            // Only the kernel child relationship is inspected after root reap.
            // Its later completion must not clear the earlier signaling error.
            owner.native.quiescent().unwrap();
            assert!(owner.poisoned);
            assert_eq!(
                owner.quiescent().unwrap_err(),
                "controller command cleanup unresolved"
            );
            for cleanup in [false, true] {
                let mut next = Spec::new("/bin/sh", ["-c", "exit 0"]);
                next.cleanup = cleanup;
                let rejected = owner.run(next).unwrap_err();
                assert_eq!(rejected.primary, "command unavailable or interrupted");
                assert_eq!(
                    rejected.cleanup.as_deref(),
                    Some("previous command unresolved")
                );
                assert!(owner.poisoned);
                assert!(owner.timing.is_empty(), "poisoned owner must not spawn");
            }
            assert_eq!(
                owner.finish().unwrap_err(),
                "controller command cleanup unresolved"
            );
            eprintln!(
                "signal-failure: injected SIGTERM EPERM retained; KILL/EOF/pre-reap group confirmation/root/native reaping completed within cleanup authority; no owner_quiescent; later native completion did not clear poison; ordinary and cleanup commands rejected before spawn"
            );
            return;
        }
        if mode == "inspection-failure" {
            // An incomplete observation never authorizes root finalization.
            // This differs from the completed reaping with sticky signal error.
            assert!(failure.cleanup.is_some());
            assert!(owner.poisoned);
            assert!(!owner.timing.iter().any(|(n, _)| *n == "root_reaped"));
            assert!(!owner.timing.iter().any(|(n, _)| *n == "owner_quiescent"));
            let mut cleanup = Spec::new("/bin/sh", ["-c", "exit 0"]);
            cleanup.cleanup = true;
            assert_eq!(
                owner.run(cleanup).unwrap_err().cleanup.as_deref(),
                Some("previous command unresolved")
            );
            assert!(owner.timing.is_empty());
            assert!(owner.finish().is_err());
            eprintln!(
                "inspection-failure: incomplete group observation did not authorize root reap or owner_quiescent; cleanup command rejected by poisoned owner"
            );
            return;
        }
        assert!(failure.cleanup.is_none(), "{failure}");
        if mode == "cancel" {
            let mut cleanup = Spec::new("/bin/sh", ["-c", "printf cleanup"]);
            cleanup.cleanup = true;
            assert_eq!(owner.run(cleanup).unwrap().stdout, b"cleanup");
            assert!(owner.finish().is_err());
        } else if mode == "second-cancel" {
            let mut cleanup = Spec::new("/bin/sh", ["-c", "exit 0"]);
            cleanup.cleanup = true;
            assert!(owner.run(cleanup).is_err());
            assert!(owner.finish().is_err());
        } else {
            let mut next = Spec::new("/bin/sh", ["-c", "printf cleanup"]);
            next.cleanup = true;
            assert_eq!(owner.run(next).unwrap().stdout, b"cleanup");
            owner.finish().unwrap();
        }
    }
}

// Explicit native qualification only. Run the same test on both supported
// platforms; an emulated amd64 container does not qualify Linux timing.
fn proxy_cleanup_timing(mut owner: Owner) {
    let work = tempfile::tempdir().unwrap();
    for sample in 0..8 {
        for mode in ["natural", "delayed", "term", "kill"] {
            let ready = work.path().join("ready");
            let identities = work.path().join("identities");
            let body = match mode {
                "natural" => "printf ready > \"$1\"; sleep 0.05; printf late >&2",
                "delayed" => "printf ready > \"$1\"; sleep 1.2; printf late >&2",
                "term" => "trap 'exit 0' TERM; printf ready > \"$1\"; while :; do sleep 0.05; done",
                "kill" => "trap '' TERM; printf ready > \"$1\"; exec sleep 30",
                _ => unreachable!(),
            };
            // The child announces that its signal disposition is installed
            // before the root exits. No signal/startup race substitutes for the
            // intended TERM-responsive or TERM-ignoring descendant.
            let mut spec = Spec::new(
                "/bin/sh",
                vec![
                    OsString::from("-c"),
                    OsString::from(
                        "/bin/sh -c \"$3\" child \"$1\" & child=$!; while [ ! -f \"$1\" ]; do sleep 0.01; done; printf '%s %s\\n' \"$$\" \"$child\" > \"$2\"; exit 0",
                    ),
                    OsString::from("root"),
                    ready.clone().into_os_string(),
                    identities.clone().into_os_string(),
                    body.into(),
                ],
            );
            spec.eice_proxy = true;
            spec.timeout = Duration::from_secs(2);
            let result = owner.run(spec);
            let origin = owner
                .timing
                .iter()
                .find(|(name, _)| *name == "cleanup_started")
                .unwrap()
                .1;
            let observed = owner
                .timing
                .iter()
                .filter(|(_, at)| *at >= origin)
                .map(|(name, at)| (*name, at.duration_since(origin).as_micros()))
                .collect::<std::collections::BTreeMap<_, _>>();
            eprintln!(
                "AG4_PROXY_OWNER_TIMING {}",
                serde_json::json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"sample":sample,"mode":mode,"microseconds_from_cleanup_start":observed})
            );
            if matches!(mode, "natural" | "delayed") {
                assert_eq!(result.unwrap().code, 0);
            } else {
                let failure = result.unwrap_err();
                assert!(failure.primary.contains("running descendants"), "{failure}");
                assert!(failure.cleanup.is_none(), "{failure}");
            }
            assert_authority_order(&owner);
            owner.quiescent().unwrap();
            let event = |name| {
                owner
                    .timing
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, at)| *at)
            };
            let root_exit = event("root_exit_observed").unwrap();
            let end = event("owner_quiescent").unwrap();
            let deadline = origin + CLEANUP_LIMIT;
            assert!(
                end <= origin + Duration::from_secs(4),
                "qualification needs one second of headroom"
            );
            for name in [
                "natural_phase_ended",
                "stdout_eof",
                "stderr_eof",
                "group_quiescence_confirmed",
                "signaling_complete",
                "root_reaped",
                "descendant_reap_started",
                "descendants_reaped",
            ] {
                assert!(event(name).unwrap() <= end, "missing {name}");
            }
            assert_eq!(
                event("term_sent").is_some(),
                matches!(mode, "term" | "kill")
            );
            assert_eq!(event("kill_sent").is_some(), mode == "kill");
            if let Some(term) = event("term_sent") {
                assert!(term >= origin + NATURAL_LIMIT);
                assert!(event("natural_phase_ended").unwrap() <= term);
            }
            // The planned maximum grace is enforced by the absolute phase
            // deadline tests. Actual dispatch lateness is evidence, not another
            // full second of grace or a guessed scheduling tolerance.
            let grace_deadline = event("term_sent")
                .map(|at| (at + TERM_LIMIT).min(origin + NATURAL_LIMIT + TERM_LIMIT));
            if mode == "kill" {
                assert!(event("kill_sent").unwrap() >= grace_deadline.unwrap());
            }
            let ids: Vec<i32> = std::fs::read_to_string(&identities)
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            assert_eq!(ids.len(), 2);
            // Supplementary only: after root reap these numeric identifiers no
            // longer carry authority. A present identifier may have been reused.
            // Never use these observations to decide production correctness.
            let mut supplementary = std::collections::BTreeMap::new();
            for (name, pid) in [
                ("group", -ids[0]),
                ("root", ids[0]),
                ("descendant_or_orphan", ids[1]),
            ] {
                let observation = loop {
                    let rc = unsafe { libc::kill(pid, 0) };
                    let error = (rc < 0).then(std::io::Error::last_os_error);
                    let now = Instant::now();
                    if error.as_ref().and_then(std::io::Error::raw_os_error) == Some(libc::ESRCH)
                        && now < deadline
                    {
                        break serde_json::json!({"status":"numeric_identifier_absent",
                            "microseconds_from_cleanup_start":now.duration_since(origin).as_micros()});
                    }
                    if error.is_some() || now >= deadline {
                        break serde_json::json!({"status":"unavailable",
                            "reason":"post-ownership identity ambiguous or observation bound exhausted"});
                    }
                    std::thread::sleep(
                        Duration::from_millis(1).min(deadline.saturating_duration_since(now)),
                    );
                };
                supplementary.insert(name, observation);
            }
            let offsets = owner
                .timing
                .iter()
                .filter(|(_, at)| *at >= root_exit)
                .map(|(name, at)| (*name, at.duration_since(root_exit).as_micros()))
                .collect::<std::collections::BTreeMap<_, _>>();
            eprintln!(
                "AG4_PROXY_TIMING {}",
                serde_json::json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
                    "sample":sample,"mode":mode,"microseconds_from_root_exit_observation":offsets,
                    "term_grace_deadline_us_from_cleanup_start":grace_deadline.map(|at| at.duration_since(origin).as_micros()),
                    "qualification_only_non_authoritative":supplementary})
            );
            std::fs::remove_file(ready).unwrap();
            std::fs::remove_file(identities).unwrap();
        }
    }
    owner.finish().unwrap();
}

// Exact installed AWS CLI, loopback-only proxy seam, synthetic credentials.
// No AWS service call: all endpoint/destination inputs are explicit and the
// test DNS is localhost. Production descriptor validation rejects localhost.
fn eice_cli_local(mut owner: Owner) {
    use std::net::TcpListener;
    let cli = PathBuf::from(
        std::env::var_os("BORROWSER_AG4_TEST_AWS_CLI").expect("pinned CLI path required"),
    );
    crate::aws::eice::cli_identity(&cli).unwrap();
    let environment = crate::aws::config::AwsEnvironment::from_lookup(|key| match key {
        "AWS_ACCESS_KEY_ID" => Some("synthetic-access".into()),
        "AWS_SECRET_ACCESS_KEY" => Some("synthetic-secret".into()),
        "AWS_SESSION_TOKEN" => Some("synthetic-token".into()),
        _ => None,
    })
    .unwrap();
    let mut version = Spec::new(&cli, ["--version"]);
    environment.apply(&mut version);
    let result = owner.run(version).unwrap();
    assert_eq!(result.code, 0);
    assert!(String::from_utf8_lossy(&result.stdout).starts_with("aws-cli/2.37.13 "));
    let argv = [
        "--region",
        "eu-west-1",
        "--no-cli-pager",
        "--no-cli-auto-prompt",
        "ec2-instance-connect",
        "open-tunnel",
        "--instance-connect-endpoint-id",
        "eice-0123456789abcdef0",
        "--instance-connect-endpoint-dns-name",
        "localhost",
        "--private-ip-address",
        "10.77.0.10",
        "--remote-port",
        "22",
        "--max-tunnel-duration",
        "0",
    ];
    for prompt in ["on", "on-partial"] {
        let mut spec = Spec::new(&cli, argv);
        environment.apply(&mut spec);
        // Deliberately hostile input in this test only; production excludes it.
        spec.env.push(("AWS_CLI_AUTO_PROMPT".into(), prompt.into()));
        spec.failure_stderr_limit = 2048;
        spec.timeout = Duration::from_secs(10);
        let result = owner.run(spec).unwrap();
        assert_ne!(result.code, 0);
        let text = String::from_utf8_lossy(&result.stderr);
        assert!(text.contains("Invalid max connection timeout"), "{text}");
        assert!(!text.to_ascii_lowercase().contains("prompt"));
        assert!(result.stdout.is_empty());
    }
    for failure in ["proxy", "tls"] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let mode = failure.to_owned();
        let server = std::thread::spawn(move || {
            let mut poll = libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(unsafe { libc::poll(&mut poll, 1, 15000) }, 1);
            let (mut stream, peer) = listener.accept().unwrap();
            assert!(peer.ip().is_loopback());
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() <= 8192);
            }
            assert!(request.starts_with(b"CONNECT localhost:443 "));
            if mode == "proxy" {
                stream
                    .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                    .unwrap();
            } else {
                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .unwrap();
                let mut handshake = [0u8; 4096];
                let count = stream.read(&mut handshake).unwrap();
                assert!(count > 0);
                assert_eq!(
                    handshake[0], 22,
                    "expected TLS handshake, never unverified plaintext WebSocket"
                );
                stream.write_all(b"not a TLS server\r\n").unwrap();
            }
        });
        let mut argv = argv.map(str::to_owned);
        argv[15] = "10".into();
        let mut spec = Spec::new(&cli, argv);
        environment.apply(&mut spec);
        spec.env
            .push(("HTTP_PROXY".into(), format!("http://{address}").into()));
        spec.timeout = Duration::from_secs(12);
        spec.failure_stderr_limit = 2048;
        let start = Instant::now();
        let result = owner.run(spec);
        server.join().unwrap();
        let diagnostic = match result {
            Ok(out) => {
                assert_ne!(out.code, 0);
                String::from_utf8(out.stderr).unwrap()
            }
            Err(e) => {
                assert!(e.cleanup.is_none(), "{e}");
                e.to_string()
            }
        };
        assert!(!diagnostic.is_empty());
        for secret in [
            "synthetic-access",
            "synthetic-secret",
            "synthetic-token",
            "X-Amz-Signature=",
        ] {
            assert!(!diagnostic.contains(secret));
        }
        eprintln!(
            "pinned EICE CLI local {failure} failure safely quiesced in {:?}: {diagnostic}",
            start.elapsed()
        );
    }
    owner.finish().unwrap();
}

#[test]
fn signal_diagnostics_preserve_errno_and_absence_is_specific() {
    for signal in ["SIGTERM", "SIGKILL"] {
        assert!(group_signal_result(signal, None).is_ok());
        assert!(
            group_signal_result(signal, Some(std::io::Error::from_raw_os_error(libc::ESRCH)))
                .is_ok()
        );
        for errno in [libc::EPERM, libc::EINVAL, libc::EACCES] {
            let error = std::io::Error::from_raw_os_error(errno);
            let text = error.to_string();
            let diagnostic = group_signal_result(signal, Some(error)).unwrap_err();
            assert!(diagnostic.contains(signal));
            assert!(diagnostic.contains(&text));
            assert!(diagnostic.contains(&format!("raw errno=Some({errno})")));
        }
    }
}

// Explicit native qualification: the exact pinned client, loopback only, no
// authentication or AWS calls. A bound listener synchronizes acceptance; EOF
// from the client ends the server, without a guessed sleep or banner race.
fn ssh_banner_timeout(mut owner: Owner) {
    use std::net::TcpListener;
    let ssh = std::env::var_os("BORROWSER_AG4_TEST_SSH").expect("pinned OpenSSH client required");
    let version = owner.run(Spec::new(&ssh, ["-V"])).unwrap();
    let pins: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../aws/toolchain.json")).unwrap();
    assert_eq!(version.code, 0);
    let version = String::from_utf8_lossy(&version.stderr);
    assert!(version.starts_with(&format!("OpenSSH_{}", pins["openssh"].as_str().unwrap())));
    eprintln!("Pinned SSH client: {version}");
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    // Nonblocking accept and a channel bound ensure a failed client launch
    // cannot strand the listener. poll synchronizes on actual socket readiness.
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let mut poll = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut poll, 1, 15000) }, 1);
        let (mut stream, peer) = listener.accept().unwrap();
        assert!(peer.ip().is_loopback());
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let mut bytes = vec![];
        loop {
            let mut part = [0; 256];
            let count = stream.read(&mut part).unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&part[..count]);
            assert!(bytes.len() <= 255, "no authentication expected");
        }
        assert!(bytes.starts_with(b"SSH-2.0-"));
        bytes
    });
    let port = port.to_string();
    let mut spec = Spec::new(
        ssh,
        [
            "-F",
            "/dev/null",
            "-T",
            "-p",
            &port,
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "IdentityAgent=none",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "GlobalKnownHostsFile=/dev/null",
            "-o",
            "ProxyCommand=none",
            "-o",
            "ProxyJump=none",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "HostKeyAlgorithms=ssh-ed25519",
            "ag4@127.0.0.1",
            "cat",
        ],
    );
    spec.clear_env = true;
    spec.input = vec![b'x'; 1024 * 1024];
    spec.failure_stderr_limit = 2048;
    let start = Instant::now();
    let result = owner.run(spec);
    let client_banner = server.join().unwrap();
    eprintln!(
        "Client banner bytes={}; owner result={result:?}; elapsed={:?}",
        client_banner.len(),
        start.elapsed()
    );
    assert!(start.elapsed() < Duration::from_secs(26));
    let failure = result.unwrap_err();
    assert!(
        failure.primary.contains("command input:")
            || failure
                .primary
                .contains("command exited before accepting its complete input")
    );
    assert!(failure.primary.contains("banner exchange"), "{failure}");
    assert!(failure.cleanup.is_none(), "{failure}");
    owner.finish().unwrap();
    eprintln!("Pinned SSH banner-timeout command owner finished; no unresolved group or children");
}

fn aws_environment(mut owner: Owner) {
    use crate::aws::{
        cloudformation::Aws,
        config::AwsEnvironment,
        state::{Creation, Record, Store},
        template,
    };
    use std::{fs, os::unix::fs::PermissionsExt};
    let work = tempfile::tempdir().unwrap();
    let executable = work.path().join("aws-helper");
    let environment_file = work.path().join("child-environment");
    let failure_file = work.path().join("failure");
    let homes_file = work.path().join("homes");
    fs::write(&executable, format!("#!/bin/sh\nset -eu\n/usr/bin/env > '{}'\ntest ! -s \"$AWS_CONFIG_FILE\"\ntest ! -s \"$AWS_SHARED_CREDENTIALS_FILE\"\ntest ! -s \"$BOTO_CONFIG\"\ncase $1 in\n--version) printf 'aws-cli/2.37.13 synthetic\\n';;\nsts) printf '{{\"Account\":\"123456789012\"}}';;\n*) /bin/cat '{}/'$2.json;;\nesac\n", environment_file.display(), work.path().display())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let helper = fs::read_to_string(&executable).unwrap().replace(
        "case $1 in",
        &format!(
            "if [ -e '{}' ]; then /bin/cat '{}' >&2; exit 1; fi\ncase $1 in",
            failure_file.display(),
            failure_file.display()
        ),
    );
    let helper = helper.replace("case $1 in", &format!(
        "test -d \"$HOME\"\ntest -z \"$(/bin/ls -A \"$HOME\")\"\nprintf '%s %s\\n' \"$1\" \"$HOME\" >> '{}'\ncase $1 in",
        homes_file.display()));
    fs::write(&executable, helper).unwrap();
    let verify = || {
        let text = fs::read_to_string(&environment_file).unwrap();
        for expected in [
            "AWS_ACCESS_KEY_ID=synthetic-access",
            "AWS_SECRET_ACCESS_KEY=synthetic-secret",
            "AWS_SESSION_TOKEN=synthetic-token",
            "AWS_CONFIG_FILE=/dev/null",
            "AWS_SHARED_CREDENTIALS_FILE=/dev/null",
            "BOTO_CONFIG=/dev/null",
            "AWS_EC2_METADATA_DISABLED=true",
            "AWS_IGNORE_CONFIGURED_ENDPOINT_URLS=true",
        ] {
            assert!(
                text.lines().any(|line| line == expected),
                "missing {expected}"
            );
        }
        for line in text.lines() {
            let (key, _) = line.split_once('=').unwrap();
            assert!(
                matches!(
                    key,
                    "PATH"
                        | "HOME"
                        | "LC_ALL"
                        | "SHELL"
                        | "PWD"
                        | "AWS_ACCESS_KEY_ID"
                        | "AWS_SECRET_ACCESS_KEY"
                        | "AWS_SESSION_TOKEN"
                        | "AWS_CONFIG_FILE"
                        | "AWS_SHARED_CREDENTIALS_FILE"
                        | "BOTO_CONFIG"
                        | "AWS_EC2_METADATA_DISABLED"
                        | "AWS_IGNORE_CONFIGURED_ENDPOINT_URLS"
                        | "AWS_MAX_ATTEMPTS"
                        | "AWS_RETRY_MODE"
                        | "AWS_PAGER"
                        | "SHLVL"
                        | "_"
                        | "AWS_CA_BUNDLE"
                ),
                "inherited {key}"
            );
        }
    };
    let environment = AwsEnvironment::load().unwrap();
    // Version and ordinary calls share the same live empty home.
    let mut version = Spec::new(&executable, ["--version"]);
    environment.apply(&mut version);
    assert_eq!(owner.run(version).unwrap().code, 0);
    filesystem_probe(
        &mut owner,
        &environment,
        "/usr/bin/python3".into(),
        false,
        work.path(),
    );
    Aws {
        commands: &mut owner,
        executable: executable.clone(),
        environment,
        deadline: Duration::from_secs(20),
        cleanup: false,
    }
    .account("123456789012")
    .unwrap();
    verify();
    let homes = fs::read_to_string(&homes_file).unwrap();
    let homes: Vec<_> = homes
        .lines()
        .map(|line| line.split_once(' ').unwrap())
        .collect();
    assert_eq!(homes[0].0, "--version");
    assert_eq!(homes[0].1, homes[1].1);
    assert!(!std::path::Path::new(homes[0].1).exists());
    let certificate = work.path().join("public-ca.pem");
    fs::write(
        &certificate,
        include_bytes!("../../../../net/testdata/test-root-ca.pem"),
    )
    .unwrap();
    let environment = AwsEnvironment::from_lookup(|key| {
        if key == "AWS_CA_BUNDLE" {
            Some(certificate.as_os_str().to_owned())
        } else {
            std::env::var_os(key)
        }
    })
    .unwrap();
    Aws {
        commands: &mut owner,
        executable: executable.clone(),
        environment,
        deadline: Duration::from_secs(20),
        cleanup: false,
    }
    .account("123456789012")
    .unwrap();
    verify();
    assert!(
        fs::read_to_string(&environment_file)
            .unwrap()
            .lines()
            .any(|line| line == format!("AWS_CA_BUNDLE={}", certificate.display()))
    );
    // The raw credential straddles the diagnostic display boundary. Redacting
    // only a truncated prefix would expose "synthe" from the secret.
    fs::write(
        &failure_file,
        format!("{}synthetic-secret", "x".repeat(1018)),
    )
    .unwrap();
    let error = Aws {
        commands: &mut owner,
        executable: executable.clone(),
        environment: AwsEnvironment::load().unwrap(),
        deadline: Duration::from_secs(20),
        cleanup: false,
    }
    .account("123456789012")
    .unwrap_err();
    assert!(error.contains("exited 1"));
    assert!(
        !error.contains("synthe"),
        "credential prefix escaped redaction"
    );
    fs::remove_file(&failure_file).unwrap();
    owner.finish().unwrap();
    let t = template::json(include_bytes!("../../../aws/tests/synthesized.json")).unwrap();
    let store = Store::open(&work.path().join("operation"), true).unwrap();
    let mut record = Record::new(
        &template::binding(&t).unwrap(),
        template::digest(&t).unwrap(),
    );
    record.creation = Creation::Bound;
    record.stack_id = Some(crate::aws::tests::stack_id());
    for (action, response) in [
        (
            "describe-stacks",
            crate::aws::tests::stack("DELETE_COMPLETE"),
        ),
        ("get-template", serde_json::json!({"TemplateBody":t})),
        (
            "list-stack-resources",
            crate::aws::tests::resources("DELETE_COMPLETE", true),
        ),
        ("describe-instances", serde_json::json!({"Reservations":[]})),
        ("describe-volumes", serde_json::json!({"Volumes":[]})),
    ] {
        fs::write(
            work.path().join(format!("{action}.json")),
            serde_json::to_vec(&response).unwrap(),
        )
        .unwrap();
    }
    store.save(&record).unwrap();
    drop(store);
    let mut diagnostics = vec![];
    assert_eq!(
        crate::aws::cleanup(
            &[
                "--operation".into(),
                work.path().join("operation").display().to_string(),
                "--aws-cli".into(),
                executable.display().to_string()
            ],
            &mut diagnostics
        )
        .unwrap(),
        0,
        "{}",
        String::from_utf8_lossy(&diagnostics)
    );
    verify();
    let homes = fs::read_to_string(&homes_file).unwrap();
    let recovery = homes
        .lines()
        .skip_while(|line| !line.starts_with("--version "))
        .skip(1)
        .skip_while(|line| !line.starts_with("--version "))
        .collect::<Vec<_>>();
    assert!(
        recovery.len() > 2,
        "version and cleanup calls reached real child"
    );
    let home = recovery[0].split_once(' ').unwrap().1;
    assert!(
        recovery
            .iter()
            .all(|line| line.split_once(' ').unwrap().1 == home)
    );
    assert!(
        !std::path::Path::new(home).exists(),
        "recovery home removed after commands"
    );
    assert!(AwsEnvironment::from_lookup(|_| None).is_err());
    let e = AwsEnvironment::load().unwrap();
    assert!(
        !e.redact("synthetic-access synthetic-secret synthetic-token")
            .contains("synthetic-")
    );
}

// Runs through the real command-construction/launch path, not a Spec-only test.
fn filesystem_probe(
    owner: &mut Owner,
    environment: &crate::aws::config::AwsEnvironment,
    python: OsString,
    library: bool,
    work: &std::path::Path,
) {
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("aws/tests/aws_home_probe.py");
    let mut args = vec![
        "-B".into(), // Keep Apple's system Python bytecode cache out of the probe home.
        script.into_os_string(),
        work.join("fake-account-home").into_os_string(),
    ];
    if library {
        args.push("vendored-botocore".into());
    }
    let mut spec = Spec::new(python, args);
    environment.apply(&mut spec);
    let output = owner.run(spec).unwrap();
    assert_eq!(
        output.code,
        0,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pinned_cli_rejects_external_command(output: &Output) -> bool {
    output.code == 252
        && output.stdout.is_empty()
        && String::from_utf8_lossy(&output.stderr).lines().any(|line| {
            // Normalize diagnostic prose only; the rejected command stays exact.
            line.split_once("'ag4external'").is_some_and(|(prefix, _)| {
                prefix
                    .to_ascii_lowercase()
                    .ends_with("argument command: found invalid choice ")
            })
        })
}

#[test]
fn pinned_cli_external_command_rejection_is_specific() {
    let observed = "aws: [ERROR]: An error occurred (ParamValidation): argument command: Found invalid choice 'ag4external'";
    for (name, code, stdout, stderr, expected) in [
        ("observed", 252, "", observed, true),
        (
            "capitalization",
            252,
            "",
            "AWS: [error]: ARGUMENT COMMAND: FOUND INVALID CHOICE 'ag4external'",
            true,
        ),
        ("success", 0, "", observed, false),
        (
            "configuration",
            252,
            "",
            "aws: [ERROR]: The config profile (ag4external) could not be found",
            false,
        ),
        (
            "another command",
            252,
            "",
            "argument command: Found invalid choice 'sts'",
            false,
        ),
        (
            "another argument",
            252,
            "",
            "argument --profile: Found invalid choice 'ag4external'",
            false,
        ),
        ("unexpected exit", 255, "", observed, false),
        (
            "skeleton output",
            252,
            "{\"ExternalMarker\":\"\"}",
            observed,
            false,
        ),
        (
            "command prefix",
            252,
            "",
            "argument command: Found invalid choice 'ag4external-extra'",
            false,
        ),
        (
            "command case",
            252,
            "",
            "argument command: Found invalid choice 'AG4EXTERNAL'",
            false,
        ),
    ] {
        let output = Output {
            code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        };
        assert_eq!(
            pinned_cli_rejects_external_command(&output),
            expected,
            "{name}"
        );
    }
}

fn aws_model_resolution(mut owner: Owner, mode: &str) {
    use crate::aws::config::AwsEnvironment;
    let work = tempfile::tempdir().unwrap();
    let environment = AwsEnvironment::from_lookup(|key| match key {
        "AWS_ACCESS_KEY_ID" => Some("synthetic-access".into()),
        "AWS_SECRET_ACCESS_KEY" => Some("synthetic-secret".into()),
        _ => None,
    })
    .unwrap();
    if mode == "aws-model-library" {
        let python = std::env::var_os("BORROWSER_AG4_TEST_AWS_PYTHON")
            .expect("vendored CLI Python required");
        filesystem_probe(&mut owner, &environment, python, true, work.path());
    } else {
        let cli = std::env::var_os("BORROWSER_AG4_TEST_AWS_CLI").expect("pinned CLI required");
        let mut version = Spec::new(&cli, ["--version"]);
        environment.apply(&mut version);
        let output = owner.run(version).unwrap();
        let pins: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../aws/toolchain.json")).unwrap();
        assert_eq!(output.code, 0);
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .starts_with(&format!("aws-cli/{} ", pins["aws_cli"].as_str().unwrap())),
            "pinned CLI required; substitute versions do not qualify"
        );
        eprintln!(
            "AWS CLI version: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
        filesystem_probe(
            &mut owner,
            &environment,
            "/usr/bin/python3".into(),
            false,
            work.path(),
        );
        eprintln!("AWS model filesystem/HOME probe passed");
        // Generate skeleton only: no endpoint calls, no real credentials. Prove
        // the synthetic service loads from a fake home, then cannot enter AG4.
        for isolated in [false, true] {
            let mut spec = Spec::new(
                &cli,
                ["ag4external", "probe", "--generate-cli-skeleton", "input"],
            );
            environment.apply(&mut spec);
            if !isolated {
                spec.env.retain(|(key, _)| key != "HOME");
                spec.env.push((
                    "HOME".into(),
                    work.path().join("fake-account-home").into_os_string(),
                ));
            }
            let output = owner.run(spec).unwrap();
            // Retain the actual bounded owner result, using only synthetic inputs.
            eprintln!(
                "AWS model control isolated={isolated}: code={} stdout={:?} stderr={:?}",
                output.code,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if isolated {
                assert!(pinned_cli_rejects_external_command(&output));
            } else {
                assert_eq!(
                    output.code,
                    0,
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(String::from_utf8_lossy(&output.stdout).contains("ExternalMarker"));
            }
        }
    }
    owner.finish().unwrap();
    eprintln!("AWS model command owner finished");
}
