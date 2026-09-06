use external_test_provenance::{Sha256Digest, sha256};

use super::super::baseline::{
    HistoricalAdvisoryPoint, HistoricalLogicalCase, HistoricalNote, HistoricalVariant,
    HistoricalVariantIdentity, HistoricalVariantKey, ValidatedHistoricalBaseline,
};
use super::super::binary_wire::Writer;
use super::TrendError;

const LOGICAL_DOMAIN: &[u8] = b"borrowser-conformance-logical-case-fingerprint-v1\0";
const VARIANT_DOMAIN: &[u8] = b"borrowser-conformance-execution-variant-fingerprint-v1\0";
const ADVISORY_DOMAIN: &[u8] = b"borrowser-conformance-advisory-point-fingerprint-v1\0";
const NOTE_DOMAIN: &[u8] = b"borrowser-conformance-baseline-note-fingerprint-v1\0";

pub(super) fn logical(case: &HistoricalLogicalCase) -> Result<Sha256Digest, TrendError> {
    let mut writer = Writer::new(super::TREND_MAX_BYTES_V1)?;
    writer.raw(LOGICAL_DOMAIN)?;
    writer.bytes(&case.canonical_metadata_prefix)?;
    writer.u32(u32::try_from(case.variants.len()).map_err(|_| TrendError::Arithmetic)?)?;
    for variant in &case.variants {
        let mut item = Writer::new(super::TREND_MAX_BYTES_V1)?;
        item.raw(&variant_key(&variant.key)?)?;
        item.raw(variant_fingerprint(case, variant)?.as_bytes())?;
        writer.bytes(&item.finish())?;
    }
    Ok(sha256(&writer.finish()))
}

pub(super) fn variant_fingerprint(
    case: &HistoricalLogicalCase,
    variant: &HistoricalVariant,
) -> Result<Sha256Digest, TrendError> {
    let mut writer = Writer::new(super::TREND_MAX_BYTES_V1)?;
    writer.raw(VARIANT_DOMAIN)?;
    writer.bytes(&case.canonical_metadata_prefix)?;
    writer.bytes(&variant_key(&variant.key)?)?;
    writer.bytes(&variant.canonical_record)?;
    Ok(sha256(&writer.finish()))
}

pub(super) fn advisory(
    baseline: &ValidatedHistoricalBaseline,
    point: &HistoricalAdvisoryPoint,
) -> Result<Sha256Digest, TrendError> {
    let capture = baseline
        .captures
        .iter()
        .find(|capture| capture.id().as_sha256() == point.capture_id)
        .ok_or(TrendError::Framing)?;
    let mut writer = Writer::new(super::TREND_MAX_BYTES_V1)?;
    writer.raw(ADVISORY_DOMAIN)?;
    writer.bytes(&point.canonical_record)?;
    writer.bytes(capture.canonical_preimage())?;
    writer.optional(point.result.as_deref())?;
    Ok(sha256(&writer.finish()))
}

pub(super) fn note(note: &HistoricalNote) -> Result<Sha256Digest, TrendError> {
    let mut writer = Writer::new(super::TREND_MAX_BYTES_V1)?;
    writer.raw(NOTE_DOMAIN)?;
    writer.bytes(&note.canonical_record)?;
    Ok(sha256(&writer.finish()))
}

pub(super) fn variant_key(key: &HistoricalVariantKey) -> Result<Vec<u8>, TrendError> {
    let mut writer = Writer::new(1024)?;
    writer.string(&key.test_id)?;
    writer.string(&key.observation)?;
    match &key.variant {
        HistoricalVariantIdentity::Singleton => writer.string("singleton")?,
        HistoricalVariantIdentity::Rendering {
            environment,
            available_width_css_px,
        } => {
            writer.string("rendering")?;
            writer.string(environment)?;
            writer.u32(*available_width_css_px)?;
        }
    }
    Ok(writer.finish())
}

pub(super) fn advisory_key(point: &HistoricalAdvisoryPoint) -> Result<Vec<u8>, TrendError> {
    let mut writer = Writer::new(2048)?;
    writer.raw(&variant_key(&point.key.variant)?)?;
    writer.string(&point.key.comparable)?;
    writer.string(&point.key.track_id)?;
    Ok(writer.finish())
}

pub(super) fn logical_key(case: &HistoricalLogicalCase) -> Result<Vec<u8>, TrendError> {
    let mut writer = Writer::new(256)?;
    writer.string(&case.test_id)?;
    Ok(writer.finish())
}

pub(super) fn note_key(note: &HistoricalNote) -> Result<Vec<u8>, TrendError> {
    let mut writer = Writer::new(256)?;
    writer.string(&note.id)?;
    Ok(writer.finish())
}

pub(super) fn evaluation_scope(
    baseline: &ValidatedHistoricalBaseline,
) -> Result<Vec<u8>, TrendError> {
    use super::super::baseline::HistoricalEvaluationScope;
    let mut writer = Writer::new(2048)?;
    match &baseline.evaluation {
        HistoricalEvaluationScope::None => {
            writer.string("none")?;
            writer.string("not-requested")?;
        }
        HistoricalEvaluationScope::Selected {
            variant,
            comparable,
        } => {
            writer.string("selected-variant-only")?;
            writer.raw(&variant_key(variant)?)?;
            writer.string(comparable)?;
            writer.string("completed")?;
        }
        HistoricalEvaluationScope::AllDeclared => {
            writer.string("all-declared")?;
            writer.string("completed")?;
        }
    }
    Ok(writer.finish())
}
