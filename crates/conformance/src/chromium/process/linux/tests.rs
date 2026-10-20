use super::*;
use std::{
    cell::{Cell, RefCell},
    io::{self, Seek, Write},
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Child, ChildStdout, Command, ExitStatus, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

// Private, thread-local synchronization at the actual observation boundaries.
// No scheduling delays or process churn are needed to reproduce disappearance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    StatOpened,
    PidfdOpening,
    PidfdOpened,
}
type Action = Box<dyn FnOnce(Option<&mut std::fs::File>) -> std::io::Result<()>>;
thread_local! {
    static HOOK: RefCell<Option<(Step, i32, Action)>> = const { RefCell::new(None) };
}
fn arm(
    step: Step,
    pid: i32,
    action: impl FnOnce(Option<&mut std::fs::File>) -> std::io::Result<()> + 'static,
) {
    HOOK.with(|hook| {
        assert!(hook.borrow().is_none());
        *hook.borrow_mut() = Some((step, pid, Box::new(action)));
    });
}
pub(super) fn at(step: Step, pid: i32, file: Option<&mut std::fs::File>) -> std::io::Result<()> {
    let action = HOOK
        .try_with(|hook| {
            let mut hook = hook.borrow_mut();
            if hook
                .as_ref()
                .is_some_and(|(s, p, _)| *s == step && *p == pid)
            {
                hook.take().map(|(_, _, action)| action)
            } else {
                None
            }
        })
        // An unconsumed hook can own a fixture. Its destruction must be able
        // to clean that fixture without reentering destroyed hook storage.
        .unwrap_or(None);
    if let Some(action) = action {
        action(file)?;
    }
    Ok(())
}
fn consumed() {
    HOOK.with(|hook| assert!(hook.borrow().is_none(), "observation hook was not reached"));
}
fn budget() -> OwnershipBudget<'static> {
    OwnershipBudget {
        deadline: Instant::now() + Duration::from_secs(5),
        cancel: None,
    }
}
fn registry(root: i32) -> Registry {
    Registry::new(root, PathBuf::new(), PathBuf::new(), PathBuf::new())
}

const ACK_LIMIT: Duration = Duration::from_secs(5);
const EXIT_LIMIT: Duration = Duration::from_secs(5);
const KILL_LIMIT: Duration = Duration::from_secs(2);
thread_local! {
    static COMPLETIONS: Cell<usize> = const { Cell::new(0) };
}

#[must_use]
#[derive(Clone, Debug)]
struct Completion {
    status: Option<ExitStatus>,
    errors: Vec<String>,
}
impl Completion {
    fn assert_success(&self) {
        assert!(
            self.errors.is_empty() && self.status.is_some_and(|s| s.success()),
            "fixture completion: {self:?}"
        );
    }
}

// Only this private two-level fixture uses the registry during teardown.
// Its parent topology runs in an isolated subreaper, never the test runner.
struct Fixture {
    child: Child,
    identity: Option<OwnedFd>,
    topology: Option<Registry>,
    completion: Option<Completion>,
}
fn fixture_command(role: &str, stderr: Stdio) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "chromium::process::linux::tests::process_fixture",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("AG5_PROC_ROLE", role)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr);
    // Session membership is only used for the known parent/child fixture.
    // A dying isolated control also cannot leave a live fixture behind.
    let topology = role.starts_with("parent");
    // Select the negative control before fork; a newly spawned child cannot
    // have PID 0 as its parent in this process's PID namespace.
    let parent = if role == "wrong-parent" {
        0
    } else {
        unsafe { libc::getpid() }
    };
    // SAFETY: the post-fork callback captures only Copy scalars and calls
    // native process operations. Both error constructors store only an OS
    // code; no allocation, formatting, logging, locks or Rust cleanup runs.
    unsafe {
        command.pre_exec(move || {
            if (topology && libc::setsid() < 0)
                || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
            {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                // ESRCH means the expected parent was lost before exec.
                return Err(io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
    command
}
impl Fixture {
    fn new(role: &str) -> Self {
        Self::spawn(role, Stdio::inherit())
    }
    fn spawn(role: &str, stderr: Stdio) -> Self {
        let topology = role.starts_with("parent");
        if topology {
            let mut subreaper = 0;
            assert_eq!(
                unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut subreaper, 0, 0, 0) },
                0
            );
            assert_eq!(subreaper, 1, "parent fixture requires isolated subreaper");
        }
        let child = fixture_command(role, stderr).spawn().unwrap();
        // Install the guard before any fallible post-spawn setup.
        let mut fixture = Self {
            topology: topology.then(|| registry(child.id() as i32)),
            child,
            identity: None,
            completion: None,
        };
        fixture.identity = Some(pidfd(fixture.pid()).expect("fixture stable identity"));
        let stdout = fixture.child.stdout.as_ref().unwrap().as_raw_fd();
        let flags = unsafe { libc::fcntl(stdout, libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(stdout, libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        fixture
    }
    fn pid(&self) -> i32 {
        self.child.id().try_into().unwrap()
    }
    fn finish(&mut self) -> Completion {
        self.finish_with_limit(EXIT_LIMIT)
    }
    fn finish_with_limit(&mut self, limit: Duration) -> Completion {
        if let Some(done) = &self.completion {
            return done.clone();
        }
        COMPLETIONS.with(|count| count.set(count.get() + 1));
        drop(self.child.stdin.take());
        let mut done = Completion {
            status: None,
            errors: vec![],
        };
        if let Err(error) = self.collect(Instant::now() + limit, false, &mut done) {
            done.errors.push(format!("normal completion: {error}"));
            // A separate, finite fallback budget is shared by all teardown
            // operations; it is never reset by polling or protocol fragments.
            if let Err(error) = self.collect(Instant::now() + KILL_LIMIT, true, &mut done) {
                done.errors.push(format!("unresolved cleanup: {error}"));
            }
        }
        // Record reaping and every error before the caller can assert anything.
        self.completion = Some(done.clone());
        done
    }
    fn collect(
        &mut self,
        deadline: Instant,
        kill: bool,
        done: &mut Completion,
    ) -> Result<(), String> {
        let budget = OwnershipBudget {
            deadline,
            cancel: None,
        };
        if done.status.is_some() {
            // Root identity has already been consumed. Only finish reaping
            // retained/adopted children; never discover or signal by that PID.
            return self.topology.as_mut().map_or(Ok(()), |owner| {
                owner.reap_adopted(&budget).map_err(|e| e.to_string())
            });
        }
        if kill && self.topology.is_none() {
            if let Some(fd) = &self.identity {
                let rc = unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        fd.as_raw_fd(),
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    )
                };
                if rc != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
                    return Err(format!("signal fixture: {}", io::Error::last_os_error()));
                }
            } else {
                // Setup failed before pidfd acquisition. This unreaped, owned
                // Child is still the only available identity.
                self.child
                    .kill()
                    .map_err(|e| format!("kill fixture: {e}"))?;
            }
        }
        loop {
            if Instant::now() >= deadline {
                return Err("fixture completion deadline".into());
            }
            let ready = if let Some(owner) = &mut self.topology {
                owner.discover(&budget).map_err(|e| e.to_string())?;
                if kill {
                    owner
                        .signal(libc::SIGKILL, &budget)
                        .map_err(|e| e.to_string())?;
                }
                // Reserve the root until descendants have exited and been
                // observed after adoption. Never reap the root to discover later.
                owner.terminated(&budget).map_err(|e| e.to_string())?
            } else {
                true
            };
            if ready
                && let Some(status) = self
                    .child
                    .try_wait()
                    .map_err(|e| format!("wait fixture: {e}"))?
            {
                done.status = Some(status);
                if let Some(owner) = &mut self.topology {
                    owner.reap_adopted(&budget).map_err(|e| e.to_string())?;
                }
                return Ok(());
            }
            super::super::poll_fd(
                self.identity.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                libc::POLLIN,
                deadline,
            )
            .map_err(|e| e.to_string())?;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.completion.is_none() {
            let done = self.finish();
            if !done.errors.is_empty() || !done.status.is_some_and(|s| s.success()) {
                // Drop is also used during an unrelated assertion's unwind.
                // Failed diagnostic delivery must not cause a second panic.
                let _ = writeln!(io::stderr().lock(), "fixture cleanup: {done:?}");
            }
        }
    }
}

#[test]
#[ignore = "private synchronized proc observation fixture"]
fn process_fixture() {
    let role = std::env::var("AG5_PROC_ROLE").unwrap();
    if let Some(case) = role.strip_prefix("case:") {
        fixture_case(case);
        return;
    }
    if role == "exit23" {
        std::process::exit(23);
    }
    if role == "child" {
        std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink()).unwrap();
        return;
    }
    if matches!(role.as_str(), "missing" | "stalled" | "nonexit") {
        println!("AG5_READY");
        if role == "stalled" {
            print!("AG5_PID=");
        }
        std::io::stdout().flush().unwrap();
        // Deliberately unresponsive leaf; the fixture must reach its own
        // bounded kill/reap path, not an outer watchdog.
        loop {
            unsafe { libc::pause() };
        }
    }
    assert!(matches!(role.as_str(), "parent" | "parent-stalled"));
    let mut child = Fixture::new("child");
    println!("AG5_PID={}", child.pid());
    std::io::stdout().flush().unwrap();
    if role == "parent-stalled" {
        // The fallback must own and reap this descendant even when its parent
        // never acknowledges the request or services stdin EOF.
        loop {
            unsafe { libc::pause() };
        }
    }
    let mut byte = [0];
    let requested = input_byte(Instant::now() + ACK_LIMIT, &mut byte).unwrap();
    child.finish().assert_success();
    if requested {
        println!("AG5_REAPED");
        std::io::stdout().flush().unwrap();
        assert!(!input_byte(Instant::now() + ACK_LIMIT, &mut byte).unwrap());
    }
}
fn input_byte(deadline: Instant, byte: &mut [u8; 1]) -> io::Result<bool> {
    let flags = unsafe { libc::fcntl(0, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    loop {
        check_ack_deadline(deadline, "command")?;
        match io::stdin().read(byte) {
            Ok(n) => return Ok(n != 0),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                super::super::poll_fd(0, libc::POLLIN, deadline)
                    .map_err(|e| io::Error::other(e.to_string()))?;
            }
            Err(e) => return Err(e),
        }
    }
}
fn check_ack_deadline(deadline: Instant, prefix: &str) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("fixture acknowledgement deadline: {prefix}"),
        ))
    } else {
        Ok(())
    }
}
fn line(reader: &mut ChildStdout, prefix: &str, deadline: Instant) -> io::Result<String> {
    let mut line = vec![];
    // Both total output and time are bounded, including an incomplete line
    // from a live writer. The deadline belongs to the whole acknowledgement.
    for _ in 0..16384 {
        loop {
            check_ack_deadline(deadline, prefix)?;
            let mut byte = [0];
            match reader.read(&mut byte) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("missing {prefix}"),
                    ));
                }
                Ok(_) => {
                    line.push(byte[0]);
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    super::super::poll_fd(reader.as_raw_fd(), libc::POLLIN, deadline)
                        .map_err(|e| io::Error::other(e.to_string()))?;
                }
                Err(e) => return Err(e),
            }
        }
        if line.last() == Some(&b'\n') {
            let text = std::str::from_utf8(&line).map_err(io::Error::other)?;
            if let Some((_, value)) = text.split_once(prefix) {
                return Ok(format!("{prefix}{}", value.trim()));
            }
            line.clear();
        }
    }
    Err(io::Error::other(format!(
        "fixture acknowledgement output limit: {prefix}"
    )))
}

// Potentially aborting/hanging assertions run in a separate test process.
// The outer guard's deadline is a failure detector, never a passing teardown.
fn run_fixture_case(case: &str, expected_code: i32) -> String {
    let mut diagnostics = tempfile::tempfile().unwrap();
    let mut child = fixture_command(
        &format!("case:{case}"),
        Stdio::from(diagnostics.try_clone().unwrap()),
    )
    .spawn()
    .unwrap();
    drop(child.stdin.take());
    // Deliberately independent of Fixture::finish/collect: a regression to a
    // blocking fixture wait must not also disable the outer watchdog.
    let identity = pidfd(child.id() as i32);
    let result = wait_control(
        &mut child,
        identity.as_ref().ok(),
        Instant::now() + Duration::from_secs(15),
    );
    let mut fallback = vec![];
    if result.is_err() {
        let cleanup_deadline = Instant::now() + KILL_LIMIT;
        let signaled = match &identity {
            Ok(fd) => {
                let rc = unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        fd.as_raw_fd(),
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    )
                };
                if rc == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            }
            // A failed setup still has its owned Child. Check it is waitable
            // and live before using the Child handle's fallback signal.
            Err(_) => match child.try_wait() {
                Ok(None) => child.kill(),
                Ok(Some(_)) => Ok(()),
                Err(e) => Err(e),
            },
        };
        if let Err(error) = signaled {
            fallback.push(format!("outer signal: {error}"));
        }
        if let Err(error) = wait_control(&mut child, identity.as_ref().ok(), cleanup_deadline) {
            fallback.push(format!("outer unresolved cleanup: {error}"));
        }
    }
    diagnostics.rewind().unwrap();
    let mut text = String::new();
    diagnostics.take(65537).read_to_string(&mut text).unwrap();
    assert!(text.len() <= 65536, "isolated fixture diagnostic bound");
    assert!(
        result.is_ok() && fallback.is_empty(),
        "outer watchdog/cleanup failure: {result:?}; {fallback:?}\n{text}"
    );
    let status = result.unwrap();
    assert!(identity.is_ok(), "outer identity: {identity:?}");
    assert_eq!(status.code(), Some(expected_code), "{status:?}\n{text}");
    assert!(!text.contains("panic in a destructor"), "{text}");
    eprintln!("isolated fixture {case}: exit {expected_code}; no outer watchdog\n{text}");
    text
}

fn wait_control(
    child: &mut Child,
    identity: Option<&OwnedFd>,
    deadline: Instant,
) -> Result<ExitStatus, String> {
    loop {
        if Instant::now() >= deadline {
            return Err("isolated fixture exceeded outer deadline".into());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
        super::super::poll_fd(
            identity.map_or(-1, AsRawFd::as_raw_fd),
            libc::POLLIN,
            deadline,
        )
        .map_err(|e| e.to_string())?;
    }
}

#[test]
fn fixture_failure_paths_are_bounded_and_do_not_double_panic() {
    let nonzero = run_fixture_case("nonzero", 101);
    assert!(nonzero.contains("exit status: 23"), "{nonzero}");
    assert!(nonzero.contains("fixture completion:"), "{nonzero}");
    let unwind = run_fixture_case("unwind", 101);
    assert!(unwind.contains("original test assertion"), "{unwind}");
    assert!(unwind.contains("fixture cleanup:"), "{unwind}");
    let unwind = run_fixture_case("unwind-pending-hook", 101);
    assert!(
        unwind.contains("original pending-hook assertion"),
        "{unwind}"
    );
    assert!(!unwind.contains("fixture cleanup:"), "{unwind}");
    for case in [
        "missing",
        "stalled",
        "nonexit",
        "parent-stalled",
        "wait-error",
        "diagnostic-error",
    ] {
        run_fixture_case(case, 0);
    }
}

#[test]
fn fixture_completion_is_recorded_before_assertion_and_not_repeated_by_drop() {
    run_fixture_case("completed", 0);
}

#[test]
fn fixture_expected_parent_mismatch_is_a_spawn_error() {
    let text = run_fixture_case("parent-mismatch", 0);
    assert!(
        text.contains("expected-parent mismatch: spawn ESRCH"),
        "{text}"
    );
}

fn assert_no_fixture_children() {
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) }, -1);
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

fn fixture_case(case: &str) {
    // Only this isolated, otherwise childless control adopts grandchildren.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) },
        0
    );
    match case {
        "unrelated" => unrelated_disappearance(),
        "parent-mismatch" => {
            match fixture_command("wrong-parent", Stdio::inherit()).spawn() {
                Err(error) => assert_eq!(error.raw_os_error(), Some(libc::ESRCH)),
                Ok(child) => {
                    // Even an unexpected successful spawn retains its guard
                    // and bounded teardown before this control fails.
                    let mut fixture = Fixture {
                        child,
                        identity: None,
                        topology: None,
                        completion: None,
                    };
                    let done = fixture.finish();
                    panic!("expected spawn ESRCH, but executable started: {done:?}");
                }
            }
            assert_no_fixture_children();
            eprintln!("expected-parent mismatch: spawn ESRCH; no remaining child");
        }
        "nonzero" => {
            let mut child = Fixture::new("exit23");
            let done = child.finish();
            assert_eq!(done.status.unwrap().code(), Some(23));
            assert!(done.errors.is_empty(), "{done:?}");
            assert_no_fixture_children();
            eprintln!("inspectable fixture exit status: 23");
            done.assert_success(); // a normal test failure, never SIGABRT
        }
        "unwind" => {
            let panic = std::panic::catch_unwind(|| {
                let _child = Fixture::new("exit23");
                panic!("original test assertion");
            })
            .unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"original test assertion")
            );
            assert_no_fixture_children();
            std::panic::resume_unwind(panic);
        }
        "unwind-pending-hook" => {
            // A failed assertion can leave the fixture owned by an unconsumed
            // thread-local hook. Its guard must also survive TLS destruction.
            let panic = std::thread::spawn(|| {
                let mut parent = Fixture::new("parent");
                let pid = line(
                    parent.child.stdout.as_mut().unwrap(),
                    "AG5_PID=",
                    Instant::now() + ACK_LIMIT,
                )
                .unwrap()
                .trim_start_matches("AG5_PID=")
                .parse()
                .unwrap();
                arm(Step::StatOpened, pid, move |_| {
                    drop(parent);
                    Ok(())
                });
                panic!("original pending-hook assertion");
            })
            .join()
            .unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&"original pending-hook assertion")
            );
            assert_no_fixture_children();
            std::panic::resume_unwind(panic);
        }
        "completed" => {
            let before = COMPLETIONS.get();
            let mut child = Fixture::new("child");
            child.finish().assert_success();
            assert!(!alive(child.identity.as_ref().unwrap()).unwrap());
            child.finish().assert_success();
            assert_eq!(COMPLETIONS.get(), before + 1);
            drop(child);
            assert_eq!(COMPLETIONS.get(), before + 1);
        }
        "diagnostic-error" => {
            // Exercise real Drop with failed stderr delivery in this isolated
            // process. The fixture itself still owns, observes and reaps exit 23.
            let child = Fixture::new("exit23");
            let saved = unsafe { libc::fcntl(2, libc::F_DUPFD_CLOEXEC, 3) };
            assert!(saved >= 0);
            assert_eq!(unsafe { libc::close(2) }, 0);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(child)));
            assert_eq!(unsafe { libc::dup2(saved, 2) }, 2);
            assert_eq!(unsafe { libc::close(saved) }, 0);
            assert!(result.is_ok(), "diagnostic write failure panicked in Drop");
        }
        "wait-error" => {
            let mut child = Fixture::new("exit23");
            let deadline = Instant::now() + ACK_LIMIT;
            while alive(child.identity.as_ref().unwrap()).unwrap() {
                check_ack_deadline(deadline, "exit23").unwrap();
                super::super::poll_fd(
                    child.identity.as_ref().unwrap().as_raw_fd(),
                    libc::POLLIN,
                    deadline,
                )
                .unwrap();
            }
            // Deliberately steal the wait status to exercise the fixture's
            // actual ECHILD path. Signals can still only use its retained pidfd.
            let mut status = 0;
            assert_eq!(
                unsafe { libc::waitpid(child.pid(), &mut status, libc::WNOHANG) },
                child.pid()
            );
            let done = child.finish();
            assert!(done.status.is_none(), "{done:?}");
            assert_eq!(done.errors.len(), 2, "{done:?}");
            assert!(
                done.errors
                    .iter()
                    .all(|e| e.contains("wait fixture:") && e.contains("os error 10")),
                "{done:?}"
            );
            assert!(done.errors[1].contains("unresolved cleanup"), "{done:?}");
            eprintln!("inspectable wait failure: {done:?}");
        }
        "missing" | "stalled" | "nonexit" | "parent-stalled" => {
            let mut child = Fixture::new(case);
            let parent = case == "parent-stalled";
            let ready = line(
                child.child.stdout.as_mut().unwrap(),
                if parent { "AG5_PID=" } else { "AG5_READY" },
                Instant::now() + ACK_LIMIT,
            )
            .unwrap();
            let descendant = parent.then(|| {
                let pid = ready.trim_start_matches("AG5_PID=").parse::<i32>().unwrap();
                assert_eq!(ownership(&read_stat(pid).unwrap()).unwrap().0, child.pid());
                (pid, pidfd(pid).unwrap())
            });
            if case != "nonexit" {
                if parent {
                    child.child.stdin.as_mut().unwrap().write_all(b"x").unwrap();
                }
                let prefix = if parent { "AG5_REAPED" } else { "AG5_PID=" };
                let error = line(
                    child.child.stdout.as_mut().unwrap(),
                    prefix,
                    Instant::now() + Duration::from_millis(100),
                )
                .unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::TimedOut);
                assert!(error.to_string().contains(prefix), "{error}");
                assert!(
                    child.child.try_wait().unwrap().is_none(),
                    "writer must still be live"
                );
                eprintln!("bounded acknowledgement rejection: {error}");
            }
            let done = child.finish_with_limit(Duration::from_millis(100));
            assert_eq!(
                done.status.unwrap().signal(),
                Some(libc::SIGKILL),
                "{done:?}"
            );
            // The same absolute deadline can be reached between observations
            // or inside the bounded registry operation. No other primary error,
            // extra error, or unresolved fallback satisfies this control.
            assert!(
                done.errors == ["normal completion: fixture completion deadline"]
                    || (parent
                        && done.errors
                            == [format!("normal completion: {}", OwnershipError::Timeout)]),
                "{done:?}"
            );
            if let Some((pid, fd)) = descendant {
                assert!(!alive(&fd).unwrap(), "descendant {pid} survived");
                assert_eq!(read_stat(pid).unwrap_err().kind(), io::ErrorKind::NotFound);
            }
            eprintln!("bounded fallback reaped fixture topology: {done:?}");
        }
        _ => panic!("unknown private fixture case: {case}"),
    }
    assert_no_fixture_children();
}

fn assert_kernel_esrch(file: &mut std::fs::File) {
    let error = file.read_to_string(&mut String::new()).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::ESRCH));
    eprintln!("opened /proc stat descriptor after synchronized reap: {error}");
}

#[test]
fn disappearance_during_initial_stat_read_is_not_an_inspection_failure() {
    let child = Rc::new(RefCell::new(Fixture::new("child")));
    let pid = child.borrow().pid();
    arm(Step::StatOpened, pid, move |file| {
        child.borrow_mut().finish().assert_success();
        assert_kernel_esrch(file.unwrap());
        Ok(())
    });
    let mut owner = registry(pid);
    assert!(owner.observe_pid(pid, &budget()).unwrap());
    consumed();
    assert!(owner.processes.is_empty());
}

#[test]
fn unrelated_disappearance_cannot_acquire_signal_authority() {
    run_fixture_case("unrelated", 0);
}
fn unrelated_disappearance() {
    let mut parent = Fixture::new("parent");
    let pid: i32 = line(
        parent.child.stdout.as_mut().unwrap(),
        "AG5_PID=",
        Instant::now() + ACK_LIMIT,
    )
    .unwrap()
    .trim_start_matches("AG5_PID=")
    .parse()
    .unwrap();
    let (ppid, session, _) = ownership(&read_stat(pid).unwrap()).unwrap();
    assert_eq!(ppid, parent.pid());
    assert_ne!(ppid, unsafe { libc::getpid() });
    assert_ne!(session, 0);
    arm(Step::StatOpened, pid, move |file| {
        parent.child.stdin.as_mut().unwrap().write_all(b"x")?;
        assert_eq!(
            line(
                parent.child.stdout.as_mut().unwrap(),
                "AG5_REAPED",
                Instant::now() + ACK_LIMIT,
            )?,
            "AG5_REAPED"
        );
        assert_kernel_esrch(file.unwrap());
        parent.finish().assert_success();
        Ok(())
    });
    let mut owner = registry(0);
    assert!(owner.observe_pid(pid, &budget()).unwrap());
    consumed();
    assert!(owner.processes.is_empty());
    owner.signal(libc::SIGTERM, &budget()).unwrap(); // no identity to signal
}

#[test]
fn disappearance_during_pidfd_open_or_stat_revalidation_is_observed() {
    for step in [Step::PidfdOpening, Step::PidfdOpened] {
        let child = Rc::new(RefCell::new(Fixture::new("child")));
        let pid = child.borrow().pid();
        arm(step, pid, move |_| {
            if step == Step::PidfdOpening {
                child.borrow_mut().finish().assert_success();
            } else {
                arm(Step::StatOpened, pid, move |file| {
                    child.borrow_mut().finish().assert_success();
                    assert_kernel_esrch(file.unwrap());
                    Ok(())
                });
            }
            Ok(())
        });
        let mut owner = registry(pid);
        assert!(owner.observe_pid(pid, &budget()).unwrap());
        consumed();
        assert!(owner.processes.is_empty());
    }
}

#[test]
fn inspection_errors_are_not_disappearance_even_if_the_pidfd_has_exited() {
    for revalidate in [false, true] {
        for errno in [libc::EACCES, libc::EIO] {
            let child = Rc::new(RefCell::new(Fixture::new("child")));
            let pid = child.borrow().pid();
            let action = move |_: Option<&mut std::fs::File>| {
                child.borrow_mut().finish().assert_success();
                Err(std::io::Error::from_raw_os_error(errno))
            };
            if revalidate {
                arm(Step::PidfdOpened, pid, move |_| {
                    arm(Step::StatOpened, pid, action);
                    Ok(())
                });
            } else {
                arm(Step::StatOpened, pid, action);
            }
            let mut owner = registry(pid);
            let error = owner.observe_pid(pid, &budget()).unwrap_err().to_string();
            consumed();
            assert!(error.contains(&format!("os error {errno}")), "{error}");
            assert!(error.contains(&format!("/proc/{pid}/stat")), "{error}");
            assert!(owner.processes.is_empty());
        }
    }
}

#[test]
fn pidfd_permission_failure_is_not_hidden_by_later_disappearance() {
    let child = Rc::new(RefCell::new(Fixture::new("child")));
    let pid = child.borrow().pid();
    arm(Step::PidfdOpening, pid, move |_| {
        child.borrow_mut().finish().assert_success();
        Err(std::io::Error::from_raw_os_error(libc::EACCES))
    });
    let mut owner = registry(pid);
    let error = owner.observe_pid(pid, &budget()).unwrap_err().to_string();
    consumed();
    assert!(
        error.contains("pidfd_open") && error.contains("os error 13"),
        "{error}"
    );
    assert!(owner.processes.is_empty());
}

#[test]
fn revalidation_absence_requires_a_dead_stable_identity() {
    let mut child = Fixture::new("child");
    let pid = child.pid();
    arm(Step::PidfdOpened, pid, move |_| {
        arm(Step::StatOpened, pid, |_| {
            Err(std::io::Error::from_raw_os_error(libc::ESRCH))
        });
        Ok(())
    });
    let mut owner = registry(pid);
    let error = owner.observe_pid(pid, &budget()).unwrap_err().to_string();
    consumed();
    assert!(
        error.contains("revalidate read") && error.contains("os error 3"),
        "{error}"
    );
    assert!(owner.processes.is_empty());
    assert!(child.child.try_wait().unwrap().is_none());
    child.finish().assert_success();
}

#[test]
fn expired_observation_cannot_acquire_identity_or_complete_a_scan() {
    let mut child = Fixture::new("child");
    let pid = child.pid();
    let mut owner = registry(pid);
    let expired = OwnershipBudget {
        deadline: Instant::now(),
        cancel: None,
    };
    assert!(matches!(
        owner.discover(&expired),
        Err(OwnershipError::Timeout)
    ));
    assert!(!owner.complete_scan);
    assert!(owner.processes.is_empty());
    assert!(child.child.try_wait().unwrap().is_none());
    child.finish().assert_success();
}

fn replace_stat(pid: i32, text: String) {
    arm(Step::StatOpened, pid, move |file| {
        let mut replacement = tempfile::tempfile()?;
        replacement.write_all(text.as_bytes())?;
        replacement.rewind()?;
        *file.unwrap() = replacement;
        Ok(())
    });
}
#[test]
fn malformed_stat_and_changed_identity_never_acquire_signal_authority() {
    for revalidate in [false, true] {
        let mut child = Fixture::new("child");
        let pid = child.pid();
        if revalidate {
            arm(Step::PidfdOpened, pid, move |_| {
                replace_stat(pid, "malformed".into());
                Ok(())
            });
        } else {
            replace_stat(pid, "malformed".into());
        }
        let mut owner = registry(pid);
        assert!(
            owner
                .observe_pid(pid, &budget())
                .unwrap_err()
                .to_string()
                .contains("invalid proc stat")
        );
        consumed();
        assert!(owner.processes.is_empty());
        assert!(child.child.try_wait().unwrap().is_none());
        child.finish().assert_success();
    }
    // The revalidated birth tuple must agree, even when a live stable pidfd
    // exists. Model a reused numeric PID without trying to force kernel reuse.
    let mut child = Fixture::new("child");
    let pid = child.pid();
    let stat = read_stat(pid).unwrap();
    let (comm, rest) = stat.rsplit_once(')').unwrap();
    let mut fields: Vec<_> = rest.split_whitespace().map(str::to_owned).collect();
    fields[19] = (fields[19].parse::<u64>().unwrap() + 1).to_string();
    let changed = format!("{comm}) {}", fields.join(" "));
    arm(Step::PidfdOpened, pid, move |_| {
        replace_stat(pid, changed);
        Ok(())
    });
    let mut owner = registry(pid);
    assert!(
        !owner.observe_pid(pid, &budget()).unwrap(),
        "changed identity must leave the scan incomplete"
    );
    consumed();
    assert!(owner.processes.is_empty());
    owner.signal(libc::SIGTERM, &budget()).unwrap();
    assert!(child.child.try_wait().unwrap().is_none());
    // A later complete, consistent observation can acquire the real identity.
    assert!(owner.observe_pid(pid, &budget()).unwrap());
    assert!(owner.processes.contains_key(&pid));
    child.finish().assert_success();
}
