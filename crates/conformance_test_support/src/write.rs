use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path};

use tempfile::NamedTempFile;

use crate::manifest::{ConformanceManifest, serialize_manifest};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestCheck {
    Current,
    Stale,
    Missing,
}

#[derive(Debug)]
pub enum ManifestOutputError {
    OutputOutsideRepository,
    InvalidRepositoryRoot,
    InvalidOutputParent,
    SymlinkNotAllowed,
    OutputNotRegularFile,
    Read(io::Error),
    CreateTemporary(io::Error),
    WriteTemporary(io::Error),
    SyncTemporary(io::Error),
    Persist(io::Error),
}

impl fmt::Display for ManifestOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputOutsideRepository => {
                f.write_str("manifest output is outside the repository root")
            }
            Self::InvalidRepositoryRoot => f.write_str("repository root is not a directory"),
            Self::InvalidOutputParent => {
                f.write_str("manifest output parent is not a regular directory")
            }
            Self::SymlinkNotAllowed => {
                f.write_str("symlinked manifest output paths are not allowed")
            }
            Self::OutputNotRegularFile => {
                f.write_str("existing manifest output is not a regular file")
            }
            Self::Read(error) => write!(f, "failed to read manifest output: {error}"),
            Self::CreateTemporary(error) => {
                write!(f, "failed to create same-directory temporary file: {error}")
            }
            Self::WriteTemporary(error) => {
                write!(f, "failed to write complete temporary manifest: {error}")
            }
            Self::SyncTemporary(error) => {
                write!(f, "failed to synchronize temporary manifest: {error}")
            }
            Self::Persist(error) => write!(f, "failed to replace manifest output: {error}"),
        }
    }
}

impl std::error::Error for ManifestOutputError {}

pub fn check_manifest(
    repository_root: &Path,
    output_path: &Path,
    manifest: &ConformanceManifest,
) -> Result<ManifestCheck, ManifestOutputError> {
    validate_output_path(repository_root, output_path)?;
    let expected = serialize_manifest(manifest);
    match fs::read(output_path) {
        Ok(actual) if actual == expected => Ok(ManifestCheck::Current),
        Ok(_) => Ok(ManifestCheck::Stale),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(ManifestCheck::Missing),
        Err(error) => Err(ManifestOutputError::Read(error)),
    }
}

pub fn update_manifest(
    repository_root: &Path,
    output_path: &Path,
    manifest: &ConformanceManifest,
) -> Result<(), ManifestOutputError> {
    validate_output_path(repository_root, output_path)?;
    let bytes = serialize_manifest(manifest);
    let parent = output_path
        .parent()
        .ok_or(ManifestOutputError::InvalidOutputParent)?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(ManifestOutputError::CreateTemporary)?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.flush())
        .map_err(ManifestOutputError::WriteTemporary)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(ManifestOutputError::SyncTemporary)?;

    validate_output_path(repository_root, output_path)?;
    temporary.persist(output_path).map_err(|error| {
        let io_error = error.error;
        drop(error.file);
        ManifestOutputError::Persist(io_error)
    })?;
    Ok(())
}

fn validate_output_path(
    repository_root: &Path,
    output_path: &Path,
) -> Result<(), ManifestOutputError> {
    let root_metadata = fs::symlink_metadata(repository_root)
        .map_err(|_| ManifestOutputError::InvalidRepositoryRoot)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ManifestOutputError::SymlinkNotAllowed);
    }
    if !root_metadata.is_dir() {
        return Err(ManifestOutputError::InvalidRepositoryRoot);
    }
    let relative = output_path
        .strip_prefix(repository_root)
        .map_err(|_| ManifestOutputError::OutputOutsideRepository)?;
    if relative.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir
        )
    }) {
        return Err(ManifestOutputError::OutputOutsideRepository);
    }
    let Some(parent) = output_path.parent() else {
        return Err(ManifestOutputError::InvalidOutputParent);
    };
    let parent_relative = parent
        .strip_prefix(repository_root)
        .map_err(|_| ManifestOutputError::OutputOutsideRepository)?;
    let mut current = repository_root.to_path_buf();
    for component in parent_relative.components() {
        let Component::Normal(component) = component else {
            return Err(ManifestOutputError::OutputOutsideRepository);
        };
        current.push(component);
        let metadata =
            fs::symlink_metadata(&current).map_err(|_| ManifestOutputError::InvalidOutputParent)?;
        if metadata.file_type().is_symlink() {
            return Err(ManifestOutputError::SymlinkNotAllowed);
        }
        if !metadata.is_dir() {
            return Err(ManifestOutputError::InvalidOutputParent);
        }
    }
    match fs::symlink_metadata(output_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(ManifestOutputError::SymlinkNotAllowed)
        }
        Ok(metadata) if !metadata.is_file() => Err(ManifestOutputError::OutputNotRegularFile),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ManifestOutputError::Read(error)),
    }
}
