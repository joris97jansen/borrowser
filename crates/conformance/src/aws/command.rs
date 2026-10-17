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
#[cfg(test)]
static EXHAUST_OBSERVATION_ONCE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
static FAIL_TERM_ONCE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn interrupt(_: libc::c_int) {
    INTERRUPTS.fetch_add(1, Ordering::Relaxed);
}

const CLEANUP_LIMIT: Duration = Duration::from_secs(5);
const NATURAL_LIMIT: Duration = Duration::from_secs(2);
const TERM_LIMIT: Duration = Duration::from_secs(1);
const FORCED_ALLOCATION: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

// Candidate AG4 budgets: native qualification must establish one second of
// headroom. These are not a scheduler or a promise about OS scheduling latency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CleanupPhase {
    Natural { until: Instant },
    TermPending,
    TermGrace { until: Instant },
    Forced,
    Finalization,
}
struct Cleanup {
    hard_end: Instant,
    force_by: Instant,
    phase: CleanupPhase,
}
impl Cleanup {
    fn new(start: Instant, natural: bool) -> Self {
        debug_assert_eq!(
            NATURAL_LIMIT + TERM_LIMIT + FORCED_ALLOCATION,
            CLEANUP_LIMIT
        );
        Self {
            hard_end: start + CLEANUP_LIMIT,
            force_by: start + NATURAL_LIMIT + TERM_LIMIT,
            phase: if natural {
                CleanupPhase::Natural {
                    until: start + NATURAL_LIMIT,
                }
            } else {
                CleanupPhase::TermPending
            },
        }
    }
    fn end_natural(&mut self, now: Instant, failed: bool) -> bool {
        if matches!(self.phase, CleanupPhase::Natural { until } if failed || now >= until) {
            self.phase = CleanupPhase::TermPending;
            return true;
        }
        false
    }
    fn term_dispatched(&mut self, now: Instant) {
        debug_assert_eq!(self.phase, CleanupPhase::TermPending);
        self.phase = CleanupPhase::TermGrace {
            until: (now + TERM_LIMIT).min(self.force_by),
        };
    }
    fn kill_due(&self, now: Instant) -> bool {
        matches!(self.phase, CleanupPhase::TermGrace { until } if now >= until)
    }
    fn boundary(&self) -> Instant {
        match self.phase {
            CleanupPhase::Natural { until } | CleanupPhase::TermGrace { until } => until,
            CleanupPhase::TermPending => self.force_by,
            CleanupPhase::Forced | CleanupPhase::Finalization => self.hard_end,
        }
    }
    fn check_deadline(&self, now: Instant) -> Result<(), String> {
        if now >= self.hard_end {
            Err("command cleanup deadline exceeded".into())
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupObservation {
    Executing,
    // A complete scan, while the unreaped root reserves PGID identity. This
    // does not assert that the numeric group has disappeared.
    NoExecuting,
    DeadlineReached,
}
fn poll_timeout(now: Instant, boundary: Instant) -> i32 {
    // Floor sub-millisecond waits instead of knowingly crossing a boundary.
    POLL_INTERVAL
        .min(boundary.saturating_duration_since(now))
        .as_millis() as i32
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
    #[cfg(test)]
    if signal == libc::SIGTERM && FAIL_TERM_ONCE.swap(false, Ordering::Relaxed) {
        return group_signal_result(
            "SIGTERM",
            Some(std::io::Error::from_raw_os_error(libc::EPERM)),
        );
    }
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
        let mut cleanup: Option<Cleanup> = None;
        let mut observed_group = GroupObservation::DeadlineReached;
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
            if (root_exited || primary.is_some()) && cleanup.is_none() {
                let start = Instant::now();
                let natural = spec.eice_proxy
                    && root_exited
                    && primary.is_none()
                    && match root_succeeded(pid) {
                        Ok(success) => success,
                        Err(error) => {
                            primary.get_or_insert(error.clone());
                            cleanup_error.get_or_insert(error);
                            false
                        }
                    };
                #[cfg(test)]
                self.timing.push(("cleanup_started", start));
                cleanup = Some(Cleanup::new(start, natural));
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
            // A pipe error can be the first failure in this iteration.
            if primary.is_some() && cleanup.is_none() {
                let start = Instant::now();
                #[cfg(test)]
                self.timing.push(("cleanup_started", start));
                cleanup = Some(Cleanup::new(start, false));
                input = None;
            }
            if let Some(state) = &mut cleanup {
                if state.end_natural(Instant::now(), primary.is_some()) {
                    #[cfg(test)]
                    self.timing.push(("natural_phase_ended", Instant::now()));
                    primary.get_or_insert("command left running descendants".into());
                }
                // An overdue transition precedes another scan. The last complete
                // NoExecuting observation remains valid while the root is held:
                // no executing group member remains to create more members.
                if state.kill_due(Instant::now()) {
                    if observed_group != GroupObservation::NoExecuting {
                        #[cfg(test)]
                        self.timing.push(("kill_sent", Instant::now()));
                        if let Err(error) = signal_group(pid, libc::SIGKILL) {
                            signal_errors.push(error);
                        }
                    }
                    state.phase = CleanupPhase::Forced;
                }
                if let Err(error) = state.check_deadline(Instant::now()) {
                    cleanup_error.get_or_insert(error);
                    break;
                }
                #[cfg(test)]
                self.timing.push(("group_observation", Instant::now()));
                #[cfg(test)]
                if EXHAUST_OBSERVATION_ONCE.swap(false, Ordering::Relaxed) {
                    std::thread::sleep(state.boundary().saturating_duration_since(Instant::now()));
                }
                let observed = native::group_running(pid, state.boundary());
                #[cfg(test)]
                let observed = if FAIL_OBSERVATION.load(Ordering::Relaxed) {
                    Err("injected process observation failure".into())
                } else {
                    observed
                };
                observed_group = match observed {
                    Ok(observation) => observation,
                    Err(error) => {
                        primary.get_or_insert(error.clone());
                        cleanup_error.get_or_insert(error);
                        GroupObservation::DeadlineReached
                    }
                };
                // Observation may have consumed the current phase. Never use
                // the timestamp from before the scan for escalation/completion.
                if state.end_natural(Instant::now(), primary.is_some()) {
                    #[cfg(test)]
                    self.timing.push(("natural_phase_ended", Instant::now()));
                    primary.get_or_insert("command left running descendants".into());
                }
                if let Err(error) = state.check_deadline(Instant::now()) {
                    cleanup_error.get_or_insert(error);
                    break;
                }
                if observed_group == GroupObservation::NoExecuting
                    && root_exited
                    && stdout.is_none()
                    && stderr.is_none()
                {
                    #[cfg(test)]
                    {
                        self.timing
                            .push(("group_quiescence_confirmed", Instant::now()));
                        if matches!(state.phase, CleanupPhase::Natural { .. }) {
                            self.timing.push(("natural_phase_ended", Instant::now()));
                        }
                        self.timing.push(("signaling_complete", Instant::now()));
                    }
                    // Irreversible boundary: after root reap, no PGID lookup or
                    // signal can authorize success. Only kernel child ownership
                    // remains available to the native reaper.
                    state.phase = CleanupPhase::Finalization;
                    break;
                }
                if observed_group == GroupObservation::Executing
                    && root_exited
                    && !matches!(state.phase, CleanupPhase::Natural { .. })
                {
                    primary.get_or_insert("command left running descendants".into());
                }
                if state.phase == CleanupPhase::TermPending {
                    if observed_group != GroupObservation::NoExecuting {
                        let sent = Instant::now();
                        #[cfg(test)]
                        self.timing.push(("term_sent", sent));
                        if let Err(error) = signal_group(pid, libc::SIGTERM) {
                            signal_errors.push(error);
                        }
                        state.term_dispatched(sent);
                    } else {
                        // No group signal is needed; unresolved pipes still
                        // have only the original hard cleanup authority.
                        state.phase = CleanupPhase::Forced;
                    }
                }
                if state.kill_due(Instant::now()) {
                    if observed_group != GroupObservation::NoExecuting {
                        #[cfg(test)]
                        self.timing.push(("kill_sent", Instant::now()));
                        if let Err(error) = signal_group(pid, libc::SIGKILL) {
                            signal_errors.push(error);
                        }
                    }
                    state.phase = CleanupPhase::Forced;
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
            let boundary = cleanup.as_ref().map_or(deadline, Cleanup::boundary);
            let wait = poll_timeout(Instant::now(), boundary);
            // EINTR returns to the loop: observations and absolute deadlines are
            // recomputed, including when the poll set is empty.
            if unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as _, wait) } < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    #[cfg(test)]
                    self.timing.push(("poll_interrupted", Instant::now()));
                } else {
                    primary.get_or_insert("command pipe polling failed".into());
                }
            }
        }
        let mut status = None;
        if let Some(state) = cleanup
            .as_ref()
            .filter(|s| s.phase == CleanupPhase::Finalization)
        {
            // No blocking wait. All authoritative PGID work ended above while
            // the root was unreaped; nothing below may look up or signal PGID.
            match state.check_deadline(Instant::now()) {
                Err(error) => {
                    cleanup_error.get_or_insert(error);
                }
                Ok(()) => match child.try_wait() {
                    Ok(Some(s)) => {
                        #[cfg(test)]
                        self.timing.push(("root_reaped", Instant::now()));
                        status = Some(s);
                    }
                    _ => {
                        cleanup_error.get_or_insert("command root could not be reaped".into());
                    }
                },
            }
            if status.is_some() {
                #[cfg(test)]
                self.timing
                    .push(("descendant_reap_started", Instant::now()));
                if let Err(error) = self.native.reap(state.hard_end) {
                    cleanup_error.get_or_insert(error);
                } else {
                    #[cfg(test)]
                    self.timing.push(("descendants_reaped", Instant::now()));
                }
            }
            let completed = Instant::now();
            if let Err(error) = state.check_deadline(completed) {
                cleanup_error.get_or_insert(error);
            }
            #[cfg(test)]
            if cleanup_error.is_none() && signal_errors.is_empty() {
                self.timing.push(("owner_quiescent", completed));
            }
        } else {
            cleanup_error.get_or_insert("command ownership unresolved".into());
        }
        if !signal_errors.is_empty() {
            if let Some(error) = cleanup_error.take() {
                signal_errors.push(error);
            }
            cleanup_error = Some(signal_errors.join("; "));
        }
        // Ownership finalization precedes string processing. No raw transport
        // output enters Failure, Output or qualification logs: sanitization still
        // precedes every diagnostic excerpt and persistence boundary.
        let err = super::config::sanitize(&spec.env, &err, stderr_overflow).into_bytes();
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
