//! AG4.1 witnesses use real procfs descriptors. No hook can replace an I/O result.
//!
//! The supervising test owns auxiliary A and isolated observer O as exact Child
//! handles, each in its own group. Only O's real Owner owns command root R.
//! A's descriptor opens before O asks the supervisor to release and reap A;
//! only the exact-child reap acknowledgement permits the original read.
//! The control adds supervisor-owned group leader G, kept alive through the scan.
//!
//! A valid red witness requires the target ESRCH in O's first observation,
//! successful root exit, the same propagated/sticky error, production reaping,
//! and verified fixture teardown. Any missing evidence is inconclusive and
//! still fails the test. These fixtures cannot qualify translated Linux as native.
use super::super::{
    CLEANUP_LIMIT, Commands, GroupObservation, NATURAL_LIMIT, Owner, Spec, TERM_LIMIT,
};
use std::{
    cell::RefCell,
    io::{self, Read, Write},
    os::{
        fd::{AsFd, AsRawFd},
        unix::net::UnixStream,
        unix::process::CommandExt,
    },
    process::{Child, Command, ExitStatus, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const OUTER_LIMIT: Duration = Duration::from_secs(15); // Existing isolated-owner guard.
const HELPER: &str = "aws::command::linux::tests::procfs_helper";
const ROOT_MARKER: &str = "AG41_COMMAND_ROOT_OK";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Tag {
    Ready = 1,
    Start = 2,
    Opened = 3,
    Release = 4,
    Reaped = 5,
    Complete = 6,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Outcome {
    Pass = 1,
    ObserverGap = 2,
    Inconclusive = 3,
}
struct Frame {
    tag: Tag,
    pid: i32,
    group: i32,
    outcome: Outcome,
    deadline: u64,
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

// CLOCK_MONOTONIC is shared by these local Linux processes. Conservative
// conversion pairs avoid granting extra time while transmitting an Instant.
fn monotonic_ns() -> Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(u64::try_from(time.tv_sec)? * 1_000_000_000 + u64::try_from(time.tv_nsec)?)
}
fn wire_deadline(deadline: Instant) -> Result<u64> {
    let clock = monotonic_ns()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    Ok(clock
        .checked_add(u64::try_from(remaining.as_nanos())?)
        .ok_or("deadline overflow")?)
}
fn local_deadline(clock: u64) -> Result<Instant> {
    let now = Instant::now();
    Ok(now + Duration::from_nanos(clock.saturating_sub(monotonic_ns()?)))
}

fn wait(fd: i32, events: i16, deadline: Instant) -> Result<()> {
    loop {
        require(
            Instant::now() < deadline,
            "synchronization deadline exhausted",
        )?;
        let mut poll = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let timeout = super::super::poll_timeout(Instant::now(), deadline);
        let rc = unsafe { libc::poll(&mut poll, 1, timeout) };
        if rc < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        require(
            poll.revents & libc::POLLNVAL == 0,
            "invalid control descriptor",
        )?;
        if rc > 0 {
            return Ok(());
        }
    }
}

struct Channel {
    stream: UnixStream,
    token: [u8; 16],
}
impl Channel {
    fn new(stream: UnixStream, token: [u8; 16]) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self { stream, token })
    }
    fn stdin(token: [u8; 16]) -> Result<Self> {
        Self::new(
            UnixStream::from(io::stdin().as_fd().try_clone_to_owned()?),
            token,
        )
    }
    fn send(&mut self, frame: Frame, deadline: Instant) -> Result<()> {
        let mut bytes = [0_u8; 36];
        bytes[0] = 1;
        bytes[1] = frame.tag as u8;
        bytes[2] = frame.outcome as u8;
        bytes[4..8].copy_from_slice(&frame.pid.to_le_bytes());
        bytes[8..12].copy_from_slice(&frame.group.to_le_bytes());
        bytes[12..28].copy_from_slice(&self.token);
        bytes[28..36].copy_from_slice(&frame.deadline.to_le_bytes());
        let mut written = 0;
        while written < bytes.len() {
            require(
                Instant::now() < deadline,
                "control write deadline exhausted",
            )?;
            match self.stream.write(&bytes[written..]) {
                Ok(0) => return Err("control write closed".into()),
                Ok(n) => written += n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    wait(self.stream.as_fd().as_raw_fd(), libc::POLLOUT, deadline)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        require(
            Instant::now() < deadline,
            "control write completed too late",
        )
    }
    fn receive(&mut self, deadline: Instant) -> Result<Frame> {
        let mut bytes = [0_u8; 36];
        let mut read = 0;
        while read < bytes.len() {
            require(Instant::now() < deadline, "control read deadline exhausted")?;
            match self.stream.read(&mut bytes[read..]) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into()),
                Ok(n) => read += n,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    wait(self.stream.as_fd().as_raw_fd(), libc::POLLIN, deadline)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        require(Instant::now() < deadline, "control read completed too late")?;
        require(
            bytes[0] == 1 && bytes[3] == 0 && bytes[12..28] == self.token,
            "invalid control identity/version",
        )?;
        let tag = match bytes[1] {
            1 => Tag::Ready,
            2 => Tag::Start,
            3 => Tag::Opened,
            4 => Tag::Release,
            5 => Tag::Reaped,
            6 => Tag::Complete,
            _ => return Err("invalid control tag".into()),
        };
        let outcome = match bytes[2] {
            1 => Outcome::Pass,
            2 => Outcome::ObserverGap,
            3 => Outcome::Inconclusive,
            _ => return Err("invalid control outcome".into()),
        };
        Ok(Frame {
            tag,
            pid: i32::from_le_bytes(bytes[4..8].try_into()?),
            group: i32::from_le_bytes(bytes[8..12].try_into()?),
            outcome,
            deadline: u64::from_le_bytes(bytes[28..36].try_into()?),
        })
    }
}

fn frame(tag: Tag, pid: i32, deadline: Instant) -> Result<Frame> {
    Ok(Frame {
        tag,
        pid,
        group: pid,
        outcome: Outcome::Pass,
        deadline: wire_deadline(deadline)?,
    })
}
fn parent_guard() -> Result<()> {
    let expected: i32 = std::env::var("AG41_PARENT")?.parse()?;
    require(expected > 1, "invalid expected parent")?;
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    require(
        unsafe { libc::getppid() } == expected,
        "fixture lost expected parent",
    )
}

struct Fixture {
    role: &'static str,
    child: Child,
    channel: Option<Channel>,
    status: Option<ExitStatus>,
    wait_failed: bool,
}
impl Fixture {
    fn spawn(
        role: &'static str,
        token: [u8; 16],
        deadline: Instant,
        target: i32,
        group: i32,
    ) -> Result<Self> {
        let (parent, child) = UnixStream::pair()?;
        let channel = Channel::new(parent, token)?;
        let token = token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        // The command and passed socket end are dropped immediately after spawn.
        // There are no fallible operations between spawn and installing ownership.
        let child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                HELPER,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("AG41_ROLE", role)
            .env("AG41_TOKEN", token)
            .env("AG41_PARENT", unsafe { libc::getpid() }.to_string())
            .env("AG41_DEADLINE", wire_deadline(deadline)?.to_string())
            .env("AG41_TARGET", target.to_string())
            .env("AG41_GROUP", group.to_string())
            .stdin(Stdio::from(std::os::fd::OwnedFd::from(child)))
            .process_group(0)
            .spawn()?;
        Ok(Self {
            role,
            child,
            channel: Some(channel),
            status: None,
            wait_failed: false,
        })
    }
    fn pid(&self) -> i32 {
        self.child.id().try_into().expect("native pid")
    }
    fn channel(&mut self) -> &mut Channel {
        self.channel.as_mut().expect("live channel")
    }
    fn ready(&mut self, deadline: Instant) -> Result<()> {
        let ready = self.channel().receive(deadline)?;
        require(
            ready.tag == Tag::Ready && ready.pid == self.pid() && ready.group == self.pid(),
            "fixture readiness identity mismatch",
        )
    }
    fn poll(&mut self) -> Result<bool> {
        require(
            !self.wait_failed,
            "exact-child wait authority previously failed",
        )?;
        if self.status.is_none() {
            match self.child.try_wait() {
                Ok(status) => self.status = status,
                Err(error) => {
                    self.wait_failed = true;
                    return Err(error.into());
                }
            }
        }
        Ok(self.status.is_some())
    }
    fn reap(&mut self, deadline: Instant) -> Result<ExitStatus> {
        loop {
            require(
                Instant::now() < deadline,
                "exact-child reap deadline exhausted",
            )?;
            if self.poll()? {
                require(
                    Instant::now() < deadline,
                    "exact-child reap completed too late",
                )?;
                return Ok(self.status.unwrap());
            }
            let rc = unsafe {
                libc::poll(
                    std::ptr::null_mut(),
                    0,
                    super::super::poll_timeout(Instant::now(), deadline),
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error.into());
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Teardown {
    statuses: Vec<(&'static str, Option<ExitStatus>)>,
    errors: Vec<String>,
}
struct Supervisor {
    auxiliary: Option<Fixture>,
    observer: Option<Fixture>,
    member: Option<Fixture>,
    completion: Rc<RefCell<Option<Teardown>>>,
}
impl Supervisor {
    fn new(completion: Rc<RefCell<Option<Teardown>>>) -> Self {
        Self {
            auxiliary: None,
            observer: None,
            member: None,
            completion,
        }
    }
    fn finish(&mut self) -> Teardown {
        if let Some(done) = self.completion.borrow().as_ref() {
            return done.clone();
        }
        // Close every channel first, then share one absolute teardown authority.
        let fixtures = [&mut self.auxiliary, &mut self.observer, &mut self.member];
        let mut fixtures: Vec<_> = fixtures.into_iter().filter_map(Option::as_mut).collect();
        for fixture in &mut fixtures {
            fixture.channel = None;
        }
        let start = Instant::now();
        let end = start + CLEANUP_LIMIT;
        let mut done = Teardown::default();
        loop {
            let mut all = true;
            let mut failed = false;
            for fixture in &mut fixtures {
                match fixture.poll() {
                    Ok(exited) => all &= exited,
                    Err(error) => {
                        done.errors.push(format!("{} wait: {error}", fixture.role));
                        all = false;
                        failed = true;
                    }
                }
            }
            if all || failed || Instant::now() >= start + TERM_LIMIT {
                break;
            }
            let rc = unsafe {
                libc::poll(
                    std::ptr::null_mut(),
                    0,
                    super::super::poll_timeout(Instant::now(), start + TERM_LIMIT),
                )
            };
            if rc < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    done.errors.push(format!("teardown poll: {error}"));
                    break;
                }
            }
        }
        for fixture in &mut fixtures {
            if fixture.status.is_none() && !fixture.wait_failed {
                // Only the retained, unreaped Child is authority. A successful
                // signal is never recorded as a successful reap or clean run.
                done.errors
                    .push(format!("{} needed forced teardown", fixture.role));
                if let Err(error) = fixture.child.kill() {
                    done.errors.push(format!("{} kill: {error}", fixture.role));
                }
            }
        }
        for fixture in &mut fixtures {
            if fixture.status.is_none()
                && !fixture.wait_failed
                && let Err(error) = fixture.reap(end)
            {
                done.errors.push(format!("{} reap: {error}", fixture.role));
            }
            if !fixture.status.is_some_and(|status| status.success()) {
                done.errors.push(format!(
                    "{} unsuccessful or unreaped: {:?}",
                    fixture.role, fixture.status
                ));
            }
            done.statuses.push((fixture.role, fixture.status));
        }
        if Instant::now() >= end {
            done.errors
                .push("fixture teardown deadline exhausted".into());
        }
        *self.completion.borrow_mut() = Some(done.clone());
        done
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        let done = self.finish();
        if !done.errors.is_empty() {
            let _ = writeln!(io::stderr().lock(), "AG41_INCONCLUSIVE teardown: {done:?}");
        }
    }
}

struct ReadEvidence {
    at: Instant,
    errno: Option<i32>,
    error: Option<String>,
}
struct Probe {
    target: i32,
    channel: Channel,
    opens: usize,
    reads: usize,
    opened: Option<Instant>,
    acknowledged: Option<Instant>,
    deadline: Option<Instant>,
    read: Option<ReadEvidence>,
    error: Option<String>,
}
thread_local! {
    static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
}
struct ProbeGuard;
impl ProbeGuard {
    fn install(target: i32, channel: Channel) -> Result<Self> {
        PROBE.with(|slot| {
            require(slot.borrow().is_none(), "probe already installed")?;
            *slot.borrow_mut() = Some(Probe {
                target,
                channel,
                opens: 0,
                reads: 0,
                opened: None,
                acknowledged: None,
                deadline: None,
                read: None,
                error: None,
            });
            Ok(Self)
        })
    }
    fn take(self) -> Result<Probe> {
        PROBE.with(|slot| {
            slot.borrow_mut()
                .take()
                .ok_or_else(|| "probe missing".into())
        })
    }
}
impl Drop for ProbeGuard {
    fn drop(&mut self) {
        PROBE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

pub(super) fn stat_opened(pid: i32, deadline: Instant) {
    let probe = PROBE.with(|slot| slot.borrow_mut().take());
    let Some(mut probe) = probe else {
        return;
    };
    if pid == probe.target && probe.opens == 0 {
        probe.opens += 1;
        probe.opened = Some(Instant::now());
        probe.deadline = Some(deadline);
        eprintln!(
            "AG41_STAT_OPENED pid={pid} remaining_us={}",
            deadline
                .saturating_duration_since(Instant::now())
                .as_micros()
        );
        let mut exchange = || -> Result<()> {
            probe
                .channel
                .send(frame(Tag::Opened, pid, deadline)?, deadline)?;
            let ack = probe.channel.receive(deadline)?;
            require(
                ack.tag == Tag::Reaped && ack.pid == pid && ack.group == pid,
                "wrong exact-child reap acknowledgement",
            )?;
            probe.acknowledged = Some(Instant::now());
            Ok(())
        };
        if let Err(error) = exchange() {
            probe.error = Some(error.to_string());
        }
    }
    PROBE.with(|slot| *slot.borrow_mut() = Some(probe));
}

pub(super) fn stat_read_finished(pid: i32, result: &io::Result<usize>) {
    PROBE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(probe) = slot
            .as_mut()
            .filter(|probe| probe.target == pid && probe.opens == 1)
        else {
            return;
        };
        probe.reads += 1;
        if probe.read.is_none() {
            let at = Instant::now();
            let errno = result.as_ref().err().and_then(io::Error::raw_os_error);
            eprintln!("AG41_STAT_READ pid={pid} raw_errno={errno:?} result={result:?}");
            probe.read = Some(ReadEvidence {
                at,
                errno,
                error: result.as_ref().err().map(ToString::to_string),
            });
        }
    });
}
impl Probe {
    fn validate(&self) -> Result<&ReadEvidence> {
        require(
            self.error.is_none(),
            &format!("coordination failed: {:?}", self.error),
        )?;
        require(
            self.opens == 1 && self.reads == 1,
            "hook not consumed exactly once",
        )?;
        let opened = self.opened.ok_or("missing successful open")?;
        let ack = self
            .acknowledged
            .ok_or("missing exact reap acknowledgement")?;
        let read = self.read.as_ref().ok_or("missing real read result")?;
        require(
            opened <= ack && ack <= read.at,
            "invalid open/reap/read ordering",
        )?;
        require(
            read.at < self.deadline.ok_or("missing scan deadline")?,
            "coordination/read consumed scan deadline",
        )?;
        require(
            read.errno == Some(libc::ESRCH),
            &format!("kernel observation inconclusive: errno={:?}", read.errno),
        )?;
        Ok(read)
    }
}

fn root_spec() -> Result<Spec> {
    let mut spec = Spec::new(
        std::env::current_exe()?,
        [
            "--exact",
            HELPER,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ],
    );
    spec.env = vec![
        ("AG41_ROLE".into(), "root".into()),
        (
            "AG41_PARENT".into(),
            unsafe { libc::getpid() }.to_string().into(),
        ),
    ];
    spec.stdout_limit = 4096;
    // The unchanged owner grants this natural phase only after its own
    // WNOWAIT success check. Its boundary therefore proves root success even
    // when Failure does not expose an exit code. No test reaps/observes R.
    spec.eice_proxy = true;
    Ok(spec)
}
fn owner_case(target: i32, channel: Channel) -> Result<(Outcome, Channel)> {
    let mut owner = Owner::enter().map_err(io::Error::other)?;
    let guard = ProbeGuard::install(target, channel)?;
    let result = owner.run(root_spec()?);
    // Keep genuine failures visible even when a later witness precondition is
    // missing. Diagnostic recording cannot repair or replace the result.
    eprintln!("AG41_OWNER_RAW_RESULT target_pid={target} result={result:?}");
    let probe = guard.take()?;
    let read = probe.validate()?;
    let observations = owner
        .timing
        .iter()
        .filter(|(name, at)| *name == "group_observation" && *at <= probe.opened.unwrap())
        .count();
    require(
        observations == 1,
        "target read was not in the first owner observation",
    )?;
    let event = |name| {
        owner
            .timing
            .iter()
            .find(|(label, _)| *label == name)
            .map(|(_, at)| *at)
    };
    let started = event("cleanup_started").ok_or("missing cleanup start")?;
    require(
        probe.deadline == Some(started + NATURAL_LIMIT),
        "owner did not confirm successful root exit before the first scan",
    )?;
    let confirmed =
        event("group_quiescence_confirmed").ok_or("missing complete group observation")?;
    let signals_done = event("signaling_complete").ok_or("missing signaling completion")?;
    let root = event("root_reaped").ok_or("missing production root reap")?;
    let adopted = event("descendants_reaped").ok_or("missing native descendant reap")?;
    require(
        event("stdout_eof").is_some_and(|at| at <= confirmed)
            && event("stderr_eof").is_some_and(|at| at <= confirmed),
        "both pipe EOFs must precede finalization",
    )?;
    require(
        confirmed <= signals_done
            && signals_done <= root
            && root <= adopted
            && adopted < started + CLEANUP_LIMIT,
        "ownership finalization incomplete or late",
    )?;
    let poisoned = owner.poisoned;
    eprintln!(
        "AG41_OWNER_RESULT target_pid={target} first_observation={observations} root_success_confirmed=true poisoned={poisoned} result={result:?}"
    );
    for (name, at) in &owner.timing {
        eprintln!(
            "AG41_OWNER_EVENT target_pid={target} {name} us={}",
            at.saturating_duration_since(started).as_micros()
        );
    }
    let outcome = match &result {
        Ok(output)
            if output.code == 0
                && output
                    .stdout
                    .windows(ROOT_MARKER.len())
                    .any(|text| text == ROOT_MARKER.as_bytes())
                && !poisoned =>
        {
            super::super::tests::assert_authority_order(&owner);
            require(owner.run(root_spec()?).is_ok(), "subsequent command failed")?;
            require(owner.finish().is_ok(), "owner finish failed")?;
            Outcome::Pass
        }
        Err(failure)
            if failure.cleanup.as_ref() == read.error.as_ref()
                && Some(&failure.primary) == read.error.as_ref()
                && poisoned =>
        {
            require(
                owner.finish().err().as_deref() == Some("controller command cleanup unresolved"),
                "cleanup failure was not retained at finish",
            )?;
            Outcome::ObserverGap
        }
        _ => return Err(format!("unrelated owner failure: {result:?}").into()),
    };
    Ok((outcome, probe.channel))
}
fn member_case(
    target: i32,
    group: i32,
    channel: Channel,
    deadline: Instant,
) -> Result<(Outcome, Channel)> {
    let guard = ProbeGuard::install(target, channel)?;
    let result = super::group_running(group, deadline);
    eprintln!("AG41_SURVIVING_MEMBER group={group} observation={result:?}");
    let probe = guard.take()?;
    let read = probe.validate()?;
    let outcome = match result {
        Ok(GroupObservation::Executing) => Outcome::Pass,
        Err(ref error) if Some(error) == read.error.as_ref() => Outcome::ObserverGap,
        _ => return Err("surviving member was concealed or scan was incomplete".into()),
    };
    Ok((outcome, probe.channel))
}

#[test]
#[ignore = "private AG4.1 fixture; only launched with a dedicated control socket"]
fn procfs_helper() {
    if let Err(error) = helper() {
        eprintln!("AG41_INCONCLUSIVE helper: {error}");
        panic!("AG4.1 helper did not complete its protocol");
    }
}
fn helper() -> Result<()> {
    parent_guard()?;
    let role = std::env::var("AG41_ROLE")?;
    if role == "root" {
        println!("{ROOT_MARKER}");
        return Ok(());
    }
    let token = std::env::var("AG41_TOKEN")?;
    require(
        token.len() == 32 && token.is_ascii(),
        "invalid fixture token",
    )?;
    let mut decoded = [0_u8; 16];
    for (index, byte) in decoded.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&token[index * 2..index * 2 + 2], 16)?;
    }
    let deadline = local_deadline(std::env::var("AG41_DEADLINE")?.parse()?)?;
    let mut channel = Channel::stdin(decoded)?;
    let pid = unsafe { libc::getpid() };
    let mut ready = frame(Tag::Ready, pid, deadline)?;
    ready.group = unsafe { libc::getpgrp() };
    channel.send(ready, deadline)?;
    let command = match channel.receive(deadline) {
        Ok(command) => command,
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|error| error.kind() == io::ErrorKind::UnexpectedEof) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if role == "auxiliary" || role == "member" {
        return require(
            command.tag == Tag::Release && command.pid == pid,
            "wrong fixture release",
        );
    }
    require(
        command.tag == Tag::Start && command.pid == pid,
        "wrong observer start",
    )?;
    let target: i32 = std::env::var("AG41_TARGET")?.parse()?;
    require(target > 1 && target != pid, "invalid unrelated target")?;
    let result = match role.as_str() {
        "owner" => owner_case(target, channel),
        "scan" => member_case(
            target,
            std::env::var("AG41_GROUP")?.parse()?,
            channel,
            deadline.min(Instant::now() + CLEANUP_LIMIT),
        ),
        _ => return Err("unknown fixture role".into()),
    };
    let (outcome, mut channel) = result?;
    let mut complete = frame(Tag::Complete, pid, deadline)?;
    complete.outcome = outcome;
    channel.send(complete, deadline)
}

fn exercise(
    supervisor: &mut Supervisor,
    member: bool,
    token: [u8; 16],
    deadline: Instant,
) -> Result<Outcome> {
    supervisor.auxiliary = Some(Fixture::spawn("auxiliary", token, deadline, 0, 0)?);
    let auxiliary = supervisor.auxiliary.as_mut().unwrap();
    auxiliary.ready(deadline)?;
    let target = auxiliary.pid();
    let group = if member {
        supervisor.member = Some(Fixture::spawn("member", token, deadline, 0, 0)?);
        let group = supervisor.member.as_mut().unwrap();
        group.ready(deadline)?;
        group.pid()
    } else {
        0
    };
    supervisor.observer = Some(Fixture::spawn(
        if member { "scan" } else { "owner" },
        token,
        deadline,
        target,
        group,
    )?);
    let observer = supervisor.observer.as_mut().unwrap();
    observer.ready(deadline)?;
    let observer_pid = observer.pid();
    require(
        target != observer_pid && target != group,
        "fixture groups overlap",
    )?;
    observer
        .channel()
        .send(frame(Tag::Start, observer_pid, deadline)?, deadline)?;
    let opened = observer.channel().receive(deadline)?;
    require(
        opened.tag == Tag::Opened && opened.pid == target && opened.group == target,
        "wrong successful-open notification",
    )?;
    let scan_end = deadline.min(local_deadline(opened.deadline)?);
    let auxiliary = supervisor.auxiliary.as_mut().unwrap();
    auxiliary
        .channel()
        .send(frame(Tag::Release, target, scan_end)?, scan_end)?;
    require(
        auxiliary.reap(scan_end)?.success(),
        "unrelated fixture did not exit successfully",
    )?;
    eprintln!(
        "AG41_EXACT_CHILD_REAPED pid={target} status={:?}",
        auxiliary.status
    );
    let observer = supervisor.observer.as_mut().unwrap();
    observer
        .channel()
        .send(frame(Tag::Reaped, target, scan_end)?, scan_end)?;
    let complete = observer.channel().receive(deadline)?;
    require(
        complete.tag == Tag::Complete && complete.pid == observer_pid,
        "missing observer completion",
    )?;
    if let Some(group) = supervisor.member.as_mut() {
        require(
            !group.poll()?,
            "controlled group member did not survive the observation",
        )?;
        let pid = group.pid();
        group
            .channel()
            .send(frame(Tag::Release, pid, deadline)?, deadline)?;
        require(
            group.reap(deadline)?.success(),
            "member fixture exit failed",
        )?;
        eprintln!("AG41_MEMBER_REAPED pid={pid}");
    }
    require(
        observer.reap(deadline)?.success(),
        "observer helper did not exit successfully",
    )?;
    eprintln!(
        "AG41_OBSERVER_REAPED pid={observer_pid} status={:?}",
        observer.status
    );
    Ok(complete.outcome)
}

pub(in crate::aws::command) fn supervise(member: bool) {
    let mut token = [0_u8; 16];
    getrandom::fill(&mut token).unwrap();
    let completion = Rc::new(RefCell::new(None));
    let mut supervisor = Supervisor::new(completion);
    let result = exercise(&mut supervisor, member, token, Instant::now() + OUTER_LIMIT);
    let teardown = supervisor.finish();
    if !teardown.errors.is_empty() {
        panic!("AG41_INCONCLUSIVE teardown: {teardown:?}; case={result:?}");
    }
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => panic!("AG41_INCONCLUSIVE synchronization/kernel/owner evidence: {error}"),
    };
    if outcome == Outcome::Inconclusive {
        panic!("AG41_INCONCLUSIVE helper reported incomplete evidence");
    }
    if outcome == Outcome::ObserverGap {
        if member {
            eprintln!("AG41_CONTROL_OBSERVER_ERROR teardown_verified=true");
        } else {
            eprintln!(
                "AG41_RED_WITNESS target_pid={} real_ESRCH=true first_observation=true root_success_confirmed=true propagated=true sticky=true root_and_native_reap=true teardown_verified=true",
                supervisor.auxiliary.as_ref().unwrap().pid()
            );
        }
    }
    assert_eq!(
        outcome,
        Outcome::Pass,
        "AG4.1 normal ownership-completion assertion failed; member_control={member}"
    );
}

pub(in crate::aws::command) fn teardown_on_unwind() {
    let completion = Rc::new(RefCell::new(None));
    let saved = completion.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut supervisor = Supervisor::new(completion);
        let deadline = Instant::now() + OUTER_LIMIT;
        supervisor.auxiliary = Some(Fixture::spawn("auxiliary", [7; 16], deadline, 0, 0).unwrap());
        supervisor
            .auxiliary
            .as_mut()
            .unwrap()
            .ready(deadline)
            .unwrap();
        supervisor.observer = Some(Fixture::spawn("owner", [7; 16], deadline, 0, 0).unwrap());
        supervisor
            .observer
            .as_mut()
            .unwrap()
            .ready(deadline)
            .unwrap();
        supervisor.member = Some(Fixture::spawn("member", [7; 16], deadline, 0, 0).unwrap());
        supervisor.member.as_mut().unwrap().ready(deadline).unwrap();
        panic!("intentional fixture-guard unwind");
    }));
    let panic = result.expect_err("fixture did not exercise unwinding");
    assert_eq!(
        panic.downcast_ref::<&str>().copied(),
        Some("intentional fixture-guard unwind"),
        "setup failed instead of reaching the intentional unwind"
    );
    let done = saved.borrow();
    let done = done
        .as_ref()
        .expect("unwind did not finish exact-child teardown");
    assert!(done.errors.is_empty(), "{done:?}");
    assert_eq!(
        done.statuses.len(),
        3,
        "setup failed before intentional unwind: {done:?}"
    );
    assert!(
        done.statuses
            .iter()
            .all(|(_, status)| status.is_some_and(|status| status.success())),
        "{done:?}"
    );
    eprintln!("AG41_UNWIND_TEARDOWN_VERIFIED statuses={:?}", done.statuses);
}
