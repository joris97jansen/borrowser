//! Linux controller boundary; never runs on the acquired qualification candidate.
use crate::identity::*;
use crate::{
    Error, Result, canonical,
    journal::{Journal, ROOT},
    require,
    scheduling::TimeSample,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Deployment {
    pub(crate) authority_id: AuthorityId,
    pub(crate) account_id: AccountScopeId,
    pub(crate) approved_account_currency: String,
    pub(crate) product_id: ProductId,
    pub(crate) approved_class: String,
    pub(crate) catalogue_evidence_sha256: EvidenceDigest,
    pub(crate) controller_machine_id: String,
    pub(crate) filesystem_uuid: String,
    pub(crate) approved_monthly_gross_units: u64,
    pub(crate) approved_setup_gross_units: u64,
}
impl Deployment {
    pub(crate) fn validate(&self) -> Result<()> {
        for s in [&self.controller_machine_id, &self.filesystem_uuid] {
            canonical::text(s, 128)?;
        }
        require(self.approved_class == "AX42-1", "unapproved hardware class")?;
        require(
            self.approved_account_currency == "EUR",
            "unsupported account currency approval",
        )?;
        canonical::digest(&self.catalogue_evidence_sha256)
    }
}
#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}
pub(crate) fn confined(dir: &File, path: &str, flags: i32, no_mount: bool) -> Result<File> {
    let p = std::ffi::CString::new(path).map_err(|_| Error("path encoding"))?;
    // RESOLVE_BENEATH | NO_SYMLINKS | NO_MAGICLINKS, optionally NO_XDEV.
    let how = OpenHow {
        flags: (flags | libc::O_CLOEXEC | libc::O_NONBLOCK) as u64,
        mode: if flags & libc::O_CREAT != 0 { 0o600 } else { 0 },
        resolve: 0x08 | 0x04 | 0x02 | if no_mount { 0x01 } else { 0 },
    };
    let fd = unsafe {
        libc::syscall(
            libc::SYS_openat2,
            dir.as_raw_fd(),
            p.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    require(fd >= 0, "openat2 confinement unavailable or rejected")?;
    Ok(unsafe { File::from_raw_fd(fd as i32) })
}
fn read_bound(mut f: File, max: u64) -> Result<Vec<u8>> {
    let m = f.metadata().map_err(|_| Error("file metadata"))?;
    require(m.is_file() && m.len() <= max, "file bound/type")?;
    let mut bytes = Vec::new();
    (&mut f)
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error("bounded file read"))?;
    require(bytes.len() as u64 <= max, "file grew")?;
    Ok(bytes)
}
pub(crate) fn deployment() -> Result<Deployment> {
    let root = File::open("/").map_err(|_| Error("system root"))?;
    let file = confined(
        &root,
        "etc/borrowser-host-lifecycle/deployment.json",
        libc::O_RDONLY,
        false,
    )?;
    let m = file.metadata().map_err(|_| Error("deployment metadata"))?;
    require(
        m.uid() == 0 && m.mode() & 0o022 == 0 && m.nlink() == 1,
        "deployment protection",
    )?;
    let d: Deployment = canonical::decode(&read_bound(file, 16_384)?)?;
    d.validate()?;
    Ok(d)
}
pub(crate) fn product_approval(d: &Deployment) -> Result<crate::approval::ProductApproval> {
    let root = File::open("/").map_err(|_| Error("system root"))?;
    let f = confined(
        &root,
        "etc/borrowser-host-lifecycle/product-approval.json",
        libc::O_RDONLY,
        false,
    )?;
    let m = f.metadata().map_err(|_| Error("approval metadata"))?;
    require(
        m.uid() == 0 && m.mode() & 0o022 == 0 && m.nlink() == 1,
        "approval protection",
    )?;
    let approval = crate::approval::ProductApproval::verify_bytes(
        &read_bound(f, 16_384)?,
        &d.catalogue_evidence_sha256,
    )?;
    require(
        approval.account_scope == d.account_id
            && approval.catalogue.product_id == d.product_id
            && approval.hardware_class == d.approved_class
            && approval.attested_account_currency == d.approved_account_currency
            && approval.monthly_gross_ceiling == d.approved_monthly_gross_units
            && approval.setup_gross_ceiling == d.approved_setup_gross_units,
        "deployment/product approval mismatch",
    )?;
    Ok(approval)
}
fn authority_root(d: &Deployment) -> Result<File> {
    require(
        unsafe { libc::geteuid() } != 0,
        "controller must use dedicated non-root identity",
    )?;
    let system = File::open("/").map_err(|_| Error("system root"))?;
    let root = confined(
        &system,
        &ROOT[1..],
        libc::O_RDONLY | libc::O_DIRECTORY,
        false,
    )?;
    let m = root.metadata().map_err(|_| Error("authority metadata"))?;
    require(
        m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
        "authority root protection",
    )?;
    let mounts =
        std::fs::read_to_string("/proc/self/mountinfo").map_err(|_| Error("mount inventory"))?;
    require(mounts.len() <= 1_048_576, "mount inventory bound")?;
    let mut stat: libc::statx = unsafe { std::mem::zeroed() };
    require(
        unsafe {
            libc::statx(
                root.as_raw_fd(),
                c"".as_ptr(),
                libc::AT_EMPTY_PATH,
                libc::STATX_MNT_ID,
                &mut stat,
            )
        } == 0
            && stat.stx_mask & libc::STATX_MNT_ID != 0,
        "mount identity unavailable",
    )?;
    validate_mount_inventory(&mounts, m.dev(), stat.stx_mnt_id)?;
    let machine =
        std::fs::read_to_string("/etc/machine-id").map_err(|_| Error("controller identity"))?;
    require(
        machine.trim() == d.controller_machine_id,
        "wrong controller",
    )?;
    require(
        d.filesystem_uuid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-'),
        "filesystem uuid syntax",
    )?;
    // This device symlink is used only for independently checking rdev; never for authority I/O.
    let device = std::fs::metadata(format!("/dev/disk/by-uuid/{}", d.filesystem_uuid))
        .map_err(|_| Error("filesystem device identity"))?;
    require(device.rdev() == m.dev(), "wrong filesystem")?;
    Ok(root)
}
fn validate_mount_inventory(mounts: &str, device: u64, mount_id: u64) -> Result<()> {
    let mut found = 0;
    for line in mounts.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(4) == Some(&ROOT) {
            let dash = fields
                .iter()
                .position(|s| *s == "-")
                .ok_or(Error("mount record"))?;
            require(
                fields.get(3) == Some(&"/") && fields.get(dash + 1) == Some(&"ext4"),
                "dedicated ext4 mount required",
            )?;
            let dev = format!("{}:{}", libc::major(device), libc::minor(device));
            require(
                fields.get(2) == Some(&dev.as_str()),
                "mount object mismatch",
            )?;
            require(
                fields.first().and_then(|s| s.parse::<u64>().ok()) == Some(mount_id),
                "mount identity mismatch",
            )?;
            found += 1;
        }
        require(
            !fields
                .get(4)
                .is_some_and(|p| p.starts_with(&format!("{ROOT}/"))),
            "nested authority mount",
        )?;
    }
    require(found == 1, "missing/ambiguous authority mount")?;
    Ok(())
}
pub(crate) fn open_authority(d: &Deployment, bootstrap: bool) -> Result<Journal> {
    let root = authority_root(d)?;
    if bootstrap {
        Journal::bootstrap_verified(root)
    } else {
        Journal::open_verified(root)
    }
}
pub(crate) fn now() -> Result<TimeSample> {
    fn ns(clock: libc::clockid_t) -> Result<u64> {
        let mut t = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        require(
            unsafe { libc::clock_gettime(clock, &mut t) } == 0
                && t.tv_sec >= 0
                && (0..1_000_000_000).contains(&t.tv_nsec),
            "clock read",
        )?;
        (t.tv_sec as u64)
            .checked_mul(1_000_000_000)
            .and_then(|s| s.checked_add(t.tv_nsec as u64))
            .ok_or(Error("clock overflow"))
    }
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|_| Error("boot identity"))?
        .trim()
        .to_owned();
    let time_namespace = std::fs::read_link("/proc/self/ns/time")
        .map_err(|_| Error("time namespace"))?
        .to_str()
        .ok_or(Error("time namespace encoding"))?
        .to_owned();
    let initial =
        std::fs::read_link("/proc/1/ns/time").map_err(|_| Error("initial time namespace"))?;
    require(
        initial.to_str() == Some(&time_namespace),
        "controller time namespace changed",
    )?;
    let t = TimeSample {
        boot_id,
        boottime_ns: ns(libc::CLOCK_BOOTTIME)?,
        realtime_ns: ns(libc::CLOCK_REALTIME)?,
        time_namespace,
    };
    t.validate()?;
    Ok(t)
}
/// No Debug/Serialize/Display implementation. Secrets never leave transport.
pub(crate) struct Credentials {
    pub(crate) basic: Zeroizing<String>,
    username: Zeroizing<String>,
    password: Zeroizing<String>,
}
impl Credentials {
    #[cfg(test)]
    pub(crate) fn synthetic() -> Self {
        Self {
            basic: Zeroizing::new("Basic c3ludGhldGljLXVzZXI6c3ludGhldGljLXBhc3M=".into()),
            username: Zeroizing::new("synthetic-user".into()),
            password: Zeroizing::new("synthetic-pass".into()),
        }
    }
    pub(crate) fn response_safe(&self, value: &serde_json::Value, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        match value {
            serde_json::Value::String(s) => {
                !s.contains(self.username.as_str())
                    && !s.contains(self.password.as_str())
                    && !s.contains(self.basic.as_str())
                    && !s.contains(self.basic.trim_start_matches("Basic "))
            }
            serde_json::Value::Array(a) => a.iter().all(|v| self.response_safe(v, depth + 1)),
            serde_json::Value::Object(m) => m.iter().all(|(k, v)| {
                !k.contains(self.password.as_str())
                    && !k.contains(self.username.as_str())
                    && !k.contains(self.basic.trim_start_matches("Basic "))
                    && self.response_safe(v, depth + 1)
            }),
            _ => true,
        }
    }
    pub(crate) fn load() -> Result<Self> {
        protect_credentials()?;
        let system = File::open("/").map_err(|_| Error("system root"))?;
        let file = confined(
            &system,
            "etc/borrowser-host-lifecycle/robot.credentials",
            libc::O_RDONLY,
            false,
        )?;
        let m = file.metadata().map_err(|_| Error("credential metadata"))?;
        require(
            m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0 && m.nlink() == 1,
            "credential protection",
        )?;
        let bytes = Zeroizing::new(read_bound(file, 8192)?);
        require(
            !bytes.is_empty() && !bytes.contains(&b'\n') && bytes.contains(&b':'),
            "credential format",
        )?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Error("credential encoding"))?;
        let (username, password) = text.split_once(':').ok_or(Error("credential format"))?;
        require(
            !username.is_empty()
                && !password.is_empty()
                && username.len() <= 4096
                && password.len() <= 4096,
            "credential bounds",
        )?;
        let username = Zeroizing::new(username.to_owned());
        let password = Zeroizing::new(password.to_owned());
        use base64::Engine;
        let encoded = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(&*bytes));
        Ok(Self {
            basic: Zeroizing::new(format!("Basic {}", encoded.as_str())),
            username,
            password,
        })
    }
}

fn protect_credentials() -> Result<()> {
    let limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    require(
        unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) } == 0,
        "disable core dumps",
    )?;
    require(
        unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0,
        "disable process dumpability",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mount_contract_rejects_missing_overlay_nested_and_wrong_identity() {
        let line = format!("42 1 0:1 / {ROOT} rw - ext4 /dev/test rw");
        let device = libc::makedev(0, 1);
        assert!(validate_mount_inventory(&line, device, 42).is_ok());
        for invalid in [
            String::new(),
            line.replace("ext4", "overlay"),
            line.replace("ext4", "nfs"),
            format!("{line}\n43 42 0:1 / {ROOT}/journal rw - ext4 /dev/test rw"),
        ] {
            assert!(validate_mount_inventory(&invalid, device, 42).is_err());
        }
        assert!(validate_mount_inventory(&line, device, 43).is_err());
    }
    #[test]
    fn descriptors_reject_symlinks_and_escape_after_rename() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("authority")).unwrap();
        let root = File::open(temp.path().join("authority")).unwrap();
        std::fs::write(temp.path().join("authority/value"), b"retained").unwrap();
        std::os::unix::fs::symlink("value", temp.path().join("authority/link")).unwrap();
        assert!(confined(&root, "link", libc::O_RDONLY, true).is_err());
        assert!(confined(&root, "../outside", libc::O_RDONLY, true).is_err());
        std::fs::rename(temp.path().join("authority"), temp.path().join("moved")).unwrap();
        std::fs::create_dir(temp.path().join("authority")).unwrap();
        std::fs::write(temp.path().join("authority/value"), b"substituted").unwrap();
        assert_eq!(
            read_bound(confined(&root, "value", libc::O_RDONLY, true).unwrap(), 16).unwrap(),
            b"retained"
        );
    }
    #[test]
    fn credential_protection_subprocess() {
        if std::env::var_os("LIFECYCLE_DUMPABILITY_TEST").is_none() {
            return;
        }
        protect_credentials().unwrap();
        assert_eq!(unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) }, 0);
        let mut limit = libc::rlimit {
            rlim_cur: 1,
            rlim_max: 1,
        };
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_CORE, &mut limit) }, 0);
        assert_eq!((limit.rlim_cur, limit.rlim_max), (0, 0));
    }
    #[test]
    fn credentials_disable_dumpability_and_core_dumps() {
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "linux::tests::credential_protection_subprocess"])
                .env("LIFECYCLE_DUMPABILITY_TEST", "1")
                .status()
                .unwrap()
                .success()
        );
    }
}
