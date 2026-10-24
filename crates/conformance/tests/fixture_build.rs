#[path = "support/ag7.rs"]
mod ag7;
#[path = "../build_support/fixtures.rs"]
mod fixture_build;
mod support;

fn copy_fixture_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture_tree(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}

#[test]
fn ag7_pinned_sources_and_relationship_metadata_are_authoritative() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    for mutation in [
        "test_hash",
        "reference_bytes",
        "licence_bytes",
        "kind",
        "reference_target",
        "revision",
        "attribution",
        "local_kind",
        "local_target",
    ] {
        let temp = tempfile::tempdir().unwrap();
        copy_fixture_tree(&source.join("fixtures"), &temp.path().join("fixtures"));
        let root = temp.path().join("fixtures");
        let path = root.join(if mutation.starts_with("local") {
            "wpt-style/background-color-mismatch.fixture.json"
        } else {
            "external/rgb-comments-p6.fixture.json"
        });
        let mut metadata: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match mutation {
            "test_hash" => metadata["provenance"]["sha256"] = json!("0".repeat(64)),
            "reference_bytes" => fs::write(
                root.join("external/raw/css/css-color/background-color-rgb-001-ref.html"),
                b"<!doctype html><p>changed</p>",
            )
            .unwrap(),
            "licence_bytes" => fs::write(root.join("external/raw/LICENSE.md"), b"changed").unwrap(),
            "kind" | "local_kind" => {
                metadata["relationship"]["kind"] =
                    json!("match".to_string() + if mutation == "kind" { "not" } else { "" })
            }
            "reference_target" => {
                metadata["relationship"]["input"] =
                    json!("raw/css/css-color/background-color-rgb-003-ref.html")
            }
            "local_target" => {
                metadata["relationship"]["input"] = json!("background-color-mismatch.html")
            }
            "revision" => {
                metadata["relationship"]["provenance"]["revision"] = json!("0".repeat(40))
            }
            "attribution" => metadata["provenance"]["attribution"] = json!(" "),
            _ => unreachable!(),
        }
        // Valid alternate kind still contradicts the pinned active link.
        if mutation == "kind" {
            metadata["relationship"]["kind"] = json!("mismatch");
        }
        fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
        assert!(fixture_build::generate(temp.path()).is_err(), "{mutation}");
    }
    let generated = fixture_build::generate(source).unwrap();
    assert!(generated.contains("wpt/rgb-comments-p6"));
    assert!(generated.contains("wpt/unsupported/testdriver"));
    assert!(generated.contains("Checks::Reference"));
}

#[test]
fn ag7_valid_unsupported_forms_are_excluded_not_invented_relationships() {
    for body in [
        "<link rel=match href=one.html><link rel=match href=two.html>",
        "<link rel=match href='../outside.html'>",
        "<html class=reftest-wait><script>change()</script>",
        "<meta name=fuzzy content='0-1;0-2'>",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut metadata = declaration();
        metadata["classification"] = json!({"kind":"unsupported","reason":"form outside AG7"});
        metadata["observations"] = json!({});
        metadata.as_object_mut().unwrap().remove("expected");
        metadata.as_object_mut().unwrap().remove("expectation");
        write_case(temp.path(), "excluded", metadata);
        fs::write(
            temp.path().join("fixtures/excluded.html"),
            format!("<!doctype html>{body}"),
        )
        .unwrap();
        let generated = fixture_build::generate(temp.path()).unwrap();
        assert!(generated.contains("FixtureKind::Unsupported"));
        assert!(!generated.contains("Checks::Reference"));
    }
}

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
    let named_original: Vec<_> = ["ci", "extended"]
        .into_iter()
        .map(|suite| run(&exe, temp.path(), &["--suite", suite]))
        .collect();
    assert_eq!(named_original[0].status.code(), Some(0));
    assert_eq!(named_original[1].status.code(), Some(3));
    let original = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&original);
    assert!(String::from_utf8_lossy(&original.stdout).contains("INCOMPLETE aa/partial\n"));
    assert_eq!(original.status.code(), Some(3));

    let identity = run(&exe, temp.path(), &["--ag4-artifact-info"]);
    assert!(identity.status.success());
    let original_identity: Value = serde_json::from_slice(&identity.stdout).unwrap();
    let fixture_hash = |identity: &Value, id: &str| {
        identity["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fixture| fixture["case_id"] == id)
            .unwrap_or_else(|| panic!("missing artifact identity for {id}"))["source_sha256"]
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
    for (suite, original) in ["ci", "extended"].into_iter().zip(&named_original) {
        let next = run(&exe, temp.path(), &["--suite", suite]);
        assert_eq!(next.status.code(), original.status.code());
        assert_eq!(next.stdout, original.stdout);
        assert_eq!(next.stderr, original.stderr);
    }
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
            .take_while(|fixture| fixture["case_id"] != "added/canvas")
            .any(|fixture| fixture["case_id"] == "aa/partial")
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

    // A new direct reference is discovered without Rust registration. Mutate
    // only this test-owned local copy, never the committed upstream corpus.
    let added_ref = root.join("added-reference");
    copy_fixture_tree(&root.join("wpt-style"), &added_ref);
    let meta_path = added_ref.join("background-color-mismatch.fixture.json");
    let mut meta: Value = serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
    meta["id"] = json!("added/reference");
    fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
    build(&source, &target);
    assert!(
        run(&exe, temp.path(), &["added/reference"])
            .status
            .success()
    );
    let descriptors = || -> Vec<Value> {
        let identity: Value =
            serde_json::from_slice(&run(&exe, temp.path(), &["--ag4-artifact-info"]).stdout)
                .unwrap();
        assert_eq!(
            identity["fixtures"].as_array().unwrap().len(),
            original_identity["fixtures"].as_array().unwrap().len() + 2
        );
        identity["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["case_id"] == "added/reference")
            .cloned()
            .collect()
    };
    let before = descriptors();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0]["role"], "test");
    assert_eq!(before[1]["role"], "reference");
    let ref_path = added_ref.join("red.html");
    fs::write(
        &ref_path,
        fs::read_to_string(&ref_path)
            .unwrap()
            .replace("red;", "blue;"),
    )
    .unwrap();
    build(&source, &target);
    let changed = descriptors();
    assert_eq!(before[0], changed[0]);
    assert_ne!(before[1]["source_sha256"], changed[1]["source_sha256"]);
    assert_eq!(before[1]["request"], changed[1]["request"]);
    assert!(
        run(&exe, temp.path(), &["added/reference"])
            .status
            .success()
    );
    meta["observations"]["styles"][0]["target"] = json!("absent");
    fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
    build(&source, &target);
    let requested = descriptors();
    for side in 0..2 {
        assert_eq!(
            requested[side]["source_sha256"],
            changed[side]["source_sha256"]
        );
        assert_ne!(requested[side]["request"], changed[side]["request"]);
    }
    let absent = run(&exe, temp.path(), &["added/reference"]);
    assert_eq!(absent.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&absent.stdout).contains("presence_failed"));
    fs::remove_dir_all(&added_ref).unwrap();
    build(&source, &target);
    assert_eq!(
        run(&exe, temp.path(), &["added/reference"]).status.code(),
        Some(2)
    );
    assert_eq!(run(&exe, temp.path(), &[]).stdout, original.stdout);

    // Restore the independently authored passing variant and keep it present
    // during ordinary CLI, whole-catalogue qualification and source-free runs.
    copy_fixture_tree(&root.join("wpt-style"), &added_ref);
    meta["observations"]["styles"][0]["target"] = json!("probe");
    fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
    fs::write(
        &ref_path,
        fs::read_to_string(&ref_path)
            .unwrap()
            .replace("red;", "blue;"),
    )
    .unwrap();
    // Independently authored integer notation must survive discovery, typed
    // execution, catalogue qualification and execution without this source tree.
    let mut numeric = declaration();
    numeric["id"] = json!("added/numeric");
    numeric["observations"] = json!({"styles":[{
        "target":"numeric","property":"font-size","selection":{"kind":"compare"}}]});
    numeric["expected"] = json!({"kind":"projection","path":"numeric.expected.json"});
    write_case(&crate_root, "numeric", numeric);
    fs::write(root.join("numeric.html"),
        "<!doctype html><html><head><style>#numeric { font-size: 20px; }</style></head><body><div id=\"numeric\"></div></body></html>").unwrap();
    fs::write(root.join("numeric.expected.json"), br#"{
        "dom":{"state":"not_requested"},"styles":[{"target":"numeric","property":"font-size",
            "value":{"state":"observed","value":{"state":"present","value":{"kind":"css_px","value":20}}}}],
        "layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}
    }"#).unwrap();
    build(&source, &target);
    // Exercise the ordinary whole-corpus CLI assertions against the actual
    // generated corpus with an additional passing reference, retaining its named baseline checks.
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
    eprintln!(
        "Expanded ordinary CLI qualification: {}\n{}\n{}",
        cli_tests.status,
        String::from_utf8_lossy(&cli_tests.stdout),
        String::from_utf8_lossy(&cli_tests.stderr)
    );

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
    // Restore as ordinary file additions. Renaming the saved directory back
    // would preserve timestamps older than Cargo's latest directory fingerprint.
    copy_fixture_tree(&saved_root, &root);
    fs::remove_dir_all(saved_root).unwrap();
    build(&source, &target);
    let expected = run(&exe, temp.path(), &[]);
    support::assert_local_summary_exit(&expected);
    let restored_text = String::from_utf8_lossy(&expected.stdout);
    for line in [
        "PASS canvas/root",
        "PASS added/reference",
        "PASS added/numeric",
        "PASS wpt-style/background-color-mismatch",
        "INCOMPLETE wpt/rgb-comments-p6",
        "UNSUPPORTED excluded/js",
    ] {
        assert!(restored_text.contains(line), "restored corpus: {line}");
    }
    assert!(!restored_text.contains("excluded/skipped"));
    let authored = ag7::declarations(&root);
    let expanded_identity: Value =
        serde_json::from_slice(&run(&exe, temp.path(), &["--ag4-artifact-info"]).stdout).unwrap();
    if std::env::var_os("BORROWSER_CHROMIUM_EXECUTABLE").is_some() {
        let output = Command::new(env!("CARGO"))
            .args([
                "test",
                "-p",
                "borrowser-conformance",
                "--locked",
                "--offline",
                "--test",
                "comparison_cli",
                "real_chromium_combined_cli_is_stable_and_selects_exactly",
                "--",
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
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
        eprintln!(
            "Expanded catalogue CLI qualification: {}\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success());
    }
    let standalone = temp.path().join("standalone");
    fs::copy(&exe, &standalone).unwrap();
    fs::remove_dir_all(&source).unwrap();
    for (suite, original) in ["ci", "extended"].into_iter().zip(&named_original) {
        let next = run(&standalone, temp.path(), &["--suite", suite]);
        assert_eq!(next.status.code(), original.status.code());
        assert_eq!(next.stdout, original.stdout);
        assert_eq!(next.stderr, original.stderr);
    }
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
    // All optional real captures now also prove that the copied executable
    // resolves embedded test/reference inputs after removal of its source tree.
    if let Ok(browser) = std::env::var("BORROWSER_CHROMIUM_EXECUTABLE") {
        // Actual CLI consumers share the generated catalogue. In particular,
        // capture-only output must contain no invented excluded observation.
        let output = run(
            &standalone,
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
            expanded_identity["fixtures"].as_array().unwrap().len()
        );
        assert!(
            rows.iter()
                .all(|row| row["descriptor"]["case_id"] != "excluded/js")
        );
        let output = run(
            &standalone,
            temp.path(),
            &["--compare", "--chromium-executable", &browser],
        );
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let comparison: Value = serde_json::from_slice(&output.stdout).unwrap();
        let cases = comparison["cases"].as_array().unwrap();
        assert_eq!(cases.len(), authored.len());
        assert_eq!(
            rows.iter()
                .map(|r| r["descriptor"].clone())
                .collect::<Vec<_>>(),
            *expanded_identity["fixtures"].as_array().unwrap()
        );
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
        ag7::assert_catalogue(
            &comparison,
            output.status.code().unwrap(),
            &expanded_identity["fixtures"],
            &authored,
        );
        let numeric = cases
            .iter()
            .find(|c| c["test_id"] == "added/numeric")
            .unwrap();
        assert_eq!(numeric["status"], "equal_for_requested_scope");
        assert_eq!(numeric["borrowser_expectation"]["status"], "PASS");
        assert!(numeric["relationships"].is_null());
        assert!(numeric["reference_document"].is_null());
        assert_eq!(numeric["fields"].as_array().unwrap().len(), 1);
        assert_eq!(numeric["fields"][0]["id"], "style:#numeric/font-size");
        assert_eq!(numeric["fields"][0]["status"], "equal");
        for engine in ["borrowser", "chromium"] {
            assert_eq!(
                numeric["projections"][engine]["styles"][0]["value"]["value"]["value"],
                json!({"kind":"css_px","value":20.0})
            );
            assert!(!numeric["captures"][engine].is_null());
        }
        let added = cases
            .iter()
            .find(|c| c["test_id"] == "added/reference")
            .unwrap();
        assert_eq!(added["status"], "equal_for_requested_scope");
        assert!(added["borrowser_expectation"].is_null());
        for (doc, rgba) in [
            (added, [0, 128, 0, 255]),
            (&added["reference_document"], [0, 0, 255, 255]),
        ] {
            assert_eq!(doc["fields"].as_array().unwrap().len(), 1);
            assert_eq!(doc["fields"][0]["status"], "equal");
            for engine in ["borrowser", "chromium"] {
                assert_eq!(
                    doc["projections"][engine]["styles"][0]["value"]["value"]["value"]["value"],
                    json!(rgba)
                );
                assert_eq!(added["relationships"][engine]["status"], "satisfied");
            }
        }
        // The test-owned partial case contributes one unsupported DOM field;
        // excluded/js retains its declared canvas selection and contributes a
        // separate unsupported canvas field. Neither is an upstream-coverage count.
        let excluded_case = cases
            .iter()
            .find(|c| c["test_id"] == "excluded/js")
            .unwrap();
        assert_eq!(excluded_case["fields"].as_array().unwrap().len(), 1);
        assert_eq!(excluded_case["fields"][0]["id"], "canvas_color_rgb8");
        assert_eq!(excluded_case["fields"][0]["status"], "unsupported");
        eprintln!("AG6 real Chromium mixed-catalogue capture/comparison passed");
    } else {
        eprintln!(
            "AG6 optional real Chromium mixed-catalogue check NOT EXECUTED: no pinned browser configured"
        );
    }

    if let Ok(browser) = std::env::var("BORROWSER_CHROMIUM_EXECUTABLE") {
        let output = run(
            &standalone,
            temp.path(),
            &[
                "--compare",
                "wpt-style/background-color-mismatch",
                "--chromium-executable",
                &browser,
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["summary"]["executed_documents"], 2);
        assert_eq!(
            report["cases"][0]["relationships"]["chromium"]["status"],
            "satisfied"
        );
    }
    eprintln!(
        "AG6 isolated production rebuild: add/edit/remove, literal preservation, preflight, exclusions and source-free copied executable passed"
    );
}
