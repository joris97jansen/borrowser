use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

use serde::Deserialize;

use crate::target::CssTargetAddress;

pub const CSS_FIXTURE_FORMAT_V1: &str = "borrowser-css-fixture-v1";
pub const CSS_NESTED_MAX_HTML_INPUT_BYTES: usize = 4 * 1024 * 1024;
pub const CSS_NESTED_MAX_TARGETS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CssFixtureLimits {
    max_descriptor_bytes: usize,
    max_html_input_bytes: usize,
    max_targets: usize,
    max_target_depth: usize,
    max_expected_bytes: usize,
    max_stylesheets: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssFixtureLimitConfigurationError {
    ZeroDescriptorBytes,
    ZeroExpectedObservationBytes,
    ZeroStylesheets,
    StylesheetsExceedProduction {
        configured: usize,
        production_maximum: usize,
    },
}

impl std::fmt::Display for CssFixtureLimitConfigurationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDescriptorBytes => {
                formatter.write_str("CSS fixture descriptor limit must be non-zero")
            }
            Self::ZeroExpectedObservationBytes => {
                formatter.write_str("CSS expected-observation limit must be non-zero")
            }
            Self::ZeroStylesheets => {
                formatter.write_str("CSS fixture stylesheet limit must be non-zero")
            }
            Self::StylesheetsExceedProduction {
                configured,
                production_maximum,
            } => write!(
                formatter,
                "CSS fixture stylesheet limit {configured} exceeds production maximum {production_maximum}"
            ),
        }
    }
}

impl std::error::Error for CssFixtureLimitConfigurationError {}

impl CssFixtureLimits {
    pub fn production_stylesheet_maximum() -> usize {
        css::StyleResolutionLimits::default().max_stylesheets_per_style_pass
    }

    pub fn production_target_depth_maximum() -> usize {
        html::HtmlTreeBuilderLimits::default().max_open_elements_depth
    }

    pub fn try_new(
        max_descriptor_bytes: usize,
        max_expected_bytes: usize,
        max_stylesheets: usize,
    ) -> Result<Self, CssFixtureLimitConfigurationError> {
        if max_descriptor_bytes == 0 {
            return Err(CssFixtureLimitConfigurationError::ZeroDescriptorBytes);
        }
        if max_expected_bytes == 0 {
            return Err(CssFixtureLimitConfigurationError::ZeroExpectedObservationBytes);
        }
        if max_stylesheets == 0 {
            return Err(CssFixtureLimitConfigurationError::ZeroStylesheets);
        }
        let production_maximum = Self::production_stylesheet_maximum();
        if max_stylesheets > production_maximum {
            return Err(
                CssFixtureLimitConfigurationError::StylesheetsExceedProduction {
                    configured: max_stylesheets,
                    production_maximum,
                },
            );
        }
        Ok(Self {
            max_descriptor_bytes,
            max_html_input_bytes: CSS_NESTED_MAX_HTML_INPUT_BYTES,
            max_targets: CSS_NESTED_MAX_TARGETS,
            max_target_depth: Self::production_target_depth_maximum(),
            max_expected_bytes,
            max_stylesheets,
        })
    }

    pub const fn max_descriptor_bytes(self) -> usize {
        self.max_descriptor_bytes
    }

    pub const fn max_html_input_bytes(self) -> usize {
        self.max_html_input_bytes
    }

    pub const fn max_targets(self) -> usize {
        self.max_targets
    }

    pub const fn max_target_depth(self) -> usize {
        self.max_target_depth
    }

    pub const fn max_expected_bytes(self) -> usize {
        self.max_expected_bytes
    }

    pub const fn max_stylesheets(self) -> usize {
        self.max_stylesheets
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CssExecutionProfile {
    PropertyValue,
    SelectorParsing,
    SelectorSpecificity,
    SelectorMatching,
    CascadeWinner,
    InheritanceCssWide,
    ComputedStyle,
}

impl CssExecutionProfile {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::PropertyValue => "property-value",
            Self::SelectorParsing => "selector-parsing",
            Self::SelectorSpecificity => "selector-specificity",
            Self::SelectorMatching => "selector-matching",
            Self::CascadeWinner => "cascade-winner",
            Self::InheritanceCssWide => "inheritance-css-wide",
            Self::ComputedStyle => "computed-style",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CssHtmlInputKind {
    Document,
    Fragment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CssHostNamespace {
    Html,
    Svg,
    MathMl,
}

impl CssHostNamespace {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Svg => "svg",
            Self::MathMl => "mathml",
        }
    }

    pub const fn as_element_namespace(self) -> html::ElementNamespace {
        match self {
            Self::Html => html::ElementNamespace::Html,
            Self::Svg => html::ElementNamespace::Svg,
            Self::MathMl => html::ElementNamespace::MathMl,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssFragmentContext {
    namespace: CssHostNamespace,
    local_name: String,
}

impl CssFragmentContext {
    pub fn namespace(&self) -> CssHostNamespace {
        self.namespace
    }

    pub fn local_name(&self) -> &str {
        &self.local_name
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CssStylesheetOrigin {
    UserAgent,
    User,
    Author,
}

#[derive(Debug)]
pub struct CssFixturePackage {
    pub(crate) id: String,
    pub(crate) profile: CssExecutionProfile,
    pub(crate) selector_list: Option<String>,
    pub(crate) property_stylesheet: Option<String>,
    pub(crate) stylesheets: Vec<CssStylesheetInput>,
    pub(crate) html: Option<CssHtmlInput>,
    pub(crate) property: Option<PropertyCoordinate>,
    pub(crate) targets: Vec<CssTargetAddress>,
    pub(crate) selected_properties: Vec<String>,
    pub(crate) expected: String,
    pub(crate) primary_input_path: String,
    pub(crate) referenced_paths: Vec<String>,
    pub(crate) limits: CssFixtureLimits,
}

impl CssFixturePackage {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn profile(&self) -> CssExecutionProfile {
        self.profile
    }

    pub fn html_kind(&self) -> Option<CssHtmlInputKind> {
        self.html.as_ref().map(CssHtmlInput::kind)
    }

    pub fn fragment_context(&self) -> Option<&CssFragmentContext> {
        self.html.as_ref().and_then(CssHtmlInput::fragment_context)
    }

    pub fn primary_input_path(&self) -> &str {
        &self.primary_input_path
    }

    pub fn referenced_paths(&self) -> impl Iterator<Item = &str> {
        self.referenced_paths.iter().map(String::as_str)
    }
}

#[derive(Debug)]
pub(crate) struct CssHtmlInput {
    pub(crate) request: CssHtmlRequest,
    pub(crate) source: String,
}

impl CssHtmlInput {
    fn kind(&self) -> CssHtmlInputKind {
        match self.request {
            CssHtmlRequest::Document => CssHtmlInputKind::Document,
            CssHtmlRequest::Fragment { .. } => CssHtmlInputKind::Fragment,
        }
    }

    fn fragment_context(&self) -> Option<&CssFragmentContext> {
        match &self.request {
            CssHtmlRequest::Document => None,
            CssHtmlRequest::Fragment { context } => Some(context),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CssHtmlRequest {
    Document,
    Fragment { context: CssFragmentContext },
}

#[derive(Debug)]
pub(crate) struct CssStylesheetInput {
    pub(crate) source: String,
    pub(crate) origin: CssStylesheetOrigin,
    pub(crate) order: u32,
    pub(crate) source_index: u32,
    pub(crate) namespace: Option<CssHostNamespace>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PropertyCoordinate {
    pub(crate) rule_index: usize,
    pub(crate) declaration_index: usize,
}

#[derive(Debug)]
pub enum CssFixtureLoadError {
    Io {
        path: String,
        error: std::io::Error,
    },
    Parse {
        path: String,
        error: toml::de::Error,
    },
    Invalid(CssFixtureProblem),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssFixtureProblem {
    DescriptorMustBeFixtureToml,
    DescriptorTooLarge {
        actual: usize,
        maximum: usize,
    },
    WrongFormat,
    InvalidPortablePath {
        path: String,
    },
    NonRegularOrSymlink {
        path: String,
    },
    DuplicateReferencedPath {
        path: String,
    },
    ProfileContract {
        reason: &'static str,
    },
    StorageReservation {
        storage: &'static str,
    },
    FragmentContextRequired,
    FragmentContextForbiddenForDocument,
    InvalidFragmentContextLocalName,
    DuplicateTargetLabel {
        label: String,
    },
    DuplicateSelectedProperty {
        property: String,
    },
    UnsupportedSelectedProperty {
        property: String,
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
    StylesheetNamespaceRequired,
    StylesheetNamespaceForbidden,
    UserAgentSourceMustBeZero,
    MultipleUserAgentStylesheets,
    TooManyTargets {
        actual: usize,
        maximum: usize,
    },
    TargetDepthExceeded {
        actual: usize,
        maximum: usize,
    },
    TooManySelectedProperties {
        actual: usize,
        maximum: usize,
    },
    TooManyStylesheets {
        actual: usize,
        maximum: usize,
    },
    InputTooLarge {
        path: String,
        actual: usize,
        maximum: usize,
    },
    ExpectedTooLarge {
        actual: usize,
        maximum: usize,
    },
    NonUtf8 {
        path: String,
    },
}

impl std::fmt::Display for CssFixtureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "CSS package I/O failed for {path}: {error}"),
            Self::Parse { path, error } => {
                write!(f, "CSS fixture descriptor {path} is invalid: {error}")
            }
            Self::Invalid(problem) => write!(f, "invalid nested CSS fixture: {problem}"),
        }
    }
}

impl std::error::Error for CssFixtureLoadError {}

impl std::fmt::Display for CssFixtureProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DescriptorMustBeFixtureToml => f.write_str("descriptor must be fixture.toml"),
            Self::DescriptorTooLarge { actual, maximum } => {
                write!(f, "descriptor is {actual} bytes; maximum is {maximum}")
            }
            Self::WrongFormat => f.write_str("nested CSS fixture format is unsupported"),
            Self::InvalidPortablePath { path } => write!(f, "path is not portable: {path}"),
            Self::NonRegularOrSymlink { path } => {
                write!(f, "path is not a regular non-symlink file: {path}")
            }
            Self::DuplicateReferencedPath { path } => write!(f, "path is referenced twice: {path}"),
            Self::ProfileContract { reason } => write!(f, "profile contract violation: {reason}"),
            Self::StorageReservation { storage } => {
                write!(f, "failed to reserve {storage} storage")
            }
            Self::FragmentContextRequired => f.write_str("fragment input requires context"),
            Self::FragmentContextForbiddenForDocument => {
                f.write_str("document input forbids fragment context")
            }
            Self::InvalidFragmentContextLocalName => {
                f.write_str("fragment context local name is invalid")
            }
            Self::DuplicateTargetLabel { label } => write!(f, "duplicate target label: {label}"),
            Self::DuplicateSelectedProperty { property } => {
                write!(f, "duplicate selected property: {property}")
            }
            Self::UnsupportedSelectedProperty { property } => {
                write!(f, "selected property is outside the registry: {property}")
            }
            Self::DuplicateStylesheetOrder { order } => {
                write!(f, "duplicate stylesheet order: {order}")
            }
            Self::NonMonotonicStylesheetOrder { previous, current } => write!(
                f,
                "stylesheet order is not increasing: {previous} then {current}"
            ),
            Self::DuplicateStylesheetSource { source } => {
                write!(f, "duplicate stylesheet source identity: {source}")
            }
            Self::StylesheetNamespaceRequired => {
                f.write_str("user-agent stylesheet requires an exact namespace")
            }
            Self::StylesheetNamespaceForbidden => {
                f.write_str("author/user stylesheet forbids a namespace constraint")
            }
            Self::UserAgentSourceMustBeZero => {
                f.write_str("built-in user-agent stylesheet source must be zero")
            }
            Self::MultipleUserAgentStylesheets => {
                f.write_str("only one built-in user-agent stylesheet is representable")
            }
            Self::TooManyTargets { actual, maximum } => {
                write!(f, "target count {actual} exceeds {maximum}")
            }
            Self::TargetDepthExceeded { actual, maximum } => {
                write!(f, "target depth {actual} exceeds {maximum}")
            }
            Self::TooManySelectedProperties { actual, maximum } => write!(
                f,
                "selected property count {actual} exceeds registry size {maximum}"
            ),
            Self::TooManyStylesheets { actual, maximum } => {
                write!(f, "stylesheet count {actual} exceeds {maximum}")
            }
            Self::InputTooLarge {
                path,
                actual,
                maximum,
            } => write!(f, "input {path} is {actual} bytes; maximum is {maximum}"),
            Self::ExpectedTooLarge { actual, maximum } => write!(
                f,
                "expected observation is {actual} bytes; maximum is {maximum}"
            ),
            Self::NonUtf8 { path } => write!(f, "authored text is not UTF-8: {path}"),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFixture {
    format: String,
    id: String,
    profile: CssExecutionProfile,
    input: WireInput,
    property: Option<WireProperty>,
    #[serde(default)]
    targets: Vec<CssTargetAddress>,
    expectations: WireExpectations,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireInput {
    selector_list: Option<String>,
    stylesheet: Option<String>,
    #[serde(default)]
    stylesheets: Vec<WireStylesheet>,
    html: Option<WireHtml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireStylesheet {
    path: String,
    origin: CssStylesheetOrigin,
    order: u32,
    source: u32,
    namespace: Option<CssHostNamespace>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireHtml {
    kind: CssHtmlInputKind,
    path: String,
    context: Option<WireFragmentContext>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFragmentContext {
    namespace: CssHostNamespace,
    local_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProperty {
    rule_index: usize,
    declaration_index: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireExpectations {
    snapshot: String,
    #[serde(default)]
    selected_properties: Vec<String>,
}

pub fn load_fixture_package(
    entry_path: &Path,
    limits: CssFixtureLimits,
) -> Result<CssFixturePackage, CssFixtureLoadError> {
    if entry_path.file_name().and_then(|name| name.to_str()) != Some("fixture.toml") {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::DescriptorMustBeFixtureToml,
        ));
    }
    ensure_regular(entry_path, &entry_path.display().to_string())?;
    let descriptor =
        read_bounded(entry_path, limits.max_descriptor_bytes).map_err(|error| match error {
            ReadError::Io(error) => CssFixtureLoadError::Io {
                path: entry_path.display().to_string(),
                error,
            },
            ReadError::TooLarge(actual) => {
                CssFixtureLoadError::Invalid(CssFixtureProblem::DescriptorTooLarge {
                    actual,
                    maximum: limits.max_descriptor_bytes,
                })
            }
        })?;
    let descriptor_text = std::str::from_utf8(&descriptor).map_err(|_| {
        CssFixtureLoadError::Invalid(CssFixtureProblem::NonUtf8 {
            path: entry_path.display().to_string(),
        })
    })?;
    let wire: WireFixture =
        toml::from_str(descriptor_text).map_err(|error| CssFixtureLoadError::Parse {
            path: entry_path.display().to_string(),
            error,
        })?;
    if wire.format != CSS_FIXTURE_FORMAT_V1 {
        return Err(CssFixtureLoadError::Invalid(CssFixtureProblem::WrongFormat));
    }
    validate_profile(&wire)?;
    validate_dimensions(&wire, limits)?;
    validate_html_request(wire.input.html.as_ref())?;
    validate_stylesheets(&wire.input.stylesheets, limits)?;

    let package_root = entry_path.parent().ok_or(CssFixtureLoadError::Invalid(
        CssFixtureProblem::DescriptorMustBeFixtureToml,
    ))?;
    let mut refs = BTreeSet::new();
    let selector_list = load_optional_text(
        package_root,
        wire.input.selector_list.as_deref(),
        &mut refs,
        css::SyntaxLimits::default().max_stylesheet_input_bytes,
    )?;
    let property_stylesheet = load_optional_text(
        package_root,
        wire.input.stylesheet.as_deref(),
        &mut refs,
        css::SyntaxLimits::default().max_stylesheet_input_bytes,
    )?;
    let mut stylesheets = Vec::new();
    stylesheets
        .try_reserve(wire.input.stylesheets.len())
        .map_err(|_| {
            CssFixtureLoadError::Invalid(CssFixtureProblem::StorageReservation {
                storage: "stylesheet package",
            })
        })?;
    for stylesheet in &wire.input.stylesheets {
        stylesheets.push(CssStylesheetInput {
            source: load_text(
                package_root,
                &stylesheet.path,
                &mut refs,
                css::SyntaxLimits::default().max_stylesheet_input_bytes,
            )?,
            origin: stylesheet.origin,
            order: stylesheet.order,
            source_index: stylesheet.source,
            namespace: stylesheet.namespace,
        });
    }
    let html = match &wire.input.html {
        Some(input) => Some(CssHtmlInput {
            request: match (input.kind, &input.context) {
                (CssHtmlInputKind::Document, None) => CssHtmlRequest::Document,
                (CssHtmlInputKind::Fragment, Some(context)) => CssHtmlRequest::Fragment {
                    context: CssFragmentContext {
                        namespace: context.namespace,
                        local_name: context.local_name.clone(),
                    },
                },
                _ => unreachable!("validated HTML request"),
            },
            source: load_text(
                package_root,
                &input.path,
                &mut refs,
                limits.max_html_input_bytes,
            )?,
        }),
        None => None,
    };
    let expected = load_text(
        package_root,
        &wire.expectations.snapshot,
        &mut refs,
        limits.max_expected_bytes,
    )?;
    let primary_input_path = primary_path(&wire)
        .ok_or(CssFixtureLoadError::Invalid(
            CssFixtureProblem::ProfileContract {
                reason: "profile has no primary authored input",
            },
        ))?
        .to_owned();

    Ok(CssFixturePackage {
        id: wire.id,
        profile: wire.profile,
        selector_list,
        property_stylesheet,
        stylesheets,
        html,
        property: wire.property.map(|property| PropertyCoordinate {
            rule_index: property.rule_index,
            declaration_index: property.declaration_index,
        }),
        targets: wire.targets,
        selected_properties: wire.expectations.selected_properties,
        expected,
        primary_input_path,
        referenced_paths: refs.into_iter().collect(),
        limits,
    })
}

fn validate_dimensions(
    wire: &WireFixture,
    limits: CssFixtureLimits,
) -> Result<(), CssFixtureLoadError> {
    if wire.targets.len() > limits.max_targets {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::TooManyTargets {
                actual: wire.targets.len(),
                maximum: limits.max_targets,
            },
        ));
    }
    let mut labels = BTreeSet::new();
    for target in &wire.targets {
        if !labels.insert(target.label().to_owned()) {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::DuplicateTargetLabel {
                    label: target.label().to_owned(),
                },
            ));
        }
        if target.steps().len() > limits.max_target_depth {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::TargetDepthExceeded {
                    actual: target.steps().len(),
                    maximum: limits.max_target_depth,
                },
            ));
        }
        target.validate().map_err(|_| {
            CssFixtureLoadError::Invalid(CssFixtureProblem::ProfileContract {
                reason: "invalid target address grammar",
            })
        })?;
    }
    let property_maximum = css::property_registry().entries().len();
    if wire.expectations.selected_properties.len() > property_maximum {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::TooManySelectedProperties {
                actual: wire.expectations.selected_properties.len(),
                maximum: property_maximum,
            },
        ));
    }
    let mut properties = BTreeSet::new();
    for property in &wire.expectations.selected_properties {
        if css::PropertyId::from_name(property).is_none() {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::UnsupportedSelectedProperty {
                    property: property.clone(),
                },
            ));
        }
        if !properties.insert(property.as_str()) {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::DuplicateSelectedProperty {
                    property: property.clone(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_html_request(html: Option<&WireHtml>) -> Result<(), CssFixtureLoadError> {
    let Some(html) = html else { return Ok(()) };
    match (html.kind, &html.context) {
        (CssHtmlInputKind::Document, Some(_)) => Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::FragmentContextForbiddenForDocument,
        )),
        (CssHtmlInputKind::Fragment, None) => Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::FragmentContextRequired,
        )),
        (CssHtmlInputKind::Fragment, Some(context))
            if context.local_name.is_empty()
                || context.local_name.len()
                    > html::HtmlTokenizerLimits::default().max_tag_name_bytes
                || context
                    .local_name
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace()) =>
        {
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::InvalidFragmentContextLocalName,
            ))
        }
        _ => Ok(()),
    }
}

fn validate_stylesheets(
    stylesheets: &[WireStylesheet],
    limits: CssFixtureLimits,
) -> Result<(), CssFixtureLoadError> {
    if stylesheets.len() > limits.max_stylesheets {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::TooManyStylesheets {
                actual: stylesheets.len(),
                maximum: limits.max_stylesheets,
            },
        ));
    }
    let mut orders = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut previous_order = None;
    let mut saw_user_agent = false;
    for stylesheet in stylesheets {
        if !orders.insert(stylesheet.order) {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::DuplicateStylesheetOrder {
                    order: stylesheet.order,
                },
            ));
        }
        if let Some(previous) = previous_order
            && stylesheet.order <= previous
        {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::NonMonotonicStylesheetOrder {
                    previous,
                    current: stylesheet.order,
                },
            ));
        }
        previous_order = Some(stylesheet.order);
        if !sources.insert(stylesheet.source) {
            return Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::DuplicateStylesheetSource {
                    source: stylesheet.source,
                },
            ));
        }
        match stylesheet.origin {
            CssStylesheetOrigin::UserAgent => {
                if saw_user_agent {
                    return Err(CssFixtureLoadError::Invalid(
                        CssFixtureProblem::MultipleUserAgentStylesheets,
                    ));
                }
                saw_user_agent = true;
                if stylesheet.source != 0 {
                    return Err(CssFixtureLoadError::Invalid(
                        CssFixtureProblem::UserAgentSourceMustBeZero,
                    ));
                }
                if stylesheet.namespace.is_none() {
                    return Err(CssFixtureLoadError::Invalid(
                        CssFixtureProblem::StylesheetNamespaceRequired,
                    ));
                }
            }
            CssStylesheetOrigin::User | CssStylesheetOrigin::Author => {
                if stylesheet.namespace.is_some() {
                    return Err(CssFixtureLoadError::Invalid(
                        CssFixtureProblem::StylesheetNamespaceForbidden,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_profile(wire: &WireFixture) -> Result<(), CssFixtureLoadError> {
    let has_selector = wire.input.selector_list.is_some();
    let has_property_stylesheet = wire.input.stylesheet.is_some();
    let sheet_count = wire.input.stylesheets.len();
    let has_html = wire.input.html.is_some();
    let has_property = wire.property.is_some();
    let targets = !wire.targets.is_empty();
    let properties = !wire.expectations.selected_properties.is_empty();
    let valid = match wire.profile {
        CssExecutionProfile::PropertyValue => {
            has_property_stylesheet
                && sheet_count == 0
                && has_property
                && !has_selector
                && !has_html
                && !targets
                && !properties
        }
        CssExecutionProfile::SelectorParsing | CssExecutionProfile::SelectorSpecificity => {
            has_selector
                && !has_property_stylesheet
                && sheet_count == 0
                && !has_html
                && !has_property
                && !targets
                && !properties
        }
        CssExecutionProfile::SelectorMatching => {
            has_selector
                && !has_property_stylesheet
                && sheet_count == 0
                && has_html
                && !has_property
                && targets
                && !properties
        }
        CssExecutionProfile::CascadeWinner
        | CssExecutionProfile::InheritanceCssWide
        | CssExecutionProfile::ComputedStyle => {
            !has_selector
                && !has_property_stylesheet
                && sheet_count > 0
                && has_html
                && !has_property
                && targets
                && properties
        }
    };
    valid.then_some(()).ok_or(CssFixtureLoadError::Invalid(
        CssFixtureProblem::ProfileContract {
            reason: "required/optional/forbidden profile fields do not reconcile",
        },
    ))
}

fn primary_path(wire: &WireFixture) -> Option<&str> {
    match wire.profile {
        CssExecutionProfile::PropertyValue => wire.input.stylesheet.as_deref(),
        CssExecutionProfile::CascadeWinner
        | CssExecutionProfile::InheritanceCssWide
        | CssExecutionProfile::ComputedStyle => wire
            .input
            .stylesheets
            .first()
            .map(|input| input.path.as_str()),
        CssExecutionProfile::SelectorParsing
        | CssExecutionProfile::SelectorSpecificity
        | CssExecutionProfile::SelectorMatching => wire.input.selector_list.as_deref(),
    }
}

fn load_optional_text(
    root: &Path,
    path: Option<&str>,
    refs: &mut BTreeSet<String>,
    maximum: usize,
) -> Result<Option<String>, CssFixtureLoadError> {
    path.map(|path| load_text(root, path, refs, maximum))
        .transpose()
}

fn load_text(
    root: &Path,
    relative: &str,
    refs: &mut BTreeSet<String>,
    maximum: usize,
) -> Result<String, CssFixtureLoadError> {
    validate_portable(relative)?;
    if !refs.insert(relative.to_owned()) {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::DuplicateReferencedPath {
                path: relative.to_owned(),
            },
        ));
    }
    let path = root.join(relative);
    ensure_regular(&path, relative)?;
    let bytes = read_bounded(&path, maximum).map_err(|error| match error {
        ReadError::Io(error) => CssFixtureLoadError::Io {
            path: relative.to_owned(),
            error,
        },
        ReadError::TooLarge(actual) => {
            CssFixtureLoadError::Invalid(CssFixtureProblem::InputTooLarge {
                path: relative.to_owned(),
                actual,
                maximum,
            })
        }
    })?;
    String::from_utf8(bytes).map_err(|_| {
        CssFixtureLoadError::Invalid(CssFixtureProblem::NonUtf8 {
            path: relative.to_owned(),
        })
    })
}

fn validate_portable(path: &str) -> Result<(), CssFixtureLoadError> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path).is_absolute()
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::InvalidPortablePath {
                path: path.to_owned(),
            },
        ));
    }
    Ok(())
}

fn ensure_regular(path: &Path, display: &str) -> Result<(), CssFixtureLoadError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        CssFixtureLoadError::Invalid(CssFixtureProblem::NonRegularOrSymlink {
            path: display.to_owned(),
        })
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CssFixtureLoadError::Invalid(
            CssFixtureProblem::NonRegularOrSymlink {
                path: display.to_owned(),
            },
        ));
    }
    Ok(())
}

#[derive(Debug)]
enum ReadError {
    Io(std::io::Error),
    TooLarge(usize),
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, ReadError> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(ReadError::Io)?;
    let mut bytes = Vec::new();
    let read_ceiling = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    file.by_ref()
        .take(read_ceiling)
        .read_to_end(&mut bytes)
        .map_err(ReadError::Io)?;
    if bytes.len() > maximum {
        Err(ReadError::TooLarge(bytes.len()))
    } else {
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_limits() -> CssFixtureLimits {
        CssFixtureLimits::try_new(
            16 * 1024,
            1024 * 1024,
            css::StyleResolutionLimits::default()
                .max_stylesheets_per_style_pass
                .min(16),
        )
        .expect("test fixture limits")
    }

    fn write_package(descriptor: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("fixture.toml"), descriptor).unwrap();
        for (path, contents) in files {
            std::fs::write(directory.path().join(path), contents).unwrap();
        }
        directory
    }

    fn fragment_descriptor(context: &str) -> String {
        format!(
            r#"format = "borrowser-css-fixture-v1"
id = "fragment-case"
profile = "selector-matching"
[input]
selector_list = "selectors.txt"
html = {{ kind = "fragment", path = "fragment.html"{context} }}
[[targets]]
label = "target"
steps = [{{ child_index = 0, expected_namespace = "html", expected_local_name = "div" }}]
[expectations]
snapshot = "expected.txt"
"#
        )
    }

    #[test]
    fn fragment_request_requires_strict_context_namespace_and_local_name() {
        let files = [
            ("selectors.txt", "div"),
            ("fragment.html", "<div></div>"),
            ("expected.txt", "unused"),
        ];
        let valid = write_package(
            &fragment_descriptor(", context = { namespace = \"html\", local_name = \"template\" }"),
            &files,
        );
        let fixture =
            load_fixture_package(&valid.path().join("fixture.toml"), test_limits()).unwrap();
        assert_eq!(fixture.fragment_context().unwrap().local_name(), "template");
        let missing = write_package(&fragment_descriptor(""), &files);
        assert!(matches!(
            load_fixture_package(&missing.path().join("fixture.toml"), test_limits()),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::FragmentContextRequired
            ))
        ));
        let malformed = write_package(
            &fragment_descriptor(", context = { namespace = \"html\", local_name = \"bad name\" }"),
            &files,
        );
        assert!(matches!(
            load_fixture_package(&malformed.path().join("fixture.toml"), test_limits()),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::InvalidFragmentContextLocalName
            ))
        ));
        let invalid_namespace = write_package(
            &fragment_descriptor(
                ", context = { namespace = \"unknown\", local_name = \"template\" }",
            ),
            &files,
        );
        assert!(matches!(
            load_fixture_package(
                &invalid_namespace.path().join("fixture.toml"),
                test_limits()
            ),
            Err(CssFixtureLoadError::Parse { .. })
        ));

        let maximum = html::HtmlTokenizerLimits::default().max_tag_name_bytes;
        let exact_context = format!(
            ", context = {{ namespace = \"html\", local_name = \"{}\" }}",
            "x".repeat(maximum)
        );
        let exact = write_package(&fragment_descriptor(&exact_context), &files);
        assert!(load_fixture_package(&exact.path().join("fixture.toml"), test_limits()).is_ok());
        let oversized_context = format!(
            ", context = {{ namespace = \"html\", local_name = \"{}\" }}",
            "x".repeat(maximum + 1)
        );
        let oversized = write_package(&fragment_descriptor(&oversized_context), &files);
        assert!(matches!(
            load_fixture_package(&oversized.path().join("fixture.toml"), test_limits()),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::InvalidFragmentContextLocalName
            ))
        ));
    }

    fn property_descriptor(input: &str) -> String {
        format!(
            r#"format = "borrowser-css-fixture-v1"
id = "property-case"
profile = "property-value"
[input]
{input}
[property]
rule_index = 0
declaration_index = 0
[expectations]
snapshot = "expected.txt"
"#
        )
    }

    #[test]
    fn property_value_uses_a_phase_specific_stylesheet_carrier() {
        let files = [
            ("input.css", "p { color: red; }"),
            ("expected.txt", "unused"),
        ];
        let valid = write_package(&property_descriptor("stylesheet = \"input.css\""), &files);
        let fixture = load_fixture_package(&valid.path().join("fixture.toml"), test_limits())
            .expect("phase-clean property fixture");
        assert_eq!(
            fixture.property_stylesheet.as_deref(),
            Some("p { color: red; }")
        );
        assert!(fixture.stylesheets.is_empty());
        assert_eq!(fixture.primary_input_path(), "input.css");

        let cascade_carrier = write_package(
            &property_descriptor(
                "stylesheets = [{ path = \"input.css\", origin = \"author\", order = 0, source = 0 }]",
            ),
            &files,
        );
        assert!(matches!(
            load_fixture_package(&cascade_carrier.path().join("fixture.toml"), test_limits()),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::ProfileContract { .. }
            ))
        ));

        let mixed = write_package(
            &property_descriptor(
                "stylesheet = \"input.css\"\nstylesheets = [{ path = \"other.css\", origin = \"author\", order = 0, source = 0 }]",
            ),
            &[
                ("input.css", "p { color: red; }"),
                ("other.css", "p { color: blue; }"),
                ("expected.txt", "unused"),
            ],
        );
        assert!(matches!(
            load_fixture_package(&mixed.path().join("fixture.toml"), test_limits()),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::ProfileContract { .. }
            ))
        ));
    }

    #[test]
    fn file_reader_accepts_exact_limit_and_overflow_safe_maximum() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input");
        std::fs::write(&path, vec![b'x'; 8]).unwrap();
        assert_eq!(read_bounded(&path, 8).unwrap().len(), 8);
        assert!(matches!(
            read_bounded(&path, 7),
            Err(ReadError::TooLarge(8))
        ));
        assert_eq!(read_bounded(&path, usize::MAX).unwrap().len(), 8);
    }

    fn dimension_wire() -> WireFixture {
        WireFixture {
            format: CSS_FIXTURE_FORMAT_V1.to_owned(),
            id: "dimensions".to_owned(),
            profile: CssExecutionProfile::SelectorParsing,
            input: WireInput {
                selector_list: Some("selectors.txt".to_owned()),
                stylesheet: None,
                stylesheets: vec![],
                html: None,
            },
            property: None,
            targets: vec![],
            expectations: WireExpectations {
                snapshot: "expected.txt".to_owned(),
                selected_properties: vec![],
            },
        }
    }

    fn target(label: String, depth: usize) -> CssTargetAddress {
        CssTargetAddress {
            label: crate::target::CssTargetLabel::parse(label).expect("target label"),
            steps: (0..depth)
                .map(|_| crate::target::CssTargetAddressStep {
                    child_index: 0,
                    expected_namespace: CssHostNamespace::Html,
                    expected_local_name: "div".to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn target_and_address_bounds_accept_exact_maximum_and_reject_plus_one() {
        let limits = test_limits();
        let mut wire = dimension_wire();
        wire.targets = (0..limits.max_targets)
            .map(|index| target(format!("target-{index}"), 1))
            .collect();
        validate_dimensions(&wire, limits).expect("exact target maximum");
        wire.targets.push(target("target-overflow".to_owned(), 1));
        assert!(matches!(
            validate_dimensions(&wire, limits),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::TooManyTargets { actual, maximum }
            )) if actual == maximum + 1 && maximum == limits.max_targets
        ));

        let mut wire = dimension_wire();
        wire.targets = vec![target("deep-target".to_owned(), limits.max_target_depth)];
        validate_dimensions(&wire, limits).expect("exact address depth");
        wire.targets[0]
            .steps
            .push(crate::target::CssTargetAddressStep {
                child_index: 0,
                expected_namespace: CssHostNamespace::Html,
                expected_local_name: "div".to_owned(),
            });
        assert!(matches!(
            validate_dimensions(&wire, limits),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::TargetDepthExceeded { actual, maximum }
            )) if actual == maximum + 1 && maximum == limits.max_target_depth
        ));
    }

    #[test]
    fn selected_property_bound_is_derived_from_the_canonical_registry() {
        let limits = test_limits();
        let mut wire = dimension_wire();
        wire.expectations.selected_properties = css::property_registry()
            .entries()
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect();
        validate_dimensions(&wire, limits).expect("complete unique property registry");
        wire.expectations
            .selected_properties
            .push(css::property_registry().entries()[0].name().to_owned());
        assert!(matches!(
            validate_dimensions(&wire, limits),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::TooManySelectedProperties { actual, maximum }
            )) if actual == maximum + 1 && maximum == css::property_registry().entries().len()
        ));
    }

    #[test]
    fn configured_stylesheet_bound_cannot_exceed_the_production_style_pass_bound() {
        let limits = test_limits();
        let production_maximum =
            css::StyleResolutionLimits::default().max_stylesheets_per_style_pass;
        assert!(limits.max_stylesheets <= production_maximum);
        assert!(matches!(
            CssFixtureLimits::try_new(1, 1, production_maximum + 1),
            Err(CssFixtureLimitConfigurationError::StylesheetsExceedProduction {
                configured,
                production_maximum: actual_production,
            }) if configured == production_maximum + 1 && actual_production == production_maximum
        ));
        let mut stylesheets = (0..limits.max_stylesheets)
            .map(|index| WireStylesheet {
                path: format!("s{index}.css"),
                origin: CssStylesheetOrigin::Author,
                order: u32::try_from(index).expect("production limit fits stylesheet order"),
                source: u32::try_from(index).expect("production limit fits source identity"),
                namespace: None,
            })
            .collect::<Vec<_>>();
        validate_stylesheets(&stylesheets, limits).expect("exact configured stylesheet maximum");
        let overflow = u32::try_from(limits.max_stylesheets).expect("limit fits source identity");
        stylesheets.push(WireStylesheet {
            path: "overflow.css".to_owned(),
            origin: CssStylesheetOrigin::Author,
            order: overflow,
            source: overflow,
            namespace: None,
        });
        assert!(matches!(
            validate_stylesheets(&stylesheets, limits),
            Err(CssFixtureLoadError::Invalid(
                CssFixtureProblem::TooManyStylesheets { actual, maximum }
            )) if actual == maximum + 1 && maximum == limits.max_stylesheets
        ));
    }

    fn cascade_descriptor(id: &str, stylesheets: &str) -> String {
        format!(
            r#"format = "borrowser-css-fixture-v1"
id = "{id}"
profile = "cascade-winner"
[input]
stylesheets = [{stylesheets}]
html = {{ kind = "document", path = "document.html" }}
[[targets]]
label = "target"
steps = [
  {{ child_index = 1, expected_namespace = "html", expected_local_name = "html" }},
  {{ child_index = 1, expected_namespace = "html", expected_local_name = "body" }},
  {{ child_index = 0, expected_namespace = "html", expected_local_name = "p" }},
]
[expectations]
snapshot = "expected.txt"
selected_properties = ["color"]
"#,
        )
    }

    fn cascade_expected(value: &str) -> String {
        format!(
            "format: borrowser-css-cascade-winner-observation-v1\ndocument-mode: no-quirks\ntarget: target\n  color: winner source=stylesheet specificity=(0,0,1) value={value}\n"
        )
    }

    #[test]
    fn multiple_stylesheets_preserve_explicit_source_order_in_production_cascade() {
        let expected = cascade_expected("blue");
        let directory = write_package(
            &cascade_descriptor(
                "source-order",
                "{ path = \"first.css\", origin = \"author\", order = 0, source = 0 }, \
                 { path = \"second.css\", origin = \"author\", order = 1, source = 1 }",
            ),
            &[
                ("first.css", "p { color: red; }"),
                ("second.css", "p { color: blue; }"),
                (
                    "document.html",
                    "<!doctype html><html><head></head><body><p></p></body></html>",
                ),
                ("expected.txt", &expected),
            ],
        );
        let fixture =
            load_fixture_package(&directory.path().join("fixture.toml"), test_limits()).unwrap();
        let evaluation = crate::evaluate_fixture(&fixture);
        assert!(
            matches!(
                evaluation,
                crate::CssFixtureEvaluation::Attempted {
                    outcome: crate::CssObservedExecutionOutcome::SemanticPass,
                    ..
                }
            ),
            "{evaluation:?}"
        );
    }

    #[test]
    fn multiple_stylesheets_preserve_supported_origin_order_in_production_cascade() {
        let expected = cascade_expected("blue");
        let directory = write_package(
            &cascade_descriptor(
                "origin-order",
                "{ path = \"author.css\", origin = \"author\", order = 0, source = 0 }, \
                 { path = \"user.css\", origin = \"user\", order = 1, source = 1 }",
            ),
            &[
                ("author.css", "p { color: red !important; }"),
                ("user.css", "p { color: blue !important; }"),
                (
                    "document.html",
                    "<!doctype html><html><head></head><body><p></p></body></html>",
                ),
                ("expected.txt", &expected),
            ],
        );
        let fixture =
            load_fixture_package(&directory.path().join("fixture.toml"), test_limits()).unwrap();
        let evaluation = crate::evaluate_fixture(&fixture);
        assert!(
            matches!(
                evaluation,
                crate::CssFixtureEvaluation::Attempted {
                    outcome: crate::CssObservedExecutionOutcome::SemanticPass,
                    ..
                }
            ),
            "{evaluation:?}"
        );
    }

    #[test]
    fn inline_style_enters_element_attached_production_cascade_without_a_fake_rule() {
        let expected = "format: borrowser-css-cascade-winner-observation-v1\ndocument-mode: no-quirks\ntarget: target\n  color: winner source=inline-style specificity=none value=blue\n";
        let directory = write_package(
            &cascade_descriptor(
                "inline-style",
                "{ path = \"author.css\", origin = \"author\", order = 0, source = 0 }",
            ),
            &[
                ("author.css", "p { color: red; }"),
                (
                    "document.html",
                    "<!doctype html><html><head></head><body><p style=\"color: blue\"></p></body></html>",
                ),
                ("expected.txt", expected),
            ],
        );
        let fixture =
            load_fixture_package(&directory.path().join("fixture.toml"), test_limits()).unwrap();
        let evaluation = crate::evaluate_fixture(&fixture);
        assert!(
            matches!(
                evaluation,
                crate::CssFixtureEvaluation::Attempted {
                    outcome: crate::CssObservedExecutionOutcome::SemanticPass,
                    ..
                }
            ),
            "{evaluation:?}"
        );
    }

    #[test]
    fn structural_target_failure_after_html_parsing_is_an_attempted_terminal_outcome() {
        let directory = write_package(
            &cascade_descriptor(
                "target-structure-regression",
                "{ path = \"author.css\", origin = \"author\", order = 0, source = 0 }",
            ),
            &[
                ("author.css", "p { color: red; }"),
                (
                    "document.html",
                    "<!doctype html><html><head></head><body>text<p></p></body></html>",
                ),
                ("expected.txt", "not reached"),
            ],
        );
        let fixture = load_fixture_package(&directory.path().join("fixture.toml"), test_limits())
            .expect("valid fixture before engine execution");
        let evaluation = crate::evaluate_fixture(&fixture);
        assert!(matches!(
            evaluation,
            crate::CssFixtureEvaluation::Attempted {
                outcome: crate::CssObservedExecutionOutcome::ExecutionFailure {
                    phase: crate::CssExecutionPhase::TargetResolution,
                    failure: crate::CssExecutionFailure::TargetResolution {
                        failure: crate::CssTargetResolutionFailure::ChildIsNotElement {
                            depth: 2,
                            child_index: 0,
                            actual: crate::CssTargetChildKind::Text,
                        },
                        ..
                    },
                },
                observation: None,
            }
        ));
    }

    #[test]
    fn user_agent_origin_uses_the_production_namespace_constrained_input() {
        let expected = cascade_expected("red");
        let directory = write_package(
            &cascade_descriptor(
                "user-agent-origin",
                "{ path = \"ua.css\", origin = \"user-agent\", order = 0, source = 0, namespace = \"html\" }, \
                 { path = \"author.css\", origin = \"author\", order = 1, source = 1 }",
            ),
            &[
                ("ua.css", "p { color: blue; }"),
                ("author.css", "p { color: red; }"),
                (
                    "document.html",
                    "<!doctype html><html><head></head><body><p></p></body></html>",
                ),
                ("expected.txt", &expected),
            ],
        );
        let fixture =
            load_fixture_package(&directory.path().join("fixture.toml"), test_limits()).unwrap();
        let evaluation = crate::evaluate_fixture(&fixture);
        assert!(
            matches!(
                evaluation,
                crate::CssFixtureEvaluation::Attempted {
                    outcome: crate::CssObservedExecutionOutcome::SemanticPass,
                    ..
                }
            ),
            "{evaluation:?}"
        );
    }
}
