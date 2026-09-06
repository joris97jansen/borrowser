use external_test_provenance::{Sha256Digest, sha256};

use super::super::baseline::{
    HistoricalAdvisoryPoint, HistoricalLogicalCase, HistoricalNote, HistoricalTrack,
    HistoricalVariant, HistoricalVariantKey, ValidatedHistoricalBaseline,
};
use super::{
    ADVISORY_CHANGE_CAPTURE, ADVISORY_CHANGE_EVALUATION_PRESENCE, ADVISORY_CHANGE_RESULT,
    AdvisoryTrendExtension, ConformanceTrendV1, PopulationTrend, TrendChangeKind,
    TrendCompatibilityField, TrendError, TrendPopulation, TrendPopulationCounts, TrendRecord,
    fingerprint,
};

pub(crate) fn compare_baselines_v1(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<ConformanceTrendV1, TrendError> {
    compatible(old, new)?;
    validate_track_reuse(&old.tracks, &new.tracks)?;
    let logical = compare_logical(old, new)?;
    let variants = compare_variants(old, new)?;
    let advisory = compare_advisory(old, new)?;
    let notes = compare_notes(old, new)?;
    let old_evaluated = advisory_evaluated_count(old)?;
    let new_evaluated = advisory_evaluated_count(new)?;
    let old_total = u64::try_from(old.points.len()).map_err(|_| TrendError::Arithmetic)?;
    let new_total = u64::try_from(new.points.len()).map_err(|_| TrendError::Arithmetic)?;
    Ok(ConformanceTrendV1 {
        old_baseline_sha256: sha256(old.exact_bytes()),
        new_baseline_sha256: sha256(new.exact_bytes()),
        inventory_scope: copy_string(&old.detail.inventory_scope)?,
        aggregate_contract: copy_string(&old.detail.aggregate_contract)?,
        named_lane: copy_string(&old.detail.named_lane)?,
        environment_assessment: copy_string(&old.detail.environment_assessment)?,
        old_source_set_sha256: old.detail.source_set_digest,
        new_source_set_sha256: new.detail.source_set_digest,
        old_evaluation_scope: fingerprint::evaluation_scope(old)?,
        new_evaluation_scope: fingerprint::evaluation_scope(new)?,
        old_advisory_total: old_total,
        old_advisory_evaluated: old_evaluated,
        old_advisory_unevaluated: old_total
            .checked_sub(old_evaluated)
            .ok_or(TrendError::Arithmetic)?,
        new_advisory_total: new_total,
        new_advisory_evaluated: new_evaluated,
        new_advisory_unevaluated: new_total
            .checked_sub(new_evaluated)
            .ok_or(TrendError::Arithmetic)?,
        populations: [logical, variants, advisory, notes],
    })
}

fn advisory_evaluated_count(baseline: &ValidatedHistoricalBaseline) -> Result<u64, TrendError> {
    u64::try_from(
        baseline
            .points
            .iter()
            .filter(|point| point.result.is_some())
            .count(),
    )
    .map_err(|_| TrendError::Arithmetic)
}

fn copy_string(value: &str) -> Result<String, TrendError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| TrendError::Allocation)?;
    owned.push_str(value);
    Ok(owned)
}

fn compatible(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<(), TrendError> {
    for (same, field) in [
        (
            old.detail.inventory_scope == new.detail.inventory_scope,
            TrendCompatibilityField::InventoryScope,
        ),
        (
            old.detail.aggregate_contract == new.detail.aggregate_contract,
            TrendCompatibilityField::AggregateContract,
        ),
        (
            old.detail.named_lane == new.detail.named_lane,
            TrendCompatibilityField::NamedLane,
        ),
        (
            old.detail.environment_assessment == new.detail.environment_assessment,
            TrendCompatibilityField::EnvironmentAssessment,
        ),
        (
            old.detail.population_identity_contract == new.detail.population_identity_contract,
            TrendCompatibilityField::ComponentVersions,
        ),
    ] {
        if !same {
            return Err(TrendError::Incompatible(field));
        }
    }
    Ok(())
}

fn validate_track_reuse(
    old: &[HistoricalTrack],
    new: &[HistoricalTrack],
) -> Result<(), TrendError> {
    let mut left = 0;
    let mut right = 0;
    while left < old.len() && right < new.len() {
        match old[left].id.cmp(&new[right].id) {
            std::cmp::Ordering::Less => left += 1,
            std::cmp::Ordering::Greater => right += 1,
            std::cmp::Ordering::Equal => {
                if old[left].canonical_record != new[right].canonical_record {
                    return Err(TrendError::Incompatible(
                        TrendCompatibilityField::AdvisoryTrackInvariant,
                    ));
                }
                left += 1;
                right += 1;
            }
        }
    }
    Ok(())
}

fn compare_logical(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<PopulationTrend, TrendError> {
    compare_sorted(
        TrendPopulation::LogicalCases,
        &old.detail.cases,
        &new.detail.cases,
        |value| value.test_id.as_bytes(),
        fingerprint::logical_key,
        fingerprint::logical,
        |_, _| None,
    )
}

#[derive(Clone, Copy)]
struct VariantRef<'a> {
    parent: &'a HistoricalLogicalCase,
    variant: &'a HistoricalVariant,
}

fn compare_variants(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<PopulationTrend, TrendError> {
    let old = flatten_variants(old)?;
    let new = flatten_variants(new)?;
    compare_sorted(
        TrendPopulation::ExecutionVariants,
        &old,
        &new,
        |value| &value.variant.key,
        |value| fingerprint::variant_key(&value.variant.key),
        |value| fingerprint::variant_fingerprint(value.parent, value.variant),
        |_, _| None,
    )
}

fn flatten_variants(
    baseline: &ValidatedHistoricalBaseline,
) -> Result<Vec<VariantRef<'_>>, TrendError> {
    let count = baseline
        .detail
        .cases
        .iter()
        .try_fold(0_usize, |total, case| {
            total.checked_add(case.variants.len())
        })
        .ok_or(TrendError::Arithmetic)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| TrendError::Allocation)?;
    for case in &baseline.detail.cases {
        for variant in &case.variants {
            result.push(VariantRef {
                parent: case,
                variant,
            });
        }
    }
    result.sort_unstable_by(|left, right| left.variant.key.cmp(&right.variant.key));
    if result
        .windows(2)
        .any(|pair| pair[0].variant.key == pair[1].variant.key)
    {
        return Err(TrendError::Framing);
    }
    Ok(result)
}

fn compare_advisory(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<PopulationTrend, TrendError> {
    compare_sorted_pair_fingerprint(
        TrendPopulation::AdvisoryComparisonPoints,
        &old.points,
        &new.points,
        |value| &value.key,
        fingerprint::advisory_key,
        |value| fingerprint::advisory(old, value),
        |value| fingerprint::advisory(new, value),
        advisory_extension,
    )
}

fn advisory_mask(old: &HistoricalAdvisoryPoint, new: &HistoricalAdvisoryPoint) -> u8 {
    let mut mask = 0;
    if old.capture_id != new.capture_id {
        mask |= ADVISORY_CHANGE_CAPTURE;
    }
    if old.result.is_some() != new.result.is_some() {
        mask |= ADVISORY_CHANGE_EVALUATION_PRESENCE;
    }
    if matches!((&old.result, &new.result), (Some(old), Some(new)) if old != new) {
        mask |= ADVISORY_CHANGE_RESULT;
    }
    mask
}

fn advisory_extension(
    old: Option<&HistoricalAdvisoryPoint>,
    new: Option<&HistoricalAdvisoryPoint>,
) -> Option<AdvisoryTrendExtension> {
    Some(AdvisoryTrendExtension {
        old_evaluated: old.map(|point| point.result.is_some()),
        new_evaluated: new.map(|point| point.result.is_some()),
        change_mask: match (old, new) {
            (Some(old), Some(new)) => advisory_mask(old, new),
            _ => 0,
        },
    })
}

fn compare_notes(
    old: &ValidatedHistoricalBaseline,
    new: &ValidatedHistoricalBaseline,
) -> Result<PopulationTrend, TrendError> {
    compare_sorted(
        TrendPopulation::BaselineNotes,
        &old.notes,
        &new.notes,
        |value| value.id.as_bytes(),
        fingerprint::note_key,
        fingerprint::note,
        |_, _| None,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "keeps the four merge policies explicit"
)]
fn compare_sorted<T, K: Ord + ?Sized>(
    population: TrendPopulation,
    old: &[T],
    new: &[T],
    key: impl Fn(&T) -> &K,
    encode_key: impl Fn(&T) -> Result<Vec<u8>, TrendError>,
    fingerprint: impl Fn(&T) -> Result<Sha256Digest, TrendError>,
    extension: impl Fn(Option<&T>, Option<&T>) -> Option<AdvisoryTrendExtension>,
) -> Result<PopulationTrend, TrendError> {
    let fingerprint = &fingerprint;
    compare_sorted_pair_fingerprint(
        population,
        old,
        new,
        key,
        encode_key,
        |value| fingerprint(value),
        |value| fingerprint(value),
        extension,
    )
}

#[allow(clippy::too_many_arguments)]
fn compare_sorted_pair_fingerprint<T, K: Ord + ?Sized>(
    population: TrendPopulation,
    old: &[T],
    new: &[T],
    key: impl Fn(&T) -> &K,
    encode_key: impl Fn(&T) -> Result<Vec<u8>, TrendError>,
    old_fingerprint: impl Fn(&T) -> Result<Sha256Digest, TrendError>,
    new_fingerprint: impl Fn(&T) -> Result<Sha256Digest, TrendError>,
    extension: impl Fn(Option<&T>, Option<&T>) -> Option<AdvisoryTrendExtension>,
) -> Result<PopulationTrend, TrendError> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(
            old.len()
                .checked_add(new.len())
                .ok_or(TrendError::Arithmetic)?,
        )
        .map_err(|_| TrendError::Allocation)?;
    let mut left = 0;
    let mut right = 0;
    while left < old.len() || right < new.len() {
        let ordering = match (old.get(left), new.get(right)) {
            (Some(a), Some(b)) => key(a).cmp(key(b)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => break,
        };
        let record = match ordering {
            std::cmp::Ordering::Less => {
                let value = &old[left];
                left += 1;
                TrendRecord {
                    kind: TrendChangeKind::Removed,
                    key: encode_key(value)?,
                    old_fingerprint: Some(old_fingerprint(value)?),
                    new_fingerprint: None,
                    advisory: extension(Some(value), None),
                }
            }
            std::cmp::Ordering::Greater => {
                let value = &new[right];
                right += 1;
                TrendRecord {
                    kind: TrendChangeKind::Added,
                    key: encode_key(value)?,
                    old_fingerprint: None,
                    new_fingerprint: Some(new_fingerprint(value)?),
                    advisory: extension(None, Some(value)),
                }
            }
            std::cmp::Ordering::Equal => {
                let old_value = &old[left];
                let new_value = &new[right];
                left += 1;
                right += 1;
                let old_hash = old_fingerprint(old_value)?;
                let new_hash = new_fingerprint(new_value)?;
                TrendRecord {
                    kind: if old_hash == new_hash {
                        TrendChangeKind::Unchanged
                    } else {
                        TrendChangeKind::Changed
                    },
                    key: encode_key(old_value)?,
                    old_fingerprint: Some(old_hash),
                    new_fingerprint: Some(new_hash),
                    advisory: extension(Some(old_value), Some(new_value)),
                }
            }
        };
        records.push(record);
    }
    let counts = counts(old.len(), new.len(), &records)?;
    Ok(PopulationTrend {
        population,
        counts,
        records,
    })
}

fn counts(
    old: usize,
    new: usize,
    records: &[TrendRecord],
) -> Result<TrendPopulationCounts, TrendError> {
    let mut counts = TrendPopulationCounts {
        old: u64::try_from(old).map_err(|_| TrendError::Arithmetic)?,
        new: u64::try_from(new).map_err(|_| TrendError::Arithmetic)?,
        ..TrendPopulationCounts::default()
    };
    for record in records {
        let field = match record.kind {
            TrendChangeKind::Added => &mut counts.added,
            TrendChangeKind::Removed => &mut counts.removed,
            TrendChangeKind::Unchanged => &mut counts.unchanged,
            TrendChangeKind::Changed => &mut counts.changed,
        };
        *field = field.checked_add(1).ok_or(TrendError::Arithmetic)?;
    }
    counts.reconcile()?;
    Ok(counts)
}

#[allow(dead_code)]
fn _key_type_assertion(_: &HistoricalVariantKey, _: &HistoricalNote) {}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use conformance_test_support::LanePolicyScope;
    use external_test_provenance::{
        ApplicabilityV1, ExternalArtifactFormatV1, ExternalCaptureIdClaim,
        ExternalCaptureProvenanceV1, ExternalCaptureProvenanceV1Input, ExternalIdentityV1,
        ExternalVersionV1, ImmutableRevision, NonApplicableReasonV1, ResourceNetworkPolicyV1,
        TargetParserInputContextV1, canonical_capture_id_preimage_v1,
        validate_historical_capture_identity_v1,
    };

    use super::*;
    use crate::aggregate::baseline::{
        HistoricalAdvisoryPointKey, HistoricalEvaluationScope, HistoricalVariantIdentity,
        decode_baseline_v1,
    };
    use crate::{
        AggregateExecutionRequest, build_baseline_v1, load_repository_external_advisory_evidence,
        run_repository_aggregate, seal_baseline_without_evaluation,
    };

    fn baseline() -> ValidatedHistoricalBaseline {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let run = run_repository_aggregate(
            root,
            AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap();
        let evidence = load_repository_external_advisory_evidence(root, &run).unwrap();
        let sealed = seal_baseline_without_evaluation(&run, &evidence).unwrap();
        decode_baseline_v1(&build_baseline_v1(&sealed).unwrap()).unwrap()
    }

    fn capture(version: &str) -> external_test_provenance::HistoricalCaptureIdentityV1 {
        let digest = sha256(b"recorded-artifact");
        let provenance =
            ExternalCaptureProvenanceV1::try_from_input(ExternalCaptureProvenanceV1Input {
                engine_product: ExternalIdentityV1::parse("engine").unwrap(),
                engine_version: ExternalVersionV1::parse(version).unwrap(),
                engine_build_revision: None,
                platform_os_family: ExternalIdentityV1::parse("os").unwrap(),
                platform_os_version: ExternalVersionV1::parse("1").unwrap(),
                architecture: ExternalIdentityV1::parse("arch").unwrap(),
                viewport: ApplicabilityV1::NotApplicable(
                    NonApplicableReasonV1::parse("not-used").unwrap(),
                ),
                device_scale: ApplicabilityV1::NotApplicable(
                    NonApplicableReasonV1::parse("not-used").unwrap(),
                ),
                controlled_fonts: ApplicabilityV1::NotApplicable(
                    NonApplicableReasonV1::parse("font-independent").unwrap(),
                ),
                resource_network_policy: ResourceNetworkPolicyV1::Offline,
                pinned_resources: vec![],
                fixture_source_project: ExternalIdentityV1::parse("fixture").unwrap(),
                fixture_immutable_revision: ImmutableRevision::parse("revision").unwrap(),
                fixture_content_sha256: digest,
                capture_mechanism: ExternalIdentityV1::parse("tool").unwrap(),
                capture_mechanism_version: ExternalVersionV1::parse("1").unwrap(),
                capture_algorithm: ExternalIdentityV1::parse("algorithm").unwrap(),
                capture_algorithm_version: ExternalVersionV1::parse("1").unwrap(),
                capture_algorithm_source_sha256: digest,
                capture_configuration_sha256: digest,
                invocation_arguments: vec![],
                artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
                artifact_utf8_byte_length: 17,
                artifact_sha256: digest,
                target_parser_input_context:
                    TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
                collection_policy: ExternalIdentityV1::parse("stable").unwrap(),
                collection_policy_version: ExternalVersionV1::parse("1").unwrap(),
            })
            .unwrap();
        let preimage = canonical_capture_id_preimage_v1(&provenance).unwrap();
        validate_historical_capture_identity_v1(
            &preimage,
            ExternalCaptureIdClaim::from_sha256(sha256(&preimage)),
        )
        .unwrap()
    }

    fn advisory_baseline(version: &str, result: Option<Vec<u8>>) -> ValidatedHistoricalBaseline {
        let mut baseline = baseline();
        let capture = capture(version);
        let variant = baseline
            .detail
            .cases
            .iter()
            .flat_map(|case| &case.variants)
            .find(|variant| {
                variant.key.observation == "dom-tree"
                    && matches!(variant.key.variant, HistoricalVariantIdentity::Singleton)
            })
            .unwrap()
            .key
            .clone();
        let key = HistoricalAdvisoryPointKey {
            variant,
            comparable: "web-observable-dom-tree-v1".to_owned(),
            track_id: "track".to_owned(),
        };
        let capture_id = capture.id().as_sha256();
        let mut point_record = b"point-record".to_vec();
        point_record.extend_from_slice(capture_id.as_bytes());
        baseline.captures.push(capture);
        baseline.tracks.push(HistoricalTrack {
            id: "track".to_owned(),
            invariant: vec![],
            canonical_record: b"track-invariant".to_vec(),
        });
        baseline.points.push(HistoricalAdvisoryPoint {
            key,
            capture_id,
            canonical_record: point_record,
            result,
        });
        baseline
    }

    fn equivalent_result() -> Vec<u8> {
        let mut result = Vec::from(10_u64.to_be_bytes());
        result.extend_from_slice(b"equivalent");
        result
    }

    fn labeled_result(label: &str) -> Vec<u8> {
        let mut result = Vec::from((label.len() as u64).to_be_bytes());
        result.extend_from_slice(label.as_bytes());
        result
    }

    fn advisory_record(
        old: &ValidatedHistoricalBaseline,
        new: &ValidatedHistoricalBaseline,
    ) -> TrendRecord {
        compare_baselines_v1(old, new)
            .unwrap()
            .population(TrendPopulation::AdvisoryComparisonPoints)
            .records()[0]
            .clone()
    }

    #[test]
    fn unchanged_populations_reconcile_exactly() {
        let old = baseline();
        let new = baseline();
        let trend = compare_baselines_v1(&old, &new).unwrap();
        for population in [
            TrendPopulation::LogicalCases,
            TrendPopulation::ExecutionVariants,
            TrendPopulation::AdvisoryComparisonPoints,
            TrendPopulation::BaselineNotes,
        ] {
            let counts = trend.population(population).counts();
            assert_eq!(counts.added, 0);
            assert_eq!(counts.removed, 0);
            assert_eq!(counts.changed, 0);
            assert_eq!(counts.old, counts.unchanged);
            assert_eq!(counts.new, counts.unchanged);
            counts.reconcile().unwrap();
        }
    }

    #[test]
    fn logical_membership_and_same_test_id_member_drift_are_distinct() {
        let old = baseline();
        let mut new = baseline();
        new.detail.cases.remove(0);
        let removed = compare_baselines_v1(&old, &new).unwrap();
        assert_eq!(
            removed
                .population(TrendPopulation::LogicalCases)
                .counts()
                .removed,
            1
        );
        let added = compare_baselines_v1(&new, &old).unwrap();
        assert_eq!(
            added
                .population(TrendPopulation::LogicalCases)
                .counts()
                .added,
            1
        );

        let mut changed = baseline();
        let changed_case = changed
            .detail
            .cases
            .iter_mut()
            .find(|case| !case.variants.is_empty())
            .unwrap();
        changed_case.member_digest = sha256(b"changed-member");
        changed_case
            .canonical_metadata_prefix
            .extend_from_slice(b"changed");
        let inherited_variant_count = changed_case.variants.len() as u64;
        let trend = compare_baselines_v1(&old, &changed).unwrap();
        let logical = trend.population(TrendPopulation::LogicalCases);
        assert_eq!(logical.counts().changed, 1);
        assert_eq!(logical.counts().added, 0);
        assert_eq!(logical.counts().removed, 0);
        assert!(
            logical
                .records()
                .iter()
                .any(|record| record.kind() == TrendChangeKind::Changed)
        );
        assert!(
            trend
                .population(TrendPopulation::ExecutionVariants)
                .counts()
                .changed
                >= inherited_variant_count
        );
    }

    #[test]
    fn variant_state_and_key_changes_have_frozen_membership_behavior() {
        let old = baseline();
        let mut changed = baseline();
        let case = changed
            .detail
            .cases
            .iter_mut()
            .find(|case| !case.variants.is_empty())
            .unwrap();
        case.variants[0]
            .canonical_record
            .extend_from_slice(b"changed");
        let trend = compare_baselines_v1(&old, &changed).unwrap();
        assert_eq!(
            trend
                .population(TrendPopulation::ExecutionVariants)
                .counts()
                .changed,
            1
        );

        let mut rekeyed = baseline();
        let case = rekeyed
            .detail
            .cases
            .iter_mut()
            .find(|case| !case.variants.is_empty())
            .unwrap();
        case.variants[0].key.observation.push_str("-new-key");
        let trend = compare_baselines_v1(&old, &rekeyed).unwrap();
        let counts = trend
            .population(TrendPopulation::ExecutionVariants)
            .counts();
        assert_eq!(counts.added, 1);
        assert_eq!(counts.removed, 1);
    }

    #[test]
    fn note_changes_are_isolated_and_compatibility_fails_closed() {
        let mut old = baseline();
        let mut new = baseline();
        old.notes.push(HistoricalNote {
            id: "stable-note".to_owned(),
            canonical_record: b"old-note".to_vec(),
        });
        new.notes.push(HistoricalNote {
            id: "stable-note".to_owned(),
            canonical_record: b"new-note".to_vec(),
        });
        let trend = compare_baselines_v1(&old, &new).unwrap();
        assert_eq!(
            trend
                .population(TrendPopulation::BaselineNotes)
                .counts()
                .changed,
            1
        );
        assert_eq!(
            trend
                .population(TrendPopulation::LogicalCases)
                .counts()
                .changed,
            0
        );
        assert_eq!(
            trend
                .population(TrendPopulation::ExecutionVariants)
                .counts()
                .changed,
            0
        );

        new.detail.named_lane = "local-extended".to_owned();
        assert!(matches!(
            compare_baselines_v1(&old, &new),
            Err(TrendError::Incompatible(TrendCompatibilityField::NamedLane))
        ));
    }

    #[test]
    fn every_compatibility_domain_axis_is_checked_independently() {
        let old = baseline();
        for (field, mutate) in [
            (TrendCompatibilityField::InventoryScope, 0_u8),
            (TrendCompatibilityField::AggregateContract, 1_u8),
            (TrendCompatibilityField::EnvironmentAssessment, 2_u8),
            (TrendCompatibilityField::ComponentVersions, 3_u8),
        ] {
            let mut new = baseline();
            match mutate {
                0 => new.detail.inventory_scope.push_str("-other"),
                1 => new.detail.aggregate_contract.push_str("-other"),
                2 => new.detail.environment_assessment.push_str("-other"),
                3 => new.detail.population_identity_contract.push_str("-other"),
                _ => unreachable!(),
            }
            assert!(matches!(
                compare_baselines_v1(&old, &new),
                Err(TrendError::Incompatible(actual)) if actual == field
            ));
        }
    }

    #[test]
    fn source_set_digest_is_context_not_a_per_item_fingerprint_input() {
        let old = baseline();
        let mut new = baseline();
        new.detail.source_set_digest = sha256(b"different-valid-source-set-context");
        let trend = compare_baselines_v1(&old, &new).unwrap();
        assert_eq!(
            trend
                .population(TrendPopulation::LogicalCases)
                .counts()
                .changed,
            0
        );
        assert_eq!(
            trend
                .population(TrendPopulation::ExecutionVariants)
                .counts()
                .changed,
            0
        );
    }

    #[test]
    fn advisory_track_id_reuse_requires_identical_invariant_record() {
        let mut old = baseline();
        let mut new = baseline();
        old.tracks.push(HistoricalTrack {
            id: "track".to_owned(),
            invariant: vec![],
            canonical_record: b"old".to_vec(),
        });
        new.tracks.push(HistoricalTrack {
            id: "track".to_owned(),
            invariant: vec![],
            canonical_record: b"new".to_vec(),
        });
        assert!(matches!(
            compare_baselines_v1(&old, &new),
            Err(TrendError::Incompatible(
                TrendCompatibilityField::AdvisoryTrackInvariant
            ))
        ));
    }

    #[test]
    fn advisory_membership_evaluation_and_capture_drift_stay_advisory_only() {
        let empty = baseline();
        let unevaluated = advisory_baseline("1", None);
        let evaluated = advisory_baseline("1", Some(equivalent_result()));
        let capture_drift = advisory_baseline("2", Some(equivalent_result()));

        let added = compare_baselines_v1(&empty, &unevaluated).unwrap();
        assert_eq!(
            added
                .population(TrendPopulation::AdvisoryComparisonPoints)
                .counts()
                .added,
            1
        );
        let removed = compare_baselines_v1(&unevaluated, &empty).unwrap();
        assert_eq!(
            removed
                .population(TrendPopulation::AdvisoryComparisonPoints)
                .counts()
                .removed,
            1
        );

        for (old, new) in [
            (&unevaluated, &evaluated),
            (&evaluated, &unevaluated),
            (&evaluated, &capture_drift),
        ] {
            let trend = compare_baselines_v1(old, new).unwrap();
            assert_eq!(
                trend
                    .population(TrendPopulation::AdvisoryComparisonPoints)
                    .counts()
                    .changed,
                1
            );
            assert_eq!(
                trend
                    .population(TrendPopulation::LogicalCases)
                    .counts()
                    .changed,
                0
            );
            assert_eq!(
                trend
                    .population(TrendPopulation::ExecutionVariants)
                    .counts()
                    .changed,
                0
            );
        }
        let transition = compare_baselines_v1(&unevaluated, &evaluated).unwrap();
        let record = &transition
            .population(TrendPopulation::AdvisoryComparisonPoints)
            .records()[0];
        let extension = record.advisory_extension().unwrap();
        assert_eq!(extension.old_evaluated(), Some(false));
        assert_eq!(extension.new_evaluated(), Some(true));
        assert_ne!(
            extension.change_mask() & ADVISORY_CHANGE_EVALUATION_PRESENCE,
            0
        );
    }

    #[test]
    fn advisory_result_mask_distinguishes_presence_from_two_sided_result_drift() {
        let none = advisory_baseline("1", None);
        let equivalent = advisory_baseline("1", Some(equivalent_result()));
        let different = advisory_baseline("1", Some(labeled_result("different")));
        let failure_a = advisory_baseline("1", Some(labeled_result("failure-a")));
        let failure_b = advisory_baseline("1", Some(labeled_result("failure-b")));

        for (old, new) in [(&none, &equivalent), (&equivalent, &none)] {
            let extension = advisory_record(old, new).advisory_extension().unwrap();
            assert_eq!(extension.change_mask(), ADVISORY_CHANGE_EVALUATION_PRESENCE);
        }
        for (old, new) in [
            (&equivalent, &different),
            (&equivalent, &failure_a),
            (&failure_a, &failure_b),
        ] {
            let extension = advisory_record(old, new).advisory_extension().unwrap();
            assert_eq!(extension.change_mask(), ADVISORY_CHANGE_RESULT);
        }
        let unchanged = advisory_record(&equivalent, &equivalent);
        assert_eq!(unchanged.kind(), TrendChangeKind::Unchanged);
        assert_eq!(unchanged.advisory_extension().unwrap().change_mask(), 0);
    }

    #[test]
    fn evaluation_scope_alone_does_not_fabricate_membership_change() {
        let old = baseline();
        let mut new = baseline();
        let variant = new.detail.cases[0]
            .variants
            .first()
            .map(|variant| variant.key.clone())
            .unwrap_or_else(|| {
                new.detail
                    .cases
                    .iter()
                    .flat_map(|case| &case.variants)
                    .next()
                    .unwrap()
                    .key
                    .clone()
            });
        new.evaluation = HistoricalEvaluationScope::Selected {
            variant,
            comparable: "web-observable-dom-tree-v1".to_owned(),
        };
        let trend = compare_baselines_v1(&old, &new).unwrap();
        assert_eq!(
            trend
                .population(TrendPopulation::AdvisoryComparisonPoints)
                .counts(),
            TrendPopulationCounts::default()
        );
    }
}
