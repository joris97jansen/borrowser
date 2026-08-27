use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryDiagnostic {
    pub path: String,
    pub kind: InventoryDiagnosticKind,
}

impl InventoryDiagnostic {
    pub(crate) fn new(path: impl Into<String>, kind: InventoryDiagnosticKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    pub(crate) fn sort_key(&self) -> (&str, u8, String) {
        (&self.path, self.kind.rank(), self.kind.detail_key())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryDiagnosticKind {
    RepositoryRootNotDirectory,
    FixtureRootOutsideRepository,
    FixtureRootNotDirectory,
    SymlinkNotAllowed,
    NonUtf8Path,
    NonPortablePathComponent {
        value: String,
    },
    MissingFixtureDescriptor,
    NestedFixtureBundle,
    UndeclaredBundleFile,
    NonRegularBundleEntry,
    DescriptorTooLarge {
        observed_at_least: u64,
        maximum: u64,
    },
    ReadFailed {
        operation: &'static str,
    },
    MalformedToml,
    UnsupportedDescriptorVersion {
        value: String,
    },
    UnknownDescriptorField {
        field: String,
    },
    InvalidDescriptorShape,
    InvalidTestId {
        value: String,
    },
    TestIdTooLong {
        value: String,
    },
    CaseUnsafeTestId {
        value: String,
    },
    InvalidScope {
        value: String,
    },
    InvalidObservation {
        value: String,
    },
    InvalidSourceKind {
        value: String,
    },
    InvalidReferenceKind {
        value: String,
    },
    EmptyDescription,
    InvalidRelativePath {
        field: &'static str,
        value: String,
    },
    MissingDeclaredFile {
        field: &'static str,
        value: String,
    },
    DeclaredPathNotRegularFile {
        field: &'static str,
        value: String,
    },
    DuplicateTestId {
        value: String,
        first_path: String,
    },
    CaseCollidingTestId {
        value: String,
        first_value: String,
        first_path: String,
    },
}

impl InventoryDiagnosticKind {
    pub(crate) fn rank(&self) -> u8 {
        match self {
            Self::RepositoryRootNotDirectory
            | Self::FixtureRootOutsideRepository
            | Self::FixtureRootNotDirectory => 1,
            Self::SymlinkNotAllowed | Self::NonUtf8Path | Self::NonPortablePathComponent { .. } => {
                2
            }
            Self::MissingFixtureDescriptor
            | Self::NestedFixtureBundle
            | Self::UndeclaredBundleFile
            | Self::NonRegularBundleEntry => 3,
            Self::DescriptorTooLarge { .. } | Self::ReadFailed { .. } | Self::MalformedToml => 4,
            Self::UnsupportedDescriptorVersion { .. }
            | Self::UnknownDescriptorField { .. }
            | Self::InvalidDescriptorShape => 5,
            Self::InvalidTestId { .. }
            | Self::TestIdTooLong { .. }
            | Self::CaseUnsafeTestId { .. }
            | Self::InvalidScope { .. }
            | Self::InvalidObservation { .. }
            | Self::InvalidSourceKind { .. }
            | Self::InvalidReferenceKind { .. }
            | Self::EmptyDescription => 6,
            Self::InvalidRelativePath { .. }
            | Self::MissingDeclaredFile { .. }
            | Self::DeclaredPathNotRegularFile { .. } => 7,
            Self::DuplicateTestId { .. } | Self::CaseCollidingTestId { .. } => 9,
        }
    }

    pub(crate) fn detail_key(&self) -> String {
        match self {
            Self::NonPortablePathComponent { value } => value.clone(),
            Self::DescriptorTooLarge {
                observed_at_least,
                maximum,
            } => format!("{observed_at_least:020}:{maximum:020}"),
            Self::ReadFailed { operation } => (*operation).to_owned(),
            Self::UnsupportedDescriptorVersion { value }
            | Self::InvalidTestId { value }
            | Self::TestIdTooLong { value }
            | Self::CaseUnsafeTestId { value }
            | Self::InvalidScope { value }
            | Self::InvalidObservation { value }
            | Self::InvalidSourceKind { value }
            | Self::InvalidReferenceKind { value } => value.clone(),
            Self::UnknownDescriptorField { field } => field.clone(),
            Self::InvalidRelativePath { field, value }
            | Self::MissingDeclaredFile { field, value }
            | Self::DeclaredPathNotRegularFile { field, value } => format!("{field}:{value}"),
            Self::DuplicateTestId { value, first_path } => format!("{value}:{first_path}"),
            Self::CaseCollidingTestId {
                value,
                first_value,
                first_path,
            } => format!("{value}:{first_value}:{first_path}"),
            _ => String::new(),
        }
    }
}

impl fmt::Display for InventoryDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conformance fixture {}: ", self.path)?;
        match &self.kind {
            InventoryDiagnosticKind::RepositoryRootNotDirectory => {
                f.write_str("repository root is not a regular directory")
            }
            InventoryDiagnosticKind::FixtureRootOutsideRepository => {
                f.write_str("fixture root is outside the repository root")
            }
            InventoryDiagnosticKind::FixtureRootNotDirectory => {
                f.write_str("fixture root is not a regular directory")
            }
            InventoryDiagnosticKind::SymlinkNotAllowed => f.write_str("symlinks are not allowed"),
            InventoryDiagnosticKind::NonUtf8Path => f.write_str("path is not valid UTF-8"),
            InventoryDiagnosticKind::NonPortablePathComponent { value } => {
                write!(
                    f,
                    "path component is outside the V1 portable grammar: {value:?}"
                )
            }
            InventoryDiagnosticKind::MissingFixtureDescriptor => {
                f.write_str("files outside a fixture bundle require fixture.toml metadata")
            }
            InventoryDiagnosticKind::NestedFixtureBundle => {
                f.write_str("fixture.toml is nested beneath another fixture bundle")
            }
            InventoryDiagnosticKind::UndeclaredBundleFile => {
                f.write_str("regular file is not declared by fixture.toml")
            }
            InventoryDiagnosticKind::NonRegularBundleEntry => {
                f.write_str("bundle entry is neither a regular file nor a directory")
            }
            InventoryDiagnosticKind::DescriptorTooLarge {
                observed_at_least,
                maximum,
            } => write!(
                f,
                "fixture.toml is at least {observed_at_least} bytes, above the {maximum}-byte metadata limit"
            ),
            InventoryDiagnosticKind::ReadFailed { operation } => {
                write!(f, "filesystem operation failed: {operation}")
            }
            InventoryDiagnosticKind::MalformedToml => f.write_str("fixture.toml is malformed TOML"),
            InventoryDiagnosticKind::UnsupportedDescriptorVersion { value } => {
                write!(f, "unsupported fixture descriptor format '{value}'")
            }
            InventoryDiagnosticKind::UnknownDescriptorField { field } => {
                write!(f, "unknown fixture descriptor field '{field}'")
            }
            InventoryDiagnosticKind::InvalidDescriptorShape => {
                f.write_str("fixture.toml does not match the v1 descriptor shape")
            }
            InventoryDiagnosticKind::InvalidTestId { value } => {
                write!(f, "invalid test id '{value}'")
            }
            InventoryDiagnosticKind::TestIdTooLong { value } => {
                write!(f, "test id is longer than 128 ASCII bytes: '{value}'")
            }
            InventoryDiagnosticKind::CaseUnsafeTestId { value } => {
                write!(f, "test id must be lowercase ASCII kebab case: '{value}'")
            }
            InventoryDiagnosticKind::InvalidScope { value } => {
                write!(f, "invalid inventory scope '{value}'")
            }
            InventoryDiagnosticKind::InvalidObservation { value } => {
                write!(f, "invalid observation surface '{value}'")
            }
            InventoryDiagnosticKind::InvalidSourceKind { value } => {
                write!(f, "invalid source kind '{value}'")
            }
            InventoryDiagnosticKind::InvalidReferenceKind { value } => {
                write!(f, "invalid reference kind '{value}'")
            }
            InventoryDiagnosticKind::EmptyDescription => {
                f.write_str("metadata.description must be non-empty")
            }
            InventoryDiagnosticKind::InvalidRelativePath { field, value } => {
                write!(f, "{field} is not a safe bundle-relative path: '{value}'")
            }
            InventoryDiagnosticKind::MissingDeclaredFile { field, value } => {
                write!(f, "{field} does not exist: '{value}'")
            }
            InventoryDiagnosticKind::DeclaredPathNotRegularFile { field, value } => {
                write!(f, "{field} is not a regular file: '{value}'")
            }
            InventoryDiagnosticKind::DuplicateTestId { value, first_path } => write!(
                f,
                "duplicate test id '{value}' first declared at '{first_path}'"
            ),
            InventoryDiagnosticKind::CaseCollidingTestId {
                value,
                first_value,
                first_path,
            } => write!(
                f,
                "test id '{value}' collides case-insensitively with '{first_value}' at '{first_path}'"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryErrors {
    diagnostics: Vec<InventoryDiagnostic>,
}

impl InventoryErrors {
    pub fn diagnostics(&self) -> &[InventoryDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn sorted(mut diagnostics: Vec<InventoryDiagnostic>) -> Self {
        diagnostics.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        Self { diagnostics }
    }
}

impl fmt::Display for InventoryErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for InventoryErrors {}
