use std::fmt;

pub const CONFORMANCE_FIXTURE_FORMAT_V1: &str = "borrowser-conformance-fixture-v1";
pub const CONFORMANCE_FIXTURE_FORMAT_V2: &str = "borrowser-conformance-fixture-v2";
pub const MAX_DESCRIPTOR_BYTES: u64 = 64 * 1024;
pub const MAX_EXECUTION_SUPPORT_PATHS_V2: usize = 256;
pub(crate) const MAX_PORTABLE_PATH_COMPONENT_BYTES: usize = 128;
const MAX_TEST_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FixtureFormat {
    V1,
    V2,
}

impl FixtureFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1 => CONFORMANCE_FIXTURE_FORMAT_V1,
            Self::V2 => CONFORMANCE_FIXTURE_FORMAT_V2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PortablePathComponent(String);

impl PortablePathComponent {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let bytes = value.as_bytes();
        if bytes.is_empty()
            || bytes.len() > MAX_PORTABLE_PATH_COMPONENT_BYTES
            || !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit()
            || !bytes[bytes.len() - 1].is_ascii_lowercase()
                && !bytes[bytes.len() - 1].is_ascii_digit()
            || bytes.windows(2).any(|pair| pair == b"..")
            || !bytes.iter().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'-' | b'_' | b'.')
            })
            || is_windows_reserved_basename(value)
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_windows_reserved_basename(value: &str) -> bool {
    let basename = value.split('.').next().unwrap_or(value);
    matches!(basename, "con" | "prn" | "aux" | "nul")
        || basename.len() == 4
            && (basename.starts_with("com") || basename.starts_with("lpt"))
            && matches!(basename.as_bytes()[3], b'1'..=b'9')
}

#[cfg(test)]
mod portable_path_component_tests {
    use super::*;

    #[test]
    fn v1_portable_component_grammar_is_explicit() {
        for value in ["a", "0", "fixture.toml", "test-file_1.html", "com10"] {
            assert!(PortablePathComponent::parse(value).is_some(), "{value:?}");
        }
        for value in [
            "",
            ".",
            "..",
            ".hidden",
            "trailing.",
            "trailing ",
            "two..dots",
            "Uppercase",
            "unicode-é",
            "control-\n",
            "contains\\backslash",
            "contains:colon",
            "con",
            "con.txt",
            "prn",
            "aux.css",
            "nul",
            "com1.html",
            "com9",
            "lpt1.txt",
            "lpt9",
        ] {
            assert!(PortablePathComponent::parse(value).is_none(), "{value:?}");
        }
        assert!(PortablePathComponent::parse(&"a".repeat(128)).is_some());
        assert!(PortablePathComponent::parse(&"a".repeat(129)).is_none());
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestId(String);

impl TestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parse(value: &str) -> Result<Self, TestIdValidationError> {
        if value.len() > MAX_TEST_ID_BYTES {
            return Err(TestIdValidationError::TooLong);
        }
        if value != value.to_ascii_lowercase() {
            return Err(TestIdValidationError::CaseUnsafe);
        }
        if !is_kebab_identifier(value) {
            return Err(TestIdValidationError::InvalidGrammar);
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for TestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestIdValidationError {
    TooLong,
    CaseUnsafe,
    InvalidGrammar,
}

fn is_kebab_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
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
    !previous_hyphen
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InventoryScope {
    StaticHtmlCssNoJs,
}

impl InventoryScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StaticHtmlCssNoJs => "static-html-css-no-js",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "static-html-css-no-js" => Some(Self::StaticHtmlCssNoJs),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservationSurface {
    HtmlTokenizer,
    HtmlTreeConstruction,
    DomTree,
    CssParsing,
    CssSelectors,
    CssCascade,
    ComputedStyle,
    LayoutGeometry,
    PaintOperations,
    BrowserRuntimeSemantic,
}

impl ObservationSurface {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HtmlTokenizer => "html-tokenizer",
            Self::HtmlTreeConstruction => "html-tree-construction",
            Self::DomTree => "dom-tree",
            Self::CssParsing => "css-parsing",
            Self::CssSelectors => "css-selectors",
            Self::CssCascade => "css-cascade",
            Self::ComputedStyle => "computed-style",
            Self::LayoutGeometry => "layout-geometry",
            Self::PaintOperations => "paint-operations",
            Self::BrowserRuntimeSemantic => "browser-runtime-semantic",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "html-tokenizer" => Some(Self::HtmlTokenizer),
            "html-tree-construction" => Some(Self::HtmlTreeConstruction),
            "dom-tree" => Some(Self::DomTree),
            "css-parsing" => Some(Self::CssParsing),
            "css-selectors" => Some(Self::CssSelectors),
            "css-cascade" => Some(Self::CssCascade),
            "computed-style" => Some(Self::ComputedStyle),
            "layout-geometry" => Some(Self::LayoutGeometry),
            "paint-operations" => Some(Self::PaintOperations),
            "browser-runtime-semantic" => Some(Self::BrowserRuntimeSemantic),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    Native,
    ControlledStaticPage,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::ControlledStaticPage => "controlled-static-page",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "controlled-static-page" => Some(Self::ControlledStaticPage),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceKind {
    Semantic,
    Structural,
}

impl ReferenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Structural => "structural",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "semantic" => Some(Self::Semantic),
            "structural" => Some(Self::Structural),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryPath(String);

impl RepositoryPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validated(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for RepositoryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceDeclaration {
    kind: ReferenceKind,
    path: RepositoryPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionPackage {
    entry_path: RepositoryPath,
    support_paths: Vec<RepositoryPath>,
}

impl ExecutionPackage {
    pub fn entry_path(&self) -> &RepositoryPath {
        &self.entry_path
    }

    pub fn support_paths(&self) -> &[RepositoryPath] {
        &self.support_paths
    }

    pub(crate) fn validated(
        entry_path: RepositoryPath,
        mut support_paths: Vec<RepositoryPath>,
    ) -> Self {
        support_paths.sort();
        Self {
            entry_path,
            support_paths,
        }
    }
}

impl ReferenceDeclaration {
    pub fn kind(&self) -> ReferenceKind {
        self.kind
    }

    pub fn path(&self) -> &RepositoryPath {
        &self.path
    }

    pub(crate) fn validated(kind: ReferenceKind, path: RepositoryPath) -> Self {
        Self { kind, path }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedFixture {
    format: FixtureFormat,
    id: TestId,
    fixture_path: RepositoryPath,
    test_path: RepositoryPath,
    metadata_path: RepositoryPath,
    scope: InventoryScope,
    observation: ObservationSurface,
    source_kind: SourceKind,
    reference: Option<ReferenceDeclaration>,
    execution_package: Option<ExecutionPackage>,
    description: String,
}

impl ValidatedFixture {
    pub fn format(&self) -> FixtureFormat {
        self.format
    }

    pub fn id(&self) -> &TestId {
        &self.id
    }

    pub fn fixture_path(&self) -> &RepositoryPath {
        &self.fixture_path
    }

    pub fn test_path(&self) -> &RepositoryPath {
        &self.test_path
    }

    pub fn metadata_path(&self) -> &RepositoryPath {
        &self.metadata_path
    }

    pub fn scope(&self) -> InventoryScope {
        self.scope
    }

    pub fn observation(&self) -> ObservationSurface {
        self.observation
    }

    pub fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    pub fn reference(&self) -> Option<&ReferenceDeclaration> {
        self.reference.as_ref()
    }

    pub fn execution_package(&self) -> Option<&ExecutionPackage> {
        self.execution_package.as_ref()
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validated(
        format: FixtureFormat,
        id: TestId,
        fixture_path: RepositoryPath,
        test_path: RepositoryPath,
        metadata_path: RepositoryPath,
        scope: InventoryScope,
        observation: ObservationSurface,
        source_kind: SourceKind,
        reference: Option<ReferenceDeclaration>,
        execution_package: Option<ExecutionPackage>,
        description: String,
    ) -> Self {
        Self {
            format,
            id,
            fixture_path,
            test_path,
            metadata_path,
            scope,
            observation,
            source_kind,
            reference,
            execution_package,
            description,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedInventory {
    fixtures: Vec<ValidatedFixture>,
}

impl ValidatedInventory {
    pub fn fixtures(&self) -> &[ValidatedFixture] {
        &self.fixtures
    }

    pub(crate) fn validated(fixtures: Vec<ValidatedFixture>) -> Self {
        Self { fixtures }
    }
}
