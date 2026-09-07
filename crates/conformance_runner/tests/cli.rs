use std::process::{Command, Output};

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_conformance-runner"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn legacy_argument_rejections_keep_exact_diagnostic() {
    for args in [
        vec!["--unknown"],
        vec!["--check", "--check"],
        vec!["--css", "--css"],
        vec!["--rendering", "--rendering"],
        vec!["--css", "--rendering"],
        vec!["--check", "aggregate"],
        vec!["--help"],
        vec!["parser"],
    ] {
        let out = invoke(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stdout.is_empty());
        assert_eq!(
            out.stderr,
            b"usage: conformance-runner [--css|--rendering] [--check]\n"
        );
    }
}

#[test]
fn legacy_direct_modes_and_check_order_are_unchanged() {
    for (flag, feature, enabled) in [
        (None, "html-parser", cfg!(feature = "html-parser")),
        (Some("--css"), "css", cfg!(feature = "css")),
        (
            Some("--rendering"),
            "rendering",
            cfg!(feature = "rendering"),
        ),
    ] {
        let args: Vec<_> = flag.into_iter().collect();
        let ordinary = invoke(&args);
        let mut checked = args.clone();
        checked.push("--check");
        let first = invoke(&checked);
        checked.reverse();
        let second = invoke(&checked);
        if enabled {
            assert_eq!(ordinary.status.code(), Some(0));
            assert_eq!(first.status.code(), Some(0));
            assert!(ordinary.stderr.is_empty());
            assert!(!ordinary.stdout.is_empty());
            assert_eq!(ordinary.stdout, expected_direct(feature));
        } else {
            assert_eq!(ordinary.status.code(), Some(2));
            assert!(ordinary.stdout.is_empty());
            assert_eq!(
                ordinary.stderr,
                format!("conformance runner adapter feature is not enabled: {feature}\n")
                    .as_bytes()
            );
        }
        assert_eq!(ordinary.stdout, first.stdout);
        assert_eq!(ordinary.stderr, first.stderr);
        assert_eq!(first.stdout, second.stdout);
        assert_eq!(first.stderr, second.stderr);
        assert_eq!(first.status.code(), second.status.code());
    }
}

fn expected_direct(feature: &str) -> Vec<u8> {
    let _root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match feature {
        #[cfg(feature = "html-parser")]
        "html-parser" => conformance_runner::build_report(
            conformance_runner::run_repository_parser_cases(&_root)
                .unwrap()
                .cases(),
        )
        .unwrap(),
        #[cfg(feature = "css")]
        "css" => conformance_runner::build_css_report(
            conformance_runner::run_repository_css_cases(&_root)
                .unwrap()
                .cases(),
        )
        .unwrap(),
        #[cfg(feature = "rendering")]
        "rendering" => conformance_runner::build_rendering_report(
            conformance_runner::run_repository_rendering_cases(&_root)
                .unwrap()
                .cases(),
        )
        .unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn aggregate_argument_matrix_rejects_before_feature_check() {
    let cases = [
        vec![],
        vec!["unknown"],
        vec!["summary"],
        vec!["detail"],
        vec!["summary", "--lane"],
        vec!["summary", "--lane", "all"],
        vec!["summary", "--lane", "normal-ci", "--check", "--check"],
        vec!["summary", "--lane", "normal-ci", "--lane", "local-extended"],
        vec![
            "detail",
            "--lane",
            "normal-ci",
            "--external-evidence",
            "repository",
        ],
        vec!["baseline", "--lane", "normal-ci"],
        vec![
            "baseline",
            "--lane",
            "normal-ci",
            "--external-evidence",
            "other",
        ],
        vec![
            "baseline",
            "--lane",
            "normal-ci",
            "--external-evidence",
            "repository",
            "--compare-dom-test",
            "INVALID ID",
        ],
        vec![
            "summary",
            "--lane",
            "normal-ci",
            "--compare-dom-test",
            "dom-tree-basic-document",
        ],
        vec!["summary", "--lane", "normal-ci", "--css"],
        vec!["trend"],
        vec!["trend", "--check"],
        vec!["trend", "--lane", "normal-ci"],
    ];
    for case in cases {
        let mut args = vec!["aggregate"];
        args.extend(case);
        let out = invoke(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {:?}", out.stderr);
        assert!(out.stdout.is_empty());
        assert!(
            out.stderr
                .starts_with(b"usage: conformance-runner aggregate")
        );
    }
}

#[test]
fn every_lane_requires_explicit_valid_spelling() {
    for mode in ["summary", "detail"] {
        for lane in [
            "normal-ci",
            "local-extended",
            "scheduled-extended",
            "manual-extended",
        ] {
            let out = invoke(&["aggregate", mode, "--lane", lane, "--check"]);
            assert_eq!(
                out.status.code(),
                Some(if cfg!(feature = "aggregate") { 0 } else { 3 }),
                "{mode} {lane}: {:?}",
                out.stderr
            );
            #[cfg(feature = "aggregate")]
            {
                use conformance_runner::*;
                let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
                let run = run_repository_aggregate(
                    &root,
                    AggregateExecutionRequest {
                        lane: conformance_test_support::LanePolicyScope::parse(lane).unwrap(),
                    },
                )
                .unwrap();
                let expected = if mode == "summary" {
                    build_aggregate_summary_v1(&run)
                } else {
                    build_aggregate_detail_v1(&run)
                }
                .unwrap();
                assert_eq!(out.stdout, expected);
                if lane == "normal-ci" {
                    let golden: &[u8] = if mode == "summary" {
                        include_bytes!("data/aggregate-summary-v1.txt")
                    } else {
                        include_bytes!("data/aggregate-detail-v1.txt")
                    };
                    assert_eq!(out.stdout, golden);
                }
                assert!(out.stderr.is_empty());
            }
            #[cfg(not(feature = "aggregate"))]
            {
                assert!(out.stdout.is_empty());
                assert_eq!(out.stderr, b"aggregate operation failed: conformance runner aggregate feature is not enabled\n");
            }
        }
        for lane in [
            "",
            "NORMAL-CI",
            "normal_ci",
            "legacy",
            "all-tests",
            "compatibility",
        ] {
            let out = invoke(&["aggregate", mode, "--lane", lane]);
            assert_eq!(out.status.code(), Some(2));
            assert!(out.stdout.is_empty());
        }
    }
}

fn trend_args<'a>(
    root: &'a str,
    from: &'a str,
    old: &'a str,
    to: &'a str,
    new: &'a str,
) -> Vec<&'a str> {
    vec![
        "aggregate",
        "trend",
        "--from-root",
        root,
        "--from",
        from,
        "--from-sha256",
        old,
        "--to-root",
        root,
        "--to",
        to,
        "--to-sha256",
        new,
    ]
}

#[test]
fn trend_descriptor_matrix_and_disabled_feature() {
    let hash = "0".repeat(64);
    let args = trend_args(".", "old.bin", &hash, "new.bin", &hash);
    for i in (2..args.len()).step_by(2) {
        let mut missing = args.clone();
        missing.drain(i..i + 2);
        let out = invoke(&missing);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        let mut duplicate = args.clone();
        duplicate.extend_from_slice(&args[i..i + 2]);
        assert_eq!(invoke(&duplicate).status.code(), Some(2));
    }
    for bad in [
        "00",
        "G00000000000000000000000000000000000000000000000000000000000000000",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert_eq!(
            invoke(&trend_args(".", "old", bad, "new", &hash))
                .status
                .code(),
            Some(2)
        );
    }
    for extra in ["--check", "--unknown", "--lane"] {
        let mut invalid = args.clone();
        invalid.push(extra);
        assert_eq!(invoke(&invalid).status.code(), Some(2));
    }
    let out = invoke(&args);
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
}

#[cfg(feature = "aggregate")]
#[test]
fn baselines_are_existing_vectors_and_selection_errors_are_operations() {
    let args = [
        "aggregate",
        "baseline",
        "--lane",
        "normal-ci",
        "--external-evidence",
        "repository",
    ];
    let out = invoke(&args);
    assert_eq!(out.status.code(), Some(0), "{:?}", out.stderr);
    assert_eq!(
        out.stdout,
        include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/empty.bin")
    );
    let mut selected = args.to_vec();
    selected.extend(["--compare-dom-test", "dom-tree-basic-document"]);
    let out = invoke(&selected);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        out.stdout,
        include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/selected-zero.bin")
    );
    for id in [
        "absent",
        "css-parsing-basic-stylesheet",
        "html-tree-construction-repeated-body-unavailable",
    ] {
        *selected.last_mut().unwrap() = id;
        let out = invoke(&selected);
        assert_eq!(out.status.code(), Some(3));
        assert!(out.stdout.is_empty());
    }
}

#[cfg(feature = "aggregate")]
#[test]
fn verified_trend_changes_succeed_and_invalid_inputs_publish_nothing() {
    use external_test_provenance::sha256;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    let old = include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/empty.bin");
    let new = include_bytes!(
        "../../../tests/contract-vectors/conformance-baseline-v1/selected-evidence.bin"
    );
    std::fs::write(dir.path().join("old"), old).unwrap();
    std::fs::write(dir.path().join("new"), new).unwrap();
    let oh = sha256(old).to_hex();
    let nh = sha256(new).to_hex();
    let out = invoke(&trend_args(root, "old", &oh, "new", &nh));
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert!(out.stdout.starts_with(b"borrowser-conformance-trend-v1\0"));
    let mut malformed = old.to_vec();
    malformed[0] = b'!';
    std::fs::write(dir.path().join("bad"), &malformed).unwrap();
    let bh = sha256(&malformed).to_hex();
    for args in [
        trend_args(root, "old", &nh, "new", &nh),
        trend_args(root, "bad", &bh, "new", &nh),
        trend_args(root, "missing", &oh, "new", &nh),
        trend_args(root, "../old", &oh, "new", &nh),
    ] {
        let out = invoke(&args);
        assert_eq!(out.status.code(), Some(3));
        assert!(out.stdout.is_empty());
    }
    let local = invoke(&[
        "aggregate",
        "baseline",
        "--lane",
        "local-extended",
        "--external-evidence",
        "repository",
    ]);
    assert_eq!(local.status.code(), Some(0));
    let lh = sha256(&local.stdout).to_hex();
    std::fs::write(dir.path().join("local"), local.stdout).unwrap();
    let out = invoke(&trend_args(root, "old", &oh, "local", &lh));
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
}

#[cfg(all(unix, feature = "aggregate"))]
#[test]
fn closed_stdout_is_publication_failure() {
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_conformance-runner"))
        .args(["aggregate", "detail", "--lane", "normal-ci", "--check"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(
        out.stderr
            .starts_with(b"aggregate stdout publication failed:")
    );
}

#[test]
fn every_aggregate_option_rejects_duplicates_and_unknown_options() {
    let hash = "0".repeat(64);
    let cases = vec![
        vec!["aggregate", "summary", "--lane", "normal-ci", "--check"],
        vec!["aggregate", "detail", "--lane", "normal-ci", "--check"],
        vec![
            "aggregate",
            "baseline",
            "--lane",
            "normal-ci",
            "--external-evidence",
            "repository",
            "--compare-dom-test",
            "dom-tree-basic-document",
            "--check",
        ],
        trend_args(".", "old", &hash, "new", &hash),
    ];
    for case in cases {
        let mut unknown = case.clone();
        unknown.push("--unknown");
        let out = invoke(&unknown);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        let mut index = 2;
        while index < case.len() {
            let width = if case[index] == "--check" { 1 } else { 2 };
            let mut duplicate = case.clone();
            duplicate.extend_from_slice(&case[index..index + width]);
            let out = invoke(&duplicate);
            assert_eq!(out.status.code(), Some(2), "{duplicate:?}");
            assert!(out.stdout.is_empty());
            index += width;
        }
    }
}

#[cfg(not(feature = "aggregate"))]
#[test]
fn valid_baseline_commands_still_require_aggregate_feature() {
    let mut args = vec![
        "aggregate",
        "baseline",
        "--lane",
        "normal-ci",
        "--external-evidence",
        "repository",
    ];
    for selected in [false, true] {
        if selected {
            args.extend(["--compare-dom-test", "dom-tree-basic-document"]);
        }
        let out = invoke(&args);
        assert_eq!(out.status.code(), Some(3));
        assert!(out.stdout.is_empty());
    }
}

#[cfg(all(target_os = "linux", feature = "aggregate"))]
#[test]
fn trend_preserves_os_native_non_utf8_root_and_relative_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(OsString::from_vec(b"root-\xff".to_vec()));
    std::fs::create_dir(&root).unwrap();
    let relative = OsString::from_vec(b"baseline-\xfe".to_vec());
    let bytes = include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/empty.bin");
    std::fs::write(root.join(&relative), bytes).unwrap();
    let digest = external_test_provenance::sha256(bytes).to_hex();
    let output = Command::new(env!("CARGO_BIN_EXE_conformance-runner"))
        .args(["aggregate", "trend", "--from-root"])
        .arg(&root)
        .arg("--from")
        .arg(&relative)
        .args(["--from-sha256", &digest, "--to-root"])
        .arg(&root)
        .arg("--to")
        .arg(&relative)
        .args(["--to-sha256", &digest])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert!(
        output
            .stdout
            .starts_with(b"borrowser-conformance-trend-v1\0")
    );
}

#[cfg(unix)]
#[test]
fn textual_aggregate_fields_reject_non_utf8_before_feature_check() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let hash = "0".repeat(64);
    let cases = [
        (vec!["aggregate", "summary", "--lane", "normal-ci"], 3),
        (
            vec![
                "aggregate",
                "baseline",
                "--lane",
                "normal-ci",
                "--external-evidence",
                "repository",
                "--compare-dom-test",
                "dom-tree-basic-document",
            ],
            7,
        ),
        (trend_args(".", "old", &hash, "new", &hash), 7),
    ];
    for (args, index) in cases {
        let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
        args[index] = OsString::from_vec(vec![0xff]);
        let output = Command::new(env!("CARGO_BIN_EXE_conformance-runner"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
