use super::super::error::Error;
use super::{OwnershipBudget, OwnershipError};
use std::{
    collections::BTreeMap,
    io::Read,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
};

#[cfg(test)]
mod tests;

pub(super) fn preflight() -> Result<(), Error> {
    if cfg!(not(target_arch = "x86_64")) {
        return Err(Error::UnsupportedPlatform(
            "Linux x86-64 is required".into(),
        ));
    }
    let budget = OwnershipBudget {
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        cancel: None,
    };
    let check = || -> Result<(), OwnershipError> {
        // Only standalone Chromium execution calls this; no unrelated children
        // or competing waiters are allowed in that process.
        if budget.observe(|| unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) })? != 0
        {
            return Err(super::last("enable child subreaper").to_string().into());
        }
        let pid = unsafe { libc::getpid() };
        let fd = budget
            .observe(|| pidfd(pid))?
            .map_err(|e| format!("pidfd_open capability pid={pid}: {e}"))?;
        let rc = budget.observe(|| unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                0,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        })?;
        if rc != 0 {
            return Err(super::last("pidfd signal capability").to_string().into());
        }
        Ok(())
    };
    check().map_err(|e| e.execution(super::Phase::Startup))
}
fn pidfd(pid: i32) -> std::io::Result<OwnedFd> {
    #[cfg(test)]
    tests::at(tests::Step::PidfdOpening, pid, None)?;
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}
fn alive(fd: &OwnedFd) -> Result<bool, String> {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    if unsafe { libc::poll(&mut poll, 1, 0) } < 0 {
        return Err(format!(
            "poll retained pidfd fd={}: {}",
            fd.as_raw_fd(),
            std::io::Error::last_os_error()
        ));
    }
    if poll.revents & libc::POLLNVAL != 0 {
        return Err("invalid retained pidfd".into());
    }
    Ok(poll.revents & (libc::POLLIN | libc::POLLHUP) == 0)
}
fn read_stat(pid: i32) -> std::io::Result<String> {
    let mut file = std::fs::File::open(format!("/proc/{pid}/stat"))?;
    #[cfg(test)]
    tests::at(tests::Step::StatOpened, pid, Some(&mut file))?;
    let mut stat = String::new();
    file.read_to_string(&mut stat)?;
    Ok(stat)
}
// Only proc-stat observation uses ENOENT/ESRCH as absence. A proc descriptor
// can open successfully and then return ESRCH after the task is reaped.
fn stat_disappeared(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ESRCH)
}
fn ownership(stat: &str) -> Result<(i32, i32, u64), String> {
    let fields: Vec<_> = stat
        .rsplit_once(')')
        .ok_or("invalid proc stat")?
        .1
        .split_whitespace()
        .collect();
    if fields.len() < 20 {
        return Err("truncated proc stat".into());
    }
    Ok((
        fields[1].parse().map_err(|_| "invalid parent")?,
        fields[3].parse().map_err(|_| "invalid session")?,
        fields[19].parse().map_err(|_| "invalid birth identity")?,
    ))
}
pub(super) struct Registry {
    root: i32,
    processes: BTreeMap<i32, OwnedFd>,
    complete_scan: bool,
}
impl Registry {
    pub fn new(root: i32, _: PathBuf, _: PathBuf, _: PathBuf) -> Self {
        Self {
            root,
            processes: BTreeMap::new(),
            complete_scan: false,
        }
    }
    pub fn discover(&mut self, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        self.complete_scan = false;
        let mut complete = true;
        let mut entries = budget
            .observe(|| std::fs::read_dir("/proc"))?
            .map_err(|e| format!("discover read_dir /proc: {e}"))?
            .enumerate();
        while let Some((count, entry)) = budget.observe(|| entries.next())? {
            #[cfg(test)]
            super::fault::expire(super::fault::Point::Discovery, budget.deadline);
            budget.check()?;
            if count >= 65536 {
                return Err("process enumeration exceeds bound".into());
            }
            let entry = entry.map_err(|e| format!("discover enumerate /proc: {e}"))?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
            else {
                continue;
            };
            complete &= self.observe_pid(pid, budget)?;
        }
        budget.check()?;
        self.complete_scan = complete;
        Ok(())
    }
    // true means this entry was completely observed (including disappearance).
    // false requests a fresh scan after an identity changed during acquisition.
    fn observe_pid(
        &mut self,
        pid: i32,
        budget: &OwnershipBudget<'_>,
    ) -> Result<bool, OwnershipError> {
        let read = || budget.observe(|| read_stat(pid));
        let stat = match read()? {
            Ok(s) => s,
            Err(e) if stat_disappeared(&e) => return Ok(true),
            Err(e) => return Err(format!("discover read /proc/{pid}/stat: {e}").into()),
        };
        let (parent, session, birth) =
            ownership(&stat).map_err(|e| format!("discover parse /proc/{pid}/stat: {e}"))?;
        if pid != self.root && parent != unsafe { libc::getpid() } && session != self.root {
            return Ok(true);
        }
        if let Some(fd) = self.processes.get(&pid)
            && budget.observe(|| alive(fd))??
        {
            return Ok(true);
        }
        let fd = match budget.observe(|| pidfd(pid))? {
            Ok(fd) => fd,
            // No stable identity was acquired. ESRCH says this entry ceased
            // to be observable; other acquisition errors remain failures even
            // if a later proc read would happen to find the process gone.
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => return Ok(true),
            Err(e) => return Err(format!("pidfd_open pid={pid}: {e}").into()),
        };
        #[cfg(test)]
        tests::at(tests::Step::PidfdOpened, pid, None)
            .map_err(|e| format!("pidfd_open pid={pid}: {e}"))?;
        if !budget.observe(|| alive(&fd))?? {
            return Ok(true);
        }
        // Re-read ownership after opening the stable identity.
        let again = match read()? {
            Ok(stat) => stat,
            Err(e) if stat_disappeared(&e) && !budget.observe(|| alive(&fd))?? => return Ok(true),
            Err(e) => return Err(format!("revalidate read /proc/{pid}/stat: {e}").into()),
        };
        if ownership(&again).map_err(|e| format!("revalidate parse /proc/{pid}/stat: {e}"))?
            != (parent, session, birth)
        {
            return Ok(false);
        }
        if !budget.observe(|| alive(&fd))?? {
            return Ok(true);
        }
        self.processes.insert(pid, fd);
        Ok(true)
    }
    pub fn signal(&self, signal: i32, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        budget.check()?;
        for (pid, fd) in &self.processes {
            let result = budget.observe(|| unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    fd.as_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            })?;
            if result != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                return Err(format!(
                    "pidfd_send_signal pid={pid} signal={signal}: {}",
                    std::io::Error::last_os_error()
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
        for fd in self.processes.values() {
            if budget.observe(|| alive(fd))?? {
                return Ok(false);
            }
        }
        // With the root retained as a zombie, all remaining descendants must
        // have been adopted before it became waitable. Discover them again.
        self.discover(budget)?;
        if !self.complete_scan {
            return Ok(false);
        }
        for fd in self.processes.values() {
            if budget.observe(|| alive(fd))?? {
                return Ok(false);
            }
        }
        #[cfg(test)]
        super::fault::expire(super::fault::Point::Verification, budget.deadline);
        budget.check()?;
        Ok(true)
    }
    pub fn reap_adopted(&mut self, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        loop {
            let mut status = 0;
            budget.check()?;
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
            let error = std::io::Error::last_os_error();
            #[cfg(test)]
            super::fault::expire(super::fault::Point::AdoptedReap, budget.deadline);
            budget.check()?;
            if pid > 0 {
                continue;
            }
            if pid < 0 && error.raw_os_error() == Some(libc::ECHILD) {
                return Ok(());
            }
            if pid < 0 && error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(if pid < 0 {
                format!("reap adopted waitpid(-1, WNOHANG): {error}").into()
            } else {
                "reap adopted waitpid(-1, WNOHANG): child remains live".into()
            });
        }
    }
    #[cfg(test)]
    pub fn kill_root(&mut self) -> Result<(), String> {
        let budget = OwnershipBudget {
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
            cancel: None,
        };
        self.discover(&budget).map_err(|e| e.to_string())?;
        let fd = self
            .processes
            .get(&self.root)
            .ok_or("missing root identity")?;
        let rc = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}
