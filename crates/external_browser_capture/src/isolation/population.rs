//! Quiescent membership of the owned PID namespace, never a host process search.
use crate::{CaptureError as E, Result, deadline::AttemptDeadline};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::fs::MetadataExt,
};
const MAX_PROCESSES: usize = 256;
const MAX_ROUNDS: usize = 256;

struct Member {
    pidfd: OwnedFd,
    directory: File,
    start: u64,
}
fn file_at(directory: &File, name: &str) -> Result<File> {
    rustix::fs::openat(
        directory,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| E::ProcessIdentity)
}
fn text(directory: &File, name: &str) -> Result<String> {
    let mut value = String::new();
    file_at(directory, name)?
        .take(65537)
        .read_to_string(&mut value)
        .map_err(|_| E::ProcessIdentity)?;
    if value.len() > 65536 {
        return Err(E::Limit);
    }
    Ok(value)
}
fn start(directory: &File) -> Result<u64> {
    text(directory, "stat")?
        .rsplit_once(") ")
        .ok_or(E::ProcessIdentity)?
        .1
        .split_whitespace()
        .nth(19)
        .ok_or(E::ProcessIdentity)?
        .parse()
        .map_err(|_| E::ProcessIdentity)
}
fn exited(fd: &OwnedFd) -> Result<bool> {
    let mut p = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    match unsafe { libc::poll(&mut p, 1, 0) } {
        0 => Ok(false),
        1 if p.revents & libc::POLLIN != 0 => Ok(true),
        _ => Err(E::ProcessIdentity),
    }
}
pub(super) fn ids(proc: &File) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    let entries = rustix::fs::Dir::read_from(proc).map_err(|_| E::ProcessIdentity)?;
    let mut count = 0;
    for entry in entries {
        count += 1;
        if count > 1024 {
            return Err(E::Limit);
        }
        let entry = entry.map_err(|_| E::ProcessIdentity)?;
        if let Ok(id) = entry.file_name().to_str().unwrap_or("").parse::<i32>() {
            if id != 1 {
                ids.push(id);
            }
            if ids.len() > MAX_PROCESSES {
                return Err(E::Limit);
            }
        }
    }
    ids.sort_unstable();
    Ok(ids)
}
fn acquire(proc: &File, pid: i32) -> Result<Option<Member>> {
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    if fd < 0 {
        return if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            Ok(None)
        } else {
            Err(E::ProcessIdentity)
        };
    }
    let pidfd = unsafe { OwnedFd::from_raw_fd(fd) };
    // PID may be recycled despite pidfd ownership. Check the old pidfd after
    // opening proc: if it exited, never adopt a directory for its replacement.
    let directory = file_at(proc, &pid.to_string());
    if exited(&pidfd)? {
        return Ok(None);
    }
    let directory = directory?;
    let start = start(&directory)?;
    if exited(&pidfd)? {
        return Ok(None);
    }
    Ok(Some(Member {
        pidfd,
        directory,
        start,
    }))
}
fn stopped(member: &Member) -> Result<bool> {
    // Group stop must complete for every thread, not just its leader.
    let task = file_at(&member.directory, "task")?;
    let mut n = 0;
    for entry in rustix::fs::Dir::read_from(&task).map_err(|_| E::ProcessIdentity)? {
        let entry = entry.map_err(|_| E::ProcessIdentity)?;
        let name = entry.file_name().to_str().map_err(|_| E::ProcessIdentity)?;
        if name == "." || name == ".." {
            continue;
        }
        n += 1;
        if n > 4096 {
            return Err(E::Limit);
        }
        let dir = file_at(&task, name)?;
        let status = text(&dir, "status")?;
        if !status
            .lines()
            .any(|l| l.starts_with("State:\tT") || l.starts_with("State:\tt"))
        {
            return Ok(false);
        }
    }
    Ok(n != 0)
}
/// Only the namespace PID-1 supervisor may call this; it creates no children
/// until verification/termination. Stopped members remain stopped on success.
pub(super) struct QuiescentPopulation {
    members: BTreeMap<i32, Member>,
}
pub(super) fn verify(
    proc: &File,
    allowed: &[(u64, u64)],
    main: (i32, (u64, u64)),
    deadline: AttemptDeadline,
) -> Result<QuiescentPopulation> {
    let population = quiesce(proc, deadline, MAX_ROUNDS, |_| {})?;
    population.validate(proc, allowed, main, deadline)?;
    Ok(population)
}
fn quiesce(
    proc: &File,
    deadline: AttemptDeadline,
    rounds: usize,
    mut after_scan: impl FnMut(usize),
) -> Result<QuiescentPopulation> {
    if unsafe { libc::getpid() } != 1 {
        return Err(E::Isolation);
    }
    let mut members: BTreeMap<i32, Member> = BTreeMap::new();
    for round in 0..rounds {
        deadline.check()?;
        let mut gone = Vec::new();
        for (&pid, m) in &members {
            if exited(&m.pidfd)? {
                gone.push(pid);
            }
        }
        for pid in gone {
            members.remove(&pid);
        }
        let first = ids(proc)?;
        after_scan(round);
        for pid in first {
            if members.contains_key(&pid) {
                continue;
            }
            if let Some(m) = acquire(proc, pid)? {
                if let Err(error) = super::linux::signal(&m.pidfd, libc::SIGSTOP) {
                    if exited(&m.pidfd)? {
                        continue;
                    }
                    return Err(error);
                }
                members.insert(pid, m);
            }
        }
        let mut all_stopped = true;
        for m in members.values() {
            deadline.check()?;
            if exited(&m.pidfd)? {
                all_stopped = false;
                continue;
            }
            if !stopped(m)? {
                all_stopped = false;
            }
        }
        if !all_stopped {
            unsafe {
                libc::sched_yield();
            }
            continue;
        }
        // Once every live member is stopped, only PID 1 can create processes.
        // Repeat namespace enumeration to catch forks racing the first stop.
        let mut live = Vec::new();
        for pid in ids(proc)? {
            match members.get(&pid) {
                Some(m) if !exited(&m.pidfd)? => live.push(pid),
                _ => {
                    if acquire(proc, pid)?.is_some() {
                        all_stopped = false;
                    }
                }
            }
        }
        if !all_stopped || live != members.keys().copied().collect::<Vec<_>>() {
            continue;
        }
        return Ok(QuiescentPopulation { members });
    }
    Err(E::Limit)
}
impl QuiescentPopulation {
    fn validate(
        &self,
        proc: &File,
        allowed: &[(u64, u64)],
        main: (i32, (u64, u64)),
        deadline: AttemptDeadline,
    ) -> Result<()> {
        let self_dir = file_at(proc, "1")?;
        let net = file_at(&self_dir, "ns/net")?;
        let user = file_at(&self_dir, "ns/user")?;
        let pids = file_at(&self_dir, "ns/pid")?;
        if !self.members.contains_key(&main.0) {
            return Err(E::ProcessIdentity);
        }
        let mut validated_roles = BTreeMap::new();
        let mut renderers = 0;
        let mut network_services = 0;
        let mut unsandboxed_zygotes = Vec::new();
        let mut gpu_parents = Vec::new();
        for (&pid, m) in &self.members {
            deadline.check()?;
            if exited(&m.pidfd)? || start(&m.directory)? != m.start || !stopped(m)? {
                return Err(E::ProcessIdentity);
            }
            let exe = file_at(&m.directory, "exe")?
                .metadata()
                .map_err(|_| E::ProcessIdentity)?;
            super::verification::executable_identity(
                (exe.dev(), exe.ino()),
                main.1,
                allowed.contains(&(exe.dev(), exe.ino())),
            )?;
            let status = text(&m.directory, "status")?;

            let member_user = file_at(&m.directory, "ns/user")?;
            descendant_user(member_user.try_clone().map_err(|_| E::Isolation)?, &user)?;
            let member_pid = file_at(&m.directory, "ns/pid")?;
            descendant_pid(&member_pid, &pids)?;
            let member_net = file_at(&m.directory, "ns/net")?;
            if inode(&member_net)? != inode(&net)? {
                let owner = unsafe { libc::ioctl(member_net.as_raw_fd(), 0xb701) };
                if owner < 0 {
                    return Err(E::Isolation);
                }
                descendant_user(unsafe { File::from_raw_fd(owner) }, &user)?;
            }
            let preliminary =
                super::verification::attribute_role(pid, main.0, || text(&m.directory, "cmdline"))?;
            if pid == main.0 {
                super::verification::verify_main(
                    super::verification::MainIdentity {
                        pid,
                        executable: (exe.dev(), exe.ino()),
                        namespaces: [
                            inode(&member_user)?,
                            inode(&member_pid)?,
                            inode(&member_net)?,
                        ],
                    },
                    super::verification::MainIdentity {
                        pid: main.0,
                        executable: main.1,
                        namespaces: [inode(&user)?, inode(&pids)?, inode(&net)?],
                    },
                    &status,
                )?;
            }
            let role = match preliminary {
                super::verification::PreliminaryRole::Process(role) => role,
                super::verification::PreliminaryRole::BrokerCandidate => {
                    if inode(&member_net)? != inode(&net)? {
                        return Err(E::Isolation);
                    }
                    super::verification::broker_sandbox(
                        &status,
                        inode(&member_user)? != inode(&user)?,
                        inode(&member_pid)? != inode(&pids)?,
                    )?;
                    validated_roles.insert(
                        pid,
                        (preliminary, super::verification::parent_pid(&status)?),
                    );
                    continue;
                }
            };
            super::verification::process_capabilities(
                role,
                &status,
                inode(&member_user)? != inode(&user)?,
                inode(&member_pid)? != inode(&pids)?,
            )?;
            if matches!(
                role,
                super::verification::ChromiumRole::UnsandboxedZygote
                    | super::verification::ChromiumRole::Gpu
            ) {
                if inode(&member_net)? != inode(&net)? {
                    return Err(E::Isolation);
                }
                let parent = super::verification::parent_pid(&status)?;
                if role == super::verification::ChromiumRole::UnsandboxedZygote {
                    if parent != main.0 {
                        return Err(E::ProcessIdentity);
                    }
                    unsandboxed_zygotes.push(pid);
                } else {
                    super::verification::gpu_sandbox(
                        &status,
                        inode(&member_user)? != inode(&user)?,
                        inode(&member_pid)? != inode(&pids)?,
                    )?;
                    gpu_parents.push(parent);
                }
            }
            if role == super::verification::ChromiumRole::NetworkServiceUtility {
                if inode(&member_net)? != inode(&net)?
                    || super::verification::parent_pid(&status)? != main.0
                {
                    return Err(E::ProcessIdentity);
                }
                super::verification::network_service_sandbox(
                    &status,
                    inode(&member_user)? != inode(&user)?,
                    inode(&member_pid)? != inode(&pids)?,
                )?;
                network_services += 1;
            }
            if role == super::verification::ChromiumRole::Renderer {
                renderers += 1;
                super::verification::renderer_sandbox(
                    &status,
                    inode(&pids)?,
                    inode(&member_pid)?,
                    inode(&user)?,
                    inode(&member_user)?,
                )?;
            }
            // Only completely verified members enter the relational proof.
            validated_roles.insert(
                pid,
                (preliminary, super::verification::parent_pid(&status)?),
            );
        }
        let _resolved_roles = super::verification::broker_relationships(&validated_roles)?;
        // An admitted bootstrap must lead to a live, verified specialized GPU
        // child. All recorded parents are stopped members with retained pidfds.
        super::verification::network_service_population(network_services)?;
        if renderers == 0
            || unsandboxed_zygotes.is_empty()
            || gpu_parents.is_empty()
            || gpu_parents
                .iter()
                .any(|parent| !unsandboxed_zygotes.contains(parent))
        {
            return Err(E::Sandbox);
        }
        deadline.check()
    }
}

fn inode(f: &File) -> Result<u64> {
    Ok(f.metadata().map_err(|_| E::Isolation)?.ino())
}
fn descendant_user(mut f: File, root: &File) -> Result<()> {
    for _ in 0..32 {
        if inode(&f)? == inode(root)? {
            return Ok(());
        }
        let parent = unsafe { libc::ioctl(f.as_raw_fd(), 0xb702) };
        if parent < 0 {
            return Err(E::Isolation);
        }
        f = unsafe { File::from_raw_fd(parent) };
    }
    Err(E::Limit)
}
fn descendant_pid(f: &File, root: &File) -> Result<()> {
    descendant_user(f.try_clone().map_err(|_| E::Isolation)?, root)
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use std::process::{Command, Stdio};
    #[test]
    #[ignore = "explicit Linux rootless PID/mount namespace runtime test; no browser"]
    fn quiescent_namespace_population_runtime() {
        if unsafe { libc::getpid() } != 1 {
            let status = Command::new("/usr/bin/unshare")
                .args([
                    "--user",
                    "--map-current-user",
                    "--mount",
                    "--pid",
                    "--fork",
                    "--mount-proc",
                ])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "isolation::population::runtime_tests::quiescent_namespace_population_runtime",
                    "--ignored",
                    "--test-threads=1",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let proc = File::open("/proc").unwrap();
        // Real fork from a non-main worker: no manually supplied PID list.
        let mut worker_child = std::thread::spawn(|| {
            Command::new("/bin/sh")
                .args(["-c", "read value"])
                .stdin(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .join()
        .unwrap();
        let mut concurrent = None;
        let mut exited_child = Command::new("/bin/sh")
            .args(["-c", "read value"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let exited_pid = exited_child.id() as i32;
        let old = acquire(&proc, exited_pid).unwrap().unwrap();
        exited_child.kill().unwrap();
        exited_child.wait().unwrap();
        assert!(exited(&old.pidfd).unwrap());
        assert!(start(&old.directory).is_err());
        // Force actual reuse within this owned test PID namespace. A pinned
        // pidfd remains exited even when the numeric PID is assigned again.
        std::fs::write("/proc/sys/kernel/ns_last_pid", (exited_pid - 1).to_string()).unwrap();
        let mut reused = Command::new("/bin/sh")
            .args(["-c", "read value"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        assert_eq!(reused.id() as i32, exited_pid);
        assert!(exited(&old.pidfd).unwrap());
        assert!(start(&old.directory).is_err());
        reused.kill().unwrap();
        reused.wait().unwrap();
        let mut disappearing = Command::new("/bin/sh")
            .args(["-c", "read value"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let frozen = quiesce(&proc, AttemptDeadline::new(), MAX_ROUNDS, |round| {
            if round == 0 {
                disappearing.kill().unwrap();
                disappearing.wait().unwrap();
                concurrent = Some(
                    Command::new("/bin/sh")
                        .args([
                            "-c",
                            "read value",
                            "--type=utility",
                            "--utility-sub-type=network.mojom.NetworkService",
                            "--service-sandbox-type=network",
                        ])
                        .stdin(Stdio::piped())
                        .spawn()
                        .unwrap(),
                );
            }
        })
        .unwrap();
        disappearing.wait().unwrap();
        assert!(frozen.members.contains_key(&(worker_child.id() as i32)));
        assert!(
            frozen
                .members
                .contains_key(&(concurrent.as_ref().unwrap().id() as i32))
        );
        assert!(frozen.members.values().all(|m| stopped(m).unwrap()));
        let spoof = &frozen.members[&(concurrent.as_ref().unwrap().id() as i32)];
        // A shell's original NUL-separated argv is not Chromium's flattened
        // title; neither its text nor a retained PID can confer a browser role.
        assert!(
            super::super::verification::ChromiumProcessTitle::parse(
                &text(&spoof.directory, "cmdline").unwrap()
            )
            .is_err()
        );
        let actual = file_at(&spoof.directory, "exe")
            .unwrap()
            .metadata()
            .unwrap();
        let expected = File::open("/proc/self/exe").unwrap().metadata().unwrap();
        // Even exact role text from a live, stopped, pidfd-retained object cannot
        // authorize a different executable through the population predicate.
        assert_eq!(
            super::super::verification::executable_identity(
                (actual.dev(), actual.ino()),
                (expected.dev(), expected.ino()),
                true
            ),
            Err(E::ProcessIdentity)
        );
        assert_eq!(
            frozen.validate(
                &proc,
                &[],
                (worker_child.id() as i32, (0, 0)),
                AttemptDeadline::new()
            ),
            Err(E::ProcessIdentity)
        );
        assert!(matches!(
            quiesce(&proc, AttemptDeadline::new(), 0, |_| {}),
            Err(E::Limit)
        ));
        assert!(matches!(
            quiesce(&proc, AttemptDeadline::expired(), MAX_ROUNDS, |_| {}),
            Err(E::Deadline)
        ));
        let mut growing = Vec::new();
        let unstable = quiesce(&proc, AttemptDeadline::new(), 2, |_| {
            growing.push(
                Command::new("/bin/sh")
                    .args(["-c", "read value"])
                    .stdin(Stdio::piped())
                    .spawn()
                    .unwrap(),
            );
        });
        assert!(matches!(unstable, Err(E::Limit)));
        for mut child in growing {
            child.kill().unwrap();
            child.wait().unwrap();
        }
        worker_child.kill().unwrap();
        worker_child.wait().unwrap();
        let mut concurrent = concurrent.unwrap();
        concurrent.kill().unwrap();
        concurrent.wait().unwrap();
    }
}
