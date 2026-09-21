//! Explicit native development runtime checks. These are not Phase B evidence.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod native {
    use std::process::Command;
    fn command() -> Command {
        Command::new(env!("CARGO_BIN_EXE_borrowser-host-readiness"))
    }
    fn util_args() -> [String; 3] {
        [
            "AG9G0A_UNSHARE",
            "AG9G0A_UNSHARE_SHA256",
            "AG9G0A_UNSHARE_VERSION",
        ]
        .map(|k| std::env::var(k).unwrap_or_else(|_| panic!("explicit measured {k} required")))
    }
    #[test]
    #[ignore = "explicit non-root native x86-64 Linux seccomp prerequisite"]
    fn seccomp_no_new_privs_and_enforcement() {
        let output = command().arg("host-seccomp-prerequisite").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: borrowser_host_readiness::linux::Report =
            serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report.authority,
            borrowser_host_readiness::linux::PROBE_AUTHORITY
        );
        assert!(
            report
                .observations
                .iter()
                .any(|o| o.name == "denied-syscall" && o.value == "getppid:EPERM")
        );
    }
    #[test]
    #[ignore = "explicit recorded util-linux/native rootless namespace host"]
    fn actual_util_linux_descriptor_and_namespace_prerequisite() {
        let output = command()
            .arg("host-unshare-fd-prerequisite")
            .args(util_args())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: borrowser_host_readiness::linux::Report =
            serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            report.probe,
            borrowser_host_readiness::report::HostProbe::UnshareFd
        );
    }
    #[test]
    #[ignore = "explicit recorded util-linux/native rootless namespace host"]
    fn process_inspection_without_forced_pid_reuse() {
        let output = command()
            .arg("host-process-inspection-prerequisite")
            .args(util_args())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: borrowser_host_readiness::linux::Report =
            serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            report
                .observations
                .iter()
                .any(|o| o.name == "process-inspection")
        );
    }
    #[test]
    #[ignore = "explicit non-root Linux failure-path test; no qualification"]
    fn rejects_missing_wrong_identity_and_invalid_invocation() {
        for args in [
            vec![
                "host-unshare-fd-prerequisite",
                "/missing",
                "invalid",
                "invalid",
            ],
            vec![
                "host-process-inspection-prerequisite",
                "relative",
                "invalid",
                "invalid",
            ],
            vec!["__host-namespace", "{}"],
        ] {
            let output = command().args(args).output().unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
        let mut args = util_args();
        args[1] = "0".repeat(64);
        let output = command()
            .arg("host-unshare-fd-prerequisite")
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn unsupported_host_cannot_emit_runtime_success() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_borrowser-host-readiness"))
        .arg("host-seccomp-prerequisite")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
