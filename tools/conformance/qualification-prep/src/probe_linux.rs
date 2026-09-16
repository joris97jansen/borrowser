//! Linux-only candidate identity probe. No capture or general CDP API.
//! The outer process owns unshare, which owns namespace PID 1. Parent-death
//! signals and PID-namespace teardown close the descendant lifetime boundary.
use crate::{
    Error, Result, canonical,
    configuration::ARGUMENTS,
    distribution::CandidateDistributionManifest,
    error::{after_cleanup, require},
    identity::{self, AUTHORITY, BrowserIdentity, Framer},
    linux_fs,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{fs::MetadataExt, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const DIAGNOSTICS: usize = 262144;
const WALL_MS: u64 = 120000;
const CLEANUP_MS: u64 = 10000;
#[derive(Debug)]
pub struct ProbeInput {
    pub distribution: PathBuf,
    pub manifest: CandidateDistributionManifest,
    pub executable: String,
    pub unshare: PathBuf,
    pub unshare_sha256: String,
    pub unshare_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerInput {
    helper_fd: i32,
    helper_identity: HelperIdentity,
    root: String,
    executable: String,
    executable_sha256: String,
    manifest_sha256: String,
    uid: u32,
    gid: u32,
    parent_namespaces: [u64; 4],
    parent_proc_device: u64,
    parent_proc_inode: u64,
    deadline_ms: u64,
    unshare_sha256: String,
    unshare_version: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HelperIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    length: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
    links: u64,
}
impl HelperIdentity {
    fn of(file: &File) -> Result<Self> {
        let m = file.metadata()?;
        require(m.is_file() && m.mode() & 0o111 != 0, "helper executable")?;
        linux_fs::no_capabilities(file)?;
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
            mode: m.mode(),
            uid: m.uid(),
            gid: m.gid(),
            length: m.len(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
            links: m.nlink(),
        })
    }
    fn same_object(&self, other: &Self) -> bool {
        // Rename/unlink may change ctime/nlink without changing executable bytes.
        // UID/GID presentation changes across the new user namespace; ownership
        // policy is checked at acquisition, while device/inode binds the worker.
        self.device == other.device
            && self.inode == other.inode
            && self.mode == other.mode
            && self.length == other.length
            && self.mtime == other.mtime
            && self.mtime_ns == other.mtime_ns
    }
}
struct WorkerExecutable {
    file: File,
    identity: HelperIdentity,
}
impl WorkerExecutable {
    fn acquire() -> Result<Self> {
        // Deliberately follow this kernel magic link to the running executable,
        // never the mutable pathname returned by current_exe().
        Self::retain(File::open("/proc/self/exe")?)
    }
    fn retain(file: File) -> Result<Self> {
        linux_fs::metadata(&file.metadata()?)?;
        let identity = HelperIdentity::of(&file)?;
        let file = duplicate_high(&file)?;
        require(
            identity == HelperIdentity::of(&file)?,
            "helper acquisition changed",
        )?;
        Ok(Self { file, identity })
    }
    fn configure(&self, command: &mut Command) {
        let fd = self.file.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                // All non-stdio handles close on exec except this deliberate bridge.
                if libc::syscall(
                    libc::SYS_close_range,
                    3u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) != 0
                    || libc::fcntl(fd, libc::F_SETFD, 0) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    fn check_worker(fd: i32, expected: &HelperIdentity) -> Result<File> {
        require(
            fd >= 10 && unsafe { libc::fcntl(fd, libc::F_GETFD) } >= 0,
            "helper inherited descriptor",
        )?;
        // WorkerInput is private, and this descriptor is transferred exactly once.
        let file = unsafe { File::from_raw_fd(fd) };
        require(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "helper descriptor close-on-exec",
        )?;
        let running = File::open("/proc/self/exe")?;
        require(
            expected.same_object(&HelperIdentity::of(&file)?)
                && expected.same_object(&HelperIdentity::of(&running)?),
            "worker self identity",
        )?;
        Ok(file)
    }
}
pub fn read_proc(path: &str, max: usize) -> Result<String> {
    let mut out = String::new();
    File::open(path)?
        .take(max as u64 + 1)
        .read_to_string(&mut out)?;
    require(out.len() <= max, "proc input bound")?;
    Ok(out)
}
fn now_ms() -> Result<u64> {
    let mut t: libc::timespec = unsafe { std::mem::zeroed() };
    require(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } == 0,
        "monotonic clock",
    )?;
    Ok(t.tv_sec as u64 * 1000 + t.tv_nsec as u64 / 1_000_000)
}
fn remaining(end: u64) -> Result<i32> {
    let now = now_ms()?;
    if now >= end {
        Err(Error::Timeout)
    } else {
        Ok((end - now).min(100) as i32)
    }
}
fn namespaces() -> Result<[u64; 4]> {
    let mut ids = [0; 4];
    for (slot, name) in ids.iter_mut().zip(["user", "pid", "net", "mnt"]) {
        *slot = std::fs::metadata(format!("/proc/self/ns/{name}"))?.ino();
    }
    Ok(ids)
}
pub fn verify_map(text: &str, id: u32) -> Result<()> {
    require(id != 0, "zero mapped ID")?;
    let values: Vec<_> = text.split_whitespace().collect();
    require(
        values.len() == 3
            && values[0].parse::<u32>().ok() == Some(id)
            && values[1].parse::<u32>().ok() == Some(id)
            && values[2] == "1",
        "UID/GID mapping",
    )
}
/// Additional proc mounts, including /proc subtree bind mounts, are unsupported.
pub fn verify_proc_mounts(text: &str) -> Result<()> {
    proc_mounts(text, true)
}
fn proc_mounts(text: &str, private: bool) -> Result<()> {
    let mut count = 0;
    for line in text.lines() {
        let (left, right) = line.split_once(" - ").ok_or(Error::Invalid("mountinfo"))?;
        let f: Vec<_> = left.split_whitespace().collect();
        let r: Vec<_> = right.split_whitespace().collect();
        require(f.len() >= 6 && r.len() >= 3, "mountinfo fields")?;
        if r[0] == "proc" {
            count += 1;
            require(f[3] == "/" && f[4] == "/proc", "host proc alias")?;
            if private {
                for flag in ["rw", "nosuid", "nodev", "noexec"] {
                    require(f[5].split(',').any(|s| s == flag), "proc mount flags")?;
                }
                require(
                    !f[6..]
                        .iter()
                        .any(|s| s.starts_with("shared:") || s.starts_with("master:")),
                    "private proc propagation",
                )?;
            }
        }
    }
    require(count == 1, "private proc population")
}
pub fn verify_routes(v4: &str, v6: &str) -> Result<()> {
    for line in v4.lines().skip(1) {
        let f: Vec<_> = line.split_whitespace().collect();
        require(f.len() >= 4, "IPv4 route")?;
        let flags = u32::from_str_radix(f[3], 16).map_err(|_| Error::Invalid("IPv4 flags"))?;
        require(flags & 1 == 0 || flags & 0x200 != 0, "usable IPv4 route")?;
    }
    for line in v6.lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        require(f.len() == 10, "IPv6 route")?;
        let flags = u32::from_str_radix(f[8], 16).map_err(|_| Error::Invalid("IPv6 flags"))?;
        require(flags & 1 == 0 || flags & 0x200 != 0, "usable IPv6 route")?;
    }
    Ok(())
}
fn offline() -> Result<()> {
    let text = read_proc("/proc/net/dev", 65536)?;
    let names: Vec<_> = text
        .lines()
        .skip(2)
        .map(|l| l.split_once(':').map(|(n, _)| n.trim()))
        .collect();
    require(names == [Some("lo")], "network interfaces")?;
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
    for (dst, src) in req.ifr_name.iter_mut().zip(b"lo\0") {
        *dst = *src as libc::c_char;
    }
    require(
        unsafe { libc::ioctl(socket.as_raw_fd(), libc::SIOCGIFFLAGS, &mut req) } == 0,
        "loopback flags",
    )?;
    require(
        unsafe { req.ifr_ifru.ifru_flags } & libc::IFF_UP as i16 == 0,
        "loopback up",
    )?;
    verify_routes(
        &read_proc("/proc/net/route", 65536)?,
        &read_proc("/proc/net/ipv6_route", 65536)?,
    )
}
fn verify_readonly(root: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(root.as_os_str().as_bytes()).map_err(|_| Error::Invalid("root path"))?;
    let mut v: libc::statvfs = unsafe { std::mem::zeroed() };
    require(
        unsafe { libc::statvfs(c.as_ptr(), &mut v) } == 0 && v.f_flag & libc::ST_RDONLY != 0,
        "distribution must be on read-only storage",
    )?;
    // Protect pathname ancestry as well as the payload mount. The browser has
    // no capability to rename root-owned ancestors or write a read-only ancestor.
    for ancestor in root.ancestors().skip(1) {
        let directory = linux_fs::open_root(ancestor)?;
        let m = directory.metadata()?;
        require(m.mode() & 0o022 == 0, "writable distribution ancestor")?;
        if m.uid() == unsafe { libc::getuid() } && m.mode() & 0o200 != 0 {
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            require(
                unsafe { libc::fstatvfs(directory.as_raw_fd(), &mut stat) } == 0
                    && stat.f_flag & libc::ST_RDONLY != 0,
                "owner-writable distribution ancestor",
            )?;
        }
    }
    let dev = std::fs::metadata(root)?.dev();
    let device = format!("{}:{}", libc::major(dev), libc::minor(dev));
    for line in read_proc("/proc/self/mountinfo", 262144)?.lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        require(f.len() >= 6, "mountinfo")?;
        if f[2] == device {
            require(
                f[5].split(',').any(|o| o == "ro"),
                "writable distribution device alias",
            )?;
        }
    }
    Ok(())
}
fn verify_worker(input: &WorkerInput) -> Result<()> {
    require(
        std::env::consts::ARCH == "x86_64" && unsafe { libc::getpid() } == 1,
        "native namespace PID 1",
    )?;
    require(
        unsafe { libc::getuid() } == input.uid
            && unsafe { libc::geteuid() } == input.uid
            && unsafe { libc::getgid() } == input.gid
            && unsafe { libc::getegid() } == input.gid
            && input.uid != 0
            && input.gid != 0,
        "worker user",
    )?;
    verify_map(&read_proc("/proc/self/uid_map", 4096)?, input.uid)?;
    verify_map(&read_proc("/proc/self/gid_map", 4096)?, input.gid)?;
    require(
        read_proc("/proc/self/setgroups", 128)?.trim() == "deny",
        "setgroups",
    )?;
    let mut death_signal = 0;
    require(
        unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut death_signal) } == 0
            && death_signal == libc::SIGKILL,
        "unshare child-death behavior",
    )?;
    let actual = namespaces()?;
    require(
        actual
            .iter()
            .zip(input.parent_namespaces)
            .all(|(a, p)| *a != p),
        "fresh namespaces",
    )?;
    let user = File::open("/proc/self/ns/user")?;
    let net = File::open("/proc/self/ns/net")?;
    let owner = unsafe { libc::ioctl(net.as_raw_fd(), 0xb701) }; // NS_GET_USERNS
    if owner < 0 {
        return Err(Error::Invalid("network namespace owner"));
    }
    let owner = unsafe { File::from_raw_fd(owner) };
    require(
        owner.metadata()?.ino() == user.metadata()?.ino(),
        "network ownership",
    )?;
    require(
        std::fs::read_link("/proc/self")? == Path::new("1"),
        "proc self identity",
    )?;
    let proc = std::fs::metadata("/proc")?;
    require(
        (proc.dev(), proc.ino()) != (input.parent_proc_device, input.parent_proc_inode),
        "fresh procfs",
    )?;
    verify_proc_mounts(&read_proc("/proc/self/mountinfo", 262144)?)?;
    require(numeric_pids()? == [1], "initial private proc population")?;
    let status = read_proc("/proc/self/status", 65536)?;
    require(
        status.lines().any(|s| s == "CapEff:\t0000000000000000"),
        "worker capabilities",
    )?;
    offline()?;
    verify_readonly(Path::new(&input.root))
}
fn numeric_pids() -> Result<Vec<i32>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir("/proc")? {
        let e = e?;
        if let Ok(pid) = e.file_name().to_string_lossy().parse() {
            require(out.len() < 4096, "process count")?;
            out.push(pid);
        }
    }
    out.sort();
    Ok(out)
}
fn pidfd(pid: u32) -> Result<OwnedFd> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}
fn kill(fd: &OwnedFd) -> Result<()> {
    let rc = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(Error::Cleanup)
    }
}
struct SpawnedChild {
    child: Child,
    reaped: bool,
    partial: bool,
}
impl SpawnedChild {
    fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
            partial: true,
        }
    }
    fn poll(&mut self, end: Instant) -> Result<Option<std::process::ExitStatus>> {
        loop {
            #[cfg(test)]
            let interrupted = tests::SPAWN_INTERRUPT.replace(false);
            #[cfg(not(test))]
            let interrupted = false;
            let result = if interrupted {
                Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
            } else {
                self.child.try_wait()
            };
            match result {
                Ok(status) => {
                    if status.is_some() {
                        self.reaped = true;
                    }
                    return Ok(status);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    if Instant::now() >= end {
                        return Err(Error::Cleanup);
                    }
                }
                Err(e) => {
                    // ECHILD is a lost ownership proof, not successful reaping.
                    // Suppress PID-based fallback once somebody else reaped it.
                    if e.raw_os_error() == Some(libc::ECHILD) {
                        self.reaped = true;
                    }
                    return Err(Error::Cleanup);
                }
            }
        }
    }
    fn cleanup(&mut self, handle: Option<&OwnedFd>, end: Instant) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        if self.poll(end)?.is_some() {
            return Ok(());
        }
        // Until we reap this direct child its PID cannot be recycled. PID-based
        // signaling is used only during partial acquisition, before pidfd_open.
        let signal = if let Some(handle) = handle {
            kill(handle)
        } else {
            self.child.kill().map_err(|_| Error::Cleanup)
        };
        #[cfg(test)]
        let signal = if tests::SPAWN_SIGNAL_FAILURE.get() {
            Err(Error::Cleanup)
        } else {
            signal
        };
        // Reaping is independent of the signal result, including signaling failure.
        let reap = (|| {
            while Instant::now() < end {
                if self.poll(end)?.is_some() {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(Error::Cleanup)
        })();
        after_cleanup(signal, reap)
    }
}
impl Drop for SpawnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            // Emergency fallback only: never block or count this as checked cleanup.
            if self.partial {
                let _ = self.child.kill();
            }
            let _ = self.child.try_wait();
        }
    }
}
struct OwnedChild {
    process: SpawnedChild,
    handle: OwnedFd,
}
impl OwnedChild {
    fn spawn(command: &mut Command) -> Result<Self> {
        let mut process = SpawnedChild::new(command.spawn()?);
        #[cfg(test)]
        let acquired = if tests::SPAWN_PIDFD_FAILURE.get() {
            Err(Error::Invalid("child pidfd injected"))
        } else {
            pidfd(process.child.id())
        };
        #[cfg(not(test))]
        let acquired = pidfd(process.child.id());
        match acquired {
            Ok(handle) => {
                process.partial = false; // Full owner's Drop signals through the retained pidfd.
                Ok(Self { process, handle })
            }
            Err(error) => {
                let cleanup =
                    process.cleanup(None, Instant::now() + Duration::from_millis(CLEANUP_MS));
                #[cfg(test)]
                tests::spawn_cleanup_checked(&process);
                after_cleanup(Err(error), cleanup)
            }
        }
    }
    fn poll(&mut self) -> Result<Option<std::process::ExitStatus>> {
        self.process
            .poll(Instant::now() + Duration::from_millis(CLEANUP_MS))
    }
    fn terminate(&mut self) -> Result<()> {
        self.process.cleanup(
            Some(&self.handle),
            Instant::now() + Duration::from_millis(CLEANUP_MS),
        )
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.process.reaped {
            let _ = kill(&self.handle);
        }
    }
}
fn nonblock(fd: RawFd) -> Result<()> {
    let f = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    require(
        f >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, f | libc::O_NONBLOCK) } == 0,
        "nonblocking pipe",
    )
}
fn read_available<R: Read>(r: &mut R, bytes: &mut Vec<u8>, max: usize) -> Result<bool> {
    let mut b = [0u8; 8192];
    loop {
        match r.read(&mut b) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                require(
                    bytes.len().checked_add(n).is_some_and(|n| n <= max),
                    "output limit",
                )?;
                bytes.extend_from_slice(&b[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
}
fn poll_fds(fds: &[RawFd], end: u64) -> Result<()> {
    let timeout = remaining(end)?;
    let mut p: Vec<_> = fds
        .iter()
        .map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        })
        .collect();
    let n = unsafe { libc::poll(p.as_mut_ptr(), p.len() as libc::nfds_t, timeout) };
    if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
fn capture_command(command: &mut Command, end: u64, max: usize) -> Result<Vec<u8>> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = OwnedChild::spawn(command)?;
    let mut err = Vec::new();
    let result = (|| {
        let mut stdout = child
            .process
            .child
            .stdout
            .take()
            .ok_or(Error::Invalid("stdout"))?;
        let mut stderr = child
            .process
            .child
            .stderr
            .take()
            .ok_or(Error::Invalid("stderr"))?;
        nonblock(stdout.as_raw_fd())?;
        nonblock(stderr.as_raw_fd())?;
        let mut out = Vec::new();
        loop {
            poll_fds(
                &[
                    stdout.as_raw_fd(),
                    stderr.as_raw_fd(),
                    child.handle.as_raw_fd(),
                ],
                end,
            )?;
            let a = read_available(&mut stdout, &mut out, max)?;
            let b = read_available(&mut stderr, &mut err, DIAGNOSTICS)?;
            if let Some(status) = child.poll()? {
                if !a || !b {
                    continue;
                }
                if !status.success() {
                    return Err(Error::Io(format!(
                        "preparation child failed ({status}): {}",
                        String::from_utf8_lossy(&err)
                    )));
                }
                return Ok(out);
            }
        }
    })();
    let result = after_cleanup(result, child.terminate());
    if result.is_err() {
        let _ = std::io::stderr().write_all(&err);
    }
    result
}
fn parent_death(command: &mut Command) {
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 || libc::getppid() != parent
            {
                return Err(std::io::Error::other("parent identity"));
            }
            Ok(())
        });
    }
}
fn probe_inner(input: &ProbeInput, deadline: u64) -> Result<BrowserIdentity> {
    let end = deadline;
    require(
        std::env::consts::ARCH == "x86_64"
            && unsafe { libc::getuid() } != 0
            && unsafe { libc::geteuid() } == unsafe { libc::getuid() }
            && unsafe { libc::getgid() } == unsafe { libc::getegid() }
            && unsafe { libc::getgid() } != 0
            && std::fs::read_dir("/proc/self/task")?.count() == 1,
        "native non-root single-threaded probe",
    )?;
    let helper = WorkerExecutable::acquire()?;
    require(input.unshare.is_absolute(), "absolute unshare path")?;
    canonical::digest(&input.unshare_sha256)?;
    canonical::identity(&input.unshare_version)?;
    proc_mounts(&read_proc("/proc/self/mountinfo", 262144)?, false)?;
    verify_readonly(&input.distribution)?;
    require(
        crate::distribution::inventory(&input.distribution)? == input.manifest,
        "supplied manifest mismatch",
    )?;
    let expected = input
        .manifest
        .executable_digest(&input.executable)?
        .to_owned();
    // Retain the actual unshare executable; /proc/self/fd is a kernel object handle.
    let mut unshare = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&input.unshare)?;
    linux_fs::metadata(&unshare.metadata()?)?;
    linux_fs::no_capabilities(&unshare)?;
    require(
        linux_fs::file_digest(&mut unshare, 64 * 1024 * 1024)? == input.unshare_sha256,
        "unshare executable digest",
    )?;
    let unshare_path = format!("/proc/self/fd/{}", unshare.as_raw_fd());
    let mut version = Command::new(&unshare_path);
    version.arg("--version").env_clear();
    parent_death(&mut version);
    let bytes = capture_command(&mut version, (now_ms()? + 5000).min(end), 4096)?;
    require(
        bytes == format!("{}\n", input.unshare_version).as_bytes(),
        "unshare version",
    )?;
    let workspace = fresh_workspace()?;
    let result = (|| {
        let proc = std::fs::metadata("/proc")?;
        let config = WorkerInput {
            helper_fd: helper.file.as_raw_fd(),
            helper_identity: helper.identity.clone(),
            root: input
                .distribution
                .to_str()
                .ok_or(Error::Invalid("distribution path UTF-8"))?
                .into(),
            executable: input.executable.clone(),
            executable_sha256: expected,
            manifest_sha256: canonical::hash(&input.manifest.canonical_bytes()?),
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            parent_namespaces: namespaces()?,
            parent_proc_device: proc.dev(),
            parent_proc_inode: proc.ino(),
            deadline_ms: end - CLEANUP_MS,
            unshare_sha256: input.unshare_sha256.clone(),
            unshare_version: input.unshare_version.clone(),
        };
        let config_bytes = canonical::json(&config)?;
        let config_text =
            std::str::from_utf8(&config_bytes).map_err(|_| Error::Invalid("worker input"))?;
        let mut command = Command::new(&unshare_path);
        command
            .args([
                "--user",
                "--map-current-user",
                "--net",
                "--pid",
                "--fork",
                "--mount",
                "--mount-proc",
                "--propagation",
                "private",
                "--kill-child=KILL",
                "--",
            ])
            .arg(format!("/proc/self/fd/{}", helper.file.as_raw_fd()))
            .arg("__probe-worker")
            .arg(config_text)
            .current_dir(workspace.path())
            .env_clear()
            .env("LANG", "C")
            .env("LC_ALL", "C");
        helper.configure(&mut command);
        parent_death(&mut command);
        let bytes = capture_command(&mut command, end - CLEANUP_MS, 65536)?;
        let tuple: identity::VersionTuple = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Invalid("provisional worker result"))?;
        require(canonical::json(&tuple)? == bytes, "worker canonical output")?;
        let (product, version) = identity::split_product(&tuple.product)?;
        let result = BrowserIdentity {
            format: "borrowser-preparation-browser-identity-v1".into(),
            authority: AUTHORITY.into(),
            product_raw: tuple.product.clone(),
            browser_product: product.into(),
            browser_version: version.into(),
            revision: tuple.revision,
            protocol_version: tuple.protocol,
            executable_sha256: config.executable_sha256,
            distribution_manifest_sha256: config.manifest_sha256,
            unshare_sha256: config.unshare_sha256,
            unshare_version: config.unshare_version,
        };
        result.validate()?;
        // Re-enumerate after probing: candidate cannot silently bind changed input.
        require(
            crate::distribution::inventory(&input.distribution)? == input.manifest,
            "distribution changed during probe",
        )?;
        remaining(end)?;
        Ok(result)
    })();
    after_cleanup(result, workspace.close().map_err(|_| Error::Cleanup))
}
use std::os::unix::fs::OpenOptionsExt;
fn fresh_workspace() -> Result<tempfile::TempDir> {
    use std::os::unix::fs::PermissionsExt;
    Ok(tempfile::Builder::new()
        .prefix("ag9g-preparation-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()?)
}
fn prepare_profile(cwd: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::symlink_metadata(cwd)?;
    require(
        metadata.is_dir()
            && metadata.uid() == unsafe { libc::getuid() }
            && metadata.mode() & 0o777 == 0o700,
        "private workspace ownership/mode",
    )?;
    let profile = cwd.join("profile");
    std::fs::create_dir(&profile)?;
    std::fs::set_permissions(profile, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn pipe() -> Result<(File, File)> {
    let mut fds = [-1; 2];
    require(
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } == 0,
        "pipe",
    )?;
    Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
}
fn duplicate_high(file: &File) -> Result<File> {
    let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn write_request(file: &mut File, bytes: &[u8], end: u64) -> Result<()> {
    // Requests are smaller than PIPE_BUF; nonblocking writes cannot hang.
    remaining(end)?;
    file.write_all(bytes)?;
    Ok(())
}
fn reap_descendants(end: u64) -> Result<()> {
    loop {
        let mut status = 0;
        let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if pid > 0 {
            continue;
        }
        if pid < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            return require(numeric_pids()? == [1], "remaining namespace processes");
        }
        remaining(end)?;
        std::thread::sleep(Duration::from_millis(5));
    }
}
/// Called only in the new namespace by the executable, never by a library caller.
pub fn worker(raw: &str) -> Result<Vec<u8>> {
    require(raw.len() <= 65536, "worker input limit")?;
    let i: WorkerInput = serde_json::from_str(raw).map_err(|_| Error::Invalid("worker schema"))?;
    require(
        canonical::json(&i)? == raw.as_bytes(),
        "worker canonical input",
    )?;
    let _helper = WorkerExecutable::check_worker(i.helper_fd, &i.helper_identity)?;
    verify_worker(&i)?;
    remaining(i.deadline_ms)?;
    let root = linux_fs::open_root(Path::new(&i.root))?;
    let mut executable = linux_fs::confined(&root, &i.executable)?;
    require(
        linux_fs::file_digest(&mut executable, crate::distribution::FILE_BYTES)?
            == i.executable_sha256,
        "probe executable",
    )?;
    let executable = duplicate_high(&executable)?;
    prepare_profile(&std::env::current_dir()?)?;
    let (initial_read, mut send) = pipe()?;
    let (mut receive, initial_write) = pipe()?;
    let browser_read = duplicate_high(&initial_read)?;
    let browser_write = duplicate_high(&initial_write)?;
    drop(initial_read);
    drop(initial_write);
    nonblock(send.as_raw_fd())?;
    nonblock(receive.as_raw_fd())?;
    let mut command = Command::new(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    command
        .args(ARGUMENTS)
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TMPDIR", std::env::current_dir()?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let read_fd = browser_read.as_raw_fd();
    let write_fd = browser_write.as_raw_fd();
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(read_fd, 3) < 0 || libc::dup2(write_fd, 4) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Leave the retained executable usable until exec resolves /proc/self/fd.
            if libc::syscall(
                libc::SYS_close_range,
                5u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    parent_death(&mut command);
    let mut browser = OwnedChild::spawn(&mut command)?;
    drop(browser_read);
    drop(browser_write);
    let tuple = exchange(&mut browser, &mut send, &mut receive, i.deadline_ms)?;
    reap_descendants(i.deadline_ms)?;
    offline()?;
    // Provisional internal transport is deliberately not a BrowserIdentity record.
    // Only the outer owner can construct/publish one after workspace/watchdog cleanup.
    canonical::json(&tuple)
}

fn exchange(
    browser: &mut OwnedChild,
    send: &mut File,
    receive: &mut File,
    end: u64,
) -> Result<identity::VersionTuple> {
    let mut diagnostics = Vec::new();
    let result = (|| {
        let mut stderr = browser
            .process
            .child
            .stderr
            .take()
            .ok_or(Error::Invalid("browser diagnostics"))?;
        nonblock(stderr.as_raw_fd())?;
        let mut framing = Framer::for_probe();
        let mut tuple = None;
        let mut close_ack = false;
        write_request(send, identity::version_request(), end)?;
        loop {
            poll_fds(
                &[
                    receive.as_raw_fd(),
                    stderr.as_raw_fd(),
                    browser.handle.as_raw_fd(),
                ],
                end,
            )?;
            read_available(&mut stderr, &mut diagnostics, DIAGNOSTICS)?;
            let mut buf = [0u8; 8192];
            let mut eof = false;
            loop {
                match receive.read(&mut buf) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(n) => {
                        for v in framing.push(&buf[..n])? {
                            if tuple.is_none() {
                                tuple = Some(identity::version_result(identity::response(&v, 1)?)?);
                                write_request(send, identity::close_request(), end)?;
                            } else {
                                require(
                                    !close_ack
                                        && identity::response(&v, 2)?
                                            .as_object()
                                            .is_some_and(|o| o.is_empty()),
                                    "shutdown response",
                                )?;
                                close_ack = true;
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
            }
            #[cfg(test)]
            tests::after_protocol_read(&browser.handle, tuple.is_some());
            if let Some(status) = browser.poll()? {
                require(status.success(), "browser abnormal exit")?;
                // Exit can race the previous nonblocking read. Drain all bytes
                // and observe EOF before interpreting a normal process exit.
                if !eof {
                    continue;
                }
                require(
                    tuple.is_some(),
                    "browser exited before version/orderly shutdown",
                )?;
                framing.finish()?;
                return tuple.ok_or(Error::Invalid("version missing"));
            }
            require(
                !eof || tuple.is_some(),
                "browser pipe closed before version",
            )?;
        }
    })();
    let result = after_cleanup(result, browser.terminate());
    if result.is_err() {
        let _ = std::io::stderr().write_all(&diagnostics);
    }
    result
}

// A separate process bounds even filesystem reads or exec preparation. Expiry
// kills this producer; the parent-death chain kills unshare and namespace PID 1.
// Abnormal termination never publishes a candidate and may leave local scratch.
struct ForkedWatchdog {
    pid: i32,
    reaped: bool,
}
impl ForkedWatchdog {
    fn wait_until(&mut self, end: Instant) -> Result<i32> {
        while Instant::now() < end {
            let mut status = 0;
            #[cfg(test)]
            let interrupted = tests::interrupt_watchdog_wait();
            #[cfg(not(test))]
            let interrupted = false;
            let n = if interrupted {
                -1
            } else {
                unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) }
            };
            if n == self.pid {
                self.reaped = true;
                return Ok(status);
            }
            if n < 0 {
                let error = if interrupted {
                    Some(libc::EINTR)
                } else {
                    std::io::Error::last_os_error().raw_os_error()
                };
                match error {
                    Some(libc::EINTR) => continue,
                    Some(libc::ECHILD) => {
                        self.reaped = true;
                        return Err(Error::Cleanup);
                    }
                    _ => return Err(Error::Cleanup),
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(Error::Cleanup)
    }
    fn cleanup(&mut self, handle: Option<&OwnedFd>) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        let signal = if let Some(handle) = handle {
            kill(handle)
        } else {
            let n = unsafe { libc::kill(self.pid, libc::SIGKILL) };
            if n == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(Error::Cleanup)
            }
        };
        let reap = self
            .wait_until(Instant::now() + Duration::from_secs(2))
            .map(|_| ());
        after_cleanup(signal, reap)
    }
}
impl Drop for ForkedWatchdog {
    fn drop(&mut self) {
        if !self.reaped {
            // Emergency only. Never block and never count this as checked completion.
            unsafe {
                libc::kill(self.pid, libc::SIGKILL);
                libc::waitpid(self.pid, std::ptr::null_mut(), libc::WNOHANG);
            }
        }
    }
}
struct Watchdog {
    child: ForkedWatchdog,
    handle: OwnedFd,
    stop: File,
}
impl Watchdog {
    fn arm(end: u64) -> Result<Self> {
        let parent = pidfd(unsafe { libc::getpid() } as u32)?;
        let (read, write) = pipe()?;
        let timeout = remaining(end)?;
        let _ = timeout;
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if pid == 0 {
            let mut fds = [
                libc::pollfd {
                    fd: read.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: parent.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            loop {
                let mut t: libc::timespec = unsafe { std::mem::zeroed() };
                if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut t) } != 0 {
                    unsafe { libc::_exit(2) }
                }
                let now = t.tv_sec as u64 * 1000 + t.tv_nsec as u64 / 1_000_000;
                if now >= end {
                    unsafe {
                        libc::syscall(
                            libc::SYS_pidfd_send_signal,
                            parent.as_raw_fd(),
                            libc::SIGKILL,
                            std::ptr::null::<libc::siginfo_t>(),
                            0,
                        );
                        libc::_exit(1)
                    }
                }
                let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, (end - now).min(1000) as i32) };
                if n > 0 && fds[1].revents != 0 {
                    unsafe { libc::_exit(0) }
                }
                if n > 0 && fds[0].revents & libc::POLLIN != 0 {
                    let mut byte = 0u8;
                    let n =
                        unsafe { libc::read(read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
                    unsafe { libc::_exit(if n == 1 && byte == 1 { 0 } else { 2 }) }
                }
            }
        }
        // No fallible step intervenes between fork and this owner.
        let mut child = ForkedWatchdog { pid, reaped: false };
        drop(read);
        #[cfg(test)]
        let acquired = if tests::watchdog_acquisition_failure() {
            Err(Error::Invalid("watchdog pidfd injected"))
        } else {
            pidfd(pid as u32)
        };
        #[cfg(not(test))]
        let acquired = pidfd(pid as u32);
        let handle = match acquired {
            Ok(fd) => fd,
            Err(error) => {
                let cleanup = child.cleanup(None);
                #[cfg(test)]
                let cleanup = tests::watchdog_partial_cleanup(cleanup, &child);
                return after_cleanup(Err(error), cleanup);
            }
        };
        Ok(Self {
            child,
            handle,
            stop: write,
        })
    }
    fn finish(mut self) -> Result<()> {
        let operation = (|| {
            self.stop.write_all(&[1]).map_err(|_| Error::Cleanup)?;
            let status = self
                .child
                .wait_until(Instant::now() + Duration::from_secs(2))?;
            if libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 {
                Ok(())
            } else {
                Err(Error::Cleanup)
            }
        })();
        let cleanup = self.child.cleanup(Some(&self.handle));
        after_cleanup(operation, cleanup)
    }
}

pub fn probe(input: &ProbeInput) -> Result<BrowserIdentity> {
    require(
        unsafe { libc::getuid() } != 0 && std::fs::read_dir("/proc/self/task")?.count() == 1,
        "non-root single-threaded producer",
    )?;
    let deadline = now_ms()? + WALL_MS;
    let watchdog = Watchdog::arm(deadline)?;
    let result = probe_inner(input, deadline);
    after_cleanup(result, watchdog.finish())
}
#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {
        pub(super) static SPAWN_PIDFD_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        pub(super) static SPAWN_SIGNAL_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        pub(super) static SPAWN_INTERRUPT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    pub(super) fn spawn_cleanup_checked(process: &SpawnedChild) {
        assert!(process.reaped);
        assert_eq!(
            unsafe {
                libc::waitpid(
                    process.child.id() as i32,
                    std::ptr::null_mut(),
                    libc::WNOHANG,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }
    #[test]
    fn spawned_child_entry() {
        if let Ok(mode) = std::env::var("AG9G_SPAWN_TEST")
            && mode == "hang"
        {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
    fn spawn_command(mode: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args(["--exact", "probe_linux::tests::spawned_child_entry"])
            .env("AG9G_SPAWN_TEST", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        c
    }
    #[test]
    fn spawned_child_partial_acquisition_and_transfer() {
        for signal_failure in [false, true] {
            SPAWN_PIDFD_FAILURE.set(true);
            SPAWN_SIGNAL_FAILURE.set(signal_failure);
            SPAWN_INTERRUPT.set(true);
            let result = OwnedChild::spawn(&mut spawn_command("hang"));
            SPAWN_PIDFD_FAILURE.set(false);
            SPAWN_SIGNAL_FAILURE.set(false);
            assert!(!SPAWN_INTERRUPT.get());
            assert!(
                matches!(result, Err(Error::Cleanup)) && signal_failure
                    || matches!(result, Err(Error::Invalid("child pidfd injected")))
                        && !signal_failure
            );
        }
        let mut full = OwnedChild::spawn(&mut spawn_command("hang")).unwrap();
        assert!(!full.process.reaped);
        assert!(!full.process.partial);
        full.terminate().unwrap();
        spawn_cleanup_checked(&full.process);
        full.terminate().unwrap();
    }
    #[test]
    fn spawned_child_exited_and_bounded_timeout() {
        let mut exited = SpawnedChild::new(spawn_command("exit").spawn().unwrap());
        let handle = pidfd(exited.child.id()).unwrap();
        let mut p = libc::pollfd {
            fd: handle.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut p, 1, 5000) }, 1);
        SPAWN_SIGNAL_FAILURE.set(true);
        // Already-exited child must be reaped without attempting signaling.
        exited
            .cleanup(None, Instant::now() + Duration::from_secs(2))
            .unwrap();
        SPAWN_SIGNAL_FAILURE.set(false);
        spawn_cleanup_checked(&exited);
        let mut live = SpawnedChild::new(spawn_command("hang").spawn().unwrap());
        assert_eq!(live.cleanup(None, Instant::now()), Err(Error::Cleanup));
        // Explicitly finish test-owned cleanup; Drop is not the test's proof.
        live.cleanup(None, Instant::now() + Duration::from_secs(2))
            .unwrap();
        spawn_cleanup_checked(&live);
    }
    thread_local! {static EXIT_READ_RACE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
    pub(super) fn after_protocol_read(handle: &OwnedFd, close_sent: bool) {
        if EXIT_READ_RACE.get() && close_sent {
            let mut p = libc::pollfd {
                fd: handle.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert!(
                unsafe { libc::poll(&mut p, 1, 1000) } > 0,
                "scripted peer terminal barrier"
            );
        }
    }
    thread_local! {
        static WATCHDOG_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
        static INTERRUPT_WAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    pub(super) fn interrupt_watchdog_wait() -> bool {
        INTERRUPT_WAIT.replace(false)
    }
    pub(super) fn watchdog_acquisition_failure() -> bool {
        WATCHDOG_FAULT.get() != 0
    }
    pub(super) fn watchdog_partial_cleanup(
        result: Result<()>,
        child: &ForkedWatchdog,
    ) -> Result<()> {
        assert!(child.reaped);
        assert_eq!(
            unsafe { libc::waitpid(child.pid, std::ptr::null_mut(), libc::WNOHANG) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
        if WATCHDOG_FAULT.get() == 2 {
            Err(Error::Cleanup)
        } else {
            result
        }
    }
    #[test]
    fn watchdog_partial_acquisition_cleanup_and_transfer() {
        for fault in [1, 2] {
            WATCHDOG_FAULT.set(fault);
            INTERRUPT_WAIT.set(true);
            let result = Watchdog::arm(now_ms().unwrap() + 5000);
            WATCHDOG_FAULT.set(0);
            assert!(
                matches!(result, Err(Error::Invalid("watchdog pidfd injected"))) && fault == 1
                    || matches!(result, Err(Error::Cleanup)) && fault == 2
            );
            assert!(!INTERRUPT_WAIT.get());
        }
        let mut full = Watchdog::arm(now_ms().unwrap() + 5000).unwrap();
        assert!(!full.child.reaped);
        assert_eq!(full.child.wait_until(Instant::now()), Err(Error::Cleanup));
        INTERRUPT_WAIT.set(true);
        full.finish().unwrap();
        assert!(!INTERRUPT_WAIT.get());
    }
    #[test]
    fn retained_worker_bridge_entry() {
        let Ok(raw) = std::env::var("AG9G_RETAINED_WORKER") else {
            return;
        };
        let (fd, _, unrelated): (i32, HelperIdentity, i32) = serde_json::from_str(&raw).unwrap();
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, 0);
        assert_eq!(unsafe { libc::fcntl(unrelated, libc::F_GETFD) }, -1);
        // Test-only exec bridge: proves the one descriptor survives two execs.
        // Actual util-linux namespace behavior remains an x86-64 host prerequisite.
        let error = Command::new(format!("/proc/self/fd/{fd}"))
            .args([
                "--exact",
                "probe_linux::tests::retained_worker_entry",
                "--nocapture",
            ])
            .exec();
        panic!("retained worker exec failed: {error}");
    }
    #[test]
    fn retained_worker_entry() {
        let Ok(raw) = std::env::var("AG9G_RETAINED_WORKER") else {
            return;
        };
        let (fd, expected, unrelated): (i32, HelperIdentity, i32) =
            serde_json::from_str(&raw).unwrap();
        let retained = WorkerExecutable::check_worker(fd, &expected).unwrap();
        assert_eq!(
            unsafe { libc::fcntl(retained.as_raw_fd(), libc::F_GETFD) },
            libc::FD_CLOEXEC
        );
        assert_eq!(unsafe { libc::fcntl(unrelated, libc::F_GETFD) }, -1);
    }
    #[test]
    fn retained_worker_survives_path_replacement_and_closes_unrelated_descriptors() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("helper");
        std::fs::copy("/proc/self/exe", &path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let helper = WorkerExecutable::retain(File::open(&path).unwrap()).unwrap();
        std::fs::rename(&path, d.path().join("original")).unwrap();
        std::fs::write(&path, b"not an executable").unwrap();
        let unrelated = duplicate_high(&File::open("/dev/null").unwrap()).unwrap();
        let other_fd = unrelated.as_raw_fd();
        let raw =
            serde_json::to_string(&(helper.file.as_raw_fd(), helper.identity.clone(), other_fd))
                .unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "probe_linux::tests::retained_worker_bridge_entry",
                "--nocapture",
            ])
            .env("AG9G_RETAINED_WORKER", raw);
        unsafe {
            command.pre_exec(move || {
                libc::fcntl(other_fd, libc::F_SETFD, 0);
                Ok(())
            });
        }
        helper.configure(&mut command);
        capture_command(&mut command, now_ms().unwrap() + 5000, 65536).unwrap();
        assert!(WorkerExecutable::retain(File::open(&path).unwrap()).is_err());
        let actual = WorkerExecutable::acquire().unwrap();
        let mut wrong = actual.identity.clone();
        wrong.inode ^= 1;
        let duplicate = duplicate_high(&actual.file).unwrap();
        use std::os::fd::IntoRawFd;
        assert!(WorkerExecutable::check_worker(duplicate.into_raw_fd(), &wrong).is_err());
    }
    #[test]
    fn scripted_peer_entry() {
        let Ok(mode) = std::env::var("PREP_TEST_PEER") else {
            return;
        };
        // This subprocess is a scripted protocol peer, never a browser or pin.
        let mut input = unsafe { File::from_raw_fd(3) };
        let mut output = unsafe { File::from_raw_fd(4) };
        if mode == "early" {
            return;
        }
        let mut request = vec![0; identity::version_request().len()];
        input.read_exact(&mut request).unwrap();
        assert_eq!(request, identity::version_request());
        if mode == "hang" {
            std::thread::sleep(Duration::from_secs(60));
            return;
        }
        if mode == "malformed" {
            output.write_all(b"broken\0").unwrap();
            return;
        }
        output.write_all(b"{\"id\":1,\"result\":{\"product\":\"Test/1\",\"revision\":\"@test\",\"protocolVersion\":\"test\"}}\0").unwrap();
        let mut request = vec![0; identity::close_request().len()];
        input.read_exact(&mut request).unwrap();
        assert_eq!(request, identity::close_request());
        if mode == "shutdown-hang" {
            std::thread::sleep(Duration::from_secs(60));
            return;
        }
        if mode == "shutdown-error" {
            output.write_all(b"{\"id\":2,\"error\":{}}\0").unwrap();
            return;
        }
        output.write_all(b"{\"id\":2,\"result\":{}}\0").unwrap();
    }
    fn peer(mode: &str) -> Result<identity::VersionTuple> {
        let (a, mut send) = pipe()?;
        let (mut receive, b) = pipe()?;
        let read = duplicate_high(&a)?;
        let write = duplicate_high(&b)?;
        drop(a);
        drop(b);
        nonblock(send.as_raw_fd())?;
        nonblock(receive.as_raw_fd())?;
        let mut c = Command::new(std::env::current_exe()?);
        c.args([
            "--exact",
            "probe_linux::tests::scripted_peer_entry",
            "--nocapture",
        ])
        .env("PREP_TEST_PEER", mode)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
        let r = read.as_raw_fd();
        let w = write.as_raw_fd();
        unsafe {
            c.pre_exec(move || {
                if libc::dup2(r, 3) < 0 || libc::dup2(w, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = OwnedChild::spawn(&mut c)?;
        drop(read);
        drop(write);
        let result = exchange(&mut child, &mut send, &mut receive, now_ms()? + 1500);
        assert!(
            child.process.reaped,
            "returned result must have reaped peer"
        );
        result
    }
    #[test]
    fn actual_private_pipe_exchange_and_terminal_failures() {
        EXIT_READ_RACE.set(true);
        assert_eq!(peer("valid").unwrap().product, "Test/1");
        EXIT_READ_RACE.set(false);
        for mode in [
            "early",
            "malformed",
            "hang",
            "shutdown-hang",
            "shutdown-error",
        ] {
            assert!(peer(mode).is_err(), "{mode}");
        }
    }
    #[test]
    fn deadlines_diagnostics_and_owned_process_reaping() {
        assert_eq!(remaining(now_ms().unwrap() - 1), Err(Error::Timeout));
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "exec sleep 60"]);
        parent_death(&mut c);
        assert_eq!(
            capture_command(&mut c, now_ms().unwrap() + 150, 1024),
            Err(Error::Timeout)
        );
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "printf 123456789"]);
        assert!(capture_command(&mut c, now_ms().unwrap() + 2000, 4).is_err());
    }
    #[test]
    fn parent_death_entry() {
        let Ok(path) = std::env::var("PREP_DEATH_PATH") else {
            return;
        };
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "exec sleep 60"]);
        parent_death(&mut c);
        let child = OwnedChild::spawn(&mut c).unwrap();
        std::fs::write(path, child.process.child.id().to_string()).unwrap();
        // Deliberately bypass Drop to test the kernel parent-death path.
        unsafe { libc::_exit(0) };
    }
    #[test]
    fn parent_death_terminates_descendant() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("pid");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "probe_linux::tests::parent_death_entry",
                "--nocapture",
            ])
            .env("PREP_DEATH_PATH", &path)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let pid = std::fs::read_to_string(path)
            .unwrap()
            .parse::<u32>()
            .unwrap();
        if let Ok(fd) = pidfd(pid) {
            let mut p = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert!(unsafe { libc::poll(&mut p, 1, 2000) } > 0);
        }
    }
    #[test]
    #[ignore = "explicit rootless x86-64 user/net/PID/mount namespace host required; no browser"]
    fn namespace_prerequisites_runtime() {
        let mut c = Command::new("/usr/bin/unshare");
        c.args([
            "--user",
            "--map-current-user",
            "--net",
            "--pid",
            "--fork",
            "--mount",
            "--mount-proc",
            "--propagation",
            "private",
            "--kill-child=KILL",
            "--",
            "/bin/true",
        ]);
        assert!(capture_command(&mut c, now_ms().unwrap() + 5000, 4096).is_ok());
        // This smoke validates setup only. Full actual state checks remain mandatory in worker.
    }
    #[test]
    fn watchdog_expiry_entry() {
        if std::env::var_os("PREP_WATCHDOG_TEST").is_none() {
            return;
        }
        let _watchdog = Watchdog::arm(now_ms().unwrap() + 150).unwrap();
        std::thread::sleep(Duration::from_secs(5));
        panic!("watchdog did not terminate producer");
    }
    #[test]
    fn watchdog_completion_and_fail_stop() {
        Watchdog::arm(now_ms().unwrap() + 2000)
            .unwrap()
            .finish()
            .unwrap();
        use std::os::unix::process::ExitStatusExt;
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "probe_linux::tests::watchdog_expiry_entry",
                "--nocapture",
            ])
            .env("PREP_WATCHDOG_TEST", "1")
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL));
    }
    #[test]
    fn fresh_workspace_profile_and_closed_argv() {
        let a = fresh_workspace().unwrap();
        let b = fresh_workspace().unwrap();
        assert_ne!(a.path(), b.path());
        prepare_profile(a.path()).unwrap();
        prepare_profile(b.path()).unwrap();
        assert!(prepare_profile(a.path()).is_err());
        assert_eq!(
            std::fs::metadata(a.path().join("profile")).unwrap().mode() & 0o777,
            0o700
        );
        assert!(ARGUMENTS.contains(&"--remote-debugging-pipe"));
        assert!(ARGUMENTS.contains(&"--user-data-dir=profile"));
        assert!(
            !ARGUMENTS
                .iter()
                .any(|s| s.contains("debugging-port") || *s == "--no-sandbox")
        );
        a.close().unwrap();
        b.close().unwrap();
    }
}
