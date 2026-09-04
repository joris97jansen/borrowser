#![cfg(feature = "aggregate")]

use std::fs;
use std::path::Path;

use conformance_runner::{
    AggregateExecutionRequest, ExternalRegistryDiagnosticDetail, ExternalRegistryDiagnosticField,
    ExternalRegistryDiagnosticKind, ExternalRegistryDiagnosticSubjectKey,
    ExternalRegistryTrackInvariantField, ExternalRegistryValidationPhase,
    build_aggregate_detail_v1, build_aggregate_summary_v1,
    load_repository_external_advisory_evidence, run_repository_aggregate,
};
use conformance_test_support::LanePolicyScope;
use external_test_provenance::{TargetParserInputContextV1, sha256};

const EMPTY: &str = "format = \"borrowser-cross-engine-comparison-registry-v1\"\ncaptures = []\nattachments = []\nadvisory_tracks = []\nbaseline_notes = []\n";
const ARTIFACT: &[u8] = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";
const CAPTURE_ID: &str = "sha256:4179e64c74adbe3d558f24aeab8ee011cf552ad39c7d467a4a774ee49ed404c8";
const ARTIFACT_SHA256: &str = "506b85cc1ccf668e6d99a07c6e8657efb43cb513abaa343810f0ad082b407475";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

fn aggregate() -> conformance_runner::AggregateRun {
    run_repository_aggregate(
        repository_root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .unwrap()
}

fn temporary_repository(registry: &str) -> tempfile::TempDir {
    temporary_repository_bytes(registry.as_bytes())
}

fn temporary_repository_bytes(registry: &[u8]) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let external = root.path().join("tests/conformance/external");
    fs::create_dir_all(external.join("captures")).unwrap();
    fs::write(external.join("cross-engine-comparisons.toml"), registry).unwrap();
    root
}

fn valid_registry() -> String {
    format!(
        r#"format = "borrowser-cross-engine-comparison-registry-v1"

[[captures]]
capture_id = "{CAPTURE_ID}"
artifact_path = "tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt"
provenance_format = "borrowser-external-capture-provenance-v1"
engine_product = "engine"
engine_version = "1"
platform_os_family = "os"
platform_os_version = "1"
architecture = "arch"
viewport = {{ applicability = "applicable", width_css_px = 800, height_css_px = 600 }}
device_scale = {{ applicability = "not-applicable", reason = "not-used" }}
controlled_fonts = {{ applicability = "not-applicable", reason = "font-independent" }}
resource_network_policy = "offline"
pinned_resources = []
fixture_source_project = "fixture"
fixture_immutable_revision = "revision"
fixture_content_sha256 = "{ARTIFACT_SHA256}"
capture_mechanism = "tool"
capture_mechanism_version = "1"
capture_algorithm = "algorithm"
capture_algorithm_version = "1"
capture_algorithm_source_sha256 = "{ARTIFACT_SHA256}"
capture_configuration_sha256 = "{ARTIFACT_SHA256}"
invocation_arguments = ["--one", "--one"]
artifact_format = "web-observable-dom-tree-v1"
artifact_utf8_byte_length = 115
artifact_sha256 = "{ARTIFACT_SHA256}"
target_parser_input_context = "static-text-html-utf8-scripting-disabled-v1"
collection_policy = "stable"
collection_policy_version = "1"

[[advisory_tracks]]
track_id = "track"
engine_product = "engine"
platform_os_family = "os"
architecture = "arch"
comparable_observation_surface = "web-observable-dom-tree-v1"
capture_algorithm = "algorithm"
capture_algorithm_version = "1"
target_parser_input_context = "static-text-html-utf8-scripting-disabled-v1"
collection_policy = "stable"
collection_policy_version = "1"

[[attachments]]
test_id = "dom-tree-basic-document"
observation_surface = "dom-tree"
execution_variant = {{ kind = "singleton" }}
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "{CAPTURE_ID}"

[[baseline_notes]]
note_id = "baseline"
test_id = "dom-tree-basic-document"
observation_surface = "dom-tree"
execution_variant = {{ kind = "singleton" }}
comparable_observation_surface = "web-observable-dom-tree-v1"
text = "advisory only"
capture_id = "{CAPTURE_ID}"
"#
    )
}

fn populated_repository(registry: &str, artifact: &[u8]) -> tempfile::TempDir {
    let root = temporary_repository(registry);
    fs::write(
        root.path()
            .join("tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt"),
        artifact,
    )
    .unwrap();
    root
}

fn registry_error(
    registry: &str,
    artifact: &[u8],
) -> conformance_runner::ExternalRegistryDiagnostic {
    let run = aggregate();
    let root = populated_repository(registry, artifact);
    load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap()
}

#[test]
fn exact_empty_registry_reconciles_without_changing_aggregate_truth() {
    let run = aggregate();
    let before = run.clone();
    let summary = build_aggregate_summary_v1(&run).unwrap();
    let detail = build_aggregate_detail_v1(&run).unwrap();
    let evidence = load_repository_external_advisory_evidence(repository_root(), &run).unwrap();
    assert!(evidence.captures().is_empty());
    assert!(evidence.attachments().is_empty());
    assert!(evidence.notes().is_empty());
    assert_eq!(evidence.track_count(), 0);
    assert_eq!(evidence.verified_artifact_bytes_total(), 0);
    assert_eq!(run, before);
    assert_eq!(build_aggregate_summary_v1(&run).unwrap(), summary);
    assert_eq!(build_aggregate_detail_v1(&run).unwrap(), detail);
}

#[test]
fn schema_diagnostics_are_typed_and_do_not_preserve_parser_wording() {
    let run = aggregate();
    let unknown =
        temporary_repository(&EMPTY.replace("captures = []", "captures = []\nunknown = true"));
    let error = load_repository_external_advisory_evidence(unknown.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::Schema);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidRegistrySchema
    );

    let unsupported = temporary_repository(
        &EMPTY.replace("borrowser-cross-engine-comparison-registry-v1", "future"),
    );
    let error = load_repository_external_advisory_evidence(unsupported.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::UnsupportedRegistryFormat
    );
}

#[test]
fn non_dom_authored_surface_fails_in_phase_four_without_dom_rewriting() {
    let run = aggregate();
    let registry = r#"format = "borrowser-cross-engine-comparison-registry-v1"
captures = []
advisory_tracks = []
baseline_notes = []

[[attachments]]
test_id = "dom-tree-basic-document"
observation_surface = "css-cascade"
execution_variant = { kind = "singleton" }
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
"#;
    let root = temporary_repository(registry);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidAttachmentField
    );
    assert_eq!(
        error.detail(),
        ExternalRegistryDiagnosticDetail::Field(
            ExternalRegistryDiagnosticField::ObservationSurface,
        )
    );
}

#[test]
fn fixed_registry_path_is_required() {
    let run = aggregate();
    let root = tempfile::tempdir().unwrap();
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::RegistryMissing
    );
}

#[test]
fn exact_dom_singleton_capture_track_and_note_reconcile() {
    let run = aggregate();
    let before = run.clone();
    let summary = build_aggregate_summary_v1(&run).unwrap();
    let detail = build_aggregate_detail_v1(&run).unwrap();
    let root = populated_repository(&valid_registry(), ARTIFACT);
    let evidence = load_repository_external_advisory_evidence(root.path(), &run).unwrap();
    assert_eq!(evidence.captures().len(), 1);
    assert_eq!(evidence.attachments().len(), 1);
    assert_eq!(evidence.notes().len(), 1);
    assert_eq!(evidence.track_count(), 1);
    assert_eq!(evidence.tracks().len(), 1);
    let track = &evidence.tracks()[0];
    assert_eq!(track.id().as_str(), "track");
    assert_eq!(track.engine_product().as_str(), "engine");
    assert_eq!(track.platform_os_family().as_str(), "os");
    assert_eq!(track.architecture().as_str(), "arch");
    assert_eq!(track.comparable().as_str(), "web-observable-dom-tree-v1");
    assert_eq!(track.capture_algorithm().as_str(), "algorithm");
    assert_eq!(track.capture_algorithm_version().as_str(), "1");
    assert_eq!(track.collection_policy().as_str(), "stable");
    assert_eq!(track.collection_policy_version().as_str(), "1");
    assert_eq!(
        track.target_parser_input_context(),
        TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1
    );
    assert_eq!(evidence.verified_artifact_bytes_total(), 115);
    assert_eq!(
        evidence.captures()[0].capture().artifact().bytes(),
        ARTIFACT
    );
    fs::write(
        root.path()
            .join("tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt"),
        b"replacement after validation",
    )
    .unwrap();
    assert_eq!(
        evidence.captures()[0].capture().artifact().bytes(),
        ARTIFACT,
        "validated evidence retains exact bytes instead of reopening storage"
    );
    assert_eq!(
        evidence.attachments()[0]
            .aggregate_variant()
            .key
            .observation
            .as_str(),
        "dom-tree"
    );
    assert_eq!(run, before);
    assert_eq!(build_aggregate_summary_v1(&run).unwrap(), summary);
    assert_eq!(build_aggregate_detail_v1(&run).unwrap(), detail);
}

#[test]
fn storage_path_is_non_identity_and_notes_may_omit_capture_references() {
    let run = aggregate();
    let registry = valid_registry()
        .replace(
            "empty.web-observable-dom-tree-v1.txt",
            "renamed.web-observable-dom-tree-v1.txt",
        )
        .replace(
            &format!("text = \"advisory only\"\ncapture_id = \"{CAPTURE_ID}\"\n"),
            "text = \"advisory only\"\n",
        );
    let root = temporary_repository(&registry);
    fs::write(
        root.path()
            .join("tests/conformance/external/captures/renamed.web-observable-dom-tree-v1.txt"),
        ARTIFACT,
    )
    .unwrap();
    let evidence = load_repository_external_advisory_evidence(root.path(), &run).unwrap();
    assert_eq!(
        evidence.captures()[0].capture().id().to_string(),
        CAPTURE_ID
    );
    assert_eq!(evidence.notes()[0].capture_id(), None);
}

#[test]
fn artifact_tampering_and_capture_id_mismatch_fail_closed() {
    let run = aggregate();
    let root = populated_repository(&valid_registry(), b"tampered");
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactLengthMismatch
    );

    let registry = valid_registry().replace(
        CAPTURE_ID,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    let root = populated_repository(&registry, ARTIFACT);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::CaptureIdMismatch
    );
}

#[test]
fn declared_cumulative_limit_is_owned_only_by_phase_three() {
    let run = aggregate();
    let exact = valid_registry().replace(
        "artifact_utf8_byte_length = 115",
        "artifact_utf8_byte_length = 8388608",
    );
    let root = populated_repository(&exact, ARTIFACT);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::ArtifactIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactLengthMismatch
    );

    let above = valid_registry().replace(
        "artifact_utf8_byte_length = 115",
        "artifact_utf8_byte_length = 8388609",
    );
    let root = populated_repository(&above, ARTIFACT);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::TopLevelMultiplicity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::CumulativeArtifactBytesExceeded
    );
}

#[test]
fn declared_artifact_accumulation_overflow_has_phase_three_precedence() {
    let valid = valid_registry();
    let capture_start = valid.find("[[captures]]").unwrap();
    let track_start = valid.find("[[advisory_tracks]]").unwrap();
    let capture = valid[capture_start..track_start].replace(
        "artifact_utf8_byte_length = 115",
        "artifact_utf8_byte_length = 9223372036854775807",
    );
    let mut registry = EMPTY.replace("captures = []", "");
    registry.push_str(&capture);
    registry.push_str(&capture);
    registry.push_str(&capture);
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::TopLevelMultiplicity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::DeclaredArtifactBytesOverflow
    );
}

#[test]
fn valid_dom_surface_must_resolve_for_the_authoritative_case() {
    let run = aggregate();
    for test_id in [
        "css-selector-matching-parser-dom",
        "layout-geometry-basic-block-flow",
    ] {
        let registry = valid_registry().replace("dom-tree-basic-document", test_id);
        let root = populated_repository(&registry, ARTIFACT);
        let error = load_repository_external_advisory_evidence(root.path(), &run)
            .err()
            .unwrap();
        assert_eq!(
            error.phase(),
            ExternalRegistryValidationPhase::AggregateReconciliation
        );
        assert_eq!(
            error.kind(),
            ExternalRegistryDiagnosticKind::UnknownObservationSurface
        );
    }
}

#[test]
fn track_invariant_and_internal_references_are_closed() {
    let run = aggregate();
    let changed = valid_registry().replace(
        "engine_product = \"engine\"\nplatform_os_family",
        "engine_product = \"other\"\nplatform_os_family",
    );
    let root = populated_repository(&changed, ARTIFACT);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::TrackInvariantMismatch
    );

    let unknown_track =
        valid_registry().replacen("track_id = \"track\"", "track_id = \"other\"", 1);
    let root = populated_repository(&unknown_track, ARTIFACT);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::UnknownTrackReference
    );
}

#[test]
fn note_authored_with_a_non_dom_surface_is_rejected_in_phase_four() {
    let registry = valid_registry().replace(
        "note_id = \"baseline\"\ntest_id = \"dom-tree-basic-document\"\nobservation_surface = \"dom-tree\"",
        "note_id = \"baseline\"\ntest_id = \"dom-tree-basic-document\"\nobservation_surface = \"css-cascade\"",
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidNoteField
    );
    assert_eq!(
        error.detail(),
        ExternalRegistryDiagnosticDetail::Field(
            ExternalRegistryDiagnosticField::ObservationSurface,
        )
    );
}

#[test]
fn phase_four_invalid_item_precedes_phase_five_duplicate_set_identity() {
    let duplicate_resources = format!(
        "pinned_resources = [{{ identity = \"resource\", content_sha256 = \"{ARTIFACT_SHA256}\" }}, {{ identity = \"resource\", content_sha256 = \"{ARTIFACT_SHA256}\" }}]"
    );
    let duplicate = valid_registry().replace("pinned_resources = []", &duplicate_resources);
    let error = registry_error(&duplicate, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::DuplicateIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::DuplicatePinnedResource
    );

    let invalid_and_duplicate =
        duplicate.replace("engine_product = \"engine\"", "engine_product = \"\"");
    let error = registry_error(&invalid_and_duplicate, ARTIFACT);
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidCaptureField
    );
}

#[test]
fn attachment_uniqueness_ignores_the_referenced_capture_id() {
    let mut registry = valid_registry();
    let attachment_start = registry.find("[[attachments]]").unwrap();
    let note_start = registry.find("[[baseline_notes]]").unwrap();
    let second = registry[attachment_start..note_start].replace(
        CAPTURE_ID,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
    );
    registry.insert_str(note_start, &second);
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::DuplicateIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::DuplicateAttachmentKey
    );
}

#[test]
fn phase_six_distinguishes_digest_format_and_symlink_failures() {
    let stale_digest = valid_registry().replace(
        &format!("artifact_sha256 = \"{ARTIFACT_SHA256}\""),
        "artifact_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"",
    );
    let error = registry_error(&stale_digest, ARTIFACT);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactDigestMismatch
    );

    let mut malformed = ARTIFACT.to_vec();
    malformed[0] = b'F';
    let digest = sha256(&malformed).to_hex();
    let invalid_format = valid_registry().replace(ARTIFACT_SHA256, &digest);
    let error = registry_error(&invalid_format, &malformed);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactFormatInvalid
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let run = aggregate();
        let root = populated_repository(&valid_registry(), ARTIFACT);
        let artifact_path = root
            .path()
            .join("tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt");
        fs::remove_file(&artifact_path).unwrap();
        symlink("target.web-observable-dom-tree-v1.txt", &artifact_path).unwrap();
        let error = load_repository_external_advisory_evidence(root.path(), &run)
            .err()
            .unwrap();
        assert_eq!(
            error.kind(),
            ExternalRegistryDiagnosticKind::ArtifactSymlink
        );
    }
}

#[test]
fn every_advisory_track_invariant_is_enforced() {
    let cases = [
        (
            "engine_product = \"engine\"",
            "engine_product = \"other\"",
            ExternalRegistryTrackInvariantField::EngineProduct,
        ),
        (
            "platform_os_family = \"os\"",
            "platform_os_family = \"other\"",
            ExternalRegistryTrackInvariantField::PlatformOsFamily,
        ),
        (
            "architecture = \"arch\"",
            "architecture = \"other\"",
            ExternalRegistryTrackInvariantField::Architecture,
        ),
        (
            "capture_algorithm = \"algorithm\"",
            "capture_algorithm = \"other\"",
            ExternalRegistryTrackInvariantField::CaptureAlgorithm,
        ),
        (
            "capture_algorithm_version = \"1\"",
            "capture_algorithm_version = \"2\"",
            ExternalRegistryTrackInvariantField::CaptureAlgorithmVersion,
        ),
        (
            "target_parser_input_context = \"static-text-html-utf8-scripting-disabled-v1\"",
            "target_parser_input_context = \"other\"",
            ExternalRegistryTrackInvariantField::TargetParserInputContext,
        ),
        (
            "collection_policy = \"stable\"",
            "collection_policy = \"other\"",
            ExternalRegistryTrackInvariantField::CollectionPolicy,
        ),
        (
            "collection_policy_version = \"1\"",
            "collection_policy_version = \"2\"",
            ExternalRegistryTrackInvariantField::CollectionPolicyVersion,
        ),
    ];
    for (from, to, detail) in cases {
        let mut registry = valid_registry();
        let track_start = registry.find("[[advisory_tracks]]").unwrap();
        let suffix = registry[track_start..].replacen(from, to, 1);
        registry.truncate(track_start);
        registry.push_str(&suffix);
        let error = registry_error(&registry, ARTIFACT);
        if detail == ExternalRegistryTrackInvariantField::TargetParserInputContext {
            assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
            assert_eq!(
                error.kind(),
                ExternalRegistryDiagnosticKind::InvalidTrackField
            );
        } else {
            assert_eq!(
                error.kind(),
                ExternalRegistryDiagnosticKind::TrackInvariantMismatch,
                "invariant {detail:?}"
            );
            assert_eq!(
                error.detail(),
                ExternalRegistryDiagnosticDetail::TrackInvariant(detail),
            );
        }
    }
}

#[test]
fn phase_four_diagnostic_choice_is_independent_of_capture_declaration_order() {
    let registry = valid_registry();
    let capture_start = registry.find("[[captures]]").unwrap();
    let track_start = registry.find("[[advisory_tracks]]").unwrap();
    let capture = &registry[capture_start..track_start];
    let remainder = &registry[track_start..];
    let capture_a = capture.replace(CAPTURE_ID, "a");
    let capture_z = capture.replace(CAPTURE_ID, "z");
    for captures in [
        format!("{capture_z}{capture_a}"),
        format!("{capture_a}{capture_z}"),
    ] {
        let candidate = format!("{}{}{}", &registry[..capture_start], captures, remainder);
        let error = registry_error(&candidate, ARTIFACT);
        assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
        assert_eq!(
            error.kind(),
            ExternalRegistryDiagnosticKind::InvalidCaptureIdClaim
        );
        assert_eq!(
            error.subject(),
            &ExternalRegistryDiagnosticSubjectKey::Capture {
                supplied_capture_id: "a".to_owned(),
            }
        );
    }
}

#[test]
fn malformed_attachment_subjects_use_component_tuple_order_without_delimiters() {
    let attachment_a = r#"
[[attachments]]
test_id = "a"
observation_surface = "b\u0000c"
execution_variant = { kind = "singleton" }
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
"#;
    let attachment_b = r#"
[[attachments]]
test_id = "a\u0000b"
observation_surface = "c"
execution_variant = { kind = "singleton" }
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
"#;
    for authored in [
        format!("{attachment_b}{attachment_a}"),
        format!("{attachment_a}{attachment_b}"),
    ] {
        let registry = format!(
            "format = \"borrowser-cross-engine-comparison-registry-v1\"\ncaptures = []\nadvisory_tracks = []\nbaseline_notes = []\n{authored}"
        );
        let error = registry_error(&registry, ARTIFACT);
        let ExternalRegistryDiagnosticSubjectKey::Attachment(subject) = error.subject() else {
            panic!(
                "expected typed attachment subject, got {:?}",
                error.subject()
            );
        };
        assert_eq!(subject.test_id(), "a");
        assert_eq!(subject.observation_surface(), "b\0c");
        assert_eq!(subject.execution_variant_kind(), "singleton");
        assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    }
}

#[test]
fn attachment_diagnostics_order_details_before_materializing_the_owned_subject() {
    let registry = format!(
        r#"format = "borrowser-cross-engine-comparison-registry-v1"
captures = []
advisory_tracks = []
baseline_notes = []

[[attachments]]
test_id = "dom-tree-basic-document"
observation_surface = "unsupported"
execution_variant = {{ kind = "wrong" }}
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "{CAPTURE_ID}"
"#
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidAttachmentField
    );
    assert_eq!(
        error.detail(),
        ExternalRegistryDiagnosticDetail::Field(ExternalRegistryDiagnosticField::ExecutionVariant)
    );
}

#[test]
fn earliest_failing_attachment_tuple_wins_independently_of_declaration_order() {
    let attachment = |test_id: &str| {
        format!(
            r#"
[[attachments]]
test_id = "{test_id}"
observation_surface = "unsupported"
execution_variant = {{ kind = "singleton" }}
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "track"
capture_id = "{CAPTURE_ID}"
"#
        )
    };
    let prefix = "format = \"borrowser-cross-engine-comparison-registry-v1\"\ncaptures = []\nadvisory_tracks = []\nbaseline_notes = []\n";
    for authored in [
        format!("{}{}", attachment("z"), attachment("a")),
        format!("{}{}", attachment("a"), attachment("z")),
    ] {
        let error = registry_error(&format!("{prefix}{authored}"), ARTIFACT);
        let ExternalRegistryDiagnosticSubjectKey::Attachment(subject) = error.subject() else {
            panic!("expected attachment subject");
        };
        assert_eq!(subject.test_id(), "a");
        assert_eq!(
            error.kind(),
            ExternalRegistryDiagnosticKind::InvalidAttachmentField
        );
        assert_eq!(
            error.detail(),
            ExternalRegistryDiagnosticDetail::Field(
                ExternalRegistryDiagnosticField::ObservationSurface
            )
        );
    }
}

#[test]
fn nested_and_top_level_multiplicity_limits_fail_in_their_own_phases() {
    let arguments = (0..17)
        .map(|index| format!("\"--{index}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let registry = valid_registry().replace(
        "invocation_arguments = [\"--one\", \"--one\"]",
        &format!("invocation_arguments = [{arguments}]"),
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::TooManyInvocationArguments
    );

    let mut notes = EMPTY.to_owned();
    notes = notes.replace("baseline_notes = []", "");
    for index in 0..257 {
        notes.push_str(&format!(
            "[[baseline_notes]]\nnote_id = \"note-{index}\"\ntest_id = \"dom-tree-basic-document\"\nobservation_surface = \"dom-tree\"\nexecution_variant = {{ kind = \"singleton\" }}\ncomparable_observation_surface = \"web-observable-dom-tree-v1\"\ntext = \"note\"\n"
        ));
    }
    let error = registry_error(&notes, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::TopLevelMultiplicity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::TooManyBaselineNotes
    );
}

#[test]
fn remaining_nested_multiplicity_limits_are_typed() {
    let fonts = (0..17)
        .map(|index| {
            format!(
                "{{ family = \"font-{index}\", face_style = \"regular\", version = \"1\", file_sha256 = \"{ARTIFACT_SHA256}\" }}"
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let registry = valid_registry().replace(
        "controlled_fonts = { applicability = \"not-applicable\", reason = \"font-independent\" }",
        &format!("controlled_fonts = {{ applicability = \"applicable\", items = [{fonts}] }}"),
    );
    assert_eq!(
        registry_error(&registry, ARTIFACT).kind(),
        ExternalRegistryDiagnosticKind::TooManyControlledFonts
    );

    let resources = (0..33)
        .map(|index| {
            format!("{{ identity = \"resource-{index}\", content_sha256 = \"{ARTIFACT_SHA256}\" }}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let registry = valid_registry().replace(
        "pinned_resources = []",
        &format!("pinned_resources = [{resources}]"),
    );
    assert_eq!(
        registry_error(&registry, ARTIFACT).kind(),
        ExternalRegistryDiagnosticKind::TooManyPinnedResources
    );
}

#[test]
fn every_top_level_collection_limit_is_enforced_before_record_validation() {
    let valid = valid_registry();
    let capture_start = valid.find("[[captures]]").unwrap();
    let track_start = valid.find("[[advisory_tracks]]").unwrap();
    let attachment_start = valid.find("[[attachments]]").unwrap();
    let note_start = valid.find("[[baseline_notes]]").unwrap();
    let records = [
        (
            "captures = []",
            &valid[capture_start..track_start],
            ExternalRegistryDiagnosticKind::TooManyCaptures,
        ),
        (
            "advisory_tracks = []",
            &valid[track_start..attachment_start],
            ExternalRegistryDiagnosticKind::TooManyAdvisoryTracks,
        ),
        (
            "attachments = []",
            &valid[attachment_start..note_start],
            ExternalRegistryDiagnosticKind::TooManyAttachments,
        ),
        (
            "baseline_notes = []",
            &valid[note_start..],
            ExternalRegistryDiagnosticKind::TooManyBaselineNotes,
        ),
    ];
    for (empty_declaration, record, expected) in records {
        let mut registry = EMPTY.replace(empty_declaration, "");
        for _ in 0..257 {
            registry.push_str(record);
        }
        let error = registry_error(&registry, ARTIFACT);
        assert_eq!(
            error.phase(),
            ExternalRegistryValidationPhase::TopLevelMultiplicity
        );
        assert_eq!(error.kind(), expected);
    }
}

#[test]
fn duplicate_capture_track_and_note_ids_fail_before_artifact_loading() {
    let registry = valid_registry();
    let capture_start = registry.find("[[captures]]").unwrap();
    let track_start = registry.find("[[advisory_tracks]]").unwrap();
    let attachment_start = registry.find("[[attachments]]").unwrap();
    let note_start = registry.find("[[baseline_notes]]").unwrap();
    let capture = &registry[capture_start..track_start];
    let track = &registry[track_start..attachment_start];
    let note = &registry[note_start..];

    let duplicate_capture = format!(
        "{}{}{}{}",
        &registry[..track_start],
        capture,
        &registry[track_start..note_start],
        note
    );
    assert_eq!(
        registry_error(&duplicate_capture, ARTIFACT).kind(),
        ExternalRegistryDiagnosticKind::DuplicateCaptureId
    );

    let duplicate_track = format!(
        "{}{}{}",
        &registry[..attachment_start],
        track,
        &registry[attachment_start..]
    );
    assert_eq!(
        registry_error(&duplicate_track, ARTIFACT).kind(),
        ExternalRegistryDiagnosticKind::DuplicateTrackId
    );

    let duplicate_note = format!("{registry}{note}");
    assert_eq!(
        registry_error(&duplicate_note, ARTIFACT).kind(),
        ExternalRegistryDiagnosticKind::DuplicateNoteId
    );
}

#[test]
fn unknown_test_id_is_a_phase_eight_failure() {
    let registry = valid_registry().replace(
        "dom-tree-basic-document",
        "dom-tree-not-in-authoritative-aggregate",
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::AggregateReconciliation
    );
    assert_eq!(error.kind(), ExternalRegistryDiagnosticKind::UnknownTestId);
}

#[test]
fn attachment_and_note_unknown_capture_references_fail_in_phase_seven() {
    const UNKNOWN: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    let registry = valid_registry();
    let attachment_start = registry.find("[[attachments]]").unwrap();
    let attachment_unknown = format!(
        "{}{}",
        &registry[..attachment_start],
        registry[attachment_start..].replacen(CAPTURE_ID, UNKNOWN, 1),
    );
    let error = registry_error(&attachment_unknown, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::InternalReconciliation
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::UnknownCaptureReference
    );
    assert!(matches!(
        error.subject(),
        ExternalRegistryDiagnosticSubjectKey::Attachment(_)
    ));

    let note_start = registry.find("[[baseline_notes]]").unwrap();
    let note_unknown = format!(
        "{}{}",
        &registry[..note_start],
        registry[note_start..].replacen(CAPTURE_ID, UNKNOWN, 1),
    );
    let error = registry_error(&note_unknown, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::InternalReconciliation
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::UnknownCaptureReference
    );
    assert_eq!(
        error.subject(),
        &ExternalRegistryDiagnosticSubjectKey::Note {
            note_id: "baseline".to_owned(),
        }
    );
}

#[test]
fn duplicate_controlled_font_is_a_runner_owned_phase_five_diagnostic() {
    let item = format!(
        "{{ family = \"font\", face_style = \"regular\", version = \"1\", file_sha256 = \"{ARTIFACT_SHA256}\" }}"
    );
    let registry = valid_registry().replace(
        "controlled_fonts = { applicability = \"not-applicable\", reason = \"font-independent\" }",
        &format!(
            "controlled_fonts = {{ applicability = \"applicable\", items = [{item}, {item}] }}"
        ),
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::DuplicateIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::DuplicateControlledFont
    );
}

#[test]
fn unsafe_authored_artifact_path_is_rejected_in_phase_four() {
    let registry = valid_registry().replace(
        "tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt",
        "tests/conformance/external/captures/../empty.web-observable-dom-tree-v1.txt",
    );
    let error = registry_error(&registry, ARTIFACT);
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RecordLocal);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::InvalidCaptureField
    );
    assert_eq!(
        error.detail(),
        ExternalRegistryDiagnosticDetail::Field(ExternalRegistryDiagnosticField::ArtifactPath)
    );
}

#[test]
fn missing_and_non_regular_artifacts_map_to_phase_six_diagnostics() {
    let run = aggregate();
    let root = temporary_repository(&valid_registry());
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::ArtifactIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactMissing
    );

    let root = temporary_repository(&valid_registry());
    fs::create_dir(
        root.path()
            .join("tests/conformance/external/captures/empty.web-observable-dom-tree-v1.txt"),
    )
    .unwrap();
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(
        error.phase(),
        ExternalRegistryValidationPhase::ArtifactIdentity
    );
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::ArtifactNotRegular
    );
}

#[test]
fn registry_non_regular_oversized_and_invalid_utf8_inputs_are_typed() {
    let run = aggregate();

    let root = tempfile::tempdir().unwrap();
    let external = root.path().join("tests/conformance/external");
    fs::create_dir_all(external.join("captures")).unwrap();
    fs::create_dir(external.join("cross-engine-comparisons.toml")).unwrap();
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RegistryRead);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::RegistryNotRegular
    );

    let oversized = vec![b'x'; 512 * 1024 + 1];
    let root = temporary_repository_bytes(&oversized);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RegistryRead);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::RegistryTooLarge
    );

    let root = temporary_repository_bytes(&[0xff]);
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RegistryRead);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::RegistryInvalidUtf8
    );
}

#[cfg(unix)]
#[test]
fn registry_symlink_maps_to_the_frozen_phase_one_diagnostic() {
    use std::os::unix::fs::symlink;

    let run = aggregate();
    let root = tempfile::tempdir().unwrap();
    let external = root.path().join("tests/conformance/external");
    fs::create_dir_all(external.join("captures")).unwrap();
    fs::write(external.join("target.toml"), EMPTY).unwrap();
    symlink(
        "target.toml",
        external.join("cross-engine-comparisons.toml"),
    )
    .unwrap();
    let error = load_repository_external_advisory_evidence(root.path(), &run)
        .err()
        .unwrap();
    assert_eq!(error.phase(), ExternalRegistryValidationPhase::RegistryRead);
    assert_eq!(
        error.kind(),
        ExternalRegistryDiagnosticKind::RegistrySymlink
    );
}
