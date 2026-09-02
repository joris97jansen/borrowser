use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use external_test_provenance::{ImmutableRevision, sha256};

use crate::{ValidatedWptSourceSet, WPT_MAX_FILE_BYTES, WPT_MAX_TOTAL_BYTES, WptRegistryError};

#[derive(Clone, Debug, PartialEq, Eq)]
struct GitBlobPreflight {
    object_id: String,
    size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptMaterializationError {
    Registry(WptRegistryError),
    GitUnavailable,
    RevisionMissing,
    RevisionMismatch,
    GitObjectMissing,
    UnsupportedGitMode,
    GitOutputInvalid,
    HashMismatch,
    FileTooLarge,
    TotalBytesExceeded,
    UnsafeOutput,
    Io,
}
impl From<WptRegistryError> for WptMaterializationError {
    fn from(value: WptRegistryError) -> Self {
        Self::Registry(value)
    }
}

pub fn materialize_wpt_source_set(
    repository_root: &Path,
    checkout: &Path,
    set: &ValidatedWptSourceSet,
) -> Result<(), WptMaterializationError> {
    verify_checkout_revision(checkout, set.revision())?;
    let wpt_root = repository_root.join("tests/conformance/external/wpt");
    ensure_directory_chain(repository_root, &wpt_root)?;
    let parent = wpt_root.join("raw");
    match fs::symlink_metadata(&parent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(WptMaterializationError::UnsafeOutput);
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&parent).map_err(|_| WptMaterializationError::Io)?
        }
        Err(_) => return Err(WptMaterializationError::Io),
    }
    let destination = parent.join(set.revision().as_str());
    if destination.exists() {
        crate::validate_materialized_sources(repository_root, set)?;
        return Ok(());
    }
    let transaction = tempfile::Builder::new()
        .prefix(".ag8-materialize-")
        .tempdir_in(&parent)
        .map_err(|_| WptMaterializationError::Io)?;
    let next = transaction.path().join(set.revision().as_str());
    fs::create_dir(&next).map_err(|_| WptMaterializationError::Io)?;
    let preflighted = preflight_git_blobs(
        checkout,
        set.revision(),
        set.files()
            .iter()
            .map(|file| file.identity().path().as_str()),
    )?;
    for (file, blob) in set.files().iter().zip(&preflighted) {
        let bytes = read_preflighted_git_blob(checkout, blob)?;
        if sha256(&bytes) != file.identity().sha256() {
            return Err(WptMaterializationError::HashMismatch);
        }
        let output = next.join(file.identity().path().as_str());
        if !output.starts_with(&next) {
            return Err(WptMaterializationError::UnsafeOutput);
        }
        fs::create_dir_all(
            output
                .parent()
                .ok_or(WptMaterializationError::UnsafeOutput)?,
        )
        .map_err(|_| WptMaterializationError::Io)?;
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .map_err(|_| WptMaterializationError::Io)?;
        handle
            .write_all(&bytes)
            .map_err(|_| WptMaterializationError::Io)?;
        handle.sync_all().map_err(|_| WptMaterializationError::Io)?;
    }
    fs::rename(&next, &destination).map_err(|_| WptMaterializationError::Io)?;
    Ok(())
}

fn verify_checkout_revision(
    checkout: &Path,
    revision: &ImmutableRevision,
) -> Result<(), WptMaterializationError> {
    let spec = format!("{}^{{commit}}", revision.as_str());
    let output = git(checkout, ["rev-parse", "--verify", &spec])?;
    let value = std::str::from_utf8(&output)
        .map_err(|_| WptMaterializationError::GitOutputInvalid)?
        .trim_end_matches('\n');
    if value != revision.as_str() {
        return Err(WptMaterializationError::RevisionMismatch);
    }
    Ok(())
}

fn preflight_git_blobs<'a>(
    checkout: &Path,
    revision: &ImmutableRevision,
    paths: impl Iterator<Item = &'a str>,
) -> Result<Vec<GitBlobPreflight>, WptMaterializationError> {
    let mut total = 0_u64;
    let mut blobs = Vec::new();
    for path in paths {
        let blob = preflight_git_blob(checkout, revision, path)?;
        if blob.size > WPT_MAX_FILE_BYTES {
            return Err(WptMaterializationError::FileTooLarge);
        }
        total = total
            .checked_add(blob.size)
            .ok_or(WptMaterializationError::TotalBytesExceeded)?;
        if total > WPT_MAX_TOTAL_BYTES {
            return Err(WptMaterializationError::TotalBytesExceeded);
        }
        blobs.push(blob);
    }
    Ok(blobs)
}

fn preflight_git_blob(
    checkout: &Path,
    revision: &ImmutableRevision,
    path: &str,
) -> Result<GitBlobPreflight, WptMaterializationError> {
    let tree = git(checkout, ["ls-tree", "-z", revision.as_str(), "--", path])?;
    let prefix = b"100644 blob ";
    if !tree.starts_with(prefix)
        || !tree.ends_with(b"\0")
        || tree.iter().filter(|byte| **byte == 0).count() != 1
    {
        return Err(WptMaterializationError::UnsupportedGitMode);
    }
    let tab = tree
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or(WptMaterializationError::GitOutputInvalid)?;
    if &tree[tab + 1..tree.len() - 1] != path.as_bytes() {
        return Err(WptMaterializationError::GitOutputInvalid);
    }
    let object_id = std::str::from_utf8(&tree[prefix.len()..tab])
        .map_err(|_| WptMaterializationError::GitOutputInvalid)?;
    if !matches!(object_id.len(), 40 | 64)
        || !object_id.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(WptMaterializationError::GitOutputInvalid);
    }
    let size_output = git(checkout, ["cat-file", "-s", object_id])?;
    let size = parse_git_object_size(&size_output)?;
    Ok(GitBlobPreflight {
        object_id: object_id.to_owned(),
        size,
    })
}

fn parse_git_object_size(output: &[u8]) -> Result<u64, WptMaterializationError> {
    let digits = output
        .strip_suffix(b"\n")
        .ok_or(WptMaterializationError::GitOutputInvalid)?;
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(WptMaterializationError::GitOutputInvalid);
    }
    std::str::from_utf8(digits)
        .map_err(|_| WptMaterializationError::GitOutputInvalid)?
        .parse()
        .map_err(|_| WptMaterializationError::GitOutputInvalid)
}

fn read_preflighted_git_blob(
    checkout: &Path,
    blob: &GitBlobPreflight,
) -> Result<Vec<u8>, WptMaterializationError> {
    let bytes = git(checkout, ["cat-file", "blob", blob.object_id.as_str()])?;
    if bytes.len() as u64 != blob.size {
        return Err(WptMaterializationError::GitOutputInvalid);
    }
    if bytes.len() as u64 > WPT_MAX_FILE_BYTES {
        return Err(WptMaterializationError::FileTooLarge);
    }
    Ok(bytes)
}

fn git<const N: usize>(
    checkout: &Path,
    args: [&str; N],
) -> Result<Vec<u8>, WptMaterializationError> {
    let output = Command::new("git")
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("-C")
        .arg(checkout)
        .args(args)
        .output()
        .map_err(|_| WptMaterializationError::GitUnavailable)?;
    if !output.status.success() {
        return Err(WptMaterializationError::GitObjectMissing);
    }
    Ok(output.stdout)
}
fn ensure_directory_chain(root: &Path, path: &Path) -> Result<(), WptMaterializationError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| WptMaterializationError::UnsafeOutput)?;
    let mut current = PathBuf::from(root);
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err(WptMaterializationError::UnsafeOutput);
        };
        current.push(part);
        let metadata = fs::symlink_metadata(&current).map_err(|_| WptMaterializationError::Io)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(WptMaterializationError::UnsafeOutput);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    #[test]
    fn committed_git_blob_is_used_even_when_worktree_is_dirty() {
        let temp = tempfile::tempdir().unwrap();
        Command::new("git")
            .args(["init", "-q"])
            .arg(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "config",
                "user.email",
                "ag8@example.invalid",
            ])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "config",
                "user.name",
                "AG8 Test",
            ])
            .status()
            .unwrap();
        fs::write(temp.path().join("source.html"), b"committed").unwrap();
        Command::new("git")
            .args(["-C", temp.path().to_str().unwrap(), "add", "source.html"])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "commit",
                "-qm",
                "source",
            ])
            .status()
            .unwrap();
        let revision = String::from_utf8(
            Command::new("git")
                .args(["-C", temp.path().to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let revision = ImmutableRevision::parse_git_commit(revision.trim()).unwrap();
        fs::write(temp.path().join("source.html"), b"dirty").unwrap();
        verify_checkout_revision(temp.path(), &revision).unwrap();
        assert_eq!(
            read_preflighted_git_blob(
                temp.path(),
                &preflight_git_blob(temp.path(), &revision, "source.html").unwrap(),
            )
            .unwrap(),
            b"committed"
        );
        Command::new("git")
            .args(["-C", temp.path().to_str().unwrap(), "add", "source.html"])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "commit",
                "-qm",
                "new-head",
            ])
            .status()
            .unwrap();
        verify_checkout_revision(temp.path(), &revision).unwrap();
        assert_eq!(
            read_preflighted_git_blob(
                temp.path(),
                &preflight_git_blob(temp.path(), &revision, "source.html").unwrap(),
            )
            .unwrap(),
            b"committed"
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlink_git_mode_is_rejected() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        Command::new("git")
            .args(["init", "-q"])
            .arg(temp.path())
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "config",
                "user.email",
                "ag8@example.invalid",
            ])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "config",
                "user.name",
                "AG8 Test",
            ])
            .status()
            .unwrap();
        fs::write(temp.path().join("target"), b"x").unwrap();
        symlink("target", temp.path().join("link")).unwrap();
        Command::new("git")
            .args(["-C", temp.path().to_str().unwrap(), "add", "link", "target"])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                temp.path().to_str().unwrap(),
                "commit",
                "-qm",
                "source",
            ])
            .status()
            .unwrap();
        let revision = String::from_utf8(
            Command::new("git")
                .args(["-C", temp.path().to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let revision = ImmutableRevision::parse_git_commit(revision.trim()).unwrap();
        assert_eq!(
            preflight_git_blob(temp.path(), &revision, "link"),
            Err(WptMaterializationError::UnsupportedGitMode)
        );
    }

    #[test]
    fn oversized_committed_blob_is_rejected_during_preflight() {
        let temp = tempfile::tempdir().unwrap();
        initialize_git_repository(temp.path());
        fs::write(
            temp.path().join("oversized.html"),
            vec![b'x'; WPT_MAX_FILE_BYTES as usize + 1],
        )
        .unwrap();
        let revision = commit_all(temp.path());
        assert_eq!(
            preflight_git_blobs(temp.path(), &revision, std::iter::once("oversized.html")),
            Err(WptMaterializationError::FileTooLarge)
        );
    }

    #[test]
    fn aggregate_committed_blob_size_is_rejected_before_content_reads() {
        let temp = tempfile::tempdir().unwrap();
        initialize_git_repository(temp.path());
        let file_count = (WPT_MAX_TOTAL_BYTES / WPT_MAX_FILE_BYTES + 1) as usize;
        let mut paths = Vec::new();
        for index in 0..file_count {
            let path = format!("source-{index}.html");
            fs::write(
                temp.path().join(&path),
                vec![b'x'; WPT_MAX_FILE_BYTES as usize],
            )
            .unwrap();
            paths.push(path);
        }
        let revision = commit_all(temp.path());
        assert_eq!(
            preflight_git_blobs(temp.path(), &revision, paths.iter().map(String::as_str)),
            Err(WptMaterializationError::TotalBytesExceeded)
        );
    }

    #[test]
    fn bounded_preflight_and_read_are_byte_identical() {
        let temp = tempfile::tempdir().unwrap();
        initialize_git_repository(temp.path());
        let expected = b"bounded immutable blob\n";
        fs::write(temp.path().join("source.html"), expected).unwrap();
        let revision = commit_all(temp.path());
        let blobs =
            preflight_git_blobs(temp.path(), &revision, std::iter::once("source.html")).unwrap();
        assert_eq!(
            read_preflighted_git_blob(temp.path(), &blobs[0]).unwrap(),
            expected
        );
    }

    fn initialize_git_repository(path: &Path) {
        Command::new("git")
            .args(["init", "-q"])
            .arg(path)
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                path.to_str().unwrap(),
                "config",
                "user.email",
                "ag8@example.invalid",
            ])
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                path.to_str().unwrap(),
                "config",
                "user.name",
                "AG8 Test",
            ])
            .status()
            .unwrap();
    }

    fn commit_all(path: &Path) -> ImmutableRevision {
        Command::new("git")
            .args(["-C", path.to_str().unwrap(), "add", "."])
            .status()
            .unwrap();
        Command::new("git")
            .args(["-C", path.to_str().unwrap(), "commit", "-qm", "source"])
            .status()
            .unwrap();
        let revision = String::from_utf8(
            Command::new("git")
                .args(["-C", path.to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        ImmutableRevision::parse_git_commit(revision.trim()).unwrap()
    }
}
