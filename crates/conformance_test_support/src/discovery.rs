use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::descriptor::{ParsedDescriptor, parse_descriptor};
use crate::diagnostic::{InventoryDiagnostic, InventoryDiagnosticKind, InventoryErrors};
use crate::model::{
    MAX_DESCRIPTOR_BYTES, PortablePathComponent, ReferenceDeclaration, RepositoryPath,
    ValidatedFixture, ValidatedInventory,
};

#[derive(Clone, Debug)]
pub struct InventoryRepository {
    repository_root: PathBuf,
    fixture_root: PathBuf,
}

impl InventoryRepository {
    pub fn new(repository_root: impl Into<PathBuf>, fixture_root: impl Into<PathBuf>) -> Self {
        Self {
            repository_root: repository_root.into(),
            fixture_root: fixture_root.into(),
        }
    }

    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    pub fn fixture_root(&self) -> &Path {
        &self.fixture_root
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScannedEntryKind {
    RegularFile,
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Debug)]
struct ScannedEntry {
    absolute_path: PathBuf,
    kind: ScannedEntryKind,
}

#[derive(Clone, Debug)]
struct DiscoveredBundle {
    repository_path: String,
    entries: BTreeMap<String, ScannedEntry>,
    nested_descriptors: BTreeSet<String>,
}

pub fn discover_inventory(
    repository: &InventoryRepository,
) -> Result<ValidatedInventory, InventoryErrors> {
    let mut diagnostics = Vec::new();
    if !validate_roots(repository, &mut diagnostics) {
        return Err(InventoryErrors::sorted(diagnostics));
    }

    let mut bundles = discover_bundles(repository, &mut diagnostics);
    bundles.sort_by(|left, right| left.repository_path.cmp(&right.repository_path));

    let mut fixtures = Vec::new();
    let mut raw_ids = Vec::new();
    for bundle in &bundles {
        validate_bundle(bundle, &mut raw_ids, &mut fixtures, &mut diagnostics);
    }
    validate_global_ids(&raw_ids, &mut diagnostics);

    if !diagnostics.is_empty() {
        return Err(InventoryErrors::sorted(diagnostics));
    }
    fixtures.sort_by(|left, right| left.fixture_path().cmp(right.fixture_path()));
    Ok(ValidatedInventory::validated(fixtures))
}

fn validate_roots(
    repository: &InventoryRepository,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> bool {
    let repository_metadata = match fs::symlink_metadata(&repository.repository_root) {
        Ok(metadata) => metadata,
        Err(_) => {
            diagnostics.push(InventoryDiagnostic::new(
                ".",
                InventoryDiagnosticKind::RepositoryRootNotDirectory,
            ));
            return false;
        }
    };
    if repository_metadata.file_type().is_symlink() {
        diagnostics.push(InventoryDiagnostic::new(
            ".",
            InventoryDiagnosticKind::SymlinkNotAllowed,
        ));
        return false;
    }
    if !repository_metadata.is_dir() {
        diagnostics.push(InventoryDiagnostic::new(
            ".",
            InventoryDiagnosticKind::RepositoryRootNotDirectory,
        ));
        return false;
    }
    let root_display =
        match normalize_repository_path(&repository.repository_root, &repository.fixture_root) {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(normalization_diagnostic(error));
                return false;
            }
        };
    validate_fixture_root_chain(repository, &root_display, diagnostics)
}

fn validate_fixture_root_chain(
    repository: &InventoryRepository,
    root_display: &str,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> bool {
    let Ok(relative) = repository
        .fixture_root
        .strip_prefix(&repository.repository_root)
    else {
        return false;
    };
    let mut current = repository.repository_root.clone();
    let mut display = String::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return false;
        };
        let Some(component) = component.to_str() else {
            diagnostics.push(InventoryDiagnostic::new(
                root_display,
                InventoryDiagnosticKind::NonUtf8Path,
            ));
            return false;
        };
        current.push(component);
        display = join_repository_path(&display, component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(_) => {
                diagnostics.push(InventoryDiagnostic::new(
                    root_display,
                    InventoryDiagnosticKind::FixtureRootNotDirectory,
                ));
                return false;
            }
        };
        if metadata.file_type().is_symlink() {
            diagnostics.push(InventoryDiagnostic::new(
                display,
                InventoryDiagnosticKind::SymlinkNotAllowed,
            ));
            return false;
        }
        if !metadata.is_dir() {
            diagnostics.push(InventoryDiagnostic::new(
                root_display,
                InventoryDiagnosticKind::FixtureRootNotDirectory,
            ));
            return false;
        }
    }
    true
}

fn discover_bundles(
    repository: &InventoryRepository,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> Vec<DiscoveredBundle> {
    let mut bundles = Vec::new();
    let mut stack = vec![repository.fixture_root.clone()];
    while let Some(directory) = stack.pop() {
        let display = match normalize_repository_path(&repository.repository_root, &directory) {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(normalization_diagnostic(error));
                continue;
            }
        };
        let entries = match sorted_directory_entries(&directory, &display, diagnostics) {
            Some(entries) => entries,
            None => continue,
        };
        let has_descriptor = entries.iter().any(|(name, _)| {
            name.as_ref()
                .is_some_and(|name| name.as_str() == "fixture.toml")
        });
        if has_descriptor {
            bundles.push(integrity_scan_bundle(
                repository,
                directory,
                display,
                diagnostics,
            ));
            continue;
        }

        let mut child_directories = Vec::new();
        let mut has_regular_file = false;
        for (name, entry) in entries {
            let Some(name) = name else {
                continue;
            };
            let path = entry.path();
            let child_display = join_repository_path(&display, name.as_str());
            match classify_entry(&path, &child_display, diagnostics) {
                Some(ScannedEntryKind::Directory) => child_directories.push(path),
                Some(ScannedEntryKind::RegularFile) => has_regular_file = true,
                Some(ScannedEntryKind::Symlink) => {}
                Some(ScannedEntryKind::Other) => diagnostics.push(InventoryDiagnostic::new(
                    child_display,
                    InventoryDiagnosticKind::NonRegularBundleEntry,
                )),
                None => {}
            }
        }
        if has_regular_file {
            diagnostics.push(InventoryDiagnostic::new(
                display,
                InventoryDiagnosticKind::MissingFixtureDescriptor,
            ));
        }
        for child in child_directories.into_iter().rev() {
            stack.push(child);
        }
    }
    bundles
}

fn integrity_scan_bundle(
    repository: &InventoryRepository,
    bundle_root: PathBuf,
    bundle_display: String,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> DiscoveredBundle {
    let mut entries_by_path = BTreeMap::new();
    let mut nested_descriptors = BTreeSet::new();
    let mut stack = vec![bundle_root.clone()];
    while let Some(directory) = stack.pop() {
        let directory_display = normalize_repository_path(&repository.repository_root, &directory)
            .unwrap_or_else(|_| bundle_display.clone());
        let Some(entries) = sorted_directory_entries(&directory, &directory_display, diagnostics)
        else {
            continue;
        };
        let mut child_directories = Vec::new();
        for (name, entry) in entries {
            let Some(name) = name else {
                continue;
            };
            let path = entry.path();
            let child_display = join_repository_path(&directory_display, name.as_str());
            let Some(kind) = classify_entry(&path, &child_display, diagnostics) else {
                continue;
            };
            if name.as_str() == "fixture.toml" && directory != bundle_root {
                nested_descriptors.insert(child_display.clone());
                diagnostics.push(InventoryDiagnostic::new(
                    child_display.clone(),
                    InventoryDiagnosticKind::NestedFixtureBundle,
                ));
            }
            if kind == ScannedEntryKind::Directory {
                child_directories.push(path.clone());
            } else if kind == ScannedEntryKind::Other {
                diagnostics.push(InventoryDiagnostic::new(
                    child_display.clone(),
                    InventoryDiagnosticKind::NonRegularBundleEntry,
                ));
            }
            entries_by_path.insert(
                child_display,
                ScannedEntry {
                    absolute_path: path,
                    kind,
                },
            );
        }
        for child in child_directories.into_iter().rev() {
            stack.push(child);
        }
    }

    DiscoveredBundle {
        repository_path: bundle_display,
        entries: entries_by_path,
        nested_descriptors,
    }
}

fn sorted_directory_entries(
    directory: &Path,
    display: &str,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> Option<Vec<(Option<PortablePathComponent>, fs::DirEntry)>> {
    let read_dir = match fs::read_dir(directory) {
        Ok(read_dir) => read_dir,
        Err(_) => {
            diagnostics.push(InventoryDiagnostic::new(
                display,
                InventoryDiagnosticKind::ReadFailed {
                    operation: "read-directory",
                },
            ));
            return None;
        }
    };
    let mut entries = Vec::new();
    for result in read_dir {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => {
                diagnostics.push(InventoryDiagnostic::new(
                    display,
                    InventoryDiagnosticKind::ReadFailed {
                        operation: "read-directory-entry",
                    },
                ));
                continue;
            }
        };
        match entry.file_name().into_string() {
            Ok(name) => match PortablePathComponent::parse(&name) {
                Some(name) => entries.push((Some(name), entry)),
                None => {
                    diagnostics.push(InventoryDiagnostic::new(
                        join_repository_path(display, "<non-portable-component>"),
                        InventoryDiagnosticKind::NonPortablePathComponent { value: name },
                    ));
                    entries.push((None, entry));
                }
            },
            Err(_) => {
                diagnostics.push(InventoryDiagnostic::new(
                    join_repository_path(display, "<non-utf8>"),
                    InventoryDiagnosticKind::NonUtf8Path,
                ));
                entries.push((None, entry));
            }
        }
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Some(entries)
}

fn classify_entry(
    path: &Path,
    display: &str,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> Option<ScannedEntryKind> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            diagnostics.push(InventoryDiagnostic::new(
                display,
                InventoryDiagnosticKind::ReadFailed {
                    operation: "read-entry-metadata",
                },
            ));
            return None;
        }
    };
    if metadata.file_type().is_symlink() {
        diagnostics.push(InventoryDiagnostic::new(
            display,
            InventoryDiagnosticKind::SymlinkNotAllowed,
        ));
        Some(ScannedEntryKind::Symlink)
    } else if metadata.is_file() {
        Some(ScannedEntryKind::RegularFile)
    } else if metadata.is_dir() {
        Some(ScannedEntryKind::Directory)
    } else {
        Some(ScannedEntryKind::Other)
    }
}

fn validate_bundle(
    bundle: &DiscoveredBundle,
    raw_ids: &mut Vec<(String, String)>,
    fixtures: &mut Vec<ValidatedFixture>,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) {
    let metadata_path = join_repository_path(&bundle.repository_path, "fixture.toml");
    let Some(metadata_entry) = bundle.entries.get(&metadata_path) else {
        diagnostics.push(InventoryDiagnostic::new(
            metadata_path,
            InventoryDiagnosticKind::MissingFixtureDescriptor,
        ));
        return;
    };
    if metadata_entry.kind != ScannedEntryKind::RegularFile {
        diagnostics.push(InventoryDiagnostic::new(
            metadata_path,
            InventoryDiagnosticKind::DeclaredPathNotRegularFile {
                field: "fixture.toml",
                value: "fixture.toml".to_owned(),
            },
        ));
        return;
    }
    let Some(bytes) =
        read_bounded_descriptor(&metadata_entry.absolute_path, &metadata_path, diagnostics)
    else {
        return;
    };
    let parsed = parse_descriptor(&bytes);
    if let Some(raw_id) = parsed.raw_id {
        raw_ids.push((raw_id, metadata_path.clone()));
    }
    for kind in parsed.diagnostics {
        diagnostics.push(InventoryDiagnostic::new(&metadata_path, kind));
    }
    let Some(descriptor) = parsed.descriptor else {
        return;
    };
    validate_bundle_descriptor(bundle, metadata_path, descriptor, fixtures, diagnostics);
}

fn read_bounded_descriptor(
    path: &Path,
    metadata_path: &str,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> Option<Vec<u8>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            diagnostics.push(InventoryDiagnostic::new(
                metadata_path,
                InventoryDiagnosticKind::ReadFailed {
                    operation: "open-descriptor",
                },
            ));
            return None;
        }
    };
    let mut bytes = Vec::with_capacity((MAX_DESCRIPTOR_BYTES + 1) as usize);
    if file
        .take(MAX_DESCRIPTOR_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        diagnostics.push(InventoryDiagnostic::new(
            metadata_path,
            InventoryDiagnosticKind::ReadFailed {
                operation: "read-descriptor",
            },
        ));
        return None;
    }
    if bytes.len() as u64 > MAX_DESCRIPTOR_BYTES {
        diagnostics.push(InventoryDiagnostic::new(
            metadata_path,
            InventoryDiagnosticKind::DescriptorTooLarge {
                observed_at_least: bytes.len() as u64,
                maximum: MAX_DESCRIPTOR_BYTES,
            },
        ));
        return None;
    }
    Some(bytes)
}

fn validate_bundle_descriptor(
    bundle: &DiscoveredBundle,
    metadata_path: String,
    descriptor: ParsedDescriptor,
    fixtures: &mut Vec<ValidatedFixture>,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) {
    let test_path = validate_declared_path(bundle, "test_path", &descriptor.test_path, diagnostics);
    let reference_path = descriptor.reference.as_ref().map(|reference| {
        validate_declared_path(bundle, "reference.path", &reference.path, diagnostics)
    });

    let declarations_were_safe = test_path.safe
        && reference_path
            .as_ref()
            .is_none_or(|reference| reference.safe);
    if declarations_were_safe {
        let mut declared = BTreeSet::from([metadata_path.clone()]);
        if let Some(path) = test_path.repository_path.as_ref() {
            declared.insert(path.as_str().to_owned());
        }
        if let Some(Some(path)) = reference_path
            .as_ref()
            .map(|reference| reference.repository_path.as_ref())
        {
            declared.insert(path.as_str().to_owned());
        }
        for (path, entry) in &bundle.entries {
            if entry.kind == ScannedEntryKind::RegularFile
                && !declared.contains(path)
                && !bundle.nested_descriptors.contains(path)
            {
                diagnostics.push(InventoryDiagnostic::new(
                    path,
                    InventoryDiagnosticKind::UndeclaredBundleFile,
                ));
            }
        }
    }

    let Some(test_path) = test_path.repository_path else {
        return;
    };
    let has_reference = descriptor.reference.is_some();
    let reference = match (descriptor.reference, reference_path) {
        (None, None) => None,
        (Some(reference), Some(validation)) => validation
            .repository_path
            .map(|path| ReferenceDeclaration::validated(reference.kind, path)),
        _ => None,
    };
    if has_reference && reference.is_none() {
        return;
    }
    fixtures.push(ValidatedFixture::validated(
        descriptor.test_id,
        RepositoryPath::validated(bundle.repository_path.clone()),
        test_path,
        RepositoryPath::validated(metadata_path),
        descriptor.scope,
        descriptor.observation,
        descriptor.source_kind,
        reference,
        descriptor.description,
    ));
}

struct DeclaredPathValidation {
    safe: bool,
    repository_path: Option<RepositoryPath>,
}

fn validate_declared_path(
    bundle: &DiscoveredBundle,
    field: &'static str,
    value: &str,
    diagnostics: &mut Vec<InventoryDiagnostic>,
) -> DeclaredPathValidation {
    let Some(relative) = normalize_bundle_relative_path(value) else {
        diagnostics.push(InventoryDiagnostic::new(
            join_repository_path(&bundle.repository_path, "fixture.toml"),
            InventoryDiagnosticKind::InvalidRelativePath {
                field,
                value: value.to_owned(),
            },
        ));
        return DeclaredPathValidation {
            safe: false,
            repository_path: None,
        };
    };
    if relative == "fixture.toml" {
        diagnostics.push(InventoryDiagnostic::new(
            join_repository_path(&bundle.repository_path, "fixture.toml"),
            InventoryDiagnosticKind::InvalidRelativePath {
                field,
                value: value.to_owned(),
            },
        ));
        return DeclaredPathValidation {
            safe: false,
            repository_path: None,
        };
    }
    let repository_path = join_repository_path(&bundle.repository_path, &relative);
    match bundle.entries.get(&repository_path) {
        Some(entry) if entry.kind == ScannedEntryKind::RegularFile => DeclaredPathValidation {
            safe: true,
            repository_path: Some(RepositoryPath::validated(repository_path)),
        },
        Some(_) => {
            diagnostics.push(InventoryDiagnostic::new(
                &repository_path,
                InventoryDiagnosticKind::DeclaredPathNotRegularFile {
                    field,
                    value: value.to_owned(),
                },
            ));
            DeclaredPathValidation {
                safe: true,
                repository_path: None,
            }
        }
        None => {
            diagnostics.push(InventoryDiagnostic::new(
                &repository_path,
                InventoryDiagnosticKind::MissingDeclaredFile {
                    field,
                    value: value.to_owned(),
                },
            ));
            DeclaredPathValidation {
                safe: true,
                repository_path: None,
            }
        }
    }
}

fn normalize_bundle_relative_path(value: &str) -> Option<String> {
    let mut components = Vec::new();
    for component in value.split('/') {
        components.push(PortablePathComponent::parse(component)?);
    }
    Some(
        components
            .iter()
            .map(PortablePathComponent::as_str)
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn validate_global_ids(raw_ids: &[(String, String)], diagnostics: &mut Vec<InventoryDiagnostic>) {
    let mut ids = BTreeMap::<String, (String, String)>::new();
    let mut sorted = raw_ids.to_vec();
    sorted.sort_by(|left, right| left.1.cmp(&right.1));
    for (value, path) in sorted {
        let folded = value.to_ascii_lowercase();
        if let Some((first_value, first_path)) = ids.get(&folded) {
            let kind = if first_value == &value {
                InventoryDiagnosticKind::DuplicateTestId {
                    value,
                    first_path: first_path.clone(),
                }
            } else {
                InventoryDiagnosticKind::CaseCollidingTestId {
                    value,
                    first_value: first_value.clone(),
                    first_path: first_path.clone(),
                }
            };
            diagnostics.push(InventoryDiagnostic::new(path, kind));
        } else {
            ids.insert(folded, (value, path));
        }
    }
}

enum RepositoryPathNormalizationError {
    OutsideRepository,
    NonUtf8Component { parent: String },
    NonPortableComponent { parent: String, value: String },
}

fn normalize_repository_path(
    repository_root: &Path,
    path: &Path,
) -> Result<String, RepositoryPathNormalizationError> {
    let relative = path
        .strip_prefix(repository_root)
        .map_err(|_| RepositoryPathNormalizationError::OutsideRepository)?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(RepositoryPathNormalizationError::OutsideRepository);
        };
        let parent = parts.join("/");
        let component = component.to_str().ok_or_else(|| {
            RepositoryPathNormalizationError::NonUtf8Component {
                parent: parent.clone(),
            }
        })?;
        let component = PortablePathComponent::parse(component).ok_or_else(|| {
            RepositoryPathNormalizationError::NonPortableComponent {
                parent,
                value: component.to_owned(),
            }
        })?;
        parts.push(component.as_str().to_owned());
    }
    Ok(parts.join("/"))
}

fn normalization_diagnostic(error: RepositoryPathNormalizationError) -> InventoryDiagnostic {
    match error {
        RepositoryPathNormalizationError::OutsideRepository => {
            InventoryDiagnostic::new(".", InventoryDiagnosticKind::FixtureRootOutsideRepository)
        }
        RepositoryPathNormalizationError::NonUtf8Component { parent } => InventoryDiagnostic::new(
            join_repository_path(&parent, "<non-utf8>"),
            InventoryDiagnosticKind::NonUtf8Path,
        ),
        RepositoryPathNormalizationError::NonPortableComponent { parent, value } => {
            InventoryDiagnostic::new(
                join_repository_path(&parent, "<non-portable-component>"),
                InventoryDiagnosticKind::NonPortablePathComponent { value },
            )
        }
    }
}

fn join_repository_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}/{child}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_relative_paths_reject_platform_and_escape_forms() {
        for value in [
            "",
            ".",
            "..",
            "../outside",
            "nested/../outside",
            "/absolute",
            "C:/absolute",
            "C:\\absolute",
            "nested//file",
            "Uppercase/file",
            "nested/trailing.",
            "nested/trailing ",
            "nested/con.txt",
            "nested/unicode-é",
            "nested/control-\n",
        ] {
            assert!(normalize_bundle_relative_path(value).is_none(), "{value:?}");
        }
        assert_eq!(
            normalize_bundle_relative_path("nested/input.html").as_deref(),
            Some("nested/input.html")
        );
    }
}
