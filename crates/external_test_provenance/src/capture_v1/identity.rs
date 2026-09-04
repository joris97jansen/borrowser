use crate::allocation::{
    ProductionReservation, ReservationPolicy, ReservationSite, try_reserve_vec,
};
use crate::{Sha256Digest, sha256};

use super::model::{
    ApplicabilityV1, CaptureV1Error, ControlledFontIdentityV1, ExternalCaptureId,
    ExternalCaptureProvenanceV1, ExternalIdentityV1, ExternalVersionV1, PinnedResourceIdentityV1,
    VerifiedExternalArtifactV1,
};

const DOMAIN: &[u8] = b"borrowser-external-capture-id-v1\0";

pub(super) fn canonical_font_bytes(
    family: &ExternalIdentityV1,
    face_style: &ExternalIdentityV1,
    version: &ExternalVersionV1,
    digest: Sha256Digest,
) -> Result<Vec<u8>, CaptureV1Error> {
    canonical_font_bytes_with_policy(
        family,
        face_style,
        version,
        digest,
        &mut ProductionReservation,
    )
}

fn canonical_font_bytes_with_policy(
    family: &ExternalIdentityV1,
    face_style: &ExternalIdentityV1,
    version: &ExternalVersionV1,
    digest: Sha256Digest,
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    let mut bytes = Vec::new();
    nested_string(&mut bytes, family.as_str(), reservation)?;
    nested_string(&mut bytes, face_style.as_str(), reservation)?;
    nested_string(&mut bytes, version.as_str(), reservation)?;
    extend(&mut bytes, digest.as_bytes(), reservation)?;
    Ok(bytes)
}

pub(super) fn canonical_resource_bytes(
    identity: &ExternalIdentityV1,
    digest: Sha256Digest,
) -> Result<Vec<u8>, CaptureV1Error> {
    canonical_resource_bytes_with_policy(identity, digest, &mut ProductionReservation)
}

fn canonical_resource_bytes_with_policy(
    identity: &ExternalIdentityV1,
    digest: Sha256Digest,
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    let mut bytes = Vec::new();
    nested_string(&mut bytes, identity.as_str(), reservation)?;
    extend(&mut bytes, digest.as_bytes(), reservation)?;
    Ok(bytes)
}

pub(super) fn compute_capture_id(
    provenance: &ExternalCaptureProvenanceV1,
    artifact: &VerifiedExternalArtifactV1,
) -> Result<ExternalCaptureId, CaptureV1Error> {
    Ok(ExternalCaptureId(sha256(&preimage_with_policy(
        provenance,
        artifact,
        &mut ProductionReservation,
    )?)))
}

#[cfg(test)]
fn preimage(
    provenance: &ExternalCaptureProvenanceV1,
    artifact: &VerifiedExternalArtifactV1,
) -> Result<Vec<u8>, CaptureV1Error> {
    preimage_with_policy(provenance, artifact, &mut ProductionReservation)
}

fn preimage_with_policy(
    provenance: &ExternalCaptureProvenanceV1,
    artifact: &VerifiedExternalArtifactV1,
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    let input = &provenance.input;
    let mut output = Vec::new();
    extend(&mut output, DOMAIN, reservation)?;
    tlv(&mut output, 1, provenance.format().as_bytes(), reservation)?;
    tlv(
        &mut output,
        2,
        input.engine_product.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        3,
        input.engine_version.as_str().as_bytes(),
        reservation,
    )?;
    let mut optional = Vec::new();
    match &input.engine_build_revision {
        None => extend(&mut optional, &[0], reservation)?,
        Some(value) => {
            extend(&mut optional, &[1], reservation)?;
            nested_string(&mut optional, value.as_str(), reservation)?;
        }
    }
    tlv(&mut output, 4, &optional, reservation)?;
    tlv(
        &mut output,
        5,
        input.platform_os_family.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        6,
        input.platform_os_version.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        7,
        input.architecture.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        8,
        &applicability(&input.viewport, reservation, |viewport, reservation| {
            let mut bytes = Vec::new();
            extend(&mut bytes, &viewport.width.to_be_bytes(), reservation)?;
            extend(&mut bytes, &viewport.height.to_be_bytes(), reservation)?;
            Ok(bytes)
        })?,
        reservation,
    )?;
    tlv(
        &mut output,
        9,
        &applicability(&input.device_scale, reservation, |scale, reservation| {
            let mut bytes = Vec::new();
            extend(&mut bytes, &scale.numerator().to_be_bytes(), reservation)?;
            extend(&mut bytes, &scale.denominator().to_be_bytes(), reservation)?;
            Ok(bytes)
        })?,
        reservation,
    )?;
    tlv(
        &mut output,
        10,
        &applicability(
            &input.controlled_fonts,
            reservation,
            |fonts, reservation| collection_fonts(fonts, reservation),
        )?,
        reservation,
    )?;
    tlv(
        &mut output,
        11,
        input.resource_network_policy.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        12,
        &collection_resources(&input.pinned_resources, reservation)?,
        reservation,
    )?;
    tlv(
        &mut output,
        13,
        input.fixture_source_project.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        14,
        input.fixture_immutable_revision.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        15,
        input.fixture_content_sha256.as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        16,
        input.capture_mechanism.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        17,
        input.capture_mechanism_version.as_str().as_bytes(),
        reservation,
    )?;
    let mut algorithm = Vec::new();
    nested_string(
        &mut algorithm,
        input.capture_algorithm.as_str(),
        reservation,
    )?;
    nested_string(
        &mut algorithm,
        input.capture_algorithm_version.as_str(),
        reservation,
    )?;
    tlv(&mut output, 18, &algorithm, reservation)?;
    tlv(
        &mut output,
        19,
        input.capture_algorithm_source_sha256.as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        20,
        input.capture_configuration_sha256.as_bytes(),
        reservation,
    )?;
    let mut arguments = Vec::new();
    count(
        &mut arguments,
        input.invocation_arguments.len(),
        reservation,
    )?;
    for argument in &input.invocation_arguments {
        nested_string(&mut arguments, argument, reservation)?;
    }
    tlv(&mut output, 21, &arguments, reservation)?;
    tlv(
        &mut output,
        22,
        input.artifact_format.as_str().as_bytes(),
        reservation,
    )?;
    tlv(
        &mut output,
        23,
        &artifact.utf8_byte_length().to_be_bytes(),
        reservation,
    )?;
    tlv(&mut output, 24, artifact.sha256().as_bytes(), reservation)?;
    tlv(
        &mut output,
        25,
        input.target_parser_input_context.as_str().as_bytes(),
        reservation,
    )?;
    let mut policy = Vec::new();
    nested_string(&mut policy, input.collection_policy.as_str(), reservation)?;
    nested_string(
        &mut policy,
        input.collection_policy_version.as_str(),
        reservation,
    )?;
    tlv(&mut output, 26, &policy, reservation)?;
    Ok(output)
}

fn applicability<T, P: ReservationPolicy>(
    value: &ApplicabilityV1<T>,
    reservation: &mut P,
    applicable: impl FnOnce(&T, &mut P) -> Result<Vec<u8>, CaptureV1Error>,
) -> Result<Vec<u8>, CaptureV1Error> {
    let mut bytes = Vec::new();
    match value {
        ApplicabilityV1::NotApplicable(reason) => {
            extend(&mut bytes, &[0], reservation)?;
            nested_string(&mut bytes, reason.as_str(), reservation)?;
        }
        ApplicabilityV1::Applicable(value) => {
            extend(&mut bytes, &[1], reservation)?;
            let value = applicable(value, reservation)?;
            nested_bytes(&mut bytes, &value, reservation)?;
        }
    }
    Ok(bytes)
}

fn collection_fonts(
    fonts: &[ControlledFontIdentityV1],
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    collection(
        fonts.iter().map(ControlledFontIdentityV1::canonical_bytes),
        reservation,
    )
}

fn collection_resources(
    resources: &[PinnedResourceIdentityV1],
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    collection(
        resources
            .iter()
            .map(PinnedResourceIdentityV1::canonical_bytes),
        reservation,
    )
}

fn collection<'a>(
    items: impl ExactSizeIterator<Item = &'a [u8]>,
    reservation: &mut impl ReservationPolicy,
) -> Result<Vec<u8>, CaptureV1Error> {
    let mut bytes = Vec::new();
    count(&mut bytes, items.len(), reservation)?;
    for item in items {
        nested_bytes(&mut bytes, item, reservation)?;
    }
    Ok(bytes)
}

fn count(
    output: &mut Vec<u8>,
    value: usize,
    reservation: &mut impl ReservationPolicy,
) -> Result<(), CaptureV1Error> {
    let value = u32::try_from(value).map_err(|_| CaptureV1Error::LengthOverflow)?;
    extend(output, &value.to_be_bytes(), reservation)
}

fn tlv(
    output: &mut Vec<u8>,
    tag: u16,
    payload: &[u8],
    reservation: &mut impl ReservationPolicy,
) -> Result<(), CaptureV1Error> {
    extend(output, &tag.to_be_bytes(), reservation)?;
    nested_bytes(output, payload, reservation)
}

fn nested_string(
    output: &mut Vec<u8>,
    value: &str,
    reservation: &mut impl ReservationPolicy,
) -> Result<(), CaptureV1Error> {
    nested_bytes(output, value.as_bytes(), reservation)
}

fn nested_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
    reservation: &mut impl ReservationPolicy,
) -> Result<(), CaptureV1Error> {
    let length = u64::try_from(value.len()).map_err(|_| CaptureV1Error::LengthOverflow)?;
    extend(output, &length.to_be_bytes(), reservation)?;
    extend(output, value, reservation)
}

fn extend(
    output: &mut Vec<u8>,
    bytes: &[u8],
    reservation: &mut impl ReservationPolicy,
) -> Result<(), CaptureV1Error> {
    output
        .len()
        .checked_add(bytes.len())
        .ok_or(CaptureV1Error::LengthOverflow)?;
    try_reserve_vec(
        output,
        bytes.len(),
        ReservationSite::CanonicalIdentity,
        reservation,
    )
    .map_err(|_| CaptureV1Error::Allocation)?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::{RejectReservationAt, ReservationSite};
    use crate::capture_v1::model::{
        ExternalArtifactCandidateV1, ExternalArtifactFormatV1, ExternalCaptureIdClaim,
        ExternalCaptureProvenanceV1Input, ReducedDeviceScaleV1, ResourceNetworkPolicyV1,
        TargetParserInputContextV1, ViewportCssPixelsV1,
    };
    use crate::capture_v1::{ExternalIdentityV1, ExternalVersionV1};

    fn text(value: &str) -> ExternalIdentityV1 {
        ExternalIdentityV1::parse(value).unwrap()
    }
    fn version(value: &str) -> ExternalVersionV1 {
        ExternalVersionV1::parse(value).unwrap()
    }
    fn provenance(artifact: &[u8]) -> ExternalCaptureProvenanceV1 {
        let digest = sha256(artifact);
        ExternalCaptureProvenanceV1::try_from_input(ExternalCaptureProvenanceV1Input {
            engine_product: text("engine"),
            engine_version: version("1"),
            engine_build_revision: None,
            platform_os_family: text("os"),
            platform_os_version: version("1"),
            architecture: text("arch"),
            viewport: ApplicabilityV1::Applicable(ViewportCssPixelsV1 {
                width: 800,
                height: 600,
            }),
            device_scale: ApplicabilityV1::NotApplicable(
                super::super::model::NonApplicableReasonV1::parse("not-used").unwrap(),
            ),
            controlled_fonts: ApplicabilityV1::NotApplicable(
                super::super::model::NonApplicableReasonV1::parse("font-independent").unwrap(),
            ),
            resource_network_policy: ResourceNetworkPolicyV1::Offline,
            pinned_resources: vec![],
            fixture_source_project: text("fixture"),
            fixture_immutable_revision: crate::ImmutableRevision::parse("revision").unwrap(),
            fixture_content_sha256: digest,
            capture_mechanism: text("tool"),
            capture_mechanism_version: version("1"),
            capture_algorithm: text("algorithm"),
            capture_algorithm_version: version("1"),
            capture_algorithm_source_sha256: digest,
            capture_configuration_sha256: digest,
            invocation_arguments: vec!["--one".into(), "--one".into()],
            artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
            artifact_utf8_byte_length: artifact.len() as u64,
            artifact_sha256: digest,
            target_parser_input_context:
                TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
            collection_policy: text("stable"),
            collection_policy_version: version("1"),
        })
        .unwrap()
    }

    fn verified_artifact(artifact: &[u8]) -> VerifiedExternalArtifactV1 {
        ExternalArtifactCandidateV1::from_bytes(artifact.to_vec())
            .unwrap()
            .validate(ExternalArtifactFormatV1::WebObservableDomTreeV1)
            .unwrap()
    }

    fn digest(byte: u8) -> Sha256Digest {
        sha256(&[byte])
    }

    #[test]
    fn canonical_identity_reservation_failure_is_deterministic() {
        assert_eq!(
            canonical_font_bytes_with_policy(
                &text("font"),
                &text("regular"),
                &version("1"),
                digest(1),
                &mut RejectReservationAt::new(ReservationSite::CanonicalIdentity),
            ),
            Err(CaptureV1Error::Allocation)
        );

        let artifact = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";
        assert_eq!(
            preimage_with_policy(
                &provenance(artifact),
                &verified_artifact(artifact),
                &mut RejectReservationAt::new(ReservationSite::CanonicalIdentity),
            ),
            Err(CaptureV1Error::Allocation)
        );
    }

    #[test]
    fn preimage_has_exact_domain_and_twenty_six_ordered_tags() {
        let bytes = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n".to_vec();
        let verified = verified_artifact(&bytes);
        let preimage = preimage(&provenance(&bytes), &verified).unwrap();
        assert!(preimage.starts_with(DOMAIN));
        let mut offset = DOMAIN.len();
        for expected in 1_u16..=26 {
            assert_eq!(
                u16::from_be_bytes([preimage[offset], preimage[offset + 1]]),
                expected
            );
            let length =
                u64::from_be_bytes(preimage[offset + 2..offset + 10].try_into().unwrap()) as usize;
            offset += 10 + length;
        }
        assert_eq!(offset, preimage.len());
        assert_eq!(
            sha256(&preimage).to_hex(),
            "4179e64c74adbe3d558f24aeab8ee011cf552ad39c7d467a4a774ee49ed404c8"
        );
        let computed = compute_capture_id(&provenance(&bytes), &verified).unwrap();
        assert_eq!(
            computed.to_string(),
            format!("sha256:{}", sha256(&preimage))
        );
        assert_ne!(
            ExternalCaptureIdClaim::parse(&computed.to_string())
                .unwrap()
                .as_sha256(),
            sha256(b"other")
        );
    }

    #[test]
    fn every_variable_identity_field_changes_the_preimage_and_id() {
        let artifact = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";
        let base = provenance(artifact);
        let verified = verified_artifact(artifact);
        let base_preimage = preimage(&base, &verified).unwrap();
        let base_id = compute_capture_id(&base, &verified).unwrap();

        macro_rules! changed {
            ($mutation:expr) => {{
                let mut candidate = base.clone();
                $mutation(&mut candidate.input);
                assert_ne!(preimage(&candidate, &verified).unwrap(), base_preimage);
                assert_ne!(compute_capture_id(&candidate, &verified).unwrap(), base_id);
            }};
        }

        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.engine_product =
                text("other-engine")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.engine_version = version("2")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.engine_build_revision =
                Some(text("build"))
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.platform_os_family =
                text("other-os")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.platform_os_version = version("2")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.architecture = text("other-arch")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.viewport =
                ApplicabilityV1::Applicable(ViewportCssPixelsV1 {
                    width: 801,
                    height: 600,
                })
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.device_scale =
                ApplicabilityV1::NotApplicable(
                    super::super::model::NonApplicableReasonV1::parse("different").unwrap(),
                )
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.controlled_fonts =
                ApplicabilityV1::NotApplicable(
                    super::super::model::NonApplicableReasonV1::parse("different-font-policy")
                        .unwrap(),
                )
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.resource_network_policy =
                ResourceNetworkPolicyV1::FixtureLocalOnly
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.pinned_resources =
                vec![PinnedResourceIdentityV1::new(text("resource"), digest(1)).unwrap(),]
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.fixture_source_project =
                text("other-fixture")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.fixture_immutable_revision =
                crate::ImmutableRevision::parse("other-revision").unwrap()
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.fixture_content_sha256 = digest(2)
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.capture_mechanism =
                text("other-tool")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.capture_mechanism_version =
                version("2")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.capture_algorithm =
                text("other-algorithm")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.capture_algorithm_version =
                version("2")
        );
        changed!(|input: &mut ExternalCaptureProvenanceV1Input| input
            .capture_algorithm_source_sha256 =
            digest(3));
        changed!(|input: &mut ExternalCaptureProvenanceV1Input| input
            .capture_configuration_sha256 =
            digest(4));
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.invocation_arguments =
                vec!["--two".into(), "--one".into()]
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.collection_policy =
                text("other-policy")
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.collection_policy_version =
                version("2")
        );

        let other_artifact = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 1\nnode-begin = \"comment\"\ndata = \"x\"\nnode-end = \"comment\"\nnode-end = \"document\"\n";
        let other_verified = verified_artifact(other_artifact);
        assert_ne!(
            other_verified.utf8_byte_length(),
            verified.utf8_byte_length()
        );
        assert_ne!(other_verified.sha256(), verified.sha256());
        assert_ne!(compute_capture_id(&base, &other_verified).unwrap(), base_id);
    }

    #[test]
    fn unordered_sets_are_canonical_but_arguments_are_ordered_and_repeated() {
        let artifact = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";
        let verified = verified_artifact(artifact);
        let font_a =
            ControlledFontIdentityV1::new(text("a"), text("regular"), version("1"), digest(1))
                .unwrap();
        let font_b =
            ControlledFontIdentityV1::new(text("b"), text("regular"), version("1"), digest(2))
                .unwrap();
        let resource_a = PinnedResourceIdentityV1::new(text("a"), digest(3)).unwrap();
        let resource_b = PinnedResourceIdentityV1::new(text("b"), digest(4)).unwrap();
        let mut first = provenance(artifact).input;
        first.controlled_fonts = ApplicabilityV1::Applicable(vec![font_a.clone(), font_b.clone()]);
        first.pinned_resources = vec![resource_a.clone(), resource_b.clone()];
        let mut second = first.clone();
        second.controlled_fonts = ApplicabilityV1::Applicable(vec![font_b, font_a.clone()]);
        second.pinned_resources = vec![resource_b, resource_a.clone()];
        let first = ExternalCaptureProvenanceV1::try_from_input(first).unwrap();
        let second = ExternalCaptureProvenanceV1::try_from_input(second).unwrap();
        assert_eq!(
            compute_capture_id(&first, &verified).unwrap(),
            compute_capture_id(&second, &verified).unwrap()
        );

        let mut reordered = first.input.clone();
        reordered.invocation_arguments = vec!["--one".into(), "--two".into()];
        let reordered = ExternalCaptureProvenanceV1::try_from_input(reordered).unwrap();
        assert_ne!(
            compute_capture_id(&first, &verified).unwrap(),
            compute_capture_id(&reordered, &verified).unwrap()
        );
        let mut no_repeat = first.input.clone();
        no_repeat.invocation_arguments = vec!["--one".into()];
        let no_repeat = ExternalCaptureProvenanceV1::try_from_input(no_repeat).unwrap();
        assert_ne!(
            compute_capture_id(&first, &verified).unwrap(),
            compute_capture_id(&no_repeat, &verified).unwrap()
        );

        let mut duplicate_font = first.input.clone();
        duplicate_font.controlled_fonts = ApplicabilityV1::Applicable(vec![font_a.clone(), font_a]);
        assert!(matches!(
            ExternalCaptureProvenanceV1::try_from_input(duplicate_font),
            Err(CaptureV1Error::DuplicateControlledFont)
        ));
        let mut duplicate_resource = first.input;
        duplicate_resource.pinned_resources = vec![resource_a.clone(), resource_a];
        assert!(matches!(
            ExternalCaptureProvenanceV1::try_from_input(duplicate_resource),
            Err(CaptureV1Error::DuplicatePinnedResource)
        ));
    }

    #[test]
    fn every_nested_identity_component_is_independently_identity_bearing() {
        let artifact = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";
        let verified = verified_artifact(artifact);
        let font = |family, style, version_value, digest_value| {
            ControlledFontIdentityV1::new(
                text(family),
                text(style),
                version(version_value),
                digest(digest_value),
            )
            .unwrap()
        };
        let resource = |identity, digest_value| {
            PinnedResourceIdentityV1::new(text(identity), digest(digest_value)).unwrap()
        };
        let mut input = provenance(artifact).input;
        input.device_scale = ApplicabilityV1::Applicable(ReducedDeviceScaleV1::new(1, 1).unwrap());
        input.controlled_fonts = ApplicabilityV1::Applicable(vec![font("a", "regular", "1", 1)]);
        input.pinned_resources = vec![resource("resource", 2)];
        let base = ExternalCaptureProvenanceV1::try_from_input(input).unwrap();
        let base_id = compute_capture_id(&base, &verified).unwrap();

        macro_rules! changed {
            ($mutation:expr) => {{
                let mut candidate = base.input.clone();
                $mutation(&mut candidate);
                let candidate = ExternalCaptureProvenanceV1::try_from_input(candidate).unwrap();
                assert_ne!(compute_capture_id(&candidate, &verified).unwrap(), base_id);
            }};
        }

        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.viewport =
                ApplicabilityV1::Applicable(ViewportCssPixelsV1 {
                    width: 800,
                    height: 601,
                })
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.device_scale =
                ApplicabilityV1::Applicable(ReducedDeviceScaleV1::new(2, 1).unwrap())
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.device_scale =
                ApplicabilityV1::Applicable(ReducedDeviceScaleV1::new(1, 2).unwrap())
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.controlled_fonts =
                ApplicabilityV1::Applicable(vec![font("b", "regular", "1", 1)])
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.controlled_fonts =
                ApplicabilityV1::Applicable(vec![font("a", "italic", "1", 1)])
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.controlled_fonts =
                ApplicabilityV1::Applicable(vec![font("a", "regular", "2", 1)])
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.controlled_fonts =
                ApplicabilityV1::Applicable(vec![font("a", "regular", "1", 3)])
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.pinned_resources =
                vec![resource("other-resource", 2)]
        );
        changed!(
            |input: &mut ExternalCaptureProvenanceV1Input| input.pinned_resources =
                vec![resource("resource", 3)]
        );
    }
}
