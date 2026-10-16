//! One operation record and one non-replaced advisory lock; no recovery journal.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum Creation {
    Prepared,
    Submitted,
    Bound,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum Capture {
    Pending,
    Validated,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum Cleanup {
    Pending,
    Confirmed,
    NotRequired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub version: u32,
    pub run_id: String,
    pub account: String,
    pub region: String,
    pub stack_name: String,
    pub stack_id: Option<String>,
    pub template_sha256: String,
    pub creation: Creation,
    pub capture: Capture,
    pub cleanup: Cleanup,
    pub instance_id: Option<String>,
    pub root_volume_id: Option<String>,
    pub primary_failure: Option<String>,
    pub cleanup_failure: Option<String>,
}
impl Record {
    pub fn new(b: &super::template::Binding, digest: String) -> Self {
        Self {
            version: 1,
            run_id: b.run_id.clone(),
            account: b.account.clone(),
            region: b.region.clone(),
            stack_name: b.stack_name.clone(),
            stack_id: None,
            template_sha256: digest,
            creation: Creation::Prepared,
            capture: Capture::Pending,
            cleanup: Cleanup::Pending,
            instance_id: None,
            root_volume_id: None,
            primary_failure: None,
            cleanup_failure: None,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.region != "eu-west-1"
            || self.account.len() != 12
            || !self.account.bytes().all(|c| c.is_ascii_digit())
            || self.run_id.len() != 32
            || !self
                .run_id
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || self.stack_name != format!("borrowser-ag4-{}", self.run_id)
            || self.template_sha256.len() != 64
            || !self.template_sha256.bytes().all(|c| c.is_ascii_hexdigit())
            || (self.creation == Creation::Bound && self.stack_id.is_none())
        {
            return Err("invalid operation record".into());
        }
        if let Some(id) = &self.stack_id {
            self.check_stack_id(id)?;
        }
        if (self.cleanup == Cleanup::Confirmed
            && (self.creation != Creation::Bound || self.stack_id.is_none()))
            || (self.cleanup == Cleanup::NotRequired
                && (self.stack_id.is_some() || self.creation != Creation::Prepared))
            || (self.root_volume_id.is_some() && self.instance_id.is_none())
            || (self.instance_id.is_some() && self.stack_id.is_none())
            || (self.capture == Capture::Validated && self.creation != Creation::Bound)
        {
            return Err("conflicting operation lifecycle fields".into());
        }
        for (id, prefix) in [(&self.instance_id, "i-"), (&self.root_volume_id, "vol-")] {
            if let Some(id) = id {
                let suffix = id.strip_prefix(prefix).ok_or("invalid resource identity")?;
                if suffix.len() != 17
                    || !suffix
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err("invalid resource identity".into());
                }
            }
        }
        Ok(())
    }
    pub fn check_stack_id(&self, id: &str) -> Result<(), String> {
        let prefix = format!(
            "arn:aws:cloudformation:{}:{}:stack/{}/",
            self.region, self.account, self.stack_name
        );
        if !id.starts_with(&prefix)
            || id[prefix.len()..].is_empty()
            || self.stack_id.as_ref().is_some_and(|known| known != id)
        {
            return Err("conflicting stack ARN".into());
        }
        Ok(())
    }
}

pub(super) struct Store {
    dir: PathBuf,
    _lock: File,
}
impl Store {
    pub fn open(dir: &Path, create: bool) -> Result<Self, String> {
        if create {
            fs::create_dir(dir).map_err(|e| e.to_string())?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            File::open(dir.parent().ok_or("operation parent required")?)
                .and_then(|parent| parent.sync_all())
                .map_err(|e| e.to_string())?;
        }
        let meta = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.mode() & 0o777 != 0o700
            || meta.uid() != unsafe { libc::geteuid() }
        {
            return Err("operation directory must be owned, non-symlink, mode 0700".into());
        }
        let dir = fs::canonicalize(dir).map_err(|e| e.to_string())?;
        if dir.ancestors().any(|p| p.join(".git").exists()) {
            return Err("operation state must be outside the repository".into());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("lock"))
            .map_err(|e| e.to_string())?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("operation is locked by another controller".into());
        }
        Ok(Self { dir, _lock: lock })
    }
    pub fn path(&self) -> &Path {
        &self.dir
    }
    pub fn read(&self) -> Result<Record, String> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.dir.join("operation.json"))
            .map_err(|e| e.to_string())?;
        let m = file.metadata().map_err(|e| e.to_string())?;
        if !m.is_file() || m.mode() & 0o777 != 0o600 || m.uid() != unsafe { libc::geteuid() } {
            return Err("unsafe operation record permissions".into());
        }
        let mut bytes = vec![];
        file.take(16385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16384 {
            return Err("operation record too large".into());
        }
        let value = super::template::json(&bytes)?;
        let r: Record = serde_json::from_value(value).map_err(|e| e.to_string())?;
        r.validate()?;
        Ok(r)
    }
    pub fn save(&self, r: &Record) -> Result<(), String> {
        r.validate()?;
        let bytes = serde_json::to_vec_pretty(r).map_err(|e| e.to_string())?;
        if bytes.len() > 16384 {
            return Err("operation record too large".into());
        }
        let mut tmp = tempfile::NamedTempFile::new_in(&self.dir).map_err(|e| e.to_string())?;
        tmp.write_all(&bytes)
            .and_then(|()| tmp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        if unsafe { libc::fcntl(tmp.as_raw_fd(), libc::F_FULLFSYNC) } < 0 {
            return Err("record full synchronization failed".into());
        }
        tmp.persist(self.dir.join("operation.json"))
            .map_err(|e| e.to_string())?;
        File::open(&self.dir)
            .and_then(|d| d.sync_all())
            .map_err(|e| e.to_string())
    }
}
