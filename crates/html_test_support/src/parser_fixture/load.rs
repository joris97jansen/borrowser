use super::model::{FixtureBundle, FixtureId};
use super::schema::FixtureFileV1;
use super::validate::{ValidatedFixtureSpec, validate_fixture};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureRepositoryPolicy {
    NativeConformance,
    AdaptedOrQuarantine,
}

#[derive(Clone, Debug)]
pub struct FixtureRepository {
    pub repository_root: PathBuf,
    pub fixture_root: PathBuf,
    pub policy: FixtureRepositoryPolicy,
}

impl FixtureRepository {
    pub fn native(repository_root: impl Into<PathBuf>, fixture_root: impl Into<PathBuf>) -> Self {
        Self {
            repository_root: repository_root.into(),
            fixture_root: fixture_root.into(),
            policy: FixtureRepositoryPolicy::NativeConformance,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureLoadError {
    pub path: String,
    pub kind: FixtureLoadErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixtureLoadErrorKind {
    FixtureRootOutsideRepository,
    NonUtf8Path,
    SymlinkNotAllowed,
    Io(String),
    InvalidFixtureToml(String),
    UnsupportedFixtureFormat(String),
    InvalidFixtureId(String),
    CaseUnsafeFixtureId(String),
    CaseCollidingFixtureId(String),
    DuplicateFixtureId(String),
    CaseCollidingBundlePath(String),
    NestedFixtureBundle(String),
    UnsafeRelativePath(String),
    MissingDeclaredFile(String),
    DeclaredPathNotFile(String),
    OrphanSidecar(String),
    InvalidSha256(String),
    Sha256Mismatch { expected: String, actual: String },
    InvalidUtf8TextInput,
    CarriageReturnInTextInput,
    InvalidInputExtension,
    InvalidExtensionId(String),
    InvalidDisposition(String),
    InvalidCombination(String),
}

impl std::fmt::Display for FixtureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fixture {}: ", self.path)?;
        match &self.kind {
            FixtureLoadErrorKind::FixtureRootOutsideRepository => {
                f.write_str("fixture root is outside the repository root")
            }
            FixtureLoadErrorKind::NonUtf8Path => f.write_str("path is not valid UTF-8"),
            FixtureLoadErrorKind::SymlinkNotAllowed => {
                f.write_str("symlinked fixture paths are not allowed")
            }
            FixtureLoadErrorKind::Io(message) => write!(f, "I/O error: {message}"),
            FixtureLoadErrorKind::InvalidFixtureToml(message) => {
                write!(f, "invalid fixture-v1 TOML: {message}")
            }
            FixtureLoadErrorKind::UnsupportedFixtureFormat(value) => {
                write!(f, "unsupported fixture format '{value}'")
            }
            FixtureLoadErrorKind::InvalidFixtureId(value) => {
                write!(f, "invalid fixture id '{value}'")
            }
            FixtureLoadErrorKind::CaseUnsafeFixtureId(value) => {
                write!(
                    f,
                    "fixture id is not lowercase and case-portable: '{value}'"
                )
            }
            FixtureLoadErrorKind::CaseCollidingFixtureId(value) => {
                write!(f, "fixture IDs collide case-insensitively: {value}")
            }
            FixtureLoadErrorKind::DuplicateFixtureId(value) => {
                write!(f, "duplicate fixture id '{value}'")
            }
            FixtureLoadErrorKind::CaseCollidingBundlePath(value) => {
                write!(
                    f,
                    "fixture bundle path has a case-insensitive collision: '{value}'"
                )
            }
            FixtureLoadErrorKind::NestedFixtureBundle(value) => {
                write!(f, "nested fixture bundle is not allowed: '{value}'")
            }
            FixtureLoadErrorKind::UnsafeRelativePath(value) => {
                write!(f, "unsafe bundle-relative path '{value}'")
            }
            FixtureLoadErrorKind::MissingDeclaredFile(value) => {
                write!(f, "missing declared file '{value}'")
            }
            FixtureLoadErrorKind::DeclaredPathNotFile(value) => {
                write!(f, "declared path is not a regular file: '{value}'")
            }
            FixtureLoadErrorKind::OrphanSidecar(value) => {
                write!(f, "recognized snapshot sidecar is not declared: '{value}'")
            }
            FixtureLoadErrorKind::InvalidSha256(value) => {
                write!(
                    f,
                    "invalid SHA-256 '{value}'; expected 64 lowercase hex digits"
                )
            }
            FixtureLoadErrorKind::Sha256Mismatch { expected, actual } => {
                write!(
                    f,
                    "input SHA-256 mismatch: expected {expected}, actual {actual}"
                )
            }
            FixtureLoadErrorKind::InvalidUtf8TextInput => {
                f.write_str("input declared as UTF-8 text contains invalid UTF-8")
            }
            FixtureLoadErrorKind::CarriageReturnInTextInput => f.write_str(
                "UTF-8 text input contains a carriage return; CRLF, lone CR, and trailing CR fixtures must use input.bin",
            ),
            FixtureLoadErrorKind::InvalidInputExtension => {
                f.write_str("input extension does not match its declared kind")
            }
            FixtureLoadErrorKind::InvalidExtensionId(value) => {
                write!(f, "invalid versioned extension id '{value}'")
            }
            FixtureLoadErrorKind::InvalidDisposition(message) => {
                write!(f, "invalid fixture disposition: {message}")
            }
            FixtureLoadErrorKind::InvalidCombination(message) => {
                write!(f, "invalid fixture combination: {message}")
            }
        }
    }
}

impl std::error::Error for FixtureLoadError {}

pub fn discover_and_load(
    repository: &FixtureRepository,
) -> Result<Vec<ValidatedFixtureSpec>, FixtureLoadError> {
    let fixture_root_relative = repository
        .fixture_root
        .strip_prefix(&repository.repository_root)
        .map_err(|_| {
            error(
                &repository.fixture_root,
                FixtureLoadErrorKind::FixtureRootOutsideRepository,
            )
        })?;
    let root_display = normalize_relative_path(fixture_root_relative)?;
    reject_symlink(&repository.fixture_root, &root_display)?;

    let mut bundles = Vec::new();
    discover_recursive(
        &repository.repository_root,
        &repository.fixture_root,
        &mut bundles,
    )?;
    bundles.sort_by(|left, right| {
        left.repository_relative_path()
            .cmp(right.repository_relative_path())
    });

    let mut case_paths = BTreeSet::new();
    for bundle in &bundles {
        let folded = bundle.repository_relative_path().to_ascii_lowercase();
        if !case_paths.insert(folded) {
            return Err(FixtureLoadError {
                path: bundle.repository_relative_path().to_string(),
                kind: FixtureLoadErrorKind::CaseCollidingBundlePath(
                    bundle.repository_relative_path().to_string(),
                ),
            });
        }
    }

    let mut declarations = Vec::with_capacity(bundles.len());
    let mut case_ids = BTreeMap::<String, (String, String)>::new();
    for bundle in bundles {
        let fixture_toml = read_regular_file(&bundle, "fixture.toml")?;
        let fixture_text = std::str::from_utf8(&fixture_toml).map_err(|_| FixtureLoadError {
            path: format!("{}/fixture.toml", bundle.repository_relative_path()),
            kind: FixtureLoadErrorKind::InvalidFixtureToml(
                "fixture metadata must be UTF-8".to_string(),
            ),
        })?;
        let parsed: FixtureFileV1 =
            toml::from_str(fixture_text).map_err(|err| FixtureLoadError {
                path: format!("{}/fixture.toml", bundle.repository_relative_path()),
                kind: FixtureLoadErrorKind::InvalidFixtureToml(err.to_string()),
            })?;
        let folded = parsed.id.to_ascii_lowercase();
        if let Some((first_id, first_path)) = case_ids.insert(
            folded,
            (
                parsed.id.clone(),
                bundle.repository_relative_path().to_string(),
            ),
        ) {
            let kind = if first_id == parsed.id {
                FixtureLoadErrorKind::DuplicateFixtureId(format!(
                    "{} (first declared at {first_path})",
                    parsed.id
                ))
            } else {
                FixtureLoadErrorKind::CaseCollidingFixtureId(format!(
                    "'{}' at {first_path} and '{}'",
                    first_id, parsed.id
                ))
            };
            return Err(FixtureLoadError {
                path: bundle.repository_relative_path().to_string(),
                kind,
            });
        }
        declarations.push((bundle, parsed));
    }

    let mut loaded = Vec::with_capacity(declarations.len());
    let mut ids = BTreeMap::<FixtureId, String>::new();
    for (bundle, parsed) in declarations {
        let fixture = validate_fixture(parsed, bundle, repository.policy)?;
        if let Some(first_path) = ids.insert(
            fixture.id().clone(),
            fixture.repository_relative_path().to_string(),
        ) {
            return Err(FixtureLoadError {
                path: fixture.repository_relative_path().to_string(),
                kind: FixtureLoadErrorKind::DuplicateFixtureId(format!(
                    "{} (first declared at {first_path})",
                    fixture.id().as_str()
                )),
            });
        }
        loaded.push(fixture);
    }
    Ok(loaded)
}

fn discover_recursive(
    repository_root: &Path,
    directory: &Path,
    bundles: &mut Vec<FixtureBundle>,
) -> Result<(), FixtureLoadError> {
    let relative = directory.strip_prefix(repository_root).map_err(|_| {
        error(
            directory,
            FixtureLoadErrorKind::FixtureRootOutsideRepository,
        )
    })?;
    let display = normalize_relative_path(relative)?;
    reject_symlink(directory, &display)?;

    let fixture_file = directory.join("fixture.toml");
    if fixture_file.exists() {
        reject_symlink(&fixture_file, &format!("{display}/fixture.toml"))?;
        reject_nested_fixture_bundles(repository_root, directory, true)?;
        bundles.push(FixtureBundle::validated(display, directory.to_path_buf()));
        return Ok(());
    }

    let entries = fs::read_dir(directory)
        .map_err(|err| error(directory, FixtureLoadErrorKind::Io(err.to_string())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| error(directory, FixtureLoadErrorKind::Io(err.to_string())))?;
    let mut entries = entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| FixtureLoadError {
                    path: display.clone(),
                    kind: FixtureLoadErrorKind::NonUtf8Path,
                })?;
            Ok((name, entry))
        })
        .collect::<Result<Vec<_>, FixtureLoadError>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, entry) in entries {
        let path = entry.path();
        let child_relative = path
            .strip_prefix(repository_root)
            .map_err(|_| error(&path, FixtureLoadErrorKind::FixtureRootOutsideRepository))?;
        let child_display = normalize_relative_path(child_relative)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| error(&path, FixtureLoadErrorKind::Io(err.to_string())))?;
        if metadata.file_type().is_symlink() {
            return Err(FixtureLoadError {
                path: child_display,
                kind: FixtureLoadErrorKind::SymlinkNotAllowed,
            });
        }
        if metadata.is_dir() {
            discover_recursive(repository_root, &path, bundles)?;
        }
    }
    Ok(())
}

fn reject_nested_fixture_bundles(
    repository_root: &Path,
    directory: &Path,
    bundle_root: bool,
) -> Result<(), FixtureLoadError> {
    let relative = directory.strip_prefix(repository_root).map_err(|_| {
        error(
            directory,
            FixtureLoadErrorKind::FixtureRootOutsideRepository,
        )
    })?;
    let display = normalize_relative_path(relative)?;
    let entries = fs::read_dir(directory)
        .map_err(|err| error(directory, FixtureLoadErrorKind::Io(err.to_string())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| error(directory, FixtureLoadErrorKind::Io(err.to_string())))?;
    let mut entries = entries
        .into_iter()
        .map(|entry| {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| FixtureLoadError {
                    path: display.clone(),
                    kind: FixtureLoadErrorKind::NonUtf8Path,
                })?;
            Ok((name, entry))
        })
        .collect::<Result<Vec<_>, FixtureLoadError>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, entry) in entries {
        let path = entry.path();
        let child_relative = path
            .strip_prefix(repository_root)
            .map_err(|_| error(&path, FixtureLoadErrorKind::FixtureRootOutsideRepository))?;
        let child_display = normalize_relative_path(child_relative)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| error(&path, FixtureLoadErrorKind::Io(err.to_string())))?;
        if metadata.file_type().is_symlink() {
            return Err(FixtureLoadError {
                path: child_display,
                kind: FixtureLoadErrorKind::SymlinkNotAllowed,
            });
        }
        if !bundle_root && name == "fixture.toml" {
            return Err(FixtureLoadError {
                path: child_display.clone(),
                kind: FixtureLoadErrorKind::NestedFixtureBundle(child_display),
            });
        }
        if metadata.is_dir() {
            reject_nested_fixture_bundles(repository_root, &path, false)?;
        }
    }
    Ok(())
}

pub(super) fn read_regular_file(
    bundle: &FixtureBundle,
    relative: &str,
) -> Result<Vec<u8>, FixtureLoadError> {
    validate_relative_path(relative).map_err(|kind| FixtureLoadError {
        path: bundle.repository_relative_path().to_string(),
        kind,
    })?;
    let mut current = bundle.absolute_path().to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(segment) = component else {
            unreachable!("relative path was validated")
        };
        current.push(segment);
        let display = format!(
            "{}/{}",
            bundle.repository_relative_path(),
            normalize_relative_path(Path::new(relative))?
        );
        let metadata = fs::symlink_metadata(&current).map_err(|err| {
            let kind = if err.kind() == std::io::ErrorKind::NotFound {
                FixtureLoadErrorKind::MissingDeclaredFile(relative.to_string())
            } else {
                FixtureLoadErrorKind::Io(err.to_string())
            };
            FixtureLoadError {
                path: display.clone(),
                kind,
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(FixtureLoadError {
                path: display,
                kind: FixtureLoadErrorKind::SymlinkNotAllowed,
            });
        }
    }
    let metadata = fs::metadata(&current).map_err(|err| FixtureLoadError {
        path: bundle.repository_relative_path().to_string(),
        kind: FixtureLoadErrorKind::Io(err.to_string()),
    })?;
    if !metadata.is_file() {
        return Err(FixtureLoadError {
            path: bundle.repository_relative_path().to_string(),
            kind: FixtureLoadErrorKind::DeclaredPathNotFile(relative.to_string()),
        });
    }
    fs::read(&current).map_err(|err| FixtureLoadError {
        path: bundle.repository_relative_path().to_string(),
        kind: FixtureLoadErrorKind::Io(err.to_string()),
    })
}

pub(super) fn validate_relative_path(relative: &str) -> Result<(), FixtureLoadErrorKind> {
    if relative.is_empty() || relative.contains('\\') || relative.contains(':') {
        return Err(FixtureLoadErrorKind::UnsafeRelativePath(
            relative.to_string(),
        ));
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FixtureLoadErrorKind::UnsafeRelativePath(
            relative.to_string(),
        ));
    }
    Ok(())
}

pub(super) fn normalize_relative_path(path: &Path) -> Result<String, FixtureLoadError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or_else(|| FixtureLoadError {
                    path: "<non-utf8-path>".to_string(),
                    kind: FixtureLoadErrorKind::NonUtf8Path,
                })?;
                parts.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(FixtureLoadError {
                    path: path.display().to_string(),
                    kind: FixtureLoadErrorKind::UnsafeRelativePath(path.display().to_string()),
                });
            }
        }
    }
    Ok(parts.join("/"))
}

fn reject_symlink(path: &Path, display: &str) -> Result<(), FixtureLoadError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| error(path, FixtureLoadErrorKind::Io(err.to_string())))?;
    if metadata.file_type().is_symlink() {
        return Err(FixtureLoadError {
            path: display.to_string(),
            kind: FixtureLoadErrorKind::SymlinkNotAllowed,
        });
    }
    Ok(())
}

fn error(path: &Path, kind: FixtureLoadErrorKind) -> FixtureLoadError {
    FixtureLoadError {
        path: path.to_string_lossy().replace('\\', "/"),
        kind,
    }
}
