use std::io::Write as IoWrite;

use super::super::baseline::{BASELINE_FORMAT_V1, BASELINE_VERSIONS_V1};
use super::super::binary_wire::Writer;
use super::{
    ConformanceTrendV1, PopulationTrend, TREND_FORMAT_V1, TREND_MAX_BYTES_V1, TrendError,
    TrendPopulation,
};

pub const TREND_VERSIONS_V1: [&str; 14] = [
    BASELINE_FORMAT_V1,
    BASELINE_VERSIONS_V1[0],
    BASELINE_VERSIONS_V1[1],
    BASELINE_VERSIONS_V1[2],
    BASELINE_VERSIONS_V1[3],
    BASELINE_VERSIONS_V1[4],
    BASELINE_VERSIONS_V1[5],
    BASELINE_VERSIONS_V1[6],
    BASELINE_VERSIONS_V1[7],
    BASELINE_VERSIONS_V1[8],
    BASELINE_VERSIONS_V1[9],
    BASELINE_VERSIONS_V1[10],
    BASELINE_VERSIONS_V1[11],
    BASELINE_VERSIONS_V1[12],
];

pub fn build_trend_v1(trend: &ConformanceTrendV1) -> Result<Vec<u8>, TrendError> {
    for population in &trend.populations {
        population.counts.reconcile()?;
    }
    let expected = [
        TrendPopulation::LogicalCases,
        TrendPopulation::ExecutionVariants,
        TrendPopulation::AdvisoryComparisonPoints,
        TrendPopulation::BaselineNotes,
    ];
    if trend
        .populations
        .iter()
        .zip(expected)
        .any(|(actual, expected)| actual.population != expected)
    {
        return Err(TrendError::Reconciliation);
    }

    let mut output = Writer::new(TREND_MAX_BYTES_V1)?;
    output.raw(TREND_FORMAT_V1.as_bytes())?;
    output.u8(0)?;
    output.u16(7)?;
    section(&mut output, 1, |writer| {
        for version in TREND_VERSIONS_V1 {
            writer.string(version)?;
        }
        Ok(())
    })?;
    section(&mut output, 2, |writer| {
        writer.string(&trend.inventory_scope)?;
        writer.string(&trend.aggregate_contract)?;
        writer.string(&trend.named_lane)?;
        writer.string(&trend.environment_assessment)?;
        writer.raw(trend.old_baseline_sha256.as_bytes())?;
        writer.raw(trend.new_baseline_sha256.as_bytes())?;
        writer.raw(trend.old_source_set_sha256.as_bytes())?;
        writer.raw(trend.new_source_set_sha256.as_bytes())?;
        Ok(())
    })?;
    section(&mut output, 3, |writer| {
        writer.bytes(&trend.old_evaluation_scope)?;
        writer.bytes(&trend.new_evaluation_scope)?;
        for count in [
            trend.old_advisory_total,
            trend.old_advisory_evaluated,
            trend.old_advisory_unevaluated,
            trend.new_advisory_total,
            trend.new_advisory_evaluated,
            trend.new_advisory_unevaluated,
        ] {
            writer.u64(count)?;
        }
        Ok(())
    })?;
    for (tag, population) in (4_u16..=7).zip(&trend.populations) {
        section(&mut output, tag, |writer| {
            encode_population(writer, population)
        })?;
    }
    Ok(output.finish())
}

pub fn build_and_write_trend_v1(
    trend: &ConformanceTrendV1,
    output: &mut impl IoWrite,
) -> Result<(), TrendError> {
    let bytes = build_trend_v1(trend)?;
    output.write_all(&bytes).map_err(TrendError::Output)
}

fn encode_population(writer: &mut Writer, trend: &PopulationTrend) -> Result<(), TrendError> {
    for count in [
        trend.counts.old,
        trend.counts.new,
        trend.counts.added,
        trend.counts.removed,
        trend.counts.unchanged,
        trend.counts.changed,
    ] {
        writer.u64(count)?;
    }
    writer.u32(u32::try_from(trend.records.len()).map_err(|_| TrendError::Arithmetic)?)?;
    for record in &trend.records {
        let mut body = Writer::new(TREND_MAX_BYTES_V1)?;
        body.string(record.kind.as_str())?;
        body.bytes(&record.key)?;
        optional_digest(&mut body, record.old_fingerprint)?;
        optional_digest(&mut body, record.new_fingerprint)?;
        if trend.population == TrendPopulation::AdvisoryComparisonPoints {
            let advisory = record.advisory.ok_or(TrendError::Reconciliation)?;
            optional_bool(&mut body, advisory.old_evaluated)?;
            optional_bool(&mut body, advisory.new_evaluated)?;
            body.u8(advisory.change_mask)?;
        } else if record.advisory.is_some() {
            return Err(TrendError::Reconciliation);
        }
        writer.bytes(&body.finish())?;
    }
    Ok(())
}

fn optional_digest(
    writer: &mut Writer,
    value: Option<external_test_provenance::Sha256Digest>,
) -> Result<(), TrendError> {
    writer.optional(value.as_ref().map(|digest| &digest.as_bytes()[..]))?;
    Ok(())
}

fn optional_bool(writer: &mut Writer, value: Option<bool>) -> Result<(), TrendError> {
    let byte = value.map(u8::from);
    writer.optional(byte.as_ref().map(std::slice::from_ref))?;
    Ok(())
}

fn section(
    output: &mut Writer,
    tag: u16,
    encode: impl FnOnce(&mut Writer) -> Result<(), TrendError>,
) -> Result<(), TrendError> {
    let mut body = Writer::new(TREND_MAX_BYTES_V1)?;
    encode(&mut body)?;
    output.u16(tag)?;
    output.bytes(&body.finish())?;
    Ok(())
}
