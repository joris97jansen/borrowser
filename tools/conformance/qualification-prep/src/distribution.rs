use crate::{
    Error, Result,
    canonical::{self, Writer},
    error::require,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
pub const FORMAT: &str = "borrowser-chromium-distribution-manifest-v1";
pub const MANIFEST_BYTES: usize = 1_048_576;
pub const FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryRecord {
    pub path: String,
    pub mode: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EntryRecord {
    #[serde(rename = "regular")]
    Regular {
        path: String,
        mode: u32,
        executable: bool,
        byte_length: u64,
        sha256: String,
        file_capabilities: String,
    },
    #[serde(rename = "symlink")]
    Symlink {
        path: String,
        mode: u32,
        executable: bool,
        target: String,
        target_sha256: String,
        resolved_path: String,
    },
}
impl EntryRecord {
    pub fn path(&self) -> &str {
        match self {
            Self::Regular { path, .. } | Self::Symlink { path, .. } => path,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDistributionManifest {
    pub format: String,
    pub root_mode: u32,
    #[serde(deserialize_with = "directories")]
    pub directories: Vec<DirectoryRecord>,
    #[serde(deserialize_with = "entries")]
    pub entries: Vec<EntryRecord>,
}
pub fn mode(m: u32) -> Result<()> {
    require(m <= 0o777 && m & 0o022 == 0, "mode")
}
pub fn resolve(link: &str, target: &str) -> Result<String> {
    require(
        !target.is_empty()
            && target.len() <= 1024
            && target.is_ascii()
            && !target.bytes().any(|b| b.is_ascii_control())
            && !target.starts_with('/')
            && !target.contains('\\'),
        "link target",
    )?;
    let mut parts: Vec<_> = link.split('/').collect();
    parts.pop();
    for c in target.split('/') {
        match c {
            "" => return Err(Error::Invalid("empty link component")),
            "." => (),
            ".." => {
                parts.pop().ok_or(Error::Invalid("link escape"))?;
            }
            c => parts.push(c),
        }
    }
    let p = parts.join("/");
    canonical::relative(&p)?;
    Ok(p)
}
impl CandidateDistributionManifest {
    pub fn validate(&self) -> Result<()> {
        require(self.format == FORMAT, "manifest format")?;
        mode(self.root_mode)?;
        require(
            self.directories.len() <= 1024
                && !self.entries.is_empty()
                && self.entries.len() <= 4096,
            "manifest population",
        )?;
        let mut all = BTreeSet::new();
        let mut dirs = BTreeSet::new();
        let mut last = "";
        for d in &self.directories {
            canonical::relative(&d.path)?;
            mode(d.mode)?;
            require(
                d.path.as_str() > last && all.insert(d.path.as_str()),
                "directory order/duplicate",
            )?;
            dirs.insert(d.path.as_str());
            last = &d.path;
        }
        let entries: BTreeMap<_, _> = self.entries.iter().map(|e| (e.path(), e)).collect();
        let mut total = 0u64;
        last = "";
        for e in &self.entries {
            canonical::relative(e.path())?;
            require(
                e.path() > last && all.insert(e.path()),
                "entry order/duplicate",
            )?;
            last = e.path();
            match e {
                EntryRecord::Regular {
                    mode: m,
                    executable,
                    byte_length,
                    sha256,
                    file_capabilities,
                    ..
                } => {
                    mode(*m)?;
                    canonical::digest(sha256)?;
                    require(
                        *executable == (*m & 0o111 != 0) && file_capabilities == "absent",
                        "executable/capabilities",
                    )?;
                    total = total
                        .checked_add(*byte_length)
                        .ok_or(Error::Invalid("content overflow"))?;
                    require(
                        *byte_length <= FILE_BYTES && total <= TOTAL_BYTES,
                        "content limit",
                    )?;
                }
                EntryRecord::Symlink {
                    path,
                    mode,
                    executable,
                    target,
                    target_sha256,
                    resolved_path,
                } => {
                    require(*mode == 511 && !executable, "symlink mode")?;
                    require(
                        canonical::hash(target.as_bytes()) == *target_sha256,
                        "symlink digest",
                    )?;
                    canonical::relative(resolved_path)?;
                    require(
                        resolve_chain(path, target, &entries)? == *resolved_path,
                        "resolved link",
                    )?;
                }
            }
        }
        for p in all {
            if let Some((parent, _)) = p.rsplit_once('/') {
                require(dirs.contains(parent), "missing parent")?;
            }
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut w = Writer::new(MANIFEST_BYTES);
        w.field("format", &self.format)?;
        w.field("root_mode", self.root_mode)?;
        if self.directories.is_empty() {
            w.raw("directories = []\n")?;
        }
        for d in &self.directories {
            w.raw("[[directories]]\n")?;
            w.field("path", &d.path)?;
            w.field("mode", d.mode)?;
        }
        for e in &self.entries {
            w.raw("[[entries]]\n")?;
            w.field("path", e.path())?;
            match e {
                EntryRecord::Regular {
                    mode,
                    executable,
                    byte_length,
                    sha256,
                    file_capabilities,
                    ..
                } => {
                    w.field("kind", "regular")?;
                    w.field("mode", mode)?;
                    w.field("executable", executable)?;
                    w.field("byte_length", byte_length)?;
                    w.field("sha256", sha256)?;
                    w.field("file_capabilities", file_capabilities)?;
                }
                EntryRecord::Symlink {
                    mode,
                    executable,
                    target,
                    target_sha256,
                    resolved_path,
                    ..
                } => {
                    w.field("kind", "symlink")?;
                    w.field("mode", mode)?;
                    w.field("executable", executable)?;
                    w.field("target", target)?;
                    w.field("target_sha256", target_sha256)?;
                    w.field("resolved_path", resolved_path)?;
                }
            }
        }
        Ok(w.finish())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = canonical::read(path, MANIFEST_BYTES)?;
        // Raw bytes bound parsing; retained populations receive exact validation.
        let s = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("manifest UTF-8"))?;
        let m: Self = toml::from_str(s).map_err(|_| Error::Invalid("manifest schema"))?;
        require(m.canonical_bytes()? == bytes, "noncanonical manifest")?;
        Ok(m)
    }
    pub fn executable_digest(&self, path: &str) -> Result<&str> {
        canonical::relative(path)?;
        match self.entries.iter().find(|e| e.path() == path) {
            Some(EntryRecord::Regular {
                executable: true,
                sha256,
                ..
            }) => Ok(sha256),
            _ => Err(Error::Invalid("main executable")),
        }
    }
}
pub fn resolve_chain(
    link: &str,
    target: &str,
    entries: &BTreeMap<&str, &EntryRecord>,
) -> Result<String> {
    let mut next = resolve(link, target)?;
    let mut seen = BTreeSet::from([link.to_owned()]);
    for _ in 0..16 {
        require(seen.insert(next.clone()), "link cycle")?;
        match entries.get(next.as_str()) {
            Some(EntryRecord::Regular { path, .. }) => return Ok(path.clone()),
            Some(EntryRecord::Symlink { path, target, .. }) => next = resolve(path, target)?,
            _ => return Err(Error::Invalid("link target missing/directory")),
        }
    }
    Err(Error::Invalid("link chain limit"))
}
#[cfg(target_os = "linux")]
pub fn inventory(root: &Path) -> Result<CandidateDistributionManifest> {
    crate::linux_fs::inventory(root)
}
#[cfg(not(target_os = "linux"))]
pub fn inventory(_: &Path) -> Result<CandidateDistributionManifest> {
    Err(Error::Unsupported)
}

fn directories<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<DirectoryRecord>, D::Error> {
    canonical::sequence::<D, DirectoryRecord, 1024>(d)
}
fn entries<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Vec<EntryRecord>, D::Error> {
    canonical::sequence::<D, EntryRecord, 4096>(d)
}
