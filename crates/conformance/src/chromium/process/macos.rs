//! Qualified Darwin/libproc operations. Audit tokens select identities; they do
//! not grant permission or prove capture ownership. No raw-PID signal fallback.
use super::super::error::Error;
use super::{OwnershipBudget, OwnershipError};
use std::{collections::BTreeMap, ffi::CStr, path::PathBuf};
type Token = [u32; 8];
const PROC_UID_ONLY: u32 = 4; // sys/proc_info.h
type Signal = unsafe extern "C" fn(*const Token, i32) -> i32;
type TokenPath = unsafe extern "C" fn(*const Token, *mut libc::c_void, u32) -> i32;
unsafe extern "C" {
    static mach_task_self_: u32;
    fn task_name_for_pid(task: u32, pid: i32, name: *mut u32) -> i32;
    fn mach_port_deallocate(task: u32, port: u32) -> i32;
}
fn signal_api() -> Result<Signal, String> {
    let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"proc_signal_with_audittoken".as_ptr()) };
    if p.is_null() {
        return Err("proc_signal_with_audittoken unavailable".into());
    }
    // SAFETY: signature matches the installed libproc.h; no private syscall ABI.
    Ok(unsafe { std::mem::transmute::<*mut libc::c_void, Signal>(p) })
}
fn path_api() -> Result<TokenPath, String> {
    let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"proc_pidpath_audittoken".as_ptr()) };
    if p.is_null() {
        return Err("proc_pidpath_audittoken unavailable".into());
    }
    Ok(unsafe { std::mem::transmute::<*mut libc::c_void, TokenPath>(p) })
}
struct Identity {
    port: u32,
    token: Token,
}
impl Identity {
    fn acquire(pid: i32, budget: &OwnershipBudget<'_>) -> Result<Option<Self>, OwnershipError> {
        budget.check()?;
        let mut port = 0;
        let rc = unsafe { task_name_for_pid(mach_task_self_, pid, &mut port) };
        if rc != 0 {
            budget.check()?;
            return Err(format!("task_name_for_pid({pid}): Mach {rc}").into());
        }
        let mut identity = Self {
            port,
            token: [0; 8],
        };
        // Install the port owner before a post-call deadline check can fail.
        budget.check()?;
        identity.token = match budget.observe(|| identity.token())? {
            Ok(token) => token,
            Err(0x10000003) => return Ok(None), // MACH_SEND_INVALID_DEST: exit/exec race
            Err(rc) => return Err(format!("TASK_AUDIT_TOKEN: Mach {rc}").into()),
        };
        if identity.token[5] != pid as u32 {
            return Err("audit token PID mismatch".into());
        }
        Ok(Some(identity))
    }
    fn token(&self) -> Result<Token, i32> {
        let mut token = [0; 8];
        let mut count = 8;
        let rc = unsafe { libc::task_info(self.port, 15, token.as_mut_ptr().cast(), &mut count) };
        if rc != 0 || count != 8 {
            return Err(rc);
        }
        Ok(token)
    }
    fn path(&self, budget: &OwnershipBudget<'_>) -> Result<Option<PathBuf>, OwnershipError> {
        let mut bytes = [0_i8; 4096];
        let api = budget.observe(path_api)??;
        let rc = budget.observe(|| unsafe {
            api(&self.token, bytes.as_mut_ptr().cast(), bytes.len() as u32)
        })?;
        if rc <= 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ESRCH) {
                return Ok(None);
            }
            return Err(format!("audit-token process path: {e}").into());
        }
        if rc as usize >= bytes.len() {
            return Err("truncated executable path".into());
        }
        Ok(Some(PathBuf::from(
            unsafe { CStr::from_ptr(bytes.as_ptr()) }
                .to_string_lossy()
                .as_ref(),
        )))
    }
    fn alive(&self, budget: &OwnershipBudget<'_>) -> Result<bool, OwnershipError> {
        if self.path(budget)?.is_none() {
            return Ok(false);
        }
        let Some(info) = bsd(self.token[5] as i32, budget)? else {
            return Ok(false);
        };
        if info.pbi_status == 5 {
            return Ok(false);
        } // SZOMB: cannot execute or fork
        // Recheck after numeric-PID metadata access; no numeric-PID signaling.
        Ok(self.path(budget)?.is_some())
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        unsafe {
            mach_port_deallocate(mach_task_self_, self.port);
        }
    }
}

pub(super) fn preflight() -> Result<(), Error> {
    if !cfg!(target_arch = "aarch64") {
        return Err(Error::UnsupportedPlatform("macOS arm64 is required".into()));
    }
    let budget = OwnershipBudget {
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        cancel: None,
    };
    let check = || -> Result<(), OwnershipError> {
        let signal = budget.observe(signal_api)??;
        budget.observe(path_api)??;
        let identity = Identity::acquire(unsafe { libc::getpid() }, &budget)?
            .ok_or("self task identity unavailable")?;
        let mut stale = identity.token;
        stale[7] = stale[7].wrapping_add(1);
        if budget.observe(|| unsafe { signal(&stale, libc::SIGCONT) })? != libc::ESRCH {
            return Err("kernel did not reject a mismatched audit-token generation".into());
        }
        Ok(())
    };
    check().map_err(|e| e.execution(super::Phase::Startup))
}

fn bsd(
    pid: i32,
    budget: &OwnershipBudget<'_>,
) -> Result<Option<libc::proc_bsdinfo>, OwnershipError> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    let (rc, immediate_errno) = budget.observe(|| {
        #[cfg(test)]
        if ownership_probe::deny(pid) {
            unsafe { *libc::__error() = libc::EPERM };
            return (0, Some(libc::EPERM));
        }
        let rc = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        };
        (rc, std::io::Error::last_os_error().raw_os_error())
    })?;
    #[cfg(test)]
    ownership_probe::after_budget_check();
    let post_budget_error = std::io::Error::last_os_error();
    #[cfg(test)]
    ownership_probe::observe(pid, rc, immediate_errno, post_budget_error.raw_os_error());
    // Preserve the existing decision rule during the ownership investigation.
    // The immediate errno is evidence, not a new exit/permission classification.
    if rc == 0 && post_budget_error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(None);
    }
    if rc != size {
        return Err(format!(
            "incomplete BSD process information: {pid}, rc={rc}, {post_budget_error}; immediate_errno={immediate_errno:?}; post_budget_errno={:?}",
            post_budget_error.raw_os_error()
        )
        .into());
    }
    Ok(Some(info))
}
fn arguments(
    pid: i32,
    budget: &OwnershipBudget<'_>,
) -> Result<Option<Vec<Vec<u8>>>, OwnershipError> {
    budget.check()?;
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut data = vec![0_u8; 1024 * 1024];
    let mut size = data.len();
    let (rc, errno) = budget.observe(|| {
        #[cfg(test)]
        {
            use super::fault::{self, Point};
            if fault::take(Point::ArgumentsUnavailablePersistent) {
                fault::record_persistent_argument_eio_activation();
                fault::set(Point::ArgumentsUnavailablePersistent);
                return (-1, Some(libc::EIO));
            }
            if fault::take(Point::ArgumentsUnavailable) {
                return (-1, Some(libc::EIO));
            }
        }
        let rc = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                data.as_mut_ptr().cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        // Capture errno before the budget's post-call check performs native work.
        (
            rc,
            (rc != 0)
                .then(|| std::io::Error::last_os_error().raw_os_error())
                .flatten(),
        )
    })?;
    if rc != 0 && errno == Some(libc::EIO) {
        // Darwin can fail to copy argument memory during process exit/exec
        // before BSD metadata marks the process as a zombie. This is missing
        // evidence, not proof of exit or ownership. Require a later full scan.
        return Ok(None);
    }
    if rc != 0 || size < 5 || size >= data.len() {
        return Err(format!(
            "unavailable/truncated process arguments: {pid}, rc={rc}, size={size}, errno={errno:?}"
        )
        .into());
    }
    data.truncate(size);
    let argc = i32::from_ne_bytes(data[..4].try_into().unwrap());
    if !(1..=4096).contains(&argc) {
        return Err("invalid process argc".into());
    }
    let mut pos = 4
        + data[4..]
            .iter()
            .position(|b| *b == 0)
            .ok_or("missing executable terminator")?
        + 1;
    while data.get(pos) == Some(&0) {
        budget.check()?;
        pos += 1;
    }
    let mut result = Vec::new();
    for _ in 0..argc {
        budget.check()?;
        let end = pos
            + data
                .get(pos..)
                .ok_or("missing argv")?
                .iter()
                .position(|b| *b == 0)
                .ok_or("truncated argv")?;
        result.push(data[pos..end].to_vec());
        pos = end + 1;
    }
    budget.check()?;
    Ok(Some(result))
}

pub(super) struct Registry {
    root: i32,
    profile: PathBuf,
    crash: PathBuf,
    executable: PathBuf,
    identities: BTreeMap<(u32, u32), Identity>,
    complete_scan: bool,
}
impl Registry {
    pub fn diagnostic_summary(&self) -> (usize, bool) {
        (self.identities.len(), self.complete_scan)
    }
    pub fn new(root: i32, profile: PathBuf, crash: PathBuf, executable: PathBuf) -> Self {
        Self {
            root,
            profile,
            crash,
            executable,
            identities: BTreeMap::new(),
            complete_scan: false,
        }
    }
    pub fn discover(&mut self, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        self.complete_scan = false;
        #[cfg(test)]
        ownership_probe::discovery_entry(self.complete_scan);
        let mut complete = true;
        budget.check()?;
        let capacity = 65536;
        let mut pids = vec![0_i32; capacity];
        let bytes = budget.observe(|| unsafe {
            libc::proc_listpids(
                PROC_UID_ONLY,
                libc::getuid(),
                pids.as_mut_ptr().cast(),
                (capacity * 4) as i32,
            )
        })?;
        if bytes <= 0 || bytes as usize >= capacity * 4 || bytes % 4 != 0 {
            return Err("incomplete process enumeration".into());
        }
        for pid in pids.into_iter().take(bytes as usize / 4).filter(|p| *p > 0) {
            #[cfg(test)]
            super::fault::expire(super::fault::Point::Discovery, budget.deadline);
            budget.check()?;
            let Some(info) = bsd(pid, budget).map_err(|error| match error {
                OwnershipError::Inspection(message) => OwnershipError::Inspection(format!(
                    "{message}; candidate_previously_retained={}; scan_complete={}",
                    self.identities
                        .keys()
                        .any(|(retained_pid, _)| *retained_pid == pid as u32),
                    self.complete_scan
                )),
                other => other,
            })?
            else {
                continue;
            };
            if info.pbi_uid != unsafe { libc::getuid() } || info.pbi_status == 5 {
                continue;
            }
            let session_owned =
                pid == self.root || budget.observe(|| unsafe { libc::getsid(pid) })? == self.root;
            // First cheaply narrow out-of-session candidates by executable.
            // These reads are evidence only, never used for signaling.
            let mut path = [0_i8; 4096];
            let n = budget.observe(|| unsafe {
                libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32)
            })?;
            if n <= 0 {
                if bsd(pid, budget)?.is_none_or(|i| i.pbi_status == 5) {
                    continue;
                }
                // A live process without inspectable metadata makes this scan
                // incomplete, including a potentially detached Crashpad helper.
                return Err("live process executable unavailable during ownership scan".into());
            }
            if n as usize >= path.len() {
                return Err("truncated executable path during ownership scan".into());
            }
            let candidate = PathBuf::from(
                unsafe { CStr::from_ptr(path.as_ptr()) }
                    .to_string_lossy()
                    .as_ref(),
            );
            let bundle = self.executable.parent().and_then(Path::parent);
            let possible_helper = candidate
                .file_name()
                .is_some_and(|s| s == "chrome_crashpad_handler")
                && bundle.is_some_and(|p| candidate.starts_with(p.join("Frameworks")));
            if !session_owned && !possible_helper && candidate != self.executable {
                continue;
            }
            let identity = match Identity::acquire(pid, budget) {
                Ok(Some(i)) => i,
                Ok(None) => {
                    complete = false;
                    continue;
                }
                Err(e) => {
                    if bsd(pid, budget)?.is_none_or(|i| i.pbi_status == 5) {
                        continue;
                    }
                    return Err(e);
                }
            };
            let Some(exact_path) = identity.path(budget)? else {
                // A token can become stale on exec as well as process exit.
                if bsd(pid, budget)?.is_some_and(|i| i.pbi_status != 5) {
                    complete = false;
                }
                continue;
            };
            if exact_path != candidate {
                complete = false;
                continue;
            }
            let mut owned = session_owned;
            if !owned {
                let args = match arguments(pid, budget) {
                    Ok(Some(args)) => args,
                    Ok(None) => {
                        complete = false;
                        continue;
                    }
                    Err(e) => {
                        if !identity.alive(budget)? {
                            if bsd(pid, budget)?.is_some_and(|i| i.pbi_status != 5) {
                                complete = false;
                            }
                            continue;
                        }
                        return Err(e);
                    }
                };
                let database = format!("--database={}", self.crash.display()).into_bytes();
                let profile = format!("--user-data-dir={}", self.profile.display()).into_bytes();
                owned = (possible_helper && args.contains(&database))
                    || (candidate == self.executable && args.contains(&profile));
            }
            if !owned {
                continue;
            }
            if identity.path(budget)?.is_none() {
                if bsd(pid, budget)?.is_some_and(|i| i.pbi_status != 5) {
                    complete = false;
                }
                continue;
            }
            // Validate session membership again under the retained identity.
            if session_owned
                && pid != self.root
                && budget.observe(|| unsafe { libc::getsid(pid) })? != self.root
            {
                complete = false;
                continue;
            }
            self.identities
                .insert((identity.token[5], identity.token[7]), identity);
        }
        budget.check()?;
        self.complete_scan = complete;
        Ok(())
    }
    pub fn signal(&self, signal: i32, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        let send = budget.observe(signal_api)??;
        for identity in self.identities.values() {
            if !identity.alive(budget)? {
                continue;
            }
            let error = budget.observe(|| unsafe { send(&identity.token, signal) })?;
            if error != 0 && error != libc::ESRCH {
                return Err(format!(
                    "audit-token signal: {}",
                    std::io::Error::from_raw_os_error(error)
                )
                .into());
            }
        }
        Ok(())
    }
    pub fn terminated(&mut self, budget: &OwnershipBudget<'_>) -> Result<bool, OwnershipError> {
        budget.check()?;
        if !self.complete_scan {
            return Ok(false);
        }
        for identity in self.identities.values() {
            if identity.alive(budget)? {
                return Ok(false);
            }
        }
        // Fresh enumeration after known creators have terminated catches late
        // forks. The unreaped direct child still anchors the session ID.
        self.discover(budget)?;
        budget.check()?;
        if !self.complete_scan {
            return Ok(false);
        }
        for identity in self.identities.values() {
            if identity.alive(budget)? {
                return Ok(false);
            }
        }
        #[cfg(test)]
        super::fault::expire(super::fault::Point::Verification, budget.deadline);
        budget.check()?;
        Ok(true)
    }
    pub fn reap_adopted(&mut self, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        budget.check()
    }
    #[cfg(test)]
    pub fn topology(&mut self) -> Result<Vec<(String, bool)>, OwnershipError> {
        let budget = &OwnershipBudget {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
            cancel: None,
        };
        self.discover(budget)?;
        let mut rows = Vec::new();
        for identity in self.identities.values() {
            if identity.alive(budget)? {
                let path = identity
                    .path(budget)?
                    .ok_or("process exited during topology qualification")?;
                rows.push((
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    unsafe { libc::getsid(identity.token[5] as i32) } != self.root,
                ));
            }
        }
        rows.sort();
        Ok(rows)
    }
    #[cfg(test)]
    pub fn kill_root(&mut self) -> Result<(), OwnershipError> {
        let budget = &OwnershipBudget {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
            cancel: None,
        };
        self.discover(budget)?;
        let identity = self
            .identities
            .values()
            .rev()
            .find(|i| i.token[5] == self.root as u32 && i.alive(budget).unwrap_or(false))
            .ok_or("missing live root identity")?;
        let rc = unsafe { signal_api()?(&identity.token, libc::SIGKILL) };
        if rc != 0 {
            return Err(format!("root signal: {rc}").into());
        }
        Ok(())
    }
}
use std::path::Path;

// One isolated native regression, using the actual registry and owned launcher.
// This state and the synthetic denial do not exist in the normal executable.
#[cfg(test)]
mod ownership_probe {
    use super::*;
    use crate::chromium::process::{Cancellation, OwnedChromium, RootObservation, poll_fd};
    use std::{
        cell::Cell,
        io::{Read, Write},
        os::fd::AsRawFd,
        time::{Duration, Instant},
    };

    thread_local! {
        static WATCH: Cell<i32> = const { Cell::new(0) };
        static DENY: Cell<i32> = const { Cell::new(0) };
        static CALLS: Cell<usize> = const { Cell::new(0) };
        static ENTRY: Cell<Option<bool>> = const { Cell::new(None) };
        static POST_ERRNO: Cell<Option<i32>> = const { Cell::new(None) };
    }
    pub(super) fn after_budget_check() {
        if let Some(errno) = POST_ERRNO.with(|value| value.take()) {
            unsafe { *libc::__error() = errno };
        }
    }
    pub(super) fn deny(pid: i32) -> bool {
        DENY.with(|value| value.get() == pid)
    }
    pub(super) fn observe(pid: i32, rc: i32, immediate: Option<i32>, post: Option<i32>) {
        if WATCH.with(|value| value.get() == pid) {
            CALLS.with(|value| value.set(value.get() + 1));
            eprintln!(
                "BSD probe: pid={pid} api=proc_pidinfo flavor=PROC_PIDTBSDINFO arg=0 rc={rc} immediate_errno={immediate:?} post_budget_errno={post:?} synthetic={}",
                deny(pid)
            );
            assert_eq!(immediate, post, "budget check changed errno");
        }
    }
    pub(super) fn discovery_entry(complete: bool) {
        if WATCH.with(|value| value.get() != 0) {
            ENTRY.with(|value| value.set(Some(complete)));
        }
    }
    fn keys(registry: &Registry) -> Vec<(u32, u32)> {
        registry.identities.keys().copied().collect()
    }
    fn discover(registry: &mut Registry, budget: &OwnershipBudget<'_>, expected: bool) {
        let before = keys(registry);
        let was_complete = registry.complete_scan;
        ENTRY.with(|value| value.set(None));
        let result = registry.discover(budget);
        eprintln!(
            "BSD registry: before={before:?} prior_complete={was_complete} entry_complete={:?} result={result:?} after={:?} complete={}",
            ENTRY.with(Cell::get),
            keys(registry),
            registry.complete_scan
        );
        assert_eq!(ENTRY.with(Cell::get), Some(false));
        assert_eq!(result.is_ok(), expected, "{result:?}");
        if !expected {
            assert!(
                matches!(result, Err(OwnershipError::Inspection(ref message)) if message.contains("immediate_errno=Some(1)") && message.contains("post_budget_errno=Some(1)"))
            );
            assert_eq!(
                keys(registry),
                before,
                "denied candidate changed retained identities"
            );
            assert!(!registry.complete_scan);
            assert!(
                !registry.terminated(budget).unwrap(),
                "incomplete scan authorized termination"
            );
            eprintln!(
                "BSD registry: denied candidate not admitted; terminated=false; identities preserved"
            );
        }
    }
    struct Denial;
    impl Drop for Denial {
        fn drop(&mut self) {
            DENY.with(|value| value.set(0));
            POST_ERRNO.with(|value| value.set(None));
        }
    }
    #[test]
    fn bsd_failure_diagnostics_retain_immediate_errno() {
        let pid = unsafe { libc::getpid() };
        DENY.with(|value| value.set(pid));
        let _reset = Denial;
        // Synthetic diagnostic control: no claim that the platform clock
        // changed errno. Exercise bsd's real error path with distinct values.
        POST_ERRNO.with(|value| value.set(Some(libc::EIO)));
        let budget = OwnershipBudget {
            deadline: Instant::now() + Duration::from_secs(1),
            cancel: None,
        };
        let error = match bsd(pid, &budget) {
            Err(error) => error,
            Ok(_) => panic!("inspection denial became success"),
        };
        let message = error.to_string();
        assert!(matches!(error, OwnershipError::Inspection(_)));
        assert!(message.contains("rc=0"), "{message}");
        assert!(message.contains("immediate_errno=Some(1)"), "{message}");
        assert!(message.contains("post_budget_errno=Some(5)"), "{message}");
        eprintln!("synthetic errno diagnostic: {message}");
    }
    fn deny_discovery(registry: &mut Registry, pid: i32, budget: &OwnershipBudget<'_>) {
        DENY.with(|value| value.set(pid));
        let _reset = Denial;
        discover(registry, budget, false);
    }
    fn byte(child: &mut OwnedChromium, budget: &OwnershipBudget<'_>) -> u8 {
        loop {
            budget.check().unwrap();
            let mut byte = [0];
            match child.output.read(&mut byte) {
                Ok(1) => return byte[0],
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    poll_fd(child.output.as_raw_fd(), libc::POLLIN, budget.deadline).unwrap();
                }
                other => panic!("ownership handshake: {other:?}"),
            }
        }
    }
    impl OwnedChromium {
        pub(in crate::chromium) fn test_macos_bsd_ownership_states() {
            // Four cases once each. Reserve cleanup time inside the one total
            // cap; native calls retain their documented non-interruptibility.
            let start = Instant::now();
            let end = start + Duration::from_secs(60);
            let cancel = Cancellation::default();
            let exe = std::env::current_exe().unwrap();
            let sibling_pid = std::env::var("AG2_UNRELATED_PID").unwrap().parse().unwrap();
            let sibling = Identity::acquire(
                sibling_pid,
                &OwnershipBudget {
                    deadline: end,
                    cancel: Some(&cancel),
                },
            )
            .unwrap()
            .unwrap();
            let args = [
                "--exact",
                "chromium::tests::browser_helper",
                "--ignored",
                "--nocapture",
                "--",
            ]
            .map(str::to_owned);
            eprintln!(
                "BSD probe: native EPERM reproduction unavailable; installed sandbox_init is unsupported and exposes no supported exact-query denial. Synthetic EPERM is propagation evidence only."
            );
            for case in [
                "live",
                "waitable-exit",
                "acknowledged-exec",
                "synthetic-denial",
            ] {
                let budget = OwnershipBudget {
                    deadline: (Instant::now() + Duration::from_secs(5))
                        .min(end - Duration::from_secs(5)),
                    cancel: Some(&cancel),
                };
                budget.check().unwrap();
                let mut child = Self::launch(&exe, &args, budget.deadline, &cancel).unwrap();
                let artifacts = child.artifact_path().to_owned();
                WATCH.with(|value| value.set(child.pid));
                CALLS.with(|value| value.set(0));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    assert_eq!(byte(&mut child, &budget), b'R');
                    let original = Identity::acquire(child.pid, &budget).unwrap().unwrap();
                    eprintln!(
                        "BSD case={case}: pid={} established_token={:?} path={:?} root={:?}",
                        child.pid,
                        original.token,
                        original.path(&budget).unwrap(),
                        child.failure_observation().0
                    );
                    if case == "synthetic-denial" {
                        // launch already performs discovery. A separate empty
                        // registry view exercises pre-admission rejection without
                        // discarding any of the launcher's retained authority.
                        let mut unclassified = Registry::new(
                            child.pid,
                            child.registry.profile.clone(),
                            child.registry.crash.clone(),
                            child.registry.executable.clone(),
                        );
                        deny_discovery(&mut unclassified, child.pid, &budget);
                        assert!(keys(&unclassified).is_empty());
                    }
                    discover(&mut child.registry, &budget, true);
                    let original_key = (original.token[5], original.token[7]);
                    assert!(keys(&child.registry).contains(&original_key));
                    let retained = keys(&child.registry);
                    // Discovery may be incomplete for another candidate even
                    // after admitting this live helper. Neither state permits
                    // termination success while the helper remains live.
                    assert!(!child.registry.terminated(&budget).unwrap());
                    match case {
                        "waitable-exit" => {
                            child.input.as_mut().unwrap().write_all(b"Q").unwrap();
                            // waitid is independent of libproc; WNOWAIT retains
                            // the child and anchors its PID until finish reaps it.
                            while !child.exited().unwrap() {
                                budget.check().unwrap();
                                poll_fd(child.output.as_raw_fd(), libc::POLLIN, budget.deadline)
                                    .unwrap();
                            }
                            assert!(matches!(
                                child.failure_observation().0,
                                RootObservation::Exited
                            ));
                            assert!(bsd(child.pid, &budget).unwrap().is_none());
                            // A waitable root does not complete the whole-user
                            // scan. Exercise missing candidate evidence once.
                            use super::super::fault::{self, Point};
                            fault::set(Point::ArgumentsUnavailable);
                            discover(&mut child.registry, &budget, true);
                            fault::assert_consumed();
                            assert!(!child.registry.complete_scan);
                            assert_eq!(keys(&child.registry), retained);
                            for identity in child.registry.identities.values() {
                                assert!(!identity.alive(&budget).unwrap());
                            }
                            assert!(!child.registry.terminated(&budget).unwrap());
                            assert!(!child.reaped);
                            eprintln!(
                                "BSD waitable-exit: WNOWAIT confirmed; retained identities not live; injected incomplete scan; terminated=false; root_reaped=false; retained identities unchanged"
                            );
                        }
                        "acknowledged-exec" => {
                            child.input.as_mut().unwrap().write_all(b"X").unwrap();
                            assert_eq!(byte(&mut child, &budget), b'E');
                            let replacement =
                                Identity::acquire(child.pid, &budget).unwrap().unwrap();
                            assert_eq!(replacement.token[5], original.token[5]);
                            assert_ne!(replacement.token[7], original.token[7]);
                            let stale_signal =
                                unsafe { signal_api().unwrap()(&original.token, libc::SIGCONT) };
                            eprintln!(
                                "BSD exec: old={:?} new={:?} stale_token_SIGCONT={stale_signal} old_path={:?}",
                                original.token,
                                replacement.token,
                                original.path(&budget).unwrap()
                            );
                            assert_eq!(stale_signal, libc::ESRCH);
                            assert_eq!(
                                keys(&child.registry),
                                retained,
                                "replacement admitted without discovery"
                            );
                            assert!(bsd(child.pid, &budget).unwrap().is_some());
                            discover(&mut child.registry, &budget, true);
                            assert!(keys(&child.registry).contains(&original_key));
                            assert!(
                                keys(&child.registry)
                                    .contains(&(replacement.token[5], replacement.token[7]))
                            );
                            assert!(!child.registry.terminated(&budget).unwrap());
                            eprintln!(
                                "BSD exec: fresh discovery admitted new generation; old entry retained; terminated=false"
                            );
                        }
                        "synthetic-denial" => {
                            deny_discovery(&mut child.registry, child.pid, &budget);
                            assert_eq!(keys(&child.registry), retained);
                        }
                        "live" => {
                            assert!(bsd(child.pid, &budget).unwrap().is_some());
                            assert_eq!(keys(&child.registry), retained);
                            eprintln!("BSD live: admitted; identities unchanged; terminated=false");
                        }
                        _ => unreachable!(),
                    }
                    assert!(CALLS.with(Cell::get) > 0, "observer not reached");
                }));
                // Clear test faults before cleanup, including on assertion failure.
                DENY.with(|value| value.set(0));
                let root = child.failure_observation().0;
                let cleanup = child.finish(
                    Duration::from_secs(5).min(end.saturating_duration_since(Instant::now())),
                );
                eprintln!(
                    "BSD case={case}: root_before_cleanup={root:?} cleanup={cleanup:?} {} artifacts_exist={}",
                    child.cleanup_diagnostic(),
                    artifacts.exists()
                );
                WATCH.with(|value| value.set(0));
                assert!(cleanup.is_empty(), "{cleanup:?}");
                assert!(child.registry.complete_scan);
                assert!(child.reaped);
                assert!(child.cleanup_progress.owned_terminated);
                assert!(!artifacts.exists());
                assert!(
                    sibling
                        .alive(&OwnershipBudget {
                            deadline: end,
                            cancel: Some(&cancel)
                        })
                        .unwrap()
                );
                eprintln!(
                    "BSD case={case}: unrelated sibling retained token {:?} still live",
                    sibling.token
                );
                if let Err(panic) = result {
                    std::panic::resume_unwind(panic);
                }
            }
            assert!(Instant::now() < end);
            eprintln!(
                "BSD probe completed once: elapsed={:?}; total_cap=60s",
                start.elapsed()
            );
        }
    }
}
