//! Never part of ordinary tests: feature + ignored + explicit environment opt-in.
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn invoke(
    controller: &Path,
    args: &[std::ffi::OsString],
    fault: Option<&str>,
) -> (i32, Vec<u8>, Vec<u8>) {
    let mut stdout = tempfile::tempfile().unwrap();
    let mut stderr = tempfile::tempfile().unwrap();
    let mut command = Command::new(controller);
    command
        .args(args)
        .stdout(Stdio::from(stdout.try_clone().unwrap()))
        .stderr(Stdio::from(stderr.try_clone().unwrap()));
    for name in [
        "BORROWSER_AG4_ACCEPTED_CREATE_CRASH",
        "BORROWSER_AG4_CANCEL_AFTER_CREATE",
        "BORROWSER_AG4_ACCEPTED_DELETE_CRASH",
    ] {
        command.env_remove(name);
    }
    if let Some(name) = fault {
        command.env(name, "1");
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3000);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline
            || stdout.metadata().unwrap().len() > 256 * 1024
            || stderr.metadata().unwrap().len() > 256 * 1024
        {
            // Ordinary interruption requests the production controller's cleanup.
            unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
            let cleanup_deadline = Instant::now() + Duration::from_secs(920);
            while Instant::now() < cleanup_deadline {
                if child.try_wait().unwrap().is_some() {
                    panic!(
                        "acceptance controller exceeded its bound; inspect retained operation state"
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            child.kill().unwrap();
            let _ = child.wait();
            panic!(
                "acceptance interrupted twice; operation state MUST be recovered with --aws-cleanup"
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut out = vec![];
    let mut err = vec![];
    stdout.seek(SeekFrom::Start(0)).unwrap();
    stderr.seek(SeekFrom::Start(0)).unwrap();
    stdout.take(256 * 1024 + 1).read_to_end(&mut out).unwrap();
    stderr.take(256 * 1024 + 1).read_to_end(&mut err).unwrap();
    assert!(out.len() <= 256 * 1024 && err.len() <= 256 * 1024);
    (status.code().unwrap_or(128), out, err)
}

#[test]
#[ignore = "billable AWS acceptance requires reviewed configuration and explicit opt-in"]
fn real_capture_and_interrupted_create_recovery() {
    assert_eq!(
        std::env::var("BORROWSER_AG4_AWS_ACCEPTANCE").as_deref(),
        Ok("1"),
        "explicit AWS acceptance opt-in required"
    );
    let bundle = std::path::PathBuf::from(
        std::env::var_os("BORROWSER_AG4_ACCEPTANCE_BUNDLE").expect("bundle required"),
    );
    let config = std::path::PathBuf::from(
        std::env::var_os("BORROWSER_AG4_ACCEPTANCE_CONFIG").expect("reviewed config required"),
    );
    let state = std::path::PathBuf::from(
        std::env::var_os("BORROWSER_AG4_ACCEPTANCE_STATE").expect("private state root required"),
    );
    assert!(
        fs::read_dir(&state).unwrap().next().is_none(),
        "use an empty private acceptance state root; records are retained"
    );
    let operator = super::config::Config::load(&config).unwrap();
    let controller = bundle.join("borrowser-conformance-controller");
    let mut previous = std::collections::BTreeSet::new();
    let mut cascade_report = None;
    for (selection, fault, count, expected_exit) in [
        (Some("canvas/cascade"), None, 1, 0),
        (Some("canvas/cascade"), None, 1, 0),
        (None, None, 2, 0),
        (
            Some("canvas/cascade"),
            Some("BORROWSER_AG4_ACCEPTED_CREATE_CRASH"),
            0,
            86,
        ),
        (
            Some("canvas/cascade"),
            Some("BORROWSER_AG4_CANCEL_AFTER_CREATE"),
            0,
            2,
        ),
        (
            Some("canvas/cascade"),
            Some("BORROWSER_AG4_ACCEPTED_DELETE_CRASH"),
            0,
            87,
        ),
    ] {
        let mut args = vec![
            "--compare-aws".into(),
            "--config".into(),
            config.as_os_str().to_owned(),
            "--artifact-dir".into(),
            bundle.as_os_str().to_owned(),
            "--state-dir".into(),
            state.as_os_str().to_owned(),
        ];
        if let Some(id) = selection {
            args.push(id.into());
        }
        let (code, out, err) = invoke(&controller, &args, fault);
        eprintln!("{}", String::from_utf8_lossy(&err));
        let paths: Vec<_> = fs::read_dir(&state)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| !previous.contains(p))
            .collect();
        assert_eq!(
            paths.len(),
            1,
            "one operation per invocation; diagnostics above"
        );
        let operation = &paths[0];
        previous.insert(operation.clone());
        let mut record = super::state::Store::open(operation, false)
            .unwrap()
            .read()
            .unwrap();
        let before_recovery = record.clone();
        // Cleanup-only has just a copied controller and durable state. Neither
        // bundle nor CDK/configuration paths are passed to this invocation.
        if record.cleanup != super::state::Cleanup::Confirmed {
            let temporary = tempfile::tempdir().unwrap();
            let cleanup_controller = temporary.path().join("controller");
            fs::copy(&controller, &cleanup_controller).unwrap();
            fs::set_permissions(&cleanup_controller, fs::Permissions::from_mode(0o700)).unwrap();
            let cleanup_args = vec![
                "--aws-cleanup".into(),
                "--operation".into(),
                operation.as_os_str().to_owned(),
                "--aws-cli".into(),
                operator.aws_cli.as_os_str().to_owned(),
            ];
            let (cleanup_code, _, diagnostic) = invoke(&cleanup_controller, &cleanup_args, None);
            assert_eq!(
                cleanup_code,
                0,
                "recovery remains pending at {}: {}",
                operation.display(),
                String::from_utf8_lossy(&diagnostic)
            );
            record = super::state::Store::open(operation, false)
                .unwrap()
                .read()
                .unwrap();
        }
        assert_eq!(record.cleanup, super::state::Cleanup::Confirmed);
        assert_eq!(
            code,
            expected_exit,
            "controller must match the acceptance source and feature; {}",
            String::from_utf8_lossy(&err)
        );
        if fault.is_some() {
            assert!(out.is_empty());
        }
        if expected_exit == 86 {
            assert_eq!(before_recovery.creation, super::state::Creation::Submitted);
            assert!(before_recovery.stack_id.is_none());
        }
        assert_eq!(
            fs::read_dir(&state).unwrap().count(),
            previous.len(),
            "recovery must not create another operation"
        );
        if fault.is_none() {
            assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
            let report: serde_json::Value = serde_json::from_slice(&out).unwrap();
            assert_eq!(report["summary"]["equal_fields"], count);
            if selection == Some("canvas/cascade") {
                if let Some(previous) = &cascade_report {
                    assert_eq!(previous, &out);
                }
                cascade_report = Some(out);
            }
        }
        eprintln!(
            "qualified run={} stack={} cleanup={:?}",
            record.run_id,
            record.stack_id.as_deref().unwrap(),
            record.cleanup
        );
    }
}
