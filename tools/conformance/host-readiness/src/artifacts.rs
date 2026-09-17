//! Descriptor-relative artifact verification on a controlled, frozen evidence tree.
//! Sequential barriers detect mutation/rebinding; not a hostile-storage snapshot.
use crate::Sha256;
use crate::{
    Result,
    evidence::{ArtifactRef, HostReadinessManifest, artifact_path},
    require,
};
use std::{
    ffi::CString,
    fs::{File, Metadata},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
    time::Instant,
};

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    len: u64,
    links: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
fn identity(m: &Metadata) -> Identity {
    Identity {
        dev: m.dev(),
        ino: m.ino(),
        mode: m.mode(),
        uid: m.uid(),
        gid: m.gid(),
        len: m.len(),
        links: m.nlink(),
        mtime: (m.mtime(), m.mtime_nsec()),
        ctime: (m.ctime(), m.ctime_nsec()),
    }
}
fn open_at(parent: &File, name: &str, directory: bool) -> Result<File> {
    let name = CString::new(name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if directory { libc::O_DIRECTORY } else { 0 },
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
pub fn open_root(path: &Path) -> Result<File> {
    // Root is explicitly selected authority. Descendants never resolve via its pathname.
    Ok(File::options()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?)
}
fn deadline(end: Instant) -> Result<()> {
    require(Instant::now() < end, "artifact verification deadline")
}
fn verify_one(
    root: &File,
    artifact: &ArtifactRef,
    end: Instant,
    after_read: impl FnOnce(),
) -> Result<()> {
    artifact_path(&artifact.path)?;
    let components: Vec<_> = artifact.path.split('/').collect();
    let mut directories = vec![root.try_clone()?];
    for component in &components[..components.len() - 1] {
        deadline(end)?;
        directories.push(open_at(directories.last().unwrap(), component, true)?);
    }
    let name = components.last().unwrap();
    let parent = directories.last().unwrap();
    let mut file = open_at(parent, name, false)?;
    let before = file.metadata()?;
    require(
        before.is_file() && before.len() == artifact.byte_length,
        "artifact type/length",
    )?;
    let before = identity(&before);
    let mut hash = Sha256::new();
    let mut length = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        deadline(end)?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        length = length.checked_add(n as u64).ok_or("read length overflow")?;
        require(length <= artifact.byte_length, "artifact grew")?;
        hash.update(&buffer[..n]);
    }
    after_read();
    require(
        identity(&file.metadata()?) == before,
        "opened artifact mutated",
    )?;
    require(
        length == artifact.byte_length && format!("{:x}", hash.finalize()) == artifact.sha256,
        "artifact digest/length mismatch",
    )?;
    // Reopen only for binding comparison; never hash the reopened pathname object.
    require(
        identity(&open_at(parent, name, false)?.metadata()?) == before,
        "artifact replaced",
    )?;
    for (i, component) in components[..components.len() - 1].iter().enumerate() {
        let reopened = open_at(&directories[i], component, true)?;
        let retained = directories[i + 1].metadata()?;
        let actual = reopened.metadata()?;
        require(
            (retained.dev(), retained.ino()) == (actual.dev(), actual.ino()),
            "artifact ancestor replaced",
        )?;
    }
    deadline(end)
}
pub fn verify(root: &Path, manifest: &HostReadinessManifest, end: Instant) -> Result<()> {
    manifest.validate()?;
    let root = open_root(root)?;
    for artifact in &manifest.artifacts {
        verify_one(&root, artifact, end, || {})?;
    }
    Ok(())
}
/// Bounded CLI input, no implicit canonicalization of stored evidence.
pub fn read_manifest(path: &Path) -> Result<Vec<u8>> {
    let file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    require(file.metadata()?.is_file(), "manifest regular file")?;
    let before = identity(&file.metadata()?);
    let mut bytes = Vec::new();
    (&file)
        .take((crate::evidence::MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    require(
        identity(&file.metadata()?) == before && bytes.len() <= crate::evidence::MANIFEST_BYTES,
        "manifest changed/oversized",
    )?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink, time::Duration};
    fn artifact() -> ArtifactRef {
        ArtifactRef {
            id: "data".into(),
            path: "dir/data".into(),
            byte_length: 3,
            sha256: format!("{:x}", Sha256::digest(b"abc")),
        }
    }
    #[test]
    fn retains_hash_object_and_rejects_mutation_replacement_aliases() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("dir")).unwrap();
        let path = d.path().join("dir/data");
        fs::write(&path, b"abc").unwrap();
        let root = open_root(d.path()).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        assert!(verify_one(&root, &artifact(), end, || {}).is_ok());
        assert!(
            verify_one(&root, &artifact(), end, || {
                fs::write(&path, b"xyz").unwrap();
            })
            .is_err()
        );
        fs::write(&path, b"abc").unwrap();
        assert!(
            verify_one(&root, &artifact(), end, || {
                fs::rename(&path, d.path().join("old")).unwrap();
                fs::write(&path, b"abc").unwrap();
            })
            .is_err()
        );
        fs::remove_file(&path).unwrap();
        symlink(d.path().join("old"), &path).unwrap();
        assert!(verify_one(&root, &artifact(), end, || {}).is_err());
        fs::remove_file(&path).unwrap();
        assert!(verify_one(&root, &artifact(), end, || {}).is_err());
        fs::write(&path, b"abc").unwrap();
        assert!(
            verify_one(&root, &artifact(), end, || {
                fs::rename(d.path().join("dir"), d.path().join("moved")).unwrap();
                fs::create_dir(d.path().join("dir")).unwrap();
            })
            .is_err()
        );
        fs::remove_dir(d.path().join("dir")).unwrap();
        symlink(d.path().join("moved"), d.path().join("dir")).unwrap();
        assert!(verify_one(&root, &artifact(), end, || {}).is_err());
    }
    #[test]
    fn bad_digest_length_and_deadline_fail() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("dir")).unwrap();
        fs::write(d.path().join("dir/data"), b"abc").unwrap();
        let root = open_root(d.path()).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        let mut a = artifact();
        a.sha256 = "0".repeat(64);
        assert!(verify_one(&root, &a, end, || {}).is_err());
        a = artifact();
        a.byte_length = 2;
        assert!(verify_one(&root, &a, end, || {}).is_err());
        assert!(verify_one(&root, &artifact(), Instant::now(), || {}).is_err());
    }
}
