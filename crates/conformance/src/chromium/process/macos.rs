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
    let rc = budget.observe(|| unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    })?;
    if rc == 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        return Ok(None);
    }
    if rc != size {
        return Err(format!(
            "incomplete BSD process information: {pid}, rc={rc}, {}",
            std::io::Error::last_os_error()
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
            let Some(info) = bsd(pid, budget)? else {
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
