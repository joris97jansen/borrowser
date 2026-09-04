// The closed diagnostic owns its complete typed attachment tuple. Record-local
// selection compares allocation-free kind/detail values, then moves the one
// already-owned subject into the returned error.
#![allow(clippy::result_large_err)]

use conformance_test_support::{ObservationSurface, PortablePathComponent, TestId};
use external_test_provenance::{
    ApplicabilityV1, CaptureV1Error, ControlledFontIdentityV1, ExternalArtifactFormatV1,
    ExternalCaptureIdClaim, ExternalCaptureProvenanceV1, ExternalCaptureProvenanceV1Input,
    ExternalIdentityV1, ExternalVersionV1, ImmutableRevision, NonApplicableReasonV1,
    PinnedResourceIdentityV1, ReducedDeviceScaleV1, ResourceNetworkPolicyV1, Sha256Digest,
    TargetParserInputContextV1, ViewportCssPixelsV1,
};

use super::allocation::{ReservationPolicy, ReservationSite, try_reserve_vec};
use super::diagnostic::{
    ExternalRegistryAttachmentSubjectKey, ExternalRegistryDiagnostic,
    ExternalRegistryDiagnosticComponent as Component, ExternalRegistryDiagnosticDetail as Detail,
    ExternalRegistryDiagnosticField as Field, ExternalRegistryDiagnosticKind as Kind,
    ExternalRegistryDiagnosticSubjectKey as Subject,
    ExternalRegistryDiagnosticWithoutSubject as LocalDiagnostic,
    ExternalRegistryRecordCollection as Collection, ExternalRegistryValidationPhase as Phase,
    keep_least, keep_least_without_subject,
};
use super::model::*;
use super::wire::*;
use super::{
    EXTERNAL_COMPARISON_REGISTRY_FORMAT_V1, MAX_EXTERNAL_RECORDS_V1,
    MAX_VERIFIED_EXTERNAL_ARTIFACT_BYTES_V1,
};

pub(super) fn parse_schema(bytes: &[u8]) -> Result<RegistryWire, ExternalRegistryDiagnostic> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::RegistryRead,
            Subject::Registry,
            Kind::RegistryInvalidUtf8,
            Detail::Component(Component::Utf8),
        )
    })?;
    let wire: RegistryWire = toml::from_str(text).map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::Schema,
            Subject::Registry,
            Kind::InvalidRegistrySchema,
            Detail::Component(Component::ClosedSchema),
        )
    })?;
    if wire.format != EXTERNAL_COMPARISON_REGISTRY_FORMAT_V1 {
        return Err(ExternalRegistryDiagnostic::new(
            Phase::Schema,
            Subject::Registry,
            Kind::UnsupportedRegistryFormat,
            Detail::Field(Field::RegistryFormat),
        ));
    }
    Ok(wire)
}

pub(super) fn validate_registry_with_policy(
    wire: RegistryWire,
    reservation: &mut impl ReservationPolicy,
) -> Result<UniqueRegistry, ExternalRegistryDiagnostic> {
    let declared_artifact_bytes_total = validate_phase3(&wire)?;
    let local = validate_phase4(wire, declared_artifact_bytes_total, reservation)?;
    validate_phase5(local, reservation)
}

fn validate_phase3(wire: &RegistryWire) -> Result<u64, ExternalRegistryDiagnostic> {
    let mut least = None;
    for (count, kind, detail) in [
        (
            wire.captures.len(),
            Kind::TooManyCaptures,
            Collection::Captures,
        ),
        (
            wire.attachments.len(),
            Kind::TooManyAttachments,
            Collection::Attachments,
        ),
        (
            wire.advisory_tracks.len(),
            Kind::TooManyAdvisoryTracks,
            Collection::AdvisoryTracks,
        ),
        (
            wire.baseline_notes.len(),
            Kind::TooManyBaselineNotes,
            Collection::BaselineNotes,
        ),
    ] {
        if count > MAX_EXTERNAL_RECORDS_V1 {
            keep_least(
                &mut least,
                ExternalRegistryDiagnostic::new(
                    Phase::TopLevelMultiplicity,
                    Subject::Registry,
                    kind,
                    Detail::RecordCollection(detail),
                ),
            );
        }
    }
    let mut total = 0_u64;
    for capture in &wire.captures {
        match total.checked_add(capture.artifact_utf8_byte_length) {
            Some(value) => total = value,
            None => keep_least(
                &mut least,
                ExternalRegistryDiagnostic::new(
                    Phase::TopLevelMultiplicity,
                    Subject::Registry,
                    Kind::DeclaredArtifactBytesOverflow,
                    Detail::Component(Component::ArtifactLengthSum),
                ),
            ),
        }
    }
    if total > MAX_VERIFIED_EXTERNAL_ARTIFACT_BYTES_V1 {
        keep_least(
            &mut least,
            ExternalRegistryDiagnostic::new(
                Phase::TopLevelMultiplicity,
                Subject::Registry,
                Kind::CumulativeArtifactBytesExceeded,
                Detail::Component(Component::ArtifactLengthSum),
            ),
        );
    }
    least.map_or(Ok(total), Err)
}

fn validate_phase4(
    wire: RegistryWire,
    declared_artifact_bytes_total: u64,
    reservation: &mut impl ReservationPolicy,
) -> Result<LocallyValidatedRegistry, ExternalRegistryDiagnostic> {
    let RegistryWire {
        format: _,
        captures: capture_wires,
        attachments: attachment_wires,
        advisory_tracks: track_wires,
        baseline_notes: note_wires,
    } = wire;

    let mut captures = Vec::new();
    try_reserve_vec(
        &mut captures,
        capture_wires.len(),
        ReservationSite::Phase4Captures,
        reservation,
    )
    .map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::RecordLocal,
            Subject::CaptureCollection,
            Kind::InvalidCaptureField,
            Detail::Field(Field::Allocation),
        )
    })?;
    let mut least = None;
    for capture in capture_wires {
        match validate_capture(capture) {
            Ok(value) => captures.push(value),
            Err(error) => keep_least(&mut least, error),
        }
    }
    if let Some(error) = least {
        return Err(error);
    }

    let mut tracks = Vec::new();
    try_reserve_vec(
        &mut tracks,
        track_wires.len(),
        ReservationSite::Phase4Tracks,
        reservation,
    )
    .map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::RecordLocal,
            Subject::TrackCollection,
            Kind::InvalidTrackField,
            Detail::Field(Field::Allocation),
        )
    })?;
    let mut least = None;
    for track in track_wires {
        match validate_track(track) {
            Ok(value) => tracks.push(value),
            Err(error) => keep_least(&mut least, error),
        }
    }
    if let Some(error) = least {
        return Err(error);
    }

    let mut attachments = Vec::new();
    try_reserve_vec(
        &mut attachments,
        attachment_wires.len(),
        ReservationSite::Phase4Attachments,
        reservation,
    )
    .map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::RecordLocal,
            Subject::AttachmentCollection,
            Kind::InvalidAttachmentField,
            Detail::Field(Field::Allocation),
        )
    })?;
    let mut least = None;
    for attachment in attachment_wires {
        match validate_attachment(attachment) {
            Ok(value) => attachments.push(value),
            Err(error) => keep_least(&mut least, error),
        }
    }
    if let Some(error) = least {
        return Err(error);
    }

    let mut notes = Vec::new();
    try_reserve_vec(
        &mut notes,
        note_wires.len(),
        ReservationSite::Phase4Notes,
        reservation,
    )
    .map_err(|_| {
        ExternalRegistryDiagnostic::new(
            Phase::RecordLocal,
            Subject::NoteCollection,
            Kind::InvalidNoteField,
            Detail::Field(Field::Allocation),
        )
    })?;
    let mut least = None;
    for note in note_wires {
        match validate_note(note) {
            Ok(value) => notes.push(value),
            Err(error) => keep_least(&mut least, error),
        }
    }
    if let Some(error) = least {
        return Err(error);
    }
    Ok(LocallyValidatedRegistry {
        captures,
        attachments,
        tracks,
        notes,
        declared_artifact_bytes_total,
    })
}

fn validate_capture(
    wire: CaptureWire,
) -> Result<LocallyValidatedCapture, ExternalRegistryDiagnostic> {
    if let Some(problem) = validate_capture_fields(&wire) {
        return Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Capture {
                supplied_capture_id: wire.capture_id,
            },
        ));
    }
    let CaptureWire {
        capture_id,
        artifact_path,
        provenance_format: _,
        engine_product,
        engine_version,
        engine_build_revision,
        platform_os_family,
        platform_os_version,
        architecture,
        viewport,
        device_scale,
        controlled_fonts,
        resource_network_policy,
        pinned_resources,
        fixture_source_project,
        fixture_immutable_revision,
        fixture_content_sha256,
        capture_mechanism,
        capture_mechanism_version,
        capture_algorithm,
        capture_algorithm_version,
        capture_algorithm_source_sha256,
        capture_configuration_sha256,
        invocation_arguments,
        artifact_format,
        artifact_utf8_byte_length,
        artifact_sha256,
        target_parser_input_context,
        collection_policy,
        collection_policy_version,
    } = wire;
    let parsed: Result<_, LocalDiagnostic> = (|| {
        let claim = ExternalCaptureIdClaim::parse(&capture_id)
            .map_err(|_| local_field(Kind::InvalidCaptureIdClaim, Field::CaptureId))?;
        // `validate_capture_fields` already proved the borrowed spelling. Move
        // the one wire allocation into typed storage without reparsing or
        // allocating a second filename solely as grammar evidence.
        let artifact_path = ArtifactStoragePath {
            full: artifact_path,
        };
        let input = ExternalCaptureProvenanceV1Input {
            engine_product: identity(&engine_product, Field::EngineProduct)?,
            engine_version: version(&engine_version, Field::EngineVersion)?,
            engine_build_revision: engine_build_revision
                .as_deref()
                .map(ExternalIdentityV1::parse)
                .transpose()
                .map_err(|_| local_field(Kind::InvalidCaptureField, Field::EngineBuildRevision))?,
            platform_os_family: identity(&platform_os_family, Field::PlatformOsFamily)?,
            platform_os_version: version(&platform_os_version, Field::PlatformOsVersion)?,
            architecture: identity(&architecture, Field::Architecture)?,
            viewport: parse_viewport(viewport)?,
            device_scale: parse_scale(device_scale)?,
            controlled_fonts: parse_fonts(controlled_fonts)?,
            resource_network_policy: parse_resource_policy(&resource_network_policy).ok_or_else(
                || local_field(Kind::InvalidCaptureField, Field::ResourceNetworkPolicy),
            )?,
            pinned_resources: parse_resources(pinned_resources)?,
            fixture_source_project: identity(&fixture_source_project, Field::FixtureSourceProject)?,
            fixture_immutable_revision: ImmutableRevision::parse(&fixture_immutable_revision)
                .map_err(|_| {
                    local_field(Kind::InvalidCaptureField, Field::FixtureImmutableRevision)
                })?,
            fixture_content_sha256: digest(&fixture_content_sha256, Field::FixtureContentSha256)?,
            capture_mechanism: identity(&capture_mechanism, Field::CaptureMechanism)?,
            capture_mechanism_version: version(
                &capture_mechanism_version,
                Field::CaptureMechanismVersion,
            )?,
            capture_algorithm: identity(&capture_algorithm, Field::CaptureAlgorithm)?,
            capture_algorithm_version: version(
                &capture_algorithm_version,
                Field::CaptureAlgorithmVersion,
            )?,
            capture_algorithm_source_sha256: digest(
                &capture_algorithm_source_sha256,
                Field::CaptureAlgorithmSourceSha256,
            )?,
            capture_configuration_sha256: digest(
                &capture_configuration_sha256,
                Field::CaptureConfigurationSha256,
            )?,
            invocation_arguments,
            artifact_format: (artifact_format == "web-observable-dom-tree-v1")
                .then_some(ExternalArtifactFormatV1::WebObservableDomTreeV1)
                .ok_or_else(|| local_field(Kind::InvalidCaptureField, Field::ArtifactFormat))?,
            artifact_utf8_byte_length,
            artifact_sha256: digest(&artifact_sha256, Field::ArtifactSha256)?,
            target_parser_input_context: (target_parser_input_context
                == "static-text-html-utf8-scripting-disabled-v1")
                .then_some(TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1)
                .ok_or_else(|| {
                    local_field(Kind::InvalidCaptureField, Field::TargetParserInputContext)
                })?,
            collection_policy: identity(&collection_policy, Field::CollectionPolicy)?,
            collection_policy_version: version(
                &collection_policy_version,
                Field::CollectionPolicyVersion,
            )?,
        };
        Ok((claim, artifact_path, input))
    })();
    match parsed {
        Ok((claim, artifact_path, provenance_input)) => Ok(LocallyValidatedCapture {
            claim,
            claim_text: capture_id,
            artifact_path,
            provenance_input,
        }),
        Err(problem) => Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Capture {
                supplied_capture_id: capture_id,
            },
        )),
    }
}

fn validate_capture_fields(wire: &CaptureWire) -> Option<LocalDiagnostic> {
    let subject = wire.capture_id.as_str();
    let mut least = None;
    if ExternalCaptureIdClaim::parse(subject).is_err() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidCaptureIdClaim, Field::CaptureId),
        );
    }
    if wire.invocation_arguments.len() > 16 {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::TooManyInvocationArguments, Field::InvocationArguments),
        );
    }
    for (index, argument) in wire.invocation_arguments.iter().enumerate() {
        if argument.len() > 1024 {
            keep_least_without_subject(
                &mut least,
                LocalDiagnostic::new(
                    Kind::InvocationArgumentTooLong,
                    Detail::InvocationArgumentIndex(index),
                ),
            );
        }
    }
    let font_count = match &wire.controlled_fonts {
        FontsWire::Applicable { items } => items.len(),
        FontsWire::NotApplicable { .. } => 0,
    };
    if font_count > 16 {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::TooManyControlledFonts, Field::ControlledFonts),
        );
    }
    if wire.pinned_resources.len() > 32 {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::TooManyPinnedResources, Field::PinnedResources),
        );
    }
    let mut invalid = |is_invalid: bool, field: Field| {
        if is_invalid {
            keep_least_without_subject(&mut least, local_field(Kind::InvalidCaptureField, field));
        }
    };
    invalid(
        wire.provenance_format != "borrowser-external-capture-provenance-v1",
        Field::ProvenanceFormat,
    );
    invalid(
        !artifact_path_is_valid(&wire.artifact_path),
        Field::ArtifactPath,
    );
    for (value, field) in [
        (&wire.engine_product, Field::EngineProduct),
        (&wire.platform_os_family, Field::PlatformOsFamily),
        (&wire.architecture, Field::Architecture),
        (&wire.fixture_source_project, Field::FixtureSourceProject),
        (&wire.capture_mechanism, Field::CaptureMechanism),
        (&wire.capture_algorithm, Field::CaptureAlgorithm),
        (&wire.collection_policy, Field::CollectionPolicy),
    ] {
        invalid(ExternalIdentityV1::parse(value).is_err(), field);
    }
    if let Some(value) = &wire.engine_build_revision {
        invalid(
            ExternalIdentityV1::parse(value).is_err(),
            Field::EngineBuildRevision,
        );
    }
    for (value, field) in [
        (&wire.engine_version, Field::EngineVersion),
        (&wire.platform_os_version, Field::PlatformOsVersion),
        (
            &wire.capture_mechanism_version,
            Field::CaptureMechanismVersion,
        ),
        (
            &wire.capture_algorithm_version,
            Field::CaptureAlgorithmVersion,
        ),
        (
            &wire.collection_policy_version,
            Field::CollectionPolicyVersion,
        ),
    ] {
        invalid(ExternalVersionV1::parse(value).is_err(), field);
    }
    invalid(
        ImmutableRevision::parse(&wire.fixture_immutable_revision).is_err(),
        Field::FixtureImmutableRevision,
    );
    for (value, field) in [
        (&wire.fixture_content_sha256, Field::FixtureContentSha256),
        (
            &wire.capture_algorithm_source_sha256,
            Field::CaptureAlgorithmSourceSha256,
        ),
        (
            &wire.capture_configuration_sha256,
            Field::CaptureConfigurationSha256,
        ),
        (&wire.artifact_sha256, Field::ArtifactSha256),
    ] {
        invalid(Sha256Digest::parse(value).is_err(), field);
    }
    invalid(
        parse_resource_policy(&wire.resource_network_policy).is_none(),
        Field::ResourceNetworkPolicy,
    );
    invalid(
        wire.artifact_format != "web-observable-dom-tree-v1",
        Field::ArtifactFormat,
    );
    invalid(
        wire.target_parser_input_context != "static-text-html-utf8-scripting-disabled-v1",
        Field::TargetParserInputContext,
    );
    invalid(
        validate_viewport_ref(&wire.viewport).is_err(),
        Field::Viewport,
    );
    invalid(
        validate_scale_ref(&wire.device_scale).is_err(),
        Field::DeviceScale,
    );
    invalid(
        validate_fonts_ref(&wire.controlled_fonts).is_err(),
        Field::ControlledFonts,
    );
    invalid(
        validate_resources_ref(&wire.pinned_resources).is_err(),
        Field::PinnedResources,
    );
    least
}

fn validate_viewport_ref(value: &ViewportWire) -> Result<(), ()> {
    match value {
        ViewportWire::Applicable { .. } => Ok(()),
        ViewportWire::NotApplicable { reason } => NonApplicableReasonV1::parse(reason)
            .map(|_| ())
            .map_err(|_| ()),
    }
}

fn validate_scale_ref(value: &DeviceScaleWire) -> Result<(), ()> {
    match value {
        DeviceScaleWire::Applicable {
            numerator,
            denominator,
        } => ReducedDeviceScaleV1::new(*numerator, *denominator)
            .map(|_| ())
            .map_err(|_| ()),
        DeviceScaleWire::NotApplicable { reason } => NonApplicableReasonV1::parse(reason)
            .map(|_| ())
            .map_err(|_| ()),
    }
}

fn validate_fonts_ref(value: &FontsWire) -> Result<(), ()> {
    match value {
        FontsWire::NotApplicable { reason } => NonApplicableReasonV1::parse(reason)
            .map(|_| ())
            .map_err(|_| ()),
        FontsWire::Applicable { items } => {
            if items.is_empty() {
                return Err(());
            }
            for item in items {
                ControlledFontIdentityV1::new(
                    ExternalIdentityV1::parse(&item.family).map_err(|_| ())?,
                    ExternalIdentityV1::parse(&item.face_style).map_err(|_| ())?,
                    ExternalVersionV1::parse(&item.version).map_err(|_| ())?,
                    Sha256Digest::parse(&item.file_sha256).map_err(|_| ())?,
                )
                .map_err(|_| ())?;
            }
            Ok(())
        }
    }
}

fn validate_resources_ref(items: &[ResourceWire]) -> Result<(), ()> {
    for item in items {
        PinnedResourceIdentityV1::new(
            ExternalIdentityV1::parse(&item.identity).map_err(|_| ())?,
            Sha256Digest::parse(&item.content_sha256).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
    }
    Ok(())
}

fn validate_track(wire: TrackWire) -> Result<ValidatedAdvisoryTrack, ExternalRegistryDiagnostic> {
    let mut least = None;
    if !AdvisoryTrackId::is_valid(&wire.track_id) {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidTrackId, Field::TrackId),
        );
    }
    for (value, field) in [
        (&wire.engine_product, Field::EngineProduct),
        (&wire.platform_os_family, Field::PlatformOsFamily),
        (&wire.architecture, Field::Architecture),
        (&wire.capture_algorithm, Field::CaptureAlgorithm),
        (&wire.collection_policy, Field::CollectionPolicy),
    ] {
        if ExternalIdentityV1::parse(value).is_err() {
            keep_least_without_subject(&mut least, local_field(Kind::InvalidTrackField, field));
        }
    }
    for (value, field) in [
        (
            &wire.capture_algorithm_version,
            Field::CaptureAlgorithmVersion,
        ),
        (
            &wire.collection_policy_version,
            Field::CollectionPolicyVersion,
        ),
    ] {
        if ExternalVersionV1::parse(value).is_err() {
            keep_least_without_subject(&mut least, local_field(Kind::InvalidTrackField, field));
        }
    }
    if wire.target_parser_input_context != "static-text-html-utf8-scripting-disabled-v1" {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidTrackField, Field::TargetParserInputContext),
        );
    }
    if ComparableObservationSurface::parse(&wire.comparable_observation_surface).is_none() {
        keep_least_without_subject(
            &mut least,
            local_field(
                Kind::UnsupportedComparableSurface,
                Field::ComparableObservationSurface,
            ),
        );
    }
    if let Some(problem) = least {
        return Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Track {
                track_id: wire.track_id,
            },
        ));
    }
    let TrackWire {
        track_id,
        engine_product,
        platform_os_family,
        architecture,
        comparable_observation_surface,
        capture_algorithm,
        capture_algorithm_version,
        target_parser_input_context: _,
        collection_policy,
        collection_policy_version,
    } = wire;
    let mut id = match AdvisoryTrackId::parse_owned(track_id) {
        Ok(id) => id,
        Err(track_id) => {
            return Err(local_field(Kind::InvalidTrackId, Field::TrackId)
                .with_subject(Phase::RecordLocal, Subject::Track { track_id }));
        }
    };
    let parsed: Result<_, LocalDiagnostic> = (|| {
        Ok((
            ExternalIdentityV1::parse(&architecture)
                .map_err(|_| local_field(Kind::InvalidTrackField, Field::Architecture))?,
            ExternalIdentityV1::parse(&capture_algorithm)
                .map_err(|_| local_field(Kind::InvalidTrackField, Field::CaptureAlgorithm))?,
            ExternalVersionV1::parse(&capture_algorithm_version).map_err(|_| {
                local_field(Kind::InvalidTrackField, Field::CaptureAlgorithmVersion)
            })?,
            ExternalIdentityV1::parse(&collection_policy)
                .map_err(|_| local_field(Kind::InvalidTrackField, Field::CollectionPolicy))?,
            ExternalVersionV1::parse(&collection_policy_version).map_err(|_| {
                local_field(Kind::InvalidTrackField, Field::CollectionPolicyVersion)
            })?,
            ComparableObservationSurface::parse(&comparable_observation_surface).ok_or_else(
                || {
                    local_field(
                        Kind::UnsupportedComparableSurface,
                        Field::ComparableObservationSurface,
                    )
                },
            )?,
            ExternalIdentityV1::parse(&engine_product)
                .map_err(|_| local_field(Kind::InvalidTrackField, Field::EngineProduct))?,
            ExternalIdentityV1::parse(&platform_os_family)
                .map_err(|_| local_field(Kind::InvalidTrackField, Field::PlatformOsFamily))?,
            TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
        ))
    })();
    match parsed {
        Ok((
            architecture,
            capture_algorithm,
            capture_algorithm_version,
            collection_policy,
            collection_policy_version,
            comparable,
            engine_product,
            platform_os_family,
            target_parser_input_context,
        )) => Ok(ValidatedAdvisoryTrack {
            id,
            architecture,
            capture_algorithm,
            capture_algorithm_version,
            collection_policy,
            collection_policy_version,
            comparable,
            engine_product,
            platform_os_family,
            target_parser_input_context,
        }),
        Err(problem) => Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Track {
                track_id: id.take_string(),
            },
        )),
    }
}

fn validate_attachment(
    wire: AttachmentWire,
) -> Result<TypedAttachment, ExternalRegistryDiagnostic> {
    let raw_subject = ExternalRegistryAttachmentSubjectKey::new(
        wire.test_id,
        wire.observation_surface,
        wire.execution_variant.kind,
        wire.comparable_observation_surface,
        wire.track_id,
        wire.capture_id,
    );
    let mut least = None;
    if parse_dom_surface(raw_subject.observation_surface()).is_none() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidAttachmentField, Field::ObservationSurface),
        );
    }
    if raw_subject.execution_variant_kind() != "singleton" {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidAttachmentField, Field::ExecutionVariant),
        );
    }
    if TestId::parse(raw_subject.test_id()).is_err() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidAttachmentField, Field::TestId),
        );
    }
    if !AdvisoryTrackId::is_valid(raw_subject.track_id()) {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidAttachmentField, Field::TrackId),
        );
    }
    if ExternalCaptureIdClaim::parse(raw_subject.capture_id()).is_err() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidAttachmentField, Field::CaptureId),
        );
    }
    if ComparableObservationSurface::parse(raw_subject.comparable_observation_surface()).is_none() {
        keep_least_without_subject(
            &mut least,
            local_field(
                Kind::UnsupportedComparableSurface,
                Field::ComparableObservationSurface,
            ),
        );
    }
    if let Some(problem) = least {
        return Err(problem.with_subject(Phase::RecordLocal, Subject::Attachment(raw_subject)));
    }
    let parsed: Result<_, LocalDiagnostic> = (|| {
        Ok((
            TestId::parse(raw_subject.test_id())
                .map_err(|_| local_field(Kind::InvalidAttachmentField, Field::TestId))?,
            parse_dom_surface(raw_subject.observation_surface()).ok_or_else(|| {
                local_field(Kind::InvalidAttachmentField, Field::ObservationSurface)
            })?,
            ComparableObservationSurface::parse(raw_subject.comparable_observation_surface())
                .ok_or_else(|| {
                    local_field(
                        Kind::UnsupportedComparableSurface,
                        Field::ComparableObservationSurface,
                    )
                })?,
            ExternalCaptureIdClaim::parse(raw_subject.capture_id())
                .map_err(|_| local_field(Kind::InvalidAttachmentField, Field::CaptureId))?,
        ))
    })();
    match parsed {
        Ok((test_id, observation, comparable, capture_claim)) => Ok(TypedAttachment {
            test_id,
            observation,
            comparable,
            capture_claim,
            raw_subject,
        }),
        Err(problem) => {
            Err(problem.with_subject(Phase::RecordLocal, Subject::Attachment(raw_subject)))
        }
    }
}

fn validate_note(wire: NoteWire) -> Result<TypedNote, ExternalRegistryDiagnostic> {
    let mut least = None;
    if !BaselineNoteId::is_valid(&wire.note_id) {
        keep_least_without_subject(&mut least, local_field(Kind::InvalidNoteId, Field::NoteId));
    }
    if wire.text.len() > 1024 {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::BaselineNoteTextTooLong, Field::Text),
        );
    } else if wire.text.is_empty()
        || wire.text.trim() != wire.text
        || wire.text.chars().any(char::is_control)
    {
        keep_least_without_subject(&mut least, local_field(Kind::InvalidNoteField, Field::Text));
    }
    if parse_dom_surface(&wire.observation_surface).is_none() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidNoteField, Field::ObservationSurface),
        );
    }
    if wire.execution_variant.kind != "singleton" {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidNoteField, Field::ExecutionVariant),
        );
    }
    if TestId::parse(&wire.test_id).is_err() {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidNoteField, Field::TestId),
        );
    }
    if wire
        .capture_id
        .as_deref()
        .map(ExternalCaptureIdClaim::parse)
        .transpose()
        .is_err()
    {
        keep_least_without_subject(
            &mut least,
            local_field(Kind::InvalidNoteField, Field::CaptureId),
        );
    }
    if ComparableObservationSurface::parse(&wire.comparable_observation_surface).is_none() {
        keep_least_without_subject(
            &mut least,
            local_field(
                Kind::UnsupportedComparableSurface,
                Field::ComparableObservationSurface,
            ),
        );
    }
    if let Some(problem) = least {
        return Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Note {
                note_id: wire.note_id,
            },
        ));
    }
    let NoteWire {
        note_id,
        test_id,
        observation_surface,
        execution_variant: _,
        comparable_observation_surface,
        text,
        capture_id,
    } = wire;
    let mut id = match BaselineNoteId::parse_owned(note_id) {
        Ok(id) => id,
        Err(note_id) => {
            return Err(local_field(Kind::InvalidNoteId, Field::NoteId)
                .with_subject(Phase::RecordLocal, Subject::Note { note_id }));
        }
    };
    let parsed: Result<_, LocalDiagnostic> = (|| {
        Ok((
            TestId::parse(&test_id)
                .map_err(|_| local_field(Kind::InvalidNoteField, Field::TestId))?,
            parse_dom_surface(&observation_surface)
                .ok_or_else(|| local_field(Kind::InvalidNoteField, Field::ObservationSurface))?,
            ComparableObservationSurface::parse(&comparable_observation_surface).ok_or_else(
                || {
                    local_field(
                        Kind::UnsupportedComparableSurface,
                        Field::ComparableObservationSurface,
                    )
                },
            )?,
            capture_id
                .as_deref()
                .map(ExternalCaptureIdClaim::parse)
                .transpose()
                .map_err(|_| local_field(Kind::InvalidNoteField, Field::CaptureId))?,
        ))
    })();
    match parsed {
        Ok((test_id, observation, comparable, capture_claim)) => Ok(TypedNote {
            id,
            test_id,
            observation,
            comparable,
            text,
            capture_claim,
        }),
        Err(problem) => Err(problem.with_subject(
            Phase::RecordLocal,
            Subject::Note {
                note_id: id.take_string(),
            },
        )),
    }
}

fn validate_phase5(
    mut local: LocallyValidatedRegistry,
    reservation: &mut impl ReservationPolicy,
) -> Result<UniqueRegistry, ExternalRegistryDiagnostic> {
    let mut captures = Vec::new();
    try_reserve_vec(
        &mut captures,
        local.captures.len(),
        ReservationSite::Phase5Captures,
        reservation,
    )
    .map_err(|_| {
        duplicate_diag(
            Subject::CaptureCollection,
            Kind::DuplicateCaptureId,
            Detail::Field(Field::Allocation),
        )
    })?;
    local
        .captures
        .sort_by(|a, b| a.claim_text.as_bytes().cmp(b.claim_text.as_bytes()));
    for index in 0..local.captures.len() {
        if index + 1 < local.captures.len()
            && local.captures[index].claim_text == local.captures[index + 1].claim_text
        {
            let supplied_capture_id = std::mem::take(&mut local.captures[index].claim_text);
            return Err(duplicate_diag(
                Subject::Capture {
                    supplied_capture_id,
                },
                Kind::DuplicateCaptureId,
                Detail::Field(Field::CaptureId),
            ));
        }
        let capture = &mut local.captures[index];
        if let ApplicabilityV1::Applicable(fonts) = &mut capture.provenance_input.controlled_fonts {
            fonts.sort_by(|a, b| a.canonical_bytes().cmp(b.canonical_bytes()));
            if fonts
                .windows(2)
                .any(|p| p[0].canonical_bytes() == p[1].canonical_bytes())
            {
                return Err(duplicate_diag(
                    Subject::Capture {
                        supplied_capture_id: std::mem::take(&mut capture.claim_text),
                    },
                    Kind::DuplicateControlledFont,
                    Detail::Field(Field::ControlledFonts),
                ));
            }
        }
        capture
            .provenance_input
            .pinned_resources
            .sort_by(|a, b| a.canonical_bytes().cmp(b.canonical_bytes()));
        if capture
            .provenance_input
            .pinned_resources
            .windows(2)
            .any(|p| p[0].canonical_bytes() == p[1].canonical_bytes())
        {
            return Err(duplicate_diag(
                Subject::Capture {
                    supplied_capture_id: std::mem::take(&mut capture.claim_text),
                },
                Kind::DuplicatePinnedResource,
                Detail::Field(Field::PinnedResources),
            ));
        }
    }
    for capture in local.captures {
        let LocallyValidatedCapture {
            claim,
            claim_text,
            artifact_path,
            provenance_input,
        } = capture;
        let provenance = match ExternalCaptureProvenanceV1::try_from_input(provenance_input) {
            Ok(provenance) => provenance,
            Err(error) => {
                let subject = Subject::Capture {
                    supplied_capture_id: claim_text,
                };
                return Err(match error {
                    CaptureV1Error::DuplicateControlledFont => duplicate_diag(
                        subject,
                        Kind::DuplicateControlledFont,
                        Detail::Field(Field::ControlledFonts),
                    ),
                    CaptureV1Error::DuplicatePinnedResource => duplicate_diag(
                        subject,
                        Kind::DuplicatePinnedResource,
                        Detail::Field(Field::PinnedResources),
                    ),
                    _ => duplicate_diag(
                        subject,
                        Kind::DuplicateCaptureId,
                        Detail::Component(Component::ProvenanceInvariant),
                    ),
                });
            }
        };
        captures.push(UniqueCapture {
            claim,
            claim_text,
            artifact_path,
            provenance,
        });
    }
    local
        .tracks
        .sort_by(|a, b| a.id.as_str().as_bytes().cmp(b.id.as_str().as_bytes()));
    if let Some(index) = local.tracks.windows(2).position(|p| p[0].id == p[1].id) {
        let track_id = local.tracks[index].id.take_string();
        return Err(duplicate_diag(
            Subject::Track { track_id },
            Kind::DuplicateTrackId,
            Detail::Field(Field::TrackId),
        ));
    }
    local.attachments.sort_by(|a, b| {
        a.uniqueness_cmp(b)
            .then_with(|| a.raw_subject.contract_cmp(&b.raw_subject))
    });
    if let Some(index) = local
        .attachments
        .windows(2)
        .position(|p| p[0].has_same_uniqueness_key(&p[1]))
    {
        let subject = local.attachments[index].raw_subject.take();
        return Err(duplicate_diag(
            Subject::Attachment(subject),
            Kind::DuplicateAttachmentKey,
            Detail::Field(Field::AttachmentKey),
        ));
    }
    local
        .notes
        .sort_by(|a, b| a.id.as_str().as_bytes().cmp(b.id.as_str().as_bytes()));
    if let Some(index) = local.notes.windows(2).position(|p| p[0].id == p[1].id) {
        let note_id = local.notes[index].id.take_string();
        return Err(duplicate_diag(
            Subject::Note { note_id },
            Kind::DuplicateNoteId,
            Detail::Field(Field::NoteId),
        ));
    }
    Ok(UniqueRegistry {
        captures,
        attachments: local.attachments,
        tracks: local.tracks,
        notes: local.notes,
        declared_artifact_bytes_total: local.declared_artifact_bytes_total,
    })
}

fn artifact_path_is_valid(value: &str) -> bool {
    let Some(file) = value.strip_prefix("tests/conformance/external/captures/") else {
        return false;
    };
    if file.contains('/') || !file.ends_with(".web-observable-dom-tree-v1.txt") {
        return false;
    }
    PortablePathComponent::is_valid(file)
}

fn parse_dom_surface(value: &str) -> Option<ObservationSurface> {
    ObservationSurface::parse(value).filter(|surface| *surface == ObservationSurface::DomTree)
}
fn identity(value: &str, field: Field) -> Result<ExternalIdentityV1, LocalDiagnostic> {
    ExternalIdentityV1::parse(value).map_err(|_| local_field(Kind::InvalidCaptureField, field))
}
fn version(value: &str, field: Field) -> Result<ExternalVersionV1, LocalDiagnostic> {
    ExternalVersionV1::parse(value).map_err(|_| local_field(Kind::InvalidCaptureField, field))
}
fn digest(value: &str, field: Field) -> Result<Sha256Digest, LocalDiagnostic> {
    Sha256Digest::parse(value).map_err(|_| local_field(Kind::InvalidCaptureField, field))
}
fn parse_resource_policy(value: &str) -> Option<ResourceNetworkPolicyV1> {
    match value {
        "offline" => Some(ResourceNetworkPolicyV1::Offline),
        "fixture-local-only" => Some(ResourceNetworkPolicyV1::FixtureLocalOnly),
        "recorded-local-closure" => Some(ResourceNetworkPolicyV1::RecordedLocalClosure),
        _ => None,
    }
}
fn parse_viewport(
    value: ViewportWire,
) -> Result<ApplicabilityV1<ViewportCssPixelsV1>, LocalDiagnostic> {
    match value {
        ViewportWire::Applicable {
            width_css_px,
            height_css_px,
        } => Ok(ApplicabilityV1::Applicable(ViewportCssPixelsV1 {
            width: width_css_px,
            height: height_css_px,
        })),
        ViewportWire::NotApplicable { reason } => Ok(ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse(&reason)
                .map_err(|_| local_field(Kind::InvalidCaptureField, Field::Viewport))?,
        )),
    }
}
fn parse_scale(
    value: DeviceScaleWire,
) -> Result<ApplicabilityV1<ReducedDeviceScaleV1>, LocalDiagnostic> {
    match value {
        DeviceScaleWire::Applicable {
            numerator,
            denominator,
        } => Ok(ApplicabilityV1::Applicable(
            ReducedDeviceScaleV1::new(numerator, denominator)
                .map_err(|_| local_field(Kind::InvalidCaptureField, Field::DeviceScale))?,
        )),
        DeviceScaleWire::NotApplicable { reason } => Ok(ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse(&reason)
                .map_err(|_| local_field(Kind::InvalidCaptureField, Field::DeviceScale))?,
        )),
    }
}
fn parse_fonts(
    value: FontsWire,
) -> Result<ApplicabilityV1<Vec<ControlledFontIdentityV1>>, LocalDiagnostic> {
    match value {
        FontsWire::NotApplicable { reason } => Ok(ApplicabilityV1::NotApplicable(
            NonApplicableReasonV1::parse(&reason)
                .map_err(|_| local_field(Kind::InvalidCaptureField, Field::ControlledFonts))?,
        )),
        FontsWire::Applicable { items } => {
            if items.is_empty() {
                return Err(local_field(
                    Kind::InvalidCaptureField,
                    Field::ControlledFonts,
                ));
            }
            let mut out = Vec::new();
            out.try_reserve(items.len()).map_err(|_| {
                local_field(Kind::InvalidCaptureField, Field::ControlledFontsAllocation)
            })?;
            for item in items {
                out.push(
                    ControlledFontIdentityV1::new(
                        ExternalIdentityV1::parse(&item.family).map_err(|_| {
                            local_field(Kind::InvalidCaptureField, Field::FontFamily)
                        })?,
                        ExternalIdentityV1::parse(&item.face_style).map_err(|_| {
                            local_field(Kind::InvalidCaptureField, Field::FontFaceStyle)
                        })?,
                        ExternalVersionV1::parse(&item.version).map_err(|_| {
                            local_field(Kind::InvalidCaptureField, Field::FontVersion)
                        })?,
                        Sha256Digest::parse(&item.file_sha256).map_err(|_| {
                            local_field(Kind::InvalidCaptureField, Field::FontSha256)
                        })?,
                    )
                    .map_err(|_| local_field(Kind::InvalidCaptureField, Field::FontCanonical))?,
                );
            }
            Ok(ApplicabilityV1::Applicable(out))
        }
    }
}
fn parse_resources(
    items: Vec<ResourceWire>,
) -> Result<Vec<PinnedResourceIdentityV1>, LocalDiagnostic> {
    let mut out = Vec::new();
    out.try_reserve(items.len())
        .map_err(|_| local_field(Kind::InvalidCaptureField, Field::ResourcesAllocation))?;
    for item in items {
        out.push(
            PinnedResourceIdentityV1::new(
                ExternalIdentityV1::parse(&item.identity)
                    .map_err(|_| local_field(Kind::InvalidCaptureField, Field::ResourceIdentity))?,
                Sha256Digest::parse(&item.content_sha256)
                    .map_err(|_| local_field(Kind::InvalidCaptureField, Field::ResourceSha256))?,
            )
            .map_err(|_| local_field(Kind::InvalidCaptureField, Field::ResourceCanonical))?,
        );
    }
    Ok(out)
}

const fn local_field(kind: Kind, field: Field) -> LocalDiagnostic {
    LocalDiagnostic::new(kind, Detail::Field(field))
}
fn duplicate_diag(subject: Subject, kind: Kind, detail: Detail) -> ExternalRegistryDiagnostic {
    ExternalRegistryDiagnostic::new(Phase::DuplicateIdentity, subject, kind, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::external_registry::allocation::RejectReservationAt;

    const CLAIM: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn capture(capture_id: &str) -> CaptureWire {
        CaptureWire {
            capture_id: capture_id.to_owned(),
            artifact_path:
                "tests/conformance/external/captures/capture.web-observable-dom-tree-v1.txt"
                    .to_owned(),
            provenance_format: "borrowser-external-capture-provenance-v1".to_owned(),
            engine_product: "engine".to_owned(),
            engine_version: "1".to_owned(),
            engine_build_revision: None,
            platform_os_family: "os".to_owned(),
            platform_os_version: "1".to_owned(),
            architecture: "arch".to_owned(),
            viewport: ViewportWire::NotApplicable {
                reason: "surface-independent".to_owned(),
            },
            device_scale: DeviceScaleWire::NotApplicable {
                reason: "surface-independent".to_owned(),
            },
            controlled_fonts: FontsWire::NotApplicable {
                reason: "font-independent".to_owned(),
            },
            resource_network_policy: "offline".to_owned(),
            pinned_resources: Vec::new(),
            fixture_source_project: "fixture".to_owned(),
            fixture_immutable_revision: "revision".to_owned(),
            fixture_content_sha256: DIGEST.to_owned(),
            capture_mechanism: "mechanism".to_owned(),
            capture_mechanism_version: "1".to_owned(),
            capture_algorithm: "algorithm".to_owned(),
            capture_algorithm_version: "1".to_owned(),
            capture_algorithm_source_sha256: DIGEST.to_owned(),
            capture_configuration_sha256: DIGEST.to_owned(),
            invocation_arguments: Vec::new(),
            artifact_format: "web-observable-dom-tree-v1".to_owned(),
            artifact_utf8_byte_length: 0,
            artifact_sha256: DIGEST.to_owned(),
            target_parser_input_context: "static-text-html-utf8-scripting-disabled-v1".to_owned(),
            collection_policy: "stable".to_owned(),
            collection_policy_version: "1".to_owned(),
        }
    }

    fn track(track_id: &str) -> TrackWire {
        TrackWire {
            track_id: track_id.to_owned(),
            engine_product: "engine".to_owned(),
            platform_os_family: "os".to_owned(),
            architecture: "arch".to_owned(),
            comparable_observation_surface: "web-observable-dom-tree-v1".to_owned(),
            capture_algorithm: "algorithm".to_owned(),
            capture_algorithm_version: "1".to_owned(),
            target_parser_input_context: "static-text-html-utf8-scripting-disabled-v1".to_owned(),
            collection_policy: "stable".to_owned(),
            collection_policy_version: "1".to_owned(),
        }
    }

    fn attachment(test_id: &str) -> AttachmentWire {
        AttachmentWire {
            test_id: test_id.to_owned(),
            observation_surface: "dom-tree".to_owned(),
            execution_variant: ExecutionVariantWire {
                kind: "singleton".to_owned(),
            },
            comparable_observation_surface: "web-observable-dom-tree-v1".to_owned(),
            track_id: "track".to_owned(),
            capture_id: CLAIM.to_owned(),
        }
    }

    fn note(note_id: &str) -> NoteWire {
        NoteWire {
            note_id: note_id.to_owned(),
            test_id: "dom-tree-basic-document".to_owned(),
            observation_surface: "dom-tree".to_owned(),
            execution_variant: ExecutionVariantWire {
                kind: "singleton".to_owned(),
            },
            comparable_observation_surface: "web-observable-dom-tree-v1".to_owned(),
            text: "advisory".to_owned(),
            capture_id: None,
        }
    }

    fn wire(captures: Vec<CaptureWire>, tracks: Vec<TrackWire>) -> RegistryWire {
        RegistryWire {
            format: EXTERNAL_COMPARISON_REGISTRY_FORMAT_V1.to_owned(),
            captures,
            attachments: vec![attachment("dom-tree-basic-document")],
            advisory_tracks: tracks,
            baseline_notes: vec![note("note")],
        }
    }

    #[test]
    fn earlier_capture_diagnostic_beats_every_later_collection_allocation_failure() {
        for site in [
            ReservationSite::Phase4Tracks,
            ReservationSite::Phase4Attachments,
            ReservationSite::Phase4Notes,
        ] {
            let error = validate_phase4(
                wire(vec![capture("invalid")], vec![track("track")]),
                0,
                &mut RejectReservationAt::new(site),
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), Kind::InvalidCaptureIdClaim, "{site:?}");
            assert_eq!(
                error.subject(),
                &Subject::Capture {
                    supplied_capture_id: "invalid".to_owned(),
                },
                "{site:?}",
            );
        }
    }

    #[test]
    fn earlier_track_diagnostic_beats_attachment_and_note_allocation_failures() {
        for site in [
            ReservationSite::Phase4Attachments,
            ReservationSite::Phase4Notes,
        ] {
            let error = validate_phase4(
                wire(Vec::new(), vec![track("INVALID")]),
                0,
                &mut RejectReservationAt::new(site),
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), Kind::InvalidTrackId, "{site:?}");
            assert_eq!(
                error.subject(),
                &Subject::Track {
                    track_id: "INVALID".to_owned(),
                },
                "{site:?}",
            );
        }
    }

    #[test]
    fn collection_subject_precedes_individual_and_record_order_is_irrelevant() {
        let error = validate_phase4(
            wire(vec![capture("z"), capture("a")], vec![track("track")]),
            0,
            &mut RejectReservationAt::new(ReservationSite::Phase4Captures),
        )
        .err()
        .unwrap();
        assert_eq!(error.subject(), &Subject::CaptureCollection);
        assert_eq!(error.detail(), Detail::Field(Field::Allocation));

        for mut captures in [
            vec![capture("z"), capture("a")],
            vec![capture("a"), capture("z")],
        ] {
            let error = validate_phase4(
                wire(std::mem::take(&mut captures), vec![track("track")]),
                0,
                &mut RejectReservationAt::new(ReservationSite::Phase4Notes),
            )
            .err()
            .unwrap();
            assert_eq!(
                error.subject(),
                &Subject::Capture {
                    supplied_capture_id: "a".to_owned(),
                },
            );
        }
    }
}
