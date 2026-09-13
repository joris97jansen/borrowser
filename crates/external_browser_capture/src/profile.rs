#![cfg_attr(not(target_os = "linux"), allow(dead_code))]
//! Profiles are created only in an exclusive private mount, before browser fork.
use crate::{CaptureError as E, Result, deadline::AttemptDeadline};
use std::{fs::File, os::unix::fs::MetadataExt};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileIdentity {
    pub device: u64,
    pub inode: u64,
}
pub struct FreshProfile {
    directory: File,
    identity: ProfileIdentity,
}
impl FreshProfile {
    #[cfg(target_os = "linux")]
    pub(crate) fn create(
        workspace: &crate::linux_mount::PrivateWorkspace,
        deadline: AttemptDeadline,
    ) -> Result<Self> {
        Self::create_with(workspace, deadline, || {})
    }
    #[cfg(target_os = "linux")]
    fn create_with(
        workspace: &crate::linux_mount::PrivateWorkspace,
        deadline: AttemptDeadline,
        after_mkdir: impl FnOnce(),
    ) -> Result<Self> {
        deadline.check()?;
        let parent = workspace.directory();
        rustix::fs::mkdirat(parent, "profile", rustix::fs::Mode::from_raw_mode(0o700))
            .map_err(|_| E::Launch)?;
        after_mkdir();
        // Fresh filesystem root, not the replaceable placeholder directory, is
        // the profile. Even substitution at the old mkdir/open interval cannot
        // supply profile bytes: a new empty tmpfs covers that placeholder.
        use std::os::fd::AsRawFd;
        let target =
            std::path::PathBuf::from(format!("/proc/self/fd/{}/profile", parent.as_raw_fd()));
        crate::linux_mount::tmpfs(&target)?;
        workspace.seal()?;
        // The private tmpfs and its parent have no other executing population or
        // host-backed alias. No untrusted process can replace the new entry.
        let directory = rustix::fs::openat2(
            parent,
            "profile",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            rustix::fs::ResolveFlags::BENEATH
                | rustix::fs::ResolveFlags::NO_SYMLINKS
                | rustix::fs::ResolveFlags::NO_MAGICLINKS,
        )
        .map(File::from)
        .map_err(|_| E::Path)?;
        let profile = Self::retain(directory)?;
        profile.verify_empty()?;
        deadline.check()?;
        Ok(profile)
    }
    pub(crate) fn retain(directory: File) -> Result<Self> {
        let m = directory.metadata().map_err(|_| E::Read)?;
        if !m.is_dir() {
            return Err(E::Path);
        }
        Ok(Self {
            directory,
            identity: ProfileIdentity {
                device: m.dev(),
                inode: m.ino(),
            },
        })
    }
    #[cfg(any(target_os = "linux", test))]
    fn verify_empty(&self) -> Result<()> {
        for e in rustix::fs::Dir::read_from(&self.directory).map_err(|_| E::Read)? {
            let e = e.map_err(|_| E::Read)?;
            if ![b".".as_slice(), b"..".as_slice()].contains(&e.file_name().to_bytes()) {
                return Err(E::Launch);
            }
        }
        Ok(())
    }
    pub fn identity(&self) -> ProfileIdentity {
        self.identity
    }
    pub(crate) fn verify_object(&self, visible: &File, deadline: AttemptDeadline) -> Result<()> {
        deadline.check()?;
        let m = visible.metadata().map_err(|_| E::Read)?;
        if !m.is_dir() || (m.dev(), m.ino()) != (self.identity.device, self.identity.inode) {
            return Err(E::ProcessIdentity);
        }
        let retained = self.directory.metadata().map_err(|_| E::Read)?;
        if (retained.dev(), retained.ino()) != (m.dev(), m.ino()) {
            return Err(E::ProcessIdentity);
        }
        deadline.check()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonempty_replacement_wrong_identity_and_expiry() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let p = FreshProfile::retain(File::open(a.path()).unwrap()).unwrap();
        assert!(p.verify_empty().is_ok());
        std::fs::write(a.path().join("prefs"), b"x").unwrap();
        assert_eq!(p.verify_empty(), Err(E::Launch));
        assert_eq!(
            p.verify_object(&File::open(b.path()).unwrap(), AttemptDeadline::new()),
            Err(E::ProcessIdentity)
        );
        assert_eq!(
            p.verify_object(&File::open(a.path()).unwrap(), AttemptDeadline::expired()),
            Err(E::Deadline)
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod private_runtime_tests {
    use super::*;
    #[test]
    #[ignore = "explicit rootless Linux mount/profile race test"]
    fn private_profile_old_creation_race() {
        if std::env::var_os("AG9G_PROFILE_TEST_PRIVATE").is_none() {
            let status = std::process::Command::new("/usr/bin/unshare")
                .args(["--user", "--map-current-user", "--mount"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "profile::private_runtime_tests::private_profile_old_creation_race",
                    "--ignored",
                    "--test-threads=1",
                ])
                .env("AG9G_PROFILE_TEST_PRIVATE", "1")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        crate::linux_mount::private_namespace().unwrap();
        crate::linux_mount::tmpfs(std::path::Path::new("/tmp")).unwrap();
        std::fs::create_dir("/tmp/ag9g").unwrap();
        let workspace = crate::linux_mount::PrivateWorkspace::create().unwrap();
        let profile = FreshProfile::create_with(&workspace, AttemptDeadline::new(), || {
            // Exact former race: substitute a preexisting, nonempty directory
            // between mkdir and acquiring the profile FD.
            std::fs::rename("/tmp/ag9g/workspace/profile", "/tmp/ag9g/workspace/old").unwrap();
            std::fs::create_dir("/tmp/ag9g/workspace/profile").unwrap();
            std::fs::write("/tmp/ag9g/workspace/profile/injected", b"not fresh").unwrap();
        })
        .unwrap();
        profile.verify_empty().unwrap();
        assert!(
            std::fs::rename("/tmp/ag9g/workspace/profile", "/tmp/ag9g/workspace/swapped").is_err()
        );
        profile
            .verify_object(
                &File::open("/tmp/ag9g/workspace/profile").unwrap(),
                AttemptDeadline::new(),
            )
            .unwrap();
        assert_eq!(
            profile.verify_object(
                &File::open("/tmp/ag9g/workspace/old").unwrap(),
                AttemptDeadline::new()
            ),
            Err(E::ProcessIdentity)
        );
        assert!(FreshProfile::create(&workspace, AttemptDeadline::expired()).is_err());
        // Private mounts are destroyed with this test process's namespace.
    }
}
