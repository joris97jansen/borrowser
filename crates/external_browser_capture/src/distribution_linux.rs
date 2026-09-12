//! Linux descriptor-relative distribution snapshot. Never follows supplied symlinks.
use super::*;
use ring::digest::{Context, SHA256};
use rustix::fs::{Mode, OFlags, open, openat};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::OwnedFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
};

fn checked_metadata(file: &File, mode: u32) -> Result<std::fs::Metadata> {
    let m = file.metadata().map_err(|_| E::Read)?;
    let uid = unsafe { libc::getuid() };
    if m.mode() & 0o7777 != mode || (m.uid() != 0 && m.uid() != uid) {
        return Err(E::Mode);
    }
    Ok(m)
}
fn no_capabilities(file: &File) -> Result<()> {
    let mut b = [0u8; 128];
    match rustix::fs::fgetxattr(file, "security.capability", &mut b[..]) {
        Err(rustix::io::Errno::NODATA) => Ok(()),
        _ => Err(E::Capability),
    }
}
fn copy_file(source: File, output: &Path, len: u64, digest: &str, mode: u32) -> Result<File> {
    copy_file_budget(source, output, len, digest, mode, None)
}
fn copy_file_budget(
    mut source: File,
    output: &Path,
    len: u64,
    digest: &str,
    mode: u32,
    deadline: Option<crate::deadline::AttemptDeadline>,
) -> Result<File> {
    use std::io::Seek;
    source.rewind().map_err(|_| E::Read)?;
    let before = checked_metadata(&source, mode)?;
    if !before.is_file() || before.len() != len {
        return Err(E::Distribution);
    }
    no_capabilities(&source)?;
    let mut destination = File::options()
        .write(true)
        .read(true)
        .create_new(true)
        .open(output)
        .map_err(|_| E::Publication)?;
    let mut hasher = Context::new(&SHA256);
    let mut total = 0u64;
    let mut buf = [0u8; 65536];
    loop {
        if let Some(d) = deadline {
            d.check()?;
        }
        let n = source.read(&mut buf).map_err(|_| E::Read)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or(E::Limit)?;
        if total > len {
            return Err(E::Digest);
        }
        hasher.update(&buf[..n]);
        destination
            .write_all(&buf[..n])
            .map_err(|_| E::Publication)?;
    }
    let actual = hasher
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let after = checked_metadata(&source, mode)?;
    if total != len
        || actual != digest
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(E::Digest);
    }
    destination
        .set_permissions(std::fs::Permissions::from_mode(mode))
        .map_err(|_| E::Mode)?;
    destination.sync_all().map_err(|_| E::Publication)?;
    // Verify staged bytes through the same opened output, not a replacement path.
    use std::io::SeekFrom;
    destination.seek(SeekFrom::Start(0)).map_err(|_| E::Read)?;
    let mut staged = Context::new(&SHA256);
    loop {
        if let Some(d) = deadline {
            d.check()?;
        }
        let n = destination.read(&mut buf).map_err(|_| E::Read)?;
        if n == 0 {
            break;
        }
        staged.update(&buf[..n]);
    }
    if staged.finish().as_ref() != hex(digest)?.as_slice() {
        return Err(E::Digest);
    }
    let metadata = checked_metadata(&destination, mode)?;
    if !metadata.is_file() || metadata.len() != len {
        return Err(E::Distribution);
    }
    no_capabilities(&destination)?;
    use std::os::fd::AsRawFd;
    let retained =
        File::open(format!("/proc/self/fd/{}", destination.as_raw_fd())).map_err(|_| E::Read)?;
    let m = retained.metadata().map_err(|_| E::Read)?;
    if (m.dev(), m.ino()) != (metadata.dev(), metadata.ino()) {
        return Err(E::ProcessIdentity);
    }
    Ok(retained)
}
fn hex(value: &str) -> Result<Vec<u8>> {
    wire::digest(value)?;
    (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| E::Digest))
        .collect()
}

pub(super) fn snapshot(
    m: &DistributionManifest,
    supplied: &Path,
    executable: &str,
    digest: &str,
) -> Result<VerifiedDistribution> {
    let root = open(
        supplied,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| E::Path)?;
    checked_metadata(
        &File::from(rustix::io::dup(&root).map_err(|_| E::Read)?),
        m.root_mode,
    )?;
    let temp = tempfile::tempdir().map_err(|_| E::Publication)?;
    let snapshot_root = temp.path().join("distribution");
    std::fs::create_dir(&snapshot_root).map_err(|_| E::Publication)?;
    // Retain every validated directory object through copying; never resolve a
    // checked parent pathname again after a rename/replacement.
    let mut directories = BTreeMap::<String, OwnedFd>::new();
    directories.insert(String::new(), root);
    for d in &m.directories {
        let (parent, name) = d.path.rsplit_once('/').unwrap_or(("", &d.path));
        let fd = openat(
            directories.get(parent).ok_or(E::Path)?,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| E::Path)?;
        directories.insert(d.path.clone(), fd);
    }
    let all: BTreeSet<_> = m
        .directories
        .iter()
        .map(|d| d.path.as_str())
        .chain(m.entries.iter().map(Entry::path))
        .collect();
    // Every directory is enumerated through its opened FD; no supplied-path walk.
    for name in std::iter::once("").chain(m.directories.iter().map(|d| d.path.as_str())) {
        let fd = directories.get(name).ok_or(E::Path)?;
        let file = File::from(rustix::io::dup(fd).map_err(|_| E::Read)?);
        let wanted = if name.is_empty() {
            m.root_mode
        } else {
            m.directories
                .iter()
                .find(|d| d.path == name)
                .ok_or(E::Distribution)?
                .mode
        };
        checked_metadata(&file, wanted)?;
        let dir = rustix::fs::Dir::read_from(fd).map_err(|_| E::Read)?;
        let mut count = 0usize;
        for entry in dir {
            count += 1;
            if count > 5122 {
                return Err(E::Limit);
            }
            let entry = entry.map_err(|_| E::Read)?;
            let n = entry.file_name().to_str().map_err(|_| E::Path)?;
            if n == "." || n == ".." {
                continue;
            }
            let path = if name.is_empty() {
                n.to_owned()
            } else {
                format!("{name}/{n}")
            };
            if !all.contains(path.as_str()) {
                return Err(E::Distribution);
            }
        }
    }
    for d in &m.directories {
        std::fs::create_dir(snapshot_root.join(&d.path)).map_err(|_| E::Publication)?;
    }
    let mut files = Vec::new();
    for e in &m.entries {
        let (parent, n) = e.path().rsplit_once('/').unwrap_or(("", e.path()));
        let p = directories.get(parent).ok_or(E::Path)?;
        match e {
            Entry::Regular {
                mode,
                byte_length,
                sha256,
                ..
            } => {
                let fd = openat(
                    p,
                    n,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| E::Read)?;
                let retained = copy_file(
                    File::from(fd),
                    &snapshot_root.join(e.path()),
                    *byte_length,
                    sha256,
                    *mode,
                )?;
                files.push((e.clone(), retained));
            }
            Entry::Symlink { mode, target, .. } => {
                let fd = openat(
                    p,
                    n,
                    OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| E::Read)?;
                let f = File::from(fd);
                let meta = checked_metadata(&f, *mode)?;
                if !meta.file_type().is_symlink() {
                    return Err(E::Symlink);
                }
                let actual = rustix::fs::readlinkat(&f, "", Vec::new()).map_err(|_| E::Symlink)?;
                if actual.as_bytes() != target.as_bytes() {
                    return Err(E::Symlink);
                }
                std::os::unix::fs::symlink(target, snapshot_root.join(e.path()))
                    .map_err(|_| E::Publication)?;
                let staged = File::from(
                    open(
                        snapshot_root.join(e.path()),
                        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| E::Symlink)?,
                );
                if !checked_metadata(&staged, *mode)?.file_type().is_symlink()
                    || rustix::fs::readlinkat(&staged, "", Vec::new())
                        .map_err(|_| E::Symlink)?
                        .as_bytes()
                        != target.as_bytes()
                {
                    return Err(E::Symlink);
                }
            }
        }
    }
    let selected = m
        .entries
        .iter()
        .find(|e| e.path() == executable)
        .ok_or(E::Distribution)?;
    if !matches!(selected,Entry::Regular{executable:true,sha256,..}if sha256==digest) {
        return Err(E::Distribution);
    }
    for d in m.directories.iter().rev() {
        std::fs::set_permissions(
            snapshot_root.join(&d.path),
            std::fs::Permissions::from_mode(d.mode),
        )
        .map_err(|_| E::Mode)?;
    }
    std::fs::set_permissions(&snapshot_root, std::fs::Permissions::from_mode(m.root_mode))
        .map_err(|_| E::Mode)?;
    for (path, mode) in std::iter::once((snapshot_root.clone(), m.root_mode)).chain(
        m.directories
            .iter()
            .map(|d| (snapshot_root.join(&d.path), d.mode)),
    ) {
        let staged = File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| E::Mode)?,
        );
        if !checked_metadata(&staged, mode)?.is_dir() {
            return Err(E::Mode);
        }
    }
    let executable = snapshot_root.join(executable);
    Ok(VerifiedDistribution {
        directory: temp,
        executable,
        files,
        manifest: m.clone(),
        directories: m.directories.iter().map(|d| d.path.clone()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opened_input_survives_path_replacement_and_content_change_fails() {
        let root = tempfile::tempdir().unwrap();
        let p = root.path().join("input");
        std::fs::write(&p, b"original").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        let opened = File::open(&p).unwrap();
        std::fs::rename(&p, root.path().join("old")).unwrap();
        std::fs::write(&p, b"replacement").unwrap();
        copy_file(
            opened,
            &root.path().join("out"),
            8,
            &sha256(b"original").to_string(),
            0o600,
        )
        .unwrap();
        let opened = File::open(&p).unwrap();
        assert!(
            copy_file(
                opened,
                &root.path().join("bad"),
                8,
                &sha256(b"original").to_string(),
                0o600
            )
            .is_err()
        );
    }
    #[test]
    fn private_snapshot_keeps_manifest_root_mode_inside_private_envelope() {
        use std::os::unix::fs::PermissionsExt;
        let supplied = tempfile::tempdir().unwrap();
        std::fs::set_permissions(supplied.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(supplied.path().join("chrome"), b"x").unwrap();
        std::fs::set_permissions(
            supplied.path().join("chrome"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let manifest = super::super::tests::manifest();
        let snapshot = snapshot(
            &manifest,
            supplied.path(),
            "chrome",
            &sha256(b"x").to_string(),
        )
        .unwrap();
        assert_eq!(
            std::fs::metadata(snapshot.root()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(snapshot.executable().parent().unwrap())
                .unwrap()
                .mode()
                & 0o777,
            0o755
        );
        std::fs::write(supplied.path().join("chrome"), b"changed").unwrap();
        assert_eq!(std::fs::read(snapshot.executable()).unwrap(), b"x");
        snapshot.close().unwrap();
    }
}

#[cfg(feature = "chromium-cdp")]
pub(crate) struct FrozenDistribution {
    pub executable: File,
    pub path: PathBuf,
    pub identities: Vec<(u64, u64)>,
}
// Called only inside the new rootless mount namespace, before launching any browser.
#[cfg(feature = "chromium-cdp")]
pub(crate) fn freeze(
    d: &VerifiedDistribution,
    deadline: crate::deadline::AttemptDeadline,
) -> Result<FrozenDistribution> {
    deadline.check()?;
    let root = PathBuf::from("/tmp/ag9g/distribution");
    std::fs::create_dir_all(&root).map_err(|_| E::Publication)?;
    crate::linux_mount::tmpfs(&root)?;
    for dir in &d.manifest.directories {
        deadline.check()?;
        std::fs::create_dir(root.join(&dir.path)).map_err(|_| E::Publication)?;
    }
    let mut main = None;
    let mut identities = Vec::new();
    for (entry, file) in &d.files {
        deadline.check()?;
        if let Entry::Regular {
            path,
            mode,
            byte_length,
            sha256,
            executable,
            ..
        } = entry
        {
            let retained = copy_file_budget(
                file.try_clone().map_err(|_| E::Read)?,
                &root.join(path),
                *byte_length,
                sha256,
                *mode,
                Some(deadline),
            )?;
            if *executable {
                let m = retained.metadata().map_err(|_| E::Read)?;
                identities.push((m.dev(), m.ino()));
            }
            if d.directory.path().join("distribution").join(path) == d.executable {
                main = Some(retained);
            }
        }
    }
    for entry in &d.manifest.entries {
        deadline.check()?;
        if let Entry::Symlink { path, target, .. } = entry {
            std::os::unix::fs::symlink(target, root.join(path)).map_err(|_| E::Symlink)?;
        }
    }
    for dir in d.manifest.directories.iter().rev() {
        std::fs::set_permissions(
            root.join(&dir.path),
            std::fs::Permissions::from_mode(dir.mode),
        )
        .map_err(|_| E::Mode)?;
    }
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(d.manifest.root_mode))
        .map_err(|_| E::Mode)?;
    // Fresh tmpfs has no writable host alias. Remount before any exec; browser has no mount capability.
    crate::linux_mount::readonly(&root)?;
    deadline.check()?;
    Ok(FrozenDistribution {
        executable: main.ok_or(E::Distribution)?,
        path: root.join(
            d.executable
                .strip_prefix(d.directory.path().join("distribution"))
                .map_err(|_| E::Path)?,
        ),
        identities,
    })
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    fn objects() -> (tempfile::TempDir, VerifiedDistribution) {
        let supplied = tempfile::tempdir().unwrap();
        std::fs::set_permissions(supplied.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut m = super::super::tests::manifest();
        m.entries.push(Entry::Regular {
            path: "helper".into(),
            mode: 0o755,
            executable: true,
            byte_length: 1,
            sha256: sha256(b"x").to_string(),
            file_capabilities: "absent".into(),
        });
        for p in ["chrome", "helper"] {
            std::fs::write(supplied.path().join(p), b"x").unwrap();
            std::fs::set_permissions(
                supplied.path().join(p),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        let d = snapshot(&m, supplied.path(), "chrome", &sha256(b"x").to_string()).unwrap();
        (supplied, d)
    }
    #[test]
    fn supplied_main_staged_main_and_helper_replacements_never_supply_frozen_bytes() {
        let (supplied, d) = objects();
        let out = tempfile::tempdir().unwrap();
        std::fs::write(supplied.path().join("chrome"), b"unreviewed supplied bytes").unwrap();
        for name in ["chrome", "helper"] {
            let path = d.root().join("distribution").join(name);
            std::fs::rename(&path, path.with_extension("old")).unwrap();
            std::fs::write(&path, b"unreviewed staged bytes").unwrap();
        }
        for (e, file) in &d.files {
            if let Entry::Regular {
                path,
                byte_length,
                sha256,
                mode,
                ..
            } = e
            {
                let checked = copy_file(
                    file.try_clone().unwrap(),
                    &out.path().join(path),
                    *byte_length,
                    sha256,
                    *mode,
                )
                .unwrap();
                assert_eq!(checked.metadata().unwrap().len(), 1);
                assert_eq!(std::fs::read(out.path().join(path)).unwrap(), b"x");
            }
        }
    }
    #[test]
    fn in_place_change_to_retained_helper_is_rejected_before_exec() {
        let (_supplied, d) = objects();
        let out = tempfile::tempdir().unwrap();
        std::fs::write(d.root().join("distribution/helper"), b"y").unwrap();
        let (_, f) = d.files.iter().find(|(e, _)| e.path() == "helper").unwrap();
        assert!(
            copy_file(
                f.try_clone().unwrap(),
                &out.path().join("helper"),
                1,
                &sha256(b"x").to_string(),
                0o755
            )
            .is_err()
        );
    }
    #[cfg(feature = "chromium-cdp")]
    #[test]
    #[ignore = "explicit rootless Linux mount test; no browser; requires /usr/bin/unshare"]
    fn immutable_tree_rejects_main_and_helper_replacement() {
        if std::env::var_os("AG9G_TEST_MOUNT_CHILD").is_none() {
            let status=std::process::Command::new("/usr/bin/unshare").args(["--user","--map-current-user","--mount"])
    .arg(std::env::current_exe().unwrap()).args(["--ignored","--exact","distribution::linux::replacement_tests::immutable_tree_rejects_main_and_helper_replacement"])
    .env("AG9G_TEST_MOUNT_CHILD","1").status().unwrap();
            assert!(status.success());
            return;
        }
        let (_supplied, d) = objects();
        crate::linux_mount::private_namespace().unwrap();
        crate::linux_mount::tmpfs(Path::new("/tmp")).unwrap();
        let frozen = freeze(&d, crate::deadline::AttemptDeadline::new()).unwrap();
        crate::linux_mount::readonly(Path::new("/tmp")).unwrap();
        for name in ["chrome", "helper"] {
            let path = frozen.path.parent().unwrap().join(name);
            assert!(std::fs::write(&path, b"malicious").is_err());
            assert!(std::fs::remove_file(&path).is_err());
            assert!(std::fs::rename(&path, path.with_extension("old")).is_err());
        }
        assert_eq!(frozen.executable.metadata().unwrap().len(), 1);
    }
}
