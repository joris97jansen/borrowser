use conformance_test_support::{
    ExternalAdapterVersion, ExternalLineageId, FixtureSource, HarnessFeatureId, InventoryScope,
    ReconciledExternalFixtureLineages, SourceKind, SourceRecordId, TestId, ValidatedFixture,
};
use external_test_provenance::{Sha256Digest, sha256};

pub const AGGREGATE_LOGICAL_CASE_MEMBER_IDENTITY_V1: &str =
    "borrowser-conformance-logical-case-member-v1";
pub const AGGREGATE_LOGICAL_CASE_SOURCE_SET_IDENTITY_V1: &str =
    "borrowser-conformance-logical-case-source-set-v1";

const MEMBER_DOMAIN: &[u8] = b"borrowser-conformance-logical-case-member-v1\0";
const SOURCE_SET_DOMAIN: &[u8] = b"borrowser-conformance-logical-case-source-set-v1\0";
const SHA256_BYTES: usize = 32;
const FIELD_FRAMING_BYTES: usize = 2 + 8;
const SEQUENCE_COUNT_BYTES: usize = 8;
const SEQUENCE_ITEM_LENGTH_BYTES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateLogicalSourceIdentity {
    source: AggregateLogicalSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AggregateLogicalSource {
    Native,
    ControlledStaticPage,
    ExternalDerived {
        source_record: SourceRecordId,
        lineage: ExternalLineageId,
        adapter: HarnessFeatureId,
        adapter_version: ExternalAdapterVersion,
    },
}

impl AggregateLogicalSourceIdentity {
    pub const fn source_kind(&self) -> SourceKind {
        match self.source {
            AggregateLogicalSource::Native => SourceKind::Native,
            AggregateLogicalSource::ControlledStaticPage => SourceKind::ControlledStaticPage,
            AggregateLogicalSource::ExternalDerived { .. } => SourceKind::ExternalDerived,
        }
    }

    pub const fn kind_label(&self) -> &'static str {
        match self.source {
            AggregateLogicalSource::Native => "native",
            AggregateLogicalSource::ControlledStaticPage => "controlled-static-page",
            AggregateLogicalSource::ExternalDerived { .. } => "external-derived",
        }
    }

    pub const fn source_record(&self) -> Option<&SourceRecordId> {
        match &self.source {
            AggregateLogicalSource::ExternalDerived { source_record, .. } => Some(source_record),
            AggregateLogicalSource::Native | AggregateLogicalSource::ControlledStaticPage => None,
        }
    }

    pub const fn lineage(&self) -> Option<&ExternalLineageId> {
        match &self.source {
            AggregateLogicalSource::ExternalDerived { lineage, .. } => Some(lineage),
            AggregateLogicalSource::Native | AggregateLogicalSource::ControlledStaticPage => None,
        }
    }

    pub const fn adapter(&self) -> Option<&HarnessFeatureId> {
        match &self.source {
            AggregateLogicalSource::ExternalDerived { adapter, .. } => Some(adapter),
            AggregateLogicalSource::Native | AggregateLogicalSource::ControlledStaticPage => None,
        }
    }

    pub const fn adapter_version(&self) -> Option<&ExternalAdapterVersion> {
        match &self.source {
            AggregateLogicalSource::ExternalDerived {
                adapter_version, ..
            } => Some(adapter_version),
            AggregateLogicalSource::Native | AggregateLogicalSource::ControlledStaticPage => None,
        }
    }

    pub(crate) fn matches_fixture_source(&self, fixture: &ValidatedFixture) -> bool {
        match (fixture.source(), &self.source) {
            (FixtureSource::Native, AggregateLogicalSource::Native)
            | (FixtureSource::ControlledStaticPage, AggregateLogicalSource::ControlledStaticPage) => {
                true
            }
            (
                FixtureSource::ExternalDerived {
                    lineage_id,
                    adapter,
                    adapter_version,
                },
                AggregateLogicalSource::ExternalDerived {
                    lineage,
                    adapter: aggregate_adapter,
                    adapter_version: aggregate_version,
                    ..
                },
            ) => {
                lineage_id == lineage
                    && adapter == aggregate_adapter
                    && adapter_version == aggregate_version
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateLogicalCaseMemberDigest(Sha256Digest);

impl AggregateLogicalCaseMemberDigest {
    pub const fn as_sha256(&self) -> &Sha256Digest {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateLogicalCaseSourceSetDigest(Sha256Digest);

impl AggregateLogicalCaseSourceSetDigest {
    pub const fn as_sha256(&self) -> &Sha256Digest {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateIdentityError {
    MissingReconciledExternalLineage { test_id: String },
    LengthOverflow,
    DuplicateTestId { test_id: String },
    DuplicateMemberDigest,
    AllocationFailure,
}

pub(crate) fn source_identity(
    fixture: &ValidatedFixture,
    external_lineages: Option<&ReconciledExternalFixtureLineages<'_>>,
) -> Result<AggregateLogicalSourceIdentity, AggregateIdentityError> {
    match fixture.source() {
        FixtureSource::Native => Ok(AggregateLogicalSourceIdentity {
            source: AggregateLogicalSource::Native,
        }),
        FixtureSource::ControlledStaticPage => Ok(AggregateLogicalSourceIdentity {
            source: AggregateLogicalSource::ControlledStaticPage,
        }),
        FixtureSource::ExternalDerived {
            lineage_id,
            adapter,
            adapter_version,
        } => {
            let declaration = external_lineages
                .and_then(|lineages| lineages.declaration_for(fixture.id()))
                .filter(|declaration| {
                    declaration.derived_test_id() == fixture.id()
                        && declaration.id() == lineage_id
                        && declaration.adapter() == adapter
                        && declaration.adapter_version() == adapter_version
                })
                .ok_or_else(
                    || AggregateIdentityError::MissingReconciledExternalLineage {
                        test_id: fixture.id().as_str().to_owned(),
                    },
                )?;
            Ok(AggregateLogicalSourceIdentity {
                source: AggregateLogicalSource::ExternalDerived {
                    source_record: declaration.source_record().clone(),
                    lineage: declaration.id().clone(),
                    adapter: declaration.adapter().clone(),
                    adapter_version: declaration.adapter_version().clone(),
                },
            })
        }
    }
}

pub(crate) fn member_digest(
    fixture: &ValidatedFixture,
    source: &AggregateLogicalSourceIdentity,
) -> Result<AggregateLogicalCaseMemberDigest, AggregateIdentityError> {
    let mut fields = [None; 8];
    fields[0] = Some(fixture.scope().as_str().as_bytes());
    fields[1] = Some(fixture.id().as_str().as_bytes());
    fields[2] = Some(fixture.observation().as_str().as_bytes());
    fields[3] = Some(source.kind_label().as_bytes());
    if let AggregateLogicalSource::ExternalDerived {
        source_record,
        lineage,
        adapter,
        adapter_version,
    } = &source.source
    {
        fields[4] = Some(source_record.as_str().as_bytes());
        fields[5] = Some(lineage.as_str().as_bytes());
        fields[6] = Some(adapter.as_str().as_bytes());
        fields[7] = Some(adapter_version.as_str().as_bytes());
    }
    let preimage = build_tlv_preimage(MEMBER_DOMAIN, &fields)?;
    Ok(AggregateLogicalCaseMemberDigest(sha256(&preimage)))
}

pub(crate) fn source_set_digest(
    scope: InventoryScope,
    members: &[(&TestId, AggregateLogicalCaseMemberDigest)],
) -> Result<AggregateLogicalCaseSourceSetDigest, AggregateIdentityError> {
    let mut ordered = Vec::new();
    ordered
        .try_reserve(members.len())
        .map_err(|_| AggregateIdentityError::AllocationFailure)?;
    ordered.extend_from_slice(members);
    ordered.sort_unstable_by(|(left, _), (right, _)| {
        left.as_str().as_bytes().cmp(right.as_str().as_bytes())
    });
    for pair in ordered.windows(2) {
        if pair[0].0.as_str().as_bytes() == pair[1].0.as_str().as_bytes() {
            return Err(AggregateIdentityError::DuplicateTestId {
                test_id: pair[0].0.as_str().to_owned(),
            });
        }
    }

    let mut digests = Vec::new();
    digests
        .try_reserve(ordered.len())
        .map_err(|_| AggregateIdentityError::AllocationFailure)?;
    digests.extend(
        ordered
            .iter()
            .map(|(_, digest)| *digest.as_sha256().as_bytes()),
    );
    digests.sort_unstable_by(|left, right| left.as_slice().cmp(right.as_slice()));
    if digests.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(AggregateIdentityError::DuplicateMemberDigest);
    }

    let count = u64::try_from(ordered.len()).map_err(|_| AggregateIdentityError::LengthOverflow)?;
    let item_size = SEQUENCE_ITEM_LENGTH_BYTES
        .checked_add(SHA256_BYTES)
        .ok_or(AggregateIdentityError::LengthOverflow)?;
    let sequence_len = ordered
        .len()
        .checked_mul(item_size)
        .and_then(|value| value.checked_add(SEQUENCE_COUNT_BYTES))
        .ok_or(AggregateIdentityError::LengthOverflow)?;
    let mut sequence = Vec::new();
    sequence
        .try_reserve(sequence_len)
        .map_err(|_| AggregateIdentityError::AllocationFailure)?;
    sequence.extend_from_slice(&count.to_be_bytes());
    let digest_length =
        u64::try_from(SHA256_BYTES).map_err(|_| AggregateIdentityError::LengthOverflow)?;
    for (_, digest) in ordered {
        sequence.extend_from_slice(&digest_length.to_be_bytes());
        sequence.extend_from_slice(digest.as_sha256().as_bytes());
    }
    debug_assert_eq!(sequence.len(), sequence_len);
    let fields = [Some(scope.as_str().as_bytes()), Some(sequence.as_slice())];
    let preimage = build_tlv_preimage(SOURCE_SET_DOMAIN, &fields)?;
    Ok(AggregateLogicalCaseSourceSetDigest(sha256(&preimage)))
}

fn build_tlv_preimage(
    domain: &[u8],
    fields: &[Option<&[u8]>],
) -> Result<Vec<u8>, AggregateIdentityError> {
    let total = fields.iter().try_fold(domain.len(), |total, payload| {
        let Some(payload) = payload else {
            return Ok(total);
        };
        let _ = u64::try_from(payload.len()).map_err(|_| AggregateIdentityError::LengthOverflow)?;
        total
            .checked_add(FIELD_FRAMING_BYTES)
            .and_then(|value| value.checked_add(payload.len()))
            .ok_or(AggregateIdentityError::LengthOverflow)
    })?;
    let mut preimage = Vec::new();
    preimage
        .try_reserve(total)
        .map_err(|_| AggregateIdentityError::AllocationFailure)?;
    preimage.extend_from_slice(domain);
    for (index, payload) in fields.iter().enumerate() {
        let Some(payload) = payload else {
            continue;
        };
        let tag_index = index
            .checked_add(1)
            .ok_or(AggregateIdentityError::LengthOverflow)?;
        let tag = u16::try_from(tag_index).map_err(|_| AggregateIdentityError::LengthOverflow)?;
        let length =
            u64::try_from(payload.len()).map_err(|_| AggregateIdentityError::LengthOverflow)?;
        preimage.extend_from_slice(&tag.to_be_bytes());
        preimage.extend_from_slice(&length.to_be_bytes());
        preimage.extend_from_slice(payload);
    }
    debug_assert_eq!(preimage.len(), total);
    Ok(preimage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_hex(digest: &Sha256Digest) -> String {
        digest.to_hex()
    }

    #[test]
    fn empty_static_source_set_has_the_frozen_v1_digest() {
        let empty_sequence = 0_u64.to_be_bytes();
        let fields = [
            Some(InventoryScope::StaticHtmlCssNoJs.as_str().as_bytes()),
            Some(empty_sequence.as_slice()),
        ];
        let preimage = build_tlv_preimage(SOURCE_SET_DOMAIN, &fields).unwrap();
        assert_eq!(preimage.len(), 98);
        let mut expected = Vec::new();
        expected.extend_from_slice(SOURCE_SET_DOMAIN);
        expected.extend_from_slice(&1_u16.to_be_bytes());
        expected.extend_from_slice(&21_u64.to_be_bytes());
        expected.extend_from_slice(b"static-html-css-no-js");
        expected.extend_from_slice(&2_u16.to_be_bytes());
        expected.extend_from_slice(&8_u64.to_be_bytes());
        expected.extend_from_slice(&0_u64.to_be_bytes());
        assert_eq!(preimage, expected);
        let digest = source_set_digest(InventoryScope::StaticHtmlCssNoJs, &[]).unwrap();
        assert_eq!(
            digest_hex(digest.as_sha256()),
            "768d27de40c959c7cebd099c1104e668b06a36da11cf367767d990760adb5270"
        );
    }

    #[test]
    fn representative_member_preimages_have_frozen_v1_digests() {
        let vectors: &[(&[&str], &str)] = &[
            (
                &[
                    "static-html-css-no-js",
                    "css-cascade-basic-author-rule",
                    "css-cascade",
                    "native",
                ],
                "587fc9b32ef9bec4d021980da198836deab422f5e0ac506ac6de7eb1e955d270",
            ),
            (
                &[
                    "static-html-css-no-js",
                    "browser-controlled-static-page-basic",
                    "browser-runtime-semantic",
                    "controlled-static-page",
                ],
                "fc500a811a274719eccd9c519c8b72bd958c8ef7ab9c2dd70df6f920b0d68178",
            ),
            (
                &[
                    "static-html-css-no-js",
                    "wpt-derived-body-background-display-none",
                    "paint-operations",
                    "external-derived",
                    "wpt-css-body-background-display-none",
                    "wpt-body-background-display-none-paint-v1",
                    "rendering-paired-semantic",
                    "1",
                ],
                "0ea3d38ffb6b70a0e29d695fe1e2ec4a858e875b6557100a548de75a9844066a",
            ),
        ];
        for (values, expected) in vectors {
            let mut fields = [None; 8];
            for (index, value) in values.iter().enumerate() {
                fields[index] = Some(value.as_bytes());
            }
            let preimage = build_tlv_preimage(MEMBER_DOMAIN, &fields).unwrap();
            assert_eq!(sha256(&preimage).to_hex(), *expected);
        }
    }

    #[test]
    fn every_member_identity_field_independently_changes_the_digest() {
        let baseline = [
            Some(b"static-html-css-no-js".as_slice()),
            Some(b"external-case".as_slice()),
            Some(b"paint-operations".as_slice()),
            Some(b"external-derived".as_slice()),
            Some(b"source-record-a".as_slice()),
            Some(b"lineage-a".as_slice()),
            Some(b"rendering-paired-semantic".as_slice()),
            Some(b"1".as_slice()),
        ];
        let baseline_digest = sha256(&build_tlv_preimage(MEMBER_DOMAIN, &baseline).unwrap());
        let replacements: &[(usize, &[u8])] = &[
            // V1 currently exposes only one typed InventoryScope. Mutating the
            // canonical tag-1 payload proves the framing still binds scope
            // without adding a fake production enum variant.
            (0, b"future-inventory-scope"),
            (1, b"other-external-case"),
            (2, b"layout-geometry"),
            (4, b"source-record-b"),
            (5, b"lineage-b"),
            (6, b"rendering-other-adapter"),
            (7, b"2"),
        ];
        for (index, replacement) in replacements {
            let mut changed = baseline;
            changed[*index] = Some(replacement);
            let changed_digest = sha256(&build_tlv_preimage(MEMBER_DOMAIN, &changed).unwrap());
            assert_ne!(
                changed_digest,
                baseline_digest,
                "member identity field at tag {} did not affect SHA-256",
                index + 1
            );
        }

        let native = [
            Some(b"static-html-css-no-js".as_slice()),
            Some(b"logical-case".as_slice()),
            Some(b"css-cascade".as_slice()),
            Some(b"native".as_slice()),
        ];
        let mut controlled = native;
        controlled[3] = Some(b"controlled-static-page");
        assert_ne!(
            sha256(&build_tlv_preimage(MEMBER_DOMAIN, &native).unwrap()),
            sha256(&build_tlv_preimage(MEMBER_DOMAIN, &controlled).unwrap())
        );
    }

    #[test]
    fn native_and_controlled_members_omit_external_tags_instead_of_encoding_empty_payloads() {
        for source_kind in [b"native".as_slice(), b"controlled-static-page".as_slice()] {
            let fields = [
                Some(b"static-html-css-no-js".as_slice()),
                Some(b"logical-case".as_slice()),
                Some(b"css-cascade".as_slice()),
                Some(source_kind),
                None,
                None,
                None,
                None,
            ];
            let preimage = build_tlv_preimage(MEMBER_DOMAIN, &fields).unwrap();
            assert_eq!(member_tags(&preimage), vec![1, 2, 3, 4]);

            let mut empty_external_fields = fields;
            empty_external_fields[4..].fill(Some(b""));
            assert_ne!(
                preimage,
                build_tlv_preimage(MEMBER_DOMAIN, &empty_external_fields).unwrap()
            );
        }
    }

    fn member_tags(preimage: &[u8]) -> Vec<u16> {
        let mut cursor = MEMBER_DOMAIN.len();
        let mut tags = Vec::new();
        while cursor < preimage.len() {
            let tag = u16::from_be_bytes(preimage[cursor..cursor + 2].try_into().unwrap());
            cursor += 2;
            let length = u64::from_be_bytes(preimage[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            cursor += usize::try_from(length).unwrap();
            tags.push(tag);
        }
        tags
    }

    #[test]
    fn source_set_is_input_order_independent_and_rejects_duplicates() {
        let first = TestId::parse("first").unwrap();
        let second = TestId::parse("second").unwrap();
        let first_digest = AggregateLogicalCaseMemberDigest(sha256(b"first"));
        let second_digest = AggregateLogicalCaseMemberDigest(sha256(b"second"));
        let forward = source_set_digest(
            InventoryScope::StaticHtmlCssNoJs,
            &[(&first, first_digest), (&second, second_digest)],
        )
        .unwrap();
        let reverse = source_set_digest(
            InventoryScope::StaticHtmlCssNoJs,
            &[(&second, second_digest), (&first, first_digest)],
        )
        .unwrap();
        assert_eq!(forward, reverse);
        let changed = AggregateLogicalCaseMemberDigest(sha256(b"changed"));
        assert_ne!(
            forward,
            source_set_digest(
                InventoryScope::StaticHtmlCssNoJs,
                &[(&first, first_digest), (&second, changed)]
            )
            .unwrap()
        );
        assert!(matches!(
            source_set_digest(
                InventoryScope::StaticHtmlCssNoJs,
                &[(&first, first_digest), (&first, second_digest)]
            ),
            Err(AggregateIdentityError::DuplicateTestId { .. })
        ));
        assert!(matches!(
            source_set_digest(
                InventoryScope::StaticHtmlCssNoJs,
                &[(&first, first_digest), (&second, first_digest)]
            ),
            Err(AggregateIdentityError::DuplicateMemberDigest)
        ));
    }
}
