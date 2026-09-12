//! Private mount namespace primitives; no host-backed writable alias to frozen files.
use crate::{CaptureError as E, Result};
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};
pub(crate) fn mount(
    source: Option<&Path>,
    target: &Path,
    kind: Option<&str>,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<()> {
    let s = source
        .map(|p| CString::new(p.as_os_str().as_bytes()))
        .transpose()
        .map_err(|_| E::Path)?;
    let t = CString::new(target.as_os_str().as_bytes()).map_err(|_| E::Path)?;
    let k = kind.map(CString::new).transpose().map_err(|_| E::Path)?;
    let d = data.map(CString::new).transpose().map_err(|_| E::Path)?;
    if unsafe {
        libc::mount(
            s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            t.as_ptr(),
            k.as_ref().map_or(std::ptr::null(), |k| k.as_ptr()),
            flags,
            d.as_ref().map_or(std::ptr::null(), |d| d.as_ptr().cast()),
        )
    } != 0
    {
        return Err(E::Isolation);
    }
    Ok(())
}
pub(crate) fn private_namespace() -> Result<()> {
    if unsafe { libc::unshare(libc::CLONE_NEWNS) } != 0 {
        return Err(E::Isolation);
    }
    mount(
        None,
        Path::new("/"),
        None,
        libc::MS_REC | libc::MS_PRIVATE,
        None,
    )
}
pub(crate) fn tmpfs(path: &Path) -> Result<()> {
    mount(
        None,
        path,
        Some("tmpfs"),
        libc::MS_NOSUID | libc::MS_NODEV,
        Some("size=8589934592,mode=0700"),
    )
}
pub(crate) fn readonly(path: &Path) -> Result<()> {
    mount(
        None,
        path,
        None,
        libc::MS_REMOUNT | libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
        None,
    )?;
    let c = CString::new(path.as_os_str().as_bytes()).map_err(|_| E::Path)?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut stat) } != 0 || stat.f_flag & libc::ST_RDONLY == 0 {
        return Err(E::Isolation);
    }
    Ok(())
}

/// Constructible only by the pre-browser namespace setup. The caller must own
/// the fresh private /tmp mount and must not fork until this workspace is sealed.
pub(crate) struct PrivateWorkspace {
    directory: std::fs::File,
}
impl PrivateWorkspace {
    pub(crate) fn create() -> Result<Self> {
        let path = Path::new("/tmp/ag9g/workspace");
        std::fs::create_dir(path).map_err(|_| E::Isolation)?;
        tmpfs(path)?;
        Ok(Self {
            directory: std::fs::File::open(path).map_err(|_| E::Isolation)?,
        })
    }
    pub(crate) fn directory(&self) -> &std::fs::File {
        &self.directory
    }
    pub(crate) fn seal(&self) -> Result<()> {
        // Profile is a separate fresh tmpfs. Protect the parent mount before
        // acquiring the final profile handle; the profile remains writable.
        // A bind remount changes only this mount, not the writable profile submount.
        mount(
            None,
            Path::new("/tmp/ag9g/workspace"),
            None,
            libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
            None,
        )
    }
}
