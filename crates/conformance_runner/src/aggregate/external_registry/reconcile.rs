// The closed diagnostic owns its complete typed attachment tuple. Phase
// validation orders borrowed subjects first and moves the winning subject into
// the returned error without allocation.
#![allow(clippy::result_large_err)]

use std::path::Path;

use external_test_provenance::{
    CaptureV1Error, SameObjectConfinedReadError, read_confined_regular_file_same_object,
    read_external_artifact_candidate_same_object,
};

use crate::{
    AggregateExecutionVariantId, AggregateRun, AggregateSubsystemResult, AggregateVariantKey,
    ExecutionVariantId, SingletonExecutionVariant,
};

use super::allocation::{
    ProductionReservation, ReservationPolicy, ReservationSite, try_reserve_vec,
};
use super::diagnostic::{
    ExternalRegistryDiagnostic, ExternalRegistryDiagnosticComponent as Component,
    ExternalRegistryDiagnosticDetail as Detail, ExternalRegistryDiagnosticField as Field,
    ExternalRegistryDiagnosticKind as Kind, ExternalRegistryDiagnosticSubjectKey as Subject,
    ExternalRegistryTrackInvariantField as TrackInvariant,
    ExternalRegistryValidationPhase as Phase,
};
use super::model::*;
use super::validate::{parse_schema, validate_registry_with_policy};
use super::{EXTERNAL_COMPARISON_REGISTRY_PATH, MAX_EXTERNAL_COMPARISON_REGISTRY_BYTES_V1};

pub fn load_repository_external_advisory_evidence<'run>(
    repository_root: &Path,
    run: &'run AggregateRun,
) -> Result<ReconciledExternalAdvisoryEvidence<'run>, ExternalRegistryDiagnostic> {
    load_repository_external_advisory_evidence_with_policy(
        repository_root,
        run,
        &mut ProductionReservation,
    )
}

fn load_repository_external_advisory_evidence_with_policy<'run>(
    repository_root: &Path,
    run: &'run AggregateRun,
    reservation: &mut impl ReservationPolicy,
) -> Result<ReconciledExternalAdvisoryEvidence<'run>, ExternalRegistryDiagnostic> {
    let registry_bytes = read_confined_regular_file_same_object(
        repository_root,
        Path::new(EXTERNAL_COMPARISON_REGISTRY_PATH),
        MAX_EXTERNAL_COMPARISON_REGISTRY_BYTES_V1,
    )
    .map_err(map_registry_read)?;
    let wire = parse_schema(&registry_bytes)?;
    let unique = validate_registry_with_policy(wire, reservation)?;
    let verified = verify_artifacts(repository_root, unique, reservation)?;
    reconcile_aggregate(run, verified, reservation)
}

struct VerifiedRegistry {
    captures: Vec<StoredValidatedCapture>,
    attachments: Vec<TypedAttachment>,
    tracks: Vec<ValidatedAdvisoryTrack>,
    notes: Vec<TypedNote>,
    actual_total: u64,
}

fn verify_artifacts(
    repository_root: &Path,
    unique: UniqueRegistry,
    reservation: &mut impl ReservationPolicy,
) -> Result<VerifiedRegistry, ExternalRegistryDiagnostic> {
    let mut captures = Vec::new();
    try_reserve_vec(
        &mut captures,
        unique.captures.len(),
        ReservationSite::Phase6Captures,
        reservation,
    )
    .map_err(|_| {
        artifact_diag(
            Subject::ArtifactCollection,
            Kind::ArtifactReadFailure,
            Detail::Field(Field::Allocation),
        )
    })?;
    let mut actual_total = 0_u64;
    for capture in unique.captures {
        let UniqueCapture {
            claim,
            claim_text,
            artifact_path,
            provenance,
        } = capture;
        let candidate = read_external_artifact_candidate_same_object(
            repository_root,
            Path::new(&artifact_path.full),
        );
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(error) => return Err(map_artifact_read(claim_text, error)),
        };
        let length_mismatch =
            candidate.actual_byte_length() != provenance.declared_artifact_utf8_byte_length();
        let digest_mismatch = candidate.actual_sha256() != provenance.declared_artifact_sha256();
        let verified = match candidate.validate(provenance.artifact_format()) {
            Ok(verified) => verified,
            Err(_) if length_mismatch => {
                return Err(artifact_diag(
                    artifact_subject(claim_text),
                    Kind::ArtifactLengthMismatch,
                    Detail::Field(Field::ArtifactLength),
                ));
            }
            Err(_) if digest_mismatch => {
                return Err(artifact_diag(
                    artifact_subject(claim_text),
                    Kind::ArtifactDigestMismatch,
                    Detail::Field(Field::ArtifactSha256),
                ));
            }
            Err(_) => {
                return Err(artifact_diag(
                    artifact_subject(claim_text),
                    Kind::ArtifactFormatInvalid,
                    Detail::Field(Field::ArtifactFormat),
                ));
            }
        };
        if length_mismatch {
            return Err(artifact_diag(
                artifact_subject(claim_text),
                Kind::ArtifactLengthMismatch,
                Detail::Field(Field::ArtifactLength),
            ));
        }
        if digest_mismatch {
            return Err(artifact_diag(
                artifact_subject(claim_text),
                Kind::ArtifactDigestMismatch,
                Detail::Field(Field::ArtifactSha256),
            ));
        }
        actual_total = match actual_total.checked_add(verified.utf8_byte_length()) {
            Some(total) => total,
            None => {
                return Err(artifact_diag(
                    artifact_subject(claim_text),
                    Kind::ActualArtifactBytesOverflow,
                    Detail::Component(Component::ActualByteSum),
                ));
            }
        };
        let validated = external_test_provenance::ValidatedExternalCaptureV1::verify(
            provenance, verified, claim,
        );
        let validated = match validated {
            Ok(validated) => validated,
            Err(error) => {
                return Err(match error {
                    CaptureV1Error::ArtifactLengthMismatch => artifact_diag(
                        artifact_subject(claim_text),
                        Kind::ArtifactLengthMismatch,
                        Detail::Field(Field::ArtifactLength),
                    ),
                    CaptureV1Error::ArtifactDigestMismatch => artifact_diag(
                        artifact_subject(claim_text),
                        Kind::ArtifactDigestMismatch,
                        Detail::Field(Field::ArtifactSha256),
                    ),
                    CaptureV1Error::CaptureIdMismatch => artifact_diag(
                        artifact_subject(claim_text),
                        Kind::CaptureIdMismatch,
                        Detail::Field(Field::CaptureId),
                    ),
                    _ => artifact_diag(
                        artifact_subject(claim_text),
                        Kind::ArtifactReadFailure,
                        Detail::Component(Component::CaptureValidation),
                    ),
                });
            }
        };
        captures.push(StoredValidatedCapture {
            artifact_path: artifact_path.full,
            capture: validated,
        });
    }
    if actual_total != unique.declared_artifact_bytes_total {
        return Err(artifact_diag(
            Subject::ArtifactCollection,
            Kind::ArtifactLengthMismatch,
            Detail::Component(Component::CumulativeLengthInvariant),
        ));
    }
    let verified = VerifiedRegistry {
        captures,
        attachments: unique.attachments,
        tracks: unique.tracks,
        notes: unique.notes,
        actual_total,
    };
    validate_internal_references(verified)
}

fn validate_internal_references(
    mut verified: VerifiedRegistry,
) -> Result<VerifiedRegistry, ExternalRegistryDiagnostic> {
    verified
        .attachments
        .sort_by(|left, right| left.raw_subject.contract_cmp(&right.raw_subject));
    for attachment in &mut verified.attachments {
        let Some(capture) = find_capture(&verified.captures, attachment.capture_claim) else {
            return Err(internal_diag(
                Subject::Attachment(attachment.raw_subject.take()),
                Kind::UnknownCaptureReference,
                Detail::Field(Field::CaptureId),
            ));
        };
        let track = verified
            .tracks
            .iter()
            .find(|track| track.id.as_str() == attachment.raw_subject.track_id());
        let Some(track) = track else {
            return Err(internal_diag(
                Subject::Attachment(attachment.raw_subject.take()),
                Kind::UnknownTrackReference,
                Detail::Field(Field::TrackId),
            ));
        };
        if let Some(field) = track_mismatch(track, capture.capture.provenance()) {
            return Err(internal_diag(
                Subject::Attachment(attachment.raw_subject.take()),
                Kind::TrackInvariantMismatch,
                Detail::TrackInvariant(field),
            ));
        }
    }
    for note in &mut verified.notes {
        if let Some(claim) = note.capture_claim
            && find_capture(&verified.captures, claim).is_none()
        {
            return Err(internal_diag(
                Subject::Note {
                    note_id: note.id.take_string(),
                },
                Kind::UnknownCaptureReference,
                Detail::Field(Field::CaptureId),
            ));
        }
    }
    verified.attachments.sort_by(|left, right| {
        left.uniqueness_cmp(right)
            .then_with(|| left.raw_subject.contract_cmp(&right.raw_subject))
    });
    Ok(verified)
}

fn track_mismatch(
    track: &ValidatedAdvisoryTrack,
    provenance: &external_test_provenance::ExternalCaptureProvenanceV1,
) -> Option<TrackInvariant> {
    [
        (
            &track.engine_product != provenance.engine_product(),
            TrackInvariant::EngineProduct,
        ),
        (
            &track.platform_os_family != provenance.platform_os_family(),
            TrackInvariant::PlatformOsFamily,
        ),
        (
            &track.architecture != provenance.architecture(),
            TrackInvariant::Architecture,
        ),
        (
            track.comparable != ComparableObservationSurface::WebObservableDomTreeV1,
            TrackInvariant::ComparableObservationSurface,
        ),
        (
            &track.capture_algorithm != provenance.capture_algorithm(),
            TrackInvariant::CaptureAlgorithm,
        ),
        (
            &track.capture_algorithm_version != provenance.capture_algorithm_version(),
            TrackInvariant::CaptureAlgorithmVersion,
        ),
        (
            track.target_parser_input_context != provenance.target_parser_input_context(),
            TrackInvariant::TargetParserInputContext,
        ),
        (
            &track.collection_policy != provenance.collection_policy(),
            TrackInvariant::CollectionPolicy,
        ),
        (
            &track.collection_policy_version != provenance.collection_policy_version(),
            TrackInvariant::CollectionPolicyVersion,
        ),
    ]
    .into_iter()
    .filter_map(|(mismatch, field)| mismatch.then_some(field))
    .min_by_key(|field| field.contract_rank())
}

fn reconcile_aggregate<'run>(
    run: &'run AggregateRun,
    verified: VerifiedRegistry,
    reservation: &mut impl ReservationPolicy,
) -> Result<ReconciledExternalAdvisoryEvidence<'run>, ExternalRegistryDiagnostic> {
    let VerifiedRegistry {
        captures,
        attachments: mut validated_attachments,
        tracks,
        notes: mut validated_notes,
        actual_total,
    } = verified;
    let mut attachments = Vec::new();
    try_reserve_vec(
        &mut attachments,
        validated_attachments.len(),
        ReservationSite::Phase8Attachments,
        reservation,
    )
    .map_err(|_| {
        aggregate_diag(
            Subject::AttachmentCollection,
            Kind::AggregateAttachmentMismatch,
            Detail::Field(Field::Allocation),
        )
    })?;
    validated_attachments.sort_by(|left, right| left.raw_subject.contract_cmp(&right.raw_subject));
    for attachment in &mut validated_attachments {
        if let Some((kind, detail)) =
            resolve_variant_problem(run, &attachment.test_id, attachment.observation)
        {
            return Err(aggregate_diag(
                Subject::Attachment(attachment.raw_subject.take()),
                kind,
                detail,
            ));
        }
        if find_capture(&captures, attachment.capture_claim).is_none() {
            return Err(aggregate_diag(
                Subject::Attachment(attachment.raw_subject.take()),
                Kind::AggregateAttachmentMismatch,
                Detail::Component(Component::ValidatedCaptureReference),
            ));
        }
    }
    validated_attachments.sort_by(|left, right| {
        left.uniqueness_cmp(right)
            .then_with(|| left.raw_subject.contract_cmp(&right.raw_subject))
    });
    for mut attachment in validated_attachments {
        let capture = find_capture(&captures, attachment.capture_claim)
            .expect("phase 7 established each attachment capture reference");
        let variant = find_admitted_variant(run, &attachment.test_id, attachment.observation)
            .expect("phase 8 established each attachment aggregate variant");
        let track_id = AdvisoryTrackId::parse_owned(attachment.raw_subject.take_track_id())
            .expect("phase 4 established the advisory-track ID grammar");
        attachments.push(ReconciledExternalAttachment {
            aggregate_variant: variant,
            comparable: attachment.comparable,
            track_id,
            capture_id: capture.capture.id(),
        });
    }

    let mut notes = Vec::new();
    try_reserve_vec(
        &mut notes,
        validated_notes.len(),
        ReservationSite::Phase8Notes,
        reservation,
    )
    .map_err(|_| {
        aggregate_diag(
            Subject::NoteCollection,
            Kind::AggregateAttachmentMismatch,
            Detail::Field(Field::Allocation),
        )
    })?;
    for note in &mut validated_notes {
        if let Some((kind, detail)) = resolve_variant_problem(run, &note.test_id, note.observation)
        {
            return Err(aggregate_diag(
                Subject::Note {
                    note_id: note.id.take_string(),
                },
                kind,
                detail,
            ));
        }
    }
    for note in validated_notes {
        let variant = find_admitted_variant(run, &note.test_id, note.observation)
            .expect("phase 8 established each note aggregate variant");
        notes.push(ReconciledBaselineNote {
            id: note.id,
            aggregate_variant: variant,
            text: note.text,
            comparable: note.comparable,
            capture_id: note.capture_claim.and_then(|claim| {
                find_capture(&captures, claim).map(|capture| capture.capture.id())
            }),
        });
    }
    Ok(ReconciledExternalAdvisoryEvidence {
        captures,
        tracks,
        attachments,
        notes,
        verified_artifact_bytes_total: actual_total,
    })
}

fn resolve_variant_problem(
    run: &AggregateRun,
    test_id: &conformance_test_support::TestId,
    observation: conformance_test_support::ObservationSurface,
) -> Option<(Kind, Detail)> {
    let Some(case) = run.cases().iter().find(|case| &case.ag.test_id == test_id) else {
        return Some((Kind::UnknownTestId, Detail::Field(Field::TestId)));
    };
    if case.ag.observation != observation {
        return Some((
            Kind::UnknownObservationSurface,
            Detail::Field(Field::ObservationSurface),
        ));
    }
    let key = AggregateVariantKey {
        test_id: test_id.clone(),
        observation,
        variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
            SingletonExecutionVariant::Singleton,
        )),
    };
    let Some(variant) = case.variants.iter().find(|variant| variant.key == key) else {
        return Some((
            Kind::UnknownExecutionVariant,
            Detail::Field(Field::ExecutionVariant),
        ));
    };
    if case.owner != conformance_test_support::SubsystemOwner::HtmlParser
        || !matches!(variant.subsystem, AggregateSubsystemResult::Parser(_))
    {
        return Some((
            Kind::AggregateAttachmentMismatch,
            Detail::Component(Component::ParserDomOwner),
        ));
    }
    None
}

fn find_admitted_variant<'run>(
    run: &'run AggregateRun,
    test_id: &conformance_test_support::TestId,
    observation: conformance_test_support::ObservationSurface,
) -> Option<&'run crate::AggregateVariantResult> {
    if resolve_variant_problem(run, test_id, observation).is_some() {
        return None;
    }
    let key = AggregateVariantKey {
        test_id: test_id.clone(),
        observation,
        variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
            SingletonExecutionVariant::Singleton,
        )),
    };
    run.cases()
        .iter()
        .find(|case| &case.ag.test_id == test_id)
        .and_then(|case| case.variants.iter().find(|variant| variant.key == key))
}

fn find_capture(
    captures: &[StoredValidatedCapture],
    claim: external_test_provenance::ExternalCaptureIdClaim,
) -> Option<&StoredValidatedCapture> {
    captures
        .iter()
        .find(|capture| capture.capture.id().as_sha256() == claim.as_sha256())
}

fn map_registry_read(error: SameObjectConfinedReadError) -> ExternalRegistryDiagnostic {
    let kind = match error {
        SameObjectConfinedReadError::InvalidRelativePath => Kind::RegistryPathUnsafe,
        SameObjectConfinedReadError::Missing => Kind::RegistryMissing,
        SameObjectConfinedReadError::Symlink => Kind::RegistrySymlink,
        SameObjectConfinedReadError::NonRegularFile
        | SameObjectConfinedReadError::NonDirectoryParent => Kind::RegistryNotRegular,
        SameObjectConfinedReadError::TooLarge => Kind::RegistryTooLarge,
        SameObjectConfinedReadError::Allocation
        | SameObjectConfinedReadError::LengthOverflow
        | SameObjectConfinedReadError::UnsupportedPlatform
        | SameObjectConfinedReadError::Io => Kind::RegistryReadFailure,
    };
    ExternalRegistryDiagnostic::new(
        Phase::RegistryRead,
        Subject::Registry,
        kind,
        Detail::Component(Component::RegistryRead),
    )
}
fn map_artifact_read(
    supplied_capture_id: String,
    error: SameObjectConfinedReadError,
) -> ExternalRegistryDiagnostic {
    let kind = match error {
        SameObjectConfinedReadError::InvalidRelativePath => Kind::ArtifactPathUnsafe,
        SameObjectConfinedReadError::Missing => Kind::ArtifactMissing,
        SameObjectConfinedReadError::Symlink => Kind::ArtifactSymlink,
        SameObjectConfinedReadError::NonRegularFile
        | SameObjectConfinedReadError::NonDirectoryParent => Kind::ArtifactNotRegular,
        SameObjectConfinedReadError::TooLarge => Kind::ArtifactTooLarge,
        SameObjectConfinedReadError::Allocation
        | SameObjectConfinedReadError::LengthOverflow
        | SameObjectConfinedReadError::UnsupportedPlatform
        | SameObjectConfinedReadError::Io => Kind::ArtifactReadFailure,
    };
    artifact_diag(
        artifact_subject(supplied_capture_id),
        kind,
        Detail::Component(Component::ArtifactRead),
    )
}
fn artifact_subject(supplied_capture_id: String) -> Subject {
    Subject::Artifact {
        supplied_capture_id,
    }
}
fn artifact_diag(subject: Subject, kind: Kind, detail: Detail) -> ExternalRegistryDiagnostic {
    ExternalRegistryDiagnostic::new(Phase::ArtifactIdentity, subject, kind, detail)
}
fn internal_diag(subject: Subject, kind: Kind, detail: Detail) -> ExternalRegistryDiagnostic {
    ExternalRegistryDiagnostic::new(Phase::InternalReconciliation, subject, kind, detail)
}
fn aggregate_diag(subject: Subject, kind: Kind, detail: Detail) -> ExternalRegistryDiagnostic {
    ExternalRegistryDiagnostic::new(Phase::AggregateReconciliation, subject, kind, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AggregateExecutionRequest, run_repository_aggregate};
    use conformance_test_support::{LanePolicyScope, ObservationSurface, TestId};
    use external_test_provenance::{
        ApplicabilityV1, ExternalArtifactFormatV1, ExternalCaptureProvenanceV1,
        ExternalCaptureProvenanceV1Input, ExternalIdentityV1, ExternalVersionV1, ImmutableRevision,
        NonApplicableReasonV1, ResourceNetworkPolicyV1, Sha256Digest, TargetParserInputContextV1,
    };

    use crate::aggregate::external_registry::allocation::RejectReservationAt;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
    }

    fn repository_run() -> AggregateRun {
        run_repository_aggregate(
            repository_root(),
            AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap()
    }

    fn identity(value: &str) -> ExternalIdentityV1 {
        ExternalIdentityV1::parse(value).unwrap()
    }

    fn version(value: &str) -> ExternalVersionV1 {
        ExternalVersionV1::parse(value).unwrap()
    }

    fn digest() -> Sha256Digest {
        external_test_provenance::sha256(b"fixture")
    }

    fn provenance(
        engine_version: &str,
        build: Option<&str>,
        os_version: &str,
    ) -> ExternalCaptureProvenanceV1 {
        ExternalCaptureProvenanceV1::try_from_input(ExternalCaptureProvenanceV1Input {
            engine_product: identity("engine"),
            engine_version: version(engine_version),
            engine_build_revision: build.map(identity),
            platform_os_family: identity("os"),
            platform_os_version: version(os_version),
            architecture: identity("arch"),
            viewport: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("surface-independent").unwrap(),
            ),
            device_scale: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("surface-independent").unwrap(),
            ),
            controlled_fonts: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("font-independent").unwrap(),
            ),
            resource_network_policy: ResourceNetworkPolicyV1::Offline,
            pinned_resources: Vec::new(),
            fixture_source_project: identity("fixture"),
            fixture_immutable_revision: ImmutableRevision::parse("revision").unwrap(),
            fixture_content_sha256: digest(),
            capture_mechanism: identity("mechanism"),
            capture_mechanism_version: version("1"),
            capture_algorithm: identity("algorithm"),
            capture_algorithm_version: version("1"),
            capture_algorithm_source_sha256: digest(),
            capture_configuration_sha256: digest(),
            invocation_arguments: Vec::new(),
            artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
            artifact_utf8_byte_length: 0,
            artifact_sha256: digest(),
            target_parser_input_context:
                TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
            collection_policy: identity("stable"),
            collection_policy_version: version("1"),
        })
        .unwrap()
    }

    fn matching_track(provenance: &ExternalCaptureProvenanceV1) -> ValidatedAdvisoryTrack {
        ValidatedAdvisoryTrack {
            id: AdvisoryTrackId::parse_owned("track".to_owned()).unwrap(),
            engine_product: provenance.engine_product().clone(),
            platform_os_family: provenance.platform_os_family().clone(),
            architecture: provenance.architecture().clone(),
            comparable: ComparableObservationSurface::WebObservableDomTreeV1,
            capture_algorithm: provenance.capture_algorithm().clone(),
            capture_algorithm_version: provenance.capture_algorithm_version().clone(),
            target_parser_input_context: provenance.target_parser_input_context(),
            collection_policy: provenance.collection_policy().clone(),
            collection_policy_version: provenance.collection_policy_version().clone(),
        }
    }

    #[test]
    fn engine_build_and_platform_version_progression_remains_admissible() {
        let first = provenance("1", None, "13");
        let track = matching_track(&first);
        assert_eq!(track_mismatch(&track, &first), None);

        let progressed = provenance("2", Some("build-7"), "14");
        assert_eq!(track_mismatch(&track, &progressed), None);
        assert_ne!(first.engine_version(), progressed.engine_version());
        assert_ne!(
            first.engine_build_revision(),
            progressed.engine_build_revision()
        );
        assert_ne!(
            first.platform_os_version(),
            progressed.platform_os_version()
        );
    }

    #[test]
    fn every_multivalued_track_invariant_rejects_a_valid_difference() {
        let provenance = provenance("1", None, "13");
        let mut track = matching_track(&provenance);

        track.architecture = identity("other");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::Architecture)
        );
        track = matching_track(&provenance);
        track.capture_algorithm = identity("other");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::CaptureAlgorithm)
        );
        track = matching_track(&provenance);
        track.capture_algorithm_version = version("2");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::CaptureAlgorithmVersion)
        );
        track = matching_track(&provenance);
        track.collection_policy = identity("other");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::CollectionPolicy)
        );
        track = matching_track(&provenance);
        track.collection_policy_version = version("2");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::CollectionPolicyVersion)
        );
        track = matching_track(&provenance);
        track.engine_product = identity("other");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::EngineProduct)
        );
        track = matching_track(&provenance);
        track.platform_os_family = identity("other");
        assert_eq!(
            track_mismatch(&track, &provenance),
            Some(TrackInvariant::PlatformOsFamily)
        );

        // These two V1 invariant domains are closed singletons, so no second
        // valid typed value exists with which to author a runtime mismatch.
        let ComparableObservationSurface::WebObservableDomTreeV1 = track.comparable;
        let TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1 =
            track.target_parser_input_context;
    }

    fn typed_attachment(test_id: &str) -> TypedAttachment {
        let capture_id = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        TypedAttachment {
            test_id: TestId::parse(test_id).unwrap(),
            observation: ObservationSurface::DomTree,
            comparable: ComparableObservationSurface::WebObservableDomTreeV1,
            capture_claim: external_test_provenance::ExternalCaptureIdClaim::parse(capture_id)
                .unwrap(),
            raw_subject: super::super::diagnostic::ExternalRegistryAttachmentSubjectKey::new(
                test_id.to_owned(),
                "dom-tree".to_owned(),
                "singleton".to_owned(),
                "web-observable-dom-tree-v1".to_owned(),
                "track".to_owned(),
                capture_id.to_owned(),
            ),
        }
    }

    fn typed_note() -> TypedNote {
        TypedNote {
            id: BaselineNoteId::parse_owned("note".to_owned()).unwrap(),
            test_id: TestId::parse("dom-tree-basic-document").unwrap(),
            observation: ObservationSurface::DomTree,
            comparable: ComparableObservationSurface::WebObservableDomTreeV1,
            text: "advisory".to_owned(),
            capture_claim: None,
        }
    }

    #[test]
    fn attachment_reconciliation_precedes_phase_eight_note_output_allocation() {
        let run = repository_run();
        let error = reconcile_aggregate(
            &run,
            VerifiedRegistry {
                captures: Vec::new(),
                attachments: vec![typed_attachment("dom-tree-not-in-authoritative-aggregate")],
                tracks: Vec::new(),
                notes: vec![typed_note()],
                actual_total: 0,
            },
            &mut RejectReservationAt::new(ReservationSite::Phase8Notes),
        )
        .err()
        .unwrap();
        assert_eq!(error.phase(), Phase::AggregateReconciliation);
        assert_eq!(error.kind(), Kind::UnknownTestId);
        assert!(matches!(error.subject(), Subject::Attachment(_)));

        let error = reconcile_aggregate(
            &run,
            VerifiedRegistry {
                captures: Vec::new(),
                attachments: Vec::new(),
                tracks: Vec::new(),
                notes: vec![typed_note()],
                actual_total: 0,
            },
            &mut RejectReservationAt::new(ReservationSite::Phase8Notes),
        )
        .err()
        .unwrap();
        assert_eq!(error.subject(), &Subject::NoteCollection);
        assert_eq!(error.detail(), Detail::Field(Field::Allocation));
    }

    #[test]
    fn missing_parser_dom_singleton_is_unknown_execution_variant() {
        let run = repository_run();
        let mut cases = run.cases().to_vec();
        let case = cases
            .iter_mut()
            .find(|case| case.ag.test_id.as_str() == "dom-tree-basic-document")
            .unwrap();
        case.variants.clear();
        let resealed = AggregateRun::try_seal(
            run.inventory_scope(),
            run.request(),
            run.environment_assessment_mode(),
            cases,
        )
        .unwrap();
        let test_id = TestId::parse("dom-tree-basic-document").unwrap();
        assert_eq!(
            resolve_variant_problem(&resealed, &test_id, ObservationSurface::DomTree),
            Some((
                Kind::UnknownExecutionVariant,
                Detail::Field(Field::ExecutionVariant),
            )),
        );
    }
}
