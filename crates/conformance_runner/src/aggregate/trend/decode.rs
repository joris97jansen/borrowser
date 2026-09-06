use external_test_provenance::Sha256Digest;

use super::super::baseline::{
    HistoricalAdvisoryPointKey, HistoricalEvaluationScope, HistoricalVariantKey,
    decode_historical_variant_key, valid_semantic_id,
};
use super::super::binary_wire::Reader;
use super::{
    ADVISORY_CHANGE_CAPTURE, ADVISORY_CHANGE_EVALUATION_PRESENCE, ADVISORY_CHANGE_RESULT,
    AdvisoryTrendExtension, ConformanceTrendV1, PopulationTrend, TREND_FORMAT_V1,
    TREND_MAX_BYTES_V1, TREND_VERSIONS_V1, TrendChangeKind, TrendError, TrendPopulation,
    TrendPopulationCounts, TrendRecord,
};

pub(crate) fn decode_trend_v1(bytes: &[u8]) -> Result<ConformanceTrendV1, TrendError> {
    if bytes.len() > TREND_MAX_BYTES_V1 {
        return Err(TrendError::TooLarge);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(TREND_FORMAT_V1.len())? != TREND_FORMAT_V1.as_bytes()
        || reader.u8()? != 0
        || reader.u16()? != 7
    {
        return Err(TrendError::Framing);
    }
    decode_versions(section(&mut reader, 1)?)?;
    let mut context = Reader::new(section(&mut reader, 2)?);
    let inventory_scope = context.owned_string()?;
    let aggregate_contract = context.owned_string()?;
    let named_lane = context.owned_string()?;
    let environment_assessment = context.owned_string()?;
    let old_baseline_sha256 = digest(&mut context)?;
    let new_baseline_sha256 = digest(&mut context)?;
    let old_source_set_sha256 = digest(&mut context)?;
    let new_source_set_sha256 = digest(&mut context)?;
    context.finish()?;
    if inventory_scope != "static-html-css-no-js"
        || aggregate_contract != "borrowser-conformance-aggregate-granularity-v1"
        || !matches!(
            named_lane.as_str(),
            "normal-ci" | "local-extended" | "scheduled-extended" | "manual-extended"
        )
        || environment_assessment != "ag9-empty-assessment-v1"
    {
        return Err(TrendError::Framing);
    }
    let mut evaluation = Reader::new(section(&mut reader, 3)?);
    let (old_evaluation_scope, old_scope) = validate_scope(evaluation.bytes()?)?;
    let (new_evaluation_scope, new_scope) = validate_scope(evaluation.bytes()?)?;
    let old_advisory_total = evaluation.u64()?;
    let old_advisory_evaluated = evaluation.u64()?;
    let old_advisory_unevaluated = evaluation.u64()?;
    let new_advisory_total = evaluation.u64()?;
    let new_advisory_evaluated = evaluation.u64()?;
    let new_advisory_unevaluated = evaluation.u64()?;
    evaluation.finish()?;
    let populations = [
        decode_population(section(&mut reader, 4)?, TrendPopulation::LogicalCases)?,
        decode_population(section(&mut reader, 5)?, TrendPopulation::ExecutionVariants)?,
        decode_population(
            section(&mut reader, 6)?,
            TrendPopulation::AdvisoryComparisonPoints,
        )?,
        decode_population(section(&mut reader, 7)?, TrendPopulation::BaselineNotes)?,
    ];
    reader.finish()?;
    validate_evaluation_summary(
        &populations[2],
        [
            old_advisory_total,
            old_advisory_evaluated,
            old_advisory_unevaluated,
            new_advisory_total,
            new_advisory_evaluated,
            new_advisory_unevaluated,
        ],
        &old_scope,
        &new_scope,
    )?;
    Ok(ConformanceTrendV1 {
        old_baseline_sha256,
        new_baseline_sha256,
        inventory_scope,
        aggregate_contract,
        named_lane,
        environment_assessment,
        old_source_set_sha256,
        new_source_set_sha256,
        old_evaluation_scope,
        new_evaluation_scope,
        old_advisory_total,
        old_advisory_evaluated,
        old_advisory_unevaluated,
        new_advisory_total,
        new_advisory_evaluated,
        new_advisory_unevaluated,
        populations,
    })
}

fn decode_versions(bytes: &[u8]) -> Result<(), TrendError> {
    let mut reader = Reader::new(bytes);
    for expected in TREND_VERSIONS_V1 {
        if reader.string()? != expected {
            return Err(TrendError::Framing);
        }
    }
    reader.finish()?;
    Ok(())
}

fn validate_scope(bytes: &[u8]) -> Result<(Vec<u8>, HistoricalEvaluationScope), TrendError> {
    let mut reader = Reader::new(bytes);
    let scope = match reader.string()? {
        "none" if reader.string()? == "not-requested" => HistoricalEvaluationScope::None,
        "selected-variant-only" => {
            let variant =
                decode_historical_variant_key(&mut reader).map_err(|_| TrendError::Framing)?;
            let comparable = reader.owned_string()?;
            if comparable != "web-observable-dom-tree-v1" || reader.string()? != "completed" {
                return Err(TrendError::Framing);
            }
            HistoricalEvaluationScope::Selected {
                variant,
                comparable,
            }
        }
        "all-declared" if reader.string()? == "completed" => HistoricalEvaluationScope::AllDeclared,
        _ => return Err(TrendError::Framing),
    };
    reader.finish()?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| TrendError::Allocation)?;
    owned.extend_from_slice(bytes);
    Ok((owned, scope))
}

fn decode_population(
    bytes: &[u8],
    population: TrendPopulation,
) -> Result<PopulationTrend, TrendError> {
    let mut reader = Reader::new(bytes);
    let counts = TrendPopulationCounts {
        old: reader.u64()?,
        new: reader.u64()?,
        added: reader.u64()?,
        removed: reader.u64()?,
        unchanged: reader.u64()?,
        changed: reader.u64()?,
    };
    counts.reconcile()?;
    let count = usize::try_from(reader.u32()?).map_err(|_| TrendError::Arithmetic)?;
    let expected = counts
        .added
        .checked_add(counts.removed)
        .and_then(|value| value.checked_add(counts.unchanged))
        .and_then(|value| value.checked_add(counts.changed))
        .ok_or(TrendError::Arithmetic)?;
    if u64::try_from(count).map_err(|_| TrendError::Arithmetic)? != expected {
        return Err(TrendError::Reconciliation);
    }
    let mut records = Vec::new();
    let mut previous_key = None;
    for _ in 0..count {
        records.try_reserve(1).map_err(|_| TrendError::Allocation)?;
        let (record, semantic_key) = decode_record(reader.bytes()?, population)?;
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &semantic_key)
        {
            return Err(TrendError::Framing);
        }
        previous_key = Some(semantic_key);
        records.push(record);
    }
    reader.finish()?;
    if counts_from_records(counts.old, counts.new, &records)? != counts {
        return Err(TrendError::Reconciliation);
    }
    Ok(PopulationTrend {
        population,
        counts,
        records,
    })
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SemanticTrendKey {
    Logical(String),
    Variant(HistoricalVariantKey),
    Advisory(HistoricalAdvisoryPointKey),
    Note(String),
}

fn decode_record(
    bytes: &[u8],
    population: TrendPopulation,
) -> Result<(TrendRecord, SemanticTrendKey), TrendError> {
    let mut reader = Reader::new(bytes);
    let kind = match reader.string()? {
        "added" => TrendChangeKind::Added,
        "removed" => TrendChangeKind::Removed,
        "unchanged" => TrendChangeKind::Unchanged,
        "changed" => TrendChangeKind::Changed,
        _ => return Err(TrendError::Framing),
    };
    let key = reader.owned_bytes()?;
    let semantic_key = validate_key(&key, population)?;
    let old_fingerprint = optional_digest(&mut reader)?;
    let new_fingerprint = optional_digest(&mut reader)?;
    let advisory = if population == TrendPopulation::AdvisoryComparisonPoints {
        Some(AdvisoryTrendExtension {
            old_evaluated: optional_bool(&mut reader)?,
            new_evaluated: optional_bool(&mut reader)?,
            change_mask: reader.u8()?,
        })
    } else {
        None
    };
    reader.finish()?;
    match kind {
        TrendChangeKind::Added if old_fingerprint.is_none() && new_fingerprint.is_some() => {}
        TrendChangeKind::Removed if old_fingerprint.is_some() && new_fingerprint.is_none() => {}
        TrendChangeKind::Unchanged
            if old_fingerprint.is_some() && old_fingerprint == new_fingerprint => {}
        TrendChangeKind::Changed
            if old_fingerprint.is_some()
                && new_fingerprint.is_some()
                && old_fingerprint != new_fingerprint => {}
        _ => return Err(TrendError::Framing),
    }
    if let Some(advisory) = advisory {
        if matches!(kind, TrendChangeKind::Added) != advisory.old_evaluated.is_none()
            || matches!(kind, TrendChangeKind::Removed) != advisory.new_evaluated.is_none()
        {
            return Err(TrendError::Framing);
        }
        if advisory.old_evaluated.is_some()
            && advisory.new_evaluated.is_some()
            && ((advisory.old_evaluated != advisory.new_evaluated)
                != (advisory.change_mask & ADVISORY_CHANGE_EVALUATION_PRESENCE != 0))
        {
            return Err(TrendError::Framing);
        }
        let allowed =
            ADVISORY_CHANGE_CAPTURE | ADVISORY_CHANGE_EVALUATION_PRESENCE | ADVISORY_CHANGE_RESULT;
        if advisory.change_mask & !allowed != 0
            || advisory.change_mask & ADVISORY_CHANGE_RESULT != 0
                && (advisory.old_evaluated != Some(true) || advisory.new_evaluated != Some(true))
            || (kind == TrendChangeKind::Changed && advisory.change_mask == 0)
            || (kind != TrendChangeKind::Changed && advisory.change_mask != 0)
        {
            return Err(TrendError::Framing);
        }
    }
    Ok((
        TrendRecord {
            kind,
            key,
            old_fingerprint,
            new_fingerprint,
            advisory,
        },
        semantic_key,
    ))
}

fn validate_key(bytes: &[u8], population: TrendPopulation) -> Result<SemanticTrendKey, TrendError> {
    let mut reader = Reader::new(bytes);
    let key = match population {
        TrendPopulation::LogicalCases => {
            let value = reader.owned_string()?;
            if !conformance_test_support::TestId::is_valid(&value) {
                return Err(TrendError::Framing);
            }
            SemanticTrendKey::Logical(value)
        }
        TrendPopulation::ExecutionVariants => SemanticTrendKey::Variant(
            decode_historical_variant_key(&mut reader).map_err(|_| TrendError::Framing)?,
        ),
        TrendPopulation::AdvisoryComparisonPoints => {
            let variant =
                decode_historical_variant_key(&mut reader).map_err(|_| TrendError::Framing)?;
            let comparable = reader.owned_string()?;
            let track_id = reader.owned_string()?;
            if comparable != "web-observable-dom-tree-v1" || !valid_semantic_id(&track_id) {
                return Err(TrendError::Framing);
            }
            SemanticTrendKey::Advisory(HistoricalAdvisoryPointKey {
                variant,
                comparable,
                track_id,
            })
        }
        TrendPopulation::BaselineNotes => {
            let value = reader.owned_string()?;
            if !valid_semantic_id(&value) {
                return Err(TrendError::Framing);
            }
            SemanticTrendKey::Note(value)
        }
    };
    reader.finish()?;
    Ok(key)
}

fn validate_evaluation_summary(
    advisory: &PopulationTrend,
    counts: [u64; 6],
    old_scope: &HistoricalEvaluationScope,
    new_scope: &HistoricalEvaluationScope,
) -> Result<(), TrendError> {
    let [
        old_total,
        old_evaluated,
        old_unevaluated,
        new_total,
        new_evaluated,
        new_unevaluated,
    ] = counts;
    if old_total != advisory.counts.old
        || new_total != advisory.counts.new
        || old_evaluated.checked_add(old_unevaluated) != Some(old_total)
        || new_evaluated.checked_add(new_unevaluated) != Some(new_total)
    {
        return Err(TrendError::Reconciliation);
    }
    let mut derived_old = 0_u64;
    let mut derived_new = 0_u64;
    for record in &advisory.records {
        let extension = record.advisory.ok_or(TrendError::Reconciliation)?;
        let key = match validate_key(&record.key, TrendPopulation::AdvisoryComparisonPoints)? {
            SemanticTrendKey::Advisory(key) => key,
            _ => return Err(TrendError::Framing),
        };
        if extension
            .old_evaluated
            .is_some_and(|evaluated| evaluated != expected_evaluated(old_scope, &key))
            || extension
                .new_evaluated
                .is_some_and(|evaluated| evaluated != expected_evaluated(new_scope, &key))
        {
            return Err(TrendError::Reconciliation);
        }
        if extension.old_evaluated == Some(true) {
            derived_old = derived_old.checked_add(1).ok_or(TrendError::Arithmetic)?;
        }
        if extension.new_evaluated == Some(true) {
            derived_new = derived_new.checked_add(1).ok_or(TrendError::Arithmetic)?;
        }
    }
    if derived_old != old_evaluated || derived_new != new_evaluated {
        return Err(TrendError::Reconciliation);
    }
    Ok(())
}

fn expected_evaluated(scope: &HistoricalEvaluationScope, key: &HistoricalAdvisoryPointKey) -> bool {
    match scope {
        HistoricalEvaluationScope::None => false,
        HistoricalEvaluationScope::Selected {
            variant,
            comparable,
        } => variant == &key.variant && comparable == &key.comparable,
        HistoricalEvaluationScope::AllDeclared => true,
    }
}

fn optional_digest(reader: &mut Reader<'_>) -> Result<Option<Sha256Digest>, TrendError> {
    reader
        .optional()?
        .map(|bytes| {
            Ok(Sha256Digest::from_bytes(
                bytes.try_into().map_err(|_| TrendError::Framing)?,
            ))
        })
        .transpose()
}

fn optional_bool(reader: &mut Reader<'_>) -> Result<Option<bool>, TrendError> {
    reader
        .optional()?
        .map(|bytes| match bytes {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(TrendError::Framing),
        })
        .transpose()
}

fn digest(reader: &mut Reader<'_>) -> Result<Sha256Digest, TrendError> {
    Ok(Sha256Digest::from_bytes(
        reader
            .take(32)?
            .try_into()
            .map_err(|_| TrendError::Framing)?,
    ))
}

fn section<'a>(reader: &mut Reader<'a>, expected: u16) -> Result<&'a [u8], TrendError> {
    if reader.u16()? != expected {
        return Err(TrendError::Framing);
    }
    Ok(reader.bytes()?)
}

fn counts_from_records(
    old: u64,
    new: u64,
    records: &[TrendRecord],
) -> Result<TrendPopulationCounts, TrendError> {
    let mut result = TrendPopulationCounts {
        old,
        new,
        ..TrendPopulationCounts::default()
    };
    for record in records {
        let value = match record.kind {
            TrendChangeKind::Added => &mut result.added,
            TrendChangeKind::Removed => &mut result.removed,
            TrendChangeKind::Unchanged => &mut result.unchanged,
            TrendChangeKind::Changed => &mut result.changed,
        };
        *value = value.checked_add(1).ok_or(TrendError::Arithmetic)?;
    }
    result.reconcile()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use external_test_provenance::sha256;

    use super::*;
    use crate::aggregate::binary_wire::Writer;
    use crate::aggregate::trend::{build_trend_v1, fingerprint};

    const UNCHANGED_VECTOR: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-trend-v1/unchanged.bin"
    ));
    const FOUR_POPULATION_VECTOR: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-trend-v1/four-population.bin"
    ));
    const EVALUATION_TRANSITION_VECTOR: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-trend-v1/evaluation-transition.bin"
    ));

    fn scope() -> Vec<u8> {
        let mut writer = Writer::new(64).unwrap();
        writer.string("none").unwrap();
        writer.string("not-requested").unwrap();
        writer.finish()
    }

    fn selected_scope() -> Vec<u8> {
        let mut writer = Writer::new(512).unwrap();
        writer.string("selected-variant-only").unwrap();
        writer
            .raw(&singleton_variant_key("dom-tree-basic-document"))
            .unwrap();
        writer.string("web-observable-dom-tree-v1").unwrap();
        writer.string("completed").unwrap();
        writer.finish()
    }

    fn population(population: TrendPopulation) -> PopulationTrend {
        PopulationTrend {
            population,
            counts: TrendPopulationCounts::default(),
            records: Vec::new(),
        }
    }

    fn empty_trend() -> ConformanceTrendV1 {
        ConformanceTrendV1 {
            old_baseline_sha256: sha256(b"old"),
            new_baseline_sha256: sha256(b"new"),
            inventory_scope: "static-html-css-no-js".to_owned(),
            aggregate_contract: "borrowser-conformance-aggregate-granularity-v1".to_owned(),
            named_lane: "normal-ci".to_owned(),
            environment_assessment: "ag9-empty-assessment-v1".to_owned(),
            old_source_set_sha256: sha256(b"old-source"),
            new_source_set_sha256: sha256(b"new-source"),
            old_evaluation_scope: scope(),
            new_evaluation_scope: scope(),
            old_advisory_total: 0,
            old_advisory_evaluated: 0,
            old_advisory_unevaluated: 0,
            new_advisory_total: 0,
            new_advisory_evaluated: 0,
            new_advisory_unevaluated: 0,
            populations: [
                population(TrendPopulation::LogicalCases),
                population(TrendPopulation::ExecutionVariants),
                population(TrendPopulation::AdvisoryComparisonPoints),
                population(TrendPopulation::BaselineNotes),
            ],
        }
    }

    fn logical_key(value: &str) -> Vec<u8> {
        let mut writer = Writer::new(256).unwrap();
        writer.string(value).unwrap();
        writer.finish()
    }

    fn unchanged_logical(value: &str) -> TrendRecord {
        let fingerprint = sha256(value.as_bytes());
        TrendRecord {
            kind: TrendChangeKind::Unchanged,
            key: logical_key(value),
            old_fingerprint: Some(fingerprint),
            new_fingerprint: Some(fingerprint),
            advisory: None,
        }
    }

    fn singleton_variant_key(value: &str) -> Vec<u8> {
        let mut writer = Writer::new(256).unwrap();
        writer.string(value).unwrap();
        writer.string("dom-tree").unwrap();
        writer.string("singleton").unwrap();
        writer.finish()
    }

    fn advisory_key(value: &str) -> Vec<u8> {
        let mut writer = Writer::new(512).unwrap();
        writer.raw(&singleton_variant_key(value)).unwrap();
        writer.string("web-observable-dom-tree-v1").unwrap();
        writer.string("track").unwrap();
        writer.finish()
    }

    fn unchanged_record(key: Vec<u8>, advisory: bool) -> TrendRecord {
        let fingerprint = sha256(&key);
        TrendRecord {
            kind: TrendChangeKind::Unchanged,
            key,
            old_fingerprint: Some(fingerprint),
            new_fingerprint: Some(fingerprint),
            advisory: advisory.then_some(AdvisoryTrendExtension {
                old_evaluated: Some(false),
                new_evaluated: Some(false),
                change_mask: 0,
            }),
        }
    }

    #[test]
    fn restored_trend_grammar_round_trips_without_public_raw_decode_authority() {
        let trend = empty_trend();
        let bytes = build_trend_v1(&trend).unwrap();
        assert_eq!(decode_trend_v1(&bytes).unwrap(), trend);
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_trend_v1(&trailing),
            Err(TrendError::TrailingBytes)
        ));
        assert!(decode_trend_v1(&bytes[..bytes.len() - 1]).is_err());
        assert!(!matches!(
            decode_trend_v1(&vec![0; TREND_MAX_BYTES_V1]),
            Err(TrendError::TooLarge)
        ));
        assert!(matches!(
            decode_trend_v1(&vec![0; TREND_MAX_BYTES_V1 + 1]),
            Err(TrendError::TooLarge)
        ));
    }

    #[test]
    fn selected_operation_descriptor_has_one_outer_frame_and_an_inline_variant_key() {
        let descriptor = selected_scope();
        let mut reader = Reader::new(&descriptor);
        assert_eq!(reader.string().unwrap(), "selected-variant-only");
        assert_eq!(reader.string().unwrap(), "dom-tree-basic-document");
        assert_eq!(reader.string().unwrap(), "dom-tree");
        assert_eq!(reader.string().unwrap(), "singleton");
        assert_eq!(reader.string().unwrap(), "web-observable-dom-tree-v1");
        assert_eq!(reader.string().unwrap(), "completed");
        reader.finish().unwrap();

        let mut trend = empty_trend();
        trend.old_evaluation_scope = descriptor.clone();
        trend.new_evaluation_scope = descriptor;
        let bytes = build_trend_v1(&trend).unwrap();
        assert_eq!(decode_trend_v1(&bytes).unwrap(), trend);
    }

    #[test]
    fn advisory_key_has_one_outer_frame_and_rejects_a_double_framed_variant_key() {
        let inline_key = advisory_key("dom-tree-basic-document");
        let mut key_reader = Reader::new(&inline_key);
        assert_eq!(key_reader.string().unwrap(), "dom-tree-basic-document");
        assert_eq!(key_reader.string().unwrap(), "dom-tree");
        assert_eq!(key_reader.string().unwrap(), "singleton");
        assert_eq!(key_reader.string().unwrap(), "web-observable-dom-tree-v1");
        assert_eq!(key_reader.string().unwrap(), "track");
        key_reader.finish().unwrap();

        // Independently fixed bytes: enter section 6, its only framed record,
        // and the record's one population-key frame. The first key field is
        // immediately S(TestId), rather than B(VariantKey).
        let mut envelope = Reader::new(EVALUATION_TRANSITION_VECTOR);
        assert_eq!(
            envelope.take(TREND_FORMAT_V1.len()).unwrap(),
            TREND_FORMAT_V1.as_bytes()
        );
        assert_eq!(envelope.u8().unwrap(), 0);
        assert_eq!(envelope.u16().unwrap(), 7);
        for tag in 1..=5 {
            section(&mut envelope, tag).unwrap();
        }
        let mut advisory_population = Reader::new(section(&mut envelope, 6).unwrap());
        for _ in 0..6 {
            advisory_population.u64().unwrap();
        }
        assert_eq!(advisory_population.u32().unwrap(), 1);
        let mut record = Reader::new(advisory_population.bytes().unwrap());
        assert_eq!(record.string().unwrap(), "changed");
        let mut independent_key = Reader::new(record.bytes().unwrap());
        assert_eq!(independent_key.string().unwrap(), "dom-tree-basic-document");
        assert_eq!(independent_key.string().unwrap(), "dom-tree");
        assert_eq!(independent_key.string().unwrap(), "singleton");
        assert_eq!(
            independent_key.string().unwrap(),
            "web-observable-dom-tree-v1"
        );
        assert_eq!(independent_key.string().unwrap(), "track");
        independent_key.finish().unwrap();

        let mut trend = empty_trend();
        trend.old_advisory_total = 1;
        trend.old_advisory_unevaluated = 1;
        trend.new_advisory_total = 1;
        trend.new_advisory_unevaluated = 1;
        trend.populations[2] = PopulationTrend {
            population: TrendPopulation::AdvisoryComparisonPoints,
            counts: TrendPopulationCounts {
                old: 1,
                new: 1,
                unchanged: 1,
                ..TrendPopulationCounts::default()
            },
            records: vec![unchanged_record(inline_key, true)],
        };
        decode_trend_v1(&build_trend_v1(&trend).unwrap()).unwrap();

        let mut double_framed_key = Writer::new(512).unwrap();
        double_framed_key
            .bytes(&singleton_variant_key("dom-tree-basic-document"))
            .unwrap();
        double_framed_key
            .string("web-observable-dom-tree-v1")
            .unwrap();
        double_framed_key.string("track").unwrap();
        trend.populations[2].records = vec![unchanged_record(double_framed_key.finish(), true)];
        assert!(matches!(
            decode_trend_v1(&build_trend_v1(&trend).unwrap()),
            Err(TrendError::Framing)
        ));
    }

    #[test]
    fn logical_keys_are_validated_in_semantic_order_not_framed_byte_order() {
        let mut trend = empty_trend();
        let long_a = "aaaaaaaaaa";
        let short_z = "z";
        assert!(logical_key(long_a) > logical_key(short_z));
        trend.populations[0] = PopulationTrend {
            population: TrendPopulation::LogicalCases,
            counts: TrendPopulationCounts {
                old: 2,
                new: 2,
                unchanged: 2,
                ..TrendPopulationCounts::default()
            },
            records: vec![unchanged_logical(long_a), unchanged_logical(short_z)],
        };
        let bytes = build_trend_v1(&trend).unwrap();
        assert_eq!(decode_trend_v1(&bytes).unwrap(), trend);

        trend.populations[0].records.swap(0, 1);
        let noncanonical = build_trend_v1(&trend).unwrap();
        assert!(matches!(
            decode_trend_v1(&noncanonical),
            Err(TrendError::Framing)
        ));
    }

    #[test]
    fn every_population_uses_typed_semantic_order_instead_of_framed_bytes() {
        let long_a = "aaaaaaaaaa";
        let short_z = "z";
        let key_pairs = [
            (logical_key(long_a), logical_key(short_z)),
            (
                singleton_variant_key(long_a),
                singleton_variant_key(short_z),
            ),
            (advisory_key(long_a), advisory_key(short_z)),
            (logical_key(long_a), logical_key(short_z)),
        ];
        assert!(key_pairs.iter().all(|(left, right)| left > right));

        let mut trend = empty_trend();
        trend.old_advisory_total = 2;
        trend.old_advisory_unevaluated = 2;
        trend.new_advisory_total = 2;
        trend.new_advisory_unevaluated = 2;
        for (index, (left, right)) in key_pairs.into_iter().enumerate() {
            trend.populations[index] = PopulationTrend {
                population: [
                    TrendPopulation::LogicalCases,
                    TrendPopulation::ExecutionVariants,
                    TrendPopulation::AdvisoryComparisonPoints,
                    TrendPopulation::BaselineNotes,
                ][index],
                counts: TrendPopulationCounts {
                    old: 2,
                    new: 2,
                    unchanged: 2,
                    ..TrendPopulationCounts::default()
                },
                records: vec![
                    unchanged_record(left, index == 2),
                    unchanged_record(right, index == 2),
                ],
            };
        }
        let canonical = build_trend_v1(&trend).unwrap();
        decode_trend_v1(&canonical).unwrap();

        for index in 0..4 {
            trend.populations[index].records.swap(0, 1);
            let noncanonical = build_trend_v1(&trend).unwrap();
            assert!(matches!(
                decode_trend_v1(&noncanonical),
                Err(TrendError::Framing)
            ));
            trend.populations[index].records.swap(0, 1);
        }
    }

    #[test]
    fn logical_fingerprint_items_frame_the_complete_key_hash_pair() {
        let case = crate::aggregate::baseline::HistoricalLogicalCase {
            test_id: "case".to_owned(),
            member_digest: sha256(b"member"),
            canonical_metadata_prefix: b"metadata".to_vec(),
            variants: vec![crate::aggregate::baseline::HistoricalVariant {
                key: HistoricalVariantKey {
                    test_id: "case".to_owned(),
                    observation: "dom-tree".to_owned(),
                    variant: crate::aggregate::baseline::HistoricalVariantIdentity::Singleton,
                },
                canonical_record: b"variant".to_vec(),
                comparison: crate::AggregateComparisonKind::AuthoredExpectedObservation,
                selection: crate::aggregate::model::AggregateSelectionProjection::Selected,
                attempt: crate::aggregate::model::AggregateAttemptProjection::Attempted(
                    crate::AggregateTerminalOutcome::SemanticPass,
                ),
            }],
        };
        let variant_hash = fingerprint::variant_fingerprint(&case, &case.variants[0]).unwrap();
        let mut expected = Writer::new(1024).unwrap();
        expected
            .raw(b"borrowser-conformance-logical-case-fingerprint-v1\0")
            .unwrap();
        expected.bytes(b"metadata").unwrap();
        expected.u32(1).unwrap();
        let mut pair = Writer::new(1024).unwrap();
        pair.string("case").unwrap();
        pair.string("dom-tree").unwrap();
        pair.string("singleton").unwrap();
        pair.raw(variant_hash.as_bytes()).unwrap();
        let pair = pair.finish();
        expected.bytes(&pair).unwrap();
        let expected = expected.finish();
        assert_eq!(fingerprint::logical(&case).unwrap(), sha256(&expected));

        let mut pair_reader = Reader::new(&pair);
        assert_eq!(pair_reader.string().unwrap(), "case");
        assert_eq!(pair_reader.string().unwrap(), "dom-tree");
        assert_eq!(pair_reader.string().unwrap(), "singleton");
        assert_eq!(pair_reader.take(32).unwrap(), variant_hash.as_bytes());
        pair_reader.finish().unwrap();

        let mut raw_key = Writer::new(256).unwrap();
        raw_key.string("case").unwrap();
        raw_key.string("dom-tree").unwrap();
        raw_key.string("singleton").unwrap();
        let mut double_framed_pair = Writer::new(512).unwrap();
        double_framed_pair.bytes(&raw_key.finish()).unwrap();
        double_framed_pair.raw(variant_hash.as_bytes()).unwrap();
        let mut forbidden = Writer::new(1024).unwrap();
        forbidden
            .raw(b"borrowser-conformance-logical-case-fingerprint-v1\0")
            .unwrap();
        forbidden.bytes(b"metadata").unwrap();
        forbidden.u32(1).unwrap();
        forbidden.bytes(&double_framed_pair.finish()).unwrap();
        assert_ne!(
            fingerprint::logical(&case).unwrap(),
            sha256(&forbidden.finish())
        );
    }

    #[test]
    fn independently_authored_trend_vectors_decode_with_frozen_semantics() {
        let vectors = [
            (
                UNCHANGED_VECTOR,
                10_608,
                "dfb48cc741f0a00d643df563e3e034efe2546cb2c4235f4f3746794ee8396702",
            ),
            (
                FOUR_POPULATION_VECTOR,
                2_003,
                "6ffdaec2af0ad71190021279d81a92808595cb3aecef4d52f2aae6a2d0c096af",
            ),
            (
                EVALUATION_TRANSITION_VECTOR,
                1_669,
                "7603a20c62d2bcf29f8be9731569623d215fd0773cafa4798b27bb264b5e0f69",
            ),
        ];
        for (bytes, length, digest) in vectors {
            assert_eq!(bytes.len(), length);
            assert_eq!(sha256(bytes).to_hex(), digest);
            decode_trend_v1(bytes).unwrap();
        }

        let unchanged = decode_trend_v1(UNCHANGED_VECTOR).unwrap();
        assert_eq!(unchanged.populations[0].counts.unchanged, 25);
        assert_eq!(unchanged.populations[1].counts.unchanged, 25);

        let all = decode_trend_v1(FOUR_POPULATION_VECTOR).unwrap();
        assert_eq!(all.populations[0].counts.removed, 1);
        assert_eq!(all.populations[1].counts.added, 1);
        assert_eq!(all.populations[2].counts.changed, 1);
        assert_eq!(all.populations[3].counts.unchanged, 1);

        let transition = decode_trend_v1(EVALUATION_TRANSITION_VECTOR).unwrap();
        let advisory = transition.populations[2].records[0].advisory.unwrap();
        assert_eq!(advisory.old_evaluated, Some(false));
        assert_eq!(advisory.new_evaluated, Some(true));
        assert_eq!(advisory.change_mask, ADVISORY_CHANGE_EVALUATION_PRESENCE);
    }

    #[test]
    fn advisory_extension_rejects_invalid_option_and_mask_bits() {
        // The independently fixed transition record ends with old O(U8), new
        // O(U8), then its U8 mask. Its old option discriminant is at 1,586.
        let mut invalid_option = EVALUATION_TRANSITION_VECTOR.to_vec();
        invalid_option[1_586] = 2;
        assert!(matches!(
            decode_trend_v1(&invalid_option),
            Err(TrendError::InvalidOption)
        ));

        let mut invalid_mask = EVALUATION_TRANSITION_VECTOR.to_vec();
        invalid_mask[1_606] = 0x08;
        assert!(matches!(
            decode_trend_v1(&invalid_mask),
            Err(TrendError::Framing)
        ));

        let mut result_drift_without_two_results = EVALUATION_TRANSITION_VECTOR.to_vec();
        result_drift_without_two_results[1_606] =
            ADVISORY_CHANGE_EVALUATION_PRESENCE | ADVISORY_CHANGE_RESULT;
        assert!(matches!(
            decode_trend_v1(&result_drift_without_two_results),
            Err(TrendError::Framing)
        ));
    }
}
