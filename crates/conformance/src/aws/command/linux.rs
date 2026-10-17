use super::GroupObservation;
use std::{
    fs,
    io::Read,
    time::{Duration, Instant},
};
pub(super) struct Scope {
    old: i32,
}
impl Scope {
    pub fn enter() -> Result<Self, String> {
        let mut old = 0;
        if unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut old, 0, 0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } != 0
        {
            return Err("cannot establish AG4 child subreaper".into());
        }
        Ok(Self { old })
    }
    pub fn reap(&self, deadline: Instant) -> Result<(), String> {
        loop {
            if Instant::now() >= deadline {
                return Err("AG4 adopted descendants unresolved before cleanup deadline".into());
            }
            let rc = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
            let error = (rc < 0).then(std::io::Error::last_os_error);
            if Instant::now() >= deadline {
                return Err("AG4 adopted descendants unresolved before cleanup deadline".into());
            }
            if error.as_ref().and_then(std::io::Error::raw_os_error) == Some(libc::ECHILD) {
                return Ok(());
            }
            if error
                .as_ref()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::Interrupted)
            {
                continue;
            }
            if rc < 0 {
                return Err("AG4 adopted descendants unresolved".into());
            }
            if rc == 0 {
                std::thread::sleep(
                    Duration::from_millis(5)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
    }
    pub fn quiescent(&self) -> Result<(), String> {
        self.reap(Instant::now() + Duration::from_millis(100))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, self.old, 0, 0, 0) };
    }
}
pub(super) fn group_running(group: i32, deadline: Instant) -> Result<GroupObservation, String> {
    if Instant::now() >= deadline {
        return Ok(GroupObservation::DeadlineReached);
    }
    let mut count = 0;
    let mut members = 0;
    let mut running = false;
    for entry in fs::read_dir("/proc").map_err(|e| e.to_string())? {
        count += 1;
        if count > 65536 {
            return Err("bounded process observation incomplete".into());
        }
        if Instant::now() >= deadline {
            return Ok(GroupObservation::DeadlineReached);
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
            .is_none()
        {
            continue;
        }
        let file = match fs::File::open(entry.path().join("stat")) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        let mut s = String::new();
        file.take(8193)
            .read_to_string(&mut s)
            .map_err(|e| e.to_string())?;
        if s.len() > 8192 {
            return Err("truncated proc stat".into());
        }
        let fields: Vec<_> = s
            .rsplit_once(')')
            .ok_or("invalid proc stat")?
            .1
            .split_whitespace()
            .collect();
        if fields.len() < 4 {
            return Err("incomplete proc stat".into());
        }
        let pg: i32 = fields[2].parse().map_err(|_| "invalid process group")?;
        if pg == group {
            members += 1;
            if members > 64 {
                return Err("command group exceeds 64 members".into());
            }
            if fields[0] != "Z" && fields[0] != "X" {
                running = true;
            }
        }
    }
    Ok(if Instant::now() >= deadline {
        GroupObservation::DeadlineReached
    } else if running {
        GroupObservation::Executing
    } else {
        GroupObservation::NoExecuting
    })
}
