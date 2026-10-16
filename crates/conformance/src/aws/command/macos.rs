use std::time::Instant;
pub(super) struct Scope;
impl Scope {
    pub fn enter() -> Result<Self, String> {
        Ok(Self)
    }
    pub fn reap(&self, _deadline: Instant) -> Result<(), String> {
        self.quiescent()
    }
    pub fn quiescent(&self) -> Result<(), String> {
        let rc = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
        if rc == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD) {
            Ok(())
        } else {
            Err("AG4 direct child unresolved".into())
        }
    }
}
pub(super) fn group_running(group: i32, deadline: Instant) -> Result<bool, String> {
    // PROC_PGRP_ONLY, from the public libproc headers. One extra slot detects overflow.
    let mut pids = [0_i32; 65];
    let bytes = unsafe {
        libc::proc_listpids(
            2,
            group as u32,
            pids.as_mut_ptr().cast(),
            std::mem::size_of_val(&pids) as i32,
        )
    };
    if bytes <= 0
        || bytes as usize >= std::mem::size_of_val(&pids)
        || !(bytes as usize).is_multiple_of(4)
    {
        return Err("incomplete command process-group observation".into());
    }
    let mut running = false;
    for pid in pids
        .into_iter()
        .take(bytes as usize / 4)
        .filter(|pid| *pid > 0)
    {
        if Instant::now() >= deadline {
            return Err("process observation deadline".into());
        }
        let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
        let rc = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                std::mem::size_of_val(&info) as i32,
            )
        };
        if rc == 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            continue;
        }
        if rc as usize != std::mem::size_of_val(&info) {
            return Err("incomplete command member observation".into());
        }
        if info.pbi_pgid != group as u32 {
            return Err("conflicting command process group".into());
        }
        if info.pbi_status != 5 {
            running = true;
        }
    }
    Ok(running)
}
