#![cfg(feature = "aggregate")]

use std::fs;
use std::path::Path;

use conformance_runner::{
    AggregateExecutionRequest, BASELINE_FORMAT_V1, BaselineFileInputV1, TREND_FORMAT_V1,
    TrendError, TrendInputSide, TrendPopulation, build_aggregate_detail_v1, build_baseline_v1,
    build_trend_v1, compare_baseline_files_v1, load_repository_external_advisory_evidence,
    run_repository_aggregate, seal_baseline_without_evaluation,
};
use conformance_test_support::LanePolicyScope;
use external_test_provenance::{Sha256Digest, sha256};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
}

fn baseline_bytes() -> Vec<u8> {
    let root = repository_root();
    let run = run_repository_aggregate(
        root,
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .unwrap();
    let evidence = load_repository_external_advisory_evidence(root, &run).unwrap();
    let sealed = seal_baseline_without_evaluation(&run, &evidence).unwrap();
    build_baseline_v1(&sealed).unwrap()
}

fn trend_for(bytes: &[u8]) -> conformance_runner::ConformanceTrendV1 {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("old.baseline"), bytes).unwrap();
    fs::write(root.path().join("new.baseline"), bytes).unwrap();
    let claim = sha256(bytes);
    let input = |name| BaselineFileInputV1 {
        root: root.path(),
        relative_path: Path::new(name),
        expected_sha256: claim,
    };
    compare_baseline_files_v1(input("old.baseline"), input("new.baseline")).unwrap()
}

#[test]
fn baseline_v1_has_exact_golden_identity_and_embeds_authoritative_detail_bytes() {
    let bytes = baseline_bytes();
    let independent = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-baseline-v1/empty.bin"
    ));
    assert_eq!(bytes, independent);
    assert_eq!(bytes.len(), 42_198);
    assert_eq!(
        sha256(&bytes).to_hex(),
        "7f0f654304d92d444ba85520e4db49199266d015a3c829bd57004447d4c0926a"
    );
    assert_eq!(
        &bytes[..BASELINE_FORMAT_V1.len()],
        BASELINE_FORMAT_V1.as_bytes()
    );
    let mut offset = BASELINE_FORMAT_V1.len();
    assert_eq!(bytes[offset], 0);
    offset += 1;
    assert_eq!(u16_at(&bytes, &mut offset), 7);
    assert_eq!(u16_at(&bytes, &mut offset), 1);
    skip_bytes(&bytes, &mut offset);
    assert_eq!(u16_at(&bytes, &mut offset), 2);
    let detail = take_bytes(&bytes, &mut offset);
    let root = repository_root();
    let run = run_repository_aggregate(
        root,
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .unwrap();
    assert_eq!(detail, build_aggregate_detail_v1(&run).unwrap());
    for tag in 3..=7 {
        assert_eq!(u16_at(&bytes, &mut offset), tag);
        skip_bytes(&bytes, &mut offset);
    }
    assert_eq!(offset, bytes.len(), "the envelope has no trailing content");
}

#[test]
fn trend_v1_has_exact_golden_identity_and_canonical_round_trip() {
    let baseline = baseline_bytes();
    let trend = trend_for(&baseline);
    let bytes = build_trend_v1(&trend).unwrap();
    let independent = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-trend-v1/unchanged.bin"
    ));
    assert_eq!(bytes, independent);
    assert_eq!(bytes.len(), 10_608);
    assert_eq!(
        sha256(&bytes).to_hex(),
        "dfb48cc741f0a00d643df563e3e034efe2546cb2c4235f4f3746794ee8396702"
    );
    assert_eq!(&bytes[..TREND_FORMAT_V1.len()], TREND_FORMAT_V1.as_bytes());
    for population in [
        TrendPopulation::LogicalCases,
        TrendPopulation::ExecutionVariants,
        TrendPopulation::AdvisoryComparisonPoints,
        TrendPopulation::BaselineNotes,
    ] {
        trend.population(population).counts().reconcile().unwrap();
    }
}

#[test]
fn verified_file_api_checks_exact_retained_bytes_before_decoding() {
    let root = tempfile::tempdir().unwrap();
    let bytes = baseline_bytes();
    fs::write(root.path().join("old.baseline"), &bytes).unwrap();
    fs::write(root.path().join("new.baseline"), &bytes).unwrap();
    let claim = sha256(&bytes);
    let input = |name| BaselineFileInputV1 {
        root: root.path(),
        relative_path: Path::new(name),
        expected_sha256: claim,
    };
    let trend = compare_baseline_files_v1(input("old.baseline"), input("new.baseline")).unwrap();
    assert_eq!(trend.old_baseline_sha256(), claim);
    assert_eq!(trend.new_baseline_sha256(), claim);

    fs::write(root.path().join("old.baseline"), b"not a baseline").unwrap();
    let wrong_claim = Sha256Digest::from_bytes([0; 32]);
    let error = compare_baseline_files_v1(
        BaselineFileInputV1 {
            root: root.path(),
            relative_path: Path::new("old.baseline"),
            expected_sha256: wrong_claim,
        },
        input("new.baseline"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TrendError::DigestMismatch {
            input: TrendInputSide::Old
        }
    ));
}

fn u16_at(bytes: &[u8], offset: &mut usize) -> u16 {
    let end = *offset + 2;
    let value = u16::from_be_bytes(bytes[*offset..end].try_into().unwrap());
    *offset = end;
    value
}

fn take_bytes<'a>(bytes: &'a [u8], offset: &mut usize) -> &'a [u8] {
    let length_end = *offset + 8;
    let length = u64::from_be_bytes(bytes[*offset..length_end].try_into().unwrap()) as usize;
    let start = length_end;
    let end = start + length;
    *offset = end;
    &bytes[start..end]
}

fn skip_bytes(bytes: &[u8], offset: &mut usize) {
    let _ = take_bytes(bytes, offset);
}

#[path = "support/mod.rs"]
mod support;

#[test]
fn ag9d_lane_excluded_baseline_round_trip() {
    use conformance_runner::*;
    let root = support::repository();
    let metadata = root.path().join("tests/conformance/expected-results.toml");
    let text = std::fs::read_to_string(&metadata).unwrap();
    let start = text.find("id = \"dom-tree-basic-document\"").unwrap();
    let updated = text[start..].replacen(
        "lane_exclusions = []",
        "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Explicit test exclusion.\" }]",
        1,
    );
    std::fs::write(metadata, format!("{}{updated}", &text[..start])).unwrap();

    let run = run_repository_aggregate(
        root.path(),
        AggregateExecutionRequest {
            lane: conformance_test_support::LanePolicyScope::NormalCi,
        },
    )
    .unwrap();
    let variant = &run
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "dom-tree-basic-document")
        .unwrap()
        .variants[0];
    assert!(matches!(variant.selection, LaneSelection::Excluded { .. }));
    assert_eq!(variant.policy, DerivedPolicyResult::NotRun);
    let evidence = load_repository_external_advisory_evidence(root.path(), &run).unwrap();
    let sealed = seal_baseline_without_evaluation(&run, &evidence).unwrap();
    let bytes = build_baseline_v1(&sealed).unwrap();
    std::fs::write(root.path().join("excluded.bin"), &bytes).unwrap();
    let input = BaselineFileInputV1 {
        root: root.path(),
        relative_path: Path::new("excluded.bin"),
        expected_sha256: external_test_provenance::sha256(&bytes),
    };
    compare_baseline_files_v1(input, input).expect(
        "AG9d must accept the live lane-excluded NotRun policy without changing lane semantics",
    );
}
