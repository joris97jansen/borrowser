use super::disposition::*;
use super::model::*;
use super::runner::{FixtureFailureDetails, execute_fixture};
use super::*;
use html::conformance::{
    CanonicalParserResult, IncompleteObservationReason, InvariantFailureCode, ObservationState,
};
use ring::digest::{SHA256, digest};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct TestRepository {
    _temp: TempDir,
    repository_root: PathBuf,
    fixture_root: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary repository");
        let repository_root = temp.path().join("repo");
        let fixture_root = repository_root.join("fixtures");
        fs::create_dir_all(&fixture_root).expect("fixture root");
        Self {
            _temp: temp,
            repository_root,
            fixture_root,
        }
    }

    fn native(&self) -> FixtureRepository {
        FixtureRepository::native(&self.repository_root, &self.fixture_root)
    }

    fn adapted(&self) -> FixtureRepository {
        FixtureRepository {
            repository_root: self.repository_root.clone(),
            fixture_root: self.fixture_root.clone(),
            policy: FixtureRepositoryPolicy::AdaptedOrQuarantine,
        }
    }
}

fn add_fixture(repository: &TestRepository, directory: &str, id: &str, input: &[u8]) -> PathBuf {
    let bundle = repository.fixture_root.join(directory);
    fs::create_dir_all(&bundle).expect("bundle");
    fs::write(bundle.join("input.html"), input).expect("input");
    fs::write(
        bundle.join("tokens.txt"),
        "# format: html5-token-v1\nCHAR text=\"hello\"\nEOF\n",
    )
    .expect("tokens");
    fs::write(bundle.join("fixture.toml"), fixture_toml(id, input)).expect("metadata");
    bundle
}

fn fixture_toml(id: &str, input: &[u8]) -> String {
    format!(
        r#"format = "borrowser-html-parser-fixture-v1"
id = "{id}"

[source]
kind = "native"

[input]
path = "input.html"
kind = "utf8-text"
sha256 = "{}"

[execution]
reference_delivery = "whole"

[execution.target]
kind = "standalone-tokenizer"

[[execution.deliveries]]
name = "whole"
unit = "unicode-scalars"
strategy = "whole"

[expectations]
tokens = "tokens.txt"

[disposition]
status = "active"
"#,
        sha256(input)
    )
}

fn rewrite(path: &Path, transform: impl FnOnce(String) -> String) {
    let original = fs::read_to_string(path).expect("read fixture metadata");
    fs::write(path, transform(original)).expect("rewrite fixture metadata");
}

fn sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest(&SHA256, bytes).as_ref() {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn load_single_native_fixture(repository: &TestRepository) -> ValidatedFixtureSpec {
    let mut fixtures = discover_and_load(&repository.native()).expect("valid fixture");
    assert_eq!(fixtures.len(), 1, "test repository has one fixture");
    fixtures.remove(0)
}

#[test]
fn discovery_is_sorted_by_normalized_repository_relative_path() {
    let repository = TestRepository::new();
    add_fixture(&repository, "z-last", "z-last", b"hello");
    add_fixture(&repository, "nested/a-first", "a-first", b"hello");

    let fixtures = discover_and_load(&repository.native()).expect("valid fixtures");
    let paths = fixtures
        .iter()
        .map(ValidatedFixtureSpec::repository_relative_path)
        .collect::<Vec<_>>();
    assert_eq!(paths, ["fixtures/nested/a-first", "fixtures/z-last"]);
}

#[test]
fn canonical_corpus_runner_executes_every_discovered_fixture_in_order() {
    let repository = TestRepository::new();
    add_fixture(&repository, "z-second", "second-fixture", b"hello");
    add_fixture(&repository, "a-first", "first-fixture", b"hello");

    let fixtures = discover_and_load(&repository.native()).expect("valid fixtures");
    let reports = run_fixture_corpus(&fixtures).expect("both fixtures execute");
    assert_eq!(
        reports
            .iter()
            .map(|report| report.fixture_id().as_str())
            .collect::<Vec<_>>(),
        ["first-fixture", "second-fixture"]
    );
    assert!(reports.iter().all(|report| report.result().is_some()));
}

#[test]
fn canonical_corpus_runner_aggregates_all_fixture_failures_with_identity() {
    let repository = TestRepository::new();
    let first = add_fixture(&repository, "a-first", "first-broken", b"hello");
    let second = add_fixture(&repository, "b-second", "second-broken", b"hello");
    fs::write(
        first.join("tokens.txt"),
        "# format: html5-token-v1\nBROKEN\nEOF\n",
    )
    .expect("first malformed snapshot");
    fs::write(
        second.join("tokens.txt"),
        "# format: html5-token-v1\nBROKEN\nEOF\n",
    )
    .expect("second malformed snapshot");

    let fixtures = discover_and_load(&repository.native()).expect("fixtures load before execution");
    let error = run_fixture_corpus(&fixtures).unwrap_err();
    assert_eq!(
        error
            .failures()
            .iter()
            .map(|failure| {
                (
                    failure.fixture_id().as_str(),
                    failure.repository_relative_path(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("first-broken", "fixtures/a-first"),
            ("second-broken", "fixtures/b-second"),
        ]
    );
    assert!(error.failures().iter().all(|failure| matches!(
        failure.error().policy,
        DispositionEvaluationError::UnexpectedOutcome {
            actual: FixtureOutcomeClassification::ExecutionFailed(
                ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens)
            ),
            ..
        }
    )));
}

#[test]
fn duplicate_fixture_ids_fail_deterministically() {
    let repository = TestRepository::new();
    add_fixture(&repository, "a", "duplicate", b"hello");
    add_fixture(&repository, "b", "duplicate", b"hello");

    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::DuplicateFixtureId(_)
    ));
    assert_eq!(error.path, "fixtures/b");
}

#[test]
fn invalid_and_case_unsafe_fixture_ids_are_rejected() {
    for (id, expected_case_error) in [("not_snake", false), ("Case-Collision", true)] {
        let repository = TestRepository::new();
        add_fixture(&repository, "case", id, b"hello");
        let error = discover_and_load(&repository.native()).unwrap_err();
        assert_eq!(
            matches!(error.kind, FixtureLoadErrorKind::CaseUnsafeFixtureId(_)),
            expected_case_error
        );
    }
}

#[test]
fn fixture_ids_that_differ_only_by_case_are_rejected_as_a_collision() {
    let repository = TestRepository::new();
    add_fixture(&repository, "a", "case-id", b"hello");
    add_fixture(&repository, "b", "Case-Id", b"hello");
    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::CaseCollidingFixtureId(_)
    ));
    assert_eq!(error.path, "fixtures/b");
}

#[test]
fn unknown_top_level_and_nested_fields_are_rejected() {
    for addition in ["\nunknown = true\n", "\n[input]\nunknown = true\n"] {
        let repository = TestRepository::new();
        let bundle = add_fixture(&repository, "unknown", "unknown-field", b"hello");
        rewrite(&bundle.join("fixture.toml"), |mut text| {
            text.push_str(addition);
            text
        });
        let error = discover_and_load(&repository.native()).unwrap_err();
        assert!(matches!(
            error.kind,
            FixtureLoadErrorKind::InvalidFixtureToml(_)
        ));
    }
}

#[test]
fn required_unknown_extension_is_an_explicit_unsupported_semantic() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "required-ext", "required-ext", b"hello");
    rewrite(&bundle.join("fixture.toml"), |mut text| {
        text.push_str(
            "\n[extensions.\"org.example.feature-v1\"]\nrequired = true\nvalue = { mode = \"strict\" }\n",
        );
        text
    });
    let fixture = discover_and_load(&repository.native())
        .expect("schema is valid")
        .remove(0);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::UnknownRequiredExtension(ref id)
        } if id == "org.example.feature-v1"
    ));
}

#[test]
fn optional_unknown_extension_is_retained_as_non_semantic_metadata() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "optional-ext", "optional-ext", b"hello");
    rewrite(&bundle.join("fixture.toml"), |mut text| {
        text.push_str(
            "\n[extensions.\"org.example.note-v1\"]\nrequired = false\nvalue = { note = \"retained\" }\n",
        );
        text
    });
    let fixture = discover_and_load(&repository.native())
        .expect("valid optional extension")
        .remove(0);
    assert!(
        fixture
            .optional_extensions()
            .contains_key("org.example.note-v1")
    );
    assert!(fixture.required_unknown_extensions().is_empty());
}

#[test]
fn malformed_extension_declarations_are_rejected() {
    for declaration in [
        "\n[extensions.\"org.example.note-v1\"]\nrequired = false\n",
        "\n[extensions.\"org.example.note-v1\"]\nrequired = false\nvalue = \"x\"\nextra = true\n",
        "\n[extensions.\"unversioned\"]\nrequired = false\nvalue = \"x\"\n",
    ] {
        let repository = TestRepository::new();
        let bundle = add_fixture(&repository, "bad-ext", "bad-ext", b"hello");
        rewrite(&bundle.join("fixture.toml"), |mut text| {
            text.push_str(declaration);
            text
        });
        assert!(discover_and_load(&repository.native()).is_err());
    }
}

#[test]
fn exact_text_input_preserves_a_terminal_newline() {
    let repository = TestRepository::new();
    add_fixture(&repository, "newline", "terminal-newline", b"hello\n");
    let fixture = discover_and_load(&repository.native())
        .expect("valid fixture")
        .remove(0);
    let ExactInput::Utf8Text { bytes, text, .. } = fixture.input() else {
        panic!("expected text input")
    };
    assert_eq!(bytes, b"hello\n");
    assert_eq!(text, "hello\n");
}

#[test]
fn validated_fixture_accessors_are_read_only_views_of_canonical_validation() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "accessors", "validated-accessors", b"hello\n");
    rewrite(&bundle.join("fixture.toml"), |text| {
        format!(
            "{text}\n[metadata]\ndescription = \"validated fixture\"\ncomments = [\"read only\"]\n"
        )
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);

    assert_eq!(fixture.id().as_str(), "validated-accessors");
    assert_eq!(fixture.repository_relative_path(), "fixtures/accessors");
    assert_eq!(fixture.input_path(), "input.html");
    assert_eq!(fixture.input_bytes(), b"hello\n");
    assert_eq!(fixture.input_text(), Some("hello\n"));
    assert_eq!(fixture.input_sha256(), sha256(b"hello\n"));
    assert_eq!(fixture.source_kind(), FixtureSourceKind::Native);
    assert_eq!(fixture.source_reference(), None);
    assert_eq!(fixture.target_kind(), ParserTargetKind::StandaloneTokenizer);
    assert_eq!(fixture.scripting_mode(), None);
    assert_eq!(fixture.reference_delivery().as_str(), "whole");
    assert_eq!(
        fixture
            .delivery_names()
            .map(DeliveryName::as_str)
            .collect::<Vec<_>>(),
        ["whole"]
    );
    assert_eq!(fixture.delivery_boundaries("whole"), Some(None));
    assert_eq!(fixture.description(), Some("validated fixture"));
    assert_eq!(fixture.comments(), ["read only"]);
}

#[test]
fn text_input_rejects_every_carriage_return_shape_but_accepts_lf() {
    for (directory, input) in [
        ("crlf", b"a\r\nb".as_slice()),
        ("lone-cr", b"a\rb".as_slice()),
        ("trailing-cr", b"a\r".as_slice()),
    ] {
        let repository = TestRepository::new();
        add_fixture(&repository, directory, directory, input);
        let error = discover_and_load(&repository.native()).unwrap_err();
        assert_eq!(error.kind, FixtureLoadErrorKind::CarriageReturnInTextInput);
        assert!(error.to_string().contains("must use input.bin"));
    }

    let repository = TestRepository::new();
    add_fixture(&repository, "lf-only", "lf-only", b"a\nb\n");
    assert!(discover_and_load(&repository.native()).is_ok());
}

#[test]
fn invalid_utf8_declared_as_text_is_rejected() {
    let repository = TestRepository::new();
    add_fixture(&repository, "invalid-utf8", "invalid-utf8", &[0xff]);
    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidUtf8TextInput
    ));
}

#[test]
fn raw_input_bytes_are_preserved_and_ae13a_reports_them_as_unsupported() {
    let bytes = b"a\r\nb\r";
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "raw", "raw-input", bytes);
    fs::rename(bundle.join("input.html"), bundle.join("input.bin")).expect("rename raw input");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("path = \"input.html\"", "path = \"input.bin\"")
            .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"")
            .replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    assert_eq!(fixture.input_bytes(), bytes);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::RawByteInput
        }
    ));
}

#[test]
fn sha256_mismatch_is_rejected() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "hash", "hash-mismatch", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(&sha256(b"hello"), &"0".repeat(64))
    });
    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::Sha256Mismatch { .. }
    ));
}

#[test]
fn missing_declared_and_orphan_recognized_sidecars_are_rejected() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "missing", "missing-sidecar", b"hello");
    fs::remove_file(bundle.join("tokens.txt")).expect("remove tokens");
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::MissingDeclaredFile(_)
    ));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "orphan", "orphan-sidecar", b"hello");
    fs::write(bundle.join("tree.txt"), "# planned\n").expect("orphan");
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::OrphanSidecar(_)
    ));
}

#[test]
fn absolute_and_parent_traversal_paths_are_rejected() {
    for unsafe_path in [
        "/tmp/input.html",
        "../input.html",
        "nested/../input.html",
        "C:/input.html",
    ] {
        let repository = TestRepository::new();
        let bundle = add_fixture(&repository, "unsafe", "unsafe-path", b"hello");
        rewrite(&bundle.join("fixture.toml"), |text| {
            text.replace(
                "path = \"input.html\"",
                &format!("path = \"{unsafe_path}\""),
            )
        });
        assert!(matches!(
            discover_and_load(&repository.native()).unwrap_err().kind,
            FixtureLoadErrorKind::UnsafeRelativePath(_)
        ));
    }
}

#[cfg(unix)]
#[test]
fn symlinked_fixture_files_are_rejected() {
    use std::os::unix::fs::symlink;

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "symlink", "symlink-input", b"hello");
    let outside = repository.repository_root.join("outside.html");
    fs::write(&outside, b"hello").expect("outside input");
    fs::remove_file(bundle.join("input.html")).expect("remove input");
    symlink(&outside, bundle.join("input.html")).expect("input symlink");
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::SymlinkNotAllowed
    ));
}

#[test]
fn nested_fixture_bundles_are_rejected_instead_of_silently_ignored() {
    let repository = TestRepository::new();
    let outer = add_fixture(&repository, "outer", "outer", b"hello");
    let nested = outer.join("nested");
    fs::create_dir_all(&nested).expect("nested bundle");
    fs::write(
        nested.join("fixture.toml"),
        fixture_toml("nested", b"hello"),
    )
    .expect("nested fixture metadata");

    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::NestedFixtureBundle(ref path)
            if path == "fixtures/outer/nested/fixture.toml"
    ));
}

#[test]
fn illegal_input_delivery_and_target_combinations_are_rejected() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "delivery", "bad-delivery", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
    });
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidCombination(_)
    ));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "target", "bad-target", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"standalone-tokenizer\"\nscripting = \"enabled\"",
        )
    });
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidCombination(_)
    ));
}

#[test]
fn input_extension_and_transition_delivery_references_are_validated() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "extension", "bad-input-extension", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("path = \"input.html\"", "path = \"input.bin\"")
    });
    fs::rename(bundle.join("input.html"), bundle.join("input.bin")).expect("rename input");
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidInputExtension
    ));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "trace", "bad-trace-delivery", b"hello");
    fs::write(bundle.join("transitions.missing.txt"), "# planned\n").expect("trace");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\ntransitions = [{ delivery = \"missing\", path = \"transitions.missing.txt\" }]",
        )
    });
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidCombination(_)
    ));
}

#[test]
fn valid_fragment_and_scripting_semantics_are_explicitly_unsupported_in_ae13a() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "fragment", "fragment-case", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nfragment = { namespace = \"html\", local_name = \"div\" }",
        )
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    assert_eq!(fixture.target_kind(), ParserTargetKind::Fragment);
    assert_eq!(fixture.scripting_mode(), Some(ScriptingMode::Disabled));
    assert_eq!(
        fixture.fragment_namespace(),
        Some(html::ElementNamespace::Html)
    );
    assert_eq!(fixture.fragment_local_name(), Some("div"));
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::FragmentParsing
        }
    ));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "namespace", "invalid-namespace", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nfragment = { namespace = \"unknown\", local_name = \"div\" }",
        )
    });
    let error = discover_and_load(&repository.native()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidCombination(_)
    ));
    assert!(error.to_string().contains("html, svg, or mathml"));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "document", "document-default", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("kind = \"standalone-tokenizer\"", "kind = \"document\"")
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    assert_eq!(fixture.target_kind(), ParserTargetKind::Document);
    assert_eq!(fixture.scripting_mode(), Some(ScriptingMode::Disabled));

    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "script", "scripting-case", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"document\"\nscripting = \"enabled\"",
        )
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::ScriptingEnabled
        }
    ));
}

#[test]
fn active_unimplemented_expectation_fails_with_typed_surface() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "tree", "tree-expectation", b"hello");
    fs::write(bundle.join("tree.txt"), "# format: html5-dom-v2\n").expect("tree");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\ntree = \"tree.txt\"",
        )
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    let error = run_fixture(&fixture).unwrap_err();
    assert_eq!(
        error.policy,
        DispositionEvaluationError::UnexpectedOutcome {
            expected: DispositionExpectation::Completed,
            actual: FixtureOutcomeClassification::UnsupportedExpectation(ExpectationSurface::Tree,),
        }
    );
}

#[test]
fn malformed_token_snapshot_is_a_typed_snapshot_failure_not_fixture_toml() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "bad-tokens", "bad-tokens", b"hello");
    fs::write(
        bundle.join("tokens.txt"),
        "# format: html5-token-v1\nMALFORMED\nEOF\n",
    )
    .expect("malformed snapshot");
    let fixture = discover_and_load(&repository.native())
        .expect("fixture metadata and paths remain valid")
        .remove(0);
    let error = run_fixture(&fixture).unwrap_err();
    assert!(matches!(
        error.policy,
        DispositionEvaluationError::UnexpectedOutcome {
            actual: FixtureOutcomeClassification::ExecutionFailed(
                ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens)
            ),
            ..
        }
    ));
    assert!(matches!(
        error.details,
        Some(FixtureFailureDetails::Message(ref message))
            if message.contains("malformed token snapshot line")
    ));
}

#[test]
fn fixture_v1_declares_all_nine_expectation_surfaces_without_executing_them() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "all-surfaces", "all-surfaces", b"hello");
    for path in [
        "parse-errors.txt",
        "implementation-diagnostics.txt",
        "document-mode.txt",
        "tree.txt",
        "patches.txt",
        "transitions.whole.txt",
        "unsupported-features.txt",
        "final-invariants.txt",
    ] {
        fs::write(bundle.join(path), "# planned AE13 surface\n").expect("planned sidecar");
    }
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            r#"tokens = "tokens.txt"
parse_errors = "parse-errors.txt"
implementation_diagnostics = "implementation-diagnostics.txt"
document_mode = "document-mode.txt"
tree = "tree.txt"
patches = "patches.txt"
transitions = [{ delivery = "whole", path = "transitions.whole.txt" }]
unsupported_features = "unsupported-features.txt"
final_invariants = "final-invariants.txt""#,
        )
    });
    let fixture = discover_and_load(&repository.native()).unwrap().remove(0);
    assert!(matches!(
        fixture.expectations().implementation_diagnostics(),
        ExpectedSurface::Compare(_)
    ));
    assert_eq!(
        fixture
            .transition_deliveries()
            .map(DeliveryName::as_str)
            .collect::<Vec<_>>(),
        ["whole"]
    );
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::UnsupportedExpectation {
            surface: ExpectationSurface::ParseErrors
        }
    ));
}

#[test]
fn non_active_native_fixtures_are_rejected() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "xfail", "native-xfail", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "status = \"active\"",
            "status = \"expected-failure\"\nreason = \"known mismatch\"\nfailure = \"tokens-mismatch\"\nreference = { kind = \"tracking-issue\", value = \"#1\" }",
        )
    });
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidDisposition(_)
    ));
}

#[test]
fn capability_policy_registry_covers_every_fixture_v1_capability() {
    use super::validate::{FixtureCapabilityPolicy, capability_policy};

    let cases = [
        (
            FixtureCapability::RawByteInput,
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::ByteDelivery,
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::UnicodeScalarChunking,
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::DocumentExecution,
            FixtureCapabilityPolicy::CompletedMustRemainActive,
        ),
        (
            FixtureCapability::FragmentParsing,
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::ScriptingEnabled,
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::UnknownRequiredExtension("org.example.feature-v1".to_string()),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::Tokens),
            FixtureCapabilityPolicy::CompletedMustRemainActive,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::ParseErrors),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::ImplementationDiagnostics),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::DocumentMode),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::Tree),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::Patches),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::Transitions),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::UnsupportedFeatures),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
        (
            FixtureCapability::Expectation(ExpectationSurface::FinalInvariants),
            FixtureCapabilityPolicy::MayUseExternalDisposition,
        ),
    ];
    for (capability, expected) in cases {
        assert_eq!(capability_policy(&capability), expected, "{capability:?}");
    }
}

#[test]
fn completed_capabilities_cannot_be_hidden_by_external_dispositions() {
    let declarations = [
        "status = \"expected-unsupported\"\nreason = \"hidden\"\ncapability = { kind = \"tokens-expectation\" }\nreference = { kind = \"tracking-issue\", value = \"#1\" }",
        "status = \"expected-failure\"\nreason = \"hidden\"\nfailure = \"tokens-mismatch\"\nreference = { kind = \"tracking-issue\", value = \"#1\" }",
        "status = \"skipped\"\nreason = \"hidden\"\nclassification = { kind = \"unsupported-capability\", capability = { kind = \"tokens-expectation\" } }\nreference = { kind = \"tracking-issue\", value = \"#1\" }",
    ];
    for (index, disposition) in declarations.into_iter().enumerate() {
        let repository = TestRepository::new();
        let bundle = add_fixture(
            &repository,
            &format!("completed-{index}"),
            &format!("completed-{index}"),
            b"hello",
        );
        rewrite(&bundle.join("fixture.toml"), |text| {
            text.replace(
                "[source]\nkind = \"native\"",
                "[source]\nkind = \"external\"\nprovenance = \"upstream/case\"",
            )
            .replace("status = \"active\"", disposition)
        });
        assert!(matches!(
            discover_and_load(&repository.adapted()).unwrap_err().kind,
            FixtureLoadErrorKind::InvalidDisposition(_)
        ));
    }
}

#[test]
fn fixture_v1_rejects_broad_external_and_environment_skip_escape_hatches() {
    for (index, classification) in ["external-fixture-exclusion", "environment-requirement"]
        .into_iter()
        .enumerate()
    {
        let repository = TestRepository::new();
        let bundle = add_fixture(
            &repository,
            &format!("removed-skip-{index}"),
            &format!("removed-skip-{index}"),
            b"hello",
        );
        rewrite(&bundle.join("fixture.toml"), |text| {
            text.replace(
                "[source]\nkind = \"native\"",
                "[source]\nkind = \"external\"\nprovenance = \"upstream/case\"",
            )
            .replace(
                "status = \"active\"",
                &format!(
                    "status = \"skipped\"\nreason = \"broad skip\"\nclassification = {{ kind = \"{classification}\", capability = {{ kind = \"tokens-expectation\" }} }}\nreference = {{ kind = \"tracking-issue\", value = \"#1\" }}"
                ),
            )
        });
        assert!(matches!(
            discover_and_load(&repository.adapted()).unwrap_err().kind,
            FixtureLoadErrorKind::InvalidFixtureToml(_)
        ));
    }
}

#[test]
fn irrelevant_fragment_skip_is_rejected_before_execution() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "skipped", "skipped-fragment", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"external\"\nprovenance = \"upstream/case\"",
        )
        .replace(
            "status = \"active\"",
            "status = \"skipped\"\nreason = \"fragment unavailable\"\nclassification = { kind = \"unsupported-capability\", capability = { kind = \"fragment-parsing\" } }\nreference = { kind = \"tracking-issue\", value = \"#2\" }",
        )
    });
    let error = discover_and_load(&repository.adapted()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidDisposition(_)
    ));
    assert!(error.to_string().contains("fragment-parsing"));
    assert!(error.to_string().contains("not relevant"));
}

#[test]
fn relevant_fragment_skip_retains_exact_capability_and_bypasses_execution() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "skipped", "skipped-fragment", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"external\"\nprovenance = \"upstream/case\"",
        )
        .replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nfragment = { namespace = \"html\", local_name = \"div\" }",
        )
        .replace(
            "status = \"active\"",
            "status = \"skipped\"\nreason = \"fragment unavailable\"\nclassification = { kind = \"unsupported-capability\", capability = { kind = \"fragment-parsing\" } }\nreference = { kind = \"tracking-issue\", value = \"#2\" }",
        )
    });
    let fixture = discover_and_load(&repository.adapted()).unwrap().remove(0);
    assert!(matches!(
        fixture.disposition(),
        FixtureDisposition::Skipped {
            classification: SkipClassification::UnsupportedCapability(
                FixtureCapability::FragmentParsing
            ),
            ..
        }
    ));
    let report = run_fixture(&fixture).unwrap();
    assert_eq!(report.disposition(), DispositionEvaluation::Skip);
    assert!(report.result().is_none());
}

#[test]
fn capability_relevance_is_exact_for_every_fixture_v1_capability() {
    use super::validate::capability_is_relevant;

    fn is_relevant(capability: FixtureCapability, fixture: &ValidatedFixtureSpec) -> bool {
        capability_is_relevant(
            &capability,
            fixture.input(),
            fixture.execution(),
            fixture.expectations(),
            fixture.required_unknown_extensions(),
        )
    }

    let text_repository = TestRepository::new();
    add_fixture(&text_repository, "text", "text", b"hello");
    let text = load_single_native_fixture(&text_repository);

    let raw_repository = TestRepository::new();
    let raw_bundle = add_fixture(&raw_repository, "raw", "raw", b"hello");
    fs::rename(raw_bundle.join("input.html"), raw_bundle.join("input.bin"))
        .expect("raw input rename");
    rewrite(&raw_bundle.join("fixture.toml"), |text| {
        text.replace("path = \"input.html\"", "path = \"input.bin\"")
            .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"")
            .replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
    });
    let raw = load_single_native_fixture(&raw_repository);

    let byte_chunks_repository = TestRepository::new();
    let byte_chunks_bundle = add_fixture(
        &byte_chunks_repository,
        "byte-chunks",
        "byte-chunks",
        b"hello",
    );
    fs::rename(
        byte_chunks_bundle.join("input.html"),
        byte_chunks_bundle.join("input.bin"),
    )
    .expect("raw input rename");
    rewrite(&byte_chunks_bundle.join("fixture.toml"), |text| {
        text.replace("path = \"input.html\"", "path = \"input.bin\"")
            .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"")
            .replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
            .replace(
                "strategy = \"whole\"",
                "strategy = \"boundaries\"\nboundaries = [1, 3]",
            )
    });
    let byte_chunks = load_single_native_fixture(&byte_chunks_repository);

    let scalar_chunks_repository = TestRepository::new();
    let scalar_chunks_bundle = add_fixture(
        &scalar_chunks_repository,
        "scalar-chunks",
        "scalar-chunks",
        b"hello",
    );
    fs::write(
        scalar_chunks_bundle.join("transitions.chunks.txt"),
        "# planned transition format\n",
    )
    .expect("transition sidecar");
    rewrite(&scalar_chunks_bundle.join("fixture.toml"), |text| {
        text.replace(
            "strategy = \"whole\"\n\n[expectations]",
            "strategy = \"whole\"\n\n[[execution.deliveries]]\nname = \"chunks\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1, 3]\n\n[expectations]",
        )
        .replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\ntransitions = [{ delivery = \"chunks\", path = \"transitions.chunks.txt\" }]",
        )
    });
    let scalar_chunks = load_single_native_fixture(&scalar_chunks_repository);

    let document_repository = TestRepository::new();
    let document_bundle = add_fixture(&document_repository, "document", "document", b"hello");
    rewrite(&document_bundle.join("fixture.toml"), |text| {
        text.replace("kind = \"standalone-tokenizer\"", "kind = \"document\"")
    });
    let document = load_single_native_fixture(&document_repository);

    let scripted_document_repository = TestRepository::new();
    let scripted_document_bundle = add_fixture(
        &scripted_document_repository,
        "scripted-document",
        "scripted-document",
        b"hello",
    );
    rewrite(&scripted_document_bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"document\"\nscripting = \"enabled\"",
        )
    });
    let scripted_document = load_single_native_fixture(&scripted_document_repository);

    let fragment_repository = TestRepository::new();
    let fragment_bundle = add_fixture(&fragment_repository, "fragment", "fragment", b"hello");
    rewrite(&fragment_bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nfragment = { namespace = \"html\", local_name = \"div\" }",
        )
    });
    let fragment = load_single_native_fixture(&fragment_repository);

    let scripted_fragment_repository = TestRepository::new();
    let scripted_fragment_bundle = add_fixture(
        &scripted_fragment_repository,
        "scripted-fragment",
        "scripted-fragment",
        b"hello",
    );
    rewrite(&scripted_fragment_bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nscripting = \"enabled\"\nfragment = { namespace = \"html\", local_name = \"div\" }",
        )
    });
    let scripted_fragment = load_single_native_fixture(&scripted_fragment_repository);

    let required_extension_repository = TestRepository::new();
    let required_extension_bundle = add_fixture(
        &required_extension_repository,
        "required-extension",
        "required-extension",
        b"hello",
    );
    rewrite(
        &required_extension_bundle.join("fixture.toml"),
        |mut text| {
            text.push_str(
            "\n[extensions.\"org.example.required-v1\"]\nrequired = true\nvalue = { mode = \"strict\" }\n",
        );
            text
        },
    );
    let required_extension = load_single_native_fixture(&required_extension_repository);

    let optional_extension_repository = TestRepository::new();
    let optional_extension_bundle = add_fixture(
        &optional_extension_repository,
        "optional-extension",
        "optional-extension",
        b"hello",
    );
    rewrite(
        &optional_extension_bundle.join("fixture.toml"),
        |mut text| {
            text.push_str(
            "\n[extensions.\"org.example.required-v1\"]\nrequired = false\nvalue = { mode = \"metadata\" }\n",
        );
            text
        },
    );
    let optional_extension = load_single_native_fixture(&optional_extension_repository);

    let all_expectations_repository = TestRepository::new();
    let all_expectations_bundle = add_fixture(
        &all_expectations_repository,
        "all-expectations",
        "all-expectations",
        b"hello",
    );
    for path in [
        "parse-errors.txt",
        "implementation-diagnostics.txt",
        "document-mode.txt",
        "tree.txt",
        "patches.txt",
        "unsupported-features.txt",
        "final-invariants.txt",
        "transitions.whole.txt",
    ] {
        fs::write(all_expectations_bundle.join(path), "# planned\n").expect("sidecar");
    }
    rewrite(&all_expectations_bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\nparse_errors = \"parse-errors.txt\"\nimplementation_diagnostics = \"implementation-diagnostics.txt\"\ndocument_mode = \"document-mode.txt\"\ntree = \"tree.txt\"\npatches = \"patches.txt\"\ntransitions = [{ delivery = \"whole\", path = \"transitions.whole.txt\" }]\nunsupported_features = \"unsupported-features.txt\"\nfinal_invariants = \"final-invariants.txt\"",
        )
    });
    let all_expectations = load_single_native_fixture(&all_expectations_repository);

    let tree_only_repository = TestRepository::new();
    let tree_only_bundle = add_fixture(&tree_only_repository, "tree-only", "tree-only", b"hello");
    fs::remove_file(tree_only_bundle.join("tokens.txt")).expect("remove token sidecar");
    fs::write(tree_only_bundle.join("tree.txt"), "# planned\n").expect("tree sidecar");
    rewrite(&tree_only_bundle.join("fixture.toml"), |text| {
        text.replace("tokens = \"tokens.txt\"", "tree = \"tree.txt\"")
    });
    let tree_only = load_single_native_fixture(&tree_only_repository);

    let cases = [
        ("raw input", FixtureCapability::RawByteInput, &raw, true),
        ("text input", FixtureCapability::RawByteInput, &text, false),
        (
            "whole byte delivery",
            FixtureCapability::ByteDelivery,
            &raw,
            true,
        ),
        (
            "chunked byte delivery",
            FixtureCapability::ByteDelivery,
            &byte_chunks,
            true,
        ),
        (
            "text delivery",
            FixtureCapability::ByteDelivery,
            &text,
            false,
        ),
        (
            "declared non-reference scalar chunks",
            FixtureCapability::UnicodeScalarChunking,
            &scalar_chunks,
            true,
        ),
        (
            "whole scalar delivery",
            FixtureCapability::UnicodeScalarChunking,
            &text,
            false,
        ),
        (
            "document target",
            FixtureCapability::DocumentExecution,
            &document,
            true,
        ),
        (
            "standalone target",
            FixtureCapability::DocumentExecution,
            &text,
            false,
        ),
        (
            "fragment is not document",
            FixtureCapability::DocumentExecution,
            &fragment,
            false,
        ),
        (
            "fragment target",
            FixtureCapability::FragmentParsing,
            &fragment,
            true,
        ),
        (
            "document is not fragment",
            FixtureCapability::FragmentParsing,
            &document,
            false,
        ),
        (
            "scripted document",
            FixtureCapability::ScriptingEnabled,
            &scripted_document,
            true,
        ),
        (
            "scripted fragment",
            FixtureCapability::ScriptingEnabled,
            &scripted_fragment,
            true,
        ),
        (
            "disabled document scripting",
            FixtureCapability::ScriptingEnabled,
            &document,
            false,
        ),
        (
            "disabled fragment scripting",
            FixtureCapability::ScriptingEnabled,
            &fragment,
            false,
        ),
        (
            "standalone scripting inapplicable",
            FixtureCapability::ScriptingEnabled,
            &text,
            false,
        ),
        (
            "exact required extension",
            FixtureCapability::UnknownRequiredExtension("org.example.required-v1".to_string()),
            &required_extension,
            true,
        ),
        (
            "different required extension",
            FixtureCapability::UnknownRequiredExtension("org.example.different-v1".to_string()),
            &required_extension,
            false,
        ),
        (
            "missing required extension",
            FixtureCapability::UnknownRequiredExtension("org.example.required-v1".to_string()),
            &text,
            false,
        ),
        (
            "optional extension",
            FixtureCapability::UnknownRequiredExtension("org.example.required-v1".to_string()),
            &optional_extension,
            false,
        ),
    ];
    for (name, capability, fixture, expected) in cases {
        assert_eq!(is_relevant(capability, fixture), expected, "{name}");
    }

    for surface in [
        ExpectationSurface::Tokens,
        ExpectationSurface::ParseErrors,
        ExpectationSurface::ImplementationDiagnostics,
        ExpectationSurface::DocumentMode,
        ExpectationSurface::Tree,
        ExpectationSurface::Patches,
        ExpectationSurface::Transitions,
        ExpectationSurface::UnsupportedFeatures,
        ExpectationSurface::FinalInvariants,
    ] {
        assert!(
            is_relevant(FixtureCapability::Expectation(surface), &all_expectations),
            "declared {surface:?} expectation must be relevant"
        );
        let fixture_without_surface = if surface == ExpectationSurface::Tokens {
            &tree_only
        } else {
            &text
        };
        assert!(
            !is_relevant(
                FixtureCapability::Expectation(surface),
                fixture_without_surface
            ),
            "undeclared {surface:?} expectation must be irrelevant"
        );
    }
}

#[test]
fn disposition_policy_table_covers_exact_outcomes_and_xpass() {
    #[derive(Clone, Copy, Debug)]
    enum ExpectedEvaluation {
        Pass,
        Skip,
        Unexpected,
        Xpass,
        Incomplete,
    }

    let unsupported_fragment = FixtureDisposition::ExpectedUnsupported {
        reason: "deferred".to_string(),
        capability: FixtureCapability::FragmentParsing,
        reference: DispositionReference::TrackingIssue("#1".to_string()),
    };
    let unsupported_tree_expectation = FixtureDisposition::ExpectedUnsupported {
        reason: "deferred".to_string(),
        capability: FixtureCapability::Expectation(ExpectationSurface::Tree),
        reference: DispositionReference::TrackingIssue("#2".to_string()),
    };
    let expected_execution_failure = FixtureDisposition::ExpectedFailure {
        reason: "known failure".to_string(),
        failure: ExpectedFailureClassification::Execution(ExecutionFailureClass::SnapshotFormat(
            ExpectationSurface::Tree,
        )),
        reference: DispositionReference::TrackingIssue("#3".to_string()),
    };
    let expected_mismatch = FixtureDisposition::ExpectedFailure {
        reason: "known mismatch".to_string(),
        failure: ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::Tree),
        reference: DispositionReference::TrackingIssue("#4".to_string()),
    };
    let expected_invariant = FixtureDisposition::ExpectedFailure {
        reason: "known invariant".to_string(),
        failure: ExpectedFailureClassification::InvariantFailure(
            InvariantFailureCode::PendingTableText,
        ),
        reference: DispositionReference::TrackingIssue("#5".to_string()),
    };
    let skipped = FixtureDisposition::Skipped {
        reason: "fragment parsing unavailable".to_string(),
        classification: SkipClassification::UnsupportedCapability(
            FixtureCapability::FragmentParsing,
        ),
        reference: DispositionReference::TrackingIssue("#6".to_string()),
    };

    let cases = vec![
        (
            "active completion",
            FixtureDisposition::Active,
            completed_success(),
            ExpectedEvaluation::Pass,
        ),
        (
            "active unsupported semantics",
            FixtureDisposition::Active,
            unsupported_semantics(FixtureCapability::FragmentParsing),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "active unsupported expectation",
            FixtureDisposition::Active,
            unsupported_expectation(ExpectationSurface::Tree),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "active execution failure",
            FixtureDisposition::Active,
            execution_failure(ExecutionFailureClass::TokenizerDriver),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "active mismatch",
            FixtureDisposition::Active,
            expectation_mismatch(ExpectationSurface::Tokens),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "active invariant",
            FixtureDisposition::Active,
            invariant_failure(vec![InvariantFailureCode::PendingTableText]),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "exact unsupported semantics",
            unsupported_fragment.clone(),
            unsupported_semantics(FixtureCapability::FragmentParsing),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong unsupported semantics",
            unsupported_fragment.clone(),
            unsupported_semantics(FixtureCapability::ScriptingEnabled),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "unsupported semantics xpass",
            unsupported_fragment,
            completed_success(),
            ExpectedEvaluation::Xpass,
        ),
        (
            "exact unsupported expectation",
            unsupported_tree_expectation.clone(),
            unsupported_expectation(ExpectationSurface::Tree),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong unsupported expectation",
            unsupported_tree_expectation,
            unsupported_expectation(ExpectationSurface::Patches),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "exact execution failure",
            expected_execution_failure.clone(),
            execution_failure(ExecutionFailureClass::SnapshotFormat(
                ExpectationSurface::Tree,
            )),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong execution failure",
            expected_execution_failure.clone(),
            execution_failure(ExecutionFailureClass::SnapshotRead(
                ExpectationSurface::Tree,
            )),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "execution failure xpass",
            expected_execution_failure,
            completed_success(),
            ExpectedEvaluation::Xpass,
        ),
        (
            "exact expectation mismatch",
            expected_mismatch.clone(),
            expectation_mismatch(ExpectationSurface::Tree),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong expectation mismatch",
            expected_mismatch,
            expectation_mismatch(ExpectationSurface::Patches),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "exact invariant failure",
            expected_invariant.clone(),
            invariant_failure(vec![InvariantFailureCode::PendingTableText]),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong invariant failure",
            expected_invariant.clone(),
            invariant_failure(vec![InvariantFailureCode::InvalidInsertionMode]),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "multiple invariant failures do not match one declaration",
            expected_invariant,
            invariant_failure(vec![
                InvariantFailureCode::PendingTableText,
                InvariantFailureCode::InvalidInsertionMode,
            ]),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "skipped is not executed",
            skipped.clone(),
            FixtureExecutionOutcome::NotExecuted,
            ExpectedEvaluation::Skip,
        ),
        (
            "skipped execution is rejected",
            skipped,
            completed_success(),
            ExpectedEvaluation::Unexpected,
        ),
        (
            "incomplete active observation",
            FixtureDisposition::Active,
            incomplete_observation(),
            ExpectedEvaluation::Incomplete,
        ),
    ];

    for (name, disposition, outcome, expected) in cases {
        let actual = evaluate_disposition(&disposition, &outcome);
        let matched = match expected {
            ExpectedEvaluation::Pass => actual == Ok(DispositionEvaluation::Pass),
            ExpectedEvaluation::Skip => actual == Ok(DispositionEvaluation::Skip),
            ExpectedEvaluation::Unexpected => matches!(
                actual,
                Err(DispositionEvaluationError::UnexpectedOutcome { .. })
            ),
            ExpectedEvaluation::Xpass => {
                matches!(actual, Err(DispositionEvaluationError::Xpass { .. }))
            }
            ExpectedEvaluation::Incomplete => {
                actual == Err(DispositionEvaluationError::IncompleteObservation)
            }
        };
        assert!(matched, "{name}: got {actual:?}");
    }
}

#[test]
fn captured_empty_is_distinct_and_incomplete_results_are_non_authoritative() {
    assert_ne!(
        ObservationState::<Vec<u8>>::Captured(Vec::new()),
        ObservationState::NotRequested
    );
    let outcome = incomplete_observation();
    assert_eq!(
        evaluate_disposition(&FixtureDisposition::Active, &outcome),
        Err(DispositionEvaluationError::IncompleteObservation)
    );
}

fn completed_success() -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::Completed {
        result: Box::new(canonical_result()),
    }
}

fn unsupported_semantics(capability: FixtureCapability) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability }
}

fn unsupported_expectation(surface: ExpectationSurface) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::UnsupportedExpectation { surface }
}

fn execution_failure(class: ExecutionFailureClass) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::ExecutionFailed {
        class,
        message: "failure".to_string(),
    }
}

fn expectation_mismatch(surface: ExpectationSurface) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::ExpectationMismatch {
        result: Box::new(canonical_result()),
        surface,
        diff: "diff".to_string(),
    }
}

fn invariant_failure(failures: Vec<InvariantFailureCode>) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::InvariantFailed {
        result: Box::new(canonical_result()),
        failures,
    }
}

fn incomplete_observation() -> FixtureExecutionOutcome {
    let mut result = canonical_result();
    result.implementation_diagnostics = ObservationState::Incomplete {
        partial: Vec::new(),
        reason: IncompleteObservationReason::StorageLimitExceeded {
            retained: 0,
            dropped: 1,
        },
    };
    FixtureExecutionOutcome::IncompleteObservation {
        result: Box::new(result),
    }
}

fn canonical_result() -> CanonicalParserResult {
    CanonicalParserResult {
        tokens: ObservationState::NotRequested,
        parse_errors: ObservationState::NotRequested,
        implementation_diagnostics: ObservationState::NotRequested,
        document_mode: ObservationState::NotRequested,
        tree: ObservationState::NotRequested,
        patches: ObservationState::NotRequested,
        transitions: ObservationState::NotRequested,
        unsupported_features: ObservationState::NotRequested,
        final_invariants: ObservationState::NotRequested,
    }
}

#[test]
fn adapted_repository_accepts_external_non_active_schema_for_policy_evaluation() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "external", "external-fragment", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"external\"\nprovenance = \"upstream/case-1\"",
        )
        .replace(
            "status = \"active\"",
            "status = \"expected-unsupported\"\nreason = \"fragment parsing deferred\"\ncapability = { kind = \"fragment-parsing\" }\nreference = { kind = \"tracking-issue\", value = \"#3\" }",
        )
        .replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"fragment\"\nfragment = { namespace = \"html\", local_name = \"div\" }",
        )
    });
    let fixture = discover_and_load(&repository.adapted()).unwrap().remove(0);
    assert!(matches!(
        fixture.disposition(),
        FixtureDisposition::ExpectedUnsupported { .. }
    ));
    let report = run_fixture(&fixture).expect("exact expected unsupported classification passes");
    assert_eq!(report.disposition(), DispositionEvaluation::Pass);
    assert!(report.result().is_none());
}
