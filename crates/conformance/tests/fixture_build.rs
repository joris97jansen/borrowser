#[path = "../build_support/fixtures.rs"]
mod fixture_build;
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

fn declaration() -> Value {
    json!({"id":"test/canvas","input":"case.html","requires_javascript":false,
        "classification":{"kind":"runnable"},"observations":{"canvas":{"kind":"compare"}},
        "expectation":{"kind":"match"},"expected":{"kind":"canvas","rgb8":[18,52,86]}})
}
fn write_case(root: &Path, stem: &str, mut data: Value) {
    fs::create_dir_all(root.join("fixtures")).unwrap();
    data["input"] = json!(format!("{stem}.html"));
    fs::write(
        root.join(format!("fixtures/{stem}.html")),
        "<!doctype html><html></html>",
    )
    .unwrap();
    fs::write(
        root.join(format!("fixtures/{stem}.fixture.json")),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
}
#[test]
fn missing_and_empty_roots_are_discovery_errors() {
    let temp = tempfile::tempdir().unwrap();
    assert!(
        fixture_build::generate(temp.path())
            .unwrap_err()
            .contains("fixtures/")
    );
    fs::create_dir(temp.path().join("fixtures")).unwrap();
    assert!(
        fixture_build::generate(temp.path())
            .unwrap_err()
            .contains("no *.fixture.json")
    );
}
#[test]
fn discovery_order_is_identity_order_independent_of_creation_order_and_root() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for (root, names) in [(a.path(), ["z", "a"]), (b.path(), ["a", "z"])] {
        for name in names {
            let mut data = declaration();
            data["id"] = json!(format!("case/{name}"));
            write_case(root, name, data);
        }
    }
    let first = fixture_build::generate(a.path()).unwrap();
    assert_eq!(first, fixture_build::generate(b.path()).unwrap());
    assert!(first.find("case/a").unwrap() < first.find("case/z").unwrap());
}
#[test]
fn malformed_declarations_fail_attributably() {
    let mutations: Vec<(&str, Value)> = vec![
        ("classification", json!({"kind":"expected_failure"})),
        (
            "classification",
            json!({"kind":"runnable","reason":"unexpected"}),
        ),
        ("classification", json!({"kind":"unsupported","reason":""})),
        (
            "classification",
            json!({"kind":"skipped","reason":"intentional"}),
        ),
        ("requires_javascript", json!(true)),
        ("requires_javascript", Value::Null),
        (
            "expectation",
            json!({"kind":"known_failure","reason":" \t"}),
        ),
        ("expectation", Value::Null),
        ("expected", Value::Null),
        ("expected", json!({"kind":"canvas","rgb8":[256,0,0]})),
        ("observations", json!({})),
        ("observations", json!({"unknown":{"kind":"compare"}})),
        (
            "observations",
            json!({"canvas":{"kind":"unsupported","reason":" "}}),
        ),
        (
            "observations",
            json!({"styles":[{"target":"x","property":"typo","selection":{"kind":"compare"}}]}),
        ),
        ("id", json!("../escape")),
        ("id", json!("Case/mixed")),
        ("unknown", json!(true)),
    ];
    for (field, value) in mutations {
        let temp = tempfile::tempdir().unwrap();
        let mut data = declaration();
        data[field] = value;
        write_case(temp.path(), "case", data);
        let error = fixture_build::generate(temp.path()).unwrap_err();
        assert!(
            error.contains("fixtures/case.fixture.json"),
            "{field}: {error}"
        );
    }
    for raw in ["{", r#"{"id":"a","id":"b"}"#] {
        let temp = tempfile::tempdir().unwrap();
        write_case(temp.path(), "case", declaration());
        fs::write(temp.path().join("fixtures/case.fixture.json"), raw).unwrap();
        let error = fixture_build::generate(temp.path()).unwrap_err();
        assert!(error.contains("line"), "{error}");
    }
}
#[test]
fn paths_duplicates_missing_references_and_orphans_fail() {
    for input in [
        "../case.html",
        "/case.html",
        "./case.html",
        "nested//case.html",
        "C:\\case.html",
        "missing.html",
        "case\n.html",
    ] {
        let temp = tempfile::tempdir().unwrap();
        write_case(temp.path(), "case", declaration());
        let mut data = declaration();
        data["input"] = json!(input);
        fs::write(
            temp.path().join("fixtures/case.fixture.json"),
            serde_json::to_vec(&data).unwrap(),
        )
        .unwrap();
        assert!(
            fixture_build::generate(temp.path())
                .unwrap_err()
                .contains("input:")
        );
    }
    let temp = tempfile::tempdir().unwrap();
    write_case(temp.path(), "a", declaration());
    write_case(temp.path(), "b", declaration());
    assert!(
        fixture_build::generate(temp.path())
            .unwrap_err()
            .contains("duplicate id")
    );
    fs::remove_file(temp.path().join("fixtures/b.fixture.json")).unwrap();
    assert!(
        fixture_build::generate(temp.path())
            .unwrap_err()
            .contains("orphan")
    );
}
#[cfg(unix)]
#[test]
fn symlinked_roots_files_and_directories_are_rejected() {
    for path in ["fixtures", "fixtures/case.html", "fixtures/link"] {
        let temp = tempfile::tempdir().unwrap();
        write_case(temp.path(), "case", declaration());
        let link = temp.path().join(path);
        if link.is_dir() {
            fs::remove_dir_all(&link).unwrap();
        } else if link.exists() {
            fs::remove_file(&link).unwrap();
        }
        std::os::unix::fs::symlink(temp.path(), link).unwrap();
        assert!(fixture_build::generate(temp.path()).is_err());
    }
}

// This intentionally builds the production crate, not a mock parser/registry.
// It is opt-in because a separate target directory compiles workspace dependencies.
#[test]
#[ignore = "isolated production Cargo rebuilds; run explicitly for AG6 packaging validation"]
fn isolated_cargo_rebuild_and_standalone_execution() {
    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if [
                "target",
                ".git",
                ".agents",
                ".codex",
                ".aws",
                ".venv",
                "__pycache__",
            ]
            .iter()
            .any(|s| name == *s)
            {
                continue;
            }
            let ty = entry.file_type().unwrap();
            if ty.is_dir() {
                copy_tree(&entry.path(), &to.join(name));
            } else if ty.is_file() {
                fs::copy(entry.path(), to.join(name)).unwrap();
            } else {
                panic!("unexpected source entry {}", entry.path().display());
            }
        }
    }
    fn build(source: &Path, target: &Path) {
        let output = Command::new(env!("CARGO"))
            .args([
                "build",
                "-p",
                "borrowser-conformance",
                "--locked",
                "--offline",
            ])
            .current_dir(source)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_BUILD_JOBS", "4")
            .env(
                "BORROWSER_AG4_SOURCE_TREE",
                "1111111111111111111111111111111111111111",
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn run(exe: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
        Command::new(exe)
            .args(args)
            .current_dir(cwd)
            .env_remove("BORROWSER_CHROMIUM_EXECUTABLE")
            .output()
            .unwrap()
    }
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        fs::copy(workspace.join(name), source.join(name)).unwrap();
    }
    for name in ["src", "crates"] {
        copy_tree(&workspace.join(name), &source.join(name));
    }
    let target = temp.path().join("target");
    let exe = target.join("debug/borrowser-conformance");
    let crate_root = source.join("crates/conformance");
    let root = crate_root.join("fixtures");

    // This runnable, partially observed fixture both sorts before added/canvas
    // and makes the initial whole-corpus run incomplete instead of successful.
    let mut partial = declaration();
    partial["id"] = json!("aa/partial");
    partial["observations"]["dom"] =
        json!({"kind":"unsupported","reason":"DOM projection excluded for this fixture"});
    partial["expected"] = json!({"kind":"projection","path":"partial.expected.json"});
    write_case(&crate_root, "partial", partial);
    fs::copy(
        root.join("root-canvas-color.html"),
        root.join("partial.html"),
    )
    .unwrap();
    fs::write(
        root.join("partial.expected.json"),
        serde_json::to_vec(&json!({
            "dom":{"state":"unsupported","value":"DOM projection excluded for this fixture"},
            "styles":[],
            "layout":{"state":"not_requested"},
            "canvas":{"state":"observed","value":[18,52,86]}
        }))
        .unwrap(),
    )
    .unwrap();
    build(&source, &target);
    let original = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&original);
    assert!(String::from_utf8_lossy(&original.stdout).contains("INCOMPLETE aa/partial\n"));
    assert_eq!(original.status.code(), Some(3));

    // Exercise the ordinary whole-corpus CLI assertions against the actual
    // generated partial-coverage corpus, retaining its named baseline checks.
    let cli_tests = Command::new(env!("CARGO"))
        .args([
            "test",
            "-p",
            "borrowser-conformance",
            "--test",
            "cli",
            "--locked",
            "--offline",
        ])
        .current_dir(&source)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_BUILD_JOBS", "4")
        .env(
            "BORROWSER_AG4_SOURCE_TREE",
            "1111111111111111111111111111111111111111",
        )
        .output()
        .unwrap();
    assert!(
        cli_tests.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&cli_tests.stdout),
        String::from_utf8_lossy(&cli_tests.stderr)
    );
    eprintln!("AG6 ordinary CLI tests passed against generated partial-coverage corpus");

    let identity = run(&exe, temp.path(), &["--ag4-artifact-info"]);
    assert!(identity.status.success());
    let original_identity: Value = serde_json::from_slice(&identity.stdout).unwrap();
    let fixture_hash = |identity: &Value, id: &str| {
        identity["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fixture| fixture["id"] == id)
            .unwrap_or_else(|| panic!("missing artifact identity for {id}"))["sha256"]
            .as_str()
            .expect("fixture SHA-256")
            .to_owned()
    };

    let mut added = declaration();
    added["id"] = json!("added/canvas");
    added["input"] = json!("added.html");
    write_case(&crate_root, "added", added.clone());
    fs::copy(root.join("root-canvas-color.html"), root.join("added.html")).unwrap();
    build(&source, &target);
    assert!(run(&exe, temp.path(), &["added/canvas"]).status.success());
    let identity: Value =
        serde_json::from_slice(&run(&exe, temp.path(), &["--ag4-artifact-info"]).stdout).unwrap();
    assert_eq!(
        identity["fixtures"].as_array().unwrap().len(),
        original_identity["fixtures"].as_array().unwrap().len() + 1
    );
    let old_hash = fixture_hash(&identity, "added/canvas");
    let earlier_hash = fixture_hash(&identity, "aa/partial");
    assert!(
        identity["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .take_while(|fixture| fixture["id"] != "added/canvas")
            .any(|fixture| fixture["id"] == "aa/partial")
    );
    let html = fs::read_to_string(root.join("added.html"))
        .unwrap()
        .replace("#123456", "#234567");
    fs::write(root.join("added.html"), html).unwrap();
    added["expected"]["rgb8"] = json!([35, 69, 103]);
    fs::write(
        root.join("added.fixture.json"),
        serde_json::to_vec(&added).unwrap(),
    )
    .unwrap();
    build(&source, &target);
    let edited = run(&exe, temp.path(), &["added/canvas"]);
    assert!(edited.status.success());
    assert!(String::from_utf8_lossy(&edited.stdout).contains("#234567"));
    let identity: Value =
        serde_json::from_slice(&run(&exe, temp.path(), &["--ag4-artifact-info"]).stdout).unwrap();
    assert_ne!(fixture_hash(&identity, "added/canvas"), old_hash);
    assert_eq!(fixture_hash(&identity, "aa/partial"), earlier_hash);
    fs::remove_file(root.join("added.html")).unwrap();
    fs::remove_file(root.join("added.fixture.json")).unwrap();
    build(&source, &target);
    assert_eq!(
        run(&exe, temp.path(), &["added/canvas"]).status.code(),
        Some(2)
    );
    let restored = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&restored);
    assert_eq!(restored.stdout, original.stdout);

    // The actual generator must encode every accepted string as inert Rust data.
    let text = "quote \" slash \\ Unicode café 🦀 newline\n tab\t CR\r NUL\0 end";
    let mut excluded = declaration();
    excluded["id"] = json!("excluded/js");
    excluded["requires_javascript"] = json!(true);
    excluded["classification"] = json!({"kind":"unsupported","reason":text});
    excluded.as_object_mut().unwrap().remove("expected");
    excluded.as_object_mut().unwrap().remove("expectation");
    // Also exercise metadata/source path literals, not only a reason string.
    write_case(&crate_root, "excluded-\"café", excluded.clone());
    build(&source, &target);
    let output = run(&exe, temp.path(), &["excluded/js"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains(text));
    for mode in ["--compare", "--compare-aws"] {
        let output = run(&exe, temp.path(), &[mode, "excluded/js"]);
        assert_eq!(
            output.status.code(),
            Some(3),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["cases"][0]["coverage"]["reason"], text);
        assert!(report["cases"][0]["captures"].is_null());
    }
    assert_eq!(
        run(&exe, temp.path(), &["--chromium", "excluded/js"])
            .status
            .code(),
        Some(2)
    );
    if let Ok(browser) = std::env::var("BORROWSER_CHROMIUM_EXECUTABLE") {
        // Actual CLI consumers share the generated catalogue. In particular,
        // capture-only output must contain no invented excluded observation.
        let output = run(
            &exe,
            temp.path(),
            &["--chromium", "--chromium-executable", &browser],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let capture: Value = serde_json::from_slice(&output.stdout).unwrap();
        let rows = capture["observations"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            original_identity["fixtures"].as_array().unwrap().len()
        );
        assert!(rows.iter().all(|row| row["test_id"] != "excluded/js"));
        let output = run(
            &exe,
            temp.path(),
            &["--compare", "--chromium-executable", &browser],
        );
        assert_eq!(
            output.status.code(),
            Some(3),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let comparison: Value = serde_json::from_slice(&output.stdout).unwrap();
        let cases = comparison["cases"].as_array().unwrap();
        assert_eq!(cases.len(), rows.len() + 1);
        assert!(
            cases
                .iter()
                .find(|c| c["test_id"] == "excluded/js")
                .unwrap()["captures"]
                .is_null()
        );
        let partial = cases
            .iter()
            .find(|case| case["test_id"] == "aa/partial")
            .expect("partial-coverage comparison");
        assert_eq!(partial["status"], "incomplete");
        assert_eq!(partial["borrowser_expectation"]["status"], "INCOMPLETE");
        assert!(!partial["captures"].is_null());
        assert_eq!(comparison["summary"]["different_fields"], 0);
        eprintln!("AG6 real Chromium mixed-catalogue capture/comparison passed");
    } else {
        eprintln!(
            "AG6 optional real Chromium mixed-catalogue check NOT EXECUTED: no pinned browser configured"
        );
    }

    // Even selecting a different case must validate all malformed expectations first.
    let expected = root.join("dom-structure.expected.json");
    let saved = fs::read(&expected).unwrap();
    let declaration_path = root.join("dom-structure.fixture.json");
    let saved_declaration = fs::read(&declaration_path).unwrap();
    let mut invalid: Value = serde_json::from_slice(&saved_declaration).unwrap();
    invalid["expectation"] =
        json!({"kind":"known_failure","reason":"must not hide invalid metadata"});
    fs::write(&declaration_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    fs::write(&expected, "{}").unwrap();
    build(&source, &target);
    for args in [
        vec!["canvas/root"],
        vec!["--chromium", "canvas/root"],
        vec!["--compare", "canvas/root"],
        vec!["--compare-aws", "canvas/root"],
        vec!["--ag4-artifact-info"],
    ] {
        let output = run(&exe, temp.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            text.contains("dom-structure.fixture.json"),
            "{args:?}: {text}"
        );
    }
    let cleanup = run(&exe, temp.path(), &["--aws-cleanup"]);
    assert!(!String::from_utf8_lossy(&cleanup.stderr).contains("dom-structure.fixture.json"));
    fs::write(expected, saved).unwrap();
    fs::write(declaration_path, saved_declaration).unwrap();
    // A nonempty corpus of excluded cases differs from accidentally empty discovery.
    let saved_root = crate_root.join("saved-fixtures");
    fs::rename(&root, &saved_root).unwrap();
    write_case(&crate_root, "excluded", excluded.clone());
    excluded["id"] = json!("excluded/skipped");
    excluded["classification"] = json!({"kind":"skipped","reason":"intentionally excluded"});
    write_case(&crate_root, "skipped", excluded);
    build(&source, &target);
    let only_excluded = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&only_excluded);
    assert!(only_excluded.status.success());
    for mode in ["--compare", "--compare-aws"] {
        let output = run(&exe, temp.path(), &[mode]);
        assert_eq!(output.status.code(), Some(3));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["cases"].as_array().unwrap().len(), 2);
        assert!(
            report["cases"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["captures"].is_null())
        );
    }
    assert_eq!(
        run(&exe, temp.path(), &["--chromium"]).status.code(),
        Some(2)
    );
    let identity: Value =
        serde_json::from_slice(&run(&exe, temp.path(), &["--ag4-artifact-info"]).stdout).unwrap();
    assert_eq!(identity["fixtures"], json!([]));
    fs::remove_dir_all(&root).unwrap();
    fs::rename(saved_root, &root).unwrap();
    build(&source, &target);
    let expected = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&expected);
    let standalone = temp.path().join("standalone");
    fs::copy(&exe, &standalone).unwrap();
    fs::remove_dir_all(&source).unwrap();
    let actual = run(&standalone, temp.path(), &[]);
    support::assert_local_summary_exit(&actual);
    assert_eq!(actual.status.code(), expected.status.code());
    assert_eq!(actual.stdout, expected.stdout);
    assert_eq!(actual.stderr, expected.stderr);
    assert!(
        run(&standalone, temp.path(), &["--ag4-artifact-info"])
            .status
            .success()
    );
    eprintln!(
        "AG6 isolated production rebuild: add/edit/remove, literal preservation, preflight, exclusions and source-free copied executable passed"
    );
}
