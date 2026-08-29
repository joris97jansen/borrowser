use super::disposition::*;
use super::mismatch::first_typed_parity_mismatch;
use super::model::*;
use super::runner::{
    FixtureFailureDetails, execute_fixture, resolve_scalar_ordinals,
    with_scalar_boundary_offset_failure, with_serialization_counter,
};
use super::*;
use html::conformance::{
    CanonicalParserResult, IncompleteObservationReason, InvariantFailureCode, ObservationState,
    ParserObservationDeliveryError, ParserObservationExecutionError, ParserObservationRequest,
};
use ring::digest::{SHA256, digest};
use std::collections::BTreeSet;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct TestRepository {
    _temp: TempDir,
    repository_root: PathBuf,
    fixture_root: PathBuf,
}

#[derive(Default)]
struct RecordingFileAccess {
    metadata_checks: Vec<String>,
    content_reads: Vec<String>,
    fail_content_reads: BTreeSet<String>,
}

#[derive(Default)]
struct CountingObservationExecutor {
    calls: usize,
}

#[test]
fn scalar_boundary_offset_reservation_failure_is_typed_and_pre_parser() {
    let error = with_scalar_boundary_offset_failure(|| {
        resolve_scalar_ordinals("aé", 2, &[1]).expect_err("injected allocation failure")
    });
    assert!(matches!(
        error,
        super::runner::ResolveExecutionError::ScalarBoundaryOffsets
    ));
}

#[test]
fn scalar_boundary_offset_failure_stops_fixture_runner_before_candidate_parser() {
    struct CountingProductionExecutor {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for CountingProductionExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            html::conformance::execute_parser_observation(request)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "scalar-resource", "scalar-resource", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1]\n\n[expectations]",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    let mut executor = CountingProductionExecutor { calls: 0 };
    let outcome = with_scalar_boundary_offset_failure(|| {
        super::runner::execute_fixture_v2_with(&fixture, &mut executor)
    });
    assert_eq!(
        executor.calls, 1,
        "only the whole baseline reaches the parser"
    );
    assert!(matches!(
        &outcome,
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::FixtureExecutionResourceExhaustion(
                FixtureExecutionResourceSite::ScalarBoundaryExecutionOffsets
            ),
            ..
        }
    ));
    assert_eq!(
        super::failure_spelling::execution_failure_name(
            ExecutionFailureClass::FixtureExecutionResourceExhaustion(
                FixtureExecutionResourceSite::ScalarBoundaryExecutionOffsets,
            ),
        ),
        "fixture-execution-resource-exhaustion:scalar-boundary-execution-offsets"
    );
}

#[test]
fn validated_fixture_delivery_rejection_is_a_runner_contradiction() {
    struct RejectingCandidate {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for RejectingCandidate {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            if self.calls == 1 {
                return html::conformance::execute_parser_observation(request);
            }
            Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::BoundaryOutOfRange { boundary_index: 7 },
            ))
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(
        &repository,
        "validated-rejection",
        "validated-rejection",
        b"hello",
    );
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1]\n\n[expectations]",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    let mut executor = RejectingCandidate { calls: 0 };
    let outcome = super::runner::execute_fixture_v2_with(&fixture, &mut executor);
    assert_eq!(
        executor.calls, 2,
        "baseline and one rejected candidate execute"
    );
    assert!(matches!(
        &outcome,
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::ValidatedFixtureInvariant(
                ValidatedFixtureInvariantCode::ValidatedBoundaryRejectedByExecutor,
            ),
            ..
        }
    ));
    assert_eq!(
        super::failure_spelling::runner_invariant_name(
            ValidatedFixtureInvariantCode::ValidatedBoundaryRejectedByExecutor,
        ),
        "validated-boundary-rejected-by-executor"
    );
    assert!(!matches!(
        &outcome,
        FixtureExecutionOutcome::CompletedV2 { .. }
    ));
}

impl super::runner::ParserObservationExecutor for CountingObservationExecutor {
    fn execute(
        &mut self,
        _: ParserObservationRequest<'_>,
    ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
        self.calls += 1;
        Ok(canonical_result())
    }
}

impl super::load::FixtureFileAccess for RecordingFileAccess {
    fn validate_regular_file_metadata(
        &mut self,
        bundle: &FixtureBundle,
        relative: &str,
    ) -> Result<(), FixtureLoadError> {
        self.metadata_checks.push(relative.to_string());
        super::load::validate_regular_file_metadata(bundle, relative)
    }

    fn read_regular_file(
        &mut self,
        bundle: &FixtureBundle,
        relative: &str,
    ) -> Result<Vec<u8>, FixtureLoadError> {
        self.content_reads.push(relative.to_string());
        if self.fail_content_reads.contains(relative) {
            return Err(FixtureLoadError {
                path: format!("{}/{}", bundle.repository_relative_path(), relative),
                kind: FixtureLoadErrorKind::Io(
                    "injected complete-content read failure".to_string(),
                ),
            });
        }
        super::load::read_regular_file(bundle, relative)
    }
}

impl RecordingFileAccess {
    fn metadata_count(&self, relative: &str) -> usize {
        self.metadata_checks
            .iter()
            .filter(|value| value.as_str() == relative)
            .count()
    }

    fn content_read_count(&self, relative: &str) -> usize {
        self.content_reads
            .iter()
            .filter(|value| value.as_str() == relative)
            .count()
    }
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

fn add_fixture_v2(repository: &TestRepository, directory: &str, id: &str, input: &[u8]) -> PathBuf {
    let bundle = repository.fixture_root.join(directory);
    fs::create_dir_all(&bundle).expect("bundle");
    fs::write(bundle.join("input.html"), input).expect("input");
    fs::write(
        bundle.join("tokens.txt"),
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"hello\"\nTOKEN ordinal=2 kind=eof\n",
    )
    .expect("tokens");
    fs::write(bundle.join("fixture.toml"), fixture_toml_v2(id, input)).expect("metadata");
    bundle
}

fn fixture_toml_v2(id: &str, input: &[u8]) -> String {
    fixture_toml(id, input).replace(
        "borrowser-html-parser-fixture-v1",
        "borrowser-html-parser-fixture-v2",
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
fn fixture_loader_dispatches_exact_v1_and_v2_schemas_before_validation() {
    let repository = TestRepository::new();
    add_fixture(&repository, "legacy", "legacy", b"hello");
    add_fixture_v2(&repository, "canonical", "canonical", b"hello");
    let fixtures = discover_and_load(&repository.native()).expect("both versions load");
    assert_eq!(fixtures.len(), 2);
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.format() == FixtureFormatVersion::V1)
    );
    assert!(
        fixtures
            .iter()
            .any(|fixture| fixture.format() == FixtureFormatVersion::V2)
    );

    let unknown = TestRepository::new();
    let bundle = add_fixture_v2(&unknown, "unknown", "unknown", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(FIXTURE_FORMAT_V2, "borrowser-html-parser-fixture-v99")
    });
    assert!(
        matches!(discover_and_load(&unknown.native()).unwrap_err().kind, FixtureLoadErrorKind::UnsupportedFixtureFormat(ref value) if value == "borrowser-html-parser-fixture-v99")
    );

    let strict = TestRepository::new();
    let bundle = add_fixture_v2(&strict, "strict", "strict", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        format!("{text}\nunknown = true\n")
    });
    assert!(matches!(
        discover_and_load(&strict.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidFixtureToml(_)
    ));
}

#[test]
fn public_execution_model_and_parse_error_strength_are_validated_semantics() {
    let legacy = TestRepository::new();
    add_fixture(&legacy, "legacy", "legacy", b"hello");
    let fixture = load_single_native_fixture(&legacy);
    assert_eq!(
        fixture.execution_model(),
        ParserFixtureExecutionModel::LegacySingleDelivery
    );

    let exact = TestRepository::new();
    let bundle = add_fixture_v2(&exact, "exact", "exact", b"hello");
    fs::write(
        bundle.join("parse-errors.txt"),
        "# format: html5-parse-errors-v1\n",
    )
    .expect("exact parse-error snapshot");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\nparse_errors = \"parse-errors.txt\"",
        )
    });
    let fixture = load_single_native_fixture(&exact);
    assert_eq!(
        fixture.execution_model(),
        ParserFixtureExecutionModel::CanonicalObservationParity
    );
    assert!(fixture.declared_expectations().any(|expectation| matches!(
        expectation,
        DeclaredExpectation::ParseErrors(ParseErrorExpectationStrength::Exact)
    )));

    let count = TestRepository::new();
    let bundle = add_fixture_v2(&count, "count", "count", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(FIXTURE_FORMAT_V2, FIXTURE_FORMAT_V3).replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\nparse_errors = { kind = \"count\", count = 3 }",
        )
    });
    let fixture = load_single_native_fixture(&count);
    assert_eq!(
        fixture.execution_model(),
        ParserFixtureExecutionModel::CanonicalObservationParity
    );
    assert!(fixture.declared_expectations().any(|expectation| matches!(
        expectation,
        DeclaredExpectation::ParseErrors(ParseErrorExpectationStrength::Count { expected: 3 })
    )));
}

#[test]
fn rich_evaluation_retains_mismatch_observation_without_recovery_rerun() {
    struct CountingExecutor {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for CountingExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            html::conformance::execute_parser_observation(request)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "mismatch", "mismatch", b"hello");
    fs::write(
        bundle.join("tokens.txt"),
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"different\"\nTOKEN ordinal=2 kind=eof\n",
    )
    .expect("valid mismatching snapshot");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1]\n\n[expectations]",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    let mut executor = CountingExecutor { calls: 0 };
    let mut access = super::load::ProductionFixtureFileAccess;
    let evaluation = super::runner::evaluate_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut access,
    );
    let planned_execution_calls = executor.calls;
    assert!(
        planned_execution_calls > 1,
        "the canonical AE plan retains baseline and parity executions"
    );
    assert!(matches!(
        evaluation.observed_outcome(),
        FixtureObservedOutcome::ExpectationMismatch {
            surface: ExpectationSurface::Tokens,
            ..
        }
    ));
    assert!(
        evaluation
            .serialize_reference_observation(ExpectationSurface::Tokens)
            .expect("canonical serialization")
            .is_some()
    );
    assert_eq!(
        executor.calls, planned_execution_calls,
        "serializing retained evidence does not rerun the parser"
    );
}

#[test]
fn fixture_v1_compatibility_path_has_no_representatives_or_mandatory_final_audit() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};
    struct MustNotExecute;
    impl super::runner::ParserObservationExecutor for MustNotExecute {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            panic!("fixture-v1 must not enter the fixture-v2 strategy executor")
        }
    }

    let repository = TestRepository::new();
    add_fixture(&repository, "legacy", "legacy", b"hello");
    let fixture = load_single_native_fixture(&repository);
    assert!(matches!(
        fixture.execution_plan(),
        ValidatedExecutionPlan::SingleDelivery(_)
    ));
    let report = super::runner::run_fixture_with_executor(&fixture, &mut MustNotExecute)
        .expect("existing fixture-v1 bundle remains compatible");
    let result = report.result().expect("fixture-v1 canonical result");
    assert!(matches!(
        result.final_invariants,
        ObservationState::NotRequested
    ));
    assert!(report.delivery_results().is_empty());
}

#[test]
fn fixture_v1_active_discovery_reads_sidecar_and_execution_rereads_it() {
    let repository = TestRepository::new();
    add_fixture(&repository, "legacy-active", "legacy-active", b"hello");
    let mut file_access = RecordingFileAccess::default();
    let fixture =
        super::load::discover_and_load_with_access(&repository.native(), &mut file_access)
            .expect("fixture-v1 discovery reads a valid sidecar")
            .remove(0);

    assert_eq!(file_access.metadata_count("tokens.txt"), 0);
    assert_eq!(file_access.content_read_count("tokens.txt"), 1);

    let mut executor = CountingObservationExecutor::default();
    let report = super::runner::run_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut file_access,
    )
    .expect("legacy fixture completes after its execution-time reread");
    assert_eq!(file_access.content_read_count("tokens.txt"), 2);
    assert_eq!(
        executor.calls, 0,
        "fixture-v1 keeps its legacy tokenizer path"
    );
    assert_eq!(report.disposition(), DispositionEvaluation::Pass);
    assert!(report.result().is_some());
}

#[test]
fn fixture_v1_sidecar_read_failure_remains_a_discovery_error() {
    let repository = TestRepository::new();
    add_fixture(
        &repository,
        "legacy-unreadable",
        "legacy-unreadable",
        b"hello",
    );
    let mut file_access = RecordingFileAccess {
        fail_content_reads: BTreeSet::from(["tokens.txt".to_string()]),
        ..RecordingFileAccess::default()
    };

    let error = super::load::discover_and_load_with_access(&repository.native(), &mut file_access)
        .expect_err("fixture-v1 unreadable sidecar must fail discovery");
    assert!(
        matches!(error.kind, FixtureLoadErrorKind::Io(ref message) if message == "injected complete-content read failure")
    );
    assert_eq!(file_access.metadata_count("tokens.txt"), 0);
    assert_eq!(file_access.content_read_count("tokens.txt"), 1);
}

#[test]
fn fixture_v1_skipped_discovery_retains_legacy_sidecar_read() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "legacy-skipped", "legacy-skipped", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
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
    let mut file_access = RecordingFileAccess::default();
    let fixture =
        super::load::discover_and_load_with_access(&repository.adapted(), &mut file_access)
            .expect("fixture-v1 skipped discovery retains the legacy read")
            .remove(0);
    assert_eq!(file_access.metadata_count("tokens.txt"), 0);
    assert_eq!(file_access.content_read_count("tokens.txt"), 1);

    let mut executor = CountingObservationExecutor::default();
    let report = super::runner::run_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut file_access,
    )
    .expect("validated legacy skip remains not executed");
    assert_eq!(file_access.content_read_count("tokens.txt"), 1);
    assert_eq!(executor.calls, 0);
    assert_eq!(report.disposition(), DispositionEvaluation::Skip);
    assert!(report.result().is_none());
    assert!(report.delivery_results().is_empty());
}

#[test]
fn fixture_v1_execution_reread_failure_keeps_legacy_snapshot_read_classification() {
    let repository = TestRepository::new();
    add_fixture(
        &repository,
        "legacy-reread-failure",
        "legacy-reread-failure",
        b"hello",
    );
    let mut file_access = RecordingFileAccess::default();
    let fixture =
        super::load::discover_and_load_with_access(&repository.native(), &mut file_access)
            .expect("fixture-v1 sidecar is readable during discovery")
            .remove(0);
    file_access
        .fail_content_reads
        .insert("tokens.txt".to_string());

    let mut executor = CountingObservationExecutor::default();
    let error = super::runner::run_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut file_access,
    )
    .expect_err("legacy execution-time reread failure remains classified");
    assert_eq!(file_access.content_read_count("tokens.txt"), 2);
    assert_eq!(executor.calls, 0);
    assert!(matches!(
        error.policy,
        DispositionEvaluationError::UnexpectedOutcome {
            actual: FixtureOutcomeClassification::ExecutionFailedV1(
                LegacyExecutionFailureClass::SnapshotRead(ExpectationSurface::Tokens)
            ),
            ..
        }
    ));
    assert!(matches!(
        error.details,
        Some(FixtureFailureDetails::Message(ref message))
            if message.contains("injected complete-content read failure")
    ));
}

#[test]
fn every_fixture_v1_expected_failure_spelling_remains_accepted_unchanged() {
    #[derive(serde::Deserialize)]
    struct Holder {
        failure: ExpectedFailureDeclaration,
    }
    let spellings = [
        "token-snapshot-read",
        "token-snapshot-format",
        "tokenizer-driver",
        "validated-fixture-invariant",
        "tokens-mismatch",
        "parse-errors-mismatch",
        "implementation-diagnostics-mismatch",
        "document-mode-mismatch",
        "tree-mismatch",
        "patches-mismatch",
        "transitions-mismatch",
        "unsupported-features-mismatch",
        "final-invariants-mismatch",
        "decoder-carry-not-empty-invariant",
        "preprocessing-not-flushed-invariant",
        "eof-emission-invalid-invariant",
        "pending-tokenizer-construct-invariant",
        "tokenizer-output-unaccounted-invariant",
        "pending-table-text-invariant",
        "invalid-insertion-mode-invariant",
        "open-elements-inconsistent-invariant",
        "active-formatting-inconsistent-invariant",
        "template-modes-inconsistent-invariant",
        "form-pointer-invalid-invariant",
        "parent-child-relationship-invalid-invariant",
        "namespace-relationship-invalid-invariant",
        "template-association-invalid-invariant",
        "patch-materialization-incomplete-invariant",
        "live-tree-mismatch-invariant",
    ];
    for spelling in spellings {
        let parsed: Holder = toml::from_str(&format!("failure = \"{spelling}\""))
            .unwrap_or_else(|error| panic!("fixture-v1 spelling {spelling} changed: {error}"));
        let _identity = parsed.failure;
    }
}

#[test]
fn fixture_v2_uses_structured_failure_tables_and_rejects_v1_scalar_syntax() {
    let structured = fixture_toml_v2("structured", b"hello").replace(
        "[disposition]\nstatus = \"active\"",
        "[disposition]\nstatus = \"expected-failure\"\nreason = \"known\"\nreference = { kind = \"tracking-issue\", value = \"#1\" }\n\n[disposition.failure]\nkind = \"parser-observation\"\nidentity = \"tokenizer-invariant\"\ncode = \"pending-text-range-invalid\"",
    );
    let parsed: FixtureFileV2 = toml::from_str(&structured).expect("structured v2 failure");
    assert!(matches!(
        parsed.disposition.failure,
        Some(ExpectedFailureDeclarationV2 {
            kind: ExpectedFailureKindDeclarationV2::ParserObservation,
            ..
        })
    ));

    let scalar = structured.replace(
        "\n[disposition.failure]\nkind = \"parser-observation\"\nidentity = \"tokenizer-invariant\"\ncode = \"pending-text-range-invalid\"",
        "\nfailure = \"tokenizer-driver\"",
    );
    assert!(toml::from_str::<FixtureFileV2>(&scalar).is_err());
}

#[test]
fn fixture_v2_failure_spellings_round_trip_exhaustively_through_one_codec() {
    use super::failure_spelling::*;
    use html::conformance::{ParserFatalIdentity, ParserObservationExecutionIdentity as I};
    use std::collections::BTreeSet;

    let mut identities = vec![
        I::ParserFatal(ParserFatalIdentity::EngineInvariant),
        I::ParserInvariant,
        I::TokenCanonicalizationInvariant,
        I::TreeTransitionTokenCanonicalizationInvariant,
        I::ObservationRecorderMissing,
        I::PatchHistoryCaptureMissing,
    ];
    identities.extend(
        all_parser_reservation_sites()
            .iter()
            .copied()
            .map(|site| I::ParserFatal(ParserFatalIdentity::ResourceExhaustion(site))),
    );
    identities.extend(
        all_tokenizer_invariants()
            .iter()
            .copied()
            .map(I::TokenizerInvariant),
    );
    identities.extend(
        all_unsupported_observation_invariants()
            .iter()
            .copied()
            .map(I::UnsupportedFeatureObservationInvariant),
    );
    identities.extend(
        all_observation_invariants()
            .iter()
            .copied()
            .map(I::ObservationInvariant),
    );
    identities.extend(
        all_observation_reservation_sites()
            .iter()
            .copied()
            .map(I::ResourceExhaustion),
    );
    identities.extend(
        all_delivery_errors()
            .iter()
            .copied()
            .map(I::InvalidDelivery),
    );
    for error in all_delivery_errors() {
        assert_eq!(delivery_error_name(*error), error.diagnostic_name());
    }

    let mut names = BTreeSet::new();
    for identity in identities {
        let spelling = parser_observation_failure_spelling(identity);
        let reparsed =
            parse_parser_observation_failure(spelling.identity, spelling.code, spelling.site)
                .expect("canonical spelling parses");
        assert_eq!(reparsed, identity);
        assert!(names.insert(parser_observation_failure_name(identity)));
    }

    for code in all_runner_invariants() {
        assert_eq!(
            parse_runner_invariant(runner_invariant_name(*code)),
            Ok(*code)
        );
    }
    assert!(parse_runner_invariant("not-a-runner-invariant").is_err());
    assert!(
        parse_parser_observation_failure(
            "tokenizer-invariant",
            Some("not-a-tokenizer-invariant"),
            None,
        )
        .is_err()
    );
    assert!(
        parse_parser_observation_failure(
            "parser-fatal-resource-exhaustion",
            None,
            Some("not-a-parser-site"),
        )
        .is_err()
    );
    assert!(
        parse_parser_observation_failure("parser-invariant", Some("contradictory-code"), None,)
            .is_err()
    );
    assert!(
        parse_parser_observation_failure(
            "observation-resource-exhaustion",
            Some("contradictory-code"),
            Some("canonical-tree-projection"),
        )
        .is_err()
    );
    assert!(
        parse_parser_observation_failure("not-a-parser-observation-identity", None, None).is_err()
    );
}

#[test]
fn disposition_mismatch_names_exact_expected_and_actual_parser_identities() {
    use html::conformance::{
        ParserObservationExecutionIdentity as I, ParserTokenizerInvariantError as T,
    };

    let disposition = FixtureDisposition::ExpectedFailureV2 {
        reason: "known".to_string(),
        failure: ExpectedFailureClassificationV2::Execution(
            ExecutionFailureClass::ParserObservation(I::TokenizerInvariant(
                T::PendingTextRangeInvalid,
            )),
        ),
        reference: DispositionReference::TrackingIssue("#1".to_string()),
    };
    let outcome = FixtureExecutionOutcome::ExecutionFailedV2 {
        class: ExecutionFailureClass::ParserObservation(I::ParserInvariant),
        message: "wording is not classification".to_string(),
    };
    let diagnostic = evaluate_disposition(&disposition, &outcome)
        .expect_err("different typed identities do not match")
        .to_string();
    assert!(
        diagnostic.contains("parser-observation:tokenizer-invariant:pending-text-range-invalid"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("parser-observation:parser-invariant"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("wording is not classification"));
}

#[test]
fn fixture_v2_expected_final_invariant_requires_an_exact_singleton() {
    let disposition = FixtureDisposition::ExpectedFailureV2 {
        reason: "known final audit failure".to_string(),
        failure: ExpectedFailureClassificationV2::FinalInvariant(
            InvariantFailureCode::PendingTableText,
        ),
        reference: DispositionReference::TrackingIssue("#ae13c".to_string()),
    };
    let exact = FixtureExecutionOutcome::FinalInvariantFailedV2 {
        strategy: "ordinal=1".to_string(),
        first_failure: InvariantFailureCode::PendingTableText,
        failure_count: 1,
    };
    assert_eq!(
        evaluate_disposition(&disposition, &exact),
        Ok(DispositionEvaluation::Pass)
    );
    for outcome in [
        FixtureExecutionOutcome::FinalInvariantFailedV2 {
            strategy: "ordinal=1".to_string(),
            first_failure: InvariantFailureCode::PendingTableText,
            failure_count: 2,
        },
        FixtureExecutionOutcome::FinalInvariantFailedV2 {
            strategy: "ordinal=1".to_string(),
            first_failure: InvariantFailureCode::InvalidInsertionMode,
            failure_count: 1,
        },
    ] {
        assert!(matches!(
            evaluate_disposition(&disposition, &outcome),
            Err(DispositionEvaluationError::UnexpectedOutcome { .. })
        ));
    }
}

#[test]
fn required_unknown_extensions_are_selected_in_ascii_lexicographic_order() {
    fn selected(extension_tables: &str) -> FixtureCapability {
        let repository = TestRepository::new();
        let bundle = add_fixture_v2(&repository, "extensions", "extensions", b"hello");
        rewrite(&bundle.join("fixture.toml"), |text| {
            format!("{text}\n{extension_tables}")
        });
        let fixture = load_single_native_fixture(&repository);
        match execute_fixture(&fixture) {
            FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability } => capability,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    let first = selected(
        "[extensions.\"org.zeta.feature-v1\"]\nrequired = true\nvalue = {}\n[extensions.\"org.alpha.feature-v1\"]\nrequired = true\nvalue = {}\n",
    );
    let second = selected(
        "[extensions.\"org.alpha.feature-v1\"]\nrequired = true\nvalue = {}\n[extensions.\"org.zeta.feature-v1\"]\nrequired = true\nvalue = {}\n",
    );
    assert_eq!(
        first,
        FixtureCapability::UnknownRequiredExtension("org.alpha.feature-v1".to_string())
    );
    assert_eq!(first, second);
}

#[test]
fn fixture_v2_skipped_disposition_short_circuits_malformed_sidecars_and_execution() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};

    struct CountingExecutor {
        calls: usize,
    }

    impl super::runner::ParserObservationExecutor for CountingExecutor {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            Ok(canonical_result())
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "skipped", "skipped", b"hello");
    let mut malformed = "malformed\n".to_string();
    malformed.push_str(&"x".repeat(256 * 1024));
    fs::write(bundle.join("tokens.txt"), malformed).expect("large malformed sidecar");
    rewrite(&bundle.join("fixture.toml"), |text| {
        format!("{}\n[extensions.\"org.example.required-v1\"]\nrequired = true\nvalue = {{}}\n", text.replace("[source]\nkind = \"native\"", "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"")
            .replace("[expectations]", "[[execution.deliveries]]\nname = \"chunked\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1]\n\n[expectations]")
            .replace("status = \"active\"", "status = \"skipped\"\nreason = \"chunking deferred\"\nclassification = { kind = \"unsupported-capability\", capability = { kind = \"unicode-scalar-chunking\" } }\nreference = { kind = \"tracking-issue\", value = \"#1\" }"))
    });
    let mut file_access = RecordingFileAccess {
        fail_content_reads: BTreeSet::from(["tokens.txt".to_string()]),
        ..RecordingFileAccess::default()
    };
    let fixture =
        super::load::discover_and_load_with_access(&repository.adapted(), &mut file_access)
            .expect("metadata-only validation does not read the failing sidecar")
            .remove(0);
    assert_eq!(file_access.metadata_count("tokens.txt"), 1);
    assert_eq!(file_access.content_read_count("tokens.txt"), 0);
    let mut executor = CountingExecutor { calls: 0 };
    let report = super::runner::run_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut file_access,
    )
    .expect("skip bypasses content reads and execution");
    assert_eq!(executor.calls, 0);
    assert_eq!(file_access.content_read_count("tokens.txt"), 0);
    assert_eq!(report.disposition(), DispositionEvaluation::Skip);
    assert!(report.result().is_none());
    assert!(report.delivery_results().is_empty());
}

#[test]
fn skipped_fixture_precedes_unsupported_raw_input_without_reading_sidecars() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};

    struct CountingExecutor(usize);
    impl super::runner::ParserObservationExecutor for CountingExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.0 += 1;
            html::conformance::execute_parser_observation(request)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "raw-skip", "raw-skip", b"hello");
    fs::rename(bundle.join("input.html"), bundle.join("input.bin")).expect("raw input rename");
    fs::write(
        bundle.join("tokens.txt"),
        "malformed and deliberately unreadable",
    )
    .expect("malformed sidecar");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
        )
        .replace("path = \"input.html\"", "path = \"input.bin\"")
        .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"")
        .replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
        .replace(
            "status = \"active\"",
            "status = \"skipped\"\nreason = \"raw input deferred\"\nclassification = { kind = \"unsupported-capability\", capability = { kind = \"raw-byte-input\" } }\nreference = { kind = \"tracking-issue\", value = \"#2\" }",
        )
    });

    let mut file_access = RecordingFileAccess {
        fail_content_reads: BTreeSet::from(["tokens.txt".to_string()]),
        ..RecordingFileAccess::default()
    };
    let fixture =
        super::load::discover_and_load_with_access(&repository.adapted(), &mut file_access)
            .expect("raw skipped fixture passes declaration validation")
            .remove(0);
    let mut executor = CountingExecutor(0);
    let report = super::runner::run_fixture_with_executor_and_access(
        &fixture,
        &mut executor,
        &mut file_access,
    )
    .expect("skip precedes unsupported input and byte delivery checks");
    assert_eq!(file_access.metadata_count("tokens.txt"), 1);
    assert_eq!(file_access.content_read_count("tokens.txt"), 0);
    assert_eq!(executor.0, 0);
    assert_eq!(report.disposition(), DispositionEvaluation::Skip);
    assert!(report.result().is_none());
    assert!(report.delivery_results().is_empty());
}

#[test]
fn active_fixture_reads_each_expected_sidecar_once_after_metadata_validation() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};

    struct CountingExecutor(usize);
    impl super::runner::ParserObservationExecutor for CountingExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.0 += 1;
            html::conformance::execute_parser_observation(request)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "active-read", "active-read", b"hello");
    fs::write(bundle.join("tokens.txt"), "malformed snapshot").expect("malformed active sidecar");
    let mut file_access = RecordingFileAccess::default();
    let fixture =
        super::load::discover_and_load_with_access(&repository.native(), &mut file_access)
            .expect("metadata validation does not parse snapshot content")
            .remove(0);
    assert_eq!(file_access.metadata_count("tokens.txt"), 1);
    assert_eq!(file_access.content_read_count("tokens.txt"), 0);

    let mut executor = CountingExecutor(0);
    let outcome =
        super::runner::execute_fixture_v2_with_access(&fixture, &mut executor, &mut file_access);
    assert!(matches!(
        outcome,
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
            ..
        }
    ));
    assert_eq!(file_access.content_read_count("tokens.txt"), 1);
    assert_eq!(
        executor.0, 1,
        "authoritative baseline precedes sidecar parsing"
    );
}

#[test]
fn unsupported_input_and_unknown_extension_precede_malformed_v2_sidecars() {
    let raw_repository = TestRepository::new();
    let raw_bundle = add_fixture_v2(&raw_repository, "raw", "raw", b"hello");
    fs::rename(raw_bundle.join("input.html"), raw_bundle.join("input.bin"))
        .expect("raw input rename");
    fs::write(raw_bundle.join("tokens.txt"), "malformed").expect("malformed sidecar");
    rewrite(&raw_bundle.join("fixture.toml"), |text| {
        text.replace("path = \"input.html\"", "path = \"input.bin\"")
            .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"")
            .replace("unit = \"unicode-scalars\"", "unit = \"bytes\"")
            .replace(
                "[source]\nkind = \"native\"",
                "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
            )
    });
    let raw = discover_and_load(&raw_repository.adapted())
        .expect("valid raw fixture")
        .remove(0);
    assert!(matches!(
        execute_fixture(&raw),
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
            ..
        }
    ));

    let extension_repository = TestRepository::new();
    let extension_bundle =
        add_fixture_v2(&extension_repository, "extension", "extension", b"hello");
    fs::write(extension_bundle.join("tokens.txt"), "malformed").expect("malformed sidecar");
    rewrite(&extension_bundle.join("fixture.toml"), |text| {
        format!("{text}\n[extensions.\"org.example.required-v1\"]\nrequired = true\nvalue = {{}}\n")
    });
    let extension = load_single_native_fixture(&extension_repository);
    assert!(matches!(
        execute_fixture(&extension),
        FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::UnknownRequiredExtension(ref value)
        } if value == "org.example.required-v1"
    ));
}

#[test]
fn fixture_v2_semantically_aliases_duplicate_declarations_and_executes_chunking() {
    let supported = TestRepository::new();
    let bundle = add_fixture_v2(&supported, "supported", "supported", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("reference_delivery = \"whole\"", "reference_delivery = \"unused-whole\"")
            .replace(
                "[expectations]",
                "[[execution.deliveries]]\nname = \"unused-whole\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]",
            )
    });
    let fixture = load_single_native_fixture(&supported);
    assert_eq!(fixture.reference_delivery().as_str(), "unused-whole");
    let ValidatedExecutionPlan::Parity(policy) = fixture.execution_plan() else {
        panic!("fixture-v2 policy");
    };
    assert_eq!(
        policy.strategies()[0]
            .origins
            .iter()
            .filter(|origin| matches!(origin, DeliveryStrategyOrigin::Declared(_)))
            .count(),
        2
    );
    let FixtureExecutionOutcome::CompletedV2 {
        deliveries,
        reference_delivery: Some(reference_delivery),
    } = execute_fixture(&fixture)
    else {
        panic!("aliased reference baseline should execute successfully");
    };
    assert_eq!(reference_delivery.as_str(), "unused-whole");
    assert!(
        deliveries[0]
            .aliases()
            .iter()
            .any(|alias| alias == &reference_delivery)
    );

    let unsupported = TestRepository::new();
    let bundle = add_fixture_v2(&unsupported, "unsupported", "unsupported", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
        "[expectations]",
        "[[execution.deliveries]]\nname = \"unused-chunked\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [1]\n\n[expectations]",
    )
    });
    let fixture = load_single_native_fixture(&unsupported);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::CompletedV2 { .. }
    ));
}

#[test]
fn strategy_semantic_equality_uses_transport_coordinates_not_storage_shape() {
    use std::num::NonZeroUsize;
    let scalar_whole = ResolvedDeliveryStrategy {
        transport: DeliveryTransport::UnicodeScalars,
        coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
        input_extent: 1,
        boundaries: CanonicalBoundarySequence::Whole,
    };
    let scalar_fixed = ResolvedDeliveryStrategy {
        boundaries: CanonicalBoundarySequence::Fixed {
            units_per_chunk: NonZeroUsize::MIN,
        },
        ..scalar_whole.clone()
    };
    let scalar_explicit = ResolvedDeliveryStrategy {
        boundaries: CanonicalBoundarySequence::Explicit(Box::new([])),
        ..scalar_whole.clone()
    };
    assert!(scalar_whole.semantically_equals(&scalar_fixed));
    assert!(scalar_whole.semantically_equals(&scalar_explicit));

    // The text "éx" has scalar extent 2 and byte extent 3. Coordinate 1
    // therefore names different semantic strategies for the two transports,
    // even though both declarations store the decimal value 1.
    let scalar_boundary = ResolvedDeliveryStrategy {
        transport: DeliveryTransport::UnicodeScalars,
        coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
        input_extent: 2,
        boundaries: CanonicalBoundarySequence::Explicit(Box::new([1])),
    };
    let byte_boundary = ResolvedDeliveryStrategy {
        transport: DeliveryTransport::Bytes,
        coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
        input_extent: 3,
        boundaries: CanonicalBoundarySequence::Explicit(Box::new([1])),
    };
    assert!(!scalar_boundary.semantically_equals(&byte_boundary));
}

#[test]
fn empty_and_one_scalar_v2_strategies_collapse_to_semantic_aliases() {
    for (id, input) in [("empty", b"".as_slice()), ("one", b"x".as_slice())] {
        let repository = TestRepository::new();
        let bundle = add_fixture_v2(&repository, id, id, input);
        rewrite(&bundle.join("fixture.toml"), |text| {
            text.replace(
                "[expectations]",
                "[[execution.deliveries]]\nname = \"explicit-empty\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = []\n\n[expectations]",
            )
        });
        let fixture = load_single_native_fixture(&repository);
        let ValidatedExecutionPlan::Parity(policy) = fixture.execution_plan() else {
            panic!("fixture-v2 policy");
        };
        assert_eq!(
            policy.strategies().len(),
            2,
            "scalar baseline and byte baseline are the only semantic strategies for {id}"
        );
        assert!(policy.strategies()[0].origins.iter().any(
            |origin| matches!(origin, DeliveryStrategyOrigin::Declared(name) if name.as_str() == "explicit-empty")
        ));
    }

    let legacy = TestRepository::new();
    let bundle = add_fixture(&legacy, "legacy-empty", "legacy-empty", b"");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "strategy = \"whole\"",
            "strategy = \"boundaries\"\nboundaries = []",
        )
    });
    assert!(
        discover_and_load(&legacy.native()).is_err(),
        "fixture-v1 retains its non-empty declared-boundary compatibility rule"
    );
}

#[test]
fn fixture_v2_final_invariants_are_supported_and_snapshot_order_remains_canonical() {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "precedence", "precedence", b"hello");
    fs::write(bundle.join("tokens.txt"), "malformed").expect("malformed tokens");
    fs::write(bundle.join("final-invariants.txt"), "malformed")
        .expect("final invariant placeholder");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\nfinal_invariants = \"final-invariants.txt\"",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
            ..
        }
    ));
}

#[test]
fn malformed_fixture_v2_sidecar_is_snapshot_format_for_its_exact_surface() {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "malformed", "malformed", b"hello");
    fs::write(
        bundle.join("tokens.txt"),
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=unknown\n",
    )
    .expect("malformed tokens");
    let fixture = load_single_native_fixture(&repository);
    assert!(matches!(
        execute_fixture(&fixture),
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
            ..
        }
    ));
}

#[test]
fn fixture_v2_executes_each_unique_strategy_once_with_the_surface_union() {
    use html::conformance::{
        ObservationRequest, ParserObservationExecutionError, ParserObservationRequest,
    };
    struct CountingExecutor {
        requests: Vec<(bool, bool, bool)>,
        shapes: Vec<String>,
    }
    impl super::runner::ParserObservationExecutor for CountingExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            let shape = match request.input {
                html::conformance::ParserObservationInput::Utf8(_) => "utf8-whole".to_string(),
                html::conformance::ParserObservationInput::Utf8FixedScalarChunks {
                    scalars_per_chunk,
                    ..
                } => format!("utf8-fixed:{scalars_per_chunk}"),
                html::conformance::ParserObservationInput::Utf8BoundaryChunks {
                    byte_offsets,
                    ..
                } => format!("utf8-explicit:{byte_offsets:?}"),
                html::conformance::ParserObservationInput::Bytes(_) => "bytes-whole".to_string(),
                html::conformance::ParserObservationInput::ByteFixedChunks {
                    bytes_per_chunk,
                    ..
                } => format!("bytes-fixed:{bytes_per_chunk}"),
                html::conformance::ParserObservationInput::ByteBoundaryChunks {
                    byte_offsets,
                    ..
                } => format!("bytes-explicit:{byte_offsets:?}"),
                html::conformance::ParserObservationInput::Utf8Chunks(_)
                | html::conformance::ParserObservationInput::ByteChunks(_) => {
                    panic!("fixture-v2 does not use pre-materialized chunk arrays")
                }
            };
            self.shapes.push(shape);
            self.requests.push((
                matches!(request.tokens, ObservationRequest::Capture { .. }),
                matches!(request.transitions, ObservationRequest::Capture { .. }),
                matches!(
                    request.unsupported_features,
                    ObservationRequest::Capture { .. }
                ),
            ));
            html::conformance::execute_parser_observation(request)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "union", "union", b"hello");
    fs::write(
        bundle.join("transitions.txt"),
        "# format: html5-tree-transitions-v1\n",
    )
    .unwrap();
    fs::write(
        bundle.join("unsupported-features.txt"),
        "# format: html5-unsupported-features-v1\n",
    )
    .unwrap();
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("kind = \"standalone-tokenizer\"", "kind = \"document\"\nscripting = \"disabled\"")
            .replace("[expectations]", "[[execution.deliveries]]\nname = \"trace-whole\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[[execution.deliveries]]\nname = \"declared-three\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [3]\n\n[expectations]")
            .replace("tokens = \"tokens.txt\"", "tokens = \"tokens.txt\"\nunsupported_features = \"unsupported-features.txt\"\n\n[[expectations.transitions]]\ndelivery = \"trace-whole\"\npath = \"transitions.txt\"")
    });
    let fixture = load_single_native_fixture(&repository);
    let mut executor = CountingExecutor {
        requests: Vec::new(),
        shapes: Vec::new(),
    };
    let (_, serialization_calls) = with_serialization_counter(|| {
        super::runner::execute_fixture_v2_with(&fixture, &mut executor)
    });
    assert_eq!(executor.requests.len(), 7);
    assert!(
        executor
            .requests
            .iter()
            .all(|request| *request == (true, true, true))
    );
    assert_eq!(
        executor.shapes.iter().collect::<BTreeSet<_>>().len(),
        executor.shapes.len(),
        "each semantically unique generated strategy executes once"
    );
    assert_eq!(
        executor.shapes,
        [
            "utf8-whole",
            "utf8-explicit:[3]",
            "bytes-whole",
            "utf8-fixed:1",
            "utf8-explicit:[1, 2, 4]",
            "bytes-fixed:1",
            "bytes-explicit:[1, 2, 4]",
        ]
    );
    assert_eq!(
        serialization_calls, 2,
        "only the applicable baseline token and transition expectations are serialized"
    );
}

#[test]
fn parity_mismatch_names_fixture_strategy_surface_and_first_record() {
    use html::conformance::{
        ObservedToken, ParserObservationExecutionError, ParserObservationRequest,
    };
    struct DivergingCandidate {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for DivergingCandidate {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            let mut result = html::conformance::execute_parser_observation(request)?;
            if self.calls == 2
                && let ObservationState::Captured(tokens) = &mut result.tokens
                && let Some(ObservedToken::Character { data }) = tokens.first_mut()
            {
                data.push_str("-candidate-only");
            }
            Ok(result)
        }
    }
    let repository = TestRepository::new();
    add_fixture_v2(
        &repository,
        "parity-diagnostic",
        "parity-diagnostic",
        b"hello",
    );
    let fixture = load_single_native_fixture(&repository);
    let (outcome, serialization_calls) = with_serialization_counter(|| {
        super::runner::execute_fixture_v2_with(&fixture, &mut DivergingCandidate { calls: 0 })
    });
    assert_eq!(
        serialization_calls, 3,
        "baseline expectation plus the selected baseline/candidate token diagnostics"
    );
    assert!(matches!(
        super::runner::failure_details(&fixture, &outcome),
        Some(FixtureFailureDetails::ParityDiff {
            surface: ExpectationSurface::Tokens,
            ..
        })
    ));
    let FixtureExecutionOutcome::ParityMismatchV2 {
        strategy,
        surface: ExpectationSurface::Tokens,
        diff,
        ..
    } = &outcome
    else {
        panic!("expected token parity mismatch: {outcome:?}");
    };
    assert!(strategy.contains("ordinal=2"), "{strategy}");
    for expected in [
        "fixture: parity-diagnostic",
        "fixture path: fixtures/parity-diagnostic",
        "delivery strategy: ordinal=2",
        "canonical surface: tokens",
        "first meaningful difference: record 1",
    ] {
        assert!(diff.contains(expected), "missing {expected}: {diff}");
    }
}

#[test]
fn successful_parity_candidate_does_not_serialize_candidate_surfaces() {
    struct CountingProductionExecutor {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for CountingProductionExecutor {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            html::conformance::execute_parser_observation(request)
        }
    }
    let repository = TestRepository::new();
    add_fixture_v2(&repository, "parity-memory", "parity-memory", b"hello");
    let fixture = load_single_native_fixture(&repository);
    let (outcome, serialization_calls) = with_serialization_counter(|| {
        super::runner::execute_fixture_v2_with(
            &fixture,
            &mut CountingProductionExecutor { calls: 0 },
        )
    });
    assert!(
        matches!(outcome, FixtureExecutionOutcome::CompletedV2 { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        serialization_calls, 1,
        "only the applicable baseline token expectation is serialized"
    );
}

#[test]
fn tree_parity_mismatch_serializes_only_the_selected_tree_surface() {
    struct DivergingTree {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for DivergingTree {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            let mut result = html::conformance::execute_parser_observation(request)?;
            if self.calls == 2
                && let ObservationState::Captured(tree) = &mut result.tree
            {
                tree.roots.clear();
            }
            Ok(result)
        }
    }

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "tree-parity", "tree-parity", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"document\"\nscripting = \"disabled\"",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    let (outcome, serialization_calls) = with_serialization_counter(|| {
        super::runner::execute_fixture_v2_with(&fixture, &mut DivergingTree { calls: 0 })
    });
    assert!(matches!(
        outcome,
        FixtureExecutionOutcome::ParityMismatchV2 {
            surface: ExpectationSurface::Tree,
            ..
        }
    ));
    assert_eq!(
        serialization_calls, 3,
        "baseline token expectation plus the selected baseline and candidate trees"
    );
}

#[test]
fn invalid_baseline_stops_before_candidates_and_sidecar_reads() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};
    struct FailingBaseline {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for FailingBaseline {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            Err(ParserObservationExecutionError::ParserInvariant)
        }
    }
    let repository = TestRepository::new();
    add_fixture_v2(&repository, "baseline-stop", "baseline-stop", b"hello");
    let fixture = load_single_native_fixture(&repository);
    let mut executor = FailingBaseline { calls: 0 };
    let outcome = super::runner::execute_fixture_v2_with(&fixture, &mut executor);
    assert!(matches!(
        outcome,
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::ParserObservation(
                html::conformance::ParserObservationExecutionIdentity::ParserInvariant
            ),
            ..
        }
    ));
    assert_eq!(executor.calls, 1);
}

#[test]
fn candidate_final_invariant_failure_is_not_parity_compared() {
    use html::conformance::{
        InvariantOutcome, ParserObservationExecutionError, ParserObservationRequest,
    };
    struct FailedCandidateAudit {
        calls: usize,
    }
    impl super::runner::ParserObservationExecutor for FailedCandidateAudit {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            self.calls += 1;
            let mut result = html::conformance::execute_parser_observation(request)?;
            if self.calls == 2
                && let ObservationState::Captured(report) = &mut result.final_invariants
            {
                report.tree_builder.pending_table_text_empty = InvariantOutcome::Failed;
            }
            Ok(result)
        }
    }
    let repository = TestRepository::new();
    add_fixture_v2(&repository, "candidate-audit", "candidate-audit", b"hello");
    let fixture = load_single_native_fixture(&repository);
    let outcome =
        super::runner::execute_fixture_v2_with(&fixture, &mut FailedCandidateAudit { calls: 0 });
    assert!(matches!(
        outcome,
        FixtureExecutionOutcome::FinalInvariantFailedV2 {
            first_failure: InvariantFailureCode::PendingTableText,
            failure_count: 1,
            ..
        }
    ));
}

#[test]
fn parser_failure_and_incomplete_state_precede_snapshot_mismatch() {
    use html::conformance::{ParserObservationExecutionError, ParserObservationRequest};
    struct Failing;
    impl super::runner::ParserObservationExecutor for Failing {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            Err(ParserObservationExecutionError::ParserInvariant)
        }
    }
    let repository = TestRepository::new();
    add_fixture_v2(&repository, "precedence", "precedence", b"different");
    let fixture = load_single_native_fixture(&repository);
    assert!(matches!(
        super::runner::execute_fixture_v2_with(&fixture, &mut Failing),
        FixtureExecutionOutcome::ExecutionFailedV2 {
            class: ExecutionFailureClass::ParserObservation(
                html::conformance::ParserObservationExecutionIdentity::ParserInvariant
            ),
            ..
        }
    ));

    struct Incomplete;
    impl super::runner::ParserObservationExecutor for Incomplete {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            let mut result = canonical_result();
            result.tokens = ObservationState::Incomplete {
                partial: Vec::new(),
                reason: IncompleteObservationReason::StorageLimitExceeded {
                    retained: 0,
                    dropped: 1,
                },
            };
            Ok(result)
        }
    }
    assert!(matches!(
        super::runner::execute_fixture_v2_with(&fixture, &mut Incomplete),
        FixtureExecutionOutcome::IncompleteObservationV2 { .. }
    ));
}

#[test]
fn multiple_fixture_v2_mismatches_select_the_fixed_first_surface() {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "mismatch-order", "mismatch-order", b"hello");
    fs::write(bundle.join("tokens.txt"), "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"wrong\"\nTOKEN ordinal=2 kind=eof\n").unwrap();
    fs::write(
        bundle.join("document-mode.txt"),
        "# format: html5-document-mode-v1\nMODE value=quirks\n",
    )
    .unwrap();
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"document\"\nscripting = \"disabled\"",
        )
        .replace(
            "tokens = \"tokens.txt\"",
            "tokens = \"tokens.txt\"\ndocument_mode = \"document-mode.txt\"",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    match execute_fixture(&fixture) {
        FixtureExecutionOutcome::ExpectationMismatchV2 {
            surface: ExpectationSurface::Tokens,
            diff,
            ..
        } => {
            for required in [
                "fixture: mismatch-order",
                "fixture path: fixtures/mismatch-order",
                "expectation surface: tokens",
                "expected snapshot: fixtures/mismatch-order/tokens.txt",
                "snapshot format: html5-token-v2",
                "first meaningful difference: record 1 (token 1)",
                "expected:",
                "actual:",
                "nearby context:",
                "expected record count:",
                "actual record count:",
            ] {
                assert!(diff.contains(required), "missing '{required}' in:\n{diff}");
            }
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    let error = run_fixture(&fixture).expect_err("a mismatch cannot expose a completed report");
    assert!(error.to_string().contains("tokens expectation mismatch"));
}

#[test]
fn transition_mismatch_selection_follows_fixture_declaration_order() {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "transition-order", "transition-order", b"<p>x");
    fs::remove_file(bundle.join("tokens.txt")).expect("remove unused token sidecar");
    fs::write(
        bundle.join("transitions.first.txt"),
        "# format: html5-tree-transitions-v1\n",
    )
    .expect("first transitions");
    fs::write(
        bundle.join("transitions.second.txt"),
        "# format: html5-tree-transitions-v1\n",
    )
    .expect("second transitions");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "kind = \"standalone-tokenizer\"",
            "kind = \"document\"\nscripting = \"disabled\"",
        )
        .replace(
            "[expectations]\ntokens = \"tokens.txt\"",
            "[[execution.deliveries]]\nname = \"first-trace\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[[execution.deliveries]]\nname = \"second-trace\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]\n\n[[expectations.transitions]]\ndelivery = \"first-trace\"\npath = \"transitions.first.txt\"\n\n[[expectations.transitions]]\ndelivery = \"second-trace\"\npath = \"transitions.second.txt\"",
        )
    });
    let fixture = load_single_native_fixture(&repository);
    match execute_fixture(&fixture) {
        FixtureExecutionOutcome::ExpectationMismatchV2 {
            ref strategy,
            surface: ExpectationSurface::Transitions,
            diff,
            ..
        } => {
            assert!(strategy.contains("declared:first-trace"), "{strategy}");
            assert!(diff.contains("transition delivery: first-trace"), "{diff}");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn canonical_fixture_guardrails_are_fixed_and_expectation_independent() {
    use html::conformance::{
        ObservationRequest, ParserObservationTarget, ScalarObservationRequest,
    };
    let guardrails = super::execution::FixtureObservationGuardrails::PRODUCTION;
    let request = super::execution::observation_request(
        ParserObservationTarget::DocumentParser,
        "input",
        super::execution::RequestedSurfaces {
            tokens: true,
            parse_errors: true,
            implementation_diagnostics: true,
            document_mode: true,
            tree: true,
            patches: true,
            transitions: true,
            unsupported_features: true,
            final_invariants: true,
        },
        guardrails,
    );
    assert_eq!(
        request.tokens,
        ObservationRequest::Capture {
            capacity: guardrails.tokens
        }
    );
    assert_eq!(
        request.parse_errors,
        ObservationRequest::Capture {
            capacity: guardrails.parse_errors
        }
    );
    assert_eq!(
        request.implementation_diagnostics,
        ObservationRequest::Capture {
            capacity: guardrails.implementation_diagnostics
        }
    );
    assert_eq!(request.document_mode, ScalarObservationRequest::Capture);
    assert_eq!(
        request.tree,
        ObservationRequest::Capture {
            capacity: guardrails.canonical_tree_units
        }
    );
    assert_eq!(
        request.patches,
        ObservationRequest::Capture {
            capacity: guardrails.patch_operations
        }
    );
    assert_eq!(
        request.transitions,
        ObservationRequest::Capture {
            capacity: guardrails.transitions
        }
    );
    assert_eq!(
        request.unsupported_features,
        ObservationRequest::Capture {
            capacity: guardrails.unsupported_features
        }
    );
}

#[test]
fn expected_sidecar_record_count_cannot_change_injected_guardrails() {
    use super::execution::FixtureObservationGuardrails;
    use html::conformance::{ObservationRequest, ObservedToken, ParserObservationRequest};

    struct CaptureTokenCapacity {
        observed: Vec<ObservationRequest>,
    }

    impl super::runner::ParserObservationExecutor for CaptureTokenCapacity {
        fn execute(
            &mut self,
            request: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, html::conformance::ParserObservationExecutionError>
        {
            self.observed.push(request.tokens);
            let mut result = canonical_result();
            result.tokens = ObservationState::Captured(vec![ObservedToken::Eof]);
            Ok(result)
        }
    }

    fn run_with_snapshot(snapshot: &str) -> Vec<ObservationRequest> {
        let repository = TestRepository::new();
        let bundle = add_fixture_v2(&repository, "guardrail", "guardrail", b"hello");
        fs::write(bundle.join("tokens.txt"), snapshot).expect("replacement snapshot");
        let fixture = load_single_native_fixture(&repository);
        let mut executor = CaptureTokenCapacity {
            observed: Vec::new(),
        };
        let _ = super::runner::execute_fixture_v2_with_guardrails(
            &fixture,
            &mut executor,
            FixtureObservationGuardrails {
                tokens: 7,
                ..FixtureObservationGuardrails::PRODUCTION
            },
        );
        executor.observed
    }

    let one_record = run_with_snapshot("# format: html5-token-v2\nTOKEN ordinal=1 kind=eof\n");
    let two_records = run_with_snapshot(
        "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"hello\"\nTOKEN ordinal=2 kind=eof\n",
    );
    assert_eq!(
        one_record,
        vec![ObservationRequest::Capture { capacity: 7 }]
    );
    assert_eq!(two_records, one_record);
}

#[test]
fn every_incomplete_requested_surface_is_rejected_before_comparison() {
    use super::execution::RequestedSurfaces;
    use super::runner::{StateIssue, first_state_issue};
    use html::DocumentMode;
    use html::conformance::{ObservedPatchStream, ObservedTree};

    fn reason() -> IncompleteObservationReason {
        IncompleteObservationReason::StorageLimitExceeded {
            retained: 1,
            dropped: 1,
        }
    }

    macro_rules! assert_incomplete {
        ($surface:expr, $field:ident, $partial:expr) => {{
            let mut result = canonical_result();
            result.$field = ObservationState::Incomplete {
                partial: $partial,
                reason: reason(),
            };
            let requested = RequestedSurfaces {
                $field: true,
                ..RequestedSurfaces::default()
            };
            assert!(matches!(
                first_state_issue(&result, requested),
                Some(StateIssue::Incomplete {
                    surface,
                    reason: IncompleteObservationReason::StorageLimitExceeded {
                        retained: 1,
                        dropped: 1
                    },
                    retained: 1,
                    dropped: 1,
                }) if surface == $surface
            ));
            let unrequested = RequestedSurfaces::default();
            assert_eq!(
                first_state_issue(&result, unrequested),
                Some(StateIssue::Invariant(
                    ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyIncomplete
                ))
            );
        }};
    }

    assert_incomplete!(ExpectationSurface::Tokens, tokens, Vec::new());
    assert_incomplete!(ExpectationSurface::ParseErrors, parse_errors, Vec::new());
    assert_incomplete!(
        ExpectationSurface::ImplementationDiagnostics,
        implementation_diagnostics,
        Vec::new()
    );
    assert_incomplete!(
        ExpectationSurface::DocumentMode,
        document_mode,
        DocumentMode::NoQuirks
    );
    assert_incomplete!(ExpectationSurface::Tree, tree, ObservedTree::default());
    assert_incomplete!(
        ExpectationSurface::Patches,
        patches,
        ObservedPatchStream::default()
    );
    assert_incomplete!(ExpectationSurface::Transitions, transitions, Vec::new());
    assert_incomplete!(
        ExpectationSurface::UnsupportedFeatures,
        unsupported_features,
        Vec::new()
    );
}

#[test]
fn incomplete_diagnostics_retain_exact_identity_for_every_surface() {
    let repository = TestRepository::new();
    add_fixture_v2(&repository, "incomplete", "incomplete", b"hello");
    let fixture = load_single_native_fixture(&repository);
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
        let outcome = FixtureExecutionOutcome::IncompleteObservationV2 {
            strategy: "ordinal=1 origins=[baseline,declared:whole]".to_string(),
            surface,
            reason: IncompleteObservationReason::StorageLimitExceeded {
                retained: 11,
                dropped: 3,
            },
            retained: 11,
            dropped: 3,
        };
        let Some(FixtureFailureDetails::Message(message)) =
            super::runner::failure_details(&fixture, &outcome)
        else {
            panic!("missing incomplete details for {}", surface.name());
        };
        for expected in [
            "strategy:".to_string(),
            format!("surface: {}", surface.name()),
            "reason: storage-limit-exceeded".to_string(),
            "retained count: 11".to_string(),
            "dropped count: 3".to_string(),
        ] {
            assert!(message.contains(&expected), "missing {expected}: {message}");
        }
    }
}

#[test]
fn every_surface_enforces_requested_and_unrequested_state_contracts() {
    use super::execution::RequestedSurfaces;
    use super::runner::{StateIssue, first_state_issue};
    use html::DocumentMode;
    use html::conformance::{NotApplicableReason, ObservedPatchStream, ObservedTree};

    macro_rules! assert_states {
        ($field:ident, $captured:expr) => {{
            let requested = RequestedSurfaces {
                $field: true,
                ..RequestedSurfaces::default()
            };

            let mut captured = canonical_result();
            captured.$field = ObservationState::Captured($captured);
            assert_eq!(first_state_issue(&captured, requested), None);
            assert_eq!(
                first_state_issue(&captured, RequestedSurfaces::default()),
                Some(StateIssue::Invariant(
                    ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyCaptured
                ))
            );

            let not_requested = canonical_result();
            assert_eq!(
                first_state_issue(&not_requested, requested),
                Some(StateIssue::Invariant(
                    ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotRequested
                ))
            );

            let mut not_applicable = canonical_result();
            not_applicable.$field = ObservationState::NotApplicable {
                reason: NotApplicableReason::DocumentParserRun,
            };
            assert_eq!(
                first_state_issue(&not_applicable, requested),
                Some(StateIssue::Invariant(
                    ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotApplicable
                ))
            );
        }};
    }

    assert_states!(tokens, Vec::new());
    assert_states!(parse_errors, Vec::new());
    assert_states!(implementation_diagnostics, Vec::new());
    assert_states!(document_mode, DocumentMode::NoQuirks);
    assert_states!(tree, ObservedTree::default());
    assert_states!(patches, ObservedPatchStream::default());
    assert_states!(transitions, Vec::new());
    assert_states!(unsupported_features, Vec::new());
}

#[test]
fn injected_guardrails_cover_exact_and_capacity_plus_one_for_every_retained_surface() {
    use super::execution::{FixtureObservationGuardrails, RequestedSurfaces, observation_request};
    use html::conformance::{
        ObservationRequest, ParserObservationTarget, execute_parser_observation,
    };

    const INPUT: &str = "<!doctype html><body class=a><body class=b><div a='first' a='second'/>\n";
    let surfaces = RequestedSurfaces {
        tokens: true,
        parse_errors: true,
        implementation_diagnostics: true,
        document_mode: true,
        tree: true,
        patches: true,
        transitions: true,
        unsupported_features: true,
        final_invariants: true,
    };
    let high = FixtureObservationGuardrails {
        tokens: 1_024,
        parse_errors: 1_024,
        implementation_diagnostics: 1_024,
        unsupported_features: 1_024,
        canonical_tree_units: 1_024,
        transitions: 1_024,
        patch_operations: 1_024,
    };
    let baseline = execute_parser_observation(observation_request(
        ParserObservationTarget::DocumentParser,
        INPUT,
        surfaces,
        high,
    ))
    .expect("baseline observation");
    macro_rules! captured_len {
        ($state:expr) => {
            match $state {
                ObservationState::Captured(values) => values.len(),
                _ => panic!("baseline collection was not captured"),
            }
        };
    }
    let token_count = captured_len!(&baseline.tokens);
    let parse_error_count = captured_len!(&baseline.parse_errors);
    let diagnostic_count = captured_len!(&baseline.implementation_diagnostics);
    let transition_count = captured_len!(&baseline.transitions);
    let unsupported_count = captured_len!(&baseline.unsupported_features);
    let patch_count = match &baseline.patches {
        ObservationState::Captured(stream) => stream.operations.len(),
        _ => panic!("baseline patches were not captured"),
    };
    let mut zero_tree = high;
    zero_tree.canonical_tree_units = 0;
    let tree_probe = execute_parser_observation(observation_request(
        ParserObservationTarget::DocumentParser,
        INPUT,
        surfaces,
        zero_tree,
    ))
    .expect("tree capacity probe");
    let tree_count = match tree_probe.tree {
        ObservationState::Incomplete {
            reason: IncompleteObservationReason::StorageLimitExceeded { dropped, .. },
            ..
        } => usize::try_from(dropped).expect("fixture-sized tree unit count"),
        _ => panic!("zero tree capacity must be incomplete"),
    };

    macro_rules! assert_boundary {
        ($policy_field:ident, $result_field:ident, $required:expr) => {{
            let required = $required;
            assert!(required > 0);
            let mut exact = high;
            exact.$policy_field = required;
            let exact_request = observation_request(
                ParserObservationTarget::DocumentParser,
                INPUT,
                surfaces,
                exact,
            );
            assert_eq!(
                exact_request.$result_field,
                ObservationRequest::Capture { capacity: required }
            );
            let exact_result = execute_parser_observation(exact_request).expect("exact capacity");
            assert!(matches!(
                exact_result.$result_field,
                ObservationState::Captured(_)
            ));

            let capacity = required.checked_sub(1).expect("positive requirement");
            let mut below = high;
            below.$policy_field = capacity;
            let below_request = observation_request(
                ParserObservationTarget::DocumentParser,
                INPUT,
                surfaces,
                below,
            );
            assert_eq!(
                below_request.$result_field,
                ObservationRequest::Capture { capacity }
            );
            let below_result =
                execute_parser_observation(below_request).expect("capacity plus one observation");
            assert!(matches!(
                below_result.$result_field,
                ObservationState::Incomplete { .. }
            ));
        }};
    }

    assert_boundary!(tokens, tokens, token_count);
    assert_boundary!(parse_errors, parse_errors, parse_error_count);
    assert_boundary!(
        implementation_diagnostics,
        implementation_diagnostics,
        diagnostic_count
    );
    assert_boundary!(canonical_tree_units, tree, tree_count);
    assert_boundary!(patch_operations, patches, patch_count);
    assert_boundary!(transitions, transitions, transition_count);
    assert_boundary!(
        unsupported_features,
        unsupported_features,
        unsupported_count
    );
}

#[test]
fn incomplete_prefix_equal_to_expected_snapshot_cannot_reach_serialization_or_comparison() {
    use super::execution::FixtureObservationGuardrails;
    use html::conformance::{
        ObservedToken, ParserObservationExecutionError, ParserObservationRequest,
    };

    struct IncompleteMatchingPrefix;
    impl super::runner::ParserObservationExecutor for IncompleteMatchingPrefix {
        fn execute(
            &mut self,
            _: ParserObservationRequest<'_>,
        ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
            let mut result = canonical_result();
            result.tokens = ObservationState::Incomplete {
                partial: vec![
                    ObservedToken::Character {
                        data: "hello".to_string(),
                    },
                    ObservedToken::Eof,
                ],
                reason: IncompleteObservationReason::StorageLimitExceeded {
                    retained: 2,
                    dropped: 1,
                },
            };
            Ok(result)
        }
    }

    let repository = TestRepository::new();
    add_fixture_v2(
        &repository,
        "incomplete-prefix",
        "incomplete-prefix",
        b"hello",
    );
    let fixture = load_single_native_fixture(&repository);
    let outcome = super::runner::execute_fixture_v2_with_guardrails(
        &fixture,
        &mut IncompleteMatchingPrefix,
        FixtureObservationGuardrails {
            tokens: 2,
            ..FixtureObservationGuardrails::PRODUCTION
        },
    );
    assert!(matches!(
        outcome,
        FixtureExecutionOutcome::IncompleteObservationV2 {
            ref strategy,
            surface: ExpectationSurface::Tokens,
            reason: IncompleteObservationReason::StorageLimitExceeded {
                retained: 2,
                dropped: 1
            },
            retained: 2,
            dropped: 1,
        } if strategy.contains("declared:whole")
    ));

    let error = super::runner::run_fixture_with_executor(&fixture, &mut IncompleteMatchingPrefix)
        .expect_err("incomplete capture cannot produce a completed report");
    let diagnostic = error.to_string();
    for expected in [
        "strategy:",
        "surface: tokens",
        "reason: storage-limit-exceeded",
        "retained count: 2",
        "dropped count: 1",
    ] {
        assert!(
            diagnostic.contains(expected),
            "missing {expected}: {diagnostic}"
        );
    }
}

#[test]
fn successful_v2_report_borrows_reference_result_from_its_single_delivery_owner() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = manifest
        .join("../..")
        .canonicalize()
        .expect("repository root");
    let fixture_root = manifest
        .join("../html/tests/fixtures/html5/conformance")
        .canonicalize()
        .expect("fixture root");
    let fixtures = discover_and_load(&FixtureRepository::native(repository_root, fixture_root))
        .expect("canonical fixture-v2 corpus");
    let fixture = fixtures
        .iter()
        .find(|fixture| fixture.id().as_str() == "document-structured-observations")
        .expect("multi-delivery canonical fixture");
    let report = run_fixture(fixture).expect("multi-delivery fixture passes");
    assert_eq!(report.delivery_results().len(), 1);
    let reference = report.result().expect("ordinary reference result");
    let owned = report
        .delivery_results()
        .iter()
        .find(|delivery| delivery.delivery().as_str() == "whole")
        .expect("reference delivery")
        .result();
    assert!(std::ptr::eq(reference, owned));
    assert_eq!(
        report.delivery_results()[0]
            .aliases()
            .iter()
            .map(DeliveryName::as_str)
            .collect::<Vec<_>>(),
        ["whole", "trace-whole"]
    );
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
            actual: FixtureOutcomeClassification::ExecutionFailedV1(
                LegacyExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens)
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
        FixtureLoadErrorKind::InvalidDelivery(
            DeliveryValidationError::UnitNotSupportedForInputDomain { .. }
        )
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

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "reference-name", "reference-name", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "reference_delivery = \"whole\"",
            "reference_delivery = \"Bad Name\"",
        )
    });
    assert!(matches!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidDelivery(
            DeliveryValidationError::InvalidReferenceDeliveryName { ref declared_name }
        ) if declared_name == "Bad Name"
    ));
}

#[test]
fn delivery_name_validation_reports_exact_typed_context() {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "invalid-name", "invalid-name", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace("name = \"whole\"", "name = \"Bad Name\"")
    });
    assert_eq!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::InvalidDeliveryName {
            delivery_index: 0,
            declared_name: "Bad Name".to_string(),
        })
    );

    let repository = TestRepository::new();
    let bundle = add_fixture_v2(&repository, "duplicate-name", "duplicate-name", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"whole\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]",
        )
    });
    assert_eq!(
        discover_and_load(&repository.native()).unwrap_err().kind,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::DuplicateDeliveryName {
            delivery_index: 1,
            declared_name: "whole".to_string(),
        })
    );
}

fn load_v2_delivery_error(
    input: &[u8],
    raw_bytes: bool,
    transform: impl FnOnce(String) -> String,
) -> FixtureLoadErrorKind {
    let repository = TestRepository::new();
    let bundle = add_fixture_v2(
        &repository,
        "typed-delivery-error",
        "typed-delivery-error",
        input,
    );
    let mut metadata = fs::read_to_string(bundle.join("fixture.toml")).expect("fixture metadata");
    if raw_bytes {
        fs::rename(bundle.join("input.html"), bundle.join("input.bin")).expect("raw input rename");
        metadata = metadata
            .replace("path = \"input.html\"", "path = \"input.bin\"")
            .replace("kind = \"utf8-text\"", "kind = \"raw-bytes\"");
    }
    fs::write(bundle.join("fixture.toml"), transform(metadata)).expect("fixture metadata rewrite");
    discover_and_load(&repository.native())
        .expect_err("fixture must produce the requested typed validation error")
        .kind
}

#[test]
fn fixture_validator_emits_every_delivery_validation_error_with_exact_context() {
    let error = load_v2_delivery_error(b"hello", false, |text| {
        let mut extra = String::new();
        for index in 0..32 {
            extra.push_str(&format!(
                "[[execution.deliveries]]\nname = \"split-{index}\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{}]\n\n",
                index + 1
            ));
        }
        text.replace("[expectations]", &format!("{extra}[expectations]"))
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyDeclaredDeliveries {
            declared: 33,
            maximum: 32,
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        let boundaries = (1..=4_097)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        text.replace(
            "[expectations]",
            &format!(
                "[[execution.deliveries]]\nname = \"oversized\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{boundaries}]\n\n[expectations]"
            ),
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyBoundaries {
            delivery_index: 1,
            declared_name: "oversized".to_owned(),
            declared: 4_097,
            maximum: 4_096,
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace("name = \"whole\"", "name = \"Bad Name\"")
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::InvalidDeliveryName {
            delivery_index: 0,
            declared_name: "Bad Name".to_owned(),
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"whole\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::DuplicateDeliveryName {
            delivery_index: 1,
            declared_name: "whole".to_owned(),
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "reference_delivery = \"whole\"",
            "reference_delivery = \"Bad Ref\"",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(
            DeliveryValidationError::InvalidReferenceDeliveryName {
                declared_name: "Bad Ref".to_owned(),
            }
        )
    );

    let error = load_v2_delivery_error(b"hello", true, |text| text);
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(
            DeliveryValidationError::UnitNotSupportedForInputDomain {
                delivery: DeliveryName::validated("whole".to_owned()),
            }
        )
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\n\n[expectations]",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::BoundariesMissing {
            delivery: DeliveryName::validated("split".to_owned()),
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\nboundaries = [1]\n\n[expectations]",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::BoundariesUnexpected {
            delivery: DeliveryName::validated("split".to_owned()),
        })
    );

    for (boundaries, expected) in [
        (
            "[0]",
            DeliveryValidationError::BoundaryAtStart {
                delivery: DeliveryName::validated("split".to_owned()),
                boundary_index: 0,
            },
        ),
        (
            "[5]",
            DeliveryValidationError::BoundaryAtEnd {
                delivery: DeliveryName::validated("split".to_owned()),
                boundary_index: 0,
            },
        ),
        (
            "[6]",
            DeliveryValidationError::BoundaryOutOfRange {
                delivery: DeliveryName::validated("split".to_owned()),
                boundary_index: 0,
            },
        ),
        (
            "[1, 1]",
            DeliveryValidationError::DuplicateBoundary {
                delivery: DeliveryName::validated("split".to_owned()),
                boundary_index: 1,
            },
        ),
        (
            "[2, 1]",
            DeliveryValidationError::UnsortedBoundary {
                delivery: DeliveryName::validated("split".to_owned()),
                boundary_index: 1,
            },
        ),
    ] {
        let error = load_v2_delivery_error(b"hello", false, |text| {
            text.replace(
                "[expectations]",
                &format!(
                    "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = {boundaries}\n\n[expectations]"
                ),
            )
        });
        assert_eq!(
            error,
            FixtureLoadErrorKind::InvalidDelivery(expected),
            "boundary declaration {boundaries}"
        );
    }

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "unit = \"unicode-scalars\"\nstrategy = \"whole\"",
            "unit = \"bytes\"\nstrategy = \"whole\"",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::MissingDomainBaseline)
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "reference_delivery = \"whole\"",
            "reference_delivery = \"missing\"",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::ReferenceDeliveryMissing {
            delivery: DeliveryName::validated("missing".to_owned()),
        })
    );

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "reference_delivery = \"whole\"",
            "reference_delivery = \"bytes\"",
        )
        .replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"bytes\"\nunit = \"bytes\"\nstrategy = \"whole\"\n\n[expectations]",
        )
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(
            DeliveryValidationError::ReferenceIsNotDomainBaseline {
                delivery: DeliveryName::validated("bytes".to_owned()),
            }
        )
    );

    let error = load_v2_delivery_error(&[b'x'; 100], false, |text| {
        let mut unique = String::new();
        for index in 1..=23 {
            unique.push_str(&format!(
                "[[execution.deliveries]]\nname = \"unique-{index}\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{index}]\n\n"
            ));
        }
        text.replace("[expectations]", &format!("{unique}[expectations]"))
    });
    assert_eq!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyUniqueStrategies {
            planned: 25,
            maximum: 24,
        })
    );
}

#[test]
fn fixture_delivery_validation_precedence_is_exact_for_combined_defects() {
    let error = load_v2_delivery_error(b"hello", false, |text| {
        let mut extra = String::new();
        for index in 0..32 {
            extra.push_str(&format!(
                "[[execution.deliveries]]\nname = \"Bad Name {index}\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n"
            ));
        }
        text.replace("[expectations]", &format!("{extra}[expectations]"))
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyDeclaredDeliveries {
            declared: 33,
            maximum: 32,
        })
    ));

    let error = load_v2_delivery_error(b"hello", false, |text| {
        let boundaries = (1..=4_097)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",");
        text.replace(
            "[expectations]",
            &format!(
                "[[execution.deliveries]]\nname = \"Bad Name\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{boundaries}]\n\n[expectations]"
            ),
        )
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyBoundaries {
            delivery_index: 1,
            declared_name,
            declared: 4_097,
            maximum: 4_096,
        }) if declared_name == "Bad Name"
    ));

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "[expectations]",
            "[[execution.deliveries]]\nname = \"Bad Name\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [0]\n\n[expectations]",
        )
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::InvalidDeliveryName {
            delivery_index: 1,
            declared_name,
        }) if declared_name == "Bad Name"
    ));

    let error = load_v2_delivery_error(b"hello", true, |text| {
        text.replace(
            "unit = \"unicode-scalars\"\nstrategy = \"whole\"",
            "unit = \"bytes\"\nstrategy = \"boundaries\"\nboundaries = [0]",
        )
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::BoundaryAtStart {
            delivery,
            boundary_index: 0,
        }) if delivery.as_str() == "whole"
    ));

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace(
            "unit = \"unicode-scalars\"\nstrategy = \"whole\"",
            "unit = \"bytes\"\nstrategy = \"whole\"",
        )
        .replace(
            "reference_delivery = \"whole\"",
            "reference_delivery = \"Bad Ref\"",
        )
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::MissingDomainBaseline)
    ));

    let error = load_v2_delivery_error(b"hello", false, |text| {
        text.replace("reference_delivery = \"whole\"", "reference_delivery = \"bytes\"")
            .replace(
                "[expectations]",
                "[[execution.deliveries]]\nname = \"bytes\"\nunit = \"bytes\"\nstrategy = \"whole\"\n\n[expectations]",
            )
    });
    assert!(matches!(
        error,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::ReferenceIsNotDomainBaseline {
            delivery,
        }) if delivery.as_str() == "bytes"
    ));
}

#[test]
fn fixture_v2_guardrails_reject_excess_before_execution_without_truncation() {
    let deliveries_repository = TestRepository::new();
    let deliveries_bundle = add_fixture_v2(
        &deliveries_repository,
        "too-many-deliveries",
        "too-many-deliveries",
        &[b'x'; 100],
    );
    let mut extra = String::new();
    for index in 0..32 {
        extra.push_str(&format!(
            "[[execution.deliveries]]\nname = \"split-{index}\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{}]\n\n",
            index + 1
        ));
    }
    rewrite(&deliveries_bundle.join("fixture.toml"), |text| {
        text.replace("[expectations]", &format!("{extra}[expectations]"))
    });
    let error = discover_and_load(&deliveries_repository.native())
        .expect_err("declared delivery guardrail");
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyDeclaredDeliveries {
            declared: 33,
            maximum: 32,
        })
    ));

    let boundaries_repository = TestRepository::new();
    let boundaries_bundle = add_fixture_v2(
        &boundaries_repository,
        "too-many-boundaries",
        "too-many-boundaries",
        &vec![b'x'; 5_000],
    );
    let boundaries = (1..=4_097)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    rewrite(&boundaries_bundle.join("fixture.toml"), |text| {
        text.replace(
            "[expectations]",
            &format!(
                "[[execution.deliveries]]\nname = \"oversized\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{boundaries}]\n\n[expectations]"
            ),
        )
    });
    let error = discover_and_load(&boundaries_repository.native()).expect_err("boundary guardrail");
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyBoundaries {
            delivery_index: 1,
            ref declared_name,
            declared: 4_097,
            maximum: 4_096,
        }) if declared_name == "oversized"
    ));

    let strategies_repository = TestRepository::new();
    let strategies_bundle = add_fixture_v2(
        &strategies_repository,
        "too-many-strategies",
        "too-many-strategies",
        &[b'x'; 100],
    );
    let mut unique = String::new();
    for index in 1..=23 {
        unique.push_str(&format!(
            "[[execution.deliveries]]\nname = \"unique-{index}\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{index}]\n\n"
        ));
    }
    rewrite(&strategies_bundle.join("fixture.toml"), |text| {
        text.replace("[expectations]", &format!("{unique}[expectations]"))
    });
    let error =
        discover_and_load(&strategies_repository.native()).expect_err("unique strategy guardrail");
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::TooManyUniqueStrategies {
            planned: 25,
            maximum: 24,
        })
    ));
}

#[test]
fn fixture_v2_delivery_validation_reports_exact_boundary_variants_and_order() {
    let cases = [
        (vec![0], "start"),
        (vec![5], "end"),
        (vec![6], "range"),
        (vec![1, 1], "duplicate"),
        (vec![2, 1], "unsorted"),
    ];
    for (boundaries, expected) in cases {
        let repository = TestRepository::new();
        let bundle = add_fixture_v2(&repository, "boundary", "boundary", b"hello");
        let encoded = boundaries
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        rewrite(&bundle.join("fixture.toml"), |text| {
            text.replace(
                "[expectations]",
                &format!(
                    "[[execution.deliveries]]\nname = \"split\"\nunit = \"unicode-scalars\"\nstrategy = \"boundaries\"\nboundaries = [{encoded}]\n\n[expectations]"
                ),
            )
        });
        let error = discover_and_load(&repository.native()).expect_err("invalid boundaries");
        let expected_name = DeliveryName::validated("split".to_string());
        match (expected, error.kind) {
            (
                "start",
                FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::BoundaryAtStart {
                    delivery,
                    boundary_index: 0,
                }),
            ) => assert_eq!(delivery, expected_name),
            (
                "end",
                FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::BoundaryAtEnd {
                    delivery,
                    boundary_index: 0,
                }),
            ) => assert_eq!(delivery, expected_name),
            (
                "range",
                FixtureLoadErrorKind::InvalidDelivery(
                    DeliveryValidationError::BoundaryOutOfRange {
                        delivery,
                        boundary_index: 0,
                    },
                ),
            ) => assert_eq!(delivery, expected_name),
            (
                "duplicate",
                FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::DuplicateBoundary {
                    delivery,
                    boundary_index: 1,
                }),
            ) => assert_eq!(delivery, expected_name),
            (
                "unsorted",
                FixtureLoadErrorKind::InvalidDelivery(DeliveryValidationError::UnsortedBoundary {
                    delivery,
                    boundary_index: 1,
                }),
            ) => assert_eq!(delivery, expected_name),
            _ => panic!("unexpected typed validation result"),
        }
    }
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
            actual: FixtureOutcomeClassification::ExecutionFailedV1(
                LegacyExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens)
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
fn completed_capabilities_cannot_be_hidden_by_non_active_dispositions() {
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
            text.replace("status = \"active\"", disposition)
        });
        assert!(matches!(
            discover_and_load(&repository.adapted()).unwrap_err().kind,
            FixtureLoadErrorKind::InvalidDisposition(_)
        ));
    }
}

#[test]
fn fixture_v1_rejects_broad_skip_escape_hatches() {
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
fn fixture_v1_rejects_legacy_free_form_external_provenance() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "legacy-external", "legacy-external", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"external\"\nprovenance = \"upstream/case\"",
        )
    });
    let error = discover_and_load(&repository.adapted()).unwrap_err();
    assert!(matches!(
        error.kind,
        FixtureLoadErrorKind::InvalidCombination(_)
    ));
    assert!(
        error
            .to_string()
            .contains("fixture-v3 structured provenance")
    );
}

#[test]
fn irrelevant_fragment_skip_is_rejected_before_execution() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "skipped", "skipped-fragment", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
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
            "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
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
        failure: ExpectedFailureClassification::Execution(
            LegacyExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tree),
        ),
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
            execution_failure(LegacyExecutionFailureClass::TokenizerDriver),
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
            execution_failure(LegacyExecutionFailureClass::SnapshotFormat(
                ExpectationSurface::Tree,
            )),
            ExpectedEvaluation::Pass,
        ),
        (
            "wrong execution failure",
            expected_execution_failure.clone(),
            execution_failure(LegacyExecutionFailureClass::SnapshotRead(
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
            FixtureExecutionOutcome::NotExecuted {
                classification: SkipClassification::UnsupportedCapability(
                    FixtureCapability::FragmentParsing,
                ),
            },
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

fn execution_failure(class: LegacyExecutionFailureClass) -> FixtureExecutionOutcome {
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
fn typed_parity_comparison_precedes_snapshot_serialization() {
    let baseline = canonical_result();
    let mut candidate = canonical_result();
    candidate.document_mode = ObservationState::NotApplicable {
        reason: html::conformance::NotApplicableReason::DocumentParserRun,
    };
    assert_eq!(
        first_typed_parity_mismatch(&baseline, &candidate).unwrap(),
        Some(ExpectationSurface::DocumentMode)
    );
    let equal = canonical_result();
    assert_eq!(
        first_typed_parity_mismatch(&baseline, &equal).unwrap(),
        None
    );
}

#[test]
fn adapted_repository_accepts_quarantine_non_active_schema_for_policy_evaluation() {
    let repository = TestRepository::new();
    let bundle = add_fixture(&repository, "external", "external-fragment", b"hello");
    rewrite(&bundle.join("fixture.toml"), |text| {
        text.replace(
            "[source]\nkind = \"native\"",
            "[source]\nkind = \"quarantine\"\ntracking_issue = \"#ae13-test\"",
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
