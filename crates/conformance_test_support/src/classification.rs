//! Source-neutral AG requirement and limitation vocabulary.
//!
//! AG3 continues to own its V1 wire model. These closed values are shared by
//! logical-test metadata and external source-record assessment without
//! changing their serialized spellings.

const MAX_SEMANTIC_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticIdentifierError;

macro_rules! semantic_identifier {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, SemanticIdentifierError> {
                is_semantic_identifier(value)
                    .then(|| Self(value.to_owned()))
                    .ok_or(SemanticIdentifierError)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

semantic_identifier!(CapabilityFeatureId);
semantic_identifier!(EnvironmentProfileId);

fn is_semantic_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_SEMANTIC_IDENTIFIER_BYTES
        || !bytes[0].is_ascii_lowercase()
        || (!bytes[bytes.len() - 1].is_ascii_lowercase()
            && !bytes[bytes.len() - 1].is_ascii_digit())
    {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in bytes {
        if *byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RequirementTag {
    NoJs,
    RequiresJs,
    RequiresDomApi,
    RequiresNetworking,
    RequiresHtmlParserFeature,
    RequiresCssFeature,
    RequiresLayoutFeature,
    RequiresPaintFeature,
    RequiresFontFeature,
    RequiresBrowserRuntimeFeature,
    RequiresPixelComparison,
    RequiresUserInteraction,
}

impl RequirementTag {
    pub(crate) const ALL: [Self; 12] = [
        Self::NoJs,
        Self::RequiresJs,
        Self::RequiresDomApi,
        Self::RequiresNetworking,
        Self::RequiresHtmlParserFeature,
        Self::RequiresCssFeature,
        Self::RequiresLayoutFeature,
        Self::RequiresPaintFeature,
        Self::RequiresFontFeature,
        Self::RequiresBrowserRuntimeFeature,
        Self::RequiresPixelComparison,
        Self::RequiresUserInteraction,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoJs => "no-js",
            Self::RequiresJs => "requires-js",
            Self::RequiresDomApi => "requires-dom-api",
            Self::RequiresNetworking => "requires-networking",
            Self::RequiresHtmlParserFeature => "requires-html-parser-feature",
            Self::RequiresCssFeature => "requires-css-feature",
            Self::RequiresLayoutFeature => "requires-layout-feature",
            Self::RequiresPaintFeature => "requires-paint-feature",
            Self::RequiresFontFeature => "requires-font-feature",
            Self::RequiresBrowserRuntimeFeature => "requires-browser-runtime-feature",
            Self::RequiresPixelComparison => "requires-pixel-comparison",
            Self::RequiresUserInteraction => "requires-user-interaction",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tag| tag.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EngineCapabilityKind {
    JavaScriptExecution,
    DomApi,
    Networking,
    HtmlParserFeature,
    CssFeature,
    LayoutFeature,
    PaintFeature,
    FontFeature,
    BrowserRuntimeFeature,
    UserInteraction,
}

impl EngineCapabilityKind {
    pub(crate) const ALL: [Self; 10] = [
        Self::JavaScriptExecution,
        Self::DomApi,
        Self::Networking,
        Self::HtmlParserFeature,
        Self::CssFeature,
        Self::LayoutFeature,
        Self::PaintFeature,
        Self::FontFeature,
        Self::BrowserRuntimeFeature,
        Self::UserInteraction,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::JavaScriptExecution => "javascript-execution",
            Self::DomApi => "dom-api",
            Self::Networking => "networking",
            Self::HtmlParserFeature => "html-parser-feature",
            Self::CssFeature => "css-feature",
            Self::LayoutFeature => "layout-feature",
            Self::PaintFeature => "paint-feature",
            Self::FontFeature => "font-feature",
            Self::BrowserRuntimeFeature => "browser-runtime-feature",
            Self::UserInteraction => "user-interaction",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    pub(crate) fn requires_feature(self) -> bool {
        !matches!(self, Self::JavaScriptExecution)
    }

    pub fn requirement_tag(self) -> RequirementTag {
        match self {
            Self::JavaScriptExecution => RequirementTag::RequiresJs,
            Self::DomApi => RequirementTag::RequiresDomApi,
            Self::Networking => RequirementTag::RequiresNetworking,
            Self::HtmlParserFeature => RequirementTag::RequiresHtmlParserFeature,
            Self::CssFeature => RequirementTag::RequiresCssFeature,
            Self::LayoutFeature => RequirementTag::RequiresLayoutFeature,
            Self::PaintFeature => RequirementTag::RequiresPaintFeature,
            Self::FontFeature => RequirementTag::RequiresFontFeature,
            Self::BrowserRuntimeFeature => RequirementTag::RequiresBrowserRuntimeFeature,
            Self::UserInteraction => RequirementTag::RequiresUserInteraction,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HarnessLimitationKind {
    MissingSubsystemAdapter,
    UnsupportedSourceFormat,
    MissingExpectedObservation,
    UnsupportedExpectationRepresentation,
    MissingObservationSurface,
    MissingComparisonSurface,
    MissingEnvironmentDescription,
    MissingEnvironmentProvisioning,
}

impl HarnessLimitationKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::MissingSubsystemAdapter,
        Self::UnsupportedSourceFormat,
        Self::MissingExpectedObservation,
        Self::UnsupportedExpectationRepresentation,
        Self::MissingObservationSurface,
        Self::MissingComparisonSurface,
        Self::MissingEnvironmentDescription,
        Self::MissingEnvironmentProvisioning,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingSubsystemAdapter => "missing-subsystem-adapter",
            Self::UnsupportedSourceFormat => "unsupported-source-format",
            Self::MissingExpectedObservation => "missing-expected-observation",
            Self::UnsupportedExpectationRepresentation => "unsupported-expectation-representation",
            Self::MissingObservationSurface => "missing-observation-surface",
            Self::MissingComparisonSurface => "missing-comparison-surface",
            Self::MissingEnvironmentDescription => "missing-environment-description",
            Self::MissingEnvironmentProvisioning => "missing-environment-provisioning",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnvironmentRequirementKind {
    ControlledFontSet,
    ViewportConfiguration,
    DeviceScale,
    PlatformConfiguration,
    ControlledResources,
    ExternalBrowser,
    PixelCaptureEnvironment,
    UserInteractionEnvironment,
}

impl EnvironmentRequirementKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::ControlledFontSet,
        Self::ViewportConfiguration,
        Self::DeviceScale,
        Self::PlatformConfiguration,
        Self::ControlledResources,
        Self::ExternalBrowser,
        Self::PixelCaptureEnvironment,
        Self::UserInteractionEnvironment,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ControlledFontSet => "controlled-font-set",
            Self::ViewportConfiguration => "viewport-configuration",
            Self::DeviceScale => "device-scale",
            Self::PlatformConfiguration => "platform-configuration",
            Self::ControlledResources => "controlled-resources",
            Self::ExternalBrowser => "external-browser",
            Self::PixelCaptureEnvironment => "pixel-capture-environment",
            Self::UserInteractionEnvironment => "user-interaction-environment",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}
