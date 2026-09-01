use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::{
    AvailableWidthCssPx, RenderingExecutionVariantId, RenderingObservationOwner,
    RenderingObservationProfile, SyntheticTextMetricsV1,
};

pub const RENDERING_FIXTURE_FORMAT_V1: &str = "borrowser-rendering-fixture-v1";
pub const RENDERING_HTML_INPUT_BYTES_V1: usize = 4 * 1024 * 1024;
pub const RENDERING_CUMULATIVE_STYLESHEET_INPUT_BYTES_V1: usize = 16 * 1024 * 1024;
pub const RENDERING_STYLESHEET_COUNT_V1: usize = 64;
pub const RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1: usize = 32 * 1024 * 1024;
pub const RENDERING_VARIANT_COUNT_V1: usize = 16;
pub const RENDERING_SELECTED_PROFILE_COUNT_V1: usize = 5;
pub const RENDERING_EXPECTATION_PAIR_COUNT_V1: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderingFixtureLimits {
    input: RenderingInputLimits,
    cumulative_stylesheet_input_bytes: usize,
    expected_snapshot_bytes: usize,
    cumulative_observation_bytes: usize,
    cumulative_expected_snapshot_bytes: usize,
    expectation_pair_count: usize,
}

/// Limits shared by document input transport, independently of the oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderingInputLimits {
    pub(crate) descriptor_bytes: usize,
    pub(crate) html_input_bytes: usize,
    pub(crate) stylesheet_input_bytes: usize,
    pub(crate) stylesheet_count: usize,
    pub(crate) variant_count: usize,
    pub(crate) selected_profile_count: usize,
}

impl RenderingInputLimits {
    pub(crate) fn v1(descriptor_bytes: usize) -> Self {
        Self {
            descriptor_bytes,
            html_input_bytes: RENDERING_HTML_INPUT_BYTES_V1,
            stylesheet_input_bytes: css::SyntaxLimits::default().max_stylesheet_input_bytes,
            stylesheet_count: RENDERING_STYLESHEET_COUNT_V1,
            variant_count: RENDERING_VARIANT_COUNT_V1,
            selected_profile_count: RENDERING_SELECTED_PROFILE_COUNT_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingFixtureLimitConfigurationError {
    ZeroDescriptorLimit,
    ZeroExpectedSnapshotLimit,
    ExpectedSnapshotLimitExceedsCumulativeLimit {
        configured: usize,
        cumulative_maximum: usize,
    },
    ObservationLimitProductDoesNotFitPlatform {
        configured: usize,
        profile_count: usize,
    },
}

impl std::fmt::Display for RenderingFixtureLimitConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDescriptorLimit => {
                formatter.write_str("rendering descriptor byte limit must be non-zero")
            }
            Self::ZeroExpectedSnapshotLimit => {
                formatter.write_str("rendering expected-snapshot byte limit must be non-zero")
            }
            Self::ExpectedSnapshotLimitExceedsCumulativeLimit {
                configured,
                cumulative_maximum,
            } => write!(
                formatter,
                "rendering expected-snapshot byte limit {configured} exceeds the V1 cumulative maximum {cumulative_maximum}"
            ),
            Self::ObservationLimitProductDoesNotFitPlatform {
                configured,
                profile_count,
            } => write!(
                formatter,
                "rendering observation byte limit {configured} times profile count {profile_count} does not fit this platform"
            ),
        }
    }
}

impl std::error::Error for RenderingFixtureLimitConfigurationError {}

impl RenderingFixtureLimits {
    pub fn try_new(
        descriptor_bytes: usize,
        expected_snapshot_bytes: usize,
    ) -> Result<Self, RenderingFixtureLimitConfigurationError> {
        if descriptor_bytes == 0 {
            return Err(RenderingFixtureLimitConfigurationError::ZeroDescriptorLimit);
        }
        if expected_snapshot_bytes == 0 {
            return Err(RenderingFixtureLimitConfigurationError::ZeroExpectedSnapshotLimit);
        }
        if expected_snapshot_bytes > RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1 {
            return Err(
                RenderingFixtureLimitConfigurationError::ExpectedSnapshotLimitExceedsCumulativeLimit {
                    configured: expected_snapshot_bytes,
                    cumulative_maximum: RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1,
                },
            );
        }
        let cumulative_observation_bytes = expected_snapshot_bytes
            .checked_mul(RENDERING_SELECTED_PROFILE_COUNT_V1)
            .ok_or(
                RenderingFixtureLimitConfigurationError::ObservationLimitProductDoesNotFitPlatform {
                    configured: expected_snapshot_bytes,
                    profile_count: RENDERING_SELECTED_PROFILE_COUNT_V1,
                },
            )?;
        Ok(Self {
            input: RenderingInputLimits::v1(descriptor_bytes),
            cumulative_stylesheet_input_bytes: RENDERING_CUMULATIVE_STYLESHEET_INPUT_BYTES_V1,
            expected_snapshot_bytes,
            cumulative_observation_bytes,
            cumulative_expected_snapshot_bytes: RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1,
            expectation_pair_count: RENDERING_EXPECTATION_PAIR_COUNT_V1,
        })
    }

    pub const fn descriptor_bytes(self) -> usize {
        self.input.descriptor_bytes
    }

    pub const fn expected_snapshot_bytes(self) -> usize {
        self.expected_snapshot_bytes
    }

    pub const fn stylesheet_count(self) -> usize {
        self.input.stylesheet_count
    }

    pub const fn expectation_pair_count(self) -> usize {
        self.expectation_pair_count
    }

    pub(crate) const fn cumulative_observation_bytes(self) -> usize {
        self.cumulative_observation_bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderingStylesheetOrigin {
    UserAgent,
    User,
    Author,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RenderingStylesheetNamespace {
    Html,
    Svg,
    MathMl,
}

impl RenderingStylesheetNamespace {
    pub(crate) const fn production(self) -> html::ElementNamespace {
        match self {
            Self::Html => html::ElementNamespace::Html,
            Self::Svg => html::ElementNamespace::Svg,
            Self::MathMl => html::ElementNamespace::MathMl,
        }
    }
}

#[derive(Debug)]
pub struct RenderingFixturePackage {
    pub(crate) id: String,
    pub(crate) owner: RenderingObservationOwner,
    pub(crate) document: RenderingDocumentInput,
    pub(crate) profiles: Vec<RenderingObservationProfile>,
    pub(crate) variants: Vec<RenderingVariant>,
    pub(crate) referenced_paths: Vec<String>,
    pub(crate) limits: RenderingFixtureLimits,
}

impl RenderingFixturePackage {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn owner(&self) -> RenderingObservationOwner {
        self.owner
    }
    pub fn primary_input_path(&self) -> &str {
        &self.document.html_path
    }
    pub fn profiles(&self) -> &[RenderingObservationProfile] {
        &self.profiles
    }
    pub fn variants(&self) -> impl Iterator<Item = RenderingVariantHandle<'_>> {
        self.variants.iter().map(|variant| RenderingVariantHandle {
            package: self,
            variant,
        })
    }
    pub fn referenced_paths(&self) -> impl Iterator<Item = &str> {
        self.referenced_paths.iter().map(String::as_str)
    }
}

#[derive(Debug)]
pub(crate) struct RenderingDocumentInput {
    pub(crate) html: String,
    pub(crate) html_path: String,
    pub(crate) stylesheets: Vec<RenderingStylesheetInput>,
    pub(crate) stylesheet_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct RenderingStylesheetInput {
    pub source_text: String,
    pub origin: RenderingStylesheetOrigin,
    pub order: u32,
    pub source: u32,
    pub namespace: Option<RenderingStylesheetNamespace>,
}

#[derive(Debug)]
pub(crate) struct RenderingVariant {
    pub id: RenderingExecutionVariantId,
    pub expectations: Vec<RenderingExpectedPath>,
}

#[derive(Clone, Copy)]
pub struct RenderingVariantHandle<'package> {
    package: &'package RenderingFixturePackage,
    variant: &'package RenderingVariant,
}

impl RenderingVariantHandle<'_> {
    pub const fn id(self) -> RenderingExecutionVariantId {
        self.variant.id
    }
}

pub struct RenderingVariantExecution<'package> {
    pub(crate) package: &'package RenderingFixturePackage,
    pub(crate) variant: &'package RenderingVariant,
    pub(crate) expected: Vec<RenderingExpectedSnapshot>,
}

impl RenderingVariantExecution<'_> {
    pub const fn id(&self) -> RenderingExecutionVariantId {
        self.variant.id
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RenderingExpectedSnapshot {
    pub(crate) profile: RenderingObservationProfile,
    pub(crate) bytes: String,
}

#[derive(Debug)]
pub enum RenderingFixtureLoadError {
    Io {
        path: String,
        error: std::io::Error,
    },
    Parse {
        path: String,
        error: toml::de::Error,
    },
    Invalid(RenderingFixtureProblem),
}

impl RenderingFixtureLoadError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::Parse { .. } => "parse",
            Self::Invalid(_) => "invalid",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingFixtureProblem {
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
    CumulativeStylesheetBytesExceeded {
        actual: usize,
        maximum: usize,
    },
    CumulativeExpectedBytesExceeded {
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
    TooManyVariants {
        actual: usize,
        maximum: usize,
    },
    TooManyProfiles {
        actual: usize,
        maximum: usize,
    },
    TooManyExpectationPairs {
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
    ExpectationSetMismatch,
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

impl std::fmt::Display for RenderingFixtureProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DescriptorMustBeFixtureToml => {
                formatter.write_str("descriptor entry must be named fixture.toml")
            }
            Self::WrongFormat => formatter.write_str("unsupported rendering fixture format"),
            Self::DescriptorTooLarge { actual, maximum } => write!(
                formatter,
                "descriptor has {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::InvalidPortablePath { path } => {
                write!(
                    formatter,
                    "path is not a portable package-relative path: {path}"
                )
            }
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
            Self::CumulativeStylesheetBytesExceeded { actual, maximum } => write!(
                formatter,
                "cumulative stylesheet input is {actual} bytes, exceeding maximum {maximum}"
            ),
            Self::CumulativeExpectedBytesExceeded { actual, maximum } => write!(
                formatter,
                "cumulative expected snapshots are {actual} bytes, exceeding maximum {maximum}"
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
                "fixture has {actual} stylesheets, exceeding maximum {maximum}"
            ),
            Self::TooManyVariants { actual, maximum } => write!(
                formatter,
                "fixture has {actual} execution variants, exceeding maximum {maximum}"
            ),
            Self::TooManyProfiles { actual, maximum } => write!(
                formatter,
                "fixture has {actual} profiles, exceeding maximum {maximum}"
            ),
            Self::TooManyExpectationPairs { actual, maximum } => write!(
                formatter,
                "fixture has {actual} variant/profile pairs, exceeding maximum {maximum}"
            ),
            Self::EmptyProfiles => formatter.write_str("profile set must be non-empty"),
            Self::EmptyVariants => formatter.write_str("variant set must be non-empty"),
            Self::DuplicateProfile { profile } => {
                write!(formatter, "profile is selected more than once: {profile}")
            }
            Self::OwnerProfileMismatch { profile } => {
                write!(
                    formatter,
                    "profile does not belong to the fixture owner: {profile}"
                )
            }
            Self::InvalidWidth { value } => write!(
                formatter,
                "available width {value} is outside the supported V1 range"
            ),
            Self::DuplicateVariant { width } => write!(
                formatter,
                "execution variant is duplicated at available width {width}"
            ),
            Self::ExpectationSetMismatch => formatter.write_str(
                "each variant must contain exactly one expectation for every selected profile",
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

impl std::error::Error for RenderingFixtureProblem {}

impl std::fmt::Display for RenderingFixtureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(f, "rendering package I/O failed for {path}: {error}")
            }
            Self::Parse { path, error } => {
                write!(f, "rendering descriptor {path} is invalid: {error}")
            }
            Self::Invalid(problem) => write!(f, "invalid rendering fixture: {problem}"),
        }
    }
}
impl std::error::Error for RenderingFixtureLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::Parse { error, .. } => Some(error),
            Self::Invalid(problem) => Some(problem),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFixture {
    format: String,
    id: String,
    profiles: Vec<RenderingObservationProfile>,
    input: WireInput,
    variants: Vec<WireVariant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireInput {
    pub(crate) html: String,
    #[serde(default)]
    pub(crate) stylesheets: Vec<WireStylesheet>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireStylesheet {
    pub(crate) path: String,
    pub(crate) origin: RenderingStylesheetOrigin,
    pub(crate) order: u32,
    pub(crate) source: u32,
    pub(crate) namespace: Option<RenderingStylesheetNamespace>,
}

#[derive(Debug)]
pub(crate) enum RenderingInputLoadError {
    Io { path: String, error: std::io::Error },
    Invalid(RenderingInputProblem),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RenderingInputProblem {
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
    ArithmeticOverflow {
        resource: &'static str,
    },
    StorageAllocation {
        resource: &'static str,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireVariant {
    environment: SyntheticTextMetricsV1,
    available_width_css_px: u32,
    expectations: Vec<WireExpectation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireExpectation {
    profile: RenderingObservationProfile,
    snapshot: String,
}

pub fn load_fixture_package(
    entry_path: &Path,
    owner: RenderingObservationOwner,
    limits: RenderingFixtureLimits,
) -> Result<RenderingFixturePackage, RenderingFixtureLoadError> {
    if entry_path.file_name().and_then(|name| name.to_str()) != Some("fixture.toml") {
        return invalid(RenderingFixtureProblem::DescriptorMustBeFixtureToml);
    }
    ensure_regular(entry_path, &entry_path.display().to_string())?;
    let descriptor =
        read_bounded(entry_path, limits.input.descriptor_bytes).map_err(|error| match error {
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::InputTooLarge {
                actual,
                maximum,
                ..
            }) => RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::DescriptorTooLarge {
                actual,
                maximum,
            }),
            other => other,
        })?;
    let descriptor_text = std::str::from_utf8(&descriptor).map_err(|_| {
        RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::NonUtf8 {
            path: entry_path.display().to_string(),
        })
    })?;
    let wire: WireFixture =
        toml::from_str(descriptor_text).map_err(|error| RenderingFixtureLoadError::Parse {
            path: entry_path.display().to_string(),
            error,
        })?;
    if wire.format != RENDERING_FIXTURE_FORMAT_V1 {
        return invalid(RenderingFixtureProblem::WrongFormat);
    }
    validate_dimensions(&wire, owner, limits)?;
    let package_root = entry_path.parent().ok_or({
        RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::DescriptorMustBeFixtureToml)
    })?;
    let mut refs = BTreeSet::new();
    let mut cumulative_stylesheet_bytes = 0usize;
    let document = load_rendering_document(
        package_root,
        wire.input.html,
        wire.input.stylesheets,
        &mut refs,
        limits.input.html_input_bytes,
        limits.input.stylesheet_input_bytes,
        map_rendering_input_error,
        |stylesheet_bytes| {
            let cumulative = checked_add_resource(
                cumulative_stylesheet_bytes,
                stylesheet_bytes,
                "cumulative stylesheet input bytes",
            )?;
            if cumulative > limits.cumulative_stylesheet_input_bytes {
                return invalid(RenderingFixtureProblem::CumulativeStylesheetBytesExceeded {
                    actual: cumulative,
                    maximum: limits.cumulative_stylesheet_input_bytes,
                });
            }
            cumulative_stylesheet_bytes = cumulative;
            Ok(())
        },
    )?;
    let mut cumulative_expected = 0usize;
    let mut variants = Vec::new();
    for variant in wire.variants {
        let width = AvailableWidthCssPx::try_new(variant.available_width_css_px).ok_or({
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::InvalidWidth {
                value: variant.available_width_css_px,
            })
        })?;
        let mut expectations = Vec::new();
        for expectation in variant.expectations {
            let path = validate_and_register_path(package_root, &expectation.snapshot, &mut refs)?;
            let length = regular_file_length(&path, &expectation.snapshot)?;
            if length > limits.expected_snapshot_bytes {
                return invalid(RenderingFixtureProblem::InputTooLarge {
                    path: expectation.snapshot,
                    actual: length,
                    maximum: limits.expected_snapshot_bytes,
                });
            }
            cumulative_expected =
                checked_add_resource(cumulative_expected, length, "expected snapshot bytes")?;
            if cumulative_expected > limits.cumulative_expected_snapshot_bytes {
                return invalid(RenderingFixtureProblem::CumulativeExpectedBytesExceeded {
                    actual: cumulative_expected,
                    maximum: limits.cumulative_expected_snapshot_bytes,
                });
            }
            expectations.push(RenderingExpectedPath {
                profile: expectation.profile,
                path,
            });
        }
        expectations.sort_by_key(|item| item.profile);
        variants.push(RenderingVariant {
            id: RenderingExecutionVariantId {
                environment: variant.environment,
                available_width_css_px: width,
            },
            expectations,
        });
    }
    variants.sort_by_key(|variant| variant.id);
    let referenced_paths = refs.into_iter().collect();
    Ok(RenderingFixturePackage {
        id: wire.id,
        owner,
        document,
        profiles: sorted_profiles(wire.profiles),
        variants,
        referenced_paths,
        limits,
    })
}

#[derive(Debug)]
pub(crate) struct RenderingExpectedPath {
    profile: RenderingObservationProfile,
    path: PathBuf,
}

pub fn load_variant_execution(
    handle: RenderingVariantHandle<'_>,
) -> Result<RenderingVariantExecution<'_>, RenderingFixtureLoadError> {
    let mut loaded = Vec::new();
    loaded
        .try_reserve(handle.variant.expectations.len())
        .map_err(|_| {
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::StorageAllocation {
                resource: "expected snapshot storage",
            })
        })?;
    for expected in &handle.variant.expectations {
        let bytes = read_bounded(
            &expected.path,
            handle.package.limits.expected_snapshot_bytes,
        )?;
        let text = String::from_utf8(bytes).map_err(|_| {
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::NonUtf8 {
                path: expected.path.display().to_string(),
            })
        })?;
        loaded.push(RenderingExpectedSnapshot {
            profile: expected.profile,
            bytes: text,
        });
    }
    Ok(RenderingVariantExecution {
        package: handle.package,
        variant: handle.variant,
        expected: loaded,
    })
}

fn validate_dimensions(
    wire: &WireFixture,
    owner: RenderingObservationOwner,
    limits: RenderingFixtureLimits,
) -> Result<(), RenderingFixtureLoadError> {
    if wire.profiles.is_empty() {
        return invalid(RenderingFixtureProblem::EmptyProfiles);
    }
    if wire.variants.is_empty() {
        return invalid(RenderingFixtureProblem::EmptyVariants);
    }
    if wire.input.stylesheets.len() > limits.input.stylesheet_count {
        return invalid(RenderingFixtureProblem::TooManyStylesheets {
            actual: wire.input.stylesheets.len(),
            maximum: limits.input.stylesheet_count,
        });
    }
    if wire.profiles.len() > limits.input.selected_profile_count {
        return invalid(RenderingFixtureProblem::TooManyProfiles {
            actual: wire.profiles.len(),
            maximum: limits.input.selected_profile_count,
        });
    }
    if wire.variants.len() > limits.input.variant_count {
        return invalid(RenderingFixtureProblem::TooManyVariants {
            actual: wire.variants.len(),
            maximum: limits.input.variant_count,
        });
    }
    let pairs = wire
        .variants
        .len()
        .checked_mul(wire.profiles.len())
        .ok_or({
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::ArithmeticOverflow {
                resource: "expectation pairs",
            })
        })?;
    if pairs > limits.expectation_pair_count {
        return invalid(RenderingFixtureProblem::TooManyExpectationPairs {
            actual: pairs,
            maximum: limits.expectation_pair_count,
        });
    }
    let mut profiles = BTreeSet::new();
    for profile in &wire.profiles {
        if profile.owner() != owner {
            return invalid(RenderingFixtureProblem::OwnerProfileMismatch {
                profile: profile.stable_label(),
            });
        }
        if !profiles.insert(*profile) {
            return invalid(RenderingFixtureProblem::DuplicateProfile {
                profile: profile.stable_label(),
            });
        }
    }
    let mut variants = BTreeSet::new();
    for variant in &wire.variants {
        if !variants.insert((variant.environment, variant.available_width_css_px)) {
            return invalid(RenderingFixtureProblem::DuplicateVariant {
                width: variant.available_width_css_px,
            });
        }
        let actual: BTreeSet<_> = variant
            .expectations
            .iter()
            .map(|item| item.profile)
            .collect();
        if actual.len() != variant.expectations.len() || actual != profiles {
            return invalid(RenderingFixtureProblem::ExpectationSetMismatch);
        }
    }
    Ok(())
}

pub(crate) fn validate_stylesheet_coordinates(
    sheets: &[WireStylesheet],
) -> Result<(), RenderingInputLoadError> {
    let mut orders = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut previous = None;
    let mut ua = 0;
    for sheet in sheets {
        if !orders.insert(sheet.order) {
            return input_invalid(RenderingInputProblem::DuplicateStylesheetOrder {
                order: sheet.order,
            });
        }
        if let Some(previous) = previous
            && sheet.order <= previous
        {
            return input_invalid(RenderingInputProblem::NonMonotonicStylesheetOrder {
                previous,
                current: sheet.order,
            });
        }
        previous = Some(sheet.order);
        if !sources.insert(sheet.source) {
            return input_invalid(RenderingInputProblem::DuplicateStylesheetSource {
                source: sheet.source,
            });
        }
        match sheet.origin {
            RenderingStylesheetOrigin::UserAgent => {
                ua += 1;
                if ua > 1 {
                    return input_invalid(RenderingInputProblem::MultipleUserAgentStylesheets);
                }
                if sheet.source != 0 {
                    return input_invalid(RenderingInputProblem::UserAgentSourceMustBeZero);
                }
                if sheet.namespace.is_none() {
                    return input_invalid(RenderingInputProblem::StylesheetNamespaceRequired);
                }
            }
            RenderingStylesheetOrigin::User | RenderingStylesheetOrigin::Author => {
                if sheet.namespace.is_some() {
                    return input_invalid(RenderingInputProblem::StylesheetNamespaceForbidden);
                }
            }
        }
    }
    Ok(())
}

fn sorted_profiles(
    mut profiles: Vec<RenderingObservationProfile>,
) -> Vec<RenderingObservationProfile> {
    profiles.sort();
    profiles
}
fn invalid<T>(problem: RenderingFixtureProblem) -> Result<T, RenderingFixtureLoadError> {
    Err(RenderingFixtureLoadError::Invalid(problem))
}

fn input_invalid<T>(problem: RenderingInputProblem) -> Result<T, RenderingInputLoadError> {
    Err(RenderingInputLoadError::Invalid(problem))
}

pub(crate) fn map_rendering_input_error(
    error: RenderingInputLoadError,
) -> RenderingFixtureLoadError {
    match error {
        RenderingInputLoadError::Io { path, error } => {
            RenderingFixtureLoadError::Io { path, error }
        }
        RenderingInputLoadError::Invalid(problem) => {
            let problem = match problem {
                RenderingInputProblem::InvalidPortablePath { path } => {
                    RenderingFixtureProblem::InvalidPortablePath { path }
                }
                RenderingInputProblem::NonRegularOrSymlink { path } => {
                    RenderingFixtureProblem::NonRegularOrSymlink { path }
                }
                RenderingInputProblem::NonUtf8 { path } => {
                    RenderingFixtureProblem::NonUtf8 { path }
                }
                RenderingInputProblem::DuplicateReferencedPath { path } => {
                    RenderingFixtureProblem::DuplicateReferencedPath { path }
                }
                RenderingInputProblem::InputTooLarge {
                    path,
                    actual,
                    maximum,
                } => RenderingFixtureProblem::InputTooLarge {
                    path,
                    actual,
                    maximum,
                },
                RenderingInputProblem::ArithmeticOverflow { resource } => {
                    RenderingFixtureProblem::ArithmeticOverflow { resource }
                }
                RenderingInputProblem::StorageAllocation { resource } => {
                    RenderingFixtureProblem::StorageAllocation { resource }
                }
                RenderingInputProblem::DuplicateStylesheetOrder { order } => {
                    RenderingFixtureProblem::DuplicateStylesheetOrder { order }
                }
                RenderingInputProblem::NonMonotonicStylesheetOrder { previous, current } => {
                    RenderingFixtureProblem::NonMonotonicStylesheetOrder { previous, current }
                }
                RenderingInputProblem::DuplicateStylesheetSource { source } => {
                    RenderingFixtureProblem::DuplicateStylesheetSource { source }
                }
                RenderingInputProblem::MultipleUserAgentStylesheets => {
                    RenderingFixtureProblem::MultipleUserAgentStylesheets
                }
                RenderingInputProblem::UserAgentSourceMustBeZero => {
                    RenderingFixtureProblem::UserAgentSourceMustBeZero
                }
                RenderingInputProblem::StylesheetNamespaceRequired => {
                    RenderingFixtureProblem::StylesheetNamespaceRequired
                }
                RenderingInputProblem::StylesheetNamespaceForbidden => {
                    RenderingFixtureProblem::StylesheetNamespaceForbidden
                }
            };
            RenderingFixtureLoadError::Invalid(problem)
        }
    }
}

pub(crate) fn checked_add_resource(
    current: usize,
    additional: usize,
    resource: &'static str,
) -> Result<usize, RenderingFixtureLoadError> {
    checked_add_rendering_input_resource(current, additional, resource)
        .map_err(map_rendering_input_error)
}

fn checked_add_rendering_input_resource(
    current: usize,
    additional: usize,
    resource: &'static str,
) -> Result<usize, RenderingInputLoadError> {
    current.checked_add(additional).ok_or({
        RenderingInputLoadError::Invalid(RenderingInputProblem::ArithmeticOverflow { resource })
    })
}

fn load_rendering_input_text(
    root: &Path,
    relative: &str,
    refs: &mut BTreeSet<String>,
    maximum: usize,
) -> Result<String, RenderingInputLoadError> {
    let path = validate_and_register_input_path(root, relative, refs)?;
    let bytes = read_bounded_rendering_input(&path, maximum)?;
    String::from_utf8(bytes).map_err(|_| {
        RenderingInputLoadError::Invalid(RenderingInputProblem::NonUtf8 {
            path: relative.to_owned(),
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn load_rendering_document<Error, MapInputError, AccountStylesheet>(
    root: &Path,
    html_path: String,
    stylesheet_wires: Vec<WireStylesheet>,
    refs: &mut BTreeSet<String>,
    html_maximum: usize,
    stylesheet_maximum: usize,
    mut map_input_error: MapInputError,
    mut account_stylesheet: AccountStylesheet,
) -> Result<RenderingDocumentInput, Error>
where
    MapInputError: FnMut(RenderingInputLoadError) -> Error,
    AccountStylesheet: FnMut(usize) -> Result<(), Error>,
{
    validate_stylesheet_coordinates(&stylesheet_wires).map_err(&mut map_input_error)?;
    let html = load_rendering_input_text(root, &html_path, refs, html_maximum)
        .map_err(&mut map_input_error)?;
    let mut stylesheet_bytes = 0usize;
    let mut stylesheets = Vec::new();
    stylesheets
        .try_reserve(stylesheet_wires.len())
        .map_err(|_| {
            map_input_error(RenderingInputLoadError::Invalid(
                RenderingInputProblem::StorageAllocation {
                    resource: "stylesheet storage",
                },
            ))
        })?;
    for sheet in stylesheet_wires {
        // Preserve the AG6 contract: validate/read/decode the individual file
        // before cumulative accounting can select an aggregate-limit error.
        let source_text = load_rendering_input_text(root, &sheet.path, refs, stylesheet_maximum)
            .map_err(&mut map_input_error)?;
        account_stylesheet(source_text.len())?;
        stylesheet_bytes = checked_add_rendering_input_resource(
            stylesheet_bytes,
            source_text.len(),
            "stylesheet bytes",
        )
        .map_err(&mut map_input_error)?;
        stylesheets.push(RenderingStylesheetInput {
            source_text,
            origin: sheet.origin,
            order: sheet.order,
            source: sheet.source,
            namespace: sheet.namespace,
        });
    }
    Ok(RenderingDocumentInput {
        html,
        html_path,
        stylesheets,
        stylesheet_bytes,
    })
}

fn validate_and_register_path(
    root: &Path,
    relative: &str,
    refs: &mut BTreeSet<String>,
) -> Result<PathBuf, RenderingFixtureLoadError> {
    validate_and_register_input_path(root, relative, refs).map_err(map_rendering_input_error)
}

fn validate_and_register_input_path(
    root: &Path,
    relative: &str,
    refs: &mut BTreeSet<String>,
) -> Result<PathBuf, RenderingInputLoadError> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return input_invalid(RenderingInputProblem::InvalidPortablePath {
            path: relative.to_owned(),
        });
    }
    if !refs.insert(relative.to_owned()) {
        return input_invalid(RenderingInputProblem::DuplicateReferencedPath {
            path: relative.to_owned(),
        });
    }
    let joined = root.join(path);
    ensure_regular_rendering_input(&joined, relative)?;
    Ok(joined)
}

pub(crate) fn ensure_regular(path: &Path, label: &str) -> Result<(), RenderingFixtureLoadError> {
    ensure_regular_rendering_input(path, label).map_err(map_rendering_input_error)
}

pub(crate) fn ensure_regular_rendering_input(
    path: &Path,
    label: &str,
) -> Result<(), RenderingInputLoadError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| RenderingInputLoadError::Io {
        path: label.to_owned(),
        error,
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return input_invalid(RenderingInputProblem::NonRegularOrSymlink {
            path: label.to_owned(),
        });
    }
    Ok(())
}

fn regular_file_length(path: &Path, label: &str) -> Result<usize, RenderingFixtureLoadError> {
    regular_rendering_input_file_length(path, label).map_err(map_rendering_input_error)
}

fn regular_rendering_input_file_length(
    path: &Path,
    label: &str,
) -> Result<usize, RenderingInputLoadError> {
    ensure_regular_rendering_input(path, label)?;
    let length = fs::metadata(path)
        .map_err(|error| RenderingInputLoadError::Io {
            path: label.to_owned(),
            error,
        })?
        .len();
    usize::try_from(length).map_err(|_| {
        RenderingInputLoadError::Invalid(RenderingInputProblem::ArithmeticOverflow {
            resource: "file length",
        })
    })
}

pub(crate) fn read_bounded(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, RenderingFixtureLoadError> {
    read_bounded_rendering_input(path, maximum).map_err(map_rendering_input_error)
}

pub(crate) fn read_bounded_rendering_input(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, RenderingInputLoadError> {
    let label = path.display().to_string();
    let length = regular_rendering_input_file_length(path, &label)?;
    if length > maximum {
        return input_invalid(RenderingInputProblem::InputTooLarge {
            path: label,
            actual: length,
            maximum,
        });
    }
    let mut bytes = Vec::new();
    bytes.try_reserve(length).map_err(|_| {
        RenderingInputLoadError::Invalid(RenderingInputProblem::StorageAllocation {
            resource: "authored file storage",
        })
    })?;
    let read_limit = maximum.checked_add(1).ok_or({
        RenderingInputLoadError::Invalid(RenderingInputProblem::ArithmeticOverflow {
            resource: "bounded authored file read",
        })
    })?;
    let file = fs::File::open(path).map_err(|error| RenderingInputLoadError::Io {
        path: label.clone(),
        error,
    })?;
    file.take(read_limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| RenderingInputLoadError::Io {
            path: label.clone(),
            error,
        })?;
    if bytes.len() > maximum {
        return input_invalid(RenderingInputProblem::InputTooLarge {
            path: label,
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_limits() -> RenderingFixtureLimits {
        RenderingFixtureLimits::try_new(64 * 1024, 8 * 1024 * 1024).unwrap()
    }

    fn paint_profiles() -> Vec<RenderingObservationProfile> {
        use crate::PaintObservationProfile::*;
        vec![
            RenderingObservationProfile::Paint(PaintSemanticArtifact),
            RenderingObservationProfile::Paint(PaintOrder),
            RenderingObservationProfile::Paint(PaintStackingContexts),
            RenderingObservationProfile::Paint(PaintLayering),
            RenderingObservationProfile::Paint(PaintOperations),
        ]
    }

    fn dimension_wire(
        stylesheet_count: usize,
        profiles: Vec<RenderingObservationProfile>,
        variant_count: usize,
    ) -> WireFixture {
        let variants = (0..variant_count)
            .map(|index| WireVariant {
                environment: SyntheticTextMetricsV1::SyntheticTextMetricsV1,
                available_width_css_px: index as u32 + 1,
                expectations: profiles
                    .iter()
                    .map(|profile| WireExpectation {
                        profile: *profile,
                        snapshot: format!("expected-{index}.txt"),
                    })
                    .collect(),
            })
            .collect();
        WireFixture {
            format: RENDERING_FIXTURE_FORMAT_V1.to_owned(),
            id: "limit-test".to_owned(),
            profiles,
            input: WireInput {
                html: "document.html".to_owned(),
                stylesheets: (0..stylesheet_count)
                    .map(|index| WireStylesheet {
                        path: format!("sheet-{index}.css"),
                        origin: RenderingStylesheetOrigin::Author,
                        order: index as u32,
                        source: index as u32,
                        namespace: None,
                    })
                    .collect(),
            },
            variants,
        }
    }

    #[test]
    fn rendering_owned_v1_limits_are_exact_and_transport_limits_are_injected() {
        let limits = RenderingFixtureLimits::try_new(123, 456).unwrap();
        assert_eq!(limits.input.descriptor_bytes, 123);
        assert_eq!(limits.input.html_input_bytes, 4 * 1024 * 1024);
        assert_eq!(
            limits.input.stylesheet_input_bytes,
            css::SyntaxLimits::default().max_stylesheet_input_bytes
        );
        assert_eq!(limits.cumulative_stylesheet_input_bytes, 16 * 1024 * 1024);
        assert_eq!(limits.input.stylesheet_count, 64);
        assert_eq!(limits.expected_snapshot_bytes, 456);
        assert_eq!(limits.cumulative_observation_bytes, 456 * 5);
        assert_eq!(limits.cumulative_expected_snapshot_bytes, 32 * 1024 * 1024);
        assert_eq!(limits.input.variant_count, 16);
        assert_eq!(limits.input.selected_profile_count, 5);
        assert_eq!(limits.expectation_pair_count, 80);
    }

    #[test]
    fn ag6_limit_construction_is_independent_of_paired_capture_policy() {
        let limits =
            RenderingFixtureLimits::try_new(1, RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1)
                .unwrap();
        assert_eq!(
            limits.expected_snapshot_bytes(),
            RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1
        );
        assert_eq!(
            limits.cumulative_observation_bytes,
            RENDERING_CUMULATIVE_EXPECTED_SNAPSHOT_BYTES_V1 * RENDERING_SELECTED_PROFILE_COUNT_V1
        );
        assert!(
            !RenderingFixtureProblem::CumulativeStylesheetBytesExceeded {
                actual: 2,
                maximum: 1,
            }
            .to_string()
            .contains("paired")
        );
    }

    #[test]
    fn cumulative_arithmetic_overflow_is_detected() {
        assert!(matches!(
            checked_add_resource(usize::MAX, 1, "test"),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::ArithmeticOverflow { resource: "test" }
            ))
        ));
    }

    #[test]
    fn closed_count_limits_accept_maximum_and_reject_maximum_plus_one() {
        let limits = fixture_limits();
        assert!(
            validate_dimensions(
                &dimension_wire(64, paint_profiles(), 16),
                RenderingObservationOwner::Paint,
                limits,
            )
            .is_ok()
        );

        assert!(matches!(
            validate_dimensions(
                &dimension_wire(65, paint_profiles(), 16),
                RenderingObservationOwner::Paint,
                limits,
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::TooManyStylesheets {
                    actual: 65,
                    maximum: 64,
                }
            ))
        ));

        let mut six_profiles = paint_profiles();
        six_profiles.push(RenderingObservationProfile::Paint(
            crate::PaintObservationProfile::PaintOperations,
        ));
        assert!(matches!(
            validate_dimensions(
                &dimension_wire(64, six_profiles, 16),
                RenderingObservationOwner::Paint,
                limits,
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::TooManyProfiles {
                    actual: 6,
                    maximum: 5,
                }
            ))
        ));

        assert!(matches!(
            validate_dimensions(
                &dimension_wire(64, paint_profiles(), 17),
                RenderingObservationOwner::Paint,
                limits,
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::TooManyVariants {
                    actual: 17,
                    maximum: 16,
                }
            ))
        ));

        let pair_limits = RenderingFixtureLimits {
            input: RenderingInputLimits {
                variant_count: 17,
                ..limits.input
            },
            ..limits
        };
        assert!(matches!(
            validate_dimensions(
                &dimension_wire(64, paint_profiles(), 17),
                RenderingObservationOwner::Paint,
                pair_limits,
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::TooManyExpectationPairs {
                    actual: 85,
                    maximum: 80,
                }
            ))
        ));
    }

    #[test]
    fn cumulative_fixture_limits_accept_maximum_and_reject_maximum_plus_one() {
        let directory = tempfile::tempdir().unwrap();
        let descriptor = directory.path().join("fixture.toml");
        std::fs::write(directory.path().join("document.html"), "x").unwrap();
        std::fs::write(directory.path().join("a.css"), "aa").unwrap();
        std::fs::write(directory.path().join("b.css"), "bb").unwrap();
        std::fs::write(directory.path().join("expected-01.txt"), "aa").unwrap();
        std::fs::write(directory.path().join("expected-02.txt"), "bb").unwrap();
        std::fs::write(
            &descriptor,
            concat!(
                "format = \"borrowser-rendering-fixture-v1\"\n",
                "id = \"cumulative-limit-test\"\n",
                "profiles = [\"layout-sizing\", \"layout-flex\"]\n",
                "[input]\nhtml = \"document.html\"\n",
                "stylesheets = [\n",
                "  { path = \"a.css\", origin = \"author\", order = 0, source = 0 },\n",
                "  { path = \"b.css\", origin = \"author\", order = 1, source = 1 },\n",
                "]\n",
                "[[variants]]\n",
                "environment = \"synthetic-text-metrics-v1\"\n",
                "available_width_css_px = 320\n",
                "expectations = [\n",
                "  { profile = \"layout-sizing\", snapshot = \"expected-01.txt\" },\n",
                "  { profile = \"layout-flex\", snapshot = \"expected-02.txt\" },\n",
                "]\n",
            ),
        )
        .unwrap();
        let limits = RenderingFixtureLimits {
            cumulative_stylesheet_input_bytes: 4,
            cumulative_expected_snapshot_bytes: 4,
            ..fixture_limits()
        };
        assert!(
            load_fixture_package(&descriptor, RenderingObservationOwner::Layout, limits).is_ok()
        );

        std::fs::write(directory.path().join("b.css"), "bbb").unwrap();
        assert!(matches!(
            load_fixture_package(&descriptor, RenderingObservationOwner::Layout, limits),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::CumulativeStylesheetBytesExceeded {
                    actual: 5,
                    maximum: 4,
                }
            ))
        ));

        std::fs::write(directory.path().join("b.css"), "bb").unwrap();
        std::fs::write(directory.path().join("expected-02.txt"), "bbb").unwrap();
        assert!(matches!(
            load_fixture_package(&descriptor, RenderingObservationOwner::Layout, limits),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::CumulativeExpectedBytesExceeded {
                    actual: 5,
                    maximum: 4,
                }
            ))
        ));
    }

    fn load_ag6_single_stylesheet_fixture(
        root: &Path,
        path: &str,
        individual_maximum: usize,
        cumulative_maximum: usize,
    ) -> Result<RenderingFixturePackage, RenderingFixtureLoadError> {
        std::fs::write(root.join("document.html"), "<div></div>").unwrap();
        std::fs::write(root.join("expected.txt"), "expected").unwrap();
        std::fs::write(
            root.join("fixture.toml"),
            format!(
                concat!(
                    "format = \"borrowser-rendering-fixture-v1\"\n",
                    "id = \"ag6-error-precedence\"\n",
                    "profiles = [\"layout-sizing\"]\n",
                    "[input]\nhtml = \"document.html\"\n",
                    "stylesheets = [{{ path = \"{}\", origin = \"author\", order = 0, source = 0 }}]\n",
                    "[[variants]]\n",
                    "environment = \"synthetic-text-metrics-v1\"\n",
                    "available_width_css_px = 320\n",
                    "expectations = [{{ profile = \"layout-sizing\", snapshot = \"expected.txt\" }}]\n",
                ),
                path
            ),
        )
        .unwrap();
        let base = fixture_limits();
        let limits = RenderingFixtureLimits {
            input: RenderingInputLimits {
                stylesheet_input_bytes: individual_maximum,
                ..base.input
            },
            cumulative_stylesheet_input_bytes: cumulative_maximum,
            ..base
        };
        load_fixture_package(
            &root.join("fixture.toml"),
            RenderingObservationOwner::Layout,
            limits,
        )
    }

    #[test]
    fn ag6_stylesheet_error_precedence_remains_read_decode_then_cumulative_accounting() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("document.html"), "<div></div>").unwrap();

        std::fs::write(directory.path().join("invalid.css"), [0xff]).unwrap();
        assert!(matches!(
            load_ag6_single_stylesheet_fixture(directory.path(), "invalid.css", 8, 0),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::NonUtf8 { .. }
            ))
        ));

        std::fs::write(directory.path().join("oversized.css"), "ab").unwrap();
        assert!(matches!(
            load_ag6_single_stylesheet_fixture(directory.path(), "oversized.css", 1, 0),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::InputTooLarge {
                    actual: 2,
                    maximum: 1,
                    ..
                }
            ))
        ));

        assert!(matches!(
            load_ag6_single_stylesheet_fixture(directory.path(), "missing.css", 8, 0),
            Err(RenderingFixtureLoadError::Io { .. })
        ));

        std::fs::create_dir(directory.path().join("directory.css")).unwrap();
        assert!(matches!(
            load_ag6_single_stylesheet_fixture(directory.path(), "directory.css", 8, 0),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::NonRegularOrSymlink { .. }
            ))
        ));
    }

    #[test]
    fn authored_file_limit_accepts_maximum_and_rejects_maximum_plus_one() {
        let directory = tempfile::tempdir().unwrap();
        let exact = directory.path().join("exact.txt");
        let oversized = directory.path().join("oversized.txt");
        std::fs::write(&exact, b"1234").unwrap();
        std::fs::write(&oversized, b"12345").unwrap();
        assert_eq!(read_bounded(&exact, 4).unwrap(), b"1234");
        assert!(matches!(
            read_bounded(&oversized, 4),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::InputTooLarge {
                    actual: 5,
                    maximum: 4,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn descriptor_maximum_plus_one_has_a_descriptor_specific_failure() {
        let directory = tempfile::tempdir().unwrap();
        let descriptor = directory.path().join("fixture.toml");
        std::fs::write(&descriptor, vec![b' '; 64 * 1024 + 1]).unwrap();
        assert!(matches!(
            load_fixture_package(
                &descriptor,
                RenderingObservationOwner::Layout,
                fixture_limits(),
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::DescriptorTooLarge {
                    actual: 65_537,
                    maximum: 65_536,
                }
            ))
        ));
    }

    #[test]
    fn backslash_paths_are_rejected_before_filesystem_access() {
        let directory = tempfile::tempdir().unwrap();
        assert!(matches!(
            validate_and_register_path(
                directory.path(),
                "rendering\\test.html",
                &mut BTreeSet::new(),
            ),
            Err(RenderingFixtureLoadError::Invalid(
                RenderingFixtureProblem::InvalidPortablePath { .. }
            ))
        ));
    }

    #[test]
    fn fixture_problem_display_is_deliberate_and_not_debug_derived() {
        assert_eq!(
            RenderingFixtureProblem::OwnerProfileMismatch {
                profile: "paint-order"
            }
            .to_string(),
            "profile does not belong to the fixture owner: paint-order"
        );
        assert_eq!(
            RenderingFixtureLoadError::Invalid(RenderingFixtureProblem::EmptyProfiles).to_string(),
            "invalid rendering fixture: profile set must be non-empty"
        );
    }
}
