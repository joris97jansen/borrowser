use crate::{ImmutableRevision, Sha256Digest, sha256};

use super::identity::{
    DOMAIN, MAX_EXTERNAL_CAPTURE_ID_PREIMAGE_BYTES_V1, canonical_capture_id_preimage,
};
use super::model::{
    ApplicabilityV1, CaptureV1Error, ControlledFontIdentityV1, ExternalArtifactFormatV1,
    ExternalCaptureId, ExternalCaptureIdClaim, ExternalCaptureProvenanceV1,
    ExternalCaptureProvenanceV1Input, ExternalIdentityV1, ExternalVersionV1, NonApplicableReasonV1,
    PinnedResourceIdentityV1, ReducedDeviceScaleV1, ResourceNetworkPolicyV1,
    TargetParserInputContextV1, ViewportCssPixelsV1,
};
use super::{
    EXTERNAL_CAPTURE_PROVENANCE_FORMAT_V1, TARGET_PARSER_INPUT_CONTEXT_V1,
    WEB_OBSERVABLE_DOM_TREE_FORMAT_V1,
};

/// A historical capture identity whose claim is consistent with canonical
/// provenance and recorded artifact identity. It does not establish that the
/// artifact bytes were read or verified in this process.
///
/// Historical claim validation cannot be elevated to live capture authority:
/// ```compile_fail
/// use external_test_provenance::{HistoricalCaptureIdentityV1, ValidatedExternalCaptureV1};
/// fn elevate(historical: HistoricalCaptureIdentityV1) -> ValidatedExternalCaptureV1 {
///     historical.into()
/// }
/// ```
pub struct HistoricalCaptureIdentityV1 {
    id: ExternalCaptureId,
    provenance: ExternalCaptureProvenanceV1,
    canonical_preimage: Vec<u8>,
}

impl HistoricalCaptureIdentityV1 {
    pub const fn id(&self) -> ExternalCaptureId {
        self.id
    }
    pub const fn provenance(&self) -> &ExternalCaptureProvenanceV1 {
        &self.provenance
    }
    pub fn canonical_preimage(&self) -> &[u8] {
        &self.canonical_preimage
    }
    pub const fn recorded_artifact_utf8_byte_length(&self) -> u64 {
        self.provenance.declared_artifact_utf8_byte_length()
    }
    pub const fn recorded_artifact_sha256(&self) -> Sha256Digest {
        self.provenance.declared_artifact_sha256()
    }
}

pub fn canonical_capture_id_preimage_v1(
    provenance: &ExternalCaptureProvenanceV1,
) -> Result<Vec<u8>, CaptureV1Error> {
    canonical_capture_id_preimage(provenance)
}

pub fn validate_historical_capture_identity_v1(
    bytes: &[u8],
    claim: ExternalCaptureIdClaim,
) -> Result<HistoricalCaptureIdentityV1, CaptureV1Error> {
    if bytes.len() > MAX_EXTERNAL_CAPTURE_ID_PREIMAGE_BYTES_V1 || !bytes.starts_with(DOMAIN) {
        return Err(CaptureV1Error::InvalidHistoricalPreimage);
    }
    let mut reader = Reader::new(&bytes[DOMAIN.len()..]);
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(26)
        .map_err(|_| CaptureV1Error::Allocation)?;
    for expected in 1..=26_u16 {
        let tag = reader.u16()?;
        if tag != expected {
            return Err(CaptureV1Error::InvalidHistoricalPreimage);
        }
        fields.push(reader.owned_bytes()?);
    }
    reader.finish()?;

    let string = |index: usize| parse_utf8(&fields[index]);
    if string(0)? != EXTERNAL_CAPTURE_PROVENANCE_FORMAT_V1
        || string(21)? != WEB_OBSERVABLE_DOM_TREE_FORMAT_V1
        || string(24)? != TARGET_PARSER_INPUT_CONTEXT_V1
    {
        return Err(CaptureV1Error::InvalidHistoricalPreimage);
    }
    let digest = |index: usize| parse_digest(&fields[index]);
    let input = ExternalCaptureProvenanceV1Input {
        engine_product: ExternalIdentityV1::parse(string(1)?)?,
        engine_version: ExternalVersionV1::parse(string(2)?)?,
        engine_build_revision: parse_optional_identity(&fields[3])?,
        platform_os_family: ExternalIdentityV1::parse(string(4)?)?,
        platform_os_version: ExternalVersionV1::parse(string(5)?)?,
        architecture: ExternalIdentityV1::parse(string(6)?)?,
        viewport: parse_viewport(&fields[7])?,
        device_scale: parse_scale(&fields[8])?,
        controlled_fonts: parse_fonts(&fields[9])?,
        resource_network_policy: match string(10)? {
            "offline" => ResourceNetworkPolicyV1::Offline,
            "fixture-local-only" => ResourceNetworkPolicyV1::FixtureLocalOnly,
            "recorded-local-closure" => ResourceNetworkPolicyV1::RecordedLocalClosure,
            _ => return Err(CaptureV1Error::InvalidHistoricalPreimage),
        },
        pinned_resources: parse_resources(&fields[11])?,
        fixture_source_project: ExternalIdentityV1::parse(string(12)?)?,
        fixture_immutable_revision: ImmutableRevision::parse_fallible(string(13)?).map_err(
            |error| match error {
                crate::identity::FallibleRevisionParseError::Validation(_) => {
                    CaptureV1Error::InvalidHistoricalPreimage
                }
                crate::identity::FallibleRevisionParseError::Allocation => {
                    CaptureV1Error::Allocation
                }
            },
        )?,
        fixture_content_sha256: digest(14)?,
        capture_mechanism: ExternalIdentityV1::parse(string(15)?)?,
        capture_mechanism_version: ExternalVersionV1::parse(string(16)?)?,
        capture_algorithm: parse_pair(&fields[17])?.0,
        capture_algorithm_version: parse_pair(&fields[17])?.1,
        capture_algorithm_source_sha256: digest(18)?,
        capture_configuration_sha256: digest(19)?,
        invocation_arguments: parse_arguments(&fields[20])?,
        artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
        artifact_utf8_byte_length: parse_u64(&fields[22])?,
        artifact_sha256: digest(23)?,
        target_parser_input_context:
            TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
        collection_policy: parse_pair(&fields[25])?.0,
        collection_policy_version: parse_pair(&fields[25])?.1,
    };
    let provenance = ExternalCaptureProvenanceV1::try_from_input(input)?;
    let canonical = canonical_capture_id_preimage(&provenance)?;
    if canonical != bytes {
        return Err(CaptureV1Error::NonCanonicalHistoricalPreimage);
    }
    let computed = sha256(&canonical);
    if computed != claim.as_sha256() {
        return Err(CaptureV1Error::CaptureIdMismatch);
    }
    Ok(HistoricalCaptureIdentityV1 {
        id: ExternalCaptureId(computed),
        provenance,
        canonical_preimage: canonical,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], CaptureV1Error> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(CaptureV1Error::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CaptureV1Error::InvalidHistoricalPreimage)?;
        self.offset = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, CaptureV1Error> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    fn u32(&mut self) -> Result<u32, CaptureV1Error> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    fn u64(&mut self) -> Result<u64, CaptureV1Error> {
        let bytes = self.take(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
    fn bytes(&mut self) -> Result<&'a [u8], CaptureV1Error> {
        let n = usize::try_from(self.u64()?).map_err(|_| CaptureV1Error::LengthOverflow)?;
        self.take(n)
    }
    fn owned_bytes(&mut self) -> Result<Vec<u8>, CaptureV1Error> {
        let value = self.bytes()?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| CaptureV1Error::Allocation)?;
        owned.extend_from_slice(value);
        Ok(owned)
    }
    fn finish(&self) -> Result<(), CaptureV1Error> {
        (self.offset == self.bytes.len())
            .then_some(())
            .ok_or(CaptureV1Error::InvalidHistoricalPreimage)
    }
}

fn parse_utf8(bytes: &[u8]) -> Result<&str, CaptureV1Error> {
    std::str::from_utf8(bytes).map_err(|_| CaptureV1Error::InvalidHistoricalPreimage)
}
fn parse_digest(bytes: &[u8]) -> Result<Sha256Digest, CaptureV1Error> {
    let value: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CaptureV1Error::InvalidHistoricalPreimage)?;
    Ok(Sha256Digest::from_bytes(value))
}
fn parse_u64(bytes: &[u8]) -> Result<u64, CaptureV1Error> {
    Ok(u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| CaptureV1Error::InvalidHistoricalPreimage)?,
    ))
}
fn parse_nested_string<'a>(r: &mut Reader<'a>) -> Result<&'a str, CaptureV1Error> {
    parse_utf8(r.bytes()?)
}
fn parse_optional_identity(bytes: &[u8]) -> Result<Option<ExternalIdentityV1>, CaptureV1Error> {
    let mut r = Reader::new(bytes);
    let value = match r.take(1)?[0] {
        0 => None,
        1 => Some(ExternalIdentityV1::parse(parse_nested_string(&mut r)?)?),
        _ => return Err(CaptureV1Error::InvalidHistoricalPreimage),
    };
    r.finish()?;
    Ok(value)
}
fn parse_not_applicable<T>(
    bytes: &[u8],
    applicable: impl FnOnce(&mut Reader<'_>) -> Result<T, CaptureV1Error>,
) -> Result<ApplicabilityV1<T>, CaptureV1Error> {
    let mut r = Reader::new(bytes);
    let value = match r.take(1)?[0] {
        0 => ApplicabilityV1::NotApplicable(NonApplicableReasonV1::parse(parse_nested_string(
            &mut r,
        )?)?),
        1 => {
            let nested = r.bytes()?;
            let mut n = Reader::new(nested);
            let value = applicable(&mut n)?;
            n.finish()?;
            ApplicabilityV1::Applicable(value)
        }
        _ => return Err(CaptureV1Error::InvalidHistoricalPreimage),
    };
    r.finish()?;
    Ok(value)
}
fn parse_viewport(bytes: &[u8]) -> Result<ApplicabilityV1<ViewportCssPixelsV1>, CaptureV1Error> {
    parse_not_applicable(bytes, |r| {
        Ok(ViewportCssPixelsV1 {
            width: r.u32()?,
            height: r.u32()?,
        })
    })
}
fn parse_scale(bytes: &[u8]) -> Result<ApplicabilityV1<ReducedDeviceScaleV1>, CaptureV1Error> {
    parse_not_applicable(bytes, |r| ReducedDeviceScaleV1::new(r.u32()?, r.u32()?))
}
fn parse_fonts(
    bytes: &[u8],
) -> Result<ApplicabilityV1<Vec<ControlledFontIdentityV1>>, CaptureV1Error> {
    parse_not_applicable(bytes, |r| {
        let count = usize::try_from(r.u32()?).map_err(|_| CaptureV1Error::LengthOverflow)?;
        if count == 0 || count > 16 {
            return Err(CaptureV1Error::InvalidHistoricalPreimage);
        }
        let mut out = Vec::new();
        out.try_reserve_exact(count)
            .map_err(|_| CaptureV1Error::Allocation)?;
        for _ in 0..count {
            let mut i = Reader::new(r.bytes()?);
            out.push(ControlledFontIdentityV1::new(
                ExternalIdentityV1::parse(parse_nested_string(&mut i)?)?,
                ExternalIdentityV1::parse(parse_nested_string(&mut i)?)?,
                ExternalVersionV1::parse(parse_nested_string(&mut i)?)?,
                parse_digest(i.take(32)?)?,
            )?);
            i.finish()?;
        }
        Ok(out)
    })
}
fn parse_resources(bytes: &[u8]) -> Result<Vec<PinnedResourceIdentityV1>, CaptureV1Error> {
    let mut r = Reader::new(bytes);
    let count = usize::try_from(r.u32()?).map_err(|_| CaptureV1Error::LengthOverflow)?;
    if count > 32 {
        return Err(CaptureV1Error::InvalidHistoricalPreimage);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| CaptureV1Error::Allocation)?;
    for _ in 0..count {
        let mut i = Reader::new(r.bytes()?);
        out.push(PinnedResourceIdentityV1::new(
            ExternalIdentityV1::parse(parse_nested_string(&mut i)?)?,
            parse_digest(i.take(32)?)?,
        )?);
        i.finish()?;
    }
    r.finish()?;
    Ok(out)
}
fn parse_arguments(bytes: &[u8]) -> Result<Vec<String>, CaptureV1Error> {
    let mut r = Reader::new(bytes);
    let count = usize::try_from(r.u32()?).map_err(|_| CaptureV1Error::LengthOverflow)?;
    if count > 16 {
        return Err(CaptureV1Error::InvalidHistoricalPreimage);
    }
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| CaptureV1Error::Allocation)?;
    for _ in 0..count {
        let value = parse_nested_string(&mut r)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| CaptureV1Error::Allocation)?;
        owned.push_str(value);
        out.push(owned);
    }
    r.finish()?;
    Ok(out)
}
fn parse_pair(bytes: &[u8]) -> Result<(ExternalIdentityV1, ExternalVersionV1), CaptureV1Error> {
    let mut r = Reader::new(bytes);
    let a = ExternalIdentityV1::parse(parse_nested_string(&mut r)?)?;
    let b = ExternalVersionV1::parse(parse_nested_string(&mut r)?)?;
    r.finish()?;
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance() -> ExternalCaptureProvenanceV1 {
        provenance_with_resources(Vec::new())
    }

    fn provenance_with_resources(
        pinned_resources: Vec<PinnedResourceIdentityV1>,
    ) -> ExternalCaptureProvenanceV1 {
        let digest = sha256(b"artifact");
        ExternalCaptureProvenanceV1::try_from_input(ExternalCaptureProvenanceV1Input {
            engine_product: ExternalIdentityV1::parse("engine").unwrap(),
            engine_version: ExternalVersionV1::parse("1").unwrap(),
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
            pinned_resources,
            fixture_source_project: ExternalIdentityV1::parse("fixture").unwrap(),
            fixture_immutable_revision: ImmutableRevision::parse("revision").unwrap(),
            fixture_content_sha256: digest,
            capture_mechanism: ExternalIdentityV1::parse("tool").unwrap(),
            capture_mechanism_version: ExternalVersionV1::parse("1").unwrap(),
            capture_algorithm: ExternalIdentityV1::parse("algorithm").unwrap(),
            capture_algorithm_version: ExternalVersionV1::parse("1").unwrap(),
            capture_algorithm_source_sha256: digest,
            capture_configuration_sha256: digest,
            invocation_arguments: vec!["--one".to_owned(), "--one".to_owned()],
            artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
            artifact_utf8_byte_length: 8,
            artifact_sha256: digest,
            target_parser_input_context:
                TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
            collection_policy: ExternalIdentityV1::parse("stable").unwrap(),
            collection_policy_version: ExternalVersionV1::parse("1").unwrap(),
        })
        .unwrap()
    }

    #[test]
    fn historical_validation_round_trips_the_live_canonical_identity() {
        let provenance = provenance();
        let bytes = canonical_capture_id_preimage_v1(&provenance).unwrap();
        let digest = sha256(&bytes);
        let claim = ExternalCaptureIdClaim::parse(&format!("sha256:{digest}")).unwrap();
        let historical = validate_historical_capture_identity_v1(&bytes, claim).unwrap();
        assert_eq!(historical.id().as_sha256(), digest);
        assert_eq!(historical.canonical_preimage(), bytes);
        assert_eq!(historical.recorded_artifact_utf8_byte_length(), 8);
        assert_eq!(historical.recorded_artifact_sha256(), sha256(b"artifact"));
    }

    #[test]
    fn historical_validation_rejects_malformed_noncanonical_and_wrong_claims() {
        let bytes = canonical_capture_id_preimage_v1(&provenance()).unwrap();
        let wrong = ExternalCaptureIdClaim::parse(&format!("sha256:{}", sha256(b"wrong"))).unwrap();
        assert_eq!(
            validate_historical_capture_identity_v1(&bytes, wrong).err(),
            Some(CaptureV1Error::CaptureIdMismatch)
        );

        let claim = ExternalCaptureIdClaim::parse(&format!("sha256:{}", sha256(&bytes))).unwrap();
        assert_eq!(
            validate_historical_capture_identity_v1(&bytes[..bytes.len() - 1], claim).err(),
            Some(CaptureV1Error::InvalidHistoricalPreimage)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            validate_historical_capture_identity_v1(&trailing, claim).err(),
            Some(CaptureV1Error::InvalidHistoricalPreimage)
        );
        let mut bad_tag = bytes;
        bad_tag[DOMAIN.len()] = 0xff;
        assert_eq!(
            validate_historical_capture_identity_v1(&bad_tag, claim).err(),
            Some(CaptureV1Error::InvalidHistoricalPreimage)
        );

        let resources = vec![
            PinnedResourceIdentityV1::new(
                ExternalIdentityV1::parse("resource-a").unwrap(),
                sha256(b"a"),
            )
            .unwrap(),
            PinnedResourceIdentityV1::new(
                ExternalIdentityV1::parse("resource-b").unwrap(),
                sha256(b"b"),
            )
            .unwrap(),
        ];
        let canonical =
            canonical_capture_id_preimage_v1(&provenance_with_resources(resources)).unwrap();
        let claim = ExternalCaptureIdClaim::from_sha256(sha256(&canonical));
        let mut field_offset = DOMAIN.len();
        let mut field_twelve = None;
        for expected in 1..=12_u16 {
            assert_eq!(
                u16::from_be_bytes(
                    canonical[field_offset..field_offset + 2]
                        .try_into()
                        .unwrap()
                ),
                expected
            );
            field_offset += 2;
            let length = u64::from_be_bytes(
                canonical[field_offset..field_offset + 8]
                    .try_into()
                    .unwrap(),
            ) as usize;
            field_offset += 8;
            let range = field_offset..field_offset + length;
            if expected == 12 {
                field_twelve = Some(range.clone());
            }
            field_offset = range.end;
        }
        let range = field_twelve.unwrap();
        let body = &canonical[range.clone()];
        assert_eq!(&body[..4], &2_u32.to_be_bytes());
        let first_length = u64::from_be_bytes(body[4..12].try_into().unwrap()) as usize + 8;
        let first = 4..4 + first_length;
        let second = first.end..body.len();
        let mut reordered = canonical.clone();
        reordered.splice(
            range,
            [2_u32.to_be_bytes().as_slice(), &body[second], &body[first]].concat(),
        );
        assert_eq!(
            validate_historical_capture_identity_v1(&reordered, claim).err(),
            Some(CaptureV1Error::NonCanonicalHistoricalPreimage)
        );
    }

    #[test]
    fn capture_preimage_ceiling_is_the_reviewed_exact_value() {
        assert_eq!(MAX_EXTERNAL_CAPTURE_ID_PREIMAGE_BYTES_V1, 33_920);
    }

    #[test]
    fn independently_authored_capture_identity_vector_validates() {
        const VECTOR: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/contract-vectors/external-capture-id-v1/representative.bin"
        ));
        let expected =
            Sha256Digest::parse("426af2e1da5e99c718e4792661dbc253a09fc69656bffb039b0f0a67b7ac0bb5")
                .unwrap();
        assert_eq!(VECTOR.len(), 696);
        assert_eq!(sha256(VECTOR), expected);

        let historical = validate_historical_capture_identity_v1(
            VECTOR,
            ExternalCaptureIdClaim::from_sha256(expected),
        )
        .unwrap();
        assert_eq!(historical.id().as_sha256(), expected);
        assert_eq!(historical.canonical_preimage(), VECTOR);
        assert_eq!(
            canonical_capture_id_preimage_v1(historical.provenance()).unwrap(),
            VECTOR
        );
        assert_eq!(historical.provenance().engine_product().as_str(), "engine");
        assert_eq!(historical.provenance().engine_version().as_str(), "1");
        assert_eq!(historical.recorded_artifact_utf8_byte_length(), 17);
        assert_eq!(
            historical.recorded_artifact_sha256(),
            Sha256Digest::parse("f0d26b6830a6d879a8ed3e8d91459c70eba189eb541583b42c6af756411a8d01")
                .unwrap()
        );
    }
}
