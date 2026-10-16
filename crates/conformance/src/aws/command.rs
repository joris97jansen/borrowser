//! Serial, foreground AG4 commands only, before any parser worker exists.
use std::{
    ffi::OsString,
    io::{Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
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
#[cfg(test)]
mod tests;

static INTERRUPTS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static FAIL_OBSERVATION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTS.fetch_add(1, Ordering::Relaxed);
}

pub(super) struct Spec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub input: Vec<u8>,
    pub timeout: Duration,
    pub stdout_limit: usize,
    pub cleanup: bool,
    pub env: Vec<(OsString, OsString)>,
    pub clear_env: bool,
    // Opt-in bounded diagnostic context for the SSH transport, never AWS credentials.
    pub failure_stderr_limit: usize,
    // Only ordinary SSH with AG4's EICE ProxyCommand expects asynchronous
    // proxy shutdown after a successful root exit.
    pub eice_proxy: bool,
}
impl Spec {
    pub fn new(
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            input: vec![],
            timeout: Duration::from_secs(20),
            stdout_limit: 256 * 1024,
            cleanup: false,
            env: vec![],
            clear_env: false,
            failure_stderr_limit: 0,
            eice_proxy: false,
        }
    }
}
#[derive(Debug)]
pub(super) struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
}
#[derive(Debug)]
pub(super) struct Failure {
    pub primary: String,
    pub cleanup: Option<String>,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.primary)?;
        if let Some(c) = &self.cleanup {
            write!(f, "; command cleanup: {c}")?;
        }
        Ok(())
    }
}
// The only testing seam: scripted CLI responses and elapsed time, not providers.
pub(super) trait Commands {
    fn run(&mut self, spec: Spec) -> Result<Output, Failure>;
    fn elapsed(&self) -> Duration;
    fn pause(&mut self, duration: Duration, cleanup: bool) -> Result<(), String>;
    fn interrupted(&self, cleanup: bool) -> bool;
}
pub(super) struct Owner {
    start: Instant,
    old: [libc::sigaction; 2],
    native: native::Scope,
    poisoned: bool,
    // Test-only observations of the actual owner, not a simulated scheduler.
    #[cfg(test)]
    pub(in crate::aws) timing: Vec<(&'static str, Instant)>,
}
impl Owner {
    pub fn enter() -> Result<Self, String> {
        let mut disposition = unsafe { std::mem::zeroed::<libc::sigaction>() };
        if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut disposition) } != 0
            || disposition.sa_sigaction != libc::SIG_DFL
            || disposition.sa_flags & libc::SA_NOCLDWAIT != 0
        {
            return Err("AG4 requires default SIGCHLD disposition to retain child identity".into());
        }
        if !cfg!(any(
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64")
        )) {
            return Err("AG4 requires native Linux x86-64 or macOS arm64".into());
        }
        let mut status = 0;
        let rc = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
        if rc != -1 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ECHILD) {
            return Err("AG4 requires a standalone process without other children".into());
        }
        let native = native::Scope::enter()?;
        let mut old = unsafe { std::mem::zeroed::<[libc::sigaction; 2]>() };
        let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
        action.sa_sigaction = interrupt as *const () as usize;
        unsafe { libc::sigemptyset(&mut action.sa_mask) };
        INTERRUPTS.store(0, Ordering::Relaxed);
        for (i, signal) in [libc::SIGINT, libc::SIGTERM].into_iter().enumerate() {
            if unsafe { libc::sigaction(signal, &action, &mut old[i]) } != 0 {
                for (j, signal) in [libc::SIGINT, libc::SIGTERM]
                    .into_iter()
                    .take(i)
                    .enumerate()
                {
                    unsafe { libc::sigaction(signal, &old[j], std::ptr::null_mut()) };
                }
                return Err("cannot install cancellation handlers".into());
            }
        }
        Ok(Self {
            start: Instant::now(),
            old,
            native,
            poisoned: false,
            #[cfg(test)]
            timing: Vec::with_capacity(12),
        })
    }
    pub fn quiescent(&self) -> Result<(), String> {
        if self.poisoned {
            return Err("controller command cleanup unresolved".into());
        }
        self.native.quiescent()
    }
    pub fn finish(self) -> Result<(), String> {
        self.quiescent()?;
        if self.interrupted(false) {
            return Err("operation interrupted".into());
        }
        Ok(()) // Drop restores handlers/subreaper before the caller starts workers.
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        for (i, signal) in [libc::SIGINT, libc::SIGTERM].into_iter().enumerate() {
            unsafe { libc::sigaction(signal, &self.old[i], std::ptr::null_mut()) };
        }
    }
}
fn nonblocking(fd: i32) -> Result<(), String> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("cannot make command pipe nonblocking".into());
    }
    Ok(())
}
fn drain(
    pipe: &mut Option<impl Read>,
    bytes: &mut Vec<u8>,
    limit: usize,
    discard: bool,
) -> Result<(), String> {
    let Some(p) = pipe else { return Ok(()) };
    let mut chunk = [0; 8192];
    match p.read(&mut chunk) {
        Ok(0) => {
            *pipe = None;
            Ok(())
        }
        Ok(n) => {
            if !discard {
                if bytes.len() + n > limit {
                    return Err("command output limit exceeded".into());
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            Ok(())
        }
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(format!("command pipe: {e}")),
    }
}
fn exited(pid: i32) -> Result<bool, String> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as _,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if rc < 0 {
        return Err(format!("command wait: {}", std::io::Error::last_os_error()));
    }
    Ok(unsafe { info.si_pid() } == pid)
}
fn root_succeeded(pid: i32) -> Result<bool, String> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    if unsafe {
        libc::waitid(
            libc::P_PID,
            pid as _,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } < 0
    {
        return Err(format!(
            "command exit observation: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(unsafe { info.si_pid() } == pid
        && info.si_code == libc::CLD_EXITED
        && unsafe { info.si_status() } == 0)
}
// Keep the signal and immediately captured errno together. ESRCH means the
// group is already absent; no other signaling error establishes cleanup.
fn group_signal_result(signal: &str, error: Option<std::io::Error>) -> Result<(), String> {
    match error {
        None => Ok(()),
        Some(error) if error.raw_os_error() == Some(libc::ESRCH) => Ok(()),
        Some(error) => Err(format!(
            "owned command group {signal} failed: {error}; raw errno={:?}",
            error.raw_os_error()
        )),
    }
}
fn signal_group(pid: i32, signal: i32) -> Result<(), String> {
    let rc = unsafe { libc::kill(-pid, signal) };
    let error = (rc < 0).then(std::io::Error::last_os_error);
    let name = match signal {
        libc::SIGTERM => "SIGTERM",
        libc::SIGKILL => "SIGKILL",
        _ => unreachable!("only AG4 cleanup signals"),
    };
    #[cfg(test)]
    eprintln!("AG4 group {pid} {name}: captured error={error:?}");
    group_signal_result(name, error)
}
impl Commands for Owner {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
    fn interrupted(&self, cleanup: bool) -> bool {
        INTERRUPTS.load(Ordering::Relaxed) >= if cleanup { 2 } else { 1 }
    }
    fn pause(&mut self, duration: Duration, cleanup: bool) -> Result<(), String> {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            if self.interrupted(cleanup) {
                return Err("operation interrupted".into());
            }
            std::thread::sleep(
                Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        Ok(())
    }
    fn run(&mut self, spec: Spec) -> Result<Output, Failure> {
        #[cfg(test)]
        self.timing.clear();
        if self.poisoned || self.interrupted(spec.cleanup) || spec.timeout.is_zero() {
            return Err(Failure {
                primary: "command unavailable or interrupted".into(),
                cleanup: self.poisoned.then(|| "previous command unresolved".into()),
            });
        }
        let mut command = Command::new(&spec.program);
        if spec.clear_env {
            command.env_clear();
        }
        command
            .args(&spec.args)
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn().map_err(|e| Failure {
            primary: format!("command spawn: {e}"),
            cleanup: None,
        })?;
        let pid = child.id() as i32;
        #[cfg(test)]
        self.timing.push(("spawned", Instant::now()));
        let mut input = child.stdin.take();
        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        let mut primary = None;
        for fd in [
            input.as_ref().unwrap().as_raw_fd(),
            stdout.as_ref().unwrap().as_raw_fd(),
            stderr.as_ref().unwrap().as_raw_fd(),
        ] {
            if let Err(e) = nonblocking(fd) {
                primary.get_or_insert(e);
            }
        }
        if primary.is_some() {
            // Never read a pipe whose O_NONBLOCK setup failed.
            input = None;
            stdout = None;
            stderr = None;
        }
        let deadline = Instant::now() + spec.timeout;
        let mut finish_deadline = None;
        let mut term_at = None;
        let mut killed = false;
        let mut out = vec![];
        let mut err = vec![];
        let mut stderr_overflow = false;
        let mut written = 0;
        let mut root_exited = false;
        let mut cleanup_error = None;
        // At most one TERM and one KILL. Later observation/deadline failures
        // must not overwrite either signaling diagnostic.
        let mut signal_errors = Vec::with_capacity(2);
        loop {
            let now = Instant::now();
            if self.interrupted(spec.cleanup) {
                primary.get_or_insert("command interrupted".into());
            }
            if now >= deadline && !root_exited {
                primary.get_or_insert("command deadline exceeded".into());
            }
            match exited(pid) {
                Ok(v) => {
                    #[cfg(test)]
                    if v && !root_exited {
                        self.timing.push(("root_exit_observed", Instant::now()));
                    }
                    root_exited = v;
                }
                Err(e) => {
                    primary.get_or_insert(e);
                }
            }
            if root_exited && written < spec.input.len() {
                primary.get_or_insert("command exited before accepting its complete input".into());
            }
            if root_exited || primary.is_some() {
                #[cfg(test)]
                if finish_deadline.is_none() {
                    self.timing.push(("cleanup_started", now));
                }
                finish_deadline.get_or_insert(now + Duration::from_secs(5));
                input = None;
            }
            if primary.is_none()
                && let Some(pipe) = input.as_mut()
            {
                if written == spec.input.len() {
                    input = None;
                } else {
                    let end = (written + 8192).min(spec.input.len());
                    match pipe.write(&spec.input[written..end]) {
                        Ok(0) => {
                            primary = Some("command input closed".into());
                        }
                        Ok(n) => written += n,
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            ) => {}
                        Err(e) => {
                            primary = Some(format!("command input: {e}"));
                        }
                    }
                }
            }
            if let Err(e) = drain(&mut stdout, &mut out, spec.stdout_limit, primary.is_some()) {
                primary.get_or_insert(e);
            }
            // Late proxy stderr remains useful after stdin EPIPE/root exit.
            // Keep draining even after the bounded retention capacity is full.
            if let Err(e) = drain(&mut stderr, &mut err, 256 * 1024, stderr_overflow) {
                stderr_overflow = true;
                primary.get_or_insert(e);
            }
            #[cfg(test)]
            for (name, eof) in [
                ("stdout_eof", stdout.is_none()),
                ("stderr_eof", stderr.is_none()),
            ] {
                if eof && !self.timing.iter().any(|(event, _)| *event == name) {
                    self.timing.push((name, Instant::now()));
                }
            }
            if root_exited || primary.is_some() {
                let group_deadline = *finish_deadline.get_or_insert(now + Duration::from_secs(5));
                let observed = native::group_running(pid, group_deadline);
                #[cfg(test)]
                let observed = if FAIL_OBSERVATION.load(Ordering::Relaxed) {
                    Err("injected process observation failure".into())
                } else {
                    observed
                };
                // A complete observation of no running members needs no
                // signal. Keep the leader unreaped until exit/pipe checks also
                // finish; errors/unknown observations still attempt cleanup.
                let signal_needed = !matches!(observed, Ok(false));
                #[cfg(test)]
                if !signal_needed
                    && !self
                        .timing
                        .iter()
                        .any(|(event, _)| *event == "group_not_running")
                {
                    self.timing.push(("group_not_running", Instant::now()));
                }
                match observed {
                    Ok(false) if root_exited && stdout.is_none() && stderr.is_none() => break,
                    Ok(true) if root_exited => {
                        // waitid keeps root identity reserved. The 40-ms reserve
                        // below remains a known, unqualified defect pending
                        // native macOS AND Linux escalation/reaping measurements.
                        // Do not replace it with another guessed number or expand
                        // the five-second outer authority. See the AG4 contract.
                        let natural = spec.eice_proxy
                            && match root_succeeded(pid) {
                                Ok(success) => success,
                                Err(e) => {
                                    primary.get_or_insert(e.clone());
                                    cleanup_error = Some(e);
                                    false
                                }
                            };
                        let escalation =
                            group_deadline - Duration::from_secs(1) - Duration::from_millis(40);
                        if !natural || now >= escalation {
                            primary.get_or_insert("command left running descendants".into());
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        primary.get_or_insert(e.clone());
                        cleanup_error = Some(e);
                    }
                }
                if signal_needed && primary.is_some() && term_at.is_none() {
                    #[cfg(test)]
                    self.timing.push(("term_sent", Instant::now()));
                    if let Err(error) = signal_group(pid, libc::SIGTERM) {
                        signal_errors.push(error);
                    }
                    term_at = Some(now);
                }
                if signal_needed
                    && term_at.is_some_and(|at| now.duration_since(at) >= Duration::from_secs(1))
                    && !killed
                {
                    #[cfg(test)]
                    self.timing.push(("kill_sent", Instant::now()));
                    if let Err(error) = signal_group(pid, libc::SIGKILL) {
                        signal_errors.push(error);
                    }
                    killed = true;
                }
                if now >= group_deadline {
                    cleanup_error = Some("command cleanup deadline exceeded".into());
                    break;
                }
            }
            let mut polls = Vec::with_capacity(3);
            if let Some(p) = &stdout {
                polls.push(libc::pollfd {
                    fd: p.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            if let Some(p) = &stderr {
                polls.push(libc::pollfd {
                    fd: p.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                });
            }
            if let Some(p) = &input {
                polls.push(libc::pollfd {
                    fd: p.as_raw_fd(),
                    events: libc::POLLOUT,
                    revents: 0,
                });
            }
            if unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as _, 20) } < 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                primary.get_or_insert("command pipe polling failed".into());
            }
        }
        // No blocking wait: root identity stays reserved until all signaling ends.
        let status = if root_exited {
            match child.try_wait() {
                Ok(Some(s)) => {
                    #[cfg(test)]
                    self.timing.push(("root_reaped", Instant::now()));
                    Some(s)
                }
                _ => {
                    cleanup_error = Some("command root could not be reaped".into());
                    None
                }
            }
        } else {
            None
        };
        // No raw transport output enters Failure, Output or qualification logs.
        let err = super::config::sanitize(&spec.env, &err, stderr_overflow).into_bytes();
        if !signal_errors.is_empty() {
            if let Some(error) = cleanup_error.take() {
                signal_errors.push(error);
            }
            cleanup_error = Some(signal_errors.join("; "));
        }
        if cleanup_error.is_none() {
            #[cfg(test)]
            self.timing
                .push(("descendant_reap_started", Instant::now()));
            if let Err(e) = self
                .native
                .reap(finish_deadline.unwrap_or_else(Instant::now))
            {
                cleanup_error = Some(e);
            } else {
                #[cfg(test)]
                self.timing.push(("descendants_reaped", Instant::now()));
            }
        }
        if cleanup_error.is_some() {
            self.poisoned = true;
        }
        if primary.is_some() || cleanup_error.is_some() {
            let mut primary = primary.unwrap_or_else(|| "command cleanup failed".into());
            if spec.failure_stderr_limit > 0 && !err.is_empty() {
                primary.push_str("; command stderr prefix: ");
                primary.push_str(&String::from_utf8_lossy(
                    &err[..err.len().min(spec.failure_stderr_limit)],
                ));
                if err.len() > spec.failure_stderr_limit {
                    primary.push_str("...[truncated]");
                }
            }
            return Err(Failure {
                primary,
                cleanup: cleanup_error,
            });
        }
        Ok(Output {
            stdout: out,
            stderr: err,
            code: status.and_then(|s| s.code()).unwrap_or(128),
        })
    }
}
