//! Linux controller boundary; never runs on the acquired qualification candidate.
use crate::deployment::DeploymentV2;
use crate::{
    Error, Result, canonical,
    journal::{Journal, ROOT},
    require,
    scheduling::TimeSample,
};
use std::{
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
};
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
pub(crate) fn deployment() -> Result<DeploymentV2> {
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
    let d: DeploymentV2 = canonical::decode(&read_bound(file, 16_384)?)?;
    d.validate()?;
    Ok(d)
}
fn authority_root(d: &DeploymentV2) -> Result<File> {
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
        machine.trim() == d.identity.controller_machine_id,
        "wrong controller",
    )?;
    require(
        d.identity
            .filesystem_uuid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-'),
        "filesystem uuid syntax",
    )?;
    // This device symlink is used only for independently checking rdev; never for authority I/O.
    let device = std::fs::metadata(format!("/dev/disk/by-uuid/{}", d.identity.filesystem_uuid))
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
pub(crate) fn open_authority(
    d: &DeploymentV2,
    bootstrap: bool,
    tool: crate::model::ToolIdentityV2,
) -> Result<Journal> {
    d.validate()?;
    let root = authority_root(d)?;
    let marker = d.marker()?;
    if bootstrap {
        let genesis = crate::model::EnvelopeV2::genesis(&marker, now()?, tool)?;
        Journal::bootstrap_verified(root, &marker, &genesis)
    } else {
        Journal::open_verified(root, &marker)
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
}
