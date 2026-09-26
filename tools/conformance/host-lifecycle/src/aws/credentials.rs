//! External secret input. Never serialized, journaled, or exposed by the CLI.
use crate::{Error, Result, require};
use serde::Deserialize;
use std::{
    ffi::CString,
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: String,
    schema_version: u64,
    access_key_id: Zeroizing<String>,
    secret_access_key: Zeroizing<String>,
    session_token: Zeroizing<String>,
    expiration_unix_seconds: u64,
}
pub(super) struct SessionSecret {
    credentials: aws_credential_types::Credentials,
    expires: SystemTime,
}
impl std::fmt::Debug for SessionSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionSecret([REDACTED])")
    }
}
impl SessionSecret {
    pub(super) fn load(path: &Path, now: SystemTime) -> Result<Self> {
        // Walk every component by descriptor; no symlinked parents or final file.
        require(path.is_absolute(), "session file absolute path required")?;
        let mut dir = File::open("/").map_err(|_| Error("session file unavailable"))?;
        let parts: Vec<_> = path.components().collect();
        require(parts.len() <= 64, "session path bound")?;
        for (i, part) in parts.iter().enumerate().skip(1) {
            let Component::Normal(name) = part else {
                return Err(Error("session path component"));
            };
            let name = CString::new(name.as_encoded_bytes())
                .map_err(|_| Error("session path component"))?;
            let last = i + 1 == parts.len();
            let flags = libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | if last {
                    libc::O_NONBLOCK
                } else {
                    libc::O_DIRECTORY
                };
            let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
            require(fd >= 0, "session file unavailable")?;
            dir = unsafe { File::from_raw_fd(fd) };
        }
        let before = dir.metadata().map_err(|_| Error("session file metadata"))?;
        require(
            before.is_file()
                && before.nlink() == 1
                && before.uid() == unsafe { libc::geteuid() }
                && before.mode() & 0o7777 == 0o600
                && before.len() > 0
                && before.len() <= 16_384,
            "session file protections",
        )?;
        let mut bytes = Zeroizing::new(Vec::new());
        (&mut dir)
            .take(16_385)
            .read_to_end(&mut bytes)
            .map_err(|_| Error("session file read"))?;
        let after = dir.metadata().map_err(|_| Error("session file metadata"))?;
        require(
            bytes.len() as u64 == before.len()
                && before.len() == after.len()
                && before.ino() == after.ino()
                && before.dev() == after.dev()
                && before.ctime() == after.ctime()
                && before.ctime_nsec() == after.ctime_nsec()
                && before.mtime() == after.mtime()
                && before.mtime_nsec() == after.mtime_nsec()
                && before.mode() == after.mode()
                && before.uid() == after.uid()
                && after.nlink() == 1,
            "session file changed",
        )?;
        Self::decode(&bytes, now)
    }
    fn decode(bytes: &[u8], now: SystemTime) -> Result<Self> {
        require(bytes.len() <= 16_384, "session document bound")?;
        let d: Document =
            serde_json::from_slice(bytes).map_err(|_| Error("invalid session document"))?;
        require(
            d.format == "borrowser-aws-operator-session" && d.schema_version == 1,
            "session document generation",
        )?;
        for (value, max) in [
            (&*d.access_key_id, 128),
            (&*d.secret_access_key, 256),
            (&*d.session_token, 8192),
        ] {
            require(
                !value.is_empty()
                    && value.len() <= max
                    && value.bytes().all(|b| b.is_ascii_graphic()),
                "session component bound",
            )?;
        }
        require(
            d.expiration_unix_seconds <= 253_402_300_799,
            "session expiry bound",
        )?;
        let expires = UNIX_EPOCH
            .checked_add(Duration::from_secs(d.expiration_unix_seconds))
            .ok_or(Error("session expiry bound"))?;
        require(expires > now, "session expired")?;
        let credentials = aws_credential_types::Credentials::new(
            d.access_key_id.to_string(),
            d.secret_access_key.to_string(),
            Some(d.session_token.to_string()),
            Some(expires),
            "explicit-session-file",
        );
        Ok(Self {
            credentials,
            expires,
        })
    }
    pub(super) fn sdk(&self, now: SystemTime) -> Result<aws_credential_types::Credentials> {
        require(now < self.expires, "session expired")?;
        Ok(self.credentials.clone())
    }
    pub(super) fn expires(&self) -> SystemTime {
        self.expires
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    fn bytes() -> Vec<u8> {
        br#"{"format":"borrowser-aws-operator-session","schema_version":1,"access_key_id":"SYNTHETICACCESS","secret_access_key":"synthetic-secret","session_token":"synthetic-token","expiration_unix_seconds":2000000000}"#.to_vec()
    }
    #[test]
    fn strict_session_schema_expiry_and_redaction() {
        let now = UNIX_EPOCH + Duration::from_secs(1_900_000_000);
        let secret = SessionSecret::decode(&bytes(), now).unwrap();
        assert_eq!(
            secret.sdk(now).unwrap().expiry(),
            Some(UNIX_EPOCH + Duration::from_secs(2_000_000_000))
        );
        assert_eq!(format!("{secret:?}"), "SessionSecret([REDACTED])");
        for field in ["access_key_id", "secret_access_key", "session_token"] {
            let mut d: serde_json::Value = serde_json::from_slice(&bytes()).unwrap();
            d.as_object_mut().unwrap().remove(field);
            assert!(SessionSecret::decode(&serde_json::to_vec(&d).unwrap(), now).is_err());
        }
        for field in ["access_key_id", "secret_access_key", "session_token"] {
            for value in ["".to_owned(), " ".to_owned(), "x".repeat(8193)] {
                let mut d: serde_json::Value = serde_json::from_slice(&bytes()).unwrap();
                d[field] = value.into();
                assert!(SessionSecret::decode(&serde_json::to_vec(&d).unwrap(), now).is_err());
            }
        }
        for suffix in [",\"unknown\":true}", ",\"session_token\":\"duplicate\"}"] {
            let b = bytes();
            let mut bad = b[..b.len() - 1].to_vec();
            bad.extend(suffix.as_bytes());
            assert!(SessionSecret::decode(&bad, now).is_err());
        }
        assert!(SessionSecret::decode(&bytes(), secret.expires).is_err());
        assert!(secret.sdk(secret.expires).is_err());
        assert!(
            !format!("{:?}", SessionSecret::decode(b"synthetic-secret", now))
                .contains("synthetic-secret")
        );
    }
    #[test]
    fn private_regular_owned_single_link_file_only() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().canonicalize().unwrap();
        let p = dir.join("session");
        std::fs::write(&p, bytes()).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        let now = UNIX_EPOCH + Duration::from_secs(1_900_000_000);
        assert!(SessionSecret::load(&p, now).is_ok());
        let link = dir.join("symlink");
        symlink(&p, &link).unwrap();
        assert!(SessionSecret::load(&link, now).is_err());
        std::fs::hard_link(&p, dir.join("hardlink")).unwrap();
        assert!(SessionSecret::load(&p, now).is_err());
        std::fs::remove_file(dir.join("hardlink")).unwrap();
        let parent_link = dir.join("linked-directory");
        symlink(&dir, &parent_link).unwrap();
        assert!(SessionSecret::load(&parent_link.join("session"), now).is_err());
        // The existing Linux image runs as root, so it can genuinely exercise
        // wrong ownership. Unprivileged hosts cannot manufacture another owner.
        if unsafe { libc::geteuid() } == 0 {
            let name = CString::new(p.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::chown(name.as_ptr(), 12345, 12345) }, 0);
            assert!(SessionSecret::load(&p, now).is_err());
            assert_eq!(unsafe { libc::chown(name.as_ptr(), 0, 0) }, 0);
        }
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(SessionSecret::load(&p, now).is_err());
    }
}
