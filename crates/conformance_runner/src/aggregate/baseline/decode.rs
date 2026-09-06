use external_test_provenance::{
    ExternalCaptureIdClaim, ExternalIdentityV1, ExternalVersionV1, HistoricalCaptureIdentityV1,
    Sha256Digest, validate_historical_capture_identity_v1,
};

use super::super::binary_wire::Reader;

use super::{
    BASELINE_FORMAT_V1, BASELINE_MAX_BYTES_V1, BASELINE_VERSIONS_V1, BaselineError,
    HistoricalAdvisoryPoint, HistoricalAdvisoryPointKey, HistoricalEvaluationScope, HistoricalNote,
    HistoricalTrack, HistoricalVariantIdentity, HistoricalVariantKey, ValidatedHistoricalBaseline,
    decode_aggregate_detail_v1,
};

const MAX_RECORDS: usize = 256;

pub(crate) fn decode_baseline_v1(
    bytes: &[u8],
) -> Result<ValidatedHistoricalBaseline, BaselineError> {
    if bytes.len() > BASELINE_MAX_BYTES_V1 {
        return Err(BaselineError::TooLarge);
    }
    let mut r = Reader::new(bytes);
    if r.take(BASELINE_FORMAT_V1.len())? != BASELINE_FORMAT_V1.as_bytes()
        || r.u8()? != 0
        || r.u16()? != 7
    {
        return Err(BaselineError::InvalidFormat);
    }
    let versions = section(&mut r, 1)?;
    decode_versions(versions)?;
    let detail_bytes = section(&mut r, 2)?;
    let detail = decode_aggregate_detail_v1(detail_bytes)?;
    let captures = decode_captures(section(&mut r, 3)?)?;
    let tracks = decode_tracks(section(&mut r, 4)?)?;
    let mut points = decode_points(section(&mut r, 5)?)?;
    let evaluation = decode_evaluation(section(&mut r, 6)?, &mut points)?;
    let notes = decode_notes(section(&mut r, 7)?)?;
    r.finish()?;
    validate_relationships(&detail, &captures, &tracks, &points, &notes)?;
    let mut exact = Vec::new();
    exact
        .try_reserve_exact(bytes.len())
        .map_err(|_| BaselineError::Allocation)?;
    exact.extend_from_slice(bytes);
    Ok(ValidatedHistoricalBaseline {
        exact_bytes: exact,
        detail,
        captures,
        tracks,
        points,
        evaluation,
        notes,
    })
}
fn section<'a>(r: &mut Reader<'a>, tag: u16) -> Result<&'a [u8], BaselineError> {
    if r.u16() != Ok(tag) {
        return Err(BaselineError::InvalidFormat);
    }
    Ok(r.bytes()?)
}
fn decode_versions(bytes: &[u8]) -> Result<(), BaselineError> {
    let mut r = Reader::new(bytes);
    for expected in BASELINE_VERSIONS_V1 {
        if r.string()? != expected {
            return Err(BaselineError::InvalidVersion);
        }
    }
    r.finish()?;
    Ok(())
}
fn count(r: &mut Reader<'_>) -> Result<usize, BaselineError> {
    let n = usize::try_from(r.u32()?).map_err(|_| BaselineError::LengthOverflow)?;
    if n > MAX_RECORDS {
        return Err(BaselineError::InvalidEvidence);
    }
    Ok(n)
}
fn digest(r: &mut Reader<'_>) -> Result<Sha256Digest, BaselineError> {
    let b: [u8; 32] = r.take(32)?.try_into().map_err(|_| BaselineError::Framing)?;
    Ok(Sha256Digest::from_bytes(b))
}
pub(crate) fn valid_semantic_id(v: &str) -> bool {
    crate::AdvisoryTrackId::is_valid(v)
}
fn decode_captures(bytes: &[u8]) -> Result<Vec<HistoricalCaptureIdentityV1>, BaselineError> {
    let mut r = Reader::new(bytes);
    let n = count(&mut r)?;
    let mut out: Vec<HistoricalCaptureIdentityV1> = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| BaselineError::Allocation)?;
    let mut last = None;
    for _ in 0..n {
        let record = r.bytes()?;
        let mut i = Reader::new(record);
        let id = digest(&mut i)?;
        if last.is_some_and(|v: Sha256Digest| v >= id) {
            return Err(if last == Some(id) {
                BaselineError::DuplicateIdentity
            } else {
                BaselineError::InvalidOrdering
            });
        }
        last = Some(id);
        let pre = i.bytes()?;
        i.finish()?;
        let claim = ExternalCaptureIdClaim::from_sha256(id);
        out.push(validate_historical_capture_identity_v1(pre, claim)?)
    }
    r.finish()?;
    Ok(out)
}
fn decode_tracks(bytes: &[u8]) -> Result<Vec<HistoricalTrack>, BaselineError> {
    let mut r = Reader::new(bytes);
    let n = count(&mut r)?;
    let mut out: Vec<HistoricalTrack> = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| BaselineError::Allocation)?;
    for _ in 0..n {
        let record = r.bytes()?;
        let mut i = Reader::new(record);
        let id = i.owned_string()?;
        if !valid_semantic_id(&id) {
            return Err(BaselineError::InvalidEvidence);
        }
        let mut invariant = Vec::new();
        invariant
            .try_reserve_exact(9)
            .map_err(|_| BaselineError::Allocation)?;
        for _ in 0..9 {
            invariant.push(i.owned_string()?)
        }
        i.finish()?;
        if ExternalIdentityV1::parse(&invariant[0]).is_err()
            || ExternalIdentityV1::parse(&invariant[1]).is_err()
            || ExternalIdentityV1::parse(&invariant[2]).is_err()
            || crate::ComparableObservationSurface::parse(&invariant[3]).is_none()
            || ExternalIdentityV1::parse(&invariant[4]).is_err()
            || ExternalVersionV1::parse(&invariant[5]).is_err()
            || invariant[6] != "static-text-html-utf8-scripting-disabled-v1"
            || ExternalIdentityV1::parse(&invariant[7]).is_err()
            || ExternalVersionV1::parse(&invariant[8]).is_err()
        {
            return Err(BaselineError::InvalidEvidence);
        }
        if let Some(previous) = out.last()
            && previous.id.as_bytes() >= id.as_bytes()
        {
            return Err(if previous.id == id {
                BaselineError::DuplicateIdentity
            } else {
                BaselineError::InvalidOrdering
            });
        }
        out.push(HistoricalTrack {
            id,
            invariant,
            canonical_record: copy_bytes(record)?,
        })
    }
    r.finish()?;
    Ok(out)
}
fn decode_points(bytes: &[u8]) -> Result<Vec<HistoricalAdvisoryPoint>, BaselineError> {
    let mut r = Reader::new(bytes);
    let n = count(&mut r)?;
    let mut out: Vec<HistoricalAdvisoryPoint> = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| BaselineError::Allocation)?;
    for _ in 0..n {
        let record = r.bytes()?;
        let mut i = Reader::new(record);
        let variant = decode_historical_variant_key(&mut i)?;
        let comparable = i.owned_string()?;
        let track_id = i.owned_string()?;
        if crate::ComparableObservationSurface::parse(&comparable).is_none()
            || !valid_semantic_id(&track_id)
        {
            return Err(BaselineError::InvalidEvidence);
        }
        let capture_id = digest(&mut i)?;
        i.finish()?;
        let key = HistoricalAdvisoryPointKey {
            variant,
            comparable,
            track_id,
        };
        if let Some(previous) = out.last()
            && previous.key >= key
        {
            return Err(if previous.key == key {
                BaselineError::DuplicateIdentity
            } else {
                BaselineError::InvalidOrdering
            });
        }
        out.push(HistoricalAdvisoryPoint {
            key,
            capture_id,
            canonical_record: copy_bytes(record)?,
            result: None,
        })
    }
    r.finish()?;
    Ok(out)
}
fn decode_evaluation(
    bytes: &[u8],
    points: &mut [HistoricalAdvisoryPoint],
) -> Result<HistoricalEvaluationScope, BaselineError> {
    let mut r = Reader::new(bytes);
    let scope = match r.string()? {
        "none" => {
            if r.string()? != "not-requested" {
                return Err(BaselineError::InvalidEvaluation);
            }
            HistoricalEvaluationScope::None
        }
        "selected-variant-only" => {
            let variant = decode_historical_variant_key(&mut r)?;
            let comparable = r.owned_string()?;
            if crate::ComparableObservationSurface::parse(&comparable).is_none()
                || r.string()? != "completed"
            {
                return Err(BaselineError::InvalidEvaluation);
            }
            HistoricalEvaluationScope::Selected {
                variant,
                comparable,
            }
        }
        "all-declared" => {
            if r.string()? != "completed" {
                return Err(BaselineError::InvalidEvaluation);
            }
            HistoricalEvaluationScope::AllDeclared
        }
        _ => return Err(BaselineError::InvalidEvaluation),
    };
    let n = count(&mut r)?;
    if n != points.len() {
        return Err(BaselineError::InvalidEvaluation);
    }
    for point in points.iter_mut() {
        let slot = r.bytes()?;
        if slot.len() > super::result::MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1 {
            return Err(BaselineError::InvalidEvaluation);
        }
        let mut slot = Reader::new(slot);
        point.result = match slot.optional()? {
            None => None,
            Some(v) => {
                super::result::validate_result(v)?;
                Some(copy_bytes(v)?)
            }
        };
        slot.finish()?;
    }
    r.finish()?;
    for p in points {
        let expected = match &scope {
            HistoricalEvaluationScope::None => false,
            HistoricalEvaluationScope::Selected {
                variant,
                comparable,
            } => &p.key.variant == variant && &p.key.comparable == comparable,
            HistoricalEvaluationScope::AllDeclared => true,
        };
        if p.result.is_some() != expected {
            return Err(BaselineError::InvalidEvaluation);
        }
    }
    Ok(scope)
}
fn decode_notes(bytes: &[u8]) -> Result<Vec<HistoricalNote>, BaselineError> {
    let mut r = Reader::new(bytes);
    let n = count(&mut r)?;
    let mut out: Vec<HistoricalNote> = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| BaselineError::Allocation)?;
    for _ in 0..n {
        let record = r.bytes()?;
        let mut i = Reader::new(record);
        let id = i.owned_string()?;
        if !valid_semantic_id(&id) {
            return Err(BaselineError::InvalidEvidence);
        }
        let key = decode_historical_variant_key(&mut i)?;
        if crate::ComparableObservationSurface::parse(i.string()?).is_none() {
            return Err(BaselineError::InvalidEvidence);
        }
        if !matches!(key.variant, HistoricalVariantIdentity::Singleton)
            || key.observation != "dom-tree"
        {
            return Err(BaselineError::InvalidEvidence);
        }
        let text = i.string()?;
        if text.is_empty()
            || text.len() > 1024
            || text.trim() != text
            || text.chars().any(char::is_control)
        {
            return Err(BaselineError::InvalidEvidence);
        }
        if let Some(v) = i.optional()?
            && v.len() != 32
        {
            return Err(BaselineError::InvalidReference);
        }
        i.finish()?;
        if let Some(previous) = out.last()
            && previous.id.as_bytes() >= id.as_bytes()
        {
            return Err(if previous.id == id {
                BaselineError::DuplicateIdentity
            } else {
                BaselineError::InvalidOrdering
            });
        }
        out.push(HistoricalNote {
            id,
            canonical_record: copy_bytes(record)?,
        })
    }
    r.finish()?;
    Ok(out)
}
pub(crate) fn decode_historical_variant_key(
    r: &mut Reader<'_>,
) -> Result<HistoricalVariantKey, BaselineError> {
    let test_id = r.owned_string()?;
    if !conformance_test_support::TestId::is_valid(&test_id) {
        return Err(BaselineError::InvalidEvidence);
    }
    let observation = r.owned_string()?;
    if conformance_test_support::ObservationSurface::parse(&observation).is_none() {
        return Err(BaselineError::InvalidEvidence);
    }
    let kind = r.string()?;
    let variant = match kind {
        "singleton" => HistoricalVariantIdentity::Singleton,
        "rendering" => {
            let environment = r.owned_string()?;
            let width = r.u32()?;
            if environment != "synthetic-text-metrics-v1" || !(1..=16_777_216).contains(&width) {
                return Err(BaselineError::InvalidEvidence);
            }
            HistoricalVariantIdentity::Rendering {
                environment,
                available_width_css_px: width,
            }
        }
        _ => return Err(BaselineError::InvalidEvidence),
    };
    Ok(HistoricalVariantKey {
        test_id,
        observation,
        variant,
    })
}
fn validate_relationships(
    detail: &super::HistoricalDetail,
    captures: &[HistoricalCaptureIdentityV1],
    tracks: &[HistoricalTrack],
    points: &[HistoricalAdvisoryPoint],
    notes: &[HistoricalNote],
) -> Result<(), BaselineError> {
    for p in points {
        if !detail
            .cases
            .iter()
            .flat_map(|c| &c.variants)
            .any(|v| v.key == p.key.variant)
        {
            return Err(BaselineError::InvalidReference);
        }
        if !matches!(p.key.variant.variant, HistoricalVariantIdentity::Singleton)
            || p.key.variant.observation != "dom-tree"
        {
            return Err(BaselineError::InvalidReference);
        }
        let capture = captures
            .iter()
            .find(|c| c.id().as_sha256() == p.capture_id)
            .ok_or(BaselineError::InvalidReference)?;
        let track = tracks
            .iter()
            .find(|t| t.id == p.key.track_id)
            .ok_or(BaselineError::InvalidReference)?;
        let provenance = capture.provenance();
        let actual = [
            provenance.engine_product().as_str(),
            provenance.platform_os_family().as_str(),
            provenance.architecture().as_str(),
            p.key.comparable.as_str(),
            provenance.capture_algorithm().as_str(),
            provenance.capture_algorithm_version().as_str(),
            provenance.target_parser_input_context().as_str(),
            provenance.collection_policy().as_str(),
            provenance.collection_policy_version().as_str(),
        ];
        if track.invariant.iter().map(String::as_str).ne(actual) {
            return Err(BaselineError::InvalidReference);
        }
    }
    for n in notes {
        let mut r = Reader::new(&n.canonical_record);
        r.string()?;
        let key = decode_historical_variant_key(&mut r)?;
        r.string()?;
        r.string()?;
        if !detail
            .cases
            .iter()
            .flat_map(|c| &c.variants)
            .any(|v| v.key == key)
        {
            return Err(BaselineError::InvalidReference);
        }
        if let Some(id) = r.optional()? {
            let digest = Sha256Digest::from_bytes(
                id.try_into().map_err(|_| BaselineError::InvalidReference)?,
            );
            if !captures.iter().any(|c| c.id().as_sha256() == digest) {
                return Err(BaselineError::InvalidReference);
            }
        }
        r.finish()?;
    }
    Ok(())
}

fn copy_bytes(bytes: &[u8]) -> Result<Vec<u8>, BaselineError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| BaselineError::Allocation)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use external_test_provenance::{Sha256Digest, sha256};

    use super::*;

    const EMPTY: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-baseline-v1/empty.bin"
    ));
    const SELECTED_ZERO: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-baseline-v1/selected-zero.bin"
    ));
    const SELECTED_EVIDENCE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-baseline-v1/selected-evidence.bin"
    ));
    const ALL_DECLARED: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/contract-vectors/conformance-baseline-v1/all-declared.bin"
    ));

    fn digest(value: &str) -> Sha256Digest {
        Sha256Digest::parse(value).unwrap()
    }

    #[test]
    fn empty_collections_are_known_and_trailing_bytes_fail() {
        assert_eq!(BASELINE_MAX_BYTES_V1, 47_219_283);
        assert!(!matches!(
            decode_baseline_v1(&vec![0; BASELINE_MAX_BYTES_V1]),
            Err(BaselineError::TooLarge)
        ));
        assert!(matches!(
            decode_baseline_v1(&vec![0; BASELINE_MAX_BYTES_V1 + 1]),
            Err(BaselineError::TooLarge)
        ));
    }

    #[test]
    fn independently_authored_baseline_vectors_decode_with_frozen_semantics() {
        let cases = [
            (
                EMPTY,
                "7f0f654304d92d444ba85520e4db49199266d015a3c829bd57004447d4c0926a",
                0,
                0,
                0,
                "none/not-requested",
            ),
            (
                SELECTED_ZERO,
                "f02d459b425e92e91a14e6578f595887ddfbeaa795f6d0f61ae2a33d7c856411",
                0,
                0,
                0,
                "selected-variant-only/completed",
            ),
            (
                SELECTED_EVIDENCE,
                "8a286050bd01ba41da927219e0d639c654fb12073b5103ecb40d5ad242c3d1a9",
                1,
                1,
                1,
                "selected-variant-only/completed",
            ),
            (
                ALL_DECLARED,
                "a5d10338383c1e56346e9a1bada9698bbf382f58e44c15a882ed9a5ac8ec1783",
                1,
                1,
                0,
                "all-declared/completed",
            ),
        ];
        for (bytes, expected_digest, points, evaluated, notes, scope) in cases {
            assert_eq!(sha256(bytes), digest(expected_digest));
            let baseline = decode_baseline_v1(bytes).unwrap();
            assert_eq!(baseline.exact_bytes(), bytes);
            assert_eq!(baseline.points.len(), points);
            assert_eq!(
                baseline
                    .points
                    .iter()
                    .filter(|point| point.result.is_some())
                    .count(),
                evaluated
            );
            assert_eq!(baseline.notes.len(), notes);
            assert_eq!(baseline.advisory_evaluation_scope(), scope);
        }
    }

    #[test]
    fn baseline_advisory_point_keeps_its_frozen_inline_variant_key() {
        let baseline = decode_baseline_v1(SELECTED_EVIDENCE).unwrap();
        let point = baseline.points.first().unwrap();
        let mut record = Reader::new(&point.canonical_record);
        assert_eq!(record.string().unwrap(), "dom-tree-basic-document");
        assert_eq!(record.string().unwrap(), "dom-tree");
        assert_eq!(record.string().unwrap(), "singleton");
        assert_eq!(record.string().unwrap(), "web-observable-dom-tree-v1");
        assert_eq!(record.string().unwrap(), "track");
        assert_eq!(record.take(32).unwrap(), point.capture_id.as_bytes());
        record.finish().unwrap();
    }

    #[test]
    fn evaluation_slots_are_independently_framed_and_fully_consumed() {
        // These offsets are fixed independently in the vector README.
        assert_eq!(&SELECTED_EVIDENCE[43_381..43_389], &27_u64.to_be_bytes());
        assert_eq!(SELECTED_EVIDENCE[43_389], 1);
        assert_eq!(&SELECTED_EVIDENCE[43_390..43_398], &18_u64.to_be_bytes());

        let mut invalid_option = SELECTED_EVIDENCE.to_vec();
        invalid_option[43_389] = 2;
        assert!(matches!(
            decode_baseline_v1(&invalid_option),
            Err(BaselineError::InvalidOption)
        ));

        let mut partial_present = SELECTED_EVIDENCE.to_vec();
        partial_present[43_381..43_389].copy_from_slice(&1_u64.to_be_bytes());
        assert!(matches!(
            decode_baseline_v1(&partial_present),
            Err(BaselineError::PrematureEof)
        ));

        let trailing_slot =
            replace_selected_evidence_slot(&[&SELECTED_EVIDENCE[43_389..43_416], &[0]].concat());
        assert!(matches!(
            decode_baseline_v1(&trailing_slot),
            Err(BaselineError::TrailingBytes)
        ));

        let oversized_slot = replace_selected_evidence_slot(&vec![
            0;
            super::super::result::MAX_ADVISORY_RESULT_SLOT_PAYLOAD_BYTES_V1
                + 1
        ]);
        assert!(matches!(
            decode_baseline_v1(&oversized_slot),
            Err(BaselineError::InvalidEvaluation)
        ));

        assert!(matches!(
            decode_baseline_v1(&SELECTED_EVIDENCE[..SELECTED_EVIDENCE.len() - 1]),
            Err(BaselineError::PrematureEof)
        ));
    }

    #[test]
    fn duplicate_and_dangling_historical_identities_fail_closed() {
        for (tag_offset, body_start, body_end) in [
            (42_095, 42_105, 42_853),
            (42_853, 42_863, 43_058),
            (43_058, 43_068, 43_223),
            (43_416, 43_426, 43_610),
        ] {
            let body = &SELECTED_EVIDENCE[body_start..body_end];
            assert_eq!(&body[..4], &1_u32.to_be_bytes());
            let duplicate = [2_u32.to_be_bytes().as_slice(), &body[4..], &body[4..]].concat();
            let bytes = replace_section_body(
                SELECTED_EVIDENCE,
                tag_offset,
                body_start,
                body_end,
                &duplicate,
            );
            assert!(matches!(
                decode_baseline_v1(&bytes),
                Err(BaselineError::DuplicateIdentity)
            ));
        }

        let mut capture_id_mismatch = SELECTED_EVIDENCE.to_vec();
        capture_id_mismatch[42_117] ^= 1;
        assert!(matches!(
            decode_baseline_v1(&capture_id_mismatch),
            Err(BaselineError::HistoricalCapture(_))
        ));

        let mut unknown_capture = SELECTED_EVIDENCE.to_vec();
        unknown_capture[43_191] ^= 1;
        assert!(matches!(
            decode_baseline_v1(&unknown_capture),
            Err(BaselineError::InvalidReference)
        ));

        let mut unknown_track = SELECTED_EVIDENCE.to_vec();
        replace_in_range(&mut unknown_track, 43_080..43_223, b"track", b"tracx");
        assert!(matches!(
            decode_baseline_v1(&unknown_track),
            Err(BaselineError::InvalidReference)
        ));

        let mut invalid_note_reference = SELECTED_EVIDENCE.to_vec();
        replace_in_range(
            &mut invalid_note_reference,
            43_438..43_610,
            b"dom-tree-basic-document",
            b"dom-tree-basic-documenx",
        );
        assert!(matches!(
            decode_baseline_v1(&invalid_note_reference),
            Err(BaselineError::InvalidReference)
        ));
    }

    #[test]
    fn versions_sections_and_track_invariants_are_closed() {
        let mut wrong_version = EMPTY.to_vec();
        replace_in_range(
            &mut wrong_version,
            46..725,
            b"borrowser-conformance-advisory-result-v1",
            b"borrowser-conformance-advisory-result-v2",
        );
        assert!(matches!(
            decode_baseline_v1(&wrong_version),
            Err(BaselineError::InvalidVersion)
        ));

        let mut wrong_tag = EMPTY.to_vec();
        wrong_tag[725..727].copy_from_slice(&3_u16.to_be_bytes());
        assert!(matches!(
            decode_baseline_v1(&wrong_tag),
            Err(BaselineError::InvalidFormat)
        ));

        let mut invariant_drift = SELECTED_EVIDENCE.to_vec();
        replace_in_range(&mut invariant_drift, 42_875..43_058, b"engine", b"enginx");
        assert!(matches!(
            decode_baseline_v1(&invariant_drift),
            Err(BaselineError::InvalidReference)
        ));
    }

    fn replace_selected_evidence_slot(payload: &[u8]) -> Vec<u8> {
        const SECTION_LENGTH_OFFSET: usize = 43_225;
        const SECTION_BODY_START: usize = 43_233;
        const SLOT_FRAME_START: usize = 43_381;
        const SLOT_FRAME_END: usize = 43_416;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&SELECTED_EVIDENCE[..SLOT_FRAME_START]);
        bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(&SELECTED_EVIDENCE[SLOT_FRAME_END..]);
        let section_length =
            bytes.len() - SECTION_BODY_START - (SELECTED_EVIDENCE.len() - SLOT_FRAME_END);
        bytes[SECTION_LENGTH_OFFSET..SECTION_BODY_START]
            .copy_from_slice(&(section_length as u64).to_be_bytes());
        bytes
    }

    fn replace_section_body(
        source: &[u8],
        tag_offset: usize,
        body_start: usize,
        body_end: usize,
        body: &[u8],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&source[..tag_offset + 2]);
        bytes.extend_from_slice(&(body.len() as u64).to_be_bytes());
        bytes.extend_from_slice(body);
        bytes.extend_from_slice(&source[body_end..]);
        assert_eq!(body_start, tag_offset + 10);
        bytes
    }

    fn replace_in_range(bytes: &mut [u8], range: std::ops::Range<usize>, old: &[u8], new: &[u8]) {
        assert_eq!(old.len(), new.len());
        let offset = bytes[range.clone()]
            .windows(old.len())
            .position(|window| window == old)
            .map(|offset| range.start + offset)
            .unwrap();
        bytes[offset..offset + old.len()].copy_from_slice(new);
    }
}
