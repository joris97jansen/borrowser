//! Independent Linux producer. Descriptors are inventory authority; paths are labels.
use crate::{Error, Result, canonical, distribution::*, error::require};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::Path,
};
fn cstr(b: &[u8]) -> Result<CString> {
    CString::new(b).map_err(|_| Error::Invalid("NUL path"))
}
pub fn open_root(path: &Path) -> Result<File> {
    require(path.is_absolute(), "absolute explicit root")?;
    let c = cstr(path.as_os_str().as_bytes())?;
    let fd = unsafe {
        libc::open(
            c.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn at(dir: &File, name: &str, flags: i32) -> Result<File> {
    let c = cstr(name.as_bytes())?;
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            c.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
pub fn confined(root: &File, relative: &str) -> Result<File> {
    canonical::relative(relative)?;
    let mut parent = root.try_clone()?;
    let mut parts = relative.split('/').peekable();
    while let Some(p) = parts.next() {
        parent = at(
            &parent,
            p,
            libc::O_RDONLY
                | if parts.peek().is_some() {
                    libc::O_DIRECTORY
                } else {
                    0
                },
        )?;
    }
    require(parent.metadata()?.is_file(), "regular confined source")?;
    Ok(parent)
}
fn stable(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
        && a.nlink() == b.nlink()
}
pub fn metadata(m: &std::fs::Metadata) -> Result<()> {
    owner(m.uid(), unsafe { libc::getuid() })?;
    if !m.file_type().is_symlink() {
        mode(m.mode() & 0o7777)?;
    }
    Ok(())
}
pub fn no_capabilities(file: &File) -> Result<()> {
    let mut buf = [0u8; 128];
    let n = unsafe {
        libc::fgetxattr(
            file.as_raw_fd(),
            c"security.capability".as_ptr(),
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    capability_result(n, std::io::Error::last_os_error().raw_os_error())
}
pub fn file_digest(file: &mut File, max: u64) -> Result<String> {
    let before = file.metadata()?;
    require(before.is_file() && before.len() <= max, "regular file size")?;
    let mut h = ring::digest::Context::new(&ring::digest::SHA256);
    let mut total = 0u64;
    let mut b = [0u8; 65536];
    loop {
        let n = file.read(&mut b)?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or(Error::Invalid("file overflow"))?;
        require(total <= max && total <= before.len(), "file grew")?;
        h.update(&b[..n]);
    }
    require(
        total == before.len() && stable(&before, &file.metadata()?),
        "file changed",
    )?;
    Ok(h.finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn names(dir: &File) -> Result<Vec<String>> {
    // Open '.' anew: dup would share the enumeration offset.
    let fd = at(dir, ".", libc::O_RDONLY | libc::O_DIRECTORY)?;
    use std::os::fd::IntoRawFd;
    let raw = fd.into_raw_fd();
    let stream = unsafe { libc::fdopendir(raw) };
    if stream.is_null() {
        unsafe { libc::close(raw) };
        return Err(std::io::Error::last_os_error().into());
    }
    let result = (|| {
        let mut names = Vec::new();
        loop {
            unsafe {
                *libc::__errno_location() = 0;
            }
            let ent = unsafe { libc::readdir(stream) };
            if ent.is_null() {
                require(unsafe { *libc::__errno_location() } == 0, "directory read")?;
                break;
            }
            let b = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) }.to_bytes();
            if b == b"." || b == b".." {
                continue;
            }
            require(names.len() < 5120, "directory population")?;
            let s = std::str::from_utf8(b).map_err(|_| Error::Invalid("filename UTF-8"))?;
            canonical::relative(s)?;
            names.push(s.to_owned());
        }
        names.sort();
        require(
            names.windows(2).all(|p| p[0] != p[1]),
            "duplicate enumeration",
        )?;
        Ok(names)
    })();
    let cleanup = unsafe { libc::closedir(stream) };
    if cleanup != 0 {
        return Err(Error::Cleanup);
    }
    result
}
struct Directory {
    file: File,
    label: String,
    before: std::fs::Metadata,
    names: Vec<String>,
}
// Each name is a single validated component relative to an owned directory.
// Keeping the original object open prevents inode reuse while the binding lives.
struct RetainedBinding {
    parent: usize,
    name: String,
    object: File,
    before: std::fs::Metadata,
}
impl RetainedBinding {
    fn verify(&self, directories: &[Directory]) -> Result<()> {
        let current = at(&directories[self.parent].file, &self.name, libc::O_PATH)?;
        let actual = current.metadata()?;
        require(
            self.before.dev() == actual.dev() && self.before.ino() == actual.ino(),
            "directory entry binding changed",
        )?;
        require(
            stable(&self.before, &actual),
            "bound object metadata changed",
        )
    }
}
pub fn inventory(root: &Path) -> Result<CandidateDistributionManifest> {
    let file = open_root(root)?;
    let before = file.metadata()?;
    metadata(&before)?;
    no_capabilities(&file)?;
    let mut dirs = vec![Directory {
        names: names(&file)?,
        file,
        label: String::new(),
        before,
    }];
    let mut m = CandidateDistributionManifest {
        format: FORMAT.into(),
        root_mode: dirs[0].before.mode() & 0o7777,
        directories: vec![],
        entries: vec![],
    };
    #[cfg(test)]
    tests::checkpoint("enumerated", root);
    let mut retained_population = dirs[0].names.len();
    let mut index = 0;
    let mut total = 0u64;
    let mut bindings = Vec::new();
    while index < dirs.len() {
        for name in dirs[index].names.clone() {
            let label = if dirs[index].label.is_empty() {
                name.clone()
            } else {
                format!("{}/{name}", dirs[index].label)
            };
            canonical::relative(&label)?;
            let object = at(&dirs[index].file, &name, libc::O_PATH)?;
            let before = object.metadata()?;
            metadata(&before)?;
            #[cfg(test)]
            tests::checkpoint("opened", &root.join(&label));
            if before.is_dir() {
                require(dirs.len() <= 1024, "directory count")?;
                let file = at(&dirs[index].file, &name, libc::O_RDONLY | libc::O_DIRECTORY)?;
                require(stable(&before, &file.metadata()?), "directory replacement")?;
                no_capabilities(&file)?;
                m.directories.push(DirectoryRecord {
                    path: label.clone(),
                    mode: before.mode() & 0o7777,
                });
                let child_names = names(&file)?;
                retained_population = retained_population
                    .checked_add(child_names.len())
                    .ok_or(Error::Invalid("population overflow"))?;
                require(retained_population <= 5120, "total enumerated population")?;
                dirs.push(Directory {
                    names: child_names,
                    file,
                    label,
                    before: before.clone(),
                });
            } else {
                require(m.entries.len() < 4096, "entry count")?;
                if before.is_file() {
                    total = total
                        .checked_add(before.len())
                        .ok_or(Error::Invalid("total overflow"))?;
                    require(
                        before.len() <= FILE_BYTES && total <= TOTAL_BYTES,
                        "content bounds",
                    )?;
                    let mut file = at(&dirs[index].file, &name, libc::O_RDONLY)?;
                    require(stable(&before, &file.metadata()?), "file replacement")?;
                    no_capabilities(&file)?;
                    let sha256 = file_digest(&mut file, FILE_BYTES)?;
                    no_capabilities(&file)?;
                    require(
                        stable(&before, &object.metadata()?),
                        "file metadata changed",
                    )?;
                    m.entries.push(EntryRecord::Regular {
                        path: label,
                        mode: before.mode() & 0o7777,
                        executable: before.mode() & 0o111 != 0,
                        byte_length: before.len(),
                        sha256,
                        file_capabilities: "absent".into(),
                    });
                } else if before.file_type().is_symlink() {
                    let mut bytes = [0u8; 1025];
                    let n = unsafe {
                        libc::readlinkat(
                            object.as_raw_fd(),
                            c"".as_ptr(),
                            bytes.as_mut_ptr().cast(),
                            bytes.len(),
                        )
                    };
                    require(n > 0 && n <= 1024, "link target length/read")?;
                    let target = std::str::from_utf8(&bytes[..n as usize])
                        .map_err(|_| Error::Invalid("link UTF-8"))?
                        .to_owned();
                    require(stable(&before, &object.metadata()?), "link changed")?;
                    resolve(&label, &target)?;
                    m.entries.push(EntryRecord::Symlink {
                        path: label,
                        mode: before.mode() & 0o7777,
                        executable: false,
                        target_sha256: canonical::hash(target.as_bytes()),
                        target,
                        resolved_path: String::new(),
                    });
                } else {
                    return Err(Error::Invalid("unsupported filesystem object"));
                }
            }
            bindings.push(RetainedBinding {
                parent: index,
                name,
                object,
                before,
            });
        }
        index += 1;
    }
    #[cfg(test)]
    tests::checkpoint("inventoried", root);
    // Check reachability first so substitution has its own diagnostic, even if
    // rename also changed directory timestamps or the detached inode's ctime.
    // O_PATH | O_NOFOLLOW inspects links themselves and cannot block on a FIFO.
    for binding in &bindings {
        binding.verify(&dirs)?;
    }
    // Recheck every retained directory, not a path reopened after traversal.
    for d in &dirs {
        require(
            stable(&d.before, &d.file.metadata()?) && names(&d.file)? == d.names,
            "directory population changed",
        )?;
    }
    for binding in &bindings {
        require(
            stable(&binding.before, &binding.object.metadata()?),
            "object mutated after inventory",
        )?;
    }
    m.directories.sort_by(|a, b| a.path.cmp(&b.path));
    m.entries.sort_by(|a, b| a.path().cmp(b.path()));
    let snapshot = m.entries.clone();
    let lookup: BTreeMap<_, _> = snapshot.iter().map(|e| (e.path(), e)).collect();
    for e in &mut m.entries {
        if let EntryRecord::Symlink {
            path,
            target,
            resolved_path,
            ..
        } = e
        {
            *resolved_path = resolve_chain(path, target, &lookup)?;
        }
    }
    m.validate()?;
    Ok(m)
}

fn owner(actual: u32, current: u32) -> Result<()> {
    require(actual == 0 || actual == current, "owner")
}
fn capability_result(n: isize, error: Option<i32>) -> Result<()> {
    require(
        n < 0 && error == Some(libc::ENODATA),
        "unprovable/attached capabilities",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    type Hook = Box<dyn FnMut(&str, &Path)>;
    thread_local! {static HOOK:std::cell::RefCell<Option<Hook>>=std::cell::RefCell::new(None);}
    pub(super) fn checkpoint(stage: &str, path: &Path) {
        HOOK.with_borrow_mut(|h| {
            if let Some(h) = h {
                h(stage, path)
            }
        });
    }
    #[test]
    fn metadata_and_capability_results_fail_closed() {
        owner(1000, 1000).unwrap();
        owner(0, 1000).unwrap();
        assert!(owner(1001, 1000).is_err());
        capability_result(-1, Some(libc::ENODATA)).unwrap();
        for (n, e) in [
            (0, None),
            (20, None),
            (-1, Some(libc::EACCES)),
            (-1, Some(libc::ENOTSUP)),
            (-1, Some(libc::EBADF)),
        ] {
            assert!(capability_result(n, e).is_err());
        }
        let fd = File::open("/dev/null").unwrap();
        assert!(!fd.metadata().unwrap().is_file());
    }
    // Real rename-over after hashing/enumeration, with no extra supplied-tree
    // names. Identical content/target and modes cannot substitute for identity.
    fn same_name_replacement(kind: &str) {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("distribution");
        std::fs::create_dir(&root).unwrap();
        let entry = root.join("entry");
        let replacement = outer.path().join("replacement");
        match kind {
            "file" => {
                std::fs::write(&entry, b"unchanged bytes").unwrap();
                std::fs::write(&replacement, b"unchanged bytes").unwrap();
            }
            "symlink" => {
                std::fs::write(root.join("target"), b"x").unwrap();
                symlink("target", &entry).unwrap();
                symlink("target", &replacement).unwrap();
            }
            "directory" => {
                std::fs::create_dir(&entry).unwrap();
                std::fs::create_dir(&replacement).unwrap();
            }
            _ => unreachable!(),
        }
        if kind != "symlink" {
            std::fs::set_permissions(
                &replacement,
                std::fs::Permissions::from_mode(std::fs::metadata(&entry).unwrap().mode() & 0o7777),
            )
            .unwrap();
        }
        let parent = open_root(&root).unwrap();
        let old = at(&parent, "entry", libc::O_PATH).unwrap();
        let before = old.metadata().unwrap();
        let population = names(&parent).unwrap();
        let readable = (kind == "file").then(|| File::open(&entry).unwrap());
        let invoked = std::rc::Rc::new(std::cell::Cell::new(false));
        let observed = invoked.clone();
        HOOK.set(Some(Box::new(move |stage, path| {
            if stage == "inventoried" {
                std::fs::rename(&replacement, path.join("entry")).unwrap();
                observed.set(true);
            }
        })));
        let result = inventory(&root);
        HOOK.set(None);
        assert!(invoked.get());
        assert_eq!(
            result.unwrap_err(),
            Error::Invalid("directory entry binding changed")
        );
        assert_eq!(names(&parent).unwrap(), population);
        let detached = old.metadata().unwrap();
        assert_eq!(
            (
                detached.dev(),
                detached.ino(),
                detached.mode(),
                detached.len()
            ),
            (before.dev(), before.ino(), before.mode(), before.len())
        );
        let reachable = at(&parent, "entry", libc::O_PATH)
            .unwrap()
            .metadata()
            .unwrap();
        assert_ne!(
            (reachable.dev(), reachable.ino()),
            (before.dev(), before.ino())
        );
        if let Some(mut readable) = readable {
            let mut contents = String::new();
            readable.read_to_string(&mut contents).unwrap();
            assert_eq!(contents, "unchanged bytes");
        }
        // Linux may update ctime/nlink on rename-over. The binding-specific
        // error above proves rejection does not depend on those side effects.
    }
    #[test]
    fn regular_same_name_replacement_after_hashing() {
        same_name_replacement("file");
    }
    #[test]
    fn symlink_same_name_replacement_after_inventory() {
        same_name_replacement("symlink");
    }
    #[test]
    fn child_directory_same_name_replacement_after_inventory() {
        same_name_replacement("directory");
    }
    #[test]
    fn replacement_and_mutation_are_detected() {
        for stage in ["enumerated", "opened", "inventoried"] {
            let d = tempfile::tempdir().unwrap();
            std::fs::write(d.path().join("chrome"), b"x").unwrap();
            let expected = stage.to_owned();
            let mut done = false;
            HOOK.set(Some(Box::new(move |stage, path| {
                if stage == expected && !done {
                    done = true;
                    if stage == "opened" {
                        std::fs::rename(path, path.with_file_name("old")).unwrap();
                        std::fs::write(path, b"replacement").unwrap();
                    } else {
                        std::fs::write(path.join("new"), b"new").unwrap();
                    }
                }
            })));
            assert!(inventory(d.path()).is_err(), "{stage}");
            HOOK.set(None);
        }
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("chrome"), b"x").unwrap();
        HOOK.set(Some(Box::new(|stage, path| {
            if stage == "inventoried" {
                std::fs::write(path.join("chrome"), b"changed").unwrap();
            }
        })));
        assert!(inventory(d.path()).is_err());
        HOOK.set(None);
    }
}
