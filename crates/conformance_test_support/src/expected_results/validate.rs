use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};

use crate::model::{PortablePathComponent, TestId, TestIdValidationError, ValidatedInventory};

use super::diagnostic::{
    ExpectedResultsDiagnostic, ExpectedResultsDiagnosticKind as K, ExpectedResultsErrors,
};
use super::model::*;
use super::schema::*;

struct ValidatedCandidate {
    id: TestId,
    classification: Classification,
    references: Vec<BorrowserReference>,
}

pub fn load_expected_results(
    repository_root: &Path,
    inventory: &ValidatedInventory,
) -> Result<ValidatedExpectedResults, ExpectedResultsErrors> {
    let registry_path = repository_root.join(EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH);
    load_expected_results_from_path(repository_root, &registry_path, inventory)
}

fn load_expected_results_from_path(
    repository_root: &Path,
    registry_path: &Path,
    inventory: &ValidatedInventory,
) -> Result<ValidatedExpectedResults, ExpectedResultsErrors> {
    let registry_subject = EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH.to_owned();
    let bytes = read_registry(repository_root, registry_path, &registry_subject)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
            registry_subject.clone(),
            K::InvalidUtf8,
        )])
    })?;
    let value = toml::from_str::<toml::Value>(text).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
            registry_subject.clone(),
            K::MalformedToml,
        )])
    })?;
    let Some(table) = value.as_table() else {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(registry_subject, K::InvalidRegistryShape),
        ]));
    };
    let format = table
        .get("format")
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if format != EXPECTED_RESULTS_FORMAT_V1 {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(
                registry_subject,
                K::UnsupportedFormat {
                    value: format.to_owned(),
                },
            ),
        ]));
    }
    let granularity = table
        .get("granularity")
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    if granularity != EXPECTED_RESULTS_GRANULARITY_V1 {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(
                registry_subject,
                K::InvalidGranularity {
                    value: granularity.to_owned(),
                },
            ),
        ]));
    }
    let wire = toml::from_str::<ExpectedResultsFileV1>(text).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
            registry_subject,
            K::InvalidRegistryShape,
        )])
    })?;
    debug_assert_eq!(wire.format, EXPECTED_RESULTS_FORMAT_V1);
    debug_assert_eq!(wire.granularity, EXPECTED_RESULTS_GRANULARITY_V1);
    validate_file(repository_root, wire, inventory)
}

fn read_registry(
    repository_root: &Path,
    registry_path: &Path,
    subject: &str,
) -> Result<Vec<u8>, ExpectedResultsErrors> {
    let relative = registry_path.strip_prefix(repository_root).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
            subject,
            K::RegistryOutsideRepository,
        )])
    })?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(subject, K::RegistryOutsideRepository),
        ]));
    }
    let mut current = repository_root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            unreachable!("components were validated above")
        };
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| {
            ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
                subject,
                K::ReadFailed,
            )])
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ExpectedResultsErrors::sorted(vec![
                ExpectedResultsDiagnostic::new(subject, K::SymlinkNotAllowed),
            ]));
        }
    }
    let metadata = fs::symlink_metadata(registry_path).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(subject, K::ReadFailed)])
    })?;
    if !metadata.is_file() {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(subject, K::RegistryNotRegularFile),
        ]));
    }
    if metadata.len() > MAX_EXPECTED_RESULTS_BYTES {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(
                subject,
                K::RegistryTooLarge {
                    observed_at_least: metadata.len(),
                    maximum: MAX_EXPECTED_RESULTS_BYTES,
                },
            ),
        ]));
    }
    let file = File::open(registry_path).map_err(|_| {
        ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(subject, K::ReadFailed)])
    })?;
    let mut bytes = Vec::new();
    file.take(MAX_EXPECTED_RESULTS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            ExpectedResultsErrors::sorted(vec![ExpectedResultsDiagnostic::new(
                subject,
                K::ReadFailed,
            )])
        })?;
    if bytes.len() as u64 > MAX_EXPECTED_RESULTS_BYTES {
        return Err(ExpectedResultsErrors::sorted(vec![
            ExpectedResultsDiagnostic::new(
                subject,
                K::RegistryTooLarge {
                    observed_at_least: MAX_EXPECTED_RESULTS_BYTES + 1,
                    maximum: MAX_EXPECTED_RESULTS_BYTES,
                },
            ),
        ]));
    }
    Ok(bytes)
}

fn validate_file(
    repository_root: &Path,
    wire: ExpectedResultsFileV1,
    inventory: &ValidatedInventory,
) -> Result<ValidatedExpectedResults, ExpectedResultsErrors> {
    let mut diagnostics = Vec::new();
    let mut candidates = Vec::new();
    let mut declared_ids = Vec::new();

    for (index, record) in wire.tests.into_iter().enumerate() {
        let fallback = format!("record-{index:08}");
        let subject = if record.id.is_empty() {
            fallback
        } else {
            record.id.clone()
        };
        let id = validate_test_id(&record.id, &subject, &mut diagnostics);
        if let Some(id) = &id {
            declared_ids.push(id.as_str().to_owned());
        }
        if let (Some(id), Some((classification, references))) = (
            id,
            validate_record(repository_root, record, &subject, &mut diagnostics),
        ) {
            candidates.push(ValidatedCandidate {
                id,
                classification,
                references,
            });
        }
    }

    let mut counts = BTreeMap::<String, usize>::new();
    for id in &declared_ids {
        *counts.entry(id.clone()).or_default() += 1;
    }
    for (id, count) in &counts {
        if *count > 1 {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                id,
                K::DuplicateTestId { value: id.clone() },
            ));
        }
    }

    let inventory_by_id = inventory
        .fixtures()
        .iter()
        .map(|fixture| (fixture.id().as_str(), fixture))
        .collect::<BTreeMap<_, _>>();
    let declared = declared_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for id in declared.iter().copied() {
        if !inventory_by_id.contains_key(id) {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                id,
                K::UnknownTestId {
                    value: id.to_owned(),
                },
            ));
        }
    }
    for id in inventory_by_id.keys().copied() {
        if !declared.contains(id) {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                id,
                K::MissingTestMetadata {
                    value: id.to_owned(),
                },
            ));
        }
    }

    if !diagnostics.is_empty() {
        return Err(ExpectedResultsErrors::sorted(diagnostics));
    }

    let records = candidates
        .into_iter()
        .map(|candidate| {
            let fixture = (*inventory_by_id
                .get(candidate.id.as_str())
                .expect("validated reconciliation guarantees the fixture"))
            .clone();
            ExpectedResultRecord::validated(fixture, candidate.classification, candidate.references)
        })
        .collect();
    Ok(ValidatedExpectedResults::validated(records))
}

fn validate_test_id(
    raw: &str,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<TestId> {
    match TestId::parse(raw) {
        Ok(id) => Some(id),
        Err(TestIdValidationError::TooLong) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::TestIdTooLong {
                    value: raw.to_owned(),
                },
            ));
            None
        }
        Err(TestIdValidationError::CaseUnsafe) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::CaseUnsafeTestId {
                    value: raw.to_owned(),
                },
            ));
            None
        }
        Err(TestIdValidationError::InvalidGrammar) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidTestId {
                    value: raw.to_owned(),
                },
            ));
            None
        }
    }
}

fn validate_record(
    repository_root: &Path,
    record: ExpectedResultRecordV1,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<(Classification, Vec<BorrowserReference>)> {
    let ExpectedResultRecordV1 {
        id: _,
        classification,
        reason,
        requirements,
        lane_exclusions,
        references,
        engine,
        harness,
        environment,
        expectation,
        stability,
    } = record;
    let references = validate_references(
        repository_root,
        references.unwrap_or_default(),
        subject,
        diagnostics,
    );
    match classification.as_str() {
        "not-yet-classified" => {
            let reason = required_reason(reason, "reason", subject, diagnostics);
            for (field, present) in [
                ("requirements", requirements.is_some()),
                ("lane_exclusions", lane_exclusions.is_some()),
                ("engine", engine.is_some()),
                ("harness", harness.is_some()),
                ("environment", environment.is_some()),
                ("expectation", expectation.is_some()),
                ("stability", stability.is_some()),
            ] {
                if present {
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::ForbiddenField {
                            field,
                            classification: "not-yet-classified",
                        },
                    ));
                }
            }
            reason.map(|reason| (Classification::NotYetClassified { reason }, references))
        }
        "classified" => {
            if reason.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::ForbiddenField {
                        field: "reason",
                        classification: "classified",
                    },
                ));
            }
            let requirements = required(requirements, "requirements", subject, diagnostics)
                .and_then(|values| validate_requirements(values, subject, diagnostics));
            let lanes = required(lane_exclusions, "lane_exclusions", subject, diagnostics)
                .and_then(|values| validate_lanes(values, subject, diagnostics));
            let engine = required(engine, "engine", subject, diagnostics);
            let harness = required(harness, "harness", subject, diagnostics);
            let environment = required(environment, "environment", subject, diagnostics)
                .and_then(|value| validate_environment(value, subject, diagnostics));
            let expectation = required(expectation, "expectation", subject, diagnostics)
                .and_then(|value| validate_expectation(value, subject, diagnostics));
            let stability = required(stability, "stability", subject, diagnostics)
                .and_then(|value| validate_stability(value, subject, diagnostics));

            let engine = match (engine, requirements.as_ref()) {
                (Some(value), Some(requirements)) => {
                    validate_engine(value, requirements, subject, diagnostics)
                }
                _ => None,
            };
            let harness = harness.and_then(|value| validate_harness(value, subject, diagnostics));
            match (
                requirements,
                engine,
                harness,
                environment,
                expectation,
                stability,
                lanes,
            ) {
                (
                    Some(requirements),
                    Some(engine),
                    Some(harness),
                    Some(environment),
                    Some(expectation),
                    Some(stability),
                    Some(lanes),
                ) => Some((
                    Classification::Classified(ClassifiedMetadata::validated(
                        requirements,
                        engine,
                        harness,
                        environment,
                        expectation,
                        stability,
                        lanes,
                    )),
                    references,
                )),
                _ => None,
            }
        }
        value => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownClassification {
                    value: value.to_owned(),
                },
            ));
            None
        }
    }
}

fn required<T>(
    value: Option<T>,
    field: &'static str,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<T> {
    if value.is_none() {
        diagnostics.push(ExpectedResultsDiagnostic::new(
            subject,
            K::MissingClassifiedField { field },
        ));
    }
    value
}

fn validate_reason(
    value: String,
    field: &'static str,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<NonEmptyReason> {
    match NonEmptyReason::parse(value) {
        Ok(reason) => Some(reason),
        Err(ReasonValidationError::Empty) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidReason {
                    field,
                    problem: "must be non-empty",
                },
            ));
            None
        }
        Err(ReasonValidationError::TooLong) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidReason {
                    field,
                    problem: "is longer than 1024 UTF-8 bytes",
                },
            ));
            None
        }
        Err(ReasonValidationError::SurroundingWhitespace) => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidReason {
                    field,
                    problem: "must not have surrounding whitespace",
                },
            ));
            None
        }
    }
}

fn required_reason(
    value: Option<String>,
    field: &'static str,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<NonEmptyReason> {
    match value {
        Some(value) => validate_reason(value, field, subject, diagnostics),
        None => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidReason {
                    field,
                    problem: "is required",
                },
            ));
            None
        }
    }
}

fn validate_requirements(
    values: Vec<String>,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<Vec<RequirementTag>> {
    let mut parsed = Vec::new();
    let mut valid = true;
    for value in values {
        match RequirementTag::parse(&value) {
            Some(tag) if parsed.contains(&tag) => {
                valid = false;
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::DuplicateRequirement { value },
                ));
            }
            Some(tag) => parsed.push(tag),
            None => {
                valid = false;
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnknownRequirement { value },
                ));
            }
        }
    }
    if parsed.contains(&RequirementTag::NoJs) && parsed.contains(&RequirementTag::RequiresJs) {
        valid = false;
        diagnostics.push(ExpectedResultsDiagnostic::new(
            subject,
            K::ContradictoryRequirements {
                left: RequirementTag::NoJs.as_str().to_owned(),
                right: RequirementTag::RequiresJs.as_str().to_owned(),
            },
        ));
    }
    valid.then_some(parsed)
}

fn validate_engine(
    wire: EngineV1,
    requirements: &[RequirementTag],
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<EngineCapabilityAvailability> {
    match wire.availability.as_str() {
        "available" => {
            if wire.missing.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnexpectedUnavailableCapability,
                ));
                None
            } else {
                Some(EngineCapabilityAvailability::Available)
            }
        }
        "not-yet-established" => {
            if wire.missing.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnexpectedUnavailableCapability,
                ));
                None
            } else {
                Some(EngineCapabilityAvailability::NotYetEstablished)
            }
        }
        "unavailable" => {
            let Some(missing) = wire.missing else {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::MissingUnavailableCapability,
                ));
                return None;
            };
            if missing.is_empty() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::MissingUnavailableCapability,
                ));
                return None;
            }
            let mut parsed = Vec::new();
            let mut keys = BTreeSet::new();
            let mut valid = true;
            for item in missing {
                let Some(kind) = EngineCapabilityKind::parse(&item.kind) else {
                    valid = false;
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::UnknownEngineCapability { value: item.kind },
                    ));
                    continue;
                };
                let feature = match (kind.requires_feature(), item.feature) {
                    (true, Some(feature)) => match CapabilityFeatureId::parse(&feature) {
                        Ok(feature) => Some(feature),
                        Err(_) => {
                            valid = false;
                            diagnostics.push(ExpectedResultsDiagnostic::new(
                                subject,
                                K::InvalidCapabilityFeature { value: feature },
                            ));
                            None
                        }
                    },
                    (true, None) => {
                        valid = false;
                        diagnostics.push(ExpectedResultsDiagnostic::new(
                            subject,
                            K::MissingCapabilityFeature {
                                capability: kind.as_str().to_owned(),
                            },
                        ));
                        None
                    }
                    (false, Some(_)) => {
                        valid = false;
                        diagnostics.push(ExpectedResultsDiagnostic::new(
                            subject,
                            K::UnexpectedCapabilityFeature {
                                capability: kind.as_str().to_owned(),
                            },
                        ));
                        None
                    }
                    (false, None) => None,
                };
                let reason =
                    required_reason(item.reason, "engine.missing.reason", subject, diagnostics);
                let requirement = kind.requirement_tag();
                if !requirements.contains(&requirement) {
                    valid = false;
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::IrrelevantEngineCapability {
                            capability: kind.as_str().to_owned(),
                            requirement: requirement.as_str().to_owned(),
                        },
                    ));
                }
                if let Some(reason) = reason {
                    let key = (kind, feature.clone());
                    let candidate = MissingEngineCapability::validated(kind, feature, reason);
                    if !keys.insert(key) {
                        valid = false;
                        diagnostics.push(ExpectedResultsDiagnostic::new(
                            subject,
                            K::DuplicateEngineCapability {
                                capability: kind.as_str().to_owned(),
                            },
                        ));
                    } else {
                        parsed.push(candidate);
                    }
                } else {
                    valid = false;
                }
            }
            parsed.sort();
            valid.then_some(EngineCapabilityAvailability::Unavailable { missing: parsed })
        }
        value => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownEngineAvailability {
                    value: value.to_owned(),
                },
            ));
            None
        }
    }
}

fn validate_harness(
    wire: HarnessV1,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<HarnessReadiness> {
    match wire.readiness.as_str() {
        "ready" => {
            if wire.limitations.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnexpectedHarnessLimitation,
                ));
                None
            } else {
                Some(HarnessReadiness::Ready)
            }
        }
        "not-yet-established" => {
            if wire.limitations.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnexpectedHarnessLimitation,
                ));
                None
            } else {
                Some(HarnessReadiness::NotYetEstablished)
            }
        }
        "not-ready" => {
            let Some(limitations) = wire.limitations else {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::MissingHarnessLimitation,
                ));
                return None;
            };
            if limitations.is_empty() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::MissingHarnessLimitation,
                ));
                return None;
            }
            let mut parsed = Vec::new();
            let mut kinds = BTreeSet::new();
            let mut valid = true;
            for limitation in limitations {
                let Some(kind) = HarnessLimitationKind::parse(&limitation.kind) else {
                    valid = false;
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::UnknownHarnessLimitation {
                            value: limitation.kind,
                        },
                    ));
                    continue;
                };
                let Some(reason) = required_reason(
                    limitation.reason,
                    "harness.limitations.reason",
                    subject,
                    diagnostics,
                ) else {
                    valid = false;
                    continue;
                };
                let candidate = HarnessLimitation::validated(kind, reason);
                if !kinds.insert(kind) {
                    valid = false;
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::DuplicateHarnessLimitation {
                            value: kind.as_str().to_owned(),
                        },
                    ));
                } else {
                    parsed.push(candidate);
                }
            }
            parsed.sort();
            valid.then_some(HarnessReadiness::NotReady {
                limitations: parsed,
            })
        }
        value => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownHarnessReadiness {
                    value: value.to_owned(),
                },
            ));
            None
        }
    }
}

fn validate_environment(
    wire: EnvironmentV1,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<ExecutionEnvironmentRequirements> {
    let mut parsed = Vec::new();
    let mut keys = BTreeSet::new();
    let mut valid = true;
    for requirement in wire.requirements {
        let Some(kind) = EnvironmentRequirementKind::parse(&requirement.kind) else {
            valid = false;
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownEnvironmentRequirement {
                    value: requirement.kind,
                },
            ));
            continue;
        };
        let Ok(profile) = EnvironmentProfileId::parse(&requirement.profile) else {
            valid = false;
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::InvalidEnvironmentProfile {
                    value: requirement.profile,
                },
            ));
            continue;
        };
        let Some(reason) = required_reason(
            requirement.reason,
            "environment.requirements.reason",
            subject,
            diagnostics,
        ) else {
            valid = false;
            continue;
        };
        let key = EnvironmentRequirementKey::validated(kind, profile);
        if !keys.insert(key.clone()) {
            valid = false;
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::DuplicateEnvironmentRequirement {
                    kind: kind.as_str().to_owned(),
                    profile: key.profile().as_str().to_owned(),
                },
            ));
        } else {
            parsed.push(EnvironmentRequirement::validated(key, reason));
        }
    }
    valid.then(|| ExecutionEnvironmentRequirements::validated(parsed))
}

fn validate_expectation(
    wire: ExpectationV1,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<Expectation> {
    match wire.kind.as_str() {
        "expected-pass" => {
            if wire.reason.is_some() || wire.failure.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnexpectedExpectedFailure,
                ));
                None
            } else {
                Some(Expectation::ExpectedPass)
            }
        }
        "expected-fail" => {
            let reason = required_reason(wire.reason, "expectation.reason", subject, diagnostics);
            let failure = match wire.failure {
                Some(failure) => match ExpectedFailureClassification::parse(&failure.kind) {
                    Some(failure) => Some(failure),
                    None => {
                        diagnostics.push(ExpectedResultsDiagnostic::new(
                            subject,
                            K::UnknownExpectedFailure {
                                value: failure.kind,
                            },
                        ));
                        None
                    }
                },
                None => {
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::MissingExpectedFailure,
                    ));
                    None
                }
            };
            match (reason, failure) {
                (Some(reason), Some(failure)) => {
                    Some(Expectation::ExpectedFail { failure, reason })
                }
                _ => None,
            }
        }
        value => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownExpectation {
                    value: value.to_owned(),
                },
            ));
            None
        }
    }
}

fn validate_stability(
    wire: StabilityV1,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<Stability> {
    match wire.state.as_str() {
        "stable" => {
            if wire.reason.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::ForbiddenField {
                        field: "stability.reason",
                        classification: "stable",
                    },
                ));
                None
            } else {
                Some(Stability::Stable)
            }
        }
        "not-yet-established" => {
            if wire.reason.is_some() {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::ForbiddenField {
                        field: "stability.reason",
                        classification: "not-yet-established stability",
                    },
                ));
                None
            } else {
                Some(Stability::NotYetEstablished)
            }
        }
        "flaky" => required_reason(wire.reason, "stability.reason", subject, diagnostics)
            .map(|reason| Stability::Flaky { reason }),
        value => {
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownStability {
                    value: value.to_owned(),
                },
            ));
            None
        }
    }
}

fn validate_lanes(
    wires: Vec<LaneExclusionV1>,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Option<Vec<LaneExclusion>> {
    let mut parsed = Vec::new();
    let mut policies = BTreeSet::new();
    let mut valid = true;
    for wire in wires {
        let Some(policy) = LanePolicyScope::parse(&wire.policy) else {
            valid = false;
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::UnknownLanePolicy { value: wire.policy },
            ));
            continue;
        };
        let Some(reason) =
            required_reason(wire.reason, "lane_exclusions.reason", subject, diagnostics)
        else {
            valid = false;
            continue;
        };
        if !policies.insert(policy) {
            valid = false;
            diagnostics.push(ExpectedResultsDiagnostic::new(
                subject,
                K::DuplicateLanePolicy {
                    value: policy.as_str().to_owned(),
                },
            ));
        } else {
            parsed.push(LaneExclusion::validated(policy, reason));
        }
    }
    parsed.sort();
    valid.then_some(parsed)
}

fn validate_references(
    repository_root: &Path,
    wires: Vec<ReferenceV1>,
    subject: &str,
    diagnostics: &mut Vec<ExpectedResultsDiagnostic>,
) -> Vec<BorrowserReference> {
    let mut parsed = Vec::new();
    let mut seen = BTreeSet::new();
    for wire in wires {
        let reference = match wire.kind.as_str() {
            "documentation" => match (wire.path, wire.issue) {
                (Some(path), None) if validate_documentation_path(repository_root, &path) => {
                    Some(BorrowserReference::Documentation { path })
                }
                (Some(path), None) => {
                    let kind = if path_is_safe_documentation(&path) {
                        K::DocumentationPathNotRegularFile { value: path }
                    } else {
                        K::InvalidDocumentationPath { value: path }
                    };
                    diagnostics.push(ExpectedResultsDiagnostic::new(subject, kind));
                    None
                }
                _ => {
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::InvalidReferenceShape {
                            kind: "documentation".to_owned(),
                        },
                    ));
                    None
                }
            },
            "tracking-issue" => match (wire.path, wire.issue) {
                (None, Some(issue)) if issue > 0 => {
                    Some(BorrowserReference::TrackingIssue { issue })
                }
                _ => {
                    diagnostics.push(ExpectedResultsDiagnostic::new(
                        subject,
                        K::InvalidReferenceShape {
                            kind: "tracking-issue".to_owned(),
                        },
                    ));
                    None
                }
            },
            value => {
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::UnknownReferenceKind {
                        value: value.to_owned(),
                    },
                ));
                None
            }
        };
        if let Some(reference) = reference {
            if !seen.insert(reference.clone()) {
                let value = match &reference {
                    BorrowserReference::Documentation { path } => {
                        format!("documentation:{path}")
                    }
                    BorrowserReference::TrackingIssue { issue } => {
                        format!("tracking-issue:{issue}")
                    }
                };
                diagnostics.push(ExpectedResultsDiagnostic::new(
                    subject,
                    K::DuplicateReference { value },
                ));
            } else {
                parsed.push(reference);
            }
        }
    }
    parsed.sort();
    parsed
}

fn validate_documentation_path(repository_root: &Path, value: &str) -> bool {
    if !path_is_safe_documentation(value) {
        return false;
    }
    let mut current = repository_root.to_path_buf();
    for component in value.split('/') {
        current.push(component);
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() {
            return false;
        }
    }
    fs::symlink_metadata(current)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn path_is_safe_documentation(value: &str) -> bool {
    if value.contains('\\') || !value.starts_with("docs/") || !value.ends_with(".md") {
        return false;
    }
    let components = value.split('/').collect::<Vec<_>>();
    components.len() >= 2
        && components
            .iter()
            .all(|component| PortablePathComponent::parse(component).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InventoryRepository, discover_inventory};

    #[test]
    fn repository_seed_model_preserves_evidenced_internal_semantics() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repository_root = crate_root.parent().unwrap().parent().unwrap();
        let inventory = discover_inventory(&InventoryRepository::new(
            repository_root,
            repository_root.join("tests/conformance/fixtures"),
        ))
        .expect("repository inventory");
        let results =
            load_expected_results(repository_root, &inventory).expect("expected results registry");

        assert_eq!(results.records().len(), 25);
        let classified_ids = results
            .records()
            .iter()
            .filter_map(|record| {
                let Classification::Classified(metadata) = record.classification() else {
                    return None;
                };
                match record.id().as_str() {
                    "html-tree-construction-repeated-body-unavailable" => {
                        let EngineCapabilityAvailability::Unavailable { missing } =
                            metadata.engine()
                        else {
                            panic!("repeated-body case remains capability-unavailable")
                        };
                        assert_eq!(missing.len(), 1);
                        assert_eq!(
                            missing[0].feature().map(CapabilityFeatureId::as_str),
                            Some("merge-attributes-into-existing-body-element")
                        );
                        assert!(matches!(metadata.harness(), HarnessReadiness::Ready));
                    }
                    "layout-reference-grid-unavailable" => {
                        let EngineCapabilityAvailability::Unavailable { missing } =
                            metadata.engine()
                        else {
                            panic!("grid reference remains capability-unavailable")
                        };
                        assert_eq!(missing.len(), 1);
                        assert_eq!(
                            missing[0].feature().map(CapabilityFeatureId::as_str),
                            Some("css-grid")
                        );
                        assert!(matches!(metadata.harness(), HarnessReadiness::Ready));
                    }
                    _ => {
                        assert!(matches!(
                            metadata.engine(),
                            EngineCapabilityAvailability::Available
                        ));
                        assert!(matches!(metadata.harness(), HarnessReadiness::Ready));
                    }
                }
                assert!(metadata.environment().requirements().is_empty());
                assert!(matches!(metadata.expectation(), Expectation::ExpectedPass));
                if matches!(
                    record.id().as_str(),
                    "layout-geometry-basic-block-flow"
                        | "paint-layering-positioned-order"
                        | "paint-operations-basic-background"
                        | "paint-semantic-artifact-ac7"
                        | "layout-reference-equivalent-simple"
                        | "paint-reference-equivalent-cascade"
                        | "paint-reference-intentional-mismatch"
                        | "paint-semantic-reference-basic"
                ) {
                    assert!(matches!(metadata.stability(), Stability::Stable));
                } else {
                    assert!(matches!(metadata.stability(), Stability::NotYetEstablished));
                }
                assert!(metadata.lane_exclusions().is_empty());
                Some(record.id().as_str())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            classified_ids,
            BTreeSet::from([
                "computed-style-basic-author-rule",
                "css-cascade-basic-author-rule",
                "css-inheritance-wide-keywords",
                "css-parsing-basic-stylesheet",
                "css-selector-matching-parser-dom",
                "css-selector-specificity-list",
                "css-selector-parsing-basic-list",
                "dom-tree-basic-document",
                "dom-tree-representative-static-document",
                "html-tokenizer-basic-document",
                "html-tokenizer-malformed-eof",
                "html-tree-construction-basic-document",
                "html-tree-construction-malformed-recovery",
                "html-tree-construction-repeated-body-unavailable",
                "layout-geometry-basic-block-flow",
                "layout-reference-equivalent-simple",
                "layout-reference-grid-unavailable",
                "paint-layering-positioned-order",
                "paint-operations-basic-background",
                "paint-reference-equivalent-cascade",
                "paint-reference-intentional-mismatch",
                "paint-semantic-artifact-ac7",
                "paint-semantic-reference-basic",
                "wpt-derived-body-background-display-none",
            ])
        );

        let unclassified_ids = results
            .records()
            .iter()
            .filter_map(|record| {
                matches!(
                    record.classification(),
                    Classification::NotYetClassified { .. }
                )
                .then_some(record.id().as_str())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            unclassified_ids,
            BTreeSet::from(["browser-controlled-static-page-basic"])
        );
    }
}
