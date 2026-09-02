use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfinedFileError {
    InvalidRelativePath,
    Missing,
    Symlink,
    NonDirectoryParent,
    NonRegularFile,
    TooLarge,
    Io,
}

pub fn read_confined_regular_file(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, ConfinedFileError> {
    let path = validate_confined_regular_file(root, relative, maximum_bytes)?;
    let bytes = fs::read(path).map_err(map_io)?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(ConfinedFileError::TooLarge);
    }
    Ok(bytes)
}

pub fn validate_confined_regular_file(
    root: &Path,
    relative: &Path,
    maximum_bytes: u64,
) -> Result<PathBuf, ConfinedFileError> {
    let path = validate_confined_path(root, relative, true)?;
    let metadata = fs::metadata(&path).map_err(map_io)?;
    if metadata.len() > maximum_bytes {
        return Err(ConfinedFileError::TooLarge);
    }
    Ok(path)
}

pub fn validate_confined_output_file(
    root: &Path,
    relative: &Path,
) -> Result<PathBuf, ConfinedFileError> {
    validate_confined_path(root, relative, false)
}

fn validate_confined_path(
    root: &Path,
    relative: &Path,
    require_file: bool,
) -> Result<PathBuf, ConfinedFileError> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(ConfinedFileError::InvalidRelativePath);
    }
    let root_metadata = fs::symlink_metadata(root).map_err(map_io)?;
    if root_metadata.file_type().is_symlink() {
        return Err(ConfinedFileError::Symlink);
    }
    if !root_metadata.is_dir() {
        return Err(ConfinedFileError::NonDirectoryParent);
    }

    let components = relative.components().collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ConfinedFileError::InvalidRelativePath);
    }

    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            unreachable!("components were validated")
        };
        current.push(part);
        let final_component = index + 1 == components.len();
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ConfinedFileError::Symlink);
            }
            Ok(metadata) if final_component && !metadata.is_file() => {
                return Err(ConfinedFileError::NonRegularFile);
            }
            Ok(metadata) if !final_component && !metadata.is_dir() => {
                return Err(ConfinedFileError::NonDirectoryParent);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if require_file || !final_component {
                    return Err(ConfinedFileError::Missing);
                }
            }
            Err(_) => return Err(ConfinedFileError::Io),
        }
    }
    Ok(current)
}

fn map_io(error: std::io::Error) -> ConfinedFileError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ConfinedFileError::Missing
    } else {
        ConfinedFileError::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_confined_file_is_bounded_and_parent_symlinks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("a/b")).unwrap();
        fs::write(root.path().join("a/b/input.toml"), b"abc").unwrap();
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 3).unwrap(),
            b"abc"
        );
        assert_eq!(
            read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 2),
            Err(ConfinedFileError::TooLarge)
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            fs::rename(root.path().join("a"), root.path().join("real-a")).unwrap();
            symlink(root.path().join("real-a"), root.path().join("a")).unwrap();
            assert_eq!(
                read_confined_regular_file(root.path(), Path::new("a/b/input.toml"), 3),
                Err(ConfinedFileError::Symlink)
            );
        }
    }

    #[test]
    fn output_validation_allows_only_a_missing_final_file() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("generated")).unwrap();
        assert_eq!(
            validate_confined_output_file(root.path(), Path::new("generated/summary.toml"))
                .unwrap(),
            root.path().join("generated/summary.toml")
        );
        assert_eq!(
            validate_confined_output_file(root.path(), Path::new("missing/summary.toml")),
            Err(ConfinedFileError::Missing)
        );
    }
}
