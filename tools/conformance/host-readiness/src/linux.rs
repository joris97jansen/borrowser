//! Narrow external kernel/util-linux proofs, never helper or collector algorithms.
//! Every public probe runs in a disposable process group under a bounded supervisor.
use crate::Sha256;
use crate::{Result, require};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crate::report::HostProbe;
pub use crate::report::{PROBE_AUTHORITY, Report};
const LIMIT: usize = crate::report::REPORT_BYTES;
const TIMEOUT: Duration = Duration::from_secs(15);
fn check(rc: i64) -> Result<i64> {
    if rc < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(rc)
    }
}
fn text(path: impl AsRef<Path>) -> Result<String> {
    let mut s = String::new();
    File::open(path)?
        .take((LIMIT + 1) as u64)
        .read_to_string(&mut s)?;
    require(s.len() <= LIMIT, "proc text limit")?;
    Ok(s)
}
fn field<'a>(s: &'a str, name: &str) -> Result<&'a str> {
    let prefix = format!("{name}:");
    let mut values = s.lines().filter_map(|l| l.strip_prefix(&prefix));
    let value = values.next().ok_or("missing process field")?.trim();
    require(values.next().is_none(), "duplicate process field")?;
    Ok(value)
}
fn no_host_privileges() -> Result<()> {
    require(
        unsafe {
            libc::getuid() != 0
                && libc::getgid() != 0
                && libc::getuid() == libc::geteuid()
                && libc::getgid() == libc::getegid()
        },
        "non-root real/effective identities",
    )?;
    let status = text("/proc/self/status")?;
    for name in ["CapEff", "CapPrm", "CapInh", "CapAmb"] {
        require(
            field(&status, name)? == "0000000000000000",
            "host execution capabilities",
        )?;
    }
    Ok(())
}
fn pidfd(pid: u32) -> Result<OwnedFd> {
    let fd = check(unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) })?;
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}
fn signal(fd: &OwnedFd, sig: i32) -> Result<()> {
    check(unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            sig,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    })?;
    Ok(())
}
fn pdeath(parent: libc::pid_t) -> std::io::Result<()> {
    if parent <= 0
        || unsafe { libc::getppid() } != parent
        || unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0
        || unsafe { libc::getppid() } != parent
    {
        return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
    }
    Ok(())
}
struct OwnedChild {
    child: Child,
    fd: Option<OwnedFd>,
    reaped: bool,
    group: bool,
}
impl OwnedChild {
    fn spawn(command: &mut Command, group: bool) -> Result<Self> {
        let parent = unsafe { libc::getpid() };
        unsafe {
            command.pre_exec(move || {
                if group && libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                pdeath(parent)
            });
        }
        let child = command.spawn()?;
        let mut owner = Self {
            child,
            fd: None,
            reaped: false,
            group,
        };
        match pidfd(owner.child.id()) {
            Ok(fd) => {
                owner.fd = Some(fd);
                Ok(owner)
            }
            Err(e) => {
                owner.cleanup()?;
                Err(e)
            }
        }
    }
    fn terminate(&self) -> Result<()> {
        // The direct child is still unreaped, so its PID/PGID cannot be recycled.
        let rc = if self.group {
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) }
        } else if let Some(fd) = &self.fd {
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    fd.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                ) as i32
            }
        } else {
            // Partial acquisition: the direct child is owned and unreaped.
            unsafe { libc::kill(self.child.id() as i32, libc::SIGKILL) }
        };
        if rc != 0 {
            require(
                std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
                "child termination",
            )?;
        }
        Ok(())
    }
    fn cleanup(&mut self) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        let killed = self.terminate();
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if self.child.try_wait()?.is_some() {
                self.reaped = true;
                return killed;
            }
            require(Instant::now() < end, "bounded child cleanup failed")?;
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn finish(&mut self, end: Instant) -> Result<()> {
        loop {
            let fd = self.fd.as_ref().ok_or("child pidfd")?;
            let mut p = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut p, 1, 0) };
            if ready < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(std::io::Error::last_os_error().into());
            }
            if ready > 0 {
                if self.group {
                    self.terminate()?;
                }
                if let Some(status) = self.child.try_wait()? {
                    self.reaped = true;
                    require(status.success(), "probe child failed")?;
                    return Ok(());
                }
            }
            require(Instant::now() < end, "child deadline")?;
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.terminate();
            let _ = self.child.try_wait();
        }
    }
}
fn capture(command: &mut Command, group: bool, end: Instant) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owner = OwnedChild::spawn(command, group)?;
    let mut diagnostics = Vec::new();
    let work = (|| {
        let mut stdout = owner.child.stdout.take().ok_or("child stdout")?;
        let mut stderr = owner.child.stderr.take().ok_or("child stderr")?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            check(unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } as i64)?;
        }
        let mut bytes = Vec::new();
        let mut output_closed = false;
        let mut error_closed = false;
        while !output_closed || !error_closed {
            require(Instant::now() < end, "probe deadline")?;
            for (reader, target, closed) in [
                (&mut stdout as &mut dyn Read, &mut bytes, &mut output_closed),
                (
                    &mut stderr as &mut dyn Read,
                    &mut diagnostics,
                    &mut error_closed,
                ),
            ] {
                if *closed {
                    continue;
                }
                let mut buffer = [0u8; 4096];
                match reader.read(&mut buffer) {
                    Ok(0) => *closed = true,
                    Ok(n) => {
                        require(target.len() + n <= LIMIT, "probe output bound")?;
                        target.extend_from_slice(&buffer[..n]);
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        owner.finish(end)?;
        Ok(bytes)
    })();
    // Cleanup precedes diagnostic transport (which can block on the caller's pipe).
    owner.cleanup()?;
    if !diagnostics.is_empty() {
        std::io::stderr().write_all(&diagnostics)?;
    }
    work
}
/// Only the CLI supplies its own executable; no arbitrary worker program API.
pub fn run(args: &[String]) -> Result<Vec<u8>> {
    let requested = HostProbe::parse(args.first().ok_or("missing probe command")?)?;
    no_host_privileges()?;
    let mut c = Command::new(std::env::current_exe()?);
    c.arg("__host-driver")
        .args(args)
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C");
    unsafe {
        c.pre_exec(|| {
            if libc::syscall(
                libc::SYS_close_range,
                3u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let bytes = capture(&mut c, true, Instant::now() + TIMEOUT)?;
    Report::parse_expected(&bytes, requested)?;
    Ok(bytes)
}
pub fn driver(args: &[String]) -> Result<Vec<u8>> {
    no_host_privileges()?;
    match args.first().map(String::as_str) {
        Some("host-seccomp-prerequisite") if args.len() == 1 => seccomp(),
        Some("host-unshare-fd-prerequisite" | "host-process-inspection-prerequisite")
            if args.len() == 4 =>
        {
            unshare(&args[0], Path::new(&args[1]), &args[2], &args[3])
        }
        _ => Err("invalid host probe arguments".into()),
    }
}
fn seccomp() -> Result<Vec<u8>> {
    check(unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } as i64)?;
    require(
        unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } == 1,
        "NoNewPrivs readback",
    )?;
    // x86-64 audit arch check, then deny getppid only. Never Chromium's policy.
    let mut filter = [
        libc::sock_filter {
            code: 0x20,
            jt: 0,
            jf: 0,
            k: 4,
        },
        libc::sock_filter {
            code: 0x15,
            jt: 1,
            jf: 0,
            k: 0xc000003e,
        },
        libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: 0x80000000,
        },
        libc::sock_filter {
            code: 0x20,
            jt: 0,
            jf: 0,
            k: 0,
        },
        libc::sock_filter {
            code: 0x15,
            jt: 0,
            jf: 1,
            k: libc::SYS_getppid as u32,
        },
        libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: 0x00050000 | libc::EPERM as u32,
        },
        libc::sock_filter {
            code: 0x06,
            jt: 0,
            jf: 0,
            k: 0x7fff0000,
        },
    ];
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    require(
        unsafe { libc::syscall(libc::SYS_getppid) } > 0,
        "seccomp positive control",
    )?;
    check(unsafe { libc::syscall(libc::SYS_seccomp, 1, 0, &program) })?;
    require(
        unsafe { libc::syscall(libc::SYS_getppid) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM),
        "seccomp enforcement",
    )?;
    let status = text("/proc/self/status")?;
    require(
        field(&status, "NoNewPrivs")? == "1" && field(&status, "Seccomp")? == "2",
        "seccomp state",
    )?;
    let mut r = Report::new(HostProbe::Seccomp);
    r.add("status", status);
    r.add("denied-syscall", "getppid:EPERM");
    r.bytes()
}
fn regular(path: &Path) -> Result<File> {
    let f = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let m = f.metadata()?;
    require(
        m.is_file() && m.mode() & 0o6022 == 0 && [0, unsafe { libc::getuid() }].contains(&m.uid()),
        "trusted regular executable",
    )?;
    let rc = unsafe {
        libc::fgetxattr(
            f.as_raw_fd(),
            c"security.capability".as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };
    require(
        rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODATA),
        "executable capability absence",
    )?;
    Ok(f)
}
fn ns() -> Result<Vec<u64>> {
    ["user", "pid", "net", "mnt"]
        .iter()
        .map(|n| Ok(std::fs::metadata(format!("/proc/self/ns/{n}"))?.ino()))
        .collect()
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bridge {
    mode: String,
    uid: u32,
    gid: u32,
    namespaces: Vec<u64>,
    proc_dev: u64,
    fd: i32,
    dev: u64,
    ino: u64,
    unshare_sha256: String,
    unshare_version: String,
}
fn unshare(mode: &str, path: &Path, digest: &str, version: &str) -> Result<Vec<u8>> {
    require(path.is_absolute(), "explicit util-linux path")?;
    crate::evidence::hex(digest, 64)?;
    crate::evidence::description(version)?;
    let mut executable = regular(path)?;
    let metadata = executable.metadata()?;
    require(
        metadata.len() <= 64 * 1024 * 1024,
        "util-linux executable bound",
    )?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut total = 0u64;
    loop {
        let n = executable.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        require(total <= metadata.len(), "util-linux changed")?;
        hash.update(&buffer[..n]);
    }
    let after = executable.metadata()?;
    require(
        total == metadata.len()
            && metadata.ino() == after.ino()
            && metadata.mtime() == after.mtime()
            && metadata.ctime() == after.ctime()
            && metadata.len() == after.len()
            && metadata.mode() == after.mode()
            && metadata.uid() == after.uid()
            && metadata.gid() == after.gid()
            && metadata.nlink() == after.nlink()
            && metadata.mtime_nsec() == after.mtime_nsec()
            && metadata.ctime_nsec() == after.ctime_nsec()
            && format!("{:x}", hash.finalize()) == digest,
        "util-linux identity",
    )?;
    let exec_path = format!("/proc/self/fd/{}", executable.as_raw_fd());
    let bytes = capture(
        Command::new(&exec_path).arg("--version"),
        false,
        Instant::now() + Duration::from_secs(2),
    )?;
    require(
        bytes == format!("{version}\n").as_bytes(),
        "util-linux version",
    )?;
    // Deliberate host fixture descriptor, not qualification-prep's WorkerExecutable.
    let own = File::open("/proc/self/exe")?;
    let fd =
        check(unsafe { libc::fcntl(own.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) } as i64)? as i32;
    let retained = unsafe { OwnedFd::from_raw_fd(fd) };
    let m = own.metadata()?;
    let input = Bridge {
        mode: mode.into(),
        uid: unsafe { libc::getuid() },
        gid: unsafe { libc::getgid() },
        namespaces: ns()?,
        proc_dev: std::fs::metadata("/proc")?.dev(),
        fd,
        dev: m.dev(),
        ino: m.ino(),
        unshare_sha256: digest.into(),
        unshare_version: version.into(),
    };
    let unrelated = File::open("/dev/null")?;
    let other = unrelated.as_raw_fd();
    let raw = serde_json::to_string(&input)?;
    let mut c = Command::new(exec_path);
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
    ])
    .arg(format!("/proc/self/fd/{fd}"))
    .arg("__host-namespace")
    .arg(raw)
    .env_clear()
    .env("LANG", "C")
    .env("LC_ALL", "C");
    unsafe {
        c.pre_exec(move || {
            if libc::fcntl(other, libc::F_SETFD, 0) < 0
                || libc::syscall(
                    libc::SYS_close_range,
                    3u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) < 0
                || libc::fcntl(fd, libc::F_SETFD, 0) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let result = capture(&mut c, false, Instant::now() + Duration::from_secs(10));
    drop(retained);
    result
}
fn fds(expected: i32) -> Result<()> {
    // /proc enumeration opens one transient directory FD; inspect after it closes.
    let mut ids = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd")? {
        require(ids.len() < 256, "descriptor population")?;
        ids.push(entry?.file_name().to_string_lossy().parse::<i32>()?);
    }
    for fd in ids {
        if fd > 2 && fd != expected {
            require(
                unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1,
                "unexpected inherited descriptor",
            )?;
        }
    }
    require(
        unsafe { libc::fcntl(expected, libc::F_GETFD) } == 0,
        "deliberate descriptor inheritance",
    )
}
fn network(report: &mut Report) -> Result<()> {
    let dev = text("/proc/net/dev")?;
    require(
        dev.lines()
            .skip(2)
            .map(|l| l.split(':').next().unwrap_or("").trim())
            .collect::<Vec<_>>()
            == ["lo"],
        "only loopback",
    )?;
    let fd = check(
        unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) } as i64,
    )? as i32;
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
    req.ifr_name[0] = b'l' as i8;
    req.ifr_name[1] = b'o' as i8;
    check(unsafe { libc::ioctl(socket.as_raw_fd(), libc::SIOCGIFFLAGS, &mut req) } as i64)?;
    require(
        unsafe { req.ifr_ifru.ifru_flags } & libc::IFF_UP as i16 == 0,
        "loopback down",
    )?;
    let v4 = text("/proc/net/route")?;
    require(
        v4.lines().skip(1).all(|l| l.trim().is_empty()),
        "collector-compatible IPv4 route prerequisite",
    )?;
    let v6 = text("/proc/net/ipv6_route")?;
    for l in v6.lines() {
        let f = l.split_whitespace().collect::<Vec<_>>();
        require(f.len() == 10, "IPv6 fields")?;
        require(
            u32::from_str_radix(f[8], 16)? & libc::RTF_REJECT as u32 != 0,
            "collector-compatible IPv6 reject route prerequisite",
        )?;
    }
    for path in [
        "/proc/net/tcp",
        "/proc/net/tcp6",
        "/proc/net/udp",
        "/proc/net/udp6",
        "/proc/net/unix",
    ] {
        require(
            text(path)?.lines().skip(1).all(|l| l.trim().is_empty()),
            "unexpected network endpoint",
        )?;
    }
    // socketpair is descriptor IPC, not a listening socket or host forwarding.
    let (mut a, mut b) = std::os::unix::net::UnixStream::pair()?;
    a.set_write_timeout(Some(Duration::from_secs(1)))?;
    b.set_read_timeout(Some(Duration::from_secs(1)))?;
    a.write_all(b"host-ipc")?;
    let mut got = [0; 8];
    b.read_exact(&mut got)?;
    require(&got == b"host-ipc", "private IPC")?;
    report.add("interfaces", dev);
    report.add("ipv4-routes", v4);
    report.add("ipv6-routes", v6);
    report.add("loopback", "down");
    report.add("private-ipc", "socketpair exchange; no listener");
    Ok(())
}
pub fn namespace_worker(raw: &str) -> Result<Vec<u8>> {
    require(raw.len() <= 8192, "bridge input bound")?;
    let input: Bridge = serde_json::from_str(raw)?;
    require(
        input.namespaces.len() == 4 && input.fd >= 10 && unsafe { libc::getpid() } == 1,
        "host namespace worker",
    )?;
    require(
        unsafe { libc::getuid() } == input.uid && unsafe { libc::getgid() } == input.gid,
        "namespace identities",
    )?;
    no_host_privileges()?;
    fds(input.fd)?;
    let m = std::fs::metadata(format!("/proc/self/fd/{}", input.fd))?;
    let own = std::fs::metadata("/proc/self/exe")?;
    require(
        (m.dev(), m.ino()) == (input.dev, input.ino)
            && (own.dev(), own.ino()) == (input.dev, input.ino),
        "retained fixture executable identity",
    )?;
    check(unsafe { libc::fcntl(input.fd, libc::F_SETFD, libc::FD_CLOEXEC) } as i64)?;
    let actual = ns()?;
    require(
        actual.iter().zip(&input.namespaces).all(|(a, b)| a != b),
        "fresh namespaces",
    )?;
    let net = File::open("/proc/self/ns/net")?;
    let fd = check(unsafe { libc::ioctl(net.as_raw_fd(), 0xb701) } as i64)?;
    let owner = unsafe { File::from_raw_fd(fd as i32) };
    require(
        owner.metadata()?.ino() == actual[0],
        "network namespace ownership",
    )?;
    let mut r = Report::new(HostProbe::parse(&input.mode)?);
    for (name, id) in [("uid", input.uid), ("gid", input.gid)] {
        let map = text(format!("/proc/self/{name}_map"))?;
        require(
            map.split_whitespace().collect::<Vec<_>>()
                == [id.to_string(), id.to_string(), "1".into()],
            "single nonzero identity mapping",
        )?;
        r.add(&format!("{name}-map"), map);
    }
    require(
        text("/proc/self/setgroups")?.trim() == "deny",
        "setgroups deny",
    )?;
    let mut death = 0;
    check(unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut death) } as i64)?;
    require(death == libc::SIGKILL, "util-linux child death signal")?;
    require(
        std::fs::read_link("/proc/self")? == Path::new("1")
            && std::fs::metadata("/proc")?.dev() != input.proc_dev,
        "private procfs identity",
    )?;
    require(pids()? == [1], "private initial proc population")?;
    let mount = text("/proc/self/mountinfo")?;
    let mut count = 0;
    for line in mount.lines() {
        let (left, right) = line.split_once(" - ").ok_or("mountinfo framing")?;
        let f = left.split_whitespace().collect::<Vec<_>>();
        require(f.len() >= 6, "mountinfo fields")?;
        require(
            !f[6..]
                .iter()
                .any(|s| s.starts_with("shared:") || s.starts_with("master:")),
            "private mount propagation",
        )?;
        if right.split_whitespace().next() == Some("proc") {
            count += 1;
            require(f[3] == "/" && f[4] == "/proc", "no proc alias")?;
            for flag in ["rw", "nosuid", "nodev", "noexec"] {
                require(f[5].split(',').any(|s| s == flag), "private proc flags")?;
            }
        }
    }
    require(count == 1, "single procfs")?;
    r.add(
        "namespaces",
        format!("parent={:?}; child={actual:?}", input.namespaces),
    );
    r.add("mountinfo", mount);
    r.add("status", text("/proc/self/status")?);
    r.add("util-linux-sha256", input.unshare_sha256);
    r.add("util-linux-version", input.unshare_version);
    r.add(
        "descriptor-boundary",
        "retained executable; unrelated descriptors closed",
    );
    network(&mut r)?;
    if input.mode == "host-process-inspection-prerequisite" {
        process_inspection(&mut r)?;
    } else {
        require(
            input.mode == "host-unshare-fd-prerequisite",
            "namespace mode",
        )?;
    }
    r.bytes()
}
fn pids() -> Result<Vec<u32>> {
    let mut ids = Vec::new();
    let mut count = 0;
    for e in std::fs::read_dir("/proc")? {
        count += 1;
        require(count <= 4096, "proc population bound")?;
        if let Ok(id) = e?.file_name().to_string_lossy().parse() {
            ids.push(id);
        }
    }
    ids.sort();
    Ok(ids)
}
fn proc_start(s: &str) -> Result<u64> {
    Ok(s.rsplit_once(") ")
        .ok_or("proc stat")?
        .1
        .split_whitespace()
        .nth(19)
        .ok_or("proc start")?
        .parse()?)
}
fn process_inspection(report: &mut Report) -> Result<()> {
    // Exactly two owned children, no quiescent-population/convergence implementation.
    let spawn = || -> Result<OwnedChild> {
        let mut c = Command::new("/proc/self/exe");
        c.arg("__host-idle")
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        OwnedChild::spawn(&mut c, false)
    };
    // Linux PDEATHSIG follows the spawning thread's lifetime, not just its process.
    // Keep that thread alive until the main thread has checked/reaped its child.
    let (transfer, receive) = std::sync::mpsc::sync_channel(1);
    let (release, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || -> Result<()> {
        let acquired = spawn();
        let alive = acquired.is_ok();
        if let Err(error) = transfer.send(acquired) {
            if let Ok(mut child) = error.0 {
                child.cleanup()?;
            }
            return Err("process fixture transfer failed".into());
        }
        if alive {
            let _ = wait.recv();
        }
        Ok(())
    });
    let result = match receive.recv() {
        Ok(Ok(child)) => inspect_owned(report, child, spawn),
        Ok(Err(error)) => Err(error),
        Err(_) => Err("process fixture acquisition failed".into()),
    };
    let _ = release.send(());
    worker.join().map_err(|_| "worker thread panic")??;
    result
}
fn inspect_owned(
    report: &mut Report,
    mut child: OwnedChild,
    spawn: impl FnOnce() -> Result<OwnedChild>,
) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(4);
    let work = (|| {
        let pid = child.child.id();
        require(pids()?.contains(&pid), "thread-created child enumeration")?;
        let proc = File::open(format!("/proc/{pid}"))?;
        let path = format!("/proc/self/fd/{}", proc.as_raw_fd());
        let start = proc_start(&text(format!("{path}/stat"))?)?;
        signal(child.fd.as_ref().ok_or("retained pidfd")?, libc::SIGSTOP)?;
        loop {
            let s = text(format!("{path}/status"))?;
            if field(&s, "State")?.starts_with('T') {
                break;
            }
            require(Instant::now() < end, "stop readback deadline")?;
            std::thread::sleep(Duration::from_millis(2));
        }
        require(
            proc_start(&text(format!("{path}/stat"))?)? == start,
            "same retained start identity",
        )?;
        let exe = std::fs::metadata(format!("{path}/exe"))?;
        require(exe.is_file(), "process executable inspection")?;
        let mut second = spawn()?;
        let second_work: Result<()> = (|| {
            let id = second.child.id();
            require(pids()?.contains(&id), "appearing child")?;
            second.cleanup()?;
            require(!pids()?.contains(&id), "disappearing child")?;
            Ok(())
        })();
        second.cleanup()?;
        second_work?;
        child.cleanup()?;
        let mut poll = libc::pollfd {
            fd: child.fd.as_ref().unwrap().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        require(
            unsafe { libc::poll(&mut poll, 1, 0) } == 1,
            "exited retained pidfd",
        )?;
        require(text(format!("{path}/stat")).is_err(), "exited proc object")?;
        report.add("process-inspection","worker-thread child enumerated; stopped via pidfd; retained start/exe read; second child appeared/disappeared; children killed/reaped; old pidfd exited");
        Ok(())
    })();
    child.cleanup()?;
    work
}
pub fn idle() -> Result<()> {
    // The spawn hook bound PDEATHSIG to the known parent before exec.
    loop {
        unsafe {
            libc::pause();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_field_and_start_parsing() {
        assert!(field("CapEff: 0\nCapEff: 0\n", "CapEff").is_err());
        assert!(field("", "Seccomp").is_err());
        assert!(proc_start("bad").is_err());
        assert!(pdeath(0).is_err());
        assert!(pdeath(unsafe { libc::getpid() }).is_err());
    }
    #[test]
    #[ignore = "explicit Linux child lifecycle failures, no browser or namespace evidence"]
    fn supervisor_bounds_output_timeout_and_reaps_failure() {
        no_host_privileges().unwrap();
        let mut excessive = Command::new("/bin/sh");
        excessive.args(["-c", "while :; do printf 'excess-output'; done"]);
        assert!(
            capture(
                &mut excessive,
                true,
                Instant::now() + Duration::from_secs(2)
            )
            .is_err()
        );
        let mut sleeping = Command::new("/bin/sleep");
        sleeping.arg("30");
        assert!(
            capture(
                &mut sleeping,
                true,
                Instant::now() + Duration::from_millis(100)
            )
            .is_err()
        );
        let mut failure = Command::new("/bin/sh");
        failure.args(["-c", "exit 7"]);
        assert!(capture(&mut failure, true, Instant::now() + Duration::from_secs(2)).is_err());
        let mut missing = Command::new("/missing/child");
        assert!(OwnedChild::spawn(&mut missing, true).is_err());
    }
    #[test]
    #[ignore = "explicit Linux process-inspection returned-error cleanup"]
    fn process_inspection_second_acquisition_failure_reaps_stopped_child() {
        no_host_privileges().unwrap();
        let mut c = Command::new("/bin/sleep");
        c.arg("30");
        let child = OwnedChild::spawn(&mut c, false).unwrap();
        let pid = child.child.id();
        let retained = pidfd(pid).unwrap();
        let mut report = Report::new(HostProbe::ProcessInspection);
        let result = inspect_owned(&mut report, child, || {
            Err("injected second acquisition failure".into())
        });
        assert!(result.is_err());
        assert!(report.observations.is_empty());
        let mut p = libc::pollfd {
            fd: retained.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut p, 1, 0) }, 1);
        assert_eq!(
            unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
    }
    #[test]
    #[ignore = "explicit Linux inherited-descriptor failure and retained-object identity test"]
    fn descriptor_leak_is_rejected() {
        let own = File::open("/proc/self/exe").unwrap();
        let fd = unsafe { libc::fcntl(own.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        assert!(fd >= 10);
        let retained = unsafe { OwnedFd::from_raw_fd(fd) };
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFD, 0) }, 0);
        let leak = File::open("/dev/null").unwrap();
        assert!(fds(fd).is_err());
        drop(leak);
        drop(retained);
    }
    #[test]
    #[ignore = "explicit Linux seccomp error path in disposable test subprocess"]
    fn seccomp_install_failure_is_not_support() {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args([
            "--exact",
            "linux::tests::seccomp_failure_child",
            "--ignored",
            "--nocapture",
        ])
        .env("AG9G0A_TEST_SECCOMP_CHILD", "1");
        capture(&mut c, true, Instant::now() + Duration::from_secs(3)).unwrap();
    }
    #[test]
    #[ignore = "private test subprocess entry"]
    fn seccomp_failure_child() {
        assert_eq!(
            std::env::var("AG9G0A_TEST_SECCOMP_CHILD").as_deref(),
            Ok("1")
        );
        assert_eq!(
            unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) },
            0
        );
        let invalid = libc::sock_fprog {
            len: 0,
            filter: std::ptr::null_mut(),
        };
        assert!(check(unsafe { libc::syscall(libc::SYS_seccomp, 1, 0, &invalid) }).is_err());
    }
}
