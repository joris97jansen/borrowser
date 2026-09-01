use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::fixture::{
    RenderingDocumentInput, RenderingInputLimits, RenderingInputLoadError, RenderingInputProblem,
    WireInput, ensure_regular_rendering_input, load_rendering_document,
    read_bounded_rendering_input,
};
use crate::{
    AvailableWidthCssPx, RenderingExecutionVariantId, RenderingObservationOwner,
    RenderingObservationProfile, SyntheticTextMetricsV1,
};

pub const PAIRED_RENDERING_FIXTURE_FORMAT_V1: &str = "borrowser-paired-rendering-fixture-v1";
pub const PAIRED_RENDERING_COMBINED_HTML_BYTES_V1: usize = 8 * 1024 * 1024;
pub const PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1: usize = 16 * 1024 * 1024;
pub const PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1: usize = 64;
pub const PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairedRenderingFixtureLimits {
    input: RenderingInputLimits,
    observation_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairedRenderingFixtureLimitConfigurationError {
    ZeroDescriptorLimit,
    ZeroObservationLimit,
    ObservationLimitExceedsCumulativeLimit {
        configured: usize,
        cumulative_maximum: usize,
    },
}

impl std::fmt::Display for PairedRenderingFixtureLimitConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDescriptorLimit => {
                formatter.write_str("paired rendering descriptor byte limit must be non-zero")
            }
            Self::ZeroObservationLimit => {
                formatter.write_str("paired rendering observation byte limit must be non-zero")
            }
            Self::ObservationLimitExceedsCumulativeLimit {
                configured,
                cumulative_maximum,
            } => write!(
                formatter,
                "paired rendering observation byte limit {configured} exceeds the V1 per-side cumulative maximum {cumulative_maximum}"
            ),
        }
    }
}

impl std::error::Error for PairedRenderingFixtureLimitConfigurationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairedRenderingFixtureProblem {
    DescriptorMustBeFixtureToml,
    WrongFormat,
    DescriptorTooLarge {
        actual: usize,
        maximum: usize,
    },
    InvalidPortablePath {
        path: String,
    },
    NonRegularOrSymlink {
        path: String,
    },
    NonUtf8 {
        path: String,
    },
    DuplicateReferencedPath {
        path: String,
    },
    InputTooLarge {
        path: String,
        actual: usize,
        maximum: usize,
    },
    CombinedStylesheetBytesExceeded {
        actual: usize,
        maximum: usize,
    },
    CombinedHtmlBytesExceeded {
        actual: usize,
        maximum: usize,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    StorageAllocation {
        resource: &'static str,
    },
    TooManyStylesheets {
        actual: usize,
        maximum: usize,
    },
    TooManySupportPaths {
        actual: usize,
        maximum: usize,
    },
    TooManyVariants {
        actual: usize,
        maximum: usize,
    },
    TooManyProfiles {
        actual: usize,
        maximum: usize,
    },
    EmptyProfiles,
    EmptyVariants,
    DuplicateProfile {
        profile: &'static str,
    },
    OwnerProfileMismatch {
        profile: &'static str,
    },
    InvalidWidth {
        value: u32,
    },
    DuplicateVariant {
        width: u32,
    },
    DuplicateStylesheetOrder {
        order: u32,
    },
    NonMonotonicStylesheetOrder {
        previous: u32,
        current: u32,
    },
    DuplicateStylesheetSource {
        source: u32,
    },
    MultipleUserAgentStylesheets,
    UserAgentSourceMustBeZero,
    StylesheetNamespaceRequired,
    StylesheetNamespaceForbidden,
}

impl PairedRenderingFixtureProblem {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::DescriptorMustBeFixtureToml => "descriptor-must-be-fixture-toml",
            Self::WrongFormat => "wrong-format",
            Self::DescriptorTooLarge { .. } => "descriptor-too-large",
            Self::InvalidPortablePath { .. } => "invalid-portable-path",
            Self::NonRegularOrSymlink { .. } => "non-regular-or-symlink",
            Self::NonUtf8 { .. } => "non-utf8",
            Self::DuplicateReferencedPath { .. } => "duplicate-referenced-path",
            Self::InputTooLarge { .. } => "input-too-large",
            Self::CombinedStylesheetBytesExceeded { .. } => "combined-stylesheet-bytes-exceeded",
            Self::CombinedHtmlBytesExceeded { .. } => "combined-html-bytes-exceeded",
            Self::ArithmeticOverflow { .. } => "arithmetic-overflow",
            Self::StorageAllocation { .. } => "storage-allocation",
            Self::TooManyStylesheets { .. } => "too-many-stylesheets",
            Self::TooManySupportPaths { .. } => "too-many-support-paths",
            Self::TooManyVariants { .. } => "too-many-variants",
            Self::TooManyProfiles { .. } => "too-many-profiles",
            Self::EmptyProfiles => "empty-profiles",
            Self::EmptyVariants => "empty-variants",
            Self::DuplicateProfile { .. } => "duplicate-profile",
            Self::OwnerProfileMismatch { .. } => "owner-profile-mismatch",
            Self::InvalidWidth { .. } => "invalid-width",
            Self::DuplicateVariant { .. } => "duplicate-variant",
            Self::DuplicateStylesheetOrder { .. } => "duplicate-stylesheet-order",
            Self::NonMonotonicStylesheetOrder { .. } => "non-monotonic-stylesheet-order",
            Self::DuplicateStylesheetSource { .. } => "duplicate-stylesheet-source",
            Self::MultipleUserAgentStylesheets => "multiple-user-agent-stylesheets",
            Self::UserAgentSourceMustBeZero => "user-agent-source-must-be-zero",
            Self::StylesheetNamespaceRequired => "stylesheet-namespace-required",
            Self::StylesheetNamespaceForbidden => "stylesheet-namespace-forbidden",
        }
    }
}

impl std::fmt::Display for PairedRenderingFixtureProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DescriptorMustBeFixtureToml => {
                formatter.write_str("descriptor entry must be named fixture.toml")
            }
            Self::WrongFormat => formatter.write_str("unsupported paired rendering fixture format"),
            Self::DescriptorTooLarge { actual, maximum } => write!(
                formatter,
                "descriptor has {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::InvalidPortablePath { path } => write!(
                formatter,
                "path is not a portable package-relative path: {path}"
            ),
            Self::NonRegularOrSymlink { path } => {
                write!(formatter, "path is not a regular non-symlink file: {path}")
            }
            Self::NonUtf8 { path } => write!(formatter, "file is not UTF-8: {path}"),
            Self::DuplicateReferencedPath { path } => {
                write!(formatter, "package references path more than once: {path}")
            }
            Self::InputTooLarge {
                path,
                actual,
                maximum,
            } => write!(
                formatter,
                "input {path} has {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::CombinedStylesheetBytesExceeded { actual, maximum } => write!(
                formatter,
                "combined paired stylesheet input is {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::CombinedHtmlBytesExceeded { actual, maximum } => write!(
                formatter,
                "combined paired HTML input is {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(
                    formatter,
                    "arithmetic overflow while accounting for {resource}"
                )
            }
            Self::StorageAllocation { resource } => {
                write!(formatter, "allocation failed for {resource}")
            }
            Self::TooManyStylesheets { actual, maximum } => write!(
                formatter,
                "paired document has {actual} stylesheets, exceeding maximum {maximum}"
            ),
            Self::TooManySupportPaths { actual, maximum } => write!(
                formatter,
                "paired rendering package has {actual} support paths, exceeding V1 maximum {maximum}"
            ),
            Self::TooManyVariants { actual, maximum } => write!(
                formatter,
                "paired fixture has {actual} execution variants, exceeding maximum {maximum}"
            ),
            Self::TooManyProfiles { actual, maximum } => write!(
                formatter,
                "paired fixture has {actual} profiles, exceeding maximum {maximum}"
            ),
            Self::EmptyProfiles => formatter.write_str("profile set must be non-empty"),
            Self::EmptyVariants => formatter.write_str("variant set must be non-empty"),
            Self::DuplicateProfile { profile } => {
                write!(formatter, "profile is selected more than once: {profile}")
            }
            Self::OwnerProfileMismatch { profile } => write!(
                formatter,
                "profile does not belong to the fixture owner: {profile}"
            ),
            Self::InvalidWidth { value } => write!(
                formatter,
                "available width {value} is outside the supported V1 range"
            ),
            Self::DuplicateVariant { width } => write!(
                formatter,
                "execution variant is duplicated at available width {width}"
            ),
            Self::DuplicateStylesheetOrder { order } => {
                write!(formatter, "stylesheet order is duplicated: {order}")
            }
            Self::NonMonotonicStylesheetOrder { previous, current } => write!(
                formatter,
                "stylesheet order is not strictly increasing: {previous} then {current}"
            ),
            Self::DuplicateStylesheetSource { source } => {
                write!(formatter, "stylesheet source is duplicated: {source}")
            }
            Self::MultipleUserAgentStylesheets => {
                formatter.write_str("V1 permits at most one user-agent stylesheet")
            }
            Self::UserAgentSourceMustBeZero => {
                formatter.write_str("user-agent stylesheet source must be 0")
            }
            Self::StylesheetNamespaceRequired => {
                formatter.write_str("user-agent stylesheet namespace is required")
            }
            Self::StylesheetNamespaceForbidden => {
                formatter.write_str("user and author stylesheets must not declare a namespace")
            }
        }
    }
}

impl std::error::Error for PairedRenderingFixtureProblem {}

#[derive(Debug)]
pub enum PairedRenderingFixtureLoadError {
    Io {
        path: String,
        error: std::io::Error,
    },
    Parse {
        path: String,
        error: toml::de::Error,
    },
    Invalid(PairedRenderingFixtureProblem),
}

impl PairedRenderingFixtureLoadError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::Parse { .. } => "parse",
            Self::Invalid(problem) => problem.stable_label(),
        }
    }
}

impl std::fmt::Display for PairedRenderingFixtureLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(
                    formatter,
                    "paired rendering package I/O failed for {path}: {error}"
                )
            }
            Self::Parse { path, error } => {
                write!(
                    formatter,
                    "paired rendering descriptor {path} is invalid: {error}"
                )
            }
            Self::Invalid(problem) => {
                write!(formatter, "invalid paired rendering fixture: {problem}")
            }
        }
    }
}

impl std::error::Error for PairedRenderingFixtureLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Parse { error, .. } => Some(error),
            Self::Invalid(problem) => Some(problem),
        }
    }
}

impl PairedRenderingFixtureLimits {
    pub fn try_new(
        descriptor_bytes: usize,
        observation_bytes: usize,
    ) -> Result<Self, PairedRenderingFixtureLimitConfigurationError> {
        if descriptor_bytes == 0 {
            return Err(PairedRenderingFixtureLimitConfigurationError::ZeroDescriptorLimit);
        }
        if observation_bytes == 0 {
            return Err(PairedRenderingFixtureLimitConfigurationError::ZeroObservationLimit);
        }
        if observation_bytes > PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1 {
            return Err(
                PairedRenderingFixtureLimitConfigurationError::ObservationLimitExceedsCumulativeLimit {
                    configured: observation_bytes,
                    cumulative_maximum:
                        PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
                },
            );
        }
        Ok(Self {
            input: RenderingInputLimits::v1(descriptor_bytes),
            observation_bytes,
        })
    }

    pub const fn observation_bytes(self) -> usize {
        self.observation_bytes
    }
}

#[derive(Debug)]
pub struct PairedRenderingFixturePackage {
    pub(crate) id: String,
    pub(crate) owner: RenderingObservationOwner,
    pub(crate) test: RenderingDocumentInput,
    pub(crate) reference: RenderingDocumentInput,
    pub(crate) profiles: Vec<RenderingObservationProfile>,
    pub(crate) variants: Vec<RenderingExecutionVariantId>,
    pub(crate) referenced_paths: Vec<String>,
    pub(crate) limits: PairedRenderingFixtureLimits,
}

impl PairedRenderingFixturePackage {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn owner(&self) -> RenderingObservationOwner {
        self.owner
    }

    pub fn test_input_path(&self) -> &str {
        &self.test.html_path
    }

    pub fn reference_input_path(&self) -> &str {
        &self.reference.html_path
    }

    pub fn profiles(&self) -> &[RenderingObservationProfile] {
        &self.profiles
    }

    pub fn variants(&self) -> impl Iterator<Item = PairedRenderingVariantHandle<'_>> {
        self.variants
            .iter()
            .map(|variant| PairedRenderingVariantHandle {
                package: self,
                variant,
            })
    }

    pub fn referenced_paths(&self) -> impl Iterator<Item = &str> {
        self.referenced_paths.iter().map(String::as_str)
    }
}

#[derive(Clone, Copy)]
pub struct PairedRenderingVariantHandle<'package> {
    pub(crate) package: &'package PairedRenderingFixturePackage,
    pub(crate) variant: &'package RenderingExecutionVariantId,
}

impl PairedRenderingVariantHandle<'_> {
    pub const fn id(self) -> RenderingExecutionVariantId {
        *self.variant
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePairedFixture {
    format: String,
    id: String,
    profiles: Vec<RenderingObservationProfile>,
    test: WireInput,
    reference: WireInput,
    variants: Vec<WirePairedVariant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePairedVariant {
    environment: SyntheticTextMetricsV1,
    available_width_css_px: u32,
}

pub fn load_paired_fixture_package(
    entry_path: &Path,
    owner: RenderingObservationOwner,
    limits: PairedRenderingFixtureLimits,
) -> Result<PairedRenderingFixturePackage, PairedRenderingFixtureLoadError> {
    if entry_path.file_name().and_then(|name| name.to_str()) != Some("fixture.toml") {
        return invalid(PairedRenderingFixtureProblem::DescriptorMustBeFixtureToml);
    }
    ensure_regular_rendering_input(entry_path, &entry_path.display().to_string())
        .map_err(map_input_error)?;
    let descriptor = read_bounded_rendering_input(entry_path, limits.input.descriptor_bytes)
        .map_err(|error| match error {
            RenderingInputLoadError::Invalid(RenderingInputProblem::InputTooLarge {
                actual,
                maximum,
                ..
            }) => PairedRenderingFixtureLoadError::Invalid(
                PairedRenderingFixtureProblem::DescriptorTooLarge { actual, maximum },
            ),
            other => map_input_error(other),
        })?;
    let descriptor_text = std::str::from_utf8(&descriptor).map_err(|_| {
        PairedRenderingFixtureLoadError::Invalid(PairedRenderingFixtureProblem::NonUtf8 {
            path: entry_path.display().to_string(),
        })
    })?;
    let wire: WirePairedFixture = toml::from_str(descriptor_text).map_err(|error| {
        PairedRenderingFixtureLoadError::Parse {
            path: entry_path.display().to_string(),
            error,
        }
    })?;
    if wire.format != PAIRED_RENDERING_FIXTURE_FORMAT_V1 {
        return invalid(PairedRenderingFixtureProblem::WrongFormat);
    }
    validate_dimensions(&wire, owner, limits)?;

    let package_root = entry_path.parent().ok_or({
        PairedRenderingFixtureLoadError::Invalid(
            PairedRenderingFixtureProblem::DescriptorMustBeFixtureToml,
        )
    })?;
    let mut refs = BTreeSet::new();
    let mut combined_stylesheet_bytes = 0usize;
    let test = load_rendering_document(
        package_root,
        wire.test.html,
        wire.test.stylesheets,
        &mut refs,
        limits.input.html_input_bytes,
        limits.input.stylesheet_input_bytes,
        map_input_error,
        |stylesheet_bytes| {
            account_paired_stylesheet_bytes(&mut combined_stylesheet_bytes, stylesheet_bytes)
        },
    )?;
    let reference = load_rendering_document(
        package_root,
        wire.reference.html,
        wire.reference.stylesheets,
        &mut refs,
        limits.input.html_input_bytes,
        limits.input.stylesheet_input_bytes,
        map_input_error,
        |stylesheet_bytes| {
            account_paired_stylesheet_bytes(&mut combined_stylesheet_bytes, stylesheet_bytes)
        },
    )?;
    let combined_html = paired_checked_add(
        test.html.len(),
        reference.html.len(),
        "paired HTML input bytes",
    )?;
    if combined_html > PAIRED_RENDERING_COMBINED_HTML_BYTES_V1 {
        return invalid(PairedRenderingFixtureProblem::CombinedHtmlBytesExceeded {
            actual: combined_html,
            maximum: PAIRED_RENDERING_COMBINED_HTML_BYTES_V1,
        });
    }
    let reconciled_stylesheet_bytes = paired_checked_add(
        test.stylesheet_bytes,
        reference.stylesheet_bytes,
        "paired stylesheet input bytes",
    )?;
    if reconciled_stylesheet_bytes != combined_stylesheet_bytes {
        return invalid(PairedRenderingFixtureProblem::ArithmeticOverflow {
            resource: "paired stylesheet byte reconciliation",
        });
    }

    let mut variants = wire
        .variants
        .into_iter()
        .map(|variant| {
            let width = AvailableWidthCssPx::try_new(variant.available_width_css_px).ok_or({
                PairedRenderingFixtureLoadError::Invalid(
                    PairedRenderingFixtureProblem::InvalidWidth {
                        value: variant.available_width_css_px,
                    },
                )
            })?;
            Ok(RenderingExecutionVariantId {
                environment: variant.environment,
                available_width_css_px: width,
            })
        })
        .collect::<Result<Vec<_>, PairedRenderingFixtureLoadError>>()?;
    variants.sort();
    let mut profiles = wire.profiles;
    profiles.sort();
    Ok(PairedRenderingFixturePackage {
        id: wire.id,
        owner,
        test,
        reference,
        profiles,
        variants,
        referenced_paths: refs.into_iter().collect(),
        limits,
    })
}

fn validate_dimensions(
    wire: &WirePairedFixture,
    owner: RenderingObservationOwner,
    limits: PairedRenderingFixtureLimits,
) -> Result<(), PairedRenderingFixtureLoadError> {
    if wire.profiles.is_empty() {
        return invalid(PairedRenderingFixtureProblem::EmptyProfiles);
    }
    if wire.variants.is_empty() {
        return invalid(PairedRenderingFixtureProblem::EmptyVariants);
    }
    let test_sheets = wire.test.stylesheets.len();
    let reference_sheets = wire.reference.stylesheets.len();
    if test_sheets > limits.input.stylesheet_count {
        return invalid(PairedRenderingFixtureProblem::TooManyStylesheets {
            actual: test_sheets,
            maximum: limits.input.stylesheet_count,
        });
    }
    if reference_sheets > limits.input.stylesheet_count {
        return invalid(PairedRenderingFixtureProblem::TooManyStylesheets {
            actual: reference_sheets,
            maximum: limits.input.stylesheet_count,
        });
    }
    let combined_sheets =
        paired_checked_add(test_sheets, reference_sheets, "paired stylesheet count")?;
    if combined_sheets > PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1 {
        return invalid(PairedRenderingFixtureProblem::TooManySupportPaths {
            actual: combined_sheets,
            maximum: PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1,
        });
    }
    if wire.profiles.len() > limits.input.selected_profile_count {
        return invalid(PairedRenderingFixtureProblem::TooManyProfiles {
            actual: wire.profiles.len(),
            maximum: limits.input.selected_profile_count,
        });
    }
    if wire.variants.len() > limits.input.variant_count {
        return invalid(PairedRenderingFixtureProblem::TooManyVariants {
            actual: wire.variants.len(),
            maximum: limits.input.variant_count,
        });
    }
    let mut profiles = BTreeSet::new();
    for profile in &wire.profiles {
        if profile.owner() != owner {
            return invalid(PairedRenderingFixtureProblem::OwnerProfileMismatch {
                profile: profile.stable_label(),
            });
        }
        if !profiles.insert(*profile) {
            return invalid(PairedRenderingFixtureProblem::DuplicateProfile {
                profile: profile.stable_label(),
            });
        }
    }
    let mut variants = BTreeSet::new();
    for variant in &wire.variants {
        if !variants.insert((variant.environment, variant.available_width_css_px)) {
            return invalid(PairedRenderingFixtureProblem::DuplicateVariant {
                width: variant.available_width_css_px,
            });
        }
    }
    Ok(())
}

fn paired_checked_add(
    current: usize,
    additional: usize,
    resource: &'static str,
) -> Result<usize, PairedRenderingFixtureLoadError> {
    current.checked_add(additional).ok_or({
        PairedRenderingFixtureLoadError::Invalid(
            PairedRenderingFixtureProblem::ArithmeticOverflow { resource },
        )
    })
}

fn account_paired_stylesheet_bytes(
    cumulative: &mut usize,
    stylesheet_bytes: usize,
) -> Result<(), PairedRenderingFixtureLoadError> {
    let actual = paired_checked_add(
        *cumulative,
        stylesheet_bytes,
        "combined paired stylesheet input bytes",
    )?;
    if actual > PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1 {
        return invalid(
            PairedRenderingFixtureProblem::CombinedStylesheetBytesExceeded {
                actual,
                maximum: PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1,
            },
        );
    }
    *cumulative = actual;
    Ok(())
}

fn map_input_error(error: RenderingInputLoadError) -> PairedRenderingFixtureLoadError {
    match error {
        RenderingInputLoadError::Io { path, error } => {
            PairedRenderingFixtureLoadError::Io { path, error }
        }
        RenderingInputLoadError::Invalid(problem) => {
            let problem = match problem {
                RenderingInputProblem::InvalidPortablePath { path } => {
                    PairedRenderingFixtureProblem::InvalidPortablePath { path }
                }
                RenderingInputProblem::NonRegularOrSymlink { path } => {
                    PairedRenderingFixtureProblem::NonRegularOrSymlink { path }
                }
                RenderingInputProblem::NonUtf8 { path } => {
                    PairedRenderingFixtureProblem::NonUtf8 { path }
                }
                RenderingInputProblem::DuplicateReferencedPath { path } => {
                    PairedRenderingFixtureProblem::DuplicateReferencedPath { path }
                }
                RenderingInputProblem::InputTooLarge {
                    path,
                    actual,
                    maximum,
                } => PairedRenderingFixtureProblem::InputTooLarge {
                    path,
                    actual,
                    maximum,
                },
                RenderingInputProblem::ArithmeticOverflow { resource } => {
                    PairedRenderingFixtureProblem::ArithmeticOverflow { resource }
                }
                RenderingInputProblem::StorageAllocation { resource } => {
                    PairedRenderingFixtureProblem::StorageAllocation { resource }
                }
                RenderingInputProblem::DuplicateStylesheetOrder { order } => {
                    PairedRenderingFixtureProblem::DuplicateStylesheetOrder { order }
                }
                RenderingInputProblem::NonMonotonicStylesheetOrder { previous, current } => {
                    PairedRenderingFixtureProblem::NonMonotonicStylesheetOrder { previous, current }
                }
                RenderingInputProblem::DuplicateStylesheetSource { source } => {
                    PairedRenderingFixtureProblem::DuplicateStylesheetSource { source }
                }
                RenderingInputProblem::MultipleUserAgentStylesheets => {
                    PairedRenderingFixtureProblem::MultipleUserAgentStylesheets
                }
                RenderingInputProblem::UserAgentSourceMustBeZero => {
                    PairedRenderingFixtureProblem::UserAgentSourceMustBeZero
                }
                RenderingInputProblem::StylesheetNamespaceRequired => {
                    PairedRenderingFixtureProblem::StylesheetNamespaceRequired
                }
                RenderingInputProblem::StylesheetNamespaceForbidden => {
                    PairedRenderingFixtureProblem::StylesheetNamespaceForbidden
                }
            };
            PairedRenderingFixtureLoadError::Invalid(problem)
        }
    }
}

fn invalid<T>(
    problem: PairedRenderingFixtureProblem,
) -> Result<T, PairedRenderingFixtureLoadError> {
    Err(PairedRenderingFixtureLoadError::Invalid(problem))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PaintObservationProfile, RenderingObservationProfile};

    fn package(descriptor: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::write(temporary.path().join("fixture.toml"), descriptor).unwrap();
        for (path, contents) in files {
            let path = temporary.path().join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(path, contents).unwrap();
        }
        temporary
    }

    fn limits() -> PairedRenderingFixtureLimits {
        PairedRenderingFixtureLimits::try_new(64 * 1024, 8 * 1024 * 1024).unwrap()
    }

    #[test]
    fn paired_capture_limit_accepts_exact_per_side_boundary_and_rejects_plus_one() {
        assert_eq!(
            PairedRenderingFixtureLimits::try_new(0, 1),
            Err(PairedRenderingFixtureLimitConfigurationError::ZeroDescriptorLimit)
        );
        assert_eq!(
            PairedRenderingFixtureLimits::try_new(1, 0),
            Err(PairedRenderingFixtureLimitConfigurationError::ZeroObservationLimit)
        );
        assert!(
            PairedRenderingFixtureLimits::try_new(
                64 * 1024,
                PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
            )
            .is_ok()
        );
        assert_eq!(
            PairedRenderingFixtureLimits::try_new(
                64 * 1024,
                PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1 + 1,
            ),
            Err(
                PairedRenderingFixtureLimitConfigurationError::ObservationLimitExceedsCumulativeLimit {
                    configured:
                        PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1 + 1,
                    cumulative_maximum:
                        PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
                }
            )
        );
    }

    #[test]
    fn paired_only_failures_have_closed_deterministic_labels() {
        let support = PairedRenderingFixtureProblem::TooManySupportPaths {
            actual: 65,
            maximum: PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1,
        };
        assert_eq!(support.stable_label(), "too-many-support-paths");
        assert_eq!(
            PairedRenderingFixtureLoadError::Invalid(support).stable_label(),
            "too-many-support-paths"
        );
        let mut cumulative = PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1;
        let combined = account_paired_stylesheet_bytes(&mut cumulative, 1).unwrap_err();
        assert!(matches!(
            &combined,
            PairedRenderingFixtureLoadError::Invalid(
                PairedRenderingFixtureProblem::CombinedStylesheetBytesExceeded {
                    actual,
                    maximum: PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1,
                }
            ) if *actual == PAIRED_RENDERING_COMBINED_STYLESHEET_BYTES_V1 + 1
        ));
        assert_eq!(
            combined.stable_label(),
            "combined-stylesheet-bytes-exceeded"
        );
    }

    fn descriptor(profiles: &str, variants: &str, test_sheets: &str) -> String {
        descriptor_with_stylesheets(
            profiles,
            variants,
            test_sheets,
            "{ path = \"reference.css\", origin = \"author\", order = 0, source = 0 }",
        )
    }

    fn descriptor_with_stylesheets(
        profiles: &str,
        variants: &str,
        test_sheets: &str,
        reference_sheets: &str,
    ) -> String {
        format!(
            r#"format = "borrowser-paired-rendering-fixture-v1"
id = "paired-loader-test"
profiles = [{profiles}]
[test]
html = "test.html"
stylesheets = [{test_sheets}]
[reference]
html = "reference.html"
stylesheets = [{reference_sheets}]
{variants}
"#
        )
    }

    const FILES: &[(&str, &str)] = &[
        ("test.html", "<!doctype html><div></div>"),
        ("reference.html", "<!doctype html><div></div>"),
        ("test.css", "div { display: block; }"),
        ("second.css", "div { color: red; }"),
        ("reference.css", "div { display: block; }"),
    ];

    #[test]
    fn validated_profiles_and_variants_use_typed_order() {
        let descriptor = descriptor(
            "\"paint-operations\", \"paint-order\"",
            concat!(
                "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 640\n",
                "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
            ),
            "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
        );
        let temporary = package(&descriptor, FILES);
        let loaded = load_paired_fixture_package(
            &temporary.path().join("fixture.toml"),
            RenderingObservationOwner::Paint,
            limits(),
        )
        .unwrap();
        assert_eq!(
            loaded.profiles(),
            &[
                RenderingObservationProfile::Paint(PaintObservationProfile::PaintOrder),
                RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations),
            ]
        );
        assert_eq!(
            loaded
                .variants()
                .map(|variant| variant.id().available_width_css_px.get())
                .collect::<Vec<_>>(),
            [320, 640]
        );
    }

    #[test]
    fn duplicate_profiles_and_variants_are_invalid_before_ordering() {
        for (profiles, variants, expected) in [
            (
                "\"paint-order\", \"paint-order\"",
                "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                "profile",
            ),
            (
                "\"paint-order\"",
                concat!(
                    "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                    "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                ),
                "variant",
            ),
        ] {
            let temporary = package(
                &descriptor(
                    profiles,
                    variants,
                    "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
                ),
                FILES,
            );
            let error = load_paired_fixture_package(
                &temporary.path().join("fixture.toml"),
                RenderingObservationOwner::Paint,
                limits(),
            )
            .unwrap_err();
            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn stylesheet_coordinates_are_validated_independently_per_side() {
        let temporary = package(
            &descriptor(
                "\"paint-order\"",
                "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                concat!(
                    "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 },",
                    "{ path = \"second.css\", origin = \"user\", order = 0, source = 1 }",
                ),
            ),
            FILES,
        );
        assert!(matches!(
            load_paired_fixture_package(
                &temporary.path().join("fixture.toml"),
                RenderingObservationOwner::Paint,
                limits(),
            ),
            Err(PairedRenderingFixtureLoadError::Invalid(
                PairedRenderingFixtureProblem::DuplicateStylesheetOrder { order: 0 }
            ))
        ));
    }

    #[test]
    fn paired_stylesheets_preserve_origin_source_and_namespace_contract() {
        let valid = descriptor_with_stylesheets(
            "\"paint-order\"",
            "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
            concat!(
                "{ path = \"test.css\", origin = \"user-agent\", order = 0, source = 0, namespace = \"html\" },",
                "{ path = \"second.css\", origin = \"author\", order = 1, source = 1 }",
            ),
            "{ path = \"reference.css\", origin = \"user\", order = 0, source = 7 }",
        );
        let temporary = package(&valid, FILES);
        load_paired_fixture_package(
            &temporary.path().join("fixture.toml"),
            RenderingObservationOwner::Paint,
            limits(),
        )
        .unwrap();

        for (test_sheets, reference_sheets, expected) in [
            (
                "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
                "{ path = \"reference.css\", origin = \"user-agent\", order = 0, source = 0 }",
                PairedRenderingFixtureProblem::StylesheetNamespaceRequired,
            ),
            (
                "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
                "{ path = \"reference.css\", origin = \"author\", order = 0, source = 0, namespace = \"html\" }",
                PairedRenderingFixtureProblem::StylesheetNamespaceForbidden,
            ),
            (
                "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
                concat!(
                    "{ path = \"reference.css\", origin = \"author\", order = 0, source = 4 },",
                    "{ path = \"second.css\", origin = \"user\", order = 1, source = 4 }",
                ),
                PairedRenderingFixtureProblem::DuplicateStylesheetSource { source: 4 },
            ),
        ] {
            let invalid_descriptor = descriptor_with_stylesheets(
                "\"paint-order\"",
                "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                test_sheets,
                reference_sheets,
            );
            let temporary = package(&invalid_descriptor, FILES);
            assert!(matches!(
                load_paired_fixture_package(
                    &temporary.path().join("fixture.toml"),
                    RenderingObservationOwner::Paint,
                    limits(),
                ),
                Err(PairedRenderingFixtureLoadError::Invalid(problem)) if problem == expected
            ));
        }
    }

    fn stylesheet_list(prefix: &str, count: usize) -> String {
        (0..count)
            .map(|index| {
                format!(
                    "{{ path = \"{prefix}-{index}.css\", origin = \"author\", order = {index}, source = {index} }}"
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn write_stylesheets(root: &Path, prefix: &str, count: usize) {
        for index in 0..count {
            std::fs::write(root.join(format!("{prefix}-{index}.css")), "div {}\n").unwrap();
        }
    }

    #[test]
    fn paired_support_path_sublimit_accepts_64_and_rejects_65() {
        let variants = "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n";
        let exact = descriptor_with_stylesheets(
            "\"paint-order\"",
            variants,
            &stylesheet_list("test", 32),
            &stylesheet_list("reference", 32),
        );
        let temporary = package(
            &exact,
            &[
                ("test.html", "<!doctype html><div></div>"),
                ("reference.html", "<!doctype html><div></div>"),
            ],
        );
        write_stylesheets(temporary.path(), "test", 32);
        write_stylesheets(temporary.path(), "reference", 32);
        let loaded = load_paired_fixture_package(
            &temporary.path().join("fixture.toml"),
            RenderingObservationOwner::Paint,
            limits(),
        )
        .unwrap();
        assert_eq!(loaded.referenced_paths().count(), 66);

        let too_many = descriptor_with_stylesheets(
            "\"paint-order\"",
            variants,
            &stylesheet_list("test", 33),
            &stylesheet_list("reference", 32),
        );
        let temporary = package(&too_many, &[]);
        assert!(matches!(
            load_paired_fixture_package(
                &temporary.path().join("fixture.toml"),
                RenderingObservationOwner::Paint,
                limits(),
            ),
            Err(PairedRenderingFixtureLoadError::Invalid(
                PairedRenderingFixtureProblem::TooManySupportPaths {
                    actual: 65,
                    maximum: PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1,
                }
            ))
        ));
    }

    #[test]
    fn nested_parent_traversal_never_reads_an_outer_file() {
        let descriptor = descriptor(
            "\"paint-order\"",
            "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
            "{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }",
        )
        .replace("html = \"test.html\"", "html = \"../outer.html\"");
        let temporary = package(&descriptor, FILES);
        std::fs::write(
            temporary.path().parent().unwrap().join("outer.html"),
            "outer",
        )
        .unwrap();
        assert!(matches!(
            load_paired_fixture_package(
                &temporary.path().join("fixture.toml"),
                RenderingObservationOwner::Paint,
                limits(),
            ),
            Err(PairedRenderingFixtureLoadError::Invalid(
                PairedRenderingFixtureProblem::InvalidPortablePath { .. }
            ))
        ));
    }
}
