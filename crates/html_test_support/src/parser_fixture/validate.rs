use super::load::{
    FixtureLoadError, FixtureLoadErrorKind, FixtureRepositoryPolicy, normalize_relative_path,
    read_regular_file, validate_relative_path,
};
use super::model::*;
use super::schema::*;
use html::ElementNamespace;
use html::conformance::InvariantFailureCode;
use ring::digest::{SHA256, digest};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::Path;

/// A fixture whose complete serialized declaration passed the canonical v1
/// validation boundary.
///
/// Fields and construction stay in this module so callers cannot assemble a
/// partially validated value from otherwise plausible component values.
#[derive(Clone, Debug)]
pub struct ValidatedFixtureSpec {
    id: FixtureId,
    bundle: FixtureBundle,
    source: FixtureSource,
    input: ExactInput,
    execution: ValidatedExecution,
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
        match self.execution.target() {
            ValidatedParserTarget::StandaloneTokenizer => ParserTargetKind::StandaloneTokenizer,
            ValidatedParserTarget::Document { .. } => ParserTargetKind::Document,
            ValidatedParserTarget::Fragment { .. } => ParserTargetKind::Fragment,
        }
    }

    pub fn scripting_mode(&self) -> Option<ScriptingMode> {
        match self.execution.target() {
            ValidatedParserTarget::StandaloneTokenizer => None,
            ValidatedParserTarget::Document { scripting }
            | ValidatedParserTarget::Fragment { scripting, .. } => Some(*scripting),
        }
    }

    pub fn fragment_namespace(&self) -> Option<ElementNamespace> {
        match self.execution.target() {
            ValidatedParserTarget::Fragment { context, .. } => Some(context.namespace()),
            ValidatedParserTarget::StandaloneTokenizer | ValidatedParserTarget::Document { .. } => {
                None
            }
        }
    }

    pub fn fragment_local_name(&self) -> Option<&str> {
        match self.execution.target() {
            ValidatedParserTarget::Fragment { context, .. } => Some(context.local_name()),
            ValidatedParserTarget::StandaloneTokenizer | ValidatedParserTarget::Document { .. } => {
                None
            }
        }
    }

    pub fn reference_delivery(&self) -> &DeliveryName {
        self.execution.reference_delivery()
    }

    pub fn delivery_names(&self) -> impl ExactSizeIterator<Item = &DeliveryName> {
        self.execution
            .deliveries()
            .iter()
            .map(ValidatedDelivery::name)
    }

    pub fn delivery_boundaries(&self, name: &str) -> Option<Option<&[usize]>> {
        self.execution
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

    pub(super) fn input(&self) -> &ExactInput {
        &self.input
    }

    pub(super) fn execution(&self) -> &ValidatedExecution {
        &self.execution
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

pub(super) fn validate_fixture(
    declaration: FixtureFileV1,
    bundle: FixtureBundle,
    repository_policy: FixtureRepositoryPolicy,
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
    let input_bytes = read_regular_file(&bundle, &input_path)?;
    validate_sha256(&bundle, &declaration.input.sha256, &input_bytes)?;
    let input = validate_input(&bundle, declaration.input, input_bytes)?;

    let execution = validate_execution(&bundle, &input, declaration.execution)?;
    let expectations = validate_expectations(&bundle, &execution, declaration.expectations)?;
    if !has_any_expectation(&expectations) {
        return invalid_combination(&bundle, "fixture must declare at least one expectation");
    }
    validate_orphan_sidecars(&bundle, &input_path, &expectations)?;
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
        source,
        input,
        execution,
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
        (FixtureSourceKindDeclaration::External, Some(provenance), None) => {
            require_non_empty(bundle, "external provenance", &provenance)?;
            Ok(FixtureSource::External { provenance })
        }
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

fn validate_execution(
    bundle: &FixtureBundle,
    input: &ExactInput,
    declaration: ExecutionDeclaration,
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
    if !is_kebab_identifier(&declaration.reference_delivery) {
        return invalid_combination(
            bundle,
            "reference delivery must be a lowercase kebab identifier",
        );
    }
    let reference_delivery = DeliveryName::validated(declaration.reference_delivery);
    let mut names = BTreeSet::new();
    let mut deliveries = Vec::with_capacity(declaration.deliveries.len());
    let extent = match input {
        ExactInput::Utf8Text { text, .. } => text.chars().count(),
        ExactInput::RawBytes { bytes, .. } => bytes.len(),
    };
    for delivery in declaration.deliveries {
        if !is_kebab_identifier(&delivery.name) || !names.insert(delivery.name.clone()) {
            return invalid_combination(
                bundle,
                "delivery names must be unique lowercase kebab identifiers",
            );
        }
        let name = DeliveryName::validated(delivery.name);
        let validated = match (input, delivery.unit, delivery.strategy, delivery.boundaries) {
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
                name,
                boundaries: validate_boundaries(bundle, boundaries, extent)?,
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
                name,
                boundaries: validate_boundaries(bundle, boundaries, extent)?,
            },
            _ => {
                return invalid_combination(
                    bundle,
                    "input kind, delivery unit, strategy, and boundaries are inconsistent",
                );
            }
        };
        deliveries.push(validated);
    }
    if !names.contains(reference_delivery.as_str()) {
        return invalid_combination(
            bundle,
            "reference delivery does not name a declared delivery",
        );
    }
    Ok(ValidatedExecution::validated(
        target,
        reference_delivery,
        deliveries,
    ))
}

fn validate_expectations(
    bundle: &FixtureBundle,
    execution: &ValidatedExecution,
    declaration: FixtureExpectationDeclarations,
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
            read_regular_file(bundle, &transition.path)?;
            Ok(TransitionSnapshotExpectation::validated(
                delivery,
                SnapshotPath::validated(transition.path),
            ))
        })
        .collect::<Result<Vec<_>, FixtureLoadError>>()?;

    Ok(EnabledExpectations::validated(
        snapshot_surface(bundle, declaration.tokens)?,
        snapshot_surface(bundle, declaration.parse_errors)?,
        snapshot_surface(bundle, declaration.implementation_diagnostics)?,
        snapshot_surface(bundle, declaration.document_mode)?,
        snapshot_surface(bundle, declaration.tree)?,
        snapshot_surface(bundle, declaration.patches)?,
        if transitions.is_empty() {
            ExpectedSurface::NotDeclared
        } else {
            ExpectedSurface::Compare(transitions)
        },
        snapshot_surface(bundle, declaration.unsupported_features)?,
        snapshot_surface(bundle, declaration.final_invariants)?,
    ))
}

fn snapshot_surface(
    bundle: &FixtureBundle,
    path: Option<String>,
) -> Result<ExpectedSurface<SnapshotPath>, FixtureLoadError> {
    let Some(path) = path else {
        return Ok(ExpectedSurface::NotDeclared);
    };
    validate_relative_path(&path).map_err(|kind| bundle_error(bundle, kind))?;
    read_regular_file(bundle, &path)?;
    Ok(ExpectedSurface::Compare(SnapshotPath::validated(path)))
}

fn validate_orphan_sidecars(
    bundle: &FixtureBundle,
    input_path: &str,
    expectations: &EnabledExpectations,
) -> Result<(), FixtureLoadError> {
    let mut declared = BTreeSet::from(["fixture.toml".to_string(), input_path.to_string()]);
    for surface in [
        expectations.tokens(),
        expectations.parse_errors(),
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
    boundaries: Vec<usize>,
    extent: usize,
) -> Result<Vec<usize>, FixtureLoadError> {
    if boundaries.is_empty()
        || boundaries.windows(2).any(|pair| pair[0] >= pair[1])
        || boundaries
            .iter()
            .any(|boundary| *boundary == 0 || *boundary >= extent)
    {
        return invalid_combination(
            bundle,
            "chunk boundaries must be strictly increasing interior offsets",
        );
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
            ExecutionFailureClass::SnapshotRead(ExpectationSurface::Tokens),
        ),
        ExpectedFailureDeclaration::TokenSnapshotFormat => {
            ExpectedFailureClassification::Execution(ExecutionFailureClass::SnapshotFormat(
                ExpectationSurface::Tokens,
            ))
        }
        ExpectedFailureDeclaration::TokenizerDriver => {
            ExpectedFailureClassification::Execution(ExecutionFailureClass::TokenizerDriver)
        }
        ExpectedFailureDeclaration::ValidatedFixtureInvariant => {
            ExpectedFailureClassification::Execution(
                ExecutionFailureClass::ValidatedFixtureInvariant,
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
                "completed Milestone AE capability {capability:?} cannot use {disposition}"
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
        ExpectedFailureClassification::Execution(ExecutionFailureClass::SnapshotRead(surface))
        | ExpectedFailureClassification::Execution(ExecutionFailureClass::SnapshotFormat(
            surface,
        ))
        | ExpectedFailureClassification::ExpectationMismatch(surface) => {
            Some(FixtureCapability::Expectation(*surface))
        }
        ExpectedFailureClassification::Execution(ExecutionFailureClass::TokenizerDriver) => {
            Some(FixtureCapability::Expectation(ExpectationSurface::Tokens))
        }
        ExpectedFailureClassification::InvariantFailure(_) => Some(FixtureCapability::Expectation(
            ExpectationSurface::FinalInvariants,
        )),
        ExpectedFailureClassification::Execution(
            ExecutionFailureClass::ValidatedFixtureInvariant,
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

fn bundle_error(bundle: &FixtureBundle, kind: FixtureLoadErrorKind) -> FixtureLoadError {
    FixtureLoadError {
        path: bundle.repository_relative_path().to_string(),
        kind,
    }
}
