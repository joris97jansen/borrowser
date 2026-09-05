use super::*;
use crate::*;
use conformance_test_support::{LanePolicyScope, ObservationSurface, TestId};
use external_test_provenance::*;
use std::fs;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn operation() -> SelectedDomOperationRun {
    run_repository_aggregate_for_selected_dom_operation(
        &root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
        SelectedDomOperationRequest {
            selected: AggregateVariantKey {
                test_id: TestId::parse("dom-tree-basic-document").unwrap(),
                observation: ObservationSurface::DomTree,
                variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
                    SingletonExecutionVariant::Singleton,
                )),
            },
        },
    )
    .unwrap()
}
fn input(
    bytes: &[u8],
    sources: &VerifiedCaptureSourcesV1,
    fixture: Sha256Digest,
) -> ExternalCaptureProvenanceV1Input {
    let id = |s| ExternalIdentityV1::parse(s).unwrap();
    let version = || ExternalVersionV1::parse("1").unwrap();
    ExternalCaptureProvenanceV1Input {
        engine_product: id("synthetic"),
        engine_version: version(),
        engine_build_revision: None,
        platform_os_family: id("synthetic"),
        platform_os_version: version(),
        architecture: id("synthetic"),
        viewport: ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse("synthetic").unwrap(),
        ),
        device_scale: ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse("synthetic").unwrap(),
        ),
        controlled_fonts: ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse("synthetic").unwrap(),
        ),
        resource_network_policy: ResourceNetworkPolicyV1::Offline,
        pinned_resources: vec![],
        fixture_source_project: id("synthetic"),
        fixture_immutable_revision: ImmutableRevision::parse("synthetic").unwrap(),
        fixture_content_sha256: fixture,
        capture_mechanism: id("synthetic-test-only"),
        capture_mechanism_version: version(),
        capture_algorithm: id("web-observable-dom-tree-v1-inspector"),
        capture_algorithm_version: version(),
        capture_algorithm_source_sha256: sources.algorithm_sha256(),
        capture_configuration_sha256: sources.configuration_sha256(),
        invocation_arguments: vec![],
        artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
        artifact_utf8_byte_length: bytes.len() as u64,
        artifact_sha256: sha256(bytes),
        target_parser_input_context:
            TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
        collection_policy: id("synthetic"),
        collection_policy_version: version(),
    }
}
fn seal(
    tmp: &Path,
    path: &str,
    bytes: &[u8],
    input: ExternalCaptureProvenanceV1Input,
) -> ValidatedExternalCaptureV1 {
    fs::write(tmp.join(path), bytes).unwrap();
    let artifact = read_external_artifact_candidate_same_object(tmp, Path::new(path))
        .unwrap()
        .validate(ExternalArtifactFormatV1::WebObservableDomTreeV1)
        .unwrap();
    // Fixed synthetic claims, computed once by the source-neutral identity owner.
    // They deliberately bind the reviewed raw inspector/config bytes: edits require
    // reviewed claim updates, never a second identity algorithm in this crate.
    let digest = if input.capture_algorithm_source_sha256 == sha256(b"wrong") {
        "dab384b398e879532136279204b1971c5e4f658a76a4ebb24858a27c56bedc6e"
    } else if input.capture_configuration_sha256 == sha256(b"wrong") {
        "94908753ff5a7895d16e2df36f7e12705df0ce7a27e03f1c0117592e5613743f"
    } else if input.fixture_content_sha256 == sha256(b"wrong") {
        "0c18cc084576bef606859edd83dd99201f102d2abe706a1b2e13006b7369c492"
    } else if input.capture_algorithm
        != ExternalIdentityV1::parse("web-observable-dom-tree-v1-inspector").unwrap()
    {
        "f72fa7a6af6412290e358bbf3f6de0f214e938decb2d849657e29e0bfc22b879"
    } else if bytes == vectors("static-document.txt") {
        "c928394f159bf1ca40c27a2d1466c0034fb1c7286f70e8878276e2c6d4998c49"
    } else {
        assert_eq!(bytes, vectors("static-document-different.txt"));
        "a4b967a796966e54cbbaa90aea6c6fe664bc0e34e4299bda3f46470a0a9d0068"
    };
    ValidatedExternalCaptureV1::verify(
        ExternalCaptureProvenanceV1::try_from_input(input).unwrap(),
        artifact,
        ExternalCaptureIdClaim::parse(&format!("sha256:{digest}")).unwrap(),
    )
    .unwrap()
}
fn vectors(name: &str) -> Vec<u8> {
    fs::read(
        root()
            .join("tests/contract-vectors/web-observable-dom-tree-v1")
            .join(name),
    )
    .unwrap()
}
fn registry(sources: &VerifiedCaptureSourcesV1) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("tests/conformance/external/captures")).unwrap();
    fs::create_dir_all(tmp.path().join("tools/conformance")).unwrap();
    for p in [CAPTURE_ALGORITHM_PATH_V1, CAPTURE_CONFIGURATION_PATH_V1] {
        fs::copy(root().join(p), tmp.path().join(p)).unwrap();
    }
    let fixture = sha256(
        &fs::read(root().join("tests/conformance/fixtures/html/dom-tree-basic/parser/input.html"))
            .unwrap(),
    );
    let mut wire = String::from(
        "format = \"borrowser-cross-engine-comparison-registry-v1\"\nbaseline_notes = []\n",
    );
    let mut ids = vec![];
    for (index, name) in ["static-document.txt", "static-document-different.txt"]
        .into_iter()
        .enumerate()
    {
        let bytes = vectors(name);
        let path =
            format!("tests/conformance/external/captures/{index}.web-observable-dom-tree-v1.txt");
        let capture = seal(tmp.path(), &path, &bytes, input(&bytes, sources, fixture));
        let id = capture.id();
        ids.push(id);
        wire.push_str(&format!(
            r#"
[[captures]]
capture_id = "{id}"
artifact_path = "{path}"
provenance_format = "borrowser-external-capture-provenance-v1"
engine_product = "synthetic"
engine_version = "1"
platform_os_family = "synthetic"
platform_os_version = "1"
architecture = "synthetic"
viewport = {{ applicability = "not-applicable", reason = "synthetic" }}
device_scale = {{ applicability = "not-applicable", reason = "synthetic" }}
controlled_fonts = {{ applicability = "not-applicable", reason = "synthetic" }}
resource_network_policy = "offline"
pinned_resources = []
fixture_source_project = "synthetic"
fixture_immutable_revision = "synthetic"
fixture_content_sha256 = "{fixture}"
capture_mechanism = "synthetic-test-only"
capture_mechanism_version = "1"
capture_algorithm = "web-observable-dom-tree-v1-inspector"
capture_algorithm_version = "1"
capture_algorithm_source_sha256 = "{algorithm}"
capture_configuration_sha256 = "{config}"
invocation_arguments = []
artifact_format = "web-observable-dom-tree-v1"
artifact_utf8_byte_length = {length}
artifact_sha256 = "{hash}"
target_parser_input_context = "static-text-html-utf8-scripting-disabled-v1"
collection_policy = "synthetic"
collection_policy_version = "1"
"#,
            algorithm = sources.algorithm_sha256(),
            config = sources.configuration_sha256(),
            length = bytes.len(),
            hash = sha256(&bytes)
        ));
    }
    for (track, case, capture) in [
        ("a", "dom-tree-basic-document", ids[0]),
        ("b", "dom-tree-basic-document", ids[1]),
        ("c", "dom-tree-representative-static-document", ids[0]),
    ] {
        wire.push_str(&format!(
            r#"
[[advisory_tracks]]
track_id = "{track}"
engine_product = "synthetic"
platform_os_family = "synthetic"
architecture = "synthetic"
comparable_observation_surface = "web-observable-dom-tree-v1"
capture_algorithm = "web-observable-dom-tree-v1-inspector"
capture_algorithm_version = "1"
target_parser_input_context = "static-text-html-utf8-scripting-disabled-v1"
collection_policy = "synthetic"
collection_policy_version = "1"
[[attachments]]
test_id = "{case}"
observation_surface = "dom-tree"
execution_variant = {{ kind = "singleton" }}
comparable_observation_surface = "web-observable-dom-tree-v1"
track_id = "{track}"
capture_id = "{capture}"
"#
        ));
    }
    fs::write(
        tmp.path()
            .join("tests/conformance/external/cross-engine-comparisons.toml"),
        wire,
    )
    .unwrap();
    tmp
}
#[test]
fn synthetic_comparisons_are_scoped_and_use_retained_bytes() {
    let op = operation();
    let sources = VerifiedCaptureSourcesV1::load(&root()).unwrap();
    let tmp = registry(&sources);
    let ordinary = op.run.clone();
    let summary = build_aggregate_summary_v1(&op.run).unwrap();
    let detail = build_aggregate_detail_v1(&op.run).unwrap();
    let rejected = op.compare_external(tmp.path()).unwrap();
    assert_eq!(rejected.total_attachment_count(), 3);
    assert_eq!(rejected.in_scope_attachment_count(), 2);
    assert_eq!(rejected.outside_scope_attachment_count(), 1);
    assert!(rejected.evaluated().all(|(_, c)| matches!(
        c.result(),
        Err(AdvisoryComparisonFailure::UnsupportedCaptureContext)
    )));
    let evidence = load_repository_external_advisory_evidence(tmp.path(), &op.run).unwrap();
    let wire = fs::read_to_string(
        tmp.path()
            .join("tests/conformance/external/cross-engine-comparisons.toml"),
    )
    .unwrap();
    let claims: Vec<_> = wire
        .lines()
        .filter_map(|line| line.strip_prefix("capture_id = "))
        .collect();
    for stored in evidence.captures() {
        assert!(claims.contains(&format!("\"{}\"", stored.capture().id()).as_str()));
    }
    fs::remove_file(
        tmp.path()
            .join("tests/conformance/external/captures/0.web-observable-dom-tree-v1.txt"),
    )
    .unwrap();
    fs::write(
        tmp.path()
            .join("tests/conformance/external/captures/1.web-observable-dom-tree-v1.txt"),
        b"replaced!",
    )
    .unwrap();
    let compared = compare_selected(&op, evidence, &sources, &mut |_| Ok(())).unwrap();
    assert_eq!(
        compared.scope(),
        SelectedDomOperationScope::SelectedVariantOnly
    );
    assert_eq!(compared.selected(), op.selected());
    assert_eq!(
        compared
            .evaluated()
            .map(|(a, _)| a.track_id().as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        compared
            .outside_scope()
            .map(|a| a.track_id().as_str())
            .collect::<Vec<_>>(),
        ["c"]
    );
    let results: Vec<_> = compared.evaluated().map(|(_, c)| c.result()).collect();
    assert!(matches!(results[0], Ok(AdvisoryVerdict::Equivalent)));
    assert!(matches!(results[1], Ok(AdvisoryVerdict::Different { .. })));
    assert!(compared.retained_difference_bytes() > 0);
    assert_eq!(op.run, ordinary);
    assert_eq!(summary, build_aggregate_summary_v1(&op.run).unwrap());
    assert_eq!(detail, build_aggregate_detail_v1(&op.run).unwrap());
    assert!(op.compare_external(tmp.path()).is_err());
    assert_eq!(op.run, ordinary);
}
#[test]
fn sources_fixture_and_observation_failures_are_not_different() {
    let mut op = operation();
    let sources = VerifiedCaptureSourcesV1::load(&root()).unwrap();
    let bytes = vectors("static-document.txt");
    let fixture = sha256(
        &fs::read(root().join("tests/conformance/fixtures/html/dom-tree-basic/parser/input.html"))
            .unwrap(),
    );
    let tmp = tempfile::tempdir().unwrap();
    for (which, expected) in [
        (0, AdvisoryComparisonFailure::AlgorithmSourceMismatch),
        (1, AdvisoryComparisonFailure::ConfigurationSourceMismatch),
        (2, AdvisoryComparisonFailure::FixtureMismatch),
        (3, AdvisoryComparisonFailure::SourceIdentityMismatch),
    ] {
        let mut input = input(&bytes, &sources, fixture);
        match which {
            0 => input.capture_algorithm_source_sha256 = sha256(b"wrong"),
            1 => input.capture_configuration_sha256 = sha256(b"wrong"),
            2 => input.fixture_content_sha256 = sha256(b"wrong"),
            _ => input.capture_algorithm = ExternalIdentityV1::parse("wrong").unwrap(),
        }
        let capture = seal(tmp.path(), "capture.txt", &bytes, input);
        assert_eq!(
            compare_attachment(
                &op,
                &capture,
                &sources,
                &mut |_| Ok(()),
                &mut DifferenceBudget::default()
            ),
            Err(expected)
        );
    }
    let capture = seal(
        tmp.path(),
        "capture.txt",
        &bytes,
        input(&bytes, &sources, fixture),
    );
    op.observation = Err(DomObservationFailure::NotAttempted);
    assert_eq!(
        compare_attachment(
            &op,
            &capture,
            &sources,
            &mut |_| Ok(()),
            &mut DifferenceBudget::default()
        ),
        Err(AdvisoryComparisonFailure::Observation(
            DomObservationFailure::NotAttempted
        ))
    );
}

#[test]
fn complete_matching_registry_is_still_selected_scope_and_external_changes_are_isolated() {
    let op = operation();
    let sources = VerifiedCaptureSourcesV1::load(&root()).unwrap();
    let tmp = registry(&sources);
    let registry_path = tmp
        .path()
        .join("tests/conformance/external/cross-engine-comparisons.toml");
    let mut wire = fs::read_to_string(&registry_path).unwrap();
    wire.truncate(wire.rfind("[[advisory_tracks]]").unwrap());
    fs::write(&registry_path, &wire).unwrap();
    let evidence = load_repository_external_advisory_evidence(tmp.path(), &op.run).unwrap();
    let result = compare_selected(&op, evidence, &sources, &mut |_| Ok(())).unwrap();
    assert_eq!(
        result.scope(),
        SelectedDomOperationScope::SelectedVariantOnly
    );
    assert_eq!(result.total_attachment_count(), 2);
    assert_eq!(result.in_scope_attachment_count(), 2);
    assert_eq!(result.outside_scope_attachment_count(), 0);
    let baseline = op.run.clone();
    for changed in [
        wire.replace("track_id = \"a\"", "track_id = \"changed\""),
        wire.replace(
            "capture_algorithm_source_sha256 = \"",
            "capture_algorithm_source_sha256 = \"00",
        ),
        "malformed registry".into(),
    ] {
        fs::write(&registry_path, changed).unwrap();
        let _ = op.compare_external(tmp.path());
        assert_eq!(op.run, baseline);
    }
    fs::write(&registry_path, "format = \"borrowser-cross-engine-comparison-registry-v1\"\ncaptures=[]\nattachments=[]\nadvisory_tracks=[]\nbaseline_notes=[]\n").unwrap();
    assert_eq!(
        op.compare_external(tmp.path())
            .unwrap()
            .total_attachment_count(),
        0
    );
    fs::remove_file(tmp.path().join(CAPTURE_CONFIGURATION_PATH_V1)).unwrap();
    assert!(matches!(
        op.compare_external(tmp.path()),
        Err(SelectedDomOperationError::Sources(_))
    ));
    assert_eq!(op.run, baseline);
}

#[test]
fn valid_artifact_with_wrong_supplied_claim_cannot_become_a_capture() {
    let sources = VerifiedCaptureSourcesV1::load(&root()).unwrap();
    let bytes = vectors("static-document.txt");
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("artifact.txt"), &bytes).unwrap();
    let artifact =
        read_external_artifact_candidate_same_object(tmp.path(), Path::new("artifact.txt"))
            .unwrap()
            .validate(ExternalArtifactFormatV1::WebObservableDomTreeV1)
            .unwrap();
    let provenance =
        ExternalCaptureProvenanceV1::try_from_input(input(&bytes, &sources, sha256(b"fixture")))
            .unwrap();
    let wrong = ExternalCaptureIdClaim::parse(&format!("sha256:{}", "0".repeat(64))).unwrap();
    assert!(matches!(
        ValidatedExternalCaptureV1::verify(provenance, artifact, wrong),
        Err(CaptureV1Error::CaptureIdMismatch)
    ));
}
