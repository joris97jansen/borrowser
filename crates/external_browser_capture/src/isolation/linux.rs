//! A single-threaded fork supervisor owns a fresh user/net/PID namespace tree.
use crate::{
    CaptureError as E, Result,
    configuration::Configuration,
    deadline::AttemptDeadline,
    distribution::VerifiedDistribution,
    profile::{FreshProfile, ProfileIdentity},
};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{fs::MetadataExt, net::UnixStream},
    },
    path::Path,
    time::Instant,
};

pub(crate) fn prerequisites() -> Result<()> {
    // SAFETY: process queries have no pointer arguments.
    if unsafe { libc::getuid() } == 0 || unsafe { libc::geteuid() } == 0 {
        return Err(E::Isolation);
    }
    if std::fs::read_dir("/proc/self/task")
        .map_err(|_| E::Isolation)?
        .count()
        != 1
    {
        return Err(E::Isolation);
    }
    Ok(())
}
fn pidfd(pid: i32) -> Result<OwnedFd> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    if fd < 0 {
        return Err(E::ProcessIdentity);
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}
pub(super) fn signal(fd: &OwnedFd, sig: i32) -> Result<()> {
    if unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            sig,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    } < 0
    {
        return Err(E::Cleanup);
    }
    Ok(())
}
fn namespace(pid: i32, name: &str) -> Result<File> {
    File::open(format!("/proc/{pid}/ns/{name}")).map_err(|_| E::Isolation)
}
fn inode(f: &File) -> Result<u64> {
    Ok(f.metadata().map_err(|_| E::Isolation)?.ino())
}
fn process_start(pid: i32) -> Result<u64> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| E::ProcessIdentity)?;
    s.rsplit_once(") ")
        .ok_or(E::ProcessIdentity)?
        .1
        .split_whitespace()
        .nth(19)
        .ok_or(E::ProcessIdentity)?
        .parse()
        .map_err(|_| E::ProcessIdentity)
}
fn send(stream: &mut UnixStream, value: &str, deadline: AttemptDeadline) -> Result<()> {
    let end = deadline.command()?;
    stream
        .set_write_timeout(Some(end.saturating_duration_since(Instant::now())))
        .map_err(|_| E::Isolation)?;
    stream
        .write_all(value.as_bytes())
        .map_err(|_| E::Isolation)?;
    deadline.check()
}
fn line(stream: &mut UnixStream, deadline: AttemptDeadline) -> Result<String> {
    let end = deadline.command()?;
    let mut out = Vec::new();
    let mut one = [0];
    while out.len() < 256 {
        deadline.check()?;
        let left = end
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(E::Deadline)?;
        stream
            .set_read_timeout(Some(left))
            .map_err(|_| E::Isolation)?;
        stream.read_exact(&mut one).map_err(|_| E::Isolation)?;
        if one[0] == b'\n' {
            return String::from_utf8(out).map_err(|_| E::Isolation);
        }
        out.push(one[0]);
    }
    Err(E::Limit)
}
fn verify_empty_network() -> Result<()> {
    let interfaces = std::fs::read_to_string("/proc/net/dev").map_err(|_| E::Isolation)?;
    let names: Vec<_> = interfaces
        .lines()
        .skip(2)
        .map(|l| l.split(':').next().unwrap_or("").trim())
        .collect();
    if names != ["lo"] {
        return Err(E::Isolation);
    }
    // /sys may expose the host namespace. Use SIOCGIFFLAGS against this namespace instead.
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return Err(E::Isolation);
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
    req.ifr_name[0] = b'l' as i8;
    req.ifr_name[1] = b'o' as i8;
    if unsafe { libc::ioctl(fd.as_raw_fd(), libc::SIOCGIFFLAGS, &mut req) } < 0
        || unsafe { req.ifr_ifru.ifru_flags } & libc::IFF_UP as i16 != 0
    {
        return Err(E::Isolation);
    }
    if std::fs::read_to_string("/proc/net/route")
        .map_err(|_| E::Isolation)?
        .lines()
        .skip(1)
        .any(|l| !l.trim().is_empty())
    {
        return Err(E::Isolation);
    }
    let v6 = std::fs::read_to_string("/proc/net/ipv6_route").map_err(|_| E::Isolation)?;
    // A fresh namespace can have unreachable kernel reject routes; no usable route may exist.
    for l in v6.lines() {
        let fields: Vec<_> = l.split_whitespace().collect();
        if fields.len() != 10 {
            return Err(E::Isolation);
        }
        let flags = u32::from_str_radix(fields[8], 16).map_err(|_| E::Isolation)?;
        if flags & libc::RTF_REJECT as u32 == 0 {
            return Err(E::Isolation);
        }
    }
    Ok(())
}

pub(crate) struct IsolatedBrowser {
    outer: i32,
    outer_handle: OwnedFd,
    supervisor: OwnedFd,
    _browser: OwnedFd,
    supervisor_pid: i32,
    browser_pid: i32,
    start: u64,
    network: File,
    user: File,
    pids: File,
    control: UnixStream,
    transport: Option<UnixStream>,
    workspace: Option<tempfile::TempDir>,
    finished: bool,
    profile: FreshProfile,
    deadline: AttemptDeadline,
}
// This scope owns the private workspace until fork establishes process ownership.
// No fallible operation follows successful fork here; all later cleanup belongs
// to IsolatedBrowser's supervisor/pidfd lifecycle.
struct PreparedLaunch {
    workspace: tempfile::TempDir,
    control: UnixStream,
    child_control: UnixStream,
    transport: UnixStream,
    child_transport: UnixStream,
    parent_network: File,
    outer: i32,
}
fn prepare_launch() -> Result<PreparedLaunch> {
    let workspace = tempfile::tempdir().map_err(|_| E::Launch)?;
    let prepared = (|| {
        #[cfg(test)]
        prelaunch_cleanup_tests::checkpoint("socket", workspace.path())?;
        let (control, child_control) = UnixStream::pair().map_err(|_| E::Launch)?;
        let (transport, child_transport) = UnixStream::pair().map_err(|_| E::Launch)?;
        #[cfg(test)]
        prelaunch_cleanup_tests::checkpoint("namespace", workspace.path())?;
        let parent_network = namespace(unsafe { libc::getpid() }, "net")?;
        #[cfg(test)]
        prelaunch_cleanup_tests::checkpoint("fork", workspace.path())?;
        // The production caller verifies single-threaded prerequisites before entry.
        let outer = unsafe { libc::fork() };
        if outer < 0 {
            return Err(E::Launch);
        }
        Ok((
            control,
            child_control,
            transport,
            child_transport,
            parent_network,
            outer,
        ))
    })();
    match prepared {
        Ok((control, child_control, transport, child_transport, parent_network, outer)) => {
            Ok(PreparedLaunch {
                workspace,
                control,
                child_control,
                transport,
                child_transport,
                parent_network,
                outer,
            })
        }
        Err(error) => {
            let cleanup = workspace.close().map_err(|_| E::Cleanup);
            #[cfg(test)]
            let cleanup = prelaunch_cleanup_tests::closed(cleanup);
            cleanup.and(Err(error))
        }
    }
}

impl IsolatedBrowser {
    pub fn launch(
        c: &Configuration,
        distribution: &VerifiedDistribution,
        deadline: AttemptDeadline,
    ) -> Result<Self> {
        deadline.check()?;
        prerequisites()?;
        let PreparedLaunch {
            workspace,
            mut control,
            child_control,
            transport,
            child_transport,
            parent_network,
            outer,
        } = prepare_launch()?;
        if outer == 0 {
            drop(control);
            drop(transport);
            let result = supervise(
                child_control,
                child_transport,
                c,
                distribution,
                workspace.path(),
                deadline,
            );
            unsafe { libc::_exit(if result.is_ok() { 0 } else { 125 }) }
        }
        drop(child_control);
        drop(child_transport);
        let outer_handle = match pidfd(outer) {
            Ok(fd) => fd,
            Err(_) => {
                control.shutdown(std::net::Shutdown::Both).ok();
                fail_stop_at(deadline);
            }
        };
        let result = (|| {
            let frozen = line(&mut control, deadline)?;
            let fields = frozen.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 4 || fields[0] != "FROZEN" {
                return Err(E::ProcessIdentity);
            }
            let expected = (
                fields[1].parse::<u64>().map_err(|_| E::ProcessIdentity)?,
                fields[2].parse::<u64>().map_err(|_| E::ProcessIdentity)?,
            );
            let count = fields[3].parse::<usize>().map_err(|_| E::Limit)?;
            if count == 0 || count > 4096 {
                return Err(E::Limit);
            }
            let mut allowed_executables: Vec<(u64, u64)> = Vec::new();
            for _ in 0..count {
                let v = line(&mut control, deadline)?;
                let (a, b) = v.split_once(' ').ok_or(E::ProcessIdentity)?;
                allowed_executables.push((
                    a.parse().map_err(|_| E::ProcessIdentity)?,
                    b.parse().map_err(|_| E::ProcessIdentity)?,
                ));
            }
            let ready = line(&mut control, deadline)?;
            let supervisor_pid = ready
                .strip_prefix("READY ")
                .ok_or(E::Isolation)?
                .parse::<i32>()
                .map_err(|_| E::Isolation)?;
            let supervisor = pidfd(supervisor_pid)?;
            let network = namespace(supervisor_pid, "net")?;
            let user = namespace(supervisor_pid, "user")?;
            let pids = namespace(supervisor_pid, "pid")?;
            if inode(&network)? == inode(&parent_network)? {
                return Err(E::Isolation);
            }
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };
            for (name, value) in [("uid_map", uid), ("gid_map", gid)] {
                let actual = std::fs::read_to_string(format!("/proc/{supervisor_pid}/{name}"))
                    .map_err(|_| E::Isolation)?;
                let fields: Vec<_> = actual.split_whitespace().collect();
                if fields != [value.to_string(), value.to_string(), "1".into()] {
                    return Err(E::Isolation);
                }
            }
            // NS_GET_USERNS proves ownership of the network namespace, not just a differing inode.
            let owner = unsafe { libc::ioctl(network.as_raw_fd(), 0xb701) };
            if owner < 0 {
                return Err(E::Isolation);
            }
            let owner = unsafe { File::from_raw_fd(owner) };
            let status = std::fs::read_to_string(format!("/proc/{supervisor_pid}/status"))
                .map_err(|_| E::Isolation)?;
            let inner_pid = status
                .lines()
                .find(|l| l.starts_with("NSpid:"))
                .and_then(|l| l.split_whitespace().last())
                .ok_or(E::Isolation)?;
            super::verification::namespace_binding(
                inode(&parent_network)?,
                inode(&network)?,
                inode(&user)?,
                inode(&owner)?,
                inner_pid,
            )?;
            send(&mut control, "LAUNCH\n", deadline)?;
            let browser_pid = line(&mut control, deadline)?
                .strip_prefix("BROWSER ")
                .ok_or(E::Launch)?
                .parse::<i32>()
                .map_err(|_| E::Launch)?;
            if line(&mut control, deadline)? != "EXECUTED" {
                return Err(E::Launch);
            }
            #[cfg(test)]
            if TEST_HANDSHAKE_FAILURE.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(E::ProcessIdentity);
            }
            let browser = pidfd(browser_pid)?;
            let profile_line = line(&mut control, deadline)?;
            let fields: Vec<_> = profile_line.split_whitespace().collect();
            if fields.len() != 3 || fields[0] != "PROFILE" {
                return Err(E::ProcessIdentity);
            }
            let expected_profile = ProfileIdentity {
                device: fields[1].parse().map_err(|_| E::ProcessIdentity)?,
                inode: fields[2].parse().map_err(|_| E::ProcessIdentity)?,
            };
            let profile = FreshProfile::retain(
                File::open(format!(
                    "/proc/{supervisor_pid}/root/tmp/ag9g/workspace/profile"
                ))
                .map_err(|_| E::ProcessIdentity)?,
            )?;
            if profile.identity() != expected_profile {
                return Err(E::ProcessIdentity);
            }
            let start = process_start(browser_pid)?;
            if inode(&namespace(browser_pid, "net")?)? != inode(&network)?
                || inode(&namespace(browser_pid, "user")?)? != inode(&user)?
            {
                return Err(E::Isolation);
            }
            let actual = std::fs::metadata(format!("/proc/{browser_pid}/exe"))
                .map_err(|_| E::ProcessIdentity)?;
            if (actual.dev(), actual.ino()) != expected {
                return Err(E::ProcessIdentity);
            }
            Ok((
                supervisor,
                browser,
                supervisor_pid,
                browser_pid,
                start,
                network,
                user,
                pids,
                profile,
            ))
        })();
        match result {
            Ok((
                supervisor,
                browser,
                supervisor_pid,
                browser_pid,
                start,
                network,
                user,
                pids,
                profile,
            )) => Ok(Self {
                outer,
                outer_handle,
                supervisor,
                _browser: browser,
                supervisor_pid,
                browser_pid,
                start,
                network,
                user,
                pids,
                control,
                transport: Some(transport),
                workspace: Some(workspace),
                finished: false,
                profile,
                deadline,
            }),
            Err(e) => {
                let cleanup = (|| {
                    // EOF drives the supervisor's normal unwind. It stays alive
                    // to reap PID 1; never kill that reaper before its child exits.
                    control
                        .shutdown(std::net::Shutdown::Both)
                        .map_err(|_| E::Cleanup)?;
                    let status = wait_child(outer, deadline)?;
                    if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
                        return Err(E::Cleanup);
                    }
                    workspace.close().map_err(|_| E::Cleanup)?;
                    deadline.check()
                })()
                .map_err(|_| E::Cleanup);
                if cleanup.is_err() {
                    fail_stop_at(deadline);
                }
                Err(e)
            }
        }
    }
    pub fn transport(&mut self) -> Result<UnixStream> {
        self.transport.take().ok_or(E::Launch)
    }
    pub fn profile_identity(&self) -> ProfileIdentity {
        self.profile.identity()
    }
    pub fn verify_live(&self) -> Result<()> {
        self.deadline.check()?;
        let profile = File::open(format!(
            "/proc/{}/root/tmp/ag9g/workspace/profile",
            self.browser_pid
        ))
        .map_err(|_| E::ProcessIdentity)?;
        self.profile.verify_object(&profile, self.deadline)?;
        if process_start(self.browser_pid)? != self.start
            || inode(&namespace(self.browser_pid, "net")?)? != inode(&self.network)?
        {
            return Err(E::ProcessIdentity);
        }
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.browser_pid))
            .map_err(|_| E::ProcessIdentity)?;
        super::verification::cleared_capabilities(&status)?;
        if inode(&namespace(self.supervisor_pid, "user")?)? != inode(&self.user)?
            || inode(&namespace(self.supervisor_pid, "pid")?)? != inode(&self.pids)?
        {
            return Err(E::Isolation);
        }
        let _ = &self.workspace;
        Ok(())
    }
    #[cfg(test)]
    pub fn finish(self) -> Result<()> {
        self.finish_with_event_check(|| Ok(()))
    }
    pub(crate) fn finish_with_event_check(
        mut self,
        check_events: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let verification = (|| {
            send(&mut self.control, "VERIFY\n", self.deadline)?;
            if line(&mut self.control, self.deadline)? != "VERIFIED" {
                return Err(E::ProcessIdentity);
            }
            self.verify_live()?;
            check_events()
        })();
        // All paths use the same terminal cleanup. Stopped processes are never
        // resumed: killing namespace PID 1 tears down the complete population.
        let cleanup = self.cleanup();
        cleanup.and(verification)
    }
    pub fn abort(mut self) -> Result<()> {
        self.cleanup()
    }
    fn cleanup(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        // Init exit invokes the kernel's namespace teardown/reap. Keep the outer
        // alive so it waits for init, then reap the collector's owned outer.
        if terminate(&self.supervisor).is_err() {
            fail_stop_at(self.deadline);
        }
        let status = match wait_child(self.outer, self.deadline) {
            Ok(status) => status,
            Err(_) => fail_stop_at(self.deadline),
        };
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            fail_stop_at(self.deadline);
        }
        self.finished = true;
        self.workspace
            .take()
            .ok_or(E::Cleanup)?
            .close()
            .map_err(|_| E::Cleanup)?;
        self.deadline.check().map_err(|_| E::Cleanup)
    }
}
impl Drop for IsolatedBrowser {
    fn drop(&mut self) {
        if !self.finished {
            let _ = signal(&self.supervisor, libc::SIGKILL);
            unsafe {
                let _ = terminate(&self.outer_handle);
                libc::waitpid(self.outer, std::ptr::null_mut(), libc::WNOHANG);
            }
        }
    }
}

fn supervise(
    mut control: UnixStream,
    transport: UnixStream,
    c: &Configuration,
    distribution: &VerifiedDistribution,
    _workspace: &Path,
    deadline: AttemptDeadline,
) -> Result<()> {
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let parent = unsafe { libc::getppid() };
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } < 0 {
        return Err(E::Isolation);
    }
    if unsafe { libc::unshare(libc::CLONE_NEWUSER) } != 0 {
        return Err(E::Isolation);
    }
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } < 0
        || unsafe { libc::getppid() } != parent
    {
        return Err(E::Isolation);
    }
    std::fs::write("/proc/self/uid_map", format!("{uid} {uid} 1\n")).map_err(|_| E::Isolation)?;
    std::fs::write("/proc/self/setgroups", "deny\n").map_err(|_| E::Isolation)?;
    std::fs::write("/proc/self/gid_map", format!("{gid} {gid} 1\n")).map_err(|_| E::Isolation)?;
    if unsafe { libc::unshare(libc::CLONE_NEWNET | libc::CLONE_NEWPID) } != 0 {
        return Err(E::Isolation);
    }
    verify_empty_network()?;
    deadline.check()?;
    crate::linux_mount::private_namespace()?;
    crate::linux_mount::tmpfs(Path::new("/tmp"))?;
    let frozen = distribution.freeze(deadline)?;
    let private_workspace = crate::linux_mount::PrivateWorkspace::create()?;
    let profile = FreshProfile::create(&private_workspace, deadline)?;
    let workspace = Path::new("/tmp/ag9g/workspace");
    std::fs::create_dir("/tmp/ag9g/scratch").map_err(|_| E::Isolation)?;
    crate::linux_mount::tmpfs(Path::new("/tmp/ag9g/scratch"))?;
    let identity = frozen
        .executable
        .metadata()
        .map_err(|_| E::ProcessIdentity)?;
    send(
        &mut control,
        &format!(
            "FROZEN {} {} {}\n",
            identity.dev(),
            identity.ino(),
            frozen.identities.len()
        ),
        deadline,
    )?;
    for (dev, ino) in &frozen.identities {
        send(&mut control, &format!("{dev} {ino}\n"), deadline)?;
    }

    let outer_guard = pidfd(unsafe { libc::getpid() })?;
    let init = unsafe { libc::fork() };
    if init < 0 {
        return Err(E::Isolation);
    }
    if init > 0 {
        drop(control);
        drop(transport);
        // Init is deliberately killed on terminal cleanup, including success.
        // Its completed wait proves PID namespace teardown finished.
        let _status = wait_child(init, deadline)?;
        return Ok(());
    }
    let result: Result<()> = (|| {
        if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } < 0
            || unsafe { libc::getpid() } != 1
        {
            return Err(E::Isolation);
        }
        let mut parent_state = libc::pollfd {
            fd: outer_guard.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut parent_state, 1, 0) } != 0 {
            return Err(E::Isolation);
        }
        let host_proc = super::procfs::HostProc::retain(deadline)?;
        let private_proc = host_proc.cover(deadline)?;
        crate::linux_mount::readonly(Path::new("/tmp"))?;
        send(
            &mut control,
            &format!("READY {}\n", host_proc.pid()?),
            deadline,
        )?;
        if line(&mut control, deadline)? != "LAUNCH" {
            return Err(E::Isolation);
        }
        let browser = launch_object(
            host_proc,
            &frozen.executable,
            &frozen.path,
            workspace,
            &c.invocation_arguments,
            &mut control,
            &transport,
            deadline,
        )?;
        drop(transport);
        send(&mut control, "EXECUTED\n", deadline)?;
        send(
            &mut control,
            &format!(
                "PROFILE {} {}\n",
                profile.identity().device,
                profile.identity().inode
            ),
            deadline,
        )?;
        if line(&mut control, deadline)? != "VERIFY" {
            return Err(E::Cleanup);
        }
        let _population = super::population::verify(
            &private_proc,
            &frozen.identities,
            (browser, (identity.dev(), identity.ino())),
            deadline,
        )?;
        profile.verify_object(
            &File::open(workspace.join("profile")).map_err(|_| E::ProcessIdentity)?,
            deadline,
        )?;
        send(&mut control, "VERIFIED\n", deadline)?;
        // Parent terminates namespace PID 1 through its pidfd after checking
        // the frozen population. Never resume or accept another command.
        let _ = line(&mut control, deadline)?;
        Err(E::Cleanup)
    })();
    unsafe { libc::_exit(if result.is_ok() { 0 } else { 125 }) }
}

fn wait_child(pid: i32, deadline: AttemptDeadline) -> Result<i32> {
    deadline.check()?;
    let fd = pidfd(pid)?;
    let mut p = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = deadline.remaining()?.as_millis().min(i32::MAX as u128) as i32;
    if unsafe { libc::poll(&mut p, 1, ms) } != 1 {
        return Err(E::Deadline);
    }
    deadline.check()?;
    let mut status = 0;
    if unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } != pid {
        return Err(E::Cleanup);
    }
    Ok(status)
}
#[allow(clippy::too_many_arguments)]
fn launch_object(
    host_proc: super::procfs::HostProc,
    executable: &File,
    path: &Path,
    workspace: &Path,
    args: &[String],
    control: &mut UnixStream,
    transport: &UnixStream,
    deadline: AttemptDeadline,
) -> Result<i32> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    deadline.check()?;
    let mut argv = vec![CString::new(path.as_os_str().as_bytes()).map_err(|_| E::Path)?];
    for arg in args {
        argv.push(CString::new(arg.as_str()).map_err(|_| E::Arguments)?);
    }
    let mut av: Vec<_> = argv.iter().map(|s| s.as_ptr()).collect();
    av.push(std::ptr::null());
    let env = [
        c"LANG=C.UTF-8".as_ptr(),
        c"LC_ALL=C.UTF-8".as_ptr(),
        c"TMPDIR=/tmp/ag9g/scratch".as_ptr(),
        std::ptr::null(),
    ];
    let cwd = CString::new(workspace.as_os_str().as_bytes()).map_err(|_| E::Path)?;
    let (mut status, mut child_status) = UnixStream::pair().map_err(|_| E::Launch)?;
    let child = unsafe { libc::fork() };
    if child < 0 {
        return Err(E::Launch);
    }
    if child == 0 {
        drop(status);
        let result = (|| -> Result<()> {
            send(
                control,
                &format!("BROWSER {}\n", host_proc.pid()?),
                deadline,
            )?;
            drop(host_proc);
            // Parent closes its host-proc capability before releasing exec.
            if line(&mut child_status, deadline)? != "EXEC" {
                return Err(E::Isolation);
            }
            let exec = unsafe { libc::fcntl(executable.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
            let pipe = unsafe { libc::fcntl(transport.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
            if exec < 0 || pipe < 0 {
                return Err(E::Launch);
            }
            let null = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
            if null < 0 {
                return Err(E::Launch);
            }
            for fd in 0..3 {
                if unsafe { libc::dup2(null, fd) } < 0 {
                    return Err(E::Launch);
                }
            }
            if unsafe { libc::dup2(pipe, 3) } < 0
                || unsafe { libc::dup2(pipe, 4) } < 0
                || unsafe { libc::chdir(cwd.as_ptr()) } != 0
            {
                return Err(E::Launch);
            }
            if unsafe {
                libc::syscall(
                    libc::SYS_close_range,
                    5u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                )
            } < 0
            {
                return Err(E::Launch);
            }
            deadline.check()?;
            clear_exec_privileges()?;
            unsafe { exec_object(exec, av.as_ptr(), env.as_ptr()) }
        })();
        if result.is_err() {
            let _ = unsafe { libc::write(child_status.as_raw_fd(), b"!".as_ptr().cast(), 1) };
        }
        unsafe { libc::_exit(125) }
    }
    drop(child_status);
    drop(host_proc);
    send(&mut status, "EXEC\n", deadline)?;
    status
        .set_read_timeout(Some(deadline.remaining()?))
        .map_err(|_| E::Launch)?;
    let mut byte = [0];
    if status.read(&mut byte).map_err(|_| E::Launch)? != 0 {
        return Err(E::Launch);
    }
    deadline.check()?;
    Ok(child)
}

/// A kernel-timed fail-stop guard also covers synchronous filesystem/kernel calls
/// that cannot take a userspace timeout. It never extends the attempt budget.
pub(crate) struct AttemptWatchdog {
    stop: Option<UnixStream>,
    pid: i32,
    deadline: AttemptDeadline,
}
impl AttemptWatchdog {
    pub(crate) fn arm(deadline: AttemptDeadline) -> Result<Self> {
        prerequisites()?;
        deadline.check()?;
        let parent = pidfd(unsafe { libc::getpid() })?;
        let (read, write) = UnixStream::pair().map_err(|_| E::Launch)?;
        let child = unsafe { libc::fork() };
        if child < 0 {
            return Err(E::Launch);
        }
        if child == 0 {
            drop(write);
            let mut p = libc::pollfd {
                fd: read.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                let ms = deadline
                    .instant()
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(i32::MAX as u128) as i32;
                if ms <= 0 {
                    let _ = signal(&parent, libc::SIGKILL);
                    unsafe { libc::_exit(124) }
                }
                let result = unsafe { libc::poll(&mut p, 1, ms) };
                if result > 0 {
                    unsafe { libc::_exit(0) }
                }
                if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
                {
                    let _ = signal(&parent, libc::SIGKILL);
                    unsafe { libc::_exit(124) }
                }
            }
        }
        drop(read);
        Ok(Self {
            stop: Some(write),
            pid: child,
            deadline,
        })
    }
    pub(crate) fn finish(mut self) -> Result<()> {
        if self.deadline.check().is_err() {
            terminal_fail_stop();
        }
        self.stop.take();
        let status = match wait_child(self.pid, self.deadline) {
            Ok(status) => status,
            Err(_) => terminal_fail_stop(),
        };
        self.pid = 0;
        if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
            return Err(E::Cleanup);
        }
        self.deadline.check()
    }
}
impl Drop for AttemptWatchdog {
    fn drop(&mut self) {
        self.stop.take();
        if self.pid > 0 {
            unsafe {
                libc::waitpid(self.pid, std::ptr::null_mut(), libc::WNOHANG);
            }
        }
    }
}

pub(super) fn clear_exec_privileges() -> Result<()> {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = Header {
        version: 0x20080522,
        pid: 0,
    };
    let data = [Data {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe {
        libc::prctl(
            libc::PR_CAP_AMBIENT,
            libc::PR_CAP_AMBIENT_CLEAR_ALL,
            0,
            0,
            0,
        )
    } != 0
        || unsafe { libc::syscall(libc::SYS_capset, &header, data.as_ptr()) } != 0
    {
        return Err(E::Sandbox);
    }
    Ok(())
}

// SAFETY: caller supplies live, NUL-terminated argv/envp arrays; FD is the retained verified ELF object.
unsafe fn exec_object(
    fd: i32,
    argv: *const *const libc::c_char,
    envp: *const *const libc::c_char,
) -> Result<()> {
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            fd,
            c"".as_ptr(),
            argv,
            envp,
            libc::AT_EMPTY_PATH,
        )
    };
    Err(E::Launch)
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    #[test]
    fn expired_launch_handshakes_and_reap_do_not_block() {
        let (mut a, _b) = UnixStream::pair().unwrap();
        let d = AttemptDeadline::expired();
        assert_eq!(line(&mut a, d), Err(E::Deadline));
        assert_eq!(send(&mut a, "LAUNCH\n", d), Err(E::Deadline));
        assert_eq!(wait_child(-1, d), Err(E::Deadline));
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod object_exec_tests {
    use super::*;
    #[test]
    fn exec_uses_retained_object_after_executable_path_replacement() {
        use std::os::unix::fs::PermissionsExt;
        // Minimal test-only Linux ELF: exit(0). This is not a browser or qualification vector.
        fn elf(exit: u8) -> Vec<u8> {
            let mut b = vec![0u8; 132];
            b[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            for (o, v) in [(16, 2u16), (18, 62), (52, 64), (54, 56), (56, 1)] {
                b[o..o + 2].copy_from_slice(&v.to_le_bytes());
            }
            b[20..24].copy_from_slice(&1u32.to_le_bytes());
            for (o, v) in [
                (24, 0x400078u64),
                (32, 64),
                (80, 0x400000),
                (88, 0x400000),
                (96, 132),
                (104, 132),
                (112, 4096),
            ] {
                b[o..o + 8].copy_from_slice(&v.to_le_bytes());
            }
            b[64..68].copy_from_slice(&1u32.to_le_bytes());
            b[68..72].copy_from_slice(&5u32.to_le_bytes());
            b[120..].copy_from_slice(&[0xb8, 60, 0, 0, 0, 0xbf, exit, 0, 0, 0, 0x0f, 0x05]);
            b
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exe");
        std::fs::write(&path, elf(0)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let object = File::open(&path).unwrap();
        std::fs::rename(&path, dir.path().join("verified")).unwrap();
        std::fs::write(&path, elf(42)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let args = [c"retained-test-object".as_ptr(), std::ptr::null()];
        let env = [std::ptr::null()];
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            unsafe {
                let _ = exec_object(object.as_raw_fd(), args.as_ptr(), env.as_ptr());
                libc::_exit(125);
            }
        }
        let status = wait_child(child, AttemptDeadline::new()).unwrap();
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), 0);
    }
}

fn terminal_fail_stop() -> ! {
    unsafe {
        libc::kill(libc::getpid(), libc::SIGKILL);
        libc::_exit(124)
    }
}
fn terminate(fd: &OwnedFd) -> Result<()> {
    match signal(fd, libc::SIGKILL) {
        Ok(()) => Ok(()),
        Err(_) if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) => Ok(()),
        Err(_) => Err(E::Cleanup),
    }
}

fn fail_stop_at(deadline: AttemptDeadline) -> ! {
    eprintln!("external capture rejected: Cleanup; awaiting terminal attempt deadline");
    loop {
        let Ok(remaining) = deadline.remaining() else {
            terminal_fail_stop();
        };
        let ms = remaining.as_millis().max(1).min(i32::MAX as u128) as i32;
        unsafe {
            libc::poll(std::ptr::null_mut(), 0, ms);
        }
    }
}

#[cfg(test)]
mod cleanup_runtime_tests {
    use super::*;
    use std::process::{Command, Stdio};
    #[test]
    fn blocked_child_is_terminated_and_reaped_before_error_return() {
        // Actual children, including one that ignores orderly termination.
        for original in [E::Launch, E::Protocol, E::DocumentIdentity] {
            let mut child = Command::new("/bin/sh")
                .args(["-c", "trap '' TERM; echo ready; read value"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::BufRead;
            let mut ready = String::new();
            std::io::BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut ready)
                .unwrap();
            let pid = child.id() as i32;
            let fd = pidfd(pid).unwrap();
            signal(&fd, libc::SIGTERM).unwrap();
            terminate(&fd).unwrap();
            let status = wait_child(pid, AttemptDeadline::new()).unwrap();
            assert!(libc::WIFSIGNALED(status));
            assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
            assert_eq!(
                unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) },
                -1
            );
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
            assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
            assert_eq!(Ok::<_, E>(()).and(Err::<(), _>(original)), Err(original));
        }
    }
    #[test]
    fn reap_waits_for_delayed_release_without_renewing_deadline() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "read value"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id() as i32;
        let mut input = child.stdin.take().unwrap();
        let sender = std::thread::spawn(move || {
            input.write_all(b"release\n").unwrap();
        });
        let deadline = AttemptDeadline::new();
        let status = wait_child(pid, deadline).unwrap();
        sender.join().unwrap();
        assert!(libc::WIFEXITED(status));
        assert_eq!(
            unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) },
            -1
        );
        assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
    }
    #[test]
    #[ignore = "explicit Linux fork/watchdog lifecycle test"]
    fn watchdog_success_failure_and_terminal_expiry() {
        for failure in [false, true] {
            let child = unsafe { libc::fork() };
            assert!(child >= 0);
            if child == 0 {
                let d = AttemptDeadline::new();
                let guard = AttemptWatchdog::arm(d).unwrap();
                if failure {
                    terminate(&pidfd(guard.pid).unwrap()).unwrap();
                }
                let result = guard.finish();
                unsafe {
                    libc::_exit(
                        if result == if failure { Err(E::Cleanup) } else { Ok(()) } {
                            0
                        } else {
                            1
                        },
                    )
                }
            }
            let status = wait_child(child, AttemptDeadline::new()).unwrap();
            assert!(libc::WIFEXITED(status));
            assert_eq!(libc::WEXITSTATUS(status), 0);
        }
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            fail_stop_at(AttemptDeadline::expired());
        }
        let status = wait_child(child, AttemptDeadline::new()).unwrap();
        assert!(libc::WIFSIGNALED(status));
        assert_eq!(libc::WTERMSIG(status), libc::SIGKILL);
    }
}

#[cfg(test)]
static TEST_HANDSHAKE_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(all(test, target_arch = "x86_64"))]
mod attempt_failure_runtime_tests {
    use super::*;
    #[test]
    #[ignore = "explicit supported rootless Linux launch/failure/reap runtime; test ELF, never browser evidence"]
    fn actual_attempt_failure_paths_reap_owned_children() {
        let worker = unsafe { libc::fork() };
        assert!(worker >= 0);
        if worker == 0 {
            exercise();
            unsafe { libc::_exit(0) }
        }
        let status = wait_child(worker, AttemptDeadline::new()).unwrap();
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), 0);
    }
    fn exercise() {
        use std::os::unix::fs::PermissionsExt;
        // Test-only ELF executes pause in a loop. There is no CDP or DOM producer.
        let mut bytes = vec![0u8; 132];
        bytes[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        for (o, v) in [(16, 2u16), (18, 62), (52, 64), (54, 56), (56, 1)] {
            bytes[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        for (o, v) in [
            (24, 0x400078u64),
            (32, 64),
            (80, 0x400000),
            (88, 0x400000),
            (96, 132),
            (104, 132),
            (112, 4096),
        ] {
            bytes[o..o + 8].copy_from_slice(&v.to_le_bytes());
        }
        bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
        bytes[68..72].copy_from_slice(&5u32.to_le_bytes());
        bytes[120..129].copy_from_slice(&[0xb8, 34, 0, 0, 0, 0x0f, 0x05, 0xeb, 0xf7]);
        let supplied = tempfile::tempdir().unwrap();
        std::fs::set_permissions(supplied.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(supplied.path().join("chrome"), &bytes).unwrap();
        std::fs::set_permissions(
            supplied.path().join("chrome"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let digest = external_test_provenance::sha256(&bytes).to_string();
        let manifest = format!(
            "format = \"borrowser-chromium-distribution-manifest-v1\"\nroot_mode = 493\ndirectories = []\n[[entries]]\npath = \"chrome\"\nkind = \"regular\"\nmode = 493\nexecutable = true\nbyte_length = 132\nsha256 = \"{digest}\"\nfile_capabilities = \"absent\"\n"
        );
        let manifest =
            crate::distribution::DistributionManifest::parse(manifest.as_bytes()).unwrap();
        let distribution =
            VerifiedDistribution::create(&manifest, supplied.path(), "chrome", &digest).unwrap();
        for phase in 0..4 {
            let deadline = AttemptDeadline::new();
            let watchdog = AttemptWatchdog::arm(deadline).unwrap();
            let mut config = crate::configuration::specimen();
            // Invalid launch argv fails after supervisor/namespace creation but
            // before fork/exec. Only this test mutates a synthetic configuration.
            if phase == 0 {
                config.invocation_arguments.push("\0".into());
            }
            TEST_HANDSHAKE_FAILURE.store(phase == 1, std::sync::atomic::Ordering::SeqCst);
            let launched = IsolatedBrowser::launch(&config, &distribution, deadline);
            TEST_HANDSHAKE_FAILURE.store(false, std::sync::atomic::Ordering::SeqCst);
            match phase {
                0 | 1 => assert!(launched.is_err()),
                2 => {
                    let browser = launched.unwrap();
                    let protocol_failure = Err::<(), _>(E::Protocol);
                    assert_eq!(browser.abort().and(protocol_failure), Err(E::Protocol));
                }
                _ => {
                    // No renderer exists: real namespace-population verification
                    // rejects, but still proves teardown/reaping before return.
                    assert!(launched.unwrap().finish().is_err());
                }
            }
            watchdog.finish().unwrap();
            assert_eq!(
                unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) },
                -1
            );
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
        }
        distribution.close().unwrap();
        supplied.close().unwrap();
    }
    #[test]
    #[ignore = "explicit rootless Linux user/PID namespace capability runtime; no browser"]
    fn sys_admin_is_local_to_descendant_user_namespace() {
        assert_ne!(unsafe { libc::getuid() }, 0);
        let deadline = AttemptDeadline::new();
        // Prepared before fork: the child path uses only syscalls and stack data.
        let uid = format!("{0} {0} 1\n", unsafe { libc::getuid() });
        let gid = format!("{0} {0} 1\n", unsafe { libc::getgid() });
        let ancestor = File::open("/proc/self/ns/user").unwrap();
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            let result = (|| -> Result<()> {
                clear_exec_privileges()?;
                if unsafe { libc::unshare(libc::CLONE_NEWUSER) } != 0 {
                    return Err(E::Isolation);
                }
                for (path, bytes) in [
                    (c"/proc/self/setgroups", b"deny".as_slice()),
                    (c"/proc/self/uid_map", uid.as_bytes()),
                    (c"/proc/self/gid_map", gid.as_bytes()),
                ] {
                    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
                    if fd < 0 {
                        return Err(E::Isolation);
                    }
                    let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
                    let closed = unsafe { libc::close(fd) };
                    if written != bytes.len() as isize || closed != 0 {
                        return Err(E::Isolation);
                    }
                }
                #[repr(C)]
                struct Header {
                    version: u32,
                    pid: i32,
                }
                #[repr(C)]
                #[derive(Clone, Copy, Default, PartialEq)]
                struct Data {
                    effective: u32,
                    permitted: u32,
                    inheritable: u32,
                }
                let header = Header {
                    version: 0x20080522,
                    pid: 0,
                };
                let expected = [
                    Data {
                        effective: 1 << 21,
                        permitted: 1 << 21,
                        inheritable: 0,
                    },
                    Data::default(),
                ];
                let mut actual = [Data::default(); 2];
                if unsafe { libc::syscall(libc::SYS_capset, &header, expected.as_ptr()) } != 0
                    || unsafe { libc::syscall(libc::SYS_capget, &header, actual.as_mut_ptr()) } != 0
                    || actual != expected
                {
                    return Err(E::Sandbox);
                }
                // SYS_ADMIN in this new user namespace does not authorize
                // entering its ancestor. It does authorize a child PID namespace.
                if unsafe { libc::setns(ancestor.as_raw_fd(), libc::CLONE_NEWUSER) } != -1
                    || std::io::Error::last_os_error().raw_os_error() != Some(libc::EPERM)
                    || unsafe { libc::unshare(libc::CLONE_NEWPID) } != 0
                {
                    return Err(E::Sandbox);
                }
                let grandchild = unsafe { libc::fork() };
                if grandchild < 0 {
                    return Err(E::Launch);
                }
                if grandchild == 0 {
                    unsafe { libc::_exit(if libc::getpid() == 1 { 0 } else { 125 }) }
                }
                let status = wait_child(grandchild, deadline)?;
                if status != 0 {
                    return Err(E::Cleanup);
                }
                Ok(())
            })();
            unsafe { libc::_exit(if result.is_ok() { 0 } else { 125 }) }
        }
        assert_eq!(wait_child(child, deadline).unwrap(), 0);
    }
    #[test]
    #[ignore = "explicit Linux kernel process-role metadata test; shell fixture, never Chromium evidence"]
    #[allow(
        clippy::zombie_processes,
        reason = "wait_child reaps through the shared bounded pidfd/waitpid lifecycle"
    )]
    fn bootstrap_role_uses_live_process_objects() {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};
        assert_ne!(unsafe { libc::getuid() }, 0);
        let deadline = AttemptDeadline::new();
        let executable = File::open("/bin/sh").unwrap().metadata().unwrap();
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "read value", "--type=zygote", "--no-zygote-sandbox"])
            .stdin(Stdio::piped());
        // SAFETY: only capability syscalls execute in this pre-exec hook.
        unsafe {
            command.pre_exec(|| {
                clear_exec_privileges().map_err(|_| std::io::Error::from_raw_os_error(libc::EPERM))
            });
        }
        let child = command.spawn().unwrap();
        let pid = child.id() as i32;
        let retained = pidfd(pid).unwrap();
        let start = process_start(pid).unwrap();
        let actual = File::open(format!("/proc/{pid}/exe"))
            .unwrap()
            .metadata()
            .unwrap();
        assert_eq!(
            (actual.dev(), actual.ino()),
            (executable.dev(), executable.ino())
        );
        for name in ["user", "pid", "net"] {
            assert_eq!(
                inode(&namespace(pid, name).unwrap()).unwrap(),
                inode(&namespace(unsafe { libc::getpid() }, name).unwrap()).unwrap()
            );
        }
        let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).unwrap();
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        // Exec-time shell argv must not become Chromium process-title authority.
        assert!(super::super::verification::ChromiumProcessTitle::parse(&cmdline).is_err());
        super::super::verification::cleared_capabilities(&status).unwrap();
        assert_eq!(
            super::super::verification::parent_pid(&status).unwrap(),
            unsafe { libc::getpid() }
        );
        assert_eq!(process_start(pid).unwrap(), start);
        signal(&retained, libc::SIGKILL).unwrap();
        let status = wait_child(pid, deadline).unwrap();
        assert!(libc::WIFSIGNALED(status));
    }
    #[test]
    #[ignore = "explicit Linux argv-memory rewrite test; not Chromium qualification"]
    fn rewritten_title_is_not_exec_argv() {
        let deadline = AttemptDeadline::new();
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            let result = (|| -> Result<()> {
                let before = std::fs::read_to_string("/proc/self/cmdline")
                    .map_err(|_| E::ProcessIdentity)?;
                if super::super::verification::ChromiumProcessTitle::parse(&before).is_ok() {
                    return Err(E::ProcessIdentity);
                }
                let stat =
                    std::fs::read_to_string("/proc/self/stat").map_err(|_| E::ProcessIdentity)?;
                let (_, fields) = stat.rsplit_once(") ").ok_or(E::ProcessIdentity)?;
                // proc stat fields 48/49 are arg_start/arg_end; fields begins at 3.
                let start = fields
                    .split_whitespace()
                    .nth(45)
                    .ok_or(E::ProcessIdentity)?
                    .parse::<usize>()
                    .map_err(|_| E::ProcessIdentity)?;
                let end = fields
                    .split_whitespace()
                    .nth(46)
                    .ok_or(E::ProcessIdentity)?
                    .parse::<usize>()
                    .map_err(|_| E::ProcessIdentity)?;
                let size = end.checked_sub(start).ok_or(E::ProcessIdentity)?;
                let title = b"chrome --type=broker\0";
                if start == 0 || size < title.len() || size > 65536 {
                    return Err(E::Limit);
                }
                // SAFETY: kernel-reported argv memory belongs to this fork child.
                // Only its bounded writable argument region is rewritten. Parent
                // memory is unchanged; no later code consumes argv pointers.
                unsafe {
                    std::ptr::write_bytes(start as *mut u8, 0, size);
                    std::ptr::copy_nonoverlapping(title.as_ptr(), start as *mut u8, title.len());
                }
                let after = std::fs::read_to_string("/proc/self/cmdline")
                    .map_err(|_| E::ProcessIdentity)?;
                let parsed = super::super::verification::ChromiumProcessTitle::parse(&after)?;
                if super::super::verification::chromium_child_role(&parsed)?
                    != super::super::verification::PreliminaryRole::BrokerCandidate
                {
                    return Err(E::ProcessIdentity);
                }
                Ok(())
            })();
            unsafe { libc::_exit(if result.is_ok() { 0 } else { 125 }) }
        }
        assert_eq!(wait_child(child, deadline).unwrap(), 0);
    }
}

#[cfg(test)]
mod prelaunch_cleanup_tests {
    use super::*;
    #[derive(Default)]
    struct Fault {
        stage: &'static str,
        cleanup_error: bool,
        path: std::path::PathBuf,
        closes: usize,
    }
    thread_local! { static FAULT: std::cell::RefCell<Fault> = Default::default(); }
    pub(super) fn checkpoint(stage: &str, path: &Path) -> Result<()> {
        FAULT.with_borrow_mut(|fault| {
            fault.path = path.to_owned();
            if fault.stage == stage {
                Err(if stage == "namespace" {
                    E::Isolation
                } else {
                    E::Launch
                })
            } else {
                Ok(())
            }
        })
    }
    pub(super) fn closed(result: Result<()>) -> Result<()> {
        FAULT.with_borrow_mut(|fault| {
            fault.closes += 1;
            if fault.cleanup_error {
                Err(E::Cleanup)
            } else {
                result
            }
        })
    }
    #[test]
    fn prelaunch_partial_cleanup_covers_socket_namespace_and_fork_failure() {
        for stage in ["socket", "namespace", "fork"] {
            for cleanup_error in [false, true] {
                FAULT.set(Fault {
                    stage,
                    cleanup_error,
                    ..Default::default()
                });
                let expected = if cleanup_error {
                    E::Cleanup
                } else if stage == "namespace" {
                    E::Isolation
                } else {
                    E::Launch
                };
                // Always fails before fork: safe in the multithreaded test harness.
                assert!(matches!(prepare_launch(), Err(error) if error == expected));
                FAULT.with_borrow(|fault| {
                    assert_eq!(fault.closes, 1);
                    assert!(!fault.path.exists());
                });
            }
        }
        FAULT.set(Fault::default());
    }
}
