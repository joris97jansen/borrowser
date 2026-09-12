//! One browser-visible, kernel-permission-governed procfs for the owned PID namespace.
//! The covered host mount is retained only as a non-inheritable handshake capability.
use crate::{CaptureError as E, Result, deadline::AttemptDeadline};
use std::{fs::File, io::Read, os::fd::AsRawFd, os::unix::fs::MetadataExt, path::Path};

pub(super) struct HostProc(File);
fn read_at(root: &File, path: &str) -> Result<String> {
    let fd = rustix::fs::openat(
        root,
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| E::Isolation)?;
    let mut text = String::new();
    File::from(fd)
        .take(262145)
        .read_to_string(&mut text)
        .map_err(|_| E::Isolation)?;
    if text.len() > 262144 {
        return Err(E::Limit);
    }
    Ok(text)
}
// Reject other proc mounts, including bind-mounted proc subtrees. Mountinfo uses
// escaped path fields: only the exact unescaped /proc spelling is accepted.
fn audit_mounts(text: &str) -> Result<()> {
    let mut count = 0;
    for (n, line) in text.lines().enumerate() {
        if n >= 4096 {
            return Err(E::Limit);
        }
        let (left, right) = line.split_once(" - ").ok_or(E::Isolation)?;
        let fields: Vec<_> = left.split_whitespace().collect();
        if fields.len() < 6 {
            return Err(E::Isolation);
        }
        if right.split_whitespace().next() == Some("proc") {
            if fields[3] != "/" || fields[4] != "/proc" {
                return Err(E::Isolation);
            }
            count += 1;
        }
    }
    if count != 1 {
        return Err(E::Isolation);
    }
    Ok(())
}
impl HostProc {
    pub(super) fn retain(deadline: AttemptDeadline) -> Result<Self> {
        deadline.check()?;
        let fd = rustix::fs::open(
            "/proc",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| E::Isolation)?;
        let root = File::from(fd);
        if unsafe { libc::fcntl(root.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC == 0 {
            return Err(E::Isolation);
        }
        proc_type(&root)?;
        audit_mounts(&read_at(&root, "self/mountinfo")?)?;
        Ok(Self(root))
    }
    pub(super) fn pid(&self) -> Result<i32> {
        read_at(&self.0, "self/stat")?
            .split_whitespace()
            .next()
            .ok_or(E::ProcessIdentity)?
            .parse()
            .map_err(|_| E::ProcessIdentity)
    }
    pub(super) fn cover(&self, deadline: AttemptDeadline) -> Result<File> {
        deadline.check()?;
        if unsafe { libc::getpid() } != 1 {
            return Err(E::Isolation);
        }
        // Recheck immediately before covering; no browser exists yet and this
        // mount namespace has private propagation and a single owner.
        audit_mounts(&read_at(&self.0, "self/mountinfo")?)?;
        crate::linux_mount::mount(
            None,
            Path::new("/proc"),
            Some("proc"),
            libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
            None,
        )?;
        let private = File::open("/proc").map_err(|_| E::Isolation)?;
        verify(&private)?;
        if private.metadata().map_err(|_| E::Isolation)?.dev()
            == self.0.metadata().map_err(|_| E::Isolation)?.dev()
        {
            return Err(E::Isolation);
        }
        // Before the browser is forked this namespace contains only init.
        let mut count = 0;
        for entry in std::fs::read_dir("/proc").map_err(|_| E::Isolation)? {
            count += 1;
            if count > 4096 {
                return Err(E::Limit);
            }
            let name = entry.map_err(|_| E::Isolation)?.file_name();
            if let Ok(pid) = name.to_string_lossy().parse::<u32>()
                && pid != 1
            {
                return Err(E::Isolation);
            }
        }
        deadline.check()?;
        Ok(private)
    }
}
fn proc_type(root: &File) -> Result<()> {
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstatfs(root.as_raw_fd(), &mut stat) } != 0
        || stat.f_type != libc::PROC_SUPER_MAGIC
    {
        return Err(E::Isolation);
    }
    Ok(())
}
fn verify(private: &File) -> Result<()> {
    proc_type(private)?;
    let visible = std::fs::metadata("/proc").map_err(|_| E::Isolation)?;
    let retained = private.metadata().map_err(|_| E::Isolation)?;
    if (visible.dev(), visible.ino()) != (retained.dev(), retained.ino())
        || std::fs::read_link("/proc/self").map_err(|_| E::Isolation)? != Path::new("1")
        || !read_at(private, "1/stat")?.starts_with("1 ")
    {
        return Err(E::Isolation);
    }
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let flags = libc::ST_NOSUID | libc::ST_NODEV | libc::ST_NOEXEC;
    if unsafe { libc::fstatvfs(private.as_raw_fd(), &mut stat) } != 0
        || stat.f_flag & flags != flags
        || stat.f_flag & libc::ST_RDONLY != 0
    {
        return Err(E::Isolation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_proc_aliases_and_subtree_binds() {
        let primary = "1 0 0:1 / /proc rw - proc proc rw\n";
        assert!(audit_mounts(primary).is_ok());
        for extra in [
            "2 0 0:1 / /alias rw - proc proc rw\n",
            "2 0 0:1 /sys /sys-alias rw - proc proc rw\n",
            primary,
        ] {
            assert!(audit_mounts(&format!("{primary}{extra}")).is_err());
        }
        assert!(audit_mounts("").is_err());
    }
    #[test]
    #[ignore = "explicit Linux rootless PID/mount namespace runtime test; no browser"]
    fn browser_visible_private_proc_runtime() {
        use std::process::Command;
        if unsafe { libc::getpid() } != 1 {
            assert_ne!(
                unsafe { libc::getuid() },
                0,
                "runtime test requires a non-root caller"
            );
            assert!(
                Command::new("/usr/bin/unshare")
                    .args(["--user", "--map-current-user", "--mount", "--pid", "--fork"])
                    .arg(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "isolation::procfs::tests::browser_visible_private_proc_runtime",
                        "--ignored",
                        "--test-threads=1"
                    ])
                    .status()
                    .unwrap()
                    .success()
            );
            return;
        }
        let deadline = AttemptDeadline::new();
        let host = HostProc::retain(deadline).unwrap();
        let host_pid = host.pid().unwrap();
        assert!(host_pid > 1);
        let stat = read_at(&host.0, "self/stat").unwrap();
        let parent: u32 = stat
            .rsplit_once(") ")
            .unwrap()
            .1
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        assert!(parent > 1);
        assert!(read_at(&host.0, &format!("{parent}/stat")).is_ok());
        let private = host.cover(deadline).unwrap();
        assert_eq!(host.pid().unwrap(), host_pid);
        assert!(!Path::new(&format!("/proc/{host_pid}")).exists());
        assert!(!Path::new(&format!("/proc/{parent}")).exists());
        verify(&private).unwrap();
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        assert!(Path::new(&format!("/proc/{}", child.id())).exists());
        assert_eq!(
            super::super::population::ids(&private).unwrap(),
            [child.id() as i32]
        );
        // An actual exec must not inherit the host capability (including its
        // magic-link alias); the descriptor is CLOEXEC before fork as well.
        assert!(
            Command::new("/bin/sh")
                .args([
                    "-c",
                    &format!("test ! -e /proc/self/fd/{}", host.0.as_raw_fd())
                ])
                .status()
                .unwrap()
                .success()
        );
        // util-linux exercises the kernel's normal mapping flow before exec;
        // this test is not a second production Chromium mapping controller.
        drop(host);
        use std::os::unix::process::CommandExt;
        let uid = unsafe { libc::getuid() }.to_string();
        let gid = unsafe { libc::getgid() }.to_string();
        let user_ns = std::fs::read_link("/proc/self/ns/user").unwrap();
        let proc_identity = format!(
            "{}:{}",
            private.metadata().unwrap().dev(),
            private.metadata().unwrap().ino()
        );
        let script = r#"
            read a b c < /proc/self/uid_map
            test "$a:$b:$c" = "$1:$1:1"
            read a b c < /proc/self/gid_map
            test "$a:$b:$c" = "$2:$2:1"
            awk '/^Cap(Eff|Prm|Inh|Amb):/ {n++; if ($2 != "0000000000000000") bad=1} END {exit (bad || n != 4)}' /proc/self/status
            read groups < /proc/self/setgroups
            test "$groups" = deny
            test "$(readlink /proc/self/ns/user)" != "$3"
            test "$(stat -c '%d:%i' /proc)" = "$4"
            test ! -e "/proc/$5"
            test ! -e "/proc/$6"
            # Maps are write-once; deny cannot be reversed. Neither the
            # ancestor init nor global proc/sys gains writable authority.
            if (printf '%s %s 1' "$1" "$1" > /proc/self/uid_map) 2>/dev/null; then exit 21; fi
            if (printf '%s %s 1' "$2" "$2" > /proc/self/gid_map) 2>/dev/null; then exit 22; fi
            if (printf allow > /proc/self/setgroups) 2>/dev/null; then exit 23; fi
            if (printf '0 0 1' > /proc/1/uid_map) 2>/dev/null; then exit 24; fi
            if (printf '0 0 1' > /proc/1/gid_map) 2>/dev/null; then exit 25; fi
            if (exec 9> /proc/sys/kernel/hostname) 2>/dev/null; then exit 26; fi
            # The covered original /proc may remain listed in mountinfo;
            # neither that record nor the new mount may have a pathname alias.
            awk '$0 ~ / - proc / {n++; if ($4 != "/" || $5 != "/proc") bad=1} END {exit (bad || n < 1 || n > 2)}' /proc/self/mountinfo
        "#;
        let mut sandbox = Command::new("/usr/bin/unshare");
        sandbox
            .args([
                "--user",
                "--map-current-user",
                "--setgroups=deny",
                "/bin/sh",
                "-ec",
                script,
                "procfs-test",
                &uid,
                &gid,
            ])
            .arg(user_ns)
            .arg(proc_identity)
            .arg(host_pid.to_string())
            .arg(parent.to_string());
        // SAFETY: the pre-exec hook performs only capability syscalls; no
        // allocations or locks. Chromium uses this same clearing function.
        unsafe {
            sandbox.pre_exec(|| {
                super::super::linux::clear_exec_privileges()
                    .map_err(|_| std::io::Error::from_raw_os_error(libc::EPERM))
            });
        }
        assert!(sandbox.status().unwrap().success());
        verify(&private).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
