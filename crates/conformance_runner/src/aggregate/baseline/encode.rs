use std::io::Write as IoWrite;

use external_test_provenance::canonical_capture_id_preimage_v1;

use super::super::binary_wire::Writer;
use crate::{AggregateExecutionVariantId, AggregateVariantKey};

use super::{
    BASELINE_FORMAT_V1, BASELINE_MAX_BYTES_V1, BaselineError, BaselineEvidence,
    SealedBaselineEvidence,
};

pub const BASELINE_VERSIONS_V1: [&str; 13] = [
    "borrowser-conformance-aggregate-detail-v1",
    "borrowser-conformance-aggregate-granularity-v1",
    "borrowser-conformance-logical-case-membership-v1",
    "borrowser-conformance-advisory-membership-v1",
    "borrowser-conformance-baseline-note-v1",
    "borrowser-conformance-advisory-result-v1",
    "borrowser-conformance-logical-case-fingerprint-v1",
    "borrowser-conformance-execution-variant-fingerprint-v1",
    "borrowser-conformance-advisory-point-fingerprint-v1",
    "borrowser-conformance-baseline-note-fingerprint-v1",
    "borrowser-external-capture-provenance-v1",
    "borrowser-external-capture-id-v1",
    "borrowser-advisory-dom-first-difference-v1",
];

pub fn build_baseline_v1(
    sealed: &SealedBaselineEvidence<'_, '_>,
) -> Result<Vec<u8>, BaselineError> {
    let detail =
        crate::build_aggregate_detail_v1(sealed.run).map_err(BaselineError::AggregateDetail)?;
    let evidence = match sealed.evidence {
        BaselineEvidence::NotRequested(v) => v,
        BaselineEvidence::Selected(v) => v.evidence(),
    };
    let mut out = Writer::new(BASELINE_MAX_BYTES_V1)?;
    out.raw(BASELINE_FORMAT_V1.as_bytes())?;
    out.u8(0)?;
    out.u16(7)?;
    section(&mut out, 1, |w| {
        for v in BASELINE_VERSIONS_V1 {
            w.string(v)?
        }
        Ok(())
    })?;
    section(&mut out, 2, |w| {
        w.raw(&detail)?;
        Ok(())
    })?;
    section(&mut out, 3, |w| encode_captures(w, evidence))?;
    section(&mut out, 4, |w| encode_tracks(w, evidence))?;
    let ordered = ordered_attachments(evidence)?;
    section(&mut out, 5, |w| {
        w.u32(to_u32(ordered.len())?)?;
        for a in &ordered {
            record(w, |r| {
                encode_live_key(r, &a.aggregate_variant().key)?;
                r.string(a.comparable().as_str())?;
                r.string(a.track_id().as_str())?;
                r.raw(a.capture_id().as_sha256().as_bytes())?;
                Ok(())
            })?
        }
        Ok(())
    })?;
    section(&mut out, 6, |w| {
        encode_evaluation(w, &sealed.evidence, &ordered)
    })?;
    section(&mut out, 7, |w| encode_notes(w, evidence))?;
    Ok(out.finish())
}

pub fn build_and_write_baseline_v1(
    sealed: &SealedBaselineEvidence<'_, '_>,
    output: &mut impl IoWrite,
) -> Result<(), BaselineError> {
    let bytes = build_baseline_v1(sealed)?;
    output.write_all(&bytes).map_err(BaselineError::Output)
}
fn section(
    out: &mut Writer,
    tag: u16,
    f: impl FnOnce(&mut Writer) -> Result<(), BaselineError>,
) -> Result<(), BaselineError> {
    let mut p = Writer::new(BASELINE_MAX_BYTES_V1)?;
    f(&mut p)?;
    out.u16(tag)?;
    out.bytes(&p.finish())?;
    Ok(())
}
fn record(
    out: &mut Writer,
    f: impl FnOnce(&mut Writer) -> Result<(), BaselineError>,
) -> Result<(), BaselineError> {
    let mut p = Writer::new(BASELINE_MAX_BYTES_V1)?;
    f(&mut p)?;
    out.bytes(&p.finish())?;
    Ok(())
}
fn to_u32(n: usize) -> Result<u32, BaselineError> {
    u32::try_from(n).map_err(|_| BaselineError::InvalidEvidence)
}
fn encode_captures(
    w: &mut Writer,
    e: &crate::ReconciledExternalAdvisoryEvidence<'_>,
) -> Result<(), BaselineError> {
    let mut c = Vec::new();
    c.try_reserve_exact(e.captures().len())
        .map_err(|_| BaselineError::Allocation)?;
    c.extend(e.captures());
    c.sort_unstable_by_key(|v| v.capture().id());
    if c.windows(2)
        .any(|p| p[0].capture().id() == p[1].capture().id())
    {
        return Err(BaselineError::DuplicateIdentity);
    }
    w.u32(to_u32(c.len())?)?;
    for v in c {
        record(w, |r| {
            r.raw(v.capture().id().as_sha256().as_bytes())?;
            let pre = canonical_capture_id_preimage_v1(v.capture().provenance())?;
            r.bytes(&pre)?;
            Ok(())
        })?
    }
    Ok(())
}
fn encode_tracks(
    w: &mut Writer,
    e: &crate::ReconciledExternalAdvisoryEvidence<'_>,
) -> Result<(), BaselineError> {
    let mut t = Vec::new();
    t.try_reserve_exact(e.tracks().len())
        .map_err(|_| BaselineError::Allocation)?;
    t.extend(e.tracks());
    t.sort_unstable_by(|a, b| a.id().as_str().as_bytes().cmp(b.id().as_str().as_bytes()));
    if t.windows(2)
        .any(|p| p[0].id().as_str() == p[1].id().as_str())
    {
        return Err(BaselineError::DuplicateIdentity);
    }
    w.u32(to_u32(t.len())?)?;
    for v in t {
        record(w, |r| {
            for s in [
                v.id().as_str(),
                v.engine_product().as_str(),
                v.platform_os_family().as_str(),
                v.architecture().as_str(),
                v.comparable().as_str(),
                v.capture_algorithm().as_str(),
                v.capture_algorithm_version().as_str(),
                v.target_parser_input_context().as_str(),
                v.collection_policy().as_str(),
                v.collection_policy_version().as_str(),
            ] {
                r.string(s)?
            }
            Ok(())
        })?
    }
    Ok(())
}
fn ordered_attachments<'a, 'run>(
    e: &'a crate::ReconciledExternalAdvisoryEvidence<'run>,
) -> Result<Vec<&'a crate::ReconciledExternalAttachment<'run>>, BaselineError> {
    let mut a = Vec::new();
    a.try_reserve_exact(e.attachments().len())
        .map_err(|_| BaselineError::Allocation)?;
    a.extend(e.attachments());
    a.sort_unstable_by(|l, r| live_point_cmp(l, r));
    if a.windows(2).any(|p| same_point(p[0], p[1])) {
        return Err(BaselineError::DuplicateIdentity);
    }
    Ok(a)
}
fn same_point(
    a: &crate::ReconciledExternalAttachment<'_>,
    b: &crate::ReconciledExternalAttachment<'_>,
) -> bool {
    a.aggregate_variant().key == b.aggregate_variant().key
        && a.comparable() == b.comparable()
        && a.track_id() == b.track_id()
}
fn live_point_cmp(
    a: &crate::ReconciledExternalAttachment<'_>,
    b: &crate::ReconciledExternalAttachment<'_>,
) -> std::cmp::Ordering {
    live_key_cmp(&a.aggregate_variant().key, &b.aggregate_variant().key)
        .then_with(|| {
            a.comparable()
                .as_str()
                .as_bytes()
                .cmp(b.comparable().as_str().as_bytes())
        })
        .then_with(|| {
            a.track_id()
                .as_str()
                .as_bytes()
                .cmp(b.track_id().as_str().as_bytes())
        })
}
fn live_key_cmp(a: &AggregateVariantKey, b: &AggregateVariantKey) -> std::cmp::Ordering {
    a.test_id
        .as_str()
        .as_bytes()
        .cmp(b.test_id.as_str().as_bytes())
        .then_with(|| {
            a.observation
                .as_str()
                .as_bytes()
                .cmp(b.observation.as_str().as_bytes())
        })
        .then_with(|| match (&a.variant, &b.variant) {
            (
                AggregateExecutionVariantId::Singleton(_),
                AggregateExecutionVariantId::Singleton(_),
            ) => std::cmp::Ordering::Equal,
            (AggregateExecutionVariantId::Singleton(_), _) => std::cmp::Ordering::Less,
            (_, AggregateExecutionVariantId::Singleton(_)) => std::cmp::Ordering::Greater,
            (
                AggregateExecutionVariantId::Rendering(a),
                AggregateExecutionVariantId::Rendering(b),
            ) => a
                .value()
                .stable_environment_label()
                .as_bytes()
                .cmp(b.value().stable_environment_label().as_bytes())
                .then_with(|| {
                    a.value()
                        .available_width_css_px
                        .get()
                        .cmp(&b.value().available_width_css_px.get())
                }),
        })
}
fn encode_live_key(w: &mut Writer, k: &AggregateVariantKey) -> Result<(), BaselineError> {
    w.string(k.test_id.as_str())?;
    w.string(k.observation.as_str())?;
    match &k.variant {
        AggregateExecutionVariantId::Singleton(_) => w.string("singleton")?,
        AggregateExecutionVariantId::Rendering(v) => {
            w.string("rendering")?;
            w.string(v.value().stable_environment_label())?;
            w.u32(v.value().available_width_css_px.get())?
        }
    }
    Ok(())
}
fn encode_evaluation(
    w: &mut Writer,
    evidence: &BaselineEvidence<'_, '_>,
    ordered: &[&crate::ReconciledExternalAttachment<'_>],
) -> Result<(), BaselineError> {
    match evidence {
        BaselineEvidence::NotRequested(_) => {
            w.string("none")?;
            w.string("not-requested")?;
            w.u32(to_u32(ordered.len())?)?;
            for _ in ordered {
                record(w, |slot| {
                    slot.optional(None)?;
                    Ok(())
                })?;
            }
        }
        BaselineEvidence::Selected(operation) => {
            let mut evaluations = Vec::new();
            evaluations
                .try_reserve_exact(operation.in_scope_attachment_count())
                .map_err(|_| BaselineError::Allocation)?;
            evaluations.extend(operation.evaluated());
            evaluations.sort_unstable_by(|left, right| live_point_cmp(left.0, right.0));
            if evaluations.len() != operation.in_scope_attachment_count()
                || evaluations
                    .windows(2)
                    .any(|pair| same_point(pair[0].0, pair[1].0))
                || evaluations.iter().any(|(attachment, comparison)| {
                    attachment.capture_id() != comparison.capture_id()
                })
            {
                return Err(BaselineError::InvalidEvaluation);
            }
            w.string("selected-variant-only")?;
            encode_live_key(w, operation.selected())?;
            w.string(operation.comparable().as_str())?;
            w.string("completed")?;
            w.u32(to_u32(ordered.len())?)?;
            let mut evaluation_index = 0;
            for attachment in ordered {
                let result = match evaluations.get(evaluation_index) {
                    Some((candidate, comparison)) => match live_point_cmp(candidate, attachment) {
                        std::cmp::Ordering::Less => {
                            return Err(BaselineError::InvalidEvaluation);
                        }
                        std::cmp::Ordering::Equal => {
                            evaluation_index = evaluation_index
                                .checked_add(1)
                                .ok_or(BaselineError::LengthOverflow)?;
                            Some(comparison.result())
                        }
                        std::cmp::Ordering::Greater => None,
                    },
                    None => None,
                };
                match result {
                    None => record(w, |slot| {
                        slot.optional(None)?;
                        Ok(())
                    })?,
                    Some(result) => {
                        let mut encoded = Writer::new(super::result::MAX_ADVISORY_RESULT_BYTES_V1)?;
                        super::result::encode_result(&mut encoded, result)?;
                        record(w, |slot| {
                            slot.optional(Some(&encoded.finish()))?;
                            Ok(())
                        })?;
                    }
                }
            }
            if evaluation_index != evaluations.len() {
                return Err(BaselineError::InvalidEvaluation);
            }
        }
    }
    Ok(())
}
fn encode_notes(
    w: &mut Writer,
    e: &crate::ReconciledExternalAdvisoryEvidence<'_>,
) -> Result<(), BaselineError> {
    let mut n = Vec::new();
    n.try_reserve_exact(e.notes().len())
        .map_err(|_| BaselineError::Allocation)?;
    n.extend(e.notes());
    n.sort_unstable_by(|a, b| a.id().as_str().as_bytes().cmp(b.id().as_str().as_bytes()));
    if n.windows(2).any(|p| p[0].id() == p[1].id()) {
        return Err(BaselineError::DuplicateIdentity);
    }
    w.u32(to_u32(n.len())?)?;
    for v in n {
        record(w, |r| {
            r.string(v.id().as_str())?;
            encode_live_key(r, &v.aggregate_variant().key)?;
            r.string(v.comparable().as_str())?;
            r.string(v.text())?;
            let capture_digest = v.capture_id().map(|id| id.as_sha256());
            r.optional(capture_digest.as_ref().map(|digest| &digest.as_bytes()[..]))?;
            Ok(())
        })?
    }
    Ok(())
}
