use crate::{Error, Result, error::require};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
pub const RECORD_BYTES: usize = 65_536;
pub fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn identity(s: &str) -> Result<()> {
    require(
        !s.is_empty() && s.len() <= 128 && s.trim() == s && !s.chars().any(char::is_control),
        "identity",
    )
}
pub fn digest(s: &str) -> Result<()> {
    require(
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "sha256",
    )
}
pub fn relative(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 256
            && s.split('/').all(|p| {
                !p.is_empty()
                    && p != "."
                    && p != ".."
                    && p.len() <= 64
                    && p.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            }),
        "relative path",
    )
}
pub fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        File::options()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?
    };
    #[cfg(not(unix))]
    let mut file = File::open(path)?;
    read_retained(&mut file, max)
}
#[cfg(unix)]
fn stable(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
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
fn read_retained(file: &mut File, max: usize) -> Result<Vec<u8>> {
    #[cfg(not(unix))]
    {
        let _ = (file, max);
        Err(Error::Unsupported)
    }
    #[cfg(unix)]
    {
        let before = file.metadata()?;
        require(
            before.is_file() && before.len() <= max as u64,
            "bounded regular input",
        )?;
        let mut bytes = Vec::new();
        (&mut *file).take(max as u64 + 1).read_to_end(&mut bytes)?;
        #[cfg(test)]
        read_tests::checkpoint();
        require(stable(&before, &file.metadata()?), "input object changed")?;
        require(
            bytes.len() <= max && bytes.len() as u64 == before.len(),
            "input size changed",
        )?;
        Ok(bytes)
    }
}

pub fn json<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let mut b = serde_json::to_vec(v).map_err(|_| Error::Invalid("record serialization"))?;
    b.push(b'\n');
    require(b.len() <= RECORD_BYTES, "record limit")?;
    Ok(b)
}
pub fn record<T: Serialize + DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read(path, RECORD_BYTES)?;
    let v: T = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid("record schema"))?;
    require(json(&v)? == bytes, "noncanonical record")?;
    Ok(v)
}
pub struct Writer {
    bytes: Vec<u8>,
    max: usize,
}
impl Writer {
    pub fn new(max: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }
    pub fn raw(&mut self, s: &str) -> Result<()> {
        require(
            self.bytes
                .len()
                .checked_add(s.len())
                .is_some_and(|n| n <= self.max),
            "canonical byte limit",
        )?;
        self.bytes
            .try_reserve(s.len())
            .map_err(|_| Error::Invalid("allocation"))?;
        self.bytes.extend_from_slice(s.as_bytes());
        Ok(())
    }
    pub fn field<T: Serialize>(&mut self, key: &str, v: T) -> Result<()> {
        self.raw(key)?;
        self.raw(" = ")?;
        self.raw(&serde_json::to_string(&v).map_err(|_| Error::Invalid("canonical field"))?)?;
        self.raw("\n")
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
/// Complete bytes become visible through one exclusive rename in an owned directory.
pub fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        PublicationDirectory::open(path)?.publish(bytes)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (path, bytes);
        Err(Error::Unsupported)
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
struct PublicationDirectory {
    directory: File,
    parent: std::path::PathBuf,
    final_name: std::ffi::CString,
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl PublicationDirectory {
    fn open(path: &Path) -> Result<Self> {
        use std::os::unix::{ffi::OsStrExt, fs::OpenOptionsExt};
        require(
            path.is_absolute() && path.file_name().is_some(),
            "absolute output path",
        )?;
        let parent = path
            .parent()
            .ok_or(Error::Invalid("output parent"))?
            .to_owned();
        let directory = File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&parent)?;
        let final_name = std::ffi::CString::new(
            path.file_name()
                .ok_or(Error::Invalid("output name"))?
                .as_bytes(),
        )
        .map_err(|_| Error::Invalid("output NUL"))?;
        Ok(Self {
            directory,
            parent,
            final_name,
        })
    }
    fn parent_bound(&self) -> Result<()> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let current = File::options()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&self.parent)?;
        let a = self.directory.metadata()?;
        let b = current.metadata()?;
        require(
            (a.dev(), a.ino()) == (b.dev(), b.ino()),
            "publication parent changed",
        )
    }
    fn bound(&self, name: &std::ffi::CStr, retained: &File) -> Result<()> {
        use std::os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        };
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let actual = unsafe { File::from_raw_fd(fd) }.metadata()?;
        let expected = retained.metadata()?;
        require(
            actual.is_file() && (actual.dev(), actual.ino()) == (expected.dev(), expected.ino()),
            "publication entry changed",
        )
    }
    fn remove(&self, name: &std::ffi::CStr, retained: &File) -> Result<()> {
        use std::os::fd::AsRawFd;
        self.bound(name, retained).map_err(|_| Error::Cleanup)?;
        require(
            unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } == 0,
            "publication unlink",
        )
        .map_err(|_| Error::Cleanup)
    }
    fn publish(&self, bytes: &[u8]) -> Result<()> {
        use ring::rand::SecureRandom;
        use std::os::fd::{AsRawFd, FromRawFd};
        let mut staged = None;
        for _ in 0..16 {
            let mut random = [0u8; 16];
            ring::rand::SystemRandom::new()
                .fill(&mut random)
                .map_err(|_| Error::Invalid("temporary randomness"))?;
            let component = format!(".qualification-prep-{}", hash(&random));
            let name =
                std::ffi::CString::new(component).map_err(|_| Error::Invalid("temporary name"))?;
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd >= 0 {
                staged = Some((name, unsafe { File::from_raw_fd(fd) }));
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EEXIST) {
                return Err(error.into());
            }
        }
        let (name, mut file) = staged.ok_or(Error::Invalid("temporary collisions"))?;
        let mut published = false;
        let operation = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            #[cfg(test)]
            publication_tests::checkpoint()?;
            #[cfg(test)]
            publication_tests::mutation("staged", &self.parent, &name);
            self.parent_bound()?;
            self.bound(&name, &file)?;
            #[cfg(target_os = "linux")]
            let rc = unsafe {
                libc::syscall(
                    libc::SYS_renameat2,
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    self.directory.as_raw_fd(),
                    self.final_name.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            #[cfg(target_os = "macos")]
            let rc = unsafe {
                libc::renameatx_np(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    self.directory.as_raw_fd(),
                    self.final_name.as_ptr(),
                    libc::RENAME_EXCL,
                )
            };
            if rc != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            published = true;
            #[cfg(test)]
            publication_tests::mutation("published", &self.parent, &self.final_name);
            self.parent_bound()?;
            self.bound(&self.final_name, &file)
        })();
        match operation {
            Ok(()) => Ok(()),
            Err(error) => {
                let cleanup = self.remove(if published { &self.final_name } else { &name }, &file);
                #[cfg(test)]
                let cleanup = publication_tests::cleanup(cleanup);
                crate::error::after_cleanup(Err(error), cleanup)
            }
        }
    }
}
/// Traverse each relative component through retained directory descriptors.
#[cfg(unix)]
pub fn read_confined(root: &Path, name: &str, max: usize) -> Result<Vec<u8>> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::OpenOptionsExt,
        },
    };
    relative(name)?;
    let mut dir = File::options()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let parts: Vec<_> = name.split('/').collect();
    for (i, p) in parts.iter().enumerate() {
        let p = CString::new(*p).map_err(|_| Error::Invalid("path"))?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if i + 1 < parts.len() {
                libc::O_DIRECTORY
            } else {
                0
            };
        let fd = unsafe { libc::openat(dir.as_raw_fd(), p.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        dir = unsafe { File::from_raw_fd(fd) };
    }
    read_retained(&mut dir, max)
}
#[cfg(not(unix))]
pub fn read_confined(_: &Path, _: &str, _: usize) -> Result<Vec<u8>> {
    Err(Error::Unsupported)
}

/// Deserialization stops at the retained population bound rather than allocating
/// an arbitrary Vec first and validating its length afterwards.
pub fn sequence<'de, D, T, const MAX: usize>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Bounded<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Bounded<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "at most {N} records")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            use serde::de::Error;
            let mut out = Vec::new();
            while let Some(v) = seq.next_element()? {
                if out.len() == N {
                    return Err(A::Error::custom("population bound"));
                }
                out.try_reserve(1)
                    .map_err(|_| A::Error::custom("allocation"))?;
                out.push(v);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(Bounded::<T, MAX>(std::marker::PhantomData))
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    thread_local! {static FAIL:std::cell::Cell<u8>=const{std::cell::Cell::new(0)};}
    type Mutation = Box<dyn FnMut(&str, &Path, &std::ffi::CStr)>;
    thread_local! {static MUTATION:std::cell::RefCell<Option<Mutation>>=std::cell::RefCell::new(None);}
    pub(super) fn mutation(stage: &str, parent: &Path, name: &std::ffi::CStr) {
        MUTATION.with_borrow_mut(|hook| {
            if let Some(h) = hook {
                h(stage, parent, name);
            }
        });
    }
    #[test]
    fn parent_rebinding_and_hardlink_cannot_redirect_publication() {
        for stage in ["staged", "published"] {
            let outer = tempfile::tempdir().unwrap();
            let parent = outer.path().join("output");
            let moved = outer.path().join("retained");
            std::fs::create_dir(&parent).unwrap();
            let renamed = moved.clone();
            MUTATION.set(Some(Box::new(move |at, parent, name| {
                if at == stage {
                    std::fs::rename(parent, &renamed).unwrap();
                    std::fs::create_dir(parent).unwrap();
                    // Even a hard link to the exact staged inode must not confer parent authority.
                    std::fs::hard_link(
                        renamed.join(name.to_str().unwrap()),
                        parent.join(name.to_str().unwrap()),
                    )
                    .unwrap();
                }
            })));
            let result = publish_new(&parent.join("candidate"), b"complete");
            MUTATION.set(None);
            assert_eq!(result, Err(Error::Invalid("publication parent changed")));
            assert_eq!(std::fs::read_dir(&moved).unwrap().count(), 0);
            if stage == "staged" {
                assert!(!parent.join("candidate").exists());
            }
            // A third-party hard link is outside owned cleanup; it cannot redirect rename/unlink.
            assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 1);
        }
    }
    pub(super) fn checkpoint() -> Result<()> {
        if FAIL.get() != 0 {
            Err(Error::Invalid("injected staging failure"))
        } else {
            Ok(())
        }
    }
    pub(super) fn cleanup(result: Result<()>) -> Result<()> {
        if FAIL.get() == 2 {
            Err(Error::Cleanup)
        } else {
            result
        }
    }
    #[test]
    fn staged_failure_leaves_no_partial_candidate_and_cleanup_error_wins() {
        let d = tempfile::tempdir().unwrap();
        for mode in [1, 2] {
            FAIL.set(mode);
            let result = publish_new(&d.path().join("candidate"), b"complete bytes");
            assert!(result.is_err());
            if mode == 2 {
                assert_eq!(result, Err(Error::Cleanup));
            }
            assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
        }
        FAIL.set(0);
    }
}

#[cfg(all(test, unix))]
mod read_tests {
    use super::*;
    type Hook = Box<dyn FnMut()>;
    thread_local! {static HOOK:std::cell::RefCell<Option<Hook>>=std::cell::RefCell::new(None);}
    pub(super) fn checkpoint() {
        HOOK.with_borrow_mut(|h| {
            if let Some(h) = h {
                h();
            }
        });
    }
    #[test]
    fn same_size_mutation_rejects_ordinary_and_confined_reads() {
        for confined in [false, true] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("input");
            std::fs::write(&path, b"original").unwrap();
            let initial = std::fs::metadata(&path).unwrap();
            let changed = path.clone();
            HOOK.set(Some(Box::new(move || {
                std::fs::write(&changed, b"modified").unwrap();
                // Set an explicit distinct mtime, avoiding filesystem timestamp-resolution assumptions.
                File::options()
                    .write(true)
                    .open(&changed)
                    .unwrap()
                    .set_times(
                        std::fs::FileTimes::new().set_modified(
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1),
                        ),
                    )
                    .unwrap();
            })));
            let result = if confined {
                read_confined(d.path(), "input", 64)
            } else {
                read(&path, 64)
            };
            HOOK.set(None);
            assert_eq!(result, Err(Error::Invalid("input object changed")));
            assert_eq!(std::fs::metadata(&path).unwrap().len(), initial.len());
            assert_eq!(read(&path, 64).unwrap(), b"modified");
            assert_eq!(read_confined(d.path(), "input", 64).unwrap(), b"modified");
        }
    }
}
