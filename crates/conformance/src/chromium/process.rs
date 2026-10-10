//! Private launcher for this single-browser CLI path. No shells, concurrent
//! launches, global supervisors, or externally supplied process identities.
use super::error::{CleanupError, Error, Phase};
use std::{
    ffi::CString,
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
use linux as native;
#[cfg(target_os = "macos")]
use macos as native;

static SIGNALLED: AtomicBool = AtomicBool::new(false);
extern "C" fn cancelled(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::Relaxed);
}

#[derive(Default)]
pub(super) struct Cancellation {
    local: AtomicBool,
}
impl Cancellation {
    pub fn check_cancelled(&self) -> Result<(), Error> {
        if self.local.load(Ordering::Relaxed) || SIGNALLED.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    pub fn check(&self, deadline: Instant, phase: Phase) -> Result<(), Error> {
        self.check_cancelled()?;
        if Instant::now() >= deadline {
            return Err(Error::Timeout(phase));
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn cancel(&self) {
        self.local.store(true, Ordering::Relaxed);
    }
}

/// The same absolute budget follows every native ownership operation. Cleanup
/// ignores cancellation so an interrupted capture can still terminate its tree.
struct OwnershipBudget<'a> {
    deadline: Instant,
    cancel: Option<&'a Cancellation>,
}
#[derive(Debug)]
enum OwnershipError {
    Timeout,
    Cancelled,
    Inspection(String),
}
impl From<String> for OwnershipError {
    fn from(value: String) -> Self {
        Self::Inspection(value)
    }
}
impl From<&str> for OwnershipError {
    fn from(value: &str) -> Self {
        Self::Inspection(value.into())
    }
}
impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => f.write_str("native ownership deadline expired"),
            Self::Cancelled => f.write_str("native ownership inspection cancelled"),
            Self::Inspection(message) => f.write_str(message),
        }
    }
}
impl OwnershipError {
    fn execution(self, phase: Phase) -> Error {
        match self {
            Self::Timeout => Error::Timeout(phase),
            Self::Cancelled => Error::Cancelled,
            Self::Inspection(message) => {
                Error::UnsupportedPlatform(format!("process ownership: {message}"))
            }
        }
    }
    fn cleanup(self, category: fn(String) -> CleanupError) -> CleanupError {
        match self {
            Self::Timeout => CleanupError::Timeout,
            other => category(other.to_string()),
        }
    }
}
impl OwnershipBudget<'_> {
    fn check(&self) -> Result<(), OwnershipError> {
        if self.cancel.is_some_and(|c| c.check_cancelled().is_err()) {
            return Err(OwnershipError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(OwnershipError::Timeout);
        }
        Ok(())
    }
    /// Native calls cannot be forcibly interrupted. Never accept their result
    /// after expiry; resource-producing closures must return an RAII owner.
    fn observe<T>(&self, operation: impl FnOnce() -> T) -> Result<T, OwnershipError> {
        self.check()?;
        let result = operation();
        self.check()?;
        Ok(result)
    }
}

/// Installed only by the standalone Chromium CLI (and isolated native tests).
pub(super) struct Standalone {
    handlers: Vec<(i32, libc::sigaction)>,
}
impl Standalone {
    pub fn enter() -> Result<Self, Error> {
        // A retained, waitable child anchors its numeric session ID. Inherited
        // SIG_IGN/SA_NOCLDWAIT would silently destroy that ownership invariant.
        let mut child_action: libc::sigaction = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut child_action) } != 0 {
            return Err(last("inspect SIGCHLD disposition"));
        }
        if child_action.sa_sigaction != libc::SIG_DFL
            || child_action.sa_flags & libc::SA_NOCLDWAIT != 0
        {
            return Err(Error::UnsupportedPlatform(
                "standalone capture requires default SIGCHLD disposition".into(),
            ));
        }
        native::preflight()?;
        SIGNALLED.store(false, Ordering::Relaxed);
        let mut guard = Self {
            handlers: Vec::new(),
        };
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGPIPE] {
            // SAFETY: fully initialized sigaction objects; signal handler only
            // performs a lock-free atomic store. SIGPIPE becomes an I/O error.
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                libc::sigemptyset(&mut action.sa_mask);
                action.sa_sigaction = if signal == libc::SIGPIPE {
                    libc::SIG_IGN
                } else {
                    cancelled as *const () as usize
                };
                let mut old = std::mem::zeroed();
                if libc::sigaction(signal, &action, &mut old) != 0 {
                    return Err(last("install cancellation handler"));
                }
                guard.handlers.push((signal, old));
            }
        }
        Ok(guard)
    }
}
impl Drop for Standalone {
    fn drop(&mut self) {
        for (signal, old) in &self.handlers {
            unsafe {
                libc::sigaction(*signal, old, std::ptr::null_mut());
            }
        }
    }
}

pub(super) fn last(operation: &'static str) -> Error {
    Error::io(operation, std::io::Error::last_os_error())
}

fn pipe() -> Result<(File, File), Error> {
    let mut raw = [-1; 2];
    // SAFETY: valid two-element output; no other thread/spawn exists in CLI.
    unsafe {
        #[cfg(target_os = "linux")]
        let result = libc::pipe2(raw.as_mut_ptr(), libc::O_CLOEXEC);
        #[cfg(not(target_os = "linux"))]
        let result = libc::pipe(raw.as_mut_ptr());
        if result != 0 {
            return Err(last("pipe"));
        }
        let originals = [OwnedFd::from_raw_fd(raw[0]), OwnedFd::from_raw_fd(raw[1])];
        let mut moved = Vec::new();
        for fd in &originals {
            let copy = libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5);
            if copy == -1 {
                return Err(last("duplicate pipe"));
            }
            moved.push(File::from_raw_fd(copy));
        }
        let write = moved.pop().unwrap();
        Ok((moved.pop().unwrap(), write))
    }
}
fn nonblocking(file: &File) -> Result<(), Error> {
    unsafe {
        let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL);
        if flags < 0 || libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(last("nonblocking pipe"));
        }
    }
    Ok(())
}
fn cstring(bytes: &[u8]) -> Result<CString, Error> {
    CString::new(bytes)
        .map_err(|_| Error::Configuration("NUL in launch argument or environment".into()))
}

pub(super) struct OwnedChromium {
    pid: libc::pid_t,
    reaped: bool,
    pub input: Option<File>,
    pub output: File,
    stderr: File,
    registry: native::Registry,
    directory: Option<tempfile::TempDir>,
    pub diagnostics: Vec<u8>,
    finished: bool,
    last_scan: Instant,
}

impl OwnedChromium {
    #[cfg(all(test, target_os = "macos"))]
    pub fn topology(&mut self) -> Result<Vec<(String, bool)>, String> {
        self.registry.topology().map_err(|e| e.to_string())
    }
    #[cfg(test)]
    pub fn kill_root(&mut self) -> Result<(), String> {
        self.registry.kill_root().map_err(|e| e.to_string())
    }
    #[cfg(test)]
    pub fn artifact_path(&self) -> &Path {
        self.directory.as_ref().unwrap().path()
    }
    #[cfg(test)]
    pub fn recover_test_children(&mut self) -> Vec<CleanupError> {
        // Rescue only the deliberately timed-out fixture. An unreaped root
        // still anchors discovery; never rescan using a consumed root PID.
        assert!(self.finished);
        if self.reaped {
            let budget = OwnershipBudget {
                deadline: Instant::now() + Duration::from_secs(5),
                cancel: None,
            };
            // All creators were verified dead before the injected reap fault.
            // Do not discover or signal anything after consuming the root PID.
            return self
                .registry
                .reap_adopted(&budget)
                .err()
                .map(|e| vec![e.cleanup(CleanupError::Reap)])
                .unwrap_or_default();
        }
        self.finished = false;
        self.finish(Duration::from_secs(5))
    }
    #[cfg(test)]
    pub fn test_discover(&mut self, deadline: Instant, cancel: &Cancellation) -> Result<(), Error> {
        self.last_scan = Instant::now() - Duration::from_secs(1);
        self.check(deadline, Phase::Capture, cancel)
    }
    pub fn launch(
        executable: &Path,
        arguments: &[String],
        deadline: Instant,
        cancel: &Cancellation,
    ) -> Result<Self, super::error::Failure> {
        let mut owner: Option<Self> = None;
        let result = (|| {
            cancel.check(deadline, Phase::Startup)?;
            let directory = tempfile::Builder::new()
                .prefix("borrowser-chromium-")
                .tempdir()
                .map_err(|e| Error::io("private profile", e))?;
            // Chromium canonicalizes its database location. Canonicalize the
            // private parent first so /var -> /private/var cannot hide Crashpad
            // from exact ownership matching on macOS.
            let root = directory
                .path()
                .canonicalize()
                .map_err(|e| Error::io("canonical private directory", e))?;
            if root.to_str().is_none() {
                return Err(Error::Configuration(
                    "private Chromium directory must be UTF-8".into(),
                ));
            }
            let profile = root.join("profile");
            let crash = root.join("crashes");
            std::fs::create_dir(&crash).map_err(|e| Error::io("private crash directory", e))?;
            let exe = cstring(executable.as_os_str().as_bytes())?;
            let mut args = vec![exe.clone()];
            for arg in arguments {
                args.push(cstring(arg.as_bytes())?);
            }
            args.push(cstring(
                format!("--user-data-dir={}", profile.display()).as_bytes(),
            )?);
            let argv: Vec<_> = args
                .iter()
                .map(|a| a.as_ptr())
                .chain(std::iter::once(std::ptr::null()))
                .collect();
            let mut env = Vec::new();
            for (key, value) in std::env::vars_os() {
                if key == "BREAKPAD_DUMP_LOCATION" {
                    continue;
                }
                let mut bytes = key.as_bytes().to_vec();
                bytes.push(b'=');
                bytes.extend_from_slice(value.as_bytes());
                env.push(cstring(&bytes)?);
            }
            env.push(cstring(
                format!("BREAKPAD_DUMP_LOCATION={}", crash.display()).as_bytes(),
            )?);
            let envp: Vec<_> = env
                .iter()
                .map(|a| a.as_ptr())
                .chain(std::iter::once(std::ptr::null()))
                .collect();
            let (command_read, command_write) = pipe()?;
            let (response_read, response_write) = pipe()?;
            let (status_read, status_write) = pipe()?;
            let (stderr_read, stderr_write) = pipe()?;
            // Configure only the parent's ends, before a child can exist.
            // Opposite pipe ends have distinct open-file descriptions: child
            // FD 3/4 and the exec-status writer remain blocking after dup2.
            for file in [&command_write, &response_read, &stderr_read, &status_read] {
                nonblocking(file)?;
                #[cfg(test)]
                if fault::take(fault::Point::Descriptor) {
                    return Err(Error::Launch {
                        stage: "injected descriptor setup",
                        errno: libc::EIO,
                    });
                }
            }
            let null = File::options()
                .read(true)
                .write(true)
                .open("/dev/null")
                .map_err(|e| Error::io("standard streams", e))?;
            // Enumerate before fork. Closing these copies in the child prevents
            // inherited ambient FDs, cancellation channels, or parent read ends
            // from keeping resources alive. No thread may open FDs concurrently.
            let fd_dir = if cfg!(target_os = "linux") {
                "/proc/self/fd"
            } else {
                "/dev/fd"
            };
            cancel.check(deadline, Phase::Startup)?;
            let mut entries =
                std::fs::read_dir(fd_dir).map_err(|e| Error::io("enumerate descriptors", e))?;
            let mut open_fds = Vec::new();
            loop {
                cancel.check(deadline, Phase::Startup)?;
                let entry = entries.next();
                cancel.check(deadline, Phase::Startup)?;
                let Some(entry) = entry else { break };
                let entry = entry.map_err(|e| Error::io("enumerate descriptors", e))?;
                if let Some(fd) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<i32>().ok())
                {
                    open_fds.push(fd);
                }
            }
            drop(entries);
            let raw = [
                command_read.as_raw_fd(),
                response_write.as_raw_fd(),
                status_write.as_raw_fd(),
                null.as_raw_fd(),
                stderr_write.as_raw_fd(),
            ];
            let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
            let mut old_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
            unsafe {
                libc::sigemptyset(&mut mask);
                libc::sigaddset(&mut mask, libc::SIGINT);
                libc::sigaddset(&mut mask, libc::SIGTERM);
                if libc::sigprocmask(libc::SIG_BLOCK, &mask, &mut old_mask) != 0 {
                    return Err(last("block cancellation during fork"));
                }
            }
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                // SAFETY: all data and pointers were allocated before fork;
                // this branch only calls async-signal-safe Unix operations.
                // It never allocates, locks, logs, panics, or drops Rust values.
                unsafe {
                    if libc::setsid() == -1 {
                        child_error(raw[2], 1);
                    }
                    for (source, destination) in [
                        (raw[3], 0),
                        (raw[3], 1),
                        (raw[4], 2),
                        (raw[0], 3),
                        (raw[1], 4),
                    ] {
                        if libc::dup2(source, destination) == -1 {
                            child_error(raw[2], 2);
                        }
                    }
                    for fd in &open_fds {
                        if *fd > 4 && *fd != raw[2] {
                            libc::close(*fd);
                        }
                    }
                    let mut action: libc::sigaction = std::mem::zeroed();
                    libc::sigemptyset(&mut action.sa_mask);
                    action.sa_sigaction = libc::SIG_DFL;
                    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGPIPE, libc::SIGCHLD] {
                        if libc::sigaction(signal, &action, std::ptr::null_mut()) == -1 {
                            child_error(raw[2], 3);
                        }
                    }
                    if libc::sigprocmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut()) != 0 {
                        child_error(raw[2], 3);
                    }
                    libc::execve(exe.as_ptr(), argv.as_ptr(), envp.as_ptr());
                    child_error(raw[2], 4);
                }
            }
            let fork_error = std::io::Error::last_os_error();
            if pid > 0 {
                owner = Some(Self {
                    pid,
                    reaped: false,
                    input: Some(command_write),
                    output: response_read,
                    stderr: stderr_read,
                    registry: native::Registry::new(pid, profile, crash, executable.to_path_buf()),
                    directory: Some(directory),
                    diagnostics: Vec::new(),
                    finished: false,
                    last_scan: Instant::now() - Duration::from_secs(1),
                });
            }
            let restored =
                unsafe { libc::sigprocmask(libc::SIG_SETMASK, &old_mask, std::ptr::null_mut()) };
            if pid < 0 {
                return Err(Error::io("fork", fork_error));
            }
            if restored != 0 {
                return Err(last("restore cancellation mask"));
            }
            #[cfg(test)]
            {
                if fault::take(fault::Point::MaskRestored) {
                    return Err(Error::Launch {
                        stage: "injected mask restoration",
                        errno: libc::EIO,
                    });
                }
                assert!(
                    !fault::take(fault::Point::DropPartial),
                    "injected partial-launch unwind"
                );
            }
            drop((
                command_read,
                response_write,
                status_write,
                stderr_write,
                null,
            ));
            let child = owner.as_mut().unwrap();
            let mut status_read = status_read;
            let mut record = Vec::new();
            loop {
                #[cfg(test)]
                {
                    if fault::take(fault::Point::CancelStartup) {
                        cancel.cancel();
                    }
                    if fault::take(fault::Point::LaunchStatus) {
                        return Err(Error::Launch {
                            stage: "injected launch-status read",
                            errno: libc::EIO,
                        });
                    }
                }
                cancel.check(deadline, Phase::Startup)?;
                let mut bytes = [0; 8];
                match status_read.read(&mut bytes) {
                    Ok(0) if record.is_empty() => break,
                    Ok(0) => {
                        return Err(Error::Launch {
                            stage: "truncated exec status",
                            errno: libc::EIO,
                        });
                    }
                    Ok(n) => {
                        record.extend_from_slice(&bytes[..n]);
                        if record.len() == 8 {
                            let stage = i32::from_ne_bytes(record[..4].try_into().unwrap());
                            let errno = i32::from_ne_bytes(record[4..].try_into().unwrap());
                            return Err(Error::Launch {
                                stage: match stage {
                                    1 => "setsid",
                                    2 => "dup2",
                                    3 => "signals",
                                    _ => "execve",
                                },
                                errno,
                            });
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(Error::io("exec status", e)),
                }
                match child.check(deadline, Phase::Startup, cancel) {
                    Err(Error::BrowserExited) => continue, // consume its exec status before classifying
                    result => result?,
                }
                poll_fd(status_read.as_raw_fd(), libc::POLLIN, deadline)?;
            }
            cancel.check(deadline, Phase::Startup)?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(owner.unwrap()),
            Err(primary) => {
                let cleanup = owner
                    .as_mut()
                    .map(|p| p.finish(Duration::from_secs(5)))
                    .unwrap_or_default();
                Err(super::error::Failure {
                    primary: Some(primary),
                    cleanup,
                })
            }
        }
    }
    pub fn check(
        &mut self,
        deadline: Instant,
        phase: Phase,
        cancel: &Cancellation,
    ) -> Result<(), Error> {
        let budget = OwnershipBudget {
            deadline,
            cancel: Some(cancel),
        };
        budget.check().map_err(|e| e.execution(phase))?;
        self.drain_stderr(&budget).map_err(|e| e.execution(phase))?;
        if self.last_scan.elapsed() >= Duration::from_millis(20) {
            self.registry
                .discover(&budget)
                .map_err(|e| e.execution(phase))?;
            self.last_scan = Instant::now();
        }
        if budget
            .observe(|| self.exited())
            .map_err(|e| e.execution(phase))??
        {
            return Err(Error::BrowserExited);
        }
        budget.check().map_err(|e| e.execution(phase))
    }
    fn exited(&self) -> Result<bool, Error> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                self.pid as _,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if rc != 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            return Ok(false); // caller's bounded poll retries
        }
        if rc != 0 {
            return Err(last("observe child"));
        }
        Ok(unsafe { info.si_pid() } == self.pid)
    }
    fn drain_stderr(&mut self, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        let mut bytes = [0; 4096];
        // Bound work even if a failing browser continuously writes stderr.
        for _ in 0..16 {
            match budget.observe(|| self.stderr.read(&mut bytes))? {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let count = n.min(65536_usize.saturating_sub(self.diagnostics.len()));
                    self.diagnostics.extend_from_slice(&bytes[..count]);
                }
            }
        }
        budget.check()
    }
    pub fn finish(&mut self, duration: Duration) -> Vec<CleanupError> {
        if self.finished {
            return vec![CleanupError::Identity("cleanup already attempted".into())];
        }
        let start = Instant::now();
        let budget = OwnershipBudget {
            deadline: start + duration,
            cancel: None,
        };
        self.input.take(); // closes command pipe; also wakes Chromium's reader
        let mut errors = Vec::new();
        let termination = (|| -> Result<(), CleanupError> {
            loop {
                budget
                    .check()
                    .map_err(|e| e.cleanup(CleanupError::Identity))?;
                self.drain_stderr(&budget)
                    .map_err(|e| e.cleanup(CleanupError::Identity))?;
                if let Err(e) = self.registry.discover(&budget) {
                    match e {
                        OwnershipError::Timeout | OwnershipError::Cancelled => {
                            return Err(e.cleanup(CleanupError::Identity));
                        }
                        _ if errors.is_empty() => errors.push(e.cleanup(CleanupError::Identity)),
                        _ => {}
                    }
                }
                let elapsed = start.elapsed();
                let signal = if elapsed >= duration / 2 {
                    Some(libc::SIGKILL)
                } else if elapsed >= duration / 5 {
                    Some(libc::SIGTERM)
                } else {
                    None
                };
                if let Some(signal) = signal
                    && let Err(e) = self.registry.signal(signal, &budget)
                {
                    match e {
                        OwnershipError::Timeout | OwnershipError::Cancelled => {
                            return Err(e.cleanup(CleanupError::Signal));
                        }
                        _ if errors.is_empty() => errors.push(e.cleanup(CleanupError::Signal)),
                        _ => {}
                    }
                }
                let terminated = match self.registry.terminated(&budget) {
                    Ok(terminated) => terminated,
                    Err(e @ (OwnershipError::Timeout | OwnershipError::Cancelled)) => {
                        return Err(e.cleanup(CleanupError::Identity));
                    }
                    Err(e) => {
                        if errors.is_empty() {
                            errors.push(e.cleanup(CleanupError::Identity));
                        }
                        false
                    }
                };
                // Verification can itself consume the remaining budget. Never
                // accept a late or incomplete observation as cleanup success.
                budget
                    .check()
                    .map_err(|e| e.cleanup(CleanupError::Identity))?;
                if terminated
                    && budget
                        .observe(|| self.exited())
                        .map_err(|e| e.cleanup(CleanupError::Reap))?
                        .map_err(|e| CleanupError::Reap(e.to_string()))?
                {
                    return Ok(());
                }
                poll_fd(self.stderr.as_raw_fd(), libc::POLLIN, budget.deadline)
                    .map_err(|e| CleanupError::Reap(e.to_string()))?;
            }
        })();
        let complete = termination.is_ok();
        if let Err(e) = termination {
            errors.push(e);
        }
        // Keep the root waitable until all identity-dependent scans finish.
        // Expiry skips reaping rather than silently extending the deadline.
        let reaping = (|| -> Result<(), CleanupError> {
            #[cfg(test)]
            fault::expire(fault::Point::Reap, budget.deadline);
            loop {
                budget.check().map_err(|e| e.cleanup(CleanupError::Reap))?;
                let mut status = 0;
                let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
                let error = std::io::Error::last_os_error();
                // Record consumption before the post-syscall check: the PID
                // must never be reused as an identity after a successful wait.
                self.reaped = result == self.pid;
                budget.check().map_err(|e| e.cleanup(CleanupError::Reap))?;
                if result < 0 && error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                if !self.reaped {
                    return Err(CleanupError::Reap(format!(
                        "direct child {}, waitpid={result}",
                        self.pid
                    )));
                }
                break;
            }
            if complete {
                self.registry
                    .reap_adopted(&budget)
                    .map_err(|e| e.cleanup(CleanupError::Reap))?;
            }
            budget.check().map_err(|e| e.cleanup(CleanupError::Reap))
        })();
        if let Err(e) = reaping {
            errors.push(e);
        }
        if errors.is_empty() {
            if let Err(e) = budget.check() {
                errors.push(e.cleanup(CleanupError::Identity));
            } else if let Some(dir) = self.directory.take()
                && let Err(e) = dir.close()
            {
                errors.push(CleanupError::Artifacts(e));
            }
        }
        // Filesystem cleanup cannot be forcibly interrupted either. Its late
        // return also prevents success, even if removal already completed.
        if Instant::now() >= budget.deadline
            && !errors.iter().any(|e| matches!(e, CleanupError::Timeout))
        {
            errors.push(CleanupError::Timeout);
        }
        if let Some(dir) = self.directory.take() {
            eprintln!(
                "Chromium cleanup incomplete; artifacts: {}",
                dir.keep().display()
            );
        }
        self.finished = true;
        errors
    }
}
impl Drop for OwnedChromium {
    fn drop(&mut self) {
        if !self.finished {
            for error in self.finish(Duration::from_secs(5)) {
                eprintln!("Chromium emergency cleanup: {error}");
            }
        }
    }
}

// SAFETY: called only after fork, status FD is a blocking empty pipe with room
// for this single atomic record. _exit bypasses all inherited Rust destructors.
unsafe fn child_error(fd: i32, stage: i32) -> ! {
    #[cfg(target_os = "macos")]
    let errno = unsafe { *libc::__error() };
    #[cfg(target_os = "linux")]
    let errno = unsafe { *libc::__errno_location() };
    let record = [stage, errno];
    unsafe {
        loop {
            if libc::write(fd, record.as_ptr().cast(), 8) >= 0 {
                break;
            }
            #[cfg(target_os = "macos")]
            let error = *libc::__error();
            #[cfg(target_os = "linux")]
            let error = *libc::__errno_location();
            if error != libc::EINTR {
                break;
            }
        }
        libc::_exit(127);
    }
}

pub(super) fn poll_fd(fd: i32, events: i16, deadline: Instant) -> Result<(), Error> {
    let ms = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .min(20) as i32;
    let mut item = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut item, 1, ms) };
    if result < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
        return Err(last("poll"));
    }
    Ok(())
}

// Faults exist only in isolated native test processes. They neither change the
// executable's production arguments/environment nor replace ownership checks.
#[cfg(test)]
pub(super) mod fault {
    use super::*;
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum Point {
        Descriptor,
        MaskRestored,
        LaunchStatus,
        CancelStartup,
        DropPartial,
        Discovery,
        Verification,
        Reap,
        #[cfg(target_os = "macos")]
        ArgumentsUnavailable,
        #[cfg(target_os = "macos")]
        ArgumentsUnavailablePersistent,
        #[cfg(target_os = "linux")]
        AdoptedReap,
    }
    thread_local! { static POINT: std::cell::Cell<Option<Point>> = const { std::cell::Cell::new(None) }; }
    #[cfg(target_os = "macos")]
    thread_local! {
        // Arming/rearming a persistent fault is not evidence that it fired.
        static PERSISTENT_ARGUMENT_EIO_ACTIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    #[cfg(target_os = "macos")]
    pub fn reset_persistent_argument_eio_activations() {
        PERSISTENT_ARGUMENT_EIO_ACTIVATIONS.with(|count| count.set(0));
    }
    #[cfg(target_os = "macos")]
    pub fn record_persistent_argument_eio_activation() {
        PERSISTENT_ARGUMENT_EIO_ACTIVATIONS.with(|count| count.set(count.get() + 1));
    }
    #[cfg(target_os = "macos")]
    pub fn assert_persistent_argument_eio_since(previous: usize) -> usize {
        PERSISTENT_ARGUMENT_EIO_ACTIVATIONS.with(|count| {
            let current = count.get();
            assert!(
                current > previous,
                "regression did not activate persistent argument EIO"
            );
            current
        })
    }
    pub fn set(point: Point) {
        POINT.with(|p| {
            assert!(p.get().is_none());
            p.set(Some(point));
        });
    }
    pub fn take(point: Point) -> bool {
        POINT.with(|p| {
            if p.get() == Some(point) {
                p.set(None);
                true
            } else {
                false
            }
        })
    }
    pub fn assert_consumed() {
        POINT.with(|p| {
            assert!(
                p.get().is_none(),
                "regression did not reach its injected fault"
            )
        });
    }
    pub fn expire(point: Point, deadline: Instant) {
        if take(point) {
            // Model a native operation returning only after its budget ends.
            // This is a fault delay, never a production readiness condition.
            while Instant::now() < deadline {
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            }
        }
    }
}
