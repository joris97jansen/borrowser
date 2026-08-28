use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExpectedResultsDiagnostic {
    pub(crate) subject: String,
    pub(crate) kind: ExpectedResultsDiagnosticKind,
}

impl ExpectedResultsDiagnostic {
    pub(crate) fn new(subject: impl Into<String>, kind: ExpectedResultsDiagnosticKind) -> Self {
        Self {
            subject: subject.into(),
            kind,
        }
    }

    pub(crate) fn sort_key(&self) -> (&str, u16, String) {
        (&self.subject, self.kind.rank(), self.kind.detail_key())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExpectedResultsDiagnosticKind {
    RegistryOutsideRepository,
    RegistryNotRegularFile,
    SymlinkNotAllowed,
    RegistryTooLarge {
        observed_at_least: u64,
        maximum: u64,
    },
    ReadFailed,
    InvalidUtf8,
    MalformedToml,
    UnsupportedFormat {
        value: String,
    },
    InvalidGranularity {
        value: String,
    },
    InvalidRegistryShape,
    InvalidTestId {
        value: String,
    },
    TestIdTooLong {
        value: String,
    },
    CaseUnsafeTestId {
        value: String,
    },
    DuplicateTestId {
        value: String,
    },
    UnknownTestId {
        value: String,
    },
    MissingTestMetadata {
        value: String,
    },
    UnknownClassification {
        value: String,
    },
    MissingClassifiedField {
        field: &'static str,
    },
    ForbiddenField {
        field: &'static str,
        classification: &'static str,
    },
    InvalidReason {
        field: &'static str,
        problem: &'static str,
    },
    UnknownRequirement {
        value: String,
    },
    DuplicateRequirement {
        value: String,
    },
    ContradictoryRequirements {
        left: String,
        right: String,
    },
    UnknownEngineAvailability {
        value: String,
    },
    MissingUnavailableCapability,
    UnexpectedUnavailableCapability,
    UnknownEngineCapability {
        value: String,
    },
    MissingCapabilityFeature {
        capability: String,
    },
    UnexpectedCapabilityFeature {
        capability: String,
    },
    InvalidCapabilityFeature {
        value: String,
    },
    IrrelevantEngineCapability {
        capability: String,
        requirement: String,
    },
    DuplicateEngineCapability {
        capability: String,
    },
    UnknownHarnessReadiness {
        value: String,
    },
    MissingHarnessLimitation,
    UnexpectedHarnessLimitation,
    UnknownHarnessLimitation {
        value: String,
    },
    DuplicateHarnessLimitation {
        value: String,
    },
    UnknownEnvironmentRequirement {
        value: String,
    },
    InvalidEnvironmentProfile {
        value: String,
    },
    DuplicateEnvironmentRequirement {
        kind: String,
        profile: String,
    },
    UnknownExpectation {
        value: String,
    },
    MissingExpectedFailure,
    UnexpectedExpectedFailure,
    UnknownExpectedFailure {
        value: String,
    },
    UnknownStability {
        value: String,
    },
    UnknownLanePolicy {
        value: String,
    },
    DuplicateLanePolicy {
        value: String,
    },
    UnknownReferenceKind {
        value: String,
    },
    InvalidReferenceShape {
        kind: String,
    },
    DuplicateReference {
        value: String,
    },
    InvalidDocumentationPath {
        value: String,
    },
    DocumentationPathNotRegularFile {
        value: String,
    },
}

impl ExpectedResultsDiagnosticKind {
    pub(crate) fn rank(&self) -> u16 {
        match self {
            Self::RegistryOutsideRepository
            | Self::RegistryNotRegularFile
            | Self::SymlinkNotAllowed => 10,
            Self::RegistryTooLarge { .. } | Self::ReadFailed | Self::InvalidUtf8 => 20,
            Self::MalformedToml
            | Self::UnsupportedFormat { .. }
            | Self::InvalidGranularity { .. }
            | Self::InvalidRegistryShape => 30,
            Self::InvalidTestId { .. }
            | Self::TestIdTooLong { .. }
            | Self::CaseUnsafeTestId { .. } => 40,
            Self::UnknownClassification { .. }
            | Self::MissingClassifiedField { .. }
            | Self::ForbiddenField { .. }
            | Self::InvalidReason { .. } => 50,
            Self::UnknownRequirement { .. }
            | Self::DuplicateRequirement { .. }
            | Self::ContradictoryRequirements { .. } => 60,
            Self::UnknownEngineAvailability { .. }
            | Self::MissingUnavailableCapability
            | Self::UnexpectedUnavailableCapability
            | Self::UnknownEngineCapability { .. }
            | Self::MissingCapabilityFeature { .. }
            | Self::UnexpectedCapabilityFeature { .. }
            | Self::InvalidCapabilityFeature { .. }
            | Self::IrrelevantEngineCapability { .. }
            | Self::DuplicateEngineCapability { .. } => 70,
            Self::UnknownHarnessReadiness { .. }
            | Self::MissingHarnessLimitation
            | Self::UnexpectedHarnessLimitation
            | Self::UnknownHarnessLimitation { .. }
            | Self::DuplicateHarnessLimitation { .. } => 80,
            Self::UnknownEnvironmentRequirement { .. }
            | Self::InvalidEnvironmentProfile { .. }
            | Self::DuplicateEnvironmentRequirement { .. } => 90,
            Self::UnknownExpectation { .. }
            | Self::MissingExpectedFailure
            | Self::UnexpectedExpectedFailure
            | Self::UnknownExpectedFailure { .. } => 100,
            Self::UnknownStability { .. } => 110,
            Self::UnknownLanePolicy { .. } | Self::DuplicateLanePolicy { .. } => 120,
            Self::UnknownReferenceKind { .. }
            | Self::InvalidReferenceShape { .. }
            | Self::DuplicateReference { .. }
            | Self::InvalidDocumentationPath { .. }
            | Self::DocumentationPathNotRegularFile { .. } => 130,
            Self::DuplicateTestId { .. }
            | Self::UnknownTestId { .. }
            | Self::MissingTestMetadata { .. } => 200,
        }
    }

    pub(crate) fn detail_key(&self) -> String {
        match self {
            Self::RegistryOutsideRepository => "00".to_owned(),
            Self::RegistryNotRegularFile => "01".to_owned(),
            Self::SymlinkNotAllowed => "02".to_owned(),
            Self::RegistryTooLarge {
                observed_at_least,
                maximum,
            } => format!("00:{observed_at_least:020}:{maximum:020}"),
            Self::ReadFailed => "01".to_owned(),
            Self::InvalidUtf8 => "02".to_owned(),
            Self::MalformedToml => "00".to_owned(),
            Self::UnsupportedFormat { value } => format!("01:{value}"),
            Self::InvalidGranularity { value } => format!("02:{value}"),
            Self::InvalidRegistryShape => "03".to_owned(),
            Self::InvalidTestId { value } => format!("00:{value}"),
            Self::TestIdTooLong { value } => format!("01:{value}"),
            Self::CaseUnsafeTestId { value } => format!("02:{value}"),
            Self::UnknownClassification { value } => format!("00:{value}"),
            Self::MissingClassifiedField { field } => format!("01:{field}"),
            Self::ForbiddenField {
                field,
                classification,
            } => format!("02:{classification}:{field}"),
            Self::InvalidReason { field, problem } => format!("03:{field}:{problem}"),
            Self::UnknownRequirement { value } => format!("00:{value}"),
            Self::DuplicateRequirement { value } => format!("01:{value}"),
            Self::ContradictoryRequirements { left, right } => {
                format!("02:{left}:{right}")
            }
            Self::UnknownEngineAvailability { value } => format!("00:{value}"),
            Self::MissingUnavailableCapability => "01".to_owned(),
            Self::UnexpectedUnavailableCapability => "02".to_owned(),
            Self::UnknownEngineCapability { value } => format!("03:{value}"),
            Self::MissingCapabilityFeature { capability } => format!("04:{capability}"),
            Self::UnexpectedCapabilityFeature { capability } => format!("05:{capability}"),
            Self::InvalidCapabilityFeature { value } => format!("06:{value}"),
            Self::IrrelevantEngineCapability {
                capability,
                requirement,
            } => format!("07:{capability}:{requirement}"),
            Self::DuplicateEngineCapability { capability } => format!("08:{capability}"),
            Self::UnknownHarnessReadiness { value } => format!("00:{value}"),
            Self::MissingHarnessLimitation => "01".to_owned(),
            Self::UnexpectedHarnessLimitation => "02".to_owned(),
            Self::UnknownHarnessLimitation { value } => format!("03:{value}"),
            Self::DuplicateHarnessLimitation { value } => format!("04:{value}"),
            Self::UnknownEnvironmentRequirement { value } => format!("00:{value}"),
            Self::InvalidEnvironmentProfile { value } => format!("01:{value}"),
            Self::DuplicateEnvironmentRequirement { kind, profile } => {
                format!("02:{kind}:{profile}")
            }
            Self::UnknownExpectation { value } => format!("00:{value}"),
            Self::MissingExpectedFailure => "01".to_owned(),
            Self::UnexpectedExpectedFailure => "02".to_owned(),
            Self::UnknownExpectedFailure { value } => format!("03:{value}"),
            Self::UnknownStability { value } => format!("00:{value}"),
            Self::UnknownLanePolicy { value } => format!("00:{value}"),
            Self::DuplicateLanePolicy { value } => format!("01:{value}"),
            Self::UnknownReferenceKind { value } => format!("00:{value}"),
            Self::InvalidReferenceShape { kind } => format!("01:{kind}"),
            Self::DuplicateReference { value } => format!("02:{value}"),
            Self::InvalidDocumentationPath { value } => format!("03:{value}"),
            Self::DocumentationPathNotRegularFile { value } => format!("04:{value}"),
            Self::DuplicateTestId { value } => format!("00:{value}"),
            Self::UnknownTestId { value } => format!("01:{value}"),
            Self::MissingTestMetadata { value } => format!("02:{value}"),
        }
    }
}

impl fmt::Display for ExpectedResultsDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conformance expected-results {}: ", self.subject)?;
        use ExpectedResultsDiagnosticKind as K;
        match &self.kind {
            K::RegistryOutsideRepository => f.write_str("registry is outside the repository"),
            K::RegistryNotRegularFile => f.write_str("registry is not a regular file"),
            K::SymlinkNotAllowed => f.write_str("symlinked registry paths are not allowed"),
            K::RegistryTooLarge {
                observed_at_least,
                maximum,
            } => write!(
                f,
                "registry is at least {observed_at_least} bytes, above the {maximum}-byte limit"
            ),
            K::ReadFailed => f.write_str("registry read failed"),
            K::InvalidUtf8 => f.write_str("registry is not UTF-8"),
            K::MalformedToml => f.write_str("registry is malformed TOML"),
            K::UnsupportedFormat { value } => write!(f, "unsupported format '{value}'"),
            K::InvalidGranularity { value } => write!(f, "invalid granularity '{value}'"),
            K::InvalidRegistryShape => f.write_str("registry does not match the strict V1 shape"),
            K::InvalidTestId { value } => write!(f, "invalid test id '{value}'"),
            K::TestIdTooLong { value } => write!(f, "test id is too long: '{value}'"),
            K::CaseUnsafeTestId { value } => {
                write!(f, "test id must be lowercase ASCII kebab case: '{value}'")
            }
            K::DuplicateTestId { value } => write!(f, "duplicate metadata id '{value}'"),
            K::UnknownTestId { value } => write!(f, "metadata id is not discovered: '{value}'"),
            K::MissingTestMetadata { value } => {
                write!(
                    f,
                    "discovered test has no explicit metadata record: '{value}'"
                )
            }
            K::UnknownClassification { value } => write!(f, "unknown classification '{value}'"),
            K::MissingClassifiedField { field } => {
                write!(f, "classified record is missing {field}")
            }
            K::ForbiddenField {
                field,
                classification,
            } => {
                write!(f, "{field} is forbidden for {classification}")
            }
            K::InvalidReason { field, problem } => write!(f, "{field} {problem}"),
            K::UnknownRequirement { value } => write!(f, "unknown requirement '{value}'"),
            K::DuplicateRequirement { value } => write!(f, "duplicate requirement '{value}'"),
            K::ContradictoryRequirements { left, right } => {
                write!(f, "contradictory requirements '{left}' and '{right}'")
            }
            K::UnknownEngineAvailability { value } => {
                write!(f, "unknown engine availability '{value}'")
            }
            K::MissingUnavailableCapability => {
                f.write_str("unavailable engine state requires missing capabilities")
            }
            K::UnexpectedUnavailableCapability => {
                f.write_str("engine state forbids missing capabilities")
            }
            K::UnknownEngineCapability { value } => {
                write!(f, "unknown engine capability '{value}'")
            }
            K::MissingCapabilityFeature { capability } => {
                write!(f, "engine capability '{capability}' requires feature")
            }
            K::UnexpectedCapabilityFeature { capability } => {
                write!(f, "engine capability '{capability}' forbids feature")
            }
            K::InvalidCapabilityFeature { value } => {
                write!(f, "invalid capability feature '{value}'")
            }
            K::IrrelevantEngineCapability {
                capability,
                requirement,
            } => write!(
                f,
                "engine capability '{capability}' is not relevant without requirement '{requirement}'"
            ),
            K::DuplicateEngineCapability { capability } => {
                write!(f, "duplicate missing engine capability '{capability}'")
            }
            K::UnknownHarnessReadiness { value } => {
                write!(f, "unknown harness readiness '{value}'")
            }
            K::MissingHarnessLimitation => f.write_str("not-ready harness requires limitations"),
            K::UnexpectedHarnessLimitation => f.write_str("harness state forbids limitations"),
            K::UnknownHarnessLimitation { value } => {
                write!(f, "unknown harness limitation '{value}'")
            }
            K::DuplicateHarnessLimitation { value } => {
                write!(f, "duplicate harness limitation '{value}'")
            }
            K::UnknownEnvironmentRequirement { value } => {
                write!(f, "unknown environment requirement '{value}'")
            }
            K::InvalidEnvironmentProfile { value } => {
                write!(f, "invalid environment profile '{value}'")
            }
            K::DuplicateEnvironmentRequirement { kind, profile } => {
                write!(f, "duplicate environment requirement '{kind}:{profile}'")
            }
            K::UnknownExpectation { value } => write!(f, "unknown expectation '{value}'"),
            K::MissingExpectedFailure => {
                f.write_str("expected-fail requires failure classification")
            }
            K::UnexpectedExpectedFailure => {
                f.write_str("expected-pass forbids failure metadata and reason")
            }
            K::UnknownExpectedFailure { value } => write!(f, "unknown expected failure '{value}'"),
            K::UnknownStability { value } => write!(f, "unknown stability '{value}'"),
            K::UnknownLanePolicy { value } => write!(f, "unknown lane policy '{value}'"),
            K::DuplicateLanePolicy { value } => write!(f, "duplicate lane policy '{value}'"),
            K::UnknownReferenceKind { value } => write!(f, "unknown reference kind '{value}'"),
            K::InvalidReferenceShape { kind } => write!(f, "invalid '{kind}' reference shape"),
            K::DuplicateReference { value } => write!(f, "duplicate reference '{value}'"),
            K::InvalidDocumentationPath { value } => {
                write!(f, "unsafe documentation path '{value}'")
            }
            K::DocumentationPathNotRegularFile { value } => {
                write!(f, "documentation path is not a regular file: '{value}'")
            }
        }
    }
}

pub struct ExpectedResultsErrors {
    diagnostics: Vec<ExpectedResultsDiagnostic>,
}

impl ExpectedResultsErrors {
    pub(crate) fn sorted(mut diagnostics: Vec<ExpectedResultsDiagnostic>) -> Self {
        diagnostics.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        Self { diagnostics }
    }
}

impl fmt::Display for ExpectedResultsErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                f.write_str("\n")?;
            }
            write!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for ExpectedResultsErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for ExpectedResultsErrors {}
