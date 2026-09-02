use super::failure_spelling::{parse_parser_observation_failure, parse_runner_invariant};
use super::load::{
    DeliveryValidationError, FixtureFileAccess, FixtureLoadError, FixtureLoadErrorKind,
    FixturePlanningInvariant, FixtureRepositoryPolicy, normalize_relative_path,
    validate_relative_path,
};
use super::model::*;
use super::schema::*;
use external_test_provenance::parse_external_provenance_v1;
use html::ElementNamespace;
use html::conformance::InvariantFailureCode;
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

/// A fixture whose complete serialized declaration passed its selected strict
/// versioned validation boundary.
///
/// Fields and construction stay in this module so callers cannot assemble a
/// partially validated value from otherwise plausible component values.
#[derive(Clone, Debug)]
pub struct ValidatedFixtureSpec {
    id: FixtureId,
    bundle: FixtureBundle,
    #[cfg(test)]
    format: FixtureFormatVersion,
    source: FixtureSource,
    input: ExactInput,
    execution_plan: ValidatedExecutionPlan,
    expectations: EnabledExpectations,
    disposition: FixtureDisposition,
    description: Option<String>,
    comments: Vec<String>,
    optional_extensions: BTreeMap<String, ExtensionDeclaration>,
    required_unknown_extensions: Vec<String>,
}

impl ValidatedFixtureSpec {
    pub fn id(&self) -> &FixtureId {
        &self.id
    }

    pub fn repository_relative_path(&self) -> &str {
        self.bundle.repository_relative_path()
    }

    pub fn input_bytes(&self) -> &[u8] {
        self.input.bytes()
    }

    pub fn input_text(&self) -> Option<&str> {
        self.input.text()
    }

    pub fn input_path(&self) -> &str {
        self.input.path()
    }

    pub fn input_sha256(&self) -> &str {
        self.input.sha256()
    }

    pub fn source_kind(&self) -> FixtureSourceKind {
        self.source.kind()
    }

    pub fn source_reference(&self) -> Option<&str> {
        self.source.reference()
    }

    pub fn target_kind(&self) -> ParserTargetKind {
        match self.execution().target() {
            ValidatedParserTarget::StandaloneTokenizer => ParserTargetKind::StandaloneTokenizer,
            ValidatedParserTarget::Document { .. } => ParserTargetKind::Document,
            ValidatedParserTarget::Fragment { .. } => ParserTargetKind::Fragment,
        }
    }

    pub fn scripting_mode(&self) -> Option<ScriptingMode> {
        match self.execution().target() {
            ValidatedParserTarget::StandaloneTokenizer => None,
            ValidatedParserTarget::Document { scripting }
            | ValidatedParserTarget::Fragment { scripting, .. } => Some(*scripting),
        }
    }

    pub fn fragment_namespace(&self) -> Option<ElementNamespace> {
        match self.execution().target() {
            ValidatedParserTarget::Fragment { context, .. } => Some(context.namespace()),
            ValidatedParserTarget::StandaloneTokenizer | ValidatedParserTarget::Document { .. } => {
                None
            }
        }
    }

    pub fn fragment_local_name(&self) -> Option<&str> {
        match self.execution().target() {
            ValidatedParserTarget::Fragment { context, .. } => Some(context.local_name()),
            ValidatedParserTarget::StandaloneTokenizer | ValidatedParserTarget::Document { .. } => {
                None
            }
        }
    }

    pub fn reference_delivery(&self) -> &DeliveryName {
        self.execution().reference_delivery()
    }

    pub fn delivery_names(&self) -> impl ExactSizeIterator<Item = &DeliveryName> {
        self.execution()
            .deliveries()
            .iter()
            .map(ValidatedDelivery::name)
    }

    pub fn delivery_boundaries(&self, name: &str) -> Option<Option<&[usize]>> {
        self.execution()
            .deliveries()
            .iter()
            .find(|delivery| delivery.name().as_str() == name)
            .map(ValidatedDelivery::boundaries)
    }

    pub fn transition_deliveries(&self) -> impl Iterator<Item = &DeliveryName> {
        match self.expectations.transitions() {
            ExpectedSurface::NotDeclared => [].iter(),
            ExpectedSurface::Compare(transitions) => transitions.as_slice().iter(),
        }
        .map(TransitionSnapshotExpectation::delivery)
    }

    pub fn execution_model(&self) -> ParserFixtureExecutionModel {
        match self.execution_plan() {
            ValidatedExecutionPlan::SingleDelivery(_) => {
                ParserFixtureExecutionModel::LegacySingleDelivery
            }
            ValidatedExecutionPlan::Parity(_) => {
                ParserFixtureExecutionModel::CanonicalObservationParity
            }
        }
    }

    pub fn disposition_kind(&self) -> FixtureDispositionKind {
        match self.disposition() {
            FixtureDisposition::Active => FixtureDispositionKind::Active,
            FixtureDisposition::ExpectedUnsupported { .. } => {
                FixtureDispositionKind::ExpectedUnsupported
            }
            FixtureDisposition::ExpectedFailure { .. }
            | FixtureDisposition::ExpectedFailureV2 { .. } => {
                FixtureDispositionKind::ExpectedFailure
            }
            FixtureDisposition::Skipped { .. } => FixtureDispositionKind::Skipped,
        }
    }

    pub fn declared_expectations(&self) -> std::vec::IntoIter<DeclaredExpectation> {
        let expectations = self.expectations();
        let mut declared = Vec::with_capacity(9);
        if expectations.is_declared(ExpectationSurface::Tokens) {
            declared.push(DeclaredExpectation::Tokens);
        }
        if let ExpectedSurface::Compare(expectation) = expectations.parse_errors() {
            declared.push(DeclaredExpectation::ParseErrors(match expectation {
                ParseErrorExpectation::Exact(_) => ParseErrorExpectationStrength::Exact,
                ParseErrorExpectation::Count(expected) => ParseErrorExpectationStrength::Count {
                    expected: *expected,
                },
            }));
        }
        for (surface, declaration) in [
            (
                ExpectationSurface::ImplementationDiagnostics,
                DeclaredExpectation::ImplementationDiagnostics,
            ),
            (
                ExpectationSurface::DocumentMode,
                DeclaredExpectation::DocumentMode,
            ),
            (ExpectationSurface::Tree, DeclaredExpectation::Tree),
            (ExpectationSurface::Patches, DeclaredExpectation::Patches),
            (
                ExpectationSurface::Transitions,
                DeclaredExpectation::Transitions,
            ),
            (
                ExpectationSurface::UnsupportedFeatures,
                DeclaredExpectation::UnsupportedFeatures,
            ),
            (
                ExpectationSurface::FinalInvariants,
                DeclaredExpectation::FinalInvariants,
            ),
        ] {
            if expectations.is_declared(surface) {
                declared.push(declaration);
            }
        }
        declared.into_iter()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn comments(&self) -> &[String] {
        &self.comments
    }

    pub fn optional_extensions(&self) -> &BTreeMap<String, ExtensionDeclaration> {
        &self.optional_extensions
    }

    pub(super) fn bundle(&self) -> &FixtureBundle {
        &self.bundle
    }

    #[cfg(test)]
    pub(super) fn format(&self) -> FixtureFormatVersion {
        self.format
    }

    pub(super) fn input(&self) -> &ExactInput {
        &self.input
    }

    pub(super) fn execution(&self) -> &ValidatedExecution {
        self.execution_plan.execution()
    }

    pub(super) fn execution_plan(&self) -> &ValidatedExecutionPlan {
        &self.execution_plan
    }

    pub(super) fn expectations(&self) -> &EnabledExpectations {
        &self.expectations
    }

    pub(super) fn disposition(&self) -> &FixtureDisposition {
        &self.disposition
    }

    pub(super) fn required_unknown_extensions(&self) -> &[String] {
        &self.required_unknown_extensions
    }
}

pub(super) fn validate_fixture_v1(
    declaration: FixtureFileV1,
    bundle: FixtureBundle,
    repository_policy: FixtureRepositoryPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ValidatedFixtureSpec, FixtureLoadError> {
    if declaration.format != FIXTURE_FORMAT_V1 {
        return Err(bundle_error(
            &bundle,
            FixtureLoadErrorKind::UnsupportedFixtureFormat(declaration.format),
        ));
    }
    let id = validate_fixture_id(&bundle, declaration.id)?;
    let source = validate_source(&bundle, declaration.source)?;

    let input_path = declaration.input.path.clone();
    validate_relative_path(&input_path).map_err(|kind| bundle_error(&bundle, kind))?;
    let input_bytes = file_access.read_regular_file(&bundle, &input_path)?;
    validate_sha256(&bundle, &declaration.input.sha256, &input_bytes)?;
    let input = validate_input(&bundle, declaration.input, input_bytes)?;

    let execution = validate_execution_v1(&bundle, &input, declaration.execution)?;
    let expectations = validate_expectations(
        &bundle,
        &execution,
        declaration.expectations,
        SidecarValidationPolicy::LegacyV1ContentReadable,
        file_access,
    )?;
    if !has_any_expectation(&expectations) {
        return invalid_combination(&bundle, "fixture must declare at least one expectation");
    }
    validate_orphan_sidecars(&bundle, &input_path, &source, &expectations)?;
    let (optional_extensions, required_unknown_extensions) =
        validate_extensions(&bundle, declaration.extensions)?;
    let disposition = validate_disposition(
        &bundle,
        declaration.disposition,
        &input,
        &execution,
        &expectations,
        &required_unknown_extensions,
    )?;
    validate_source_disposition_policy(&bundle, repository_policy, &source, &disposition)?;

    Ok(ValidatedFixtureSpec {
        id,
        bundle,
        #[cfg(test)]
        format: FixtureFormatVersion::V1,
        source,
        input,
        execution_plan: ValidatedExecutionPlan::SingleDelivery(
            ValidatedSingleExecution::validated(execution),
        ),
        expectations,
        disposition,
        description: declaration.metadata.description,
        comments: declaration.metadata.comments,
        optional_extensions,
        required_unknown_extensions,
    })
}

fn validate_v2_expectation_identities(
    bundle: &FixtureBundle,
    expectations: &EnabledExpectations,
) -> Result<(), FixtureLoadError> {
    let ExpectedSurface::Compare(transitions) = expectations.transitions() else {
        return Ok(());
    };
    let mut deliveries = BTreeSet::new();
    for transition in transitions {
        if !deliveries.insert(transition.delivery().clone()) {
            return invalid_combination(
                bundle,
                "fixture-v2 transition expectations must have unique delivery identities",
            );
        }
    }
    Ok(())
}

pub(super) fn validate_fixture_v2(
    declaration: FixtureFileV2,
    bundle: FixtureBundle,
    repository_policy: FixtureRepositoryPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ValidatedFixtureSpec, FixtureLoadError> {
    if declaration.format != FIXTURE_FORMAT_V2 {
        return Err(bundle_error(
            &bundle,
            FixtureLoadErrorKind::UnsupportedFixtureFormat(declaration.format),
        ));
    }
    let id = validate_fixture_id(&bundle, declaration.id)?;
    let source = validate_source(&bundle, declaration.source)?;

    let input_path = declaration.input.path.clone();
    validate_relative_path(&input_path).map_err(|kind| bundle_error(&bundle, kind))?;
    let input_bytes = file_access.read_regular_file(&bundle, &input_path)?;
    validate_sha256(&bundle, &declaration.input.sha256, &input_bytes)?;
    let input = validate_input(&bundle, declaration.input, input_bytes)?;

    let execution = validate_execution_v2(&bundle, &input, declaration.execution)?;
    let expectations = validate_expectations(
        &bundle,
        &execution,
        declaration.expectations,
        SidecarValidationPolicy::MetadataOnlyV2,
        file_access,
    )?;
    validate_v2_expectation_identities(&bundle, &expectations)?;
    if !has_any_expectation(&expectations) {
        return invalid_combination(&bundle, "fixture must declare at least one expectation");
    }
    validate_orphan_sidecars(&bundle, &input_path, &source, &expectations)?;
    let (optional_extensions, required_unknown_extensions) =
        validate_extensions(&bundle, declaration.extensions)?;
    let disposition = validate_disposition_v2(
        &bundle,
        declaration.disposition,
        &input,
        &execution,
        &expectations,
        &required_unknown_extensions,
    )?;
    validate_source_disposition_policy(&bundle, repository_policy, &source, &disposition)?;

    let strategies = plan_v2_strategies(&bundle, &input, &execution)?;
    Ok(ValidatedFixtureSpec {
        id,
        bundle,
        #[cfg(test)]
        format: FixtureFormatVersion::V2,
        source,
        input,
        execution_plan: ValidatedExecutionPlan::Parity(ValidatedParityExecution::validated(
            execution, strategies,
        )),
        expectations,
        disposition,
        description: declaration.metadata.description,
        comments: declaration.metadata.comments,
        optional_extensions,
        required_unknown_extensions,
    })
}

pub(super) fn validate_fixture_v3(
    declaration: FixtureFileV3,
    bundle: FixtureBundle,
    repository_policy: FixtureRepositoryPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ValidatedFixtureSpec, FixtureLoadError> {
    if declaration.format != FIXTURE_FORMAT_V3 {
        return Err(bundle_error(
            &bundle,
            FixtureLoadErrorKind::UnsupportedFixtureFormat(declaration.format),
        ));
    }
    let id = validate_fixture_id(&bundle, declaration.id)?;
    let source = validate_source_v3(&bundle, declaration.source, file_access)?;

    let input_path = declaration.input.path.clone();
    validate_relative_path(&input_path).map_err(|kind| bundle_error(&bundle, kind))?;
    let input_bytes = file_access.read_regular_file(&bundle, &input_path)?;
    validate_sha256(&bundle, &declaration.input.sha256, &input_bytes)?;
    let input = validate_input(&bundle, declaration.input, input_bytes)?;

    let execution = validate_execution_v2(&bundle, &input, declaration.execution)?;
    let expectations = validate_expectations_v3(
        &bundle,
        &execution,
        declaration.expectations,
        SidecarValidationPolicy::MetadataOnlyV2,
        file_access,
    )?;
    validate_v2_expectation_identities(&bundle, &expectations)?;
    if !has_any_expectation(&expectations) {
        return invalid_combination(&bundle, "fixture must declare at least one expectation");
    }
    validate_orphan_sidecars(&bundle, &input_path, &source, &expectations)?;
    let (optional_extensions, required_unknown_extensions) =
        validate_extensions(&bundle, declaration.extensions)?;
    let disposition = validate_disposition_v2(
        &bundle,
        declaration.disposition,
        &input,
        &execution,
        &expectations,
        &required_unknown_extensions,
    )?;
    validate_source_disposition_policy(&bundle, repository_policy, &source, &disposition)?;

    let strategies = plan_v2_strategies(&bundle, &input, &execution)?;
    Ok(ValidatedFixtureSpec {
        id,
        bundle,
        #[cfg(test)]
        format: FixtureFormatVersion::V3,
        source,
        input,
        execution_plan: ValidatedExecutionPlan::Parity(ValidatedParityExecution::validated(
            execution, strategies,
        )),
        expectations,
        disposition,
        description: declaration.metadata.description,
        comments: declaration.metadata.comments,
        optional_extensions,
        required_unknown_extensions,
    })
}

fn validate_fixture_id(
    bundle: &FixtureBundle,
    value: String,
) -> Result<FixtureId, FixtureLoadError> {
    if value != value.to_ascii_lowercase() {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::CaseUnsafeFixtureId(value),
        ));
    }
    if !is_kebab_identifier(&value) {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidFixtureId(value),
        ));
    }
    Ok(FixtureId::validated(value))
}

fn validate_source(
    bundle: &FixtureBundle,
    source: FixtureSourceDeclaration,
) -> Result<FixtureSource, FixtureLoadError> {
    match (source.kind, source.provenance, source.tracking_issue) {
        (FixtureSourceKindDeclaration::Native, None, None) => Ok(FixtureSource::Native),
        (FixtureSourceKindDeclaration::External, Some(_), None) => invalid_combination(
            bundle,
            "external fixtures require fixture-v3 structured provenance",
        ),
        (FixtureSourceKindDeclaration::Quarantine, None, Some(tracking_issue)) => {
            require_non_empty(bundle, "quarantine tracking issue", &tracking_issue)?;
            Ok(FixtureSource::Quarantine { tracking_issue })
        }
        _ => invalid_combination(
            bundle,
            "source kind must declare exactly its required provenance or tracking field",
        ),
    }
}

fn validate_source_v3(
    bundle: &FixtureBundle,
    source: FixtureSourceDeclarationV3,
    file_access: &mut impl FixtureFileAccess,
) -> Result<FixtureSource, FixtureLoadError> {
    match (
        source.kind,
        source.provenance_record,
        source.provenance_sha256,
        source.tracking_issue,
    ) {
        (FixtureSourceKindDeclaration::Native, None, None, None) => Ok(FixtureSource::Native),
        (FixtureSourceKindDeclaration::Quarantine, None, None, Some(tracking_issue)) => {
            require_non_empty(bundle, "quarantine tracking issue", &tracking_issue)?;
            Ok(FixtureSource::Quarantine { tracking_issue })
        }
        (FixtureSourceKindDeclaration::External, Some(record_path), Some(record_sha256), None) => {
            validate_relative_path(&record_path).map_err(|kind| bundle_error(bundle, kind))?;
            let record_bytes = file_access.read_regular_file(bundle, &record_path)?;
            validate_sha256(bundle, &record_sha256, &record_bytes)?;
            let declaration = parse_external_provenance_v1(&record_bytes).map_err(|error| {
                bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidFixtureToml(format!(
                        "external provenance record: {error}"
                    )),
                )
            })?;
            let case_identity = declaration.case_identity();
            Ok(FixtureSource::External {
                provenance: record_path.clone(),
                provenance_record: Box::new(ExternalProvenance {
                    record_path,
                    provenance_sha256: record_sha256,
                    upstream_project: declaration.upstream_project().as_str().to_owned(),
                    upstream_revision: declaration.upstream_revision().as_str().to_owned(),
                    upstream_path: declaration.upstream_path().as_str().to_owned(),
                    case_identity,
                    source_file_sha256: declaration.source_file_sha256().to_hex(),
                    source_record_sha256: declaration.source_record_sha256().to_hex(),
                    license_identifier: declaration.license_identifier().as_str().to_owned(),
                    license_notice: declaration.license_notice().as_str().to_owned(),
                    attribution: declaration.attribution().as_str().to_owned(),
                    adaptation: declaration.adaptation().to_owned(),
                }),
            })
        }
        _ => invalid_combination(
            bundle,
            "fixture-v3 source kind must declare exactly its required structured provenance or tracking field",
        ),
    }
}

fn validate_disposition(
    bundle: &FixtureBundle,
    disposition: FixtureDispositionDeclaration,
    input: &ExactInput,
    execution: &ValidatedExecution,
    expectations: &EnabledExpectations,
    required_unknown_extensions: &[String],
) -> Result<FixtureDisposition, FixtureLoadError> {
    match (
        disposition.status,
        disposition.reason,
        disposition.capability,
        disposition.failure,
        disposition.classification,
        disposition.reference,
    ) {
        (FixtureDispositionStatusDeclaration::Active, None, None, None, None, None) => {
            Ok(FixtureDisposition::Active)
        }
        (
            FixtureDispositionStatusDeclaration::ExpectedUnsupported,
            Some(reason),
            Some(capability),
            None,
            None,
            Some(reference),
        ) => {
            require_non_empty(bundle, "expected-unsupported reason", &reason)?;
            let capability = map_capability(bundle, capability)?;
            require_non_active_capability(bundle, &capability, "expected unsupported")?;
            Ok(FixtureDisposition::ExpectedUnsupported {
                reason,
                capability,
                reference: validate_reference(bundle, reference)?,
            })
        }
        (
            FixtureDispositionStatusDeclaration::ExpectedFailure,
            Some(reason),
            None,
            Some(failure),
            None,
            Some(reference),
        ) => {
            require_non_empty(bundle, "expected-failure reason", &reason)?;
            let failure = map_expected_failure(failure);
            require_non_active_failure(bundle, &failure)?;
            Ok(FixtureDisposition::ExpectedFailure {
                reason,
                failure,
                reference: validate_reference(bundle, reference)?,
            })
        }
        (
            FixtureDispositionStatusDeclaration::Skipped,
            Some(reason),
            None,
            None,
            Some(classification),
            Some(reference),
        ) => {
            require_non_empty(bundle, "skipped reason", &reason)?;
            let classification = validate_skip_classification(bundle, classification)?;
            let SkipClassification::UnsupportedCapability(capability) = &classification;
            if !capability_is_relevant(
                capability,
                input,
                execution,
                expectations,
                required_unknown_extensions,
            ) {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidDisposition(format!(
                        "skipped unsupported capability '{}' is not relevant to the fixture's declared semantics",
                        capability_name(capability)
                    )),
                ));
            }
            Ok(FixtureDisposition::Skipped {
                reason,
                classification,
                reference: validate_reference(bundle, reference)?,
            })
        }
        _ => invalid_combination(
            bundle,
            "disposition fields do not match the declared status",
        ),
    }
}

fn validate_disposition_v2(
    bundle: &FixtureBundle,
    disposition: FixtureDispositionDeclarationV2,
    input: &ExactInput,
    execution: &ValidatedExecution,
    expectations: &EnabledExpectations,
    required_unknown_extensions: &[String],
) -> Result<FixtureDisposition, FixtureLoadError> {
    match (
        disposition.status,
        disposition.reason,
        disposition.capability,
        disposition.failure,
        disposition.classification,
        disposition.reference,
    ) {
        (FixtureDispositionStatusDeclaration::Active, None, None, None, None, None) => {
            Ok(FixtureDisposition::Active)
        }
        (
            FixtureDispositionStatusDeclaration::ExpectedUnsupported,
            Some(reason),
            Some(capability),
            None,
            None,
            Some(reference),
        ) => {
            require_non_empty(bundle, "expected-unsupported reason", &reason)?;
            let capability = map_capability(bundle, capability)?;
            require_non_active_capability(bundle, &capability, "expected unsupported")?;
            Ok(FixtureDisposition::ExpectedUnsupported {
                reason,
                capability,
                reference: validate_reference(bundle, reference)?,
            })
        }
        (
            FixtureDispositionStatusDeclaration::ExpectedFailure,
            Some(reason),
            None,
            Some(failure),
            None,
            Some(reference),
        ) => {
            require_non_empty(bundle, "expected-failure reason", &reason)?;
            Ok(FixtureDisposition::ExpectedFailureV2 {
                reason,
                failure: map_expected_failure_v2(bundle, failure)?,
                reference: validate_reference(bundle, reference)?,
            })
        }
        (
            FixtureDispositionStatusDeclaration::Skipped,
            Some(reason),
            None,
            None,
            Some(classification),
            Some(reference),
        ) => {
            require_non_empty(bundle, "skipped reason", &reason)?;
            let classification = validate_skip_classification(bundle, classification)?;
            let SkipClassification::UnsupportedCapability(capability) = &classification;
            if !capability_is_relevant(
                capability,
                input,
                execution,
                expectations,
                required_unknown_extensions,
            ) {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidDisposition(format!(
                        "skipped unsupported capability '{}' is not relevant to the fixture's declared semantics",
                        capability_name(capability)
                    )),
                ));
            }
            Ok(FixtureDisposition::Skipped {
                reason,
                classification,
                reference: validate_reference(bundle, reference)?,
            })
        }
        _ => invalid_combination(
            bundle,
            "fixture-v2 disposition fields do not match the declared status",
        ),
    }
}

fn map_expected_failure_v2(
    bundle: &FixtureBundle,
    declaration: ExpectedFailureDeclarationV2,
) -> Result<ExpectedFailureClassificationV2, FixtureLoadError> {
    let invalid = || {
        bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(
                "fixture-v2 failure fields do not match the selected failure kind".to_string(),
            ),
        )
    };
    match (
        declaration.kind,
        declaration.surface,
        declaration.identity,
        declaration.code,
        declaration.site,
    ) {
        (ExpectedFailureKindDeclarationV2::SnapshotRead, Some(surface), None, None, None) => {
            Ok(ExpectedFailureClassificationV2::Execution(
                ExecutionFailureClass::SnapshotRead(map_surface(surface)),
            ))
        }
        (ExpectedFailureKindDeclarationV2::SnapshotFormat, Some(surface), None, None, None) => {
            Ok(ExpectedFailureClassificationV2::Execution(
                ExecutionFailureClass::SnapshotFormat(map_surface(surface)),
            ))
        }
        (ExpectedFailureKindDeclarationV2::ParserObservation, None, Some(identity), code, site) => {
            Ok(ExpectedFailureClassificationV2::Execution(
                ExecutionFailureClass::ParserObservation(
                    parse_parser_observation_failure(&identity, code.as_deref(), site.as_deref())
                        .map_err(|error| {
                        bundle_error(
                            bundle,
                            FixtureLoadErrorKind::InvalidDisposition(error.to_string()),
                        )
                    })?,
                ),
            ))
        }
        (
            ExpectedFailureKindDeclarationV2::ValidatedRunnerInvariant,
            None,
            None,
            Some(code),
            None,
        ) => Ok(ExpectedFailureClassificationV2::Execution(
            ExecutionFailureClass::ValidatedFixtureInvariant(
                parse_runner_invariant(&code).map_err(|error| {
                    bundle_error(
                        bundle,
                        FixtureLoadErrorKind::InvalidDisposition(error.to_string()),
                    )
                })?,
            ),
        )),
        (
            ExpectedFailureKindDeclarationV2::ExpectationMismatch,
            Some(surface),
            None,
            None,
            None,
        ) => Ok(ExpectedFailureClassificationV2::ExpectationMismatch(
            map_surface(surface),
        )),
        (ExpectedFailureKindDeclarationV2::FinalInvariant, None, None, Some(code), None) => Ok(
            ExpectedFailureClassificationV2::FinalInvariant(map_final_invariant(bundle, &code)?),
        ),
        _ => Err(invalid()),
    }
}

fn map_surface(surface: ExpectationSurfaceDeclaration) -> ExpectationSurface {
    match surface {
        ExpectationSurfaceDeclaration::Tokens => ExpectationSurface::Tokens,
        ExpectationSurfaceDeclaration::ParseErrors => ExpectationSurface::ParseErrors,
        ExpectationSurfaceDeclaration::ImplementationDiagnostics => {
            ExpectationSurface::ImplementationDiagnostics
        }
        ExpectationSurfaceDeclaration::DocumentMode => ExpectationSurface::DocumentMode,
        ExpectationSurfaceDeclaration::Tree => ExpectationSurface::Tree,
        ExpectationSurfaceDeclaration::Patches => ExpectationSurface::Patches,
        ExpectationSurfaceDeclaration::Transitions => ExpectationSurface::Transitions,
        ExpectationSurfaceDeclaration::UnsupportedFeatures => {
            ExpectationSurface::UnsupportedFeatures
        }
        ExpectationSurfaceDeclaration::FinalInvariants => ExpectationSurface::FinalInvariants,
    }
}

fn validate_reference(
    bundle: &FixtureBundle,
    reference: DispositionReferenceDeclaration,
) -> Result<DispositionReference, FixtureLoadError> {
    match reference.kind {
        DispositionReferenceKindDeclaration::TrackingIssue => {
            let value = reference.value;
            require_non_empty(bundle, "tracking issue", &value)?;
            Ok(DispositionReference::TrackingIssue(value))
        }
        DispositionReferenceKindDeclaration::Provenance => {
            let value = reference.value;
            require_non_empty(bundle, "provenance reference", &value)?;
            Ok(DispositionReference::Provenance(value))
        }
    }
}

fn validate_source_disposition_policy(
    bundle: &FixtureBundle,
    policy: FixtureRepositoryPolicy,
    source: &FixtureSource,
    disposition: &FixtureDisposition,
) -> Result<(), FixtureLoadError> {
    if matches!(policy, FixtureRepositoryPolicy::NativeConformance)
        && (!matches!(source, FixtureSource::Native)
            || !matches!(disposition, FixtureDisposition::Active))
    {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(
                "native conformance fixtures must declare source=native and status=active"
                    .to_string(),
            ),
        ));
    }
    if matches!(source, FixtureSource::Native) && !matches!(disposition, FixtureDisposition::Active)
    {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(
                "native Borrowser fixtures cannot be expected-unsupported, expected-failure, or skipped"
                    .to_string(),
            ),
        ));
    }
    Ok(())
}

fn validate_input(
    bundle: &FixtureBundle,
    declaration: InputDeclaration,
    bytes: Vec<u8>,
) -> Result<ExactInput, FixtureLoadError> {
    let extension = Path::new(&declaration.path)
        .extension()
        .and_then(|value| value.to_str());
    match declaration.kind {
        InputKindDeclaration::Utf8Text => {
            if extension != Some("html") {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidInputExtension,
                ));
            }
            let text = String::from_utf8(bytes.clone())
                .map_err(|_| bundle_error(bundle, FixtureLoadErrorKind::InvalidUtf8TextInput))?;
            if bytes.contains(&b'\r') {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::CarriageReturnInTextInput,
                ));
            }
            Ok(ExactInput::Utf8Text {
                path: declaration.path,
                bytes,
                text,
                sha256: declaration.sha256,
            })
        }
        InputKindDeclaration::RawBytes => {
            if extension != Some("bin") {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidInputExtension,
                ));
            }
            Ok(ExactInput::RawBytes {
                path: declaration.path,
                bytes,
                sha256: declaration.sha256,
            })
        }
    }
}

fn validate_execution_v1(
    bundle: &FixtureBundle,
    input: &ExactInput,
    declaration: ExecutionDeclaration,
) -> Result<ValidatedExecution, FixtureLoadError> {
    validate_execution(
        bundle,
        input,
        declaration,
        ExecutionValidationPolicy::V1Compatibility,
    )
}

fn validate_execution_v2(
    bundle: &FixtureBundle,
    input: &ExactInput,
    declaration: ExecutionDeclaration,
) -> Result<ValidatedExecution, FixtureLoadError> {
    validate_execution(
        bundle,
        input,
        declaration,
        ExecutionValidationPolicy::V2Parity,
    )
}

#[derive(Clone, Copy)]
enum ExecutionValidationPolicy {
    V1Compatibility,
    V2Parity,
}

const MAX_DECLARED_DELIVERIES_V2: usize = 32;
const MAX_BOUNDARIES_PER_DELIVERY_V2: usize = 4096;
const MAX_UNIQUE_PLANNED_STRATEGIES_V2: usize = 24;

fn plan_v2_strategies(
    bundle: &FixtureBundle,
    input: &ExactInput,
    execution: &ValidatedExecution,
) -> Result<Vec<ScheduledDeliveryStrategy>, FixtureLoadError> {
    let scalar_extent = input.text().map(|text| text.chars().count());
    let byte_extent = input.bytes().len();
    let baseline = match input {
        ExactInput::Utf8Text { .. } => ResolvedDeliveryStrategy {
            transport: DeliveryTransport::UnicodeScalars,
            coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
            input_extent: scalar_extent.unwrap_or(0),
            boundaries: CanonicalBoundarySequence::Whole,
        },
        ExactInput::RawBytes { .. } => ResolvedDeliveryStrategy {
            transport: DeliveryTransport::Bytes,
            coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
            input_extent: byte_extent,
            boundaries: CanonicalBoundarySequence::Whole,
        },
    };
    let mut planned = Vec::new();
    push_or_alias_strategy(
        bundle,
        &mut planned,
        baseline,
        DeliveryStrategyOrigin::Baseline,
    )?;

    for declaration in execution.deliveries() {
        let strategy = resolved_declared_strategy(declaration, scalar_extent, byte_extent);
        push_or_alias_strategy(
            bundle,
            &mut planned,
            strategy,
            DeliveryStrategyOrigin::Declared(declaration.name().clone()),
        )?;
    }

    let fixed_seven = NonZeroUsize::new(7).ok_or_else(|| {
        bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidCombination(
                "representative fixed extent is zero".to_string(),
            ),
        )
    })?;
    let mut representatives = Vec::new();
    match input {
        ExactInput::Utf8Text { .. } => {
            representatives.push((
                "whole-bytes",
                ResolvedDeliveryStrategy {
                    transport: DeliveryTransport::Bytes,
                    coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                    input_extent: byte_extent,
                    boundaries: CanonicalBoundarySequence::Whole,
                },
            ));
            representatives.extend([
                (
                    "scalar-fixed-one",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::UnicodeScalars,
                        coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
                        input_extent: scalar_extent.unwrap_or(0),
                        boundaries: CanonicalBoundarySequence::Fixed {
                            units_per_chunk: NonZeroUsize::MIN,
                        },
                    },
                ),
                (
                    "scalar-fixed-seven",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::UnicodeScalars,
                        coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
                        input_extent: scalar_extent.unwrap_or(0),
                        boundaries: CanonicalBoundarySequence::Fixed {
                            units_per_chunk: fixed_seven,
                        },
                    },
                ),
                (
                    "scalar-edge-triplet",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::UnicodeScalars,
                        coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
                        input_extent: scalar_extent.unwrap_or(0),
                        boundaries: CanonicalBoundarySequence::Explicit(edge_triplet(
                            scalar_extent.unwrap_or(0),
                        )),
                    },
                ),
                (
                    "byte-fixed-one",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::Bytes,
                        coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                        input_extent: byte_extent,
                        boundaries: CanonicalBoundarySequence::Fixed {
                            units_per_chunk: NonZeroUsize::MIN,
                        },
                    },
                ),
                (
                    "byte-fixed-seven",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::Bytes,
                        coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                        input_extent: byte_extent,
                        boundaries: CanonicalBoundarySequence::Fixed {
                            units_per_chunk: fixed_seven,
                        },
                    },
                ),
                (
                    "byte-edge-triplet",
                    ResolvedDeliveryStrategy {
                        transport: DeliveryTransport::Bytes,
                        coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                        input_extent: byte_extent,
                        boundaries: CanonicalBoundarySequence::Explicit(edge_triplet(byte_extent)),
                    },
                ),
            ]);
        }
        ExactInput::RawBytes { .. } => representatives.extend([
            (
                "byte-fixed-one",
                ResolvedDeliveryStrategy {
                    transport: DeliveryTransport::Bytes,
                    coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                    input_extent: byte_extent,
                    boundaries: CanonicalBoundarySequence::Fixed {
                        units_per_chunk: NonZeroUsize::MIN,
                    },
                },
            ),
            (
                "byte-fixed-seven",
                ResolvedDeliveryStrategy {
                    transport: DeliveryTransport::Bytes,
                    coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                    input_extent: byte_extent,
                    boundaries: CanonicalBoundarySequence::Fixed {
                        units_per_chunk: fixed_seven,
                    },
                },
            ),
            (
                "byte-edge-triplet",
                ResolvedDeliveryStrategy {
                    transport: DeliveryTransport::Bytes,
                    coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
                    input_extent: byte_extent,
                    boundaries: CanonicalBoundarySequence::Explicit(edge_triplet(byte_extent)),
                },
            ),
        ]),
    }
    for (name, strategy) in representatives {
        push_or_alias_strategy(
            bundle,
            &mut planned,
            strategy,
            DeliveryStrategyOrigin::Representative(name),
        )?;
    }
    Ok(planned)
}

fn resolved_declared_strategy(
    delivery: &ValidatedDelivery,
    scalar_extent: Option<usize>,
    byte_extent: usize,
) -> ResolvedDeliveryStrategy {
    let boundaries = match delivery.boundaries() {
        Some(boundaries) => CanonicalBoundarySequence::Explicit(boundaries.into()),
        None => CanonicalBoundarySequence::Whole,
    };
    match delivery.transport() {
        DeliveryTransport::UnicodeScalars => ResolvedDeliveryStrategy {
            transport: DeliveryTransport::UnicodeScalars,
            coordinate_space: delivery.coordinate_space(),
            input_extent: scalar_extent.unwrap_or(0),
            boundaries,
        },
        DeliveryTransport::Bytes => ResolvedDeliveryStrategy {
            transport: DeliveryTransport::Bytes,
            coordinate_space: delivery.coordinate_space(),
            input_extent: byte_extent,
            boundaries,
        },
    }
}

fn push_or_alias_strategy(
    bundle: &FixtureBundle,
    planned: &mut Vec<ScheduledDeliveryStrategy>,
    strategy: ResolvedDeliveryStrategy,
    origin: DeliveryStrategyOrigin,
) -> Result<(), FixtureLoadError> {
    if let Some(existing) = planned
        .iter_mut()
        .find(|existing| existing.strategy.semantically_equals(&strategy))
    {
        existing.origins.push(origin);
        return Ok(());
    }
    if planned.len() >= MAX_UNIQUE_PLANNED_STRATEGIES_V2 {
        return invalid_delivery(
            bundle,
            DeliveryValidationError::TooManyUniqueStrategies {
                planned: planned.len() + 1,
                maximum: MAX_UNIQUE_PLANNED_STRATEGIES_V2,
            },
        );
    }
    let ordinal = StrategyOrdinal::checked_from_index(planned.len()).ok_or_else(|| {
        bundle_error(
            bundle,
            FixtureLoadErrorKind::InternalPlanningInvariant(
                FixturePlanningInvariant::StrategyOrdinalOverflow,
            ),
        )
    })?;
    planned.push(ScheduledDeliveryStrategy {
        ordinal,
        strategy,
        origins: vec![origin],
    });
    Ok(())
}

fn edge_triplet(extent: usize) -> Box<[usize]> {
    if extent <= 1 {
        return Box::new([]);
    }
    let mut boundaries = Vec::with_capacity(3);
    for candidate in [1, extent / 2, extent - 1] {
        if candidate > 0 && candidate < extent && boundaries.last() != Some(&candidate) {
            boundaries.push(candidate);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries.into_boxed_slice()
}

fn validate_execution(
    bundle: &FixtureBundle,
    input: &ExactInput,
    declaration: ExecutionDeclaration,
    policy: ExecutionValidationPolicy,
) -> Result<ValidatedExecution, FixtureLoadError> {
    let target = match declaration.target.kind {
        ParserTargetKindDeclaration::StandaloneTokenizer => {
            if declaration.target.scripting.is_some() || declaration.target.fragment.is_some() {
                return invalid_combination(
                    bundle,
                    "standalone-tokenizer cannot declare scripting or fragment metadata",
                );
            }
            ValidatedParserTarget::StandaloneTokenizer
        }
        ParserTargetKindDeclaration::Document => {
            if declaration.target.fragment.is_some() {
                return invalid_combination(
                    bundle,
                    "document target cannot declare fragment metadata",
                );
            }
            ValidatedParserTarget::Document {
                scripting: declaration
                    .target
                    .scripting
                    .map(map_scripting)
                    .unwrap_or(ScriptingMode::Disabled),
            }
        }
        ParserTargetKindDeclaration::Fragment => {
            let fragment = declaration.target.fragment.ok_or_else(|| {
                bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidCombination(
                        "fragment target requires fragment metadata".to_string(),
                    ),
                )
            })?;
            require_non_empty(bundle, "fragment local name", &fragment.local_name)?;
            let namespace = match fragment.namespace.as_str() {
                "html" => ElementNamespace::Html,
                "svg" => ElementNamespace::Svg,
                "mathml" => ElementNamespace::MathMl,
                _ => {
                    return invalid_combination(
                        bundle,
                        "fragment namespace must be html, svg, or mathml",
                    );
                }
            };
            ValidatedParserTarget::Fragment {
                context: FragmentContext::validated(namespace, fragment.local_name),
                scripting: declaration
                    .target
                    .scripting
                    .map(map_scripting)
                    .unwrap_or(ScriptingMode::Disabled),
            }
        }
    };

    if declaration.deliveries.is_empty() {
        return invalid_combination(bundle, "execution must declare at least one delivery");
    }
    if matches!(policy, ExecutionValidationPolicy::V2Parity)
        && declaration.deliveries.len() > MAX_DECLARED_DELIVERIES_V2
    {
        return invalid_delivery(
            bundle,
            DeliveryValidationError::TooManyDeclaredDeliveries {
                declared: declaration.deliveries.len(),
                maximum: MAX_DECLARED_DELIVERIES_V2,
            },
        );
    }
    let reference_delivery_name = declaration.reference_delivery;
    let mut names = BTreeSet::new();
    let mut deliveries = Vec::with_capacity(declaration.deliveries.len());
    let byte_extent = input.bytes().len();
    let scalar_extent = input.text().map(|text| text.chars().count());
    for (delivery_index, delivery) in declaration.deliveries.into_iter().enumerate() {
        if matches!(policy, ExecutionValidationPolicy::V2Parity)
            && delivery
                .boundaries
                .as_ref()
                .is_some_and(|boundaries| boundaries.len() > MAX_BOUNDARIES_PER_DELIVERY_V2)
        {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::TooManyBoundaries {
                    delivery_index,
                    declared_name: delivery.name.clone(),
                    declared: delivery.boundaries.as_ref().map_or(0, Vec::len),
                    maximum: MAX_BOUNDARIES_PER_DELIVERY_V2,
                },
            );
        }
        if !is_kebab_identifier(&delivery.name) {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::InvalidDeliveryName {
                    delivery_index,
                    declared_name: delivery.name,
                },
            );
        }
        if !names.insert(delivery.name.clone()) {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::DuplicateDeliveryName {
                    delivery_index,
                    declared_name: delivery.name,
                },
            );
        }
        let name = DeliveryName::validated(delivery.name);
        let validated = match (input, delivery.unit, delivery.strategy, delivery.boundaries) {
            (_, _, DeliveryStrategyDeclaration::Whole, Some(_))
                if matches!(policy, ExecutionValidationPolicy::V2Parity) =>
            {
                return invalid_delivery(
                    bundle,
                    DeliveryValidationError::BoundariesUnexpected { delivery: name },
                );
            }
            (_, _, DeliveryStrategyDeclaration::Boundaries, None)
                if matches!(policy, ExecutionValidationPolicy::V2Parity) =>
            {
                return invalid_delivery(
                    bundle,
                    DeliveryValidationError::BoundariesMissing { delivery: name },
                );
            }
            (
                ExactInput::RawBytes { .. },
                DeliveryUnitDeclaration::Bytes,
                DeliveryStrategyDeclaration::Whole,
                None,
            ) => ValidatedDelivery::WholeBytes { name },
            (
                ExactInput::RawBytes { .. },
                DeliveryUnitDeclaration::Bytes,
                DeliveryStrategyDeclaration::Boundaries,
                Some(boundaries),
            ) => ValidatedDelivery::ByteBoundaries {
                name: name.clone(),
                boundaries: validate_boundaries(bundle, &name, boundaries, byte_extent, policy)?,
            },
            (
                ExactInput::Utf8Text { .. },
                DeliveryUnitDeclaration::UnicodeScalars,
                DeliveryStrategyDeclaration::Whole,
                None,
            ) => ValidatedDelivery::WholeUnicodeScalars { name },
            (
                ExactInput::Utf8Text { .. },
                DeliveryUnitDeclaration::UnicodeScalars,
                DeliveryStrategyDeclaration::Boundaries,
                Some(boundaries),
            ) => ValidatedDelivery::UnicodeScalarBoundaries {
                name: name.clone(),
                boundaries: validate_boundaries(
                    bundle,
                    &name,
                    boundaries,
                    scalar_extent.ok_or_else(|| {
                        bundle_error(
                            bundle,
                            FixtureLoadErrorKind::InvalidDelivery(
                                DeliveryValidationError::UnitNotSupportedForInputDomain {
                                    delivery: name.clone(),
                                },
                            ),
                        )
                    })?,
                    policy,
                )?,
            },
            (
                ExactInput::Utf8Text { .. },
                DeliveryUnitDeclaration::Bytes,
                DeliveryStrategyDeclaration::Whole,
                None,
            ) if matches!(policy, ExecutionValidationPolicy::V2Parity) => {
                ValidatedDelivery::WholeBytes { name }
            }
            (
                ExactInput::Utf8Text { .. },
                DeliveryUnitDeclaration::Bytes,
                DeliveryStrategyDeclaration::Boundaries,
                Some(boundaries),
            ) if matches!(policy, ExecutionValidationPolicy::V2Parity) => {
                ValidatedDelivery::ByteBoundaries {
                    name: name.clone(),
                    boundaries: validate_boundaries(
                        bundle,
                        &name,
                        boundaries,
                        byte_extent,
                        policy,
                    )?,
                }
            }
            _ => {
                return invalid_delivery(
                    bundle,
                    DeliveryValidationError::UnitNotSupportedForInputDomain { delivery: name },
                );
            }
        };
        deliveries.push(validated);
    }
    if matches!(policy, ExecutionValidationPolicy::V2Parity) {
        let is_domain_baseline = |delivery: &ValidatedDelivery| match input {
            ExactInput::Utf8Text { .. } => {
                matches!(delivery, ValidatedDelivery::WholeUnicodeScalars { .. })
            }
            ExactInput::RawBytes { .. } => matches!(delivery, ValidatedDelivery::WholeBytes { .. }),
        };
        if !deliveries.iter().any(is_domain_baseline) {
            return invalid_delivery(bundle, DeliveryValidationError::MissingDomainBaseline);
        }
    }
    if !is_kebab_identifier(&reference_delivery_name) {
        return invalid_delivery(
            bundle,
            DeliveryValidationError::InvalidReferenceDeliveryName {
                declared_name: reference_delivery_name,
            },
        );
    }
    let reference_delivery = DeliveryName::validated(reference_delivery_name);
    if !names.contains(reference_delivery.as_str()) {
        return invalid_delivery(
            bundle,
            DeliveryValidationError::ReferenceDeliveryMissing {
                delivery: reference_delivery.clone(),
            },
        );
    }
    if matches!(policy, ExecutionValidationPolicy::V2Parity) {
        let is_domain_baseline = |delivery: &ValidatedDelivery| match input {
            ExactInput::Utf8Text { .. } => {
                matches!(delivery, ValidatedDelivery::WholeUnicodeScalars { .. })
            }
            ExactInput::RawBytes { .. } => matches!(delivery, ValidatedDelivery::WholeBytes { .. }),
        };
        let reference_is_domain_baseline = deliveries
            .iter()
            .find(|delivery| delivery.name() == &reference_delivery)
            .is_some_and(is_domain_baseline);
        if !reference_is_domain_baseline {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::ReferenceIsNotDomainBaseline {
                    delivery: reference_delivery.clone(),
                },
            );
        }
    }
    Ok(ValidatedExecution::validated(
        target,
        reference_delivery,
        deliveries,
    ))
}

#[derive(Clone, Copy)]
enum SidecarValidationPolicy {
    LegacyV1ContentReadable,
    MetadataOnlyV2,
}

impl SidecarValidationPolicy {
    fn validate_declared_sidecar(
        self,
        bundle: &FixtureBundle,
        path: &str,
        file_access: &mut impl FixtureFileAccess,
    ) -> Result<(), FixtureLoadError> {
        match self {
            Self::LegacyV1ContentReadable => {
                let _ = file_access.read_regular_file(bundle, path)?;
            }
            Self::MetadataOnlyV2 => {
                file_access.validate_regular_file_metadata(bundle, path)?;
            }
        }
        Ok(())
    }
}

fn validate_expectations(
    bundle: &FixtureBundle,
    execution: &ValidatedExecution,
    declaration: FixtureExpectationDeclarations,
    sidecar_policy: SidecarValidationPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<EnabledExpectations, FixtureLoadError> {
    let delivery_names = execution
        .deliveries()
        .iter()
        .map(|delivery| delivery.name().clone())
        .collect::<BTreeSet<_>>();
    let transitions = declaration
        .transitions
        .into_iter()
        .map(|transition| {
            validate_relative_path(&transition.path).map_err(|kind| bundle_error(bundle, kind))?;
            let delivery = DeliveryName::validated(transition.delivery);
            if !delivery_names.contains(&delivery) {
                return invalid_combination(
                    bundle,
                    "transition expectation references an undeclared delivery",
                );
            }
            sidecar_policy.validate_declared_sidecar(bundle, &transition.path, file_access)?;
            Ok(TransitionSnapshotExpectation::validated(
                delivery,
                SnapshotPath::validated(transition.path),
            ))
        })
        .collect::<Result<Vec<_>, FixtureLoadError>>()?;

    Ok(EnabledExpectations::validated(
        snapshot_surface(bundle, declaration.tokens, sidecar_policy, file_access)?,
        parse_error_surface(
            bundle,
            declaration.parse_errors,
            sidecar_policy,
            file_access,
        )?,
        snapshot_surface(
            bundle,
            declaration.implementation_diagnostics,
            sidecar_policy,
            file_access,
        )?,
        snapshot_surface(
            bundle,
            declaration.document_mode,
            sidecar_policy,
            file_access,
        )?,
        snapshot_surface(bundle, declaration.tree, sidecar_policy, file_access)?,
        snapshot_surface(bundle, declaration.patches, sidecar_policy, file_access)?,
        if transitions.is_empty() {
            ExpectedSurface::NotDeclared
        } else {
            ExpectedSurface::Compare(transitions)
        },
        snapshot_surface(
            bundle,
            declaration.unsupported_features,
            sidecar_policy,
            file_access,
        )?,
        snapshot_surface(
            bundle,
            declaration.final_invariants,
            sidecar_policy,
            file_access,
        )?,
    ))
}

fn parse_error_surface(
    bundle: &FixtureBundle,
    path: Option<String>,
    sidecar_policy: SidecarValidationPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ExpectedSurface<ParseErrorExpectation>, FixtureLoadError> {
    let Some(path) = path else {
        return Ok(ExpectedSurface::NotDeclared);
    };
    validate_relative_path(&path).map_err(|kind| bundle_error(bundle, kind))?;
    sidecar_policy.validate_declared_sidecar(bundle, &path, file_access)?;
    Ok(ExpectedSurface::Compare(ParseErrorExpectation::Exact(
        SnapshotPath::validated(path),
    )))
}

fn validate_expectations_v3(
    bundle: &FixtureBundle,
    execution: &ValidatedExecution,
    declaration: FixtureExpectationDeclarationsV3,
    sidecar_policy: SidecarValidationPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<EnabledExpectations, FixtureLoadError> {
    let FixtureExpectationDeclarationsV3 {
        tokens,
        parse_errors,
        implementation_diagnostics,
        document_mode,
        tree,
        patches,
        transitions,
        unsupported_features,
        final_invariants,
    } = declaration;
    let base = validate_expectations(
        bundle,
        execution,
        FixtureExpectationDeclarations {
            tokens,
            parse_errors: None,
            implementation_diagnostics,
            document_mode,
            tree,
            patches,
            transitions,
            unsupported_features,
            final_invariants,
        },
        sidecar_policy,
        file_access,
    )?;
    let parse_errors = match parse_errors {
        None => ExpectedSurface::NotDeclared,
        Some(ParseErrorExpectationDeclarationV3 { kind, path, count }) => match kind {
            ParseErrorExpectationKindDeclarationV3::Exact => {
                let Some(path) = path else {
                    return invalid_combination(
                        bundle,
                        "exact parse-error expectation requires path and forbids count",
                    );
                };
                if count.is_some() {
                    return invalid_combination(
                        bundle,
                        "exact parse-error expectation requires path and forbids count",
                    );
                }
                parse_error_surface(bundle, Some(path), sidecar_policy, file_access)?
            }
            ParseErrorExpectationKindDeclarationV3::Count => {
                let Some(count) = count else {
                    return invalid_combination(
                        bundle,
                        "count parse-error expectation requires count and forbids path",
                    );
                };
                if path.is_some() {
                    return invalid_combination(
                        bundle,
                        "count parse-error expectation requires count and forbids path",
                    );
                }
                ExpectedSurface::Compare(ParseErrorExpectation::Count(count))
            }
        },
    };
    Ok(EnabledExpectations::validated(
        base.tokens().clone(),
        parse_errors,
        base.implementation_diagnostics().clone(),
        base.document_mode().clone(),
        base.tree().clone(),
        base.patches().clone(),
        base.transitions().clone(),
        base.unsupported_features().clone(),
        base.final_invariants().clone(),
    ))
}

fn snapshot_surface(
    bundle: &FixtureBundle,
    path: Option<String>,
    sidecar_policy: SidecarValidationPolicy,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ExpectedSurface<SnapshotPath>, FixtureLoadError> {
    let Some(path) = path else {
        return Ok(ExpectedSurface::NotDeclared);
    };
    validate_relative_path(&path).map_err(|kind| bundle_error(bundle, kind))?;
    sidecar_policy.validate_declared_sidecar(bundle, &path, file_access)?;
    Ok(ExpectedSurface::Compare(SnapshotPath::validated(path)))
}

fn validate_orphan_sidecars(
    bundle: &FixtureBundle,
    input_path: &str,
    source: &FixtureSource,
    expectations: &EnabledExpectations,
) -> Result<(), FixtureLoadError> {
    let mut declared = BTreeSet::from(["fixture.toml".to_string(), input_path.to_string()]);
    for surface in [
        expectations.tokens(),
        expectations.implementation_diagnostics(),
        expectations.document_mode(),
        expectations.tree(),
        expectations.patches(),
        expectations.unsupported_features(),
        expectations.final_invariants(),
    ] {
        if let ExpectedSurface::Compare(path) = surface {
            declared.insert(path.as_str().to_string());
        }
    }
    if let ExpectedSurface::Compare(ParseErrorExpectation::Exact(path)) =
        expectations.parse_errors()
    {
        declared.insert(path.as_str().to_string());
    }
    if let FixtureSource::External {
        provenance_record: record,
        ..
    } = source
    {
        declared.insert(record.record_path.clone());
    }
    if let ExpectedSurface::Compare(transitions) = expectations.transitions() {
        declared.extend(
            transitions
                .iter()
                .map(|transition| transition.path().as_str().to_string()),
        );
    }
    let mut folded_paths = BTreeSet::new();
    for path in &declared {
        if !folded_paths.insert(path.to_ascii_lowercase()) {
            return invalid_combination(
                bundle,
                "declared fixture paths must not collide case-insensitively",
            );
        }
    }
    scan_orphan_sidecars(bundle, bundle.absolute_path(), &declared)
}

fn has_any_expectation(expectations: &EnabledExpectations) -> bool {
    [
        ExpectationSurface::Tokens,
        ExpectationSurface::ParseErrors,
        ExpectationSurface::ImplementationDiagnostics,
        ExpectationSurface::DocumentMode,
        ExpectationSurface::Tree,
        ExpectationSurface::Patches,
        ExpectationSurface::Transitions,
        ExpectationSurface::UnsupportedFeatures,
        ExpectationSurface::FinalInvariants,
    ]
    .into_iter()
    .any(|surface| expectations.is_declared(surface))
}

fn scan_orphan_sidecars(
    bundle: &FixtureBundle,
    directory: &Path,
    declared: &BTreeSet<String>,
) -> Result<(), FixtureLoadError> {
    let entries = fs::read_dir(directory)
        .map_err(|err| bundle_error(bundle, FixtureLoadErrorKind::Io(err.to_string())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| bundle_error(bundle, FixtureLoadErrorKind::Io(err.to_string())))?;
    let mut entries = entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| bundle_error(bundle, FixtureLoadErrorKind::NonUtf8Path))?;
            Ok((name, entry))
        })
        .collect::<Result<Vec<_>, FixtureLoadError>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, entry) in entries {
        let path = entry.path();
        let relative = path.strip_prefix(bundle.absolute_path()).map_err(|_| {
            bundle_error(
                bundle,
                FixtureLoadErrorKind::UnsafeRelativePath(path.display().to_string()),
            )
        })?;
        let relative = normalize_relative_path(relative)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| bundle_error(bundle, FixtureLoadErrorKind::Io(err.to_string())))?;
        if metadata.file_type().is_symlink() {
            return Err(FixtureLoadError {
                path: format!("{}/{}", bundle.repository_relative_path(), relative),
                kind: FixtureLoadErrorKind::SymlinkNotAllowed,
            });
        }
        if metadata.is_dir() {
            scan_orphan_sidecars(bundle, &path, declared)?;
        } else if is_recognized_sidecar(&relative) && !declared.contains(&relative) {
            return Err(bundle_error(
                bundle,
                FixtureLoadErrorKind::OrphanSidecar(relative),
            ));
        }
    }
    Ok(())
}

fn is_recognized_sidecar(relative: &str) -> bool {
    matches!(
        relative,
        "tokens.txt"
            | "parse-errors.txt"
            | "implementation-diagnostics.txt"
            | "document-mode.txt"
            | "tree.txt"
            | "patches.txt"
            | "unsupported-features.txt"
            | "final-invariants.txt"
    ) || relative
        .rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("transitions.") && name.ends_with(".txt"))
}

fn validate_extensions(
    bundle: &FixtureBundle,
    extensions: BTreeMap<String, ExtensionDeclaration>,
) -> Result<(BTreeMap<String, ExtensionDeclaration>, Vec<String>), FixtureLoadError> {
    let mut optional = BTreeMap::new();
    let mut required = Vec::new();
    for (id, declaration) in extensions {
        if !is_versioned_extension_id(&id) {
            return Err(bundle_error(
                bundle,
                FixtureLoadErrorKind::InvalidExtensionId(id),
            ));
        }
        if declaration.required {
            required.push(id);
        } else {
            optional.insert(id, declaration);
        }
    }
    required.sort();
    Ok((optional, required))
}

fn validate_sha256(
    bundle: &FixtureBundle,
    expected: &str,
    bytes: &[u8],
) -> Result<(), FixtureLoadError> {
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidSha256(expected.to_string()),
        ));
    }
    let actual_digest = digest(&SHA256, bytes);
    let mut actual = String::with_capacity(64);
    for byte in actual_digest.as_ref() {
        let _ = write!(&mut actual, "{byte:02x}");
    }
    if actual != expected {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::Sha256Mismatch {
                expected: expected.to_string(),
                actual,
            },
        ));
    }
    Ok(())
}

fn validate_boundaries(
    bundle: &FixtureBundle,
    delivery: &DeliveryName,
    boundaries: Vec<usize>,
    extent: usize,
    policy: ExecutionValidationPolicy,
) -> Result<Vec<usize>, FixtureLoadError> {
    if boundaries.is_empty() && matches!(policy, ExecutionValidationPolicy::V1Compatibility) {
        return invalid_combination(bundle, "delivery boundaries must not be empty");
    }
    for (index, boundary) in boundaries.iter().copied().enumerate() {
        if boundary == 0 {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::BoundaryAtStart {
                    delivery: delivery.clone(),
                    boundary_index: index,
                },
            );
        }
        if boundary == extent {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::BoundaryAtEnd {
                    delivery: delivery.clone(),
                    boundary_index: index,
                },
            );
        }
        if boundary > extent {
            return invalid_delivery(
                bundle,
                DeliveryValidationError::BoundaryOutOfRange {
                    delivery: delivery.clone(),
                    boundary_index: index,
                },
            );
        }
        if index > 0 {
            let previous = boundaries[index - 1];
            if boundary == previous {
                return invalid_delivery(
                    bundle,
                    DeliveryValidationError::DuplicateBoundary {
                        delivery: delivery.clone(),
                        boundary_index: index,
                    },
                );
            }
            if boundary < previous {
                return invalid_delivery(
                    bundle,
                    DeliveryValidationError::UnsortedBoundary {
                        delivery: delivery.clone(),
                        boundary_index: index,
                    },
                );
            }
        }
    }
    Ok(boundaries)
}

fn map_scripting(value: ScriptingDeclaration) -> ScriptingMode {
    match value {
        ScriptingDeclaration::Disabled => ScriptingMode::Disabled,
        ScriptingDeclaration::Enabled => ScriptingMode::Enabled,
    }
}

fn map_capability(
    bundle: &FixtureBundle,
    value: FixtureCapabilityDeclaration,
) -> Result<FixtureCapability, FixtureLoadError> {
    let capability = match (value.kind, value.id) {
        (FixtureCapabilityKindDeclaration::RawByteInput, None) => FixtureCapability::RawByteInput,
        (FixtureCapabilityKindDeclaration::ByteDelivery, None) => FixtureCapability::ByteDelivery,
        (FixtureCapabilityKindDeclaration::UnicodeScalarChunking, None) => {
            FixtureCapability::UnicodeScalarChunking
        }
        (FixtureCapabilityKindDeclaration::DocumentExecution, None) => {
            FixtureCapability::DocumentExecution
        }
        (FixtureCapabilityKindDeclaration::FragmentParsing, None) => {
            FixtureCapability::FragmentParsing
        }
        (FixtureCapabilityKindDeclaration::ScriptingEnabled, None) => {
            FixtureCapability::ScriptingEnabled
        }
        (FixtureCapabilityKindDeclaration::UnknownRequiredExtension, Some(id)) => {
            if !is_versioned_extension_id(&id) {
                return Err(bundle_error(
                    bundle,
                    FixtureLoadErrorKind::InvalidExtensionId(id),
                ));
            }
            FixtureCapability::UnknownRequiredExtension(id)
        }
        (FixtureCapabilityKindDeclaration::TokensExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::Tokens)
        }
        (FixtureCapabilityKindDeclaration::ParseErrorsExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::ParseErrors)
        }
        (FixtureCapabilityKindDeclaration::ImplementationDiagnosticsExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::ImplementationDiagnostics)
        }
        (FixtureCapabilityKindDeclaration::DocumentModeExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::DocumentMode)
        }
        (FixtureCapabilityKindDeclaration::TreeExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::Tree)
        }
        (FixtureCapabilityKindDeclaration::PatchesExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::Patches)
        }
        (FixtureCapabilityKindDeclaration::TransitionsExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::Transitions)
        }
        (FixtureCapabilityKindDeclaration::UnsupportedFeaturesExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::UnsupportedFeatures)
        }
        (FixtureCapabilityKindDeclaration::FinalInvariantsExpectation, None) => {
            FixtureCapability::Expectation(ExpectationSurface::FinalInvariants)
        }
        _ => {
            return invalid_combination(
                bundle,
                "capability id is required only for unknown-required-extension",
            );
        }
    };
    Ok(capability)
}

fn map_expected_failure(value: ExpectedFailureDeclaration) -> ExpectedFailureClassification {
    match value {
        ExpectedFailureDeclaration::TokenSnapshotRead => ExpectedFailureClassification::Execution(
            LegacyExecutionFailureClass::SnapshotRead(ExpectationSurface::Tokens),
        ),
        ExpectedFailureDeclaration::TokenSnapshotFormat => {
            ExpectedFailureClassification::Execution(LegacyExecutionFailureClass::SnapshotFormat(
                ExpectationSurface::Tokens,
            ))
        }
        ExpectedFailureDeclaration::TokenizerDriver => {
            ExpectedFailureClassification::Execution(LegacyExecutionFailureClass::TokenizerDriver)
        }
        ExpectedFailureDeclaration::ValidatedFixtureInvariant => {
            ExpectedFailureClassification::Execution(
                LegacyExecutionFailureClass::ValidatedFixtureInvariant,
            )
        }
        ExpectedFailureDeclaration::TokensMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::Tokens)
        }
        ExpectedFailureDeclaration::ParseErrorsMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::ParseErrors)
        }
        ExpectedFailureDeclaration::ImplementationDiagnosticsMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(
                ExpectationSurface::ImplementationDiagnostics,
            )
        }
        ExpectedFailureDeclaration::DocumentModeMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::DocumentMode)
        }
        ExpectedFailureDeclaration::TreeMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::Tree)
        }
        ExpectedFailureDeclaration::PatchesMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::Patches)
        }
        ExpectedFailureDeclaration::TransitionsMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::Transitions)
        }
        ExpectedFailureDeclaration::UnsupportedFeaturesMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(
                ExpectationSurface::UnsupportedFeatures,
            )
        }
        ExpectedFailureDeclaration::FinalInvariantsMismatch => {
            ExpectedFailureClassification::ExpectationMismatch(ExpectationSurface::FinalInvariants)
        }
        ExpectedFailureDeclaration::DecoderCarryNotEmptyInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::DecoderCarryNotEmpty,
            )
        }
        ExpectedFailureDeclaration::PreprocessingNotFlushedInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::PreprocessingNotFlushed,
            )
        }
        ExpectedFailureDeclaration::EofEmissionInvalidInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::EofEmissionInvalid,
            )
        }
        ExpectedFailureDeclaration::PendingTokenizerConstructInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::PendingTokenizerConstruct,
            )
        }
        ExpectedFailureDeclaration::TokenizerOutputUnaccountedInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::TokenizerOutputUnaccounted,
            )
        }
        ExpectedFailureDeclaration::PendingTableTextInvariant => {
            ExpectedFailureClassification::InvariantFailure(InvariantFailureCode::PendingTableText)
        }
        ExpectedFailureDeclaration::InvalidInsertionModeInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::InvalidInsertionMode,
            )
        }
        ExpectedFailureDeclaration::OpenElementsInconsistentInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::OpenElementsInconsistent,
            )
        }
        ExpectedFailureDeclaration::ActiveFormattingInconsistentInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::ActiveFormattingInconsistent,
            )
        }
        ExpectedFailureDeclaration::TemplateModesInconsistentInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::TemplateModesInconsistent,
            )
        }
        ExpectedFailureDeclaration::FormPointerInvalidInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::FormPointerInvalid,
            )
        }
        ExpectedFailureDeclaration::ParentChildRelationshipInvalidInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::ParentChildRelationshipInvalid,
            )
        }
        ExpectedFailureDeclaration::NamespaceRelationshipInvalidInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::NamespaceRelationshipInvalid,
            )
        }
        ExpectedFailureDeclaration::TemplateAssociationInvalidInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::TemplateAssociationInvalid,
            )
        }
        ExpectedFailureDeclaration::PatchMaterializationIncompleteInvariant => {
            ExpectedFailureClassification::InvariantFailure(
                InvariantFailureCode::PatchMaterializationIncomplete,
            )
        }
        ExpectedFailureDeclaration::LiveTreeMismatchInvariant => {
            ExpectedFailureClassification::InvariantFailure(InvariantFailureCode::LiveTreeMismatch)
        }
    }
}

fn map_final_invariant(
    bundle: &FixtureBundle,
    code: &str,
) -> Result<InvariantFailureCode, FixtureLoadError> {
    let value = match code {
        "decoder-carry-not-empty" => InvariantFailureCode::DecoderCarryNotEmpty,
        "preprocessing-not-flushed" => InvariantFailureCode::PreprocessingNotFlushed,
        "eof-emission-invalid" => InvariantFailureCode::EofEmissionInvalid,
        "pending-tokenizer-construct" => InvariantFailureCode::PendingTokenizerConstruct,
        "tokenizer-output-unaccounted" => InvariantFailureCode::TokenizerOutputUnaccounted,
        "pending-table-text" => InvariantFailureCode::PendingTableText,
        "invalid-insertion-mode" => InvariantFailureCode::InvalidInsertionMode,
        "open-elements-inconsistent" => InvariantFailureCode::OpenElementsInconsistent,
        "active-formatting-inconsistent" => InvariantFailureCode::ActiveFormattingInconsistent,
        "template-modes-inconsistent" => InvariantFailureCode::TemplateModesInconsistent,
        "form-pointer-invalid" => InvariantFailureCode::FormPointerInvalid,
        "parent-child-relationship-invalid" => InvariantFailureCode::ParentChildRelationshipInvalid,
        "namespace-relationship-invalid" => InvariantFailureCode::NamespaceRelationshipInvalid,
        "template-association-invalid" => InvariantFailureCode::TemplateAssociationInvalid,
        "patch-materialization-incomplete" => InvariantFailureCode::PatchMaterializationIncomplete,
        "live-tree-mismatch" => InvariantFailureCode::LiveTreeMismatch,
        _ => {
            return Err(bundle_error(
                bundle,
                FixtureLoadErrorKind::InvalidDisposition(
                    "unknown final-invariant code".to_string(),
                ),
            ));
        }
    };
    Ok(value)
}

fn validate_skip_classification(
    bundle: &FixtureBundle,
    declaration: SkipClassificationDeclaration,
) -> Result<SkipClassification, FixtureLoadError> {
    match declaration.kind {
        SkipClassificationKindDeclaration::UnsupportedCapability => {
            let capability = declaration.capability;
            let capability = map_capability(bundle, capability)?;
            require_non_active_capability(bundle, &capability, "skipped unsupported capability")?;
            Ok(SkipClassification::UnsupportedCapability(capability))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FixtureCapabilityPolicy {
    CompletedMustRemainActive,
    MayUseExternalDisposition,
}

pub(super) fn capability_policy(capability: &FixtureCapability) -> FixtureCapabilityPolicy {
    match capability {
        FixtureCapability::DocumentExecution
        | FixtureCapability::Expectation(ExpectationSurface::Tokens) => {
            FixtureCapabilityPolicy::CompletedMustRemainActive
        }
        FixtureCapability::RawByteInput
        | FixtureCapability::ByteDelivery
        | FixtureCapability::UnicodeScalarChunking
        | FixtureCapability::FragmentParsing
        | FixtureCapability::ScriptingEnabled
        | FixtureCapability::UnknownRequiredExtension(_)
        | FixtureCapability::Expectation(ExpectationSurface::ParseErrors)
        | FixtureCapability::Expectation(ExpectationSurface::ImplementationDiagnostics)
        | FixtureCapability::Expectation(ExpectationSurface::DocumentMode)
        | FixtureCapability::Expectation(ExpectationSurface::Tree)
        | FixtureCapability::Expectation(ExpectationSurface::Patches)
        | FixtureCapability::Expectation(ExpectationSurface::Transitions)
        | FixtureCapability::Expectation(ExpectationSurface::UnsupportedFeatures)
        | FixtureCapability::Expectation(ExpectationSurface::FinalInvariants) => {
            FixtureCapabilityPolicy::MayUseExternalDisposition
        }
    }
}

/// Returns whether the fixture actually declares semantics represented by the
/// exact capability.
///
/// Every declared delivery is part of fixture semantics. The reference
/// delivery selects the ordinary comparison baseline, while a transition
/// expectation may select another already-declared delivery; neither narrows
/// which declared delivery capabilities are relevant.
pub(super) fn capability_is_relevant(
    capability: &FixtureCapability,
    input: &ExactInput,
    execution: &ValidatedExecution,
    expectations: &EnabledExpectations,
    required_unknown_extensions: &[String],
) -> bool {
    match capability {
        FixtureCapability::RawByteInput => matches!(input, ExactInput::RawBytes { .. }),
        FixtureCapability::ByteDelivery => execution.deliveries().iter().any(|delivery| {
            matches!(
                delivery,
                ValidatedDelivery::WholeBytes { .. } | ValidatedDelivery::ByteBoundaries { .. }
            )
        }),
        FixtureCapability::UnicodeScalarChunking => execution
            .deliveries()
            .iter()
            .any(|delivery| matches!(delivery, ValidatedDelivery::UnicodeScalarBoundaries { .. })),
        FixtureCapability::DocumentExecution => {
            matches!(execution.target(), ValidatedParserTarget::Document { .. })
        }
        FixtureCapability::FragmentParsing => {
            matches!(execution.target(), ValidatedParserTarget::Fragment { .. })
        }
        FixtureCapability::ScriptingEnabled => matches!(
            execution.target(),
            ValidatedParserTarget::Document {
                scripting: ScriptingMode::Enabled
            } | ValidatedParserTarget::Fragment {
                scripting: ScriptingMode::Enabled,
                ..
            }
        ),
        FixtureCapability::UnknownRequiredExtension(id) => required_unknown_extensions
            .binary_search_by(|candidate| candidate.as_str().cmp(id.as_str()))
            .is_ok(),
        FixtureCapability::Expectation(surface) => expectations.is_declared(*surface),
    }
}

fn capability_name(capability: &FixtureCapability) -> String {
    match capability {
        FixtureCapability::RawByteInput => "raw-byte-input".to_string(),
        FixtureCapability::ByteDelivery => "byte-delivery".to_string(),
        FixtureCapability::UnicodeScalarChunking => "unicode-scalar-chunking".to_string(),
        FixtureCapability::DocumentExecution => "document-execution".to_string(),
        FixtureCapability::FragmentParsing => "fragment-parsing".to_string(),
        FixtureCapability::ScriptingEnabled => "scripting-enabled".to_string(),
        FixtureCapability::UnknownRequiredExtension(id) => {
            format!("unknown-required-extension:{id}")
        }
        FixtureCapability::Expectation(ExpectationSurface::Tokens) => {
            "tokens-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::ParseErrors) => {
            "parse-errors-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::ImplementationDiagnostics) => {
            "implementation-diagnostics-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::DocumentMode) => {
            "document-mode-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::Tree) => "tree-expectation".to_string(),
        FixtureCapability::Expectation(ExpectationSurface::Patches) => {
            "patches-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::Transitions) => {
            "transitions-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::UnsupportedFeatures) => {
            "unsupported-features-expectation".to_string()
        }
        FixtureCapability::Expectation(ExpectationSurface::FinalInvariants) => {
            "final-invariants-expectation".to_string()
        }
    }
}

fn require_non_active_capability(
    bundle: &FixtureBundle,
    capability: &FixtureCapability,
    disposition: &str,
) -> Result<(), FixtureLoadError> {
    if capability_policy(capability) == FixtureCapabilityPolicy::CompletedMustRemainActive {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(format!(
                "completed Milestone AE capability {} cannot use {disposition}",
                capability_name(capability)
            )),
        ));
    }
    Ok(())
}

fn require_non_active_failure(
    bundle: &FixtureBundle,
    failure: &ExpectedFailureClassification,
) -> Result<(), FixtureLoadError> {
    let capability = match failure {
        ExpectedFailureClassification::Execution(LegacyExecutionFailureClass::SnapshotRead(
            surface,
        ))
        | ExpectedFailureClassification::Execution(LegacyExecutionFailureClass::SnapshotFormat(
            surface,
        ))
        | ExpectedFailureClassification::ExpectationMismatch(surface) => {
            Some(FixtureCapability::Expectation(*surface))
        }
        ExpectedFailureClassification::Execution(LegacyExecutionFailureClass::TokenizerDriver) => {
            Some(FixtureCapability::Expectation(ExpectationSurface::Tokens))
        }
        ExpectedFailureClassification::InvariantFailure(_) => Some(FixtureCapability::Expectation(
            ExpectationSurface::FinalInvariants,
        )),
        ExpectedFailureClassification::Execution(
            LegacyExecutionFailureClass::ValidatedFixtureInvariant,
        ) => None,
    };
    let Some(capability) = capability else {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(
                "validated fixture invariants cannot be accepted as an expected failure"
                    .to_string(),
            ),
        ));
    };
    require_non_active_capability(bundle, &capability, "expected failure")
}

fn is_kebab_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn is_versioned_extension_id(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    if segments.len() < 3 || !segments.iter().all(|segment| is_kebab_identifier(segment)) {
        return false;
    }
    let Some((_, version)) = segments
        .last()
        .and_then(|segment| segment.rsplit_once("-v"))
    else {
        return false;
    };
    !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
}

fn require_non_empty(
    bundle: &FixtureBundle,
    field: &str,
    value: &str,
) -> Result<(), FixtureLoadError> {
    if value.trim().is_empty() {
        return Err(bundle_error(
            bundle,
            FixtureLoadErrorKind::InvalidDisposition(format!("{field} must be non-empty")),
        ));
    }
    Ok(())
}

fn invalid_combination<T>(bundle: &FixtureBundle, message: &str) -> Result<T, FixtureLoadError> {
    Err(bundle_error(
        bundle,
        FixtureLoadErrorKind::InvalidCombination(message.to_string()),
    ))
}

fn invalid_delivery<T>(
    bundle: &FixtureBundle,
    error: DeliveryValidationError,
) -> Result<T, FixtureLoadError> {
    Err(bundle_error(
        bundle,
        FixtureLoadErrorKind::InvalidDelivery(error),
    ))
}

fn bundle_error(bundle: &FixtureBundle, kind: FixtureLoadErrorKind) -> FixtureLoadError {
    FixtureLoadError {
        path: bundle.repository_relative_path().to_string(),
        kind,
    }
}
