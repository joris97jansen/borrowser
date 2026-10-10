use super::super::error::Error;
use super::{OwnershipBudget, OwnershipError};
use std::{
    collections::BTreeMap,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
};

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
        let fd = budget.observe(|| pidfd(unsafe { libc::getpid() }))??;
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
fn pidfd(pid: i32) -> Result<OwnedFd, String> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().to_string());
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
        return Err(std::io::Error::last_os_error().to_string());
    }
    if poll.revents & libc::POLLNVAL != 0 {
        return Err("invalid retained pidfd".into());
    }
    Ok(poll.revents & (libc::POLLIN | libc::POLLHUP) == 0)
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
            .map_err(|e| e.to_string())?
            .enumerate();
        while let Some((count, entry)) = budget.observe(|| entries.next())? {
            #[cfg(test)]
            super::fault::expire(super::fault::Point::Discovery, budget.deadline);
            budget.check()?;
            if count >= 65536 {
                return Err("process enumeration exceeds bound".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
            else {
                continue;
            };
            let read = || budget.observe(|| std::fs::read_to_string(entry.path().join("stat")));
            let stat = match read()? {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.to_string().into()),
            };
            let (parent, session, birth) = ownership(&stat)?;
            if pid != self.root && parent != unsafe { libc::getpid() } && session != self.root {
                continue;
            }
            if let Some(fd) = self.processes.get(&pid)
                && budget.observe(|| alive(fd))??
            {
                continue;
            }
            let fd = match budget.observe(|| pidfd(pid))? {
                Ok(fd) => fd,
                Err(e) => {
                    if read()?.is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
                        continue;
                    }
                    return Err(e.into());
                }
            };
            if !budget.observe(|| alive(&fd))?? {
                continue;
            }
            // Re-read ownership after opening the stable identity.
            let again = match read()? {
                Ok(stat) => stat,
                Err(_) if !budget.observe(|| alive(&fd))?? => continue,
                Err(e) => return Err(e.to_string().into()),
            };
            if ownership(&again)? != (parent, session, birth) {
                complete = false;
                continue;
            }
            if !budget.observe(|| alive(&fd))?? {
                continue;
            }
            self.processes.insert(pid, fd);
        }
        budget.check()?;
        self.complete_scan = complete;
        Ok(())
    }
    pub fn signal(&self, signal: i32, budget: &OwnershipBudget<'_>) -> Result<(), OwnershipError> {
        budget.check()?;
        for fd in self.processes.values() {
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
                return Err(std::io::Error::last_os_error().to_string().into());
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
            return Err("remaining owned children could not be reaped".into());
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
