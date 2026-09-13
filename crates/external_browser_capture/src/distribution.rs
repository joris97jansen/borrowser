use crate::{CaptureError as E, Result, limits::MANIFEST_BYTES, wire};
use external_test_provenance::sha256;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistributionManifest {
    format: String,
    root_mode: u32,
    #[serde(deserialize_with = "wire::directories")]
    directories: Vec<Directory>,
    #[serde(deserialize_with = "wire::files")]
    entries: Vec<Entry>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Directory {
    path: String,
    mode: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Entry {
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
impl Entry {
    fn path(&self) -> &str {
        match self {
            Self::Regular { path, .. } | Self::Symlink { path, .. } => path,
        }
    }
}
fn mode(value: u32) -> Result<()> {
    if value > 0o777 || value & 0o022 != 0 {
        Err(E::Mode)
    } else {
        Ok(())
    }
}

impl DistributionManifest {
    pub fn load(root: &Path, path: &str, digest: &str) -> Result<Self> {
        let bytes = wire::read(root, path, MANIFEST_BYTES)?;
        if sha256(&bytes).to_string() != digest {
            return Err(E::Digest);
        }
        Self::parse(&bytes)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let m: Self = wire::parse(bytes, MANIFEST_BYTES)?;
        m.validate()?;
        if m.canonical_bytes()? != bytes {
            return Err(E::NonCanonical);
        }
        Ok(m)
    }
    fn validate(&self) -> Result<()> {
        if self.format != "borrowser-chromium-distribution-manifest-v1" {
            return Err(E::Field);
        }
        if self.directories.len() > 1024 || self.entries.is_empty() || self.entries.len() > 4096 {
            return Err(E::Limit);
        }
        mode(self.root_mode)?;
        let mut paths = BTreeSet::new();
        let mut previous = "";
        for d in &self.directories {
            wire::path(&d.path)?;
            mode(d.mode)?;
            if d.path.as_str() <= previous || !paths.insert(d.path.as_str()) {
                return Err(E::Distribution);
            }
            previous = &d.path;
        }
        previous = "";
        let mut total = 0u64;
        let entries: BTreeMap<_, _> = self.entries.iter().map(|e| (e.path(), e)).collect();
        for e in &self.entries {
            wire::path(e.path())?;
            if e.path() <= previous || !paths.insert(e.path()) {
                return Err(E::Distribution);
            }
            previous = e.path();
            match e {
                Entry::Regular {
                    mode: m,
                    executable,
                    byte_length,
                    sha256: d,
                    file_capabilities,
                    ..
                } => {
                    mode(*m)?;
                    wire::digest(d)?;
                    if *executable != (*m & 0o111 != 0) || file_capabilities != "absent" {
                        return Err(E::Capability);
                    }
                    total = total.checked_add(*byte_length).ok_or(E::Limit)?;
                    if *byte_length > 2 * 1024 * 1024 * 1024 || total > 8 * 1024 * 1024 * 1024 {
                        return Err(E::Limit);
                    }
                }
                Entry::Symlink {
                    path,
                    mode,
                    executable,
                    target,
                    target_sha256,
                    resolved_path,
                } => {
                    if *mode != 0o777
                        || *executable
                        || target.is_empty()
                        || target.len() > 1024
                        || !target.is_ascii()
                        || target.bytes().any(|b| b.is_ascii_control())
                    {
                        return Err(E::Symlink);
                    }
                    wire::digest(target_sha256)?;
                    wire::path(resolved_path)?;
                    if sha256(target.as_bytes()).to_string() != *target_sha256 {
                        return Err(E::Digest);
                    }
                    let mut next = resolve(path, target)?;
                    let mut seen = BTreeSet::new();
                    seen.insert(path.clone());
                    let mut resolved = false;
                    for _ in 0..16 {
                        if !seen.insert(next.clone()) {
                            break;
                        }
                        match entries.get(next.as_str()) {
                            Some(Entry::Regular { path, .. }) => {
                                resolved = path == resolved_path;
                                break;
                            }
                            Some(Entry::Symlink { path, target, .. }) => {
                                next = resolve(path, target)?;
                            }
                            _ => break,
                        }
                    }
                    if !resolved {
                        return Err(E::Symlink);
                    }
                }
            }
        }
        let dirs: BTreeSet<_> = self.directories.iter().map(|d| d.path.as_str()).collect();
        for p in &paths {
            if let Some((parent, _)) = p.rsplit_once('/')
                && !dirs.contains(parent)
            {
                return Err(E::Distribution);
            }
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut w = wire::Writer::new(MANIFEST_BYTES);
        w.field("format", &self.format)?;
        w.field("root_mode", &self.root_mode)?;
        if self.directories.is_empty() {
            w.raw("directories = []\n")?;
        }
        for d in &self.directories {
            w.raw("[[directories]]\n")?;
            w.field("path", &d.path)?;
            w.field("mode", &d.mode)?;
        }
        for e in &self.entries {
            w.raw("[[entries]]\n")?;
            w.field("path", &e.path())?;
            match e {
                Entry::Regular {
                    mode,
                    executable,
                    byte_length,
                    sha256,
                    file_capabilities,
                    ..
                } => {
                    w.field("kind", &"regular")?;
                    w.field("mode", mode)?;
                    w.field("executable", executable)?;
                    w.field("byte_length", byte_length)?;
                    w.field("sha256", sha256)?;
                    w.field("file_capabilities", file_capabilities)?;
                }
                Entry::Symlink {
                    mode,
                    executable,
                    target,
                    target_sha256,
                    resolved_path,
                    ..
                } => {
                    w.field("kind", &"symlink")?;
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
}
fn resolve(link: &str, target: &str) -> Result<String> {
    if target.starts_with('/') || target.contains('\\') {
        return Err(E::Symlink);
    }
    let mut parts: Vec<_> = link.split('/').collect();
    parts.pop();
    for p in target.split('/') {
        match p {
            "." => (),
            ".." => {
                parts.pop().ok_or(E::Symlink)?;
            }
            "" => return Err(E::Symlink),
            p => parts.push(p),
        }
    }
    let result = parts.join("/");
    wire::path(&result)?;
    Ok(result)
}

#[cfg_attr(
    not(feature = "chromium-cdp"),
    allow(
        dead_code,
        reason = "Retained objects are consumed by the optional capture adapter."
    )
)]
pub struct VerifiedDistribution {
    directory: tempfile::TempDir,
    executable: PathBuf,
    #[cfg(target_os = "linux")]
    files: Vec<(Entry, std::fs::File)>,
    #[cfg(target_os = "linux")]
    manifest: DistributionManifest,
    #[cfg(target_os = "linux")]
    directories: Vec<String>,
}
impl VerifiedDistribution {
    #[cfg(all(target_os = "linux", feature = "chromium-cdp"))]
    pub(crate) fn freeze(
        &self,
        deadline: crate::deadline::AttemptDeadline,
    ) -> Result<linux::FrozenDistribution> {
        linux::freeze(self, deadline)
    }
    #[cfg(all(target_os = "linux", any(test, feature = "chromium-cdp")))]
    pub(crate) fn close(self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            self.directory.path().join("distribution"),
            std::fs::Permissions::from_mode(0o700),
        )
        .map_err(|_| E::Cleanup)?;
        for path in &self.directories {
            std::fs::set_permissions(
                self.directory.path().join("distribution").join(path),
                std::fs::Permissions::from_mode(0o700),
            )
            .map_err(|_| E::Cleanup)?;
        }
        self.directory.close().map_err(|_| E::Cleanup)
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    pub fn root(&self) -> &Path {
        self.directory.path()
    }
    pub fn create(
        manifest: &DistributionManifest,
        supplied: &Path,
        executable: &str,
        digest: &str,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            linux::snapshot(manifest, supplied, executable, digest)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (manifest, supplied, executable, digest);
            Err(E::UnsupportedHost)
        }
    }
}

#[cfg(target_os = "linux")]
#[path = "distribution_linux.rs"]
mod linux;

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn manifest() -> DistributionManifest {
        DistributionManifest {
            format: "borrowser-chromium-distribution-manifest-v1".into(),
            root_mode: 0o755,
            directories: vec![],
            entries: vec![Entry::Regular {
                path: "chrome".into(),
                mode: 0o755,
                executable: true,
                byte_length: 1,
                sha256: sha256(b"x").to_string(),
                file_capabilities: "absent".into(),
            }],
        }
    }
    #[test]
    fn canonical_schema_modes_and_limits() {
        let mut m = manifest();
        let b = m.canonical_bytes().unwrap();
        DistributionManifest::parse(&b).unwrap();
        assert!(
            DistributionManifest::parse(&[b.as_slice(), b"unknown = true\n"].concat()).is_err()
        );
        m.root_mode = 0o777;
        assert_eq!(m.validate(), Err(E::Mode));
        m = manifest();
        if let Entry::Regular {
            file_capabilities, ..
        } = &mut m.entries[0]
        {
            *file_capabilities = "present".into();
        }
        assert!(m.validate().is_err());
        m = manifest();
        m.entries.push(Entry::Regular {
            path: "chrome".into(),
            mode: 0o4755,
            executable: true,
            byte_length: 1,
            sha256: sha256(b"x").to_string(),
            file_capabilities: "absent".into(),
        });
        assert!(m.validate().is_err());
    }
    #[test]
    fn non_ascii_symlink_target_is_outside_v1() {
        let mut m = manifest();
        m.entries.insert(
            0,
            Entry::Symlink {
                path: "a".into(),
                mode: 0o777,
                executable: false,
                target: "é".into(),
                target_sha256: sha256("é".as_bytes()).to_string(),
                resolved_path: "chrome".into(),
            },
        );
        assert_eq!(m.validate(), Err(E::Symlink));
    }
    #[test]
    fn symlink_escape_cycle_and_target_digest() {
        assert_eq!(resolve("a", "../outside"), Err(E::Symlink));
        let mut m = manifest();
        m.entries.insert(
            0,
            Entry::Symlink {
                path: "a".into(),
                mode: 0o777,
                executable: false,
                target: "a".into(),
                target_sha256: sha256(b"a").to_string(),
                resolved_path: "chrome".into(),
            },
        );
        assert_eq!(m.validate(), Err(E::Symlink));
        if let Entry::Symlink {
            target,
            target_sha256,
            ..
        } = &mut m.entries[0]
        {
            *target = "chrome".into();
            *target_sha256 = sha256(b"chrome").to_string();
        }
        m.validate().unwrap();
    }
}
