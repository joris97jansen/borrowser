//! Descriptor-rooted immutable journal. reserve/ and staging/ are never replayed.
use crate::identity::*;
use crate::{Error, Result, canonical, model::*, require};
#[cfg(not(target_os = "linux"))]
use std::os::fd::FromRawFd;
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsRawFd, unix::fs::MetadataExt},
};

pub(crate) const ROOT: &str = "/var/lib/borrowser-host-lifecycle";
const RESERVE_SLOTS: u64 = 64;
const MAX_EVENTS: u64 = 16_384;

fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| Error("authority I/O"))
}
fn name(s: &str) -> Result<CString> {
    require(
        !s.is_empty() && !s.contains('/') && s != "." && s != "..",
        "invalid authority component",
    )?;
    CString::new(s).map_err(|_| Error("authority component NUL"))
}
fn open_at(dir: &File, component: &str, flags: i32) -> Result<File> {
    #[cfg(target_os = "linux")]
    {
        name(component)?;
        crate::linux::confined(dir, component, flags, true)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let n = name(component)?;
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                n.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        require(fd >= 0, "descriptor-relative open")?;
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}
fn directory(dir: &File, component: &str) -> Result<File> {
    let file = open_at(dir, component, libc::O_RDONLY | libc::O_DIRECTORY)?;
    let m = io(file.metadata())?;
    let parent = io(dir.metadata())?;
    require(
        m.dev() == parent.dev() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
        "authority directory protection/device",
    )?;
    Ok(file)
}
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn mkdir(dir: &File, component: &str) -> Result<File> {
    let n = name(component)?;
    require(
        unsafe { libc::mkdirat(dir.as_raw_fd(), n.as_ptr(), 0o700) } == 0,
        "exclusive directory create",
    )?;
    io(dir.sync_all())?;
    directory(dir, component)
}
// libc's statvfs integer widths differ between Darwin tests and Linux production.
#[allow(clippy::unnecessary_cast)]
fn available(dir: &File) -> Result<(u64, u64)> {
    #[cfg(test)]
    if let Some(v) = SPACE.with(|s| s.get()) {
        return Ok(v);
    }
    let mut v = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    require(
        unsafe { libc::fstatvfs(dir.as_raw_fd(), v.as_mut_ptr()) } == 0,
        "filesystem capacity",
    )?;
    let v = unsafe { v.assume_init() };
    let bytes = (v.f_bavail as u64)
        .checked_mul(v.f_frsize as u64)
        .ok_or(Error("capacity overflow"))?;
    Ok((bytes, v.f_favail as u64))
}
fn entries(dir: &File) -> Result<Vec<String>> {
    let fd = unsafe {
        libc::openat(
            dir.as_raw_fd(),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    require(fd >= 0, "directory enumeration open")?;
    let dp = unsafe { libc::fdopendir(fd) };
    if dp.is_null() {
        unsafe {
            libc::close(fd);
        }
        return Err(Error("directory enumeration"));
    }
    let mut names = Vec::new();
    loop {
        #[cfg(target_os = "linux")]
        let errno = unsafe { libc::__errno_location() };
        #[cfg(target_os = "macos")]
        let errno = unsafe { libc::__error() };
        unsafe {
            *errno = 0;
        }
        let entry = unsafe { libc::readdir(dp) };
        if entry.is_null() {
            let error = unsafe { *errno };
            if error != 0 {
                unsafe {
                    libc::closedir(dp);
                }
                return Err(Error("directory enumeration interrupted"));
            }
            break;
        }
        let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let s = raw.to_str().map(str::to_owned);
        match s {
            Ok(s) if s == "." || s == ".." => {}
            Ok(s) if names.len() < MAX_EVENTS as usize + 128 => names.push(s),
            _ => {
                unsafe {
                    libc::closedir(dp);
                }
                return Err(Error("directory entry bound/encoding"));
            }
        }
    }
    require(unsafe { libc::closedir(dp) } == 0, "directory close")?;
    names.sort();
    Ok(names)
}
fn regular(f: &File) -> Result<()> {
    let m = io(f.metadata())?;
    require(
        m.is_file()
            && m.nlink() == 1
            && m.uid() == unsafe { libc::geteuid() }
            && m.mode() & 0o077 == 0,
        "private regular authority file",
    )
}
fn same(a: &File, b: &File) -> Result<()> {
    let a = io(a.metadata())?;
    let b = io(b.metadata())?;
    require(
        a.dev() == b.dev() && a.ino() == b.ino(),
        "authority object replaced",
    )
}
#[cfg(test)]
thread_local! { static FAULT: std::cell::Cell<Option<&'static str>> = const {std::cell::Cell::new(None)}; static SPACE: std::cell::Cell<Option<(u64,u64)>> = const {std::cell::Cell::new(None)}; }
fn checkpoint(_name: &'static str) -> Result<()> {
    #[cfg(test)]
    if FAULT.with(|f| f.get() == Some(_name)) {
        return Err(Error("injected publication failure"));
    }
    Ok(())
}
fn publish_name(from: &File, source: &str, to: &File, target: &str) -> Result<()> {
    let s = name(source)?;
    let t = name(target)?;
    #[cfg(target_os = "linux")]
    let rc = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            from.as_raw_fd(),
            s.as_ptr(),
            to.as_raw_fd(),
            t.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let rc = unsafe {
        libc::renameatx_np(
            from.as_raw_fd(),
            s.as_ptr(),
            to.as_raw_fd(),
            t.as_ptr(),
            libc::RENAME_EXCL,
        )
    } as i64;
    require(rc == 0, "exclusive event publication")?;
    checkpoint("directory-sync")?;
    io(to.sync_all())?;
    io(from.sync_all())
}

/// Owned local lock plus retained directories. No path-based I/O after opening.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) struct Journal {
    root: File,
    lock: File,
    journal: File,
    staging: File,
    reserve: File,
    state: AccountState,
    poisoned: bool,
}
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl Journal {
    /// Production root validation must precede this constructor. Kept crate-private;
    /// alternate roots exist only in unit tests.
    pub(crate) fn open_verified(root: File) -> Result<Self> {
        let lock = open_at(&root, "lock", libc::O_RDWR)?;
        regular(&lock)?;
        require(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "authority busy",
        )?;
        same(&lock, &open_at(&root, "lock", libc::O_RDWR)?)?;
        let journal = directory(&root, "journal")?;
        let staging = directory(&root, "staging")?;
        let reserve = directory(&root, "reserve")?;
        let mut this = Self {
            root,
            lock,
            journal,
            staging,
            reserve,
            state: AccountState::default(),
            poisoned: false,
        };
        this.replay()?;
        Ok(this)
    }
    pub(crate) fn bootstrap_verified(root: File) -> Result<Self> {
        // A fresh dedicated ext4 root may have its filesystem-created lost+found.
        require(
            entries(&root)?.iter().all(|n| n == "lost+found"),
            "authority already initialized or unexpected entries",
        )?;
        let lock = open_at(&root, "lock", libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)?;
        require(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "bootstrap lock",
        )?;
        io(lock.sync_all())?;
        io(root.sync_all())?;
        let journal = mkdir(&root, "journal")?;
        let staging = mkdir(&root, "staging")?;
        let reserve = mkdir(&root, "reserve")?;
        mkdir(&root, "evidence")?;
        for slot in 0..RESERVE_SLOTS {
            let mut f = open_at(
                &reserve,
                &format!("{slot:04}.slot"),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            )?;
            // Actual writes allocate bytes; set_len alone could create sparse files.
            io(f.write_all(&vec![0; canonical::EVENT_BYTES]))?;
            io(f.sync_all())?;
        }
        io(reserve.sync_all())?;
        Ok(Self {
            root,
            lock,
            journal,
            staging,
            reserve,
            state: AccountState::default(),
            poisoned: false,
        })
    }
    fn check_objects(&self) -> Result<()> {
        same(&self.lock, &open_at(&self.root, "lock", libc::O_RDWR)?)?;
        same(&self.journal, &directory(&self.root, "journal")?)?;
        same(&self.staging, &directory(&self.root, "staging")?)?;
        same(&self.reserve, &directory(&self.root, "reserve")?)
    }
    pub(crate) fn state(&self) -> &AccountState {
        &self.state
    }
    pub(crate) fn admit_allocation(&self) -> Result<()> {
        self.check_objects()?;
        let (bytes, inodes) = available(&self.root)?;
        let slots = entries(&self.reserve)?;
        require(slots.len() == RESERVE_SLOTS as usize, "reserve slot count")?;
        for slot in 0..RESERVE_SLOTS {
            let f = open_at(&self.reserve, &format!("{slot:04}.slot"), libc::O_RDONLY)?;
            regular(&f)?;
            require(
                io(f.metadata())?.len() == canonical::EVENT_BYTES as u64,
                "reserve slot size",
            )?;
        }
        let evidence = directory(&self.root, "evidence")?;
        let artifacts = entries(&evidence)?;
        require(artifacts.len() <= 192, "evidence recovery slots")?;
        let mut evidence_bytes = 0u64;
        for name in artifacts {
            let f = open_at(&evidence, &name, libc::O_RDONLY)?;
            regular(&f)?;
            evidence_bytes = evidence_bytes
                .checked_add(io(f.metadata())?.len())
                .ok_or(Error("evidence total overflow"))?;
        }
        require(
            evidence_bytes <= 48 * 1024 * 1024,
            "evidence recovery capacity",
        )?;
        require(
            bytes >= 1 << 30
                && inodes >= MAX_EVENTS
                && self.state.sequence < MAX_EVENTS / 4
                && entries(&self.reserve)?.len() == RESERVE_SLOTS as usize,
            "acquisition storage reserve/headroom",
        )
    }
    pub(crate) fn retain_evidence(&mut self, bytes: &[u8], expected: &str) -> Result<()> {
        require(
            !self.poisoned && !bytes.is_empty() && bytes.len() <= 1_048_576,
            "evidence bound/authority",
        )?;
        canonical::digest(expected)?;
        require(canonical::sha256(bytes) == expected, "evidence digest")?;
        let dir = directory(&self.root, "evidence")?;
        let names = entries(&dir)?;
        if names.iter().any(|n| n == expected) {
            self.verify_evidence(expected, bytes.len() as u64)?;
            io(dir.sync_all())?;
            return Ok(());
        }
        require(names.len() < 256, "evidence artifact count")?;
        let mut total = bytes.len() as u64;
        for n in names {
            total = total
                .checked_add(io(open_at(&dir, &n, libc::O_RDONLY)?.metadata())?.len())
                .ok_or(Error("evidence total overflow"))?;
        }
        require(total <= 64 * 1024 * 1024, "evidence aggregate bound")?;
        let mut random = [0; 16];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut random)
            .map_err(|_| Error("evidence staging randomness"))?;
        let temporary = format!("evidence-{}.tmp", canonical::sha256(&random));
        let mut f = open_at(
            &self.staging,
            &temporary,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        let result = (|| {
            io(f.write_all(bytes))?;
            io(f.sync_all())?;
            publish_name(&self.staging, &temporary, &dir, expected)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn verify_evidence(&self, digest: &str, length: u64) -> Result<()> {
        canonical::digest(digest)?;
        require(
            length > 0 && length <= 1_048_576,
            "evidence reference bound",
        )?;
        let dir = directory(&self.root, "evidence")?;
        let f = open_at(&dir, digest, libc::O_RDONLY)?;
        regular(&f)?;
        require(
            io(f.metadata())?.len() == length,
            "retained evidence length",
        )?;
        let mut bytes = Vec::new();
        io((&f).take(length + 1).read_to_end(&mut bytes))?;
        require(
            bytes.len() as u64 == length && canonical::sha256(&bytes) == digest,
            "retained evidence bytes",
        )
    }
    fn replay(&mut self) -> Result<()> {
        self.check_objects()?;
        let mut state = AccountState::default();
        for n in entries(&self.journal)? {
            require(
                n == format!("{:020}.json", state.sequence),
                "journal sequence/name",
            )?;
            let f = open_at(&self.journal, &n, libc::O_RDONLY)?;
            regular(&f)?;
            require(
                io(f.metadata())?.len() <= canonical::EVENT_BYTES as u64,
                "event size",
            )?;
            let mut bytes = Vec::new();
            io((&f)
                .take(canonical::EVENT_BYTES as u64 + 1)
                .read_to_end(&mut bytes))?;
            let e: Envelope = canonical::decode(&bytes)?;
            if let Some(evidence) = e.event.evidence() {
                self.verify_evidence(&evidence.sha256, evidence.bytes)?;
            }
            state = state.apply(&e)?;
            same(&f, &open_at(&self.journal, &n, libc::O_RDONLY)?)?;
        }
        // Stabilize entries whose publisher died between rename and directory sync.
        io(self.journal.sync_all())?;
        self.state = state;
        Ok(())
    }
    pub(crate) fn append(&mut self, e: &Envelope) -> Result<EventDigest> {
        require(
            !self.poisoned,
            "journal requires reopen after publication failure",
        )?;
        self.check_objects()?;
        let emergency =
            e.event.publication_class() == crate::publication::PublicationClass::Recovery;
        require(
            self.state.sequence < MAX_EVENTS - if emergency { 0 } else { RESERVE_SLOTS },
            "journal retention limit",
        )?;
        if let Some(evidence) = e.event.evidence() {
            self.verify_evidence(&evidence.sha256, evidence.bytes)?;
        }
        let next = self.state.apply(e)?;
        let bytes = canonical::encode(e)?;
        let (free_bytes, free_inodes) = available(&self.root)?;
        require(
            emergency || (free_bytes >= canonical::EVENT_BYTES as u64 && free_inodes > 0),
            "ordinary publication capacity exhausted",
        )?;
        let consume_reserve = emergency && (free_bytes < 1_048_576 || free_inodes < 128);
        let result = self.publish(&bytes, consume_reserve);
        if result.is_err() {
            self.poisoned = true;
        }
        result?;
        self.state = next;
        canonical::sha256(&bytes).parse()
    }
    fn publish(&self, bytes: &[u8], emergency: bool) -> Result<()> {
        let filename = format!("{:020}.json", self.state.sequence);
        let (source_dir, source, mut f) = if emergency {
            let source = entries(&self.reserve)?
                .into_iter()
                .find(|s| s.ends_with(".slot"))
                .ok_or(Error("recovery reserve exhausted"))?;
            let f = open_at(&self.reserve, &source, libc::O_RDWR)?;
            regular(&f)?;
            // Claim a reserve inode outside the journal before altering it. An
            // interrupted/failed publication leaves a staging artifact, never a
            // reusable journal entry or an apparently intact reserve slot.
            let claimed = format!("reserve-{source}");
            publish_name(&self.reserve, &source, &self.staging, &claimed)?;
            (&self.staging, claimed, f)
        } else {
            let mut entropy = [0u8; 16];
            ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut entropy)
                .map_err(|_| Error("staging randomness"))?;
            let source = format!("{}.tmp", canonical::sha256(&entropy));
            let f = open_at(
                &self.staging,
                &source,
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            )?;
            (&self.staging, source, f)
        };
        io(f.seek(SeekFrom::Start(0)))?;
        checkpoint("write")?;
        io(f.write_all(bytes))?;
        checkpoint("truncate")?;
        io(f.set_len(bytes.len() as u64))?;
        checkpoint("file-sync")?;
        io(f.sync_all())?;
        self.check_objects()?;
        same(&f, &open_at(source_dir, &source, libc::O_RDONLY)?)?;
        checkpoint("rename")?;
        publish_name(source_dir, &source, &self.journal, &filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn genesis() -> Envelope {
        Envelope {
            account_id: "a".parse().unwrap(),
            authority: AUTHORITY.into(),
            authority_id: "controller".parse().unwrap(),
            event: Event::AuthorityInitialized,
            format: FORMAT.into(),
            operation_id: None,
            previous_sha256: None,
            schema_version: 1,
            sequence: 0,
            time: crate::scheduling::TimeSample {
                boot_id: "00000000-0000-0000-0000-000000000001".into(),
                boottime_ns: 1,
                realtime_ns: 1,
                time_namespace: "time:[1]".into(),
            },
            tool: ToolIdentityV1 {
                package: "borrowser-host-lifecycle".into(),
                package_version: "0.1.0".into(),
                schema_version: 1,
                source_revision: "1".repeat(40),
                cargo_lock_sha256: "2".repeat(64),
                source_clean: true,
            },
        }
    }
    #[test]
    fn replay_rejects_corrupt_sequence_and_noncanonical_committed_bytes() {
        for failure in ["sequence", "whitespace", "schema", "digest"] {
            let dir = tempfile::tempdir().unwrap();
            let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
            j.append(&genesis()).unwrap();
            drop(j);
            let file = dir.path().join("journal/00000000000000000000.json");
            let original = std::fs::read(&file).unwrap();
            match failure {
                "sequence" => {
                    std::fs::rename(&file, dir.path().join("journal/00000000000000000001.json"))
                        .unwrap()
                }
                "whitespace" => {
                    let mut bytes = original;
                    bytes.push(b' ');
                    std::fs::write(&file, bytes).unwrap();
                }
                "schema" => {
                    let mut event = genesis();
                    event.schema_version = 2;
                    std::fs::write(&file, canonical::encode(&event).unwrap()).unwrap();
                }
                _ => {
                    let mut event = genesis();
                    event.previous_sha256 = Some("f".repeat(64).parse().unwrap());
                    std::fs::write(&file, canonical::encode(&event).unwrap()).unwrap();
                }
            }
            assert!(Journal::open_verified(File::open(dir.path()).unwrap()).is_err());
        }
    }
    #[test]
    fn damaged_reserve_blocks_acquisition_without_touching_journal() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        j.append(&genesis()).unwrap();
        SPACE.with(|s| s.set(Some((u64::MAX, u64::MAX))));
        assert!(j.admit_allocation().is_ok());
        std::fs::write(dir.path().join("reserve/0000.slot"), b"short").unwrap();
        assert!(j.admit_allocation().is_err());
        SPACE.with(|s| s.set(None));
        assert_eq!(j.state.sequence, 1);
    }
    #[test]
    fn failures_never_publish_partial_events_or_reuse_committed_sequence() {
        for point in ["write", "truncate", "file-sync", "rename", "directory-sync"] {
            let dir = tempfile::tempdir().unwrap();
            let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
            FAULT.with(|f| f.set(Some(point)));
            assert!(j.append(&genesis()).is_err());
            assert!(j.append(&genesis()).is_err());
            FAULT.with(|f| f.set(None));
            drop(j);
            let mut recovered = Journal::open_verified(File::open(dir.path()).unwrap()).unwrap();
            if point == "directory-sync" {
                assert_eq!(recovered.state.sequence, 1);
                assert!(recovered.append(&genesis()).is_err());
            } else {
                assert_eq!(recovered.state.sequence, 0);
                recovered.append(&genesis()).unwrap();
            }
            let names = entries(&recovered.journal).unwrap();
            assert_eq!(names, ["00000000000000000000.json"]);
        }
    }
    fn allocated_journal() -> (tempfile::TempDir, Journal) {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        for event in crate::orchestrator::command_tests::allocated_events() {
            j.append(&event).unwrap();
        }
        (dir, j)
    }
    fn next_event(j: &Journal, event: Event) -> Envelope {
        let mut e = genesis();
        e.account_id = j.state.account_id.clone().unwrap();
        e.authority_id = j.state.authority_id.clone().unwrap();
        e.operation_id = Some("op".parse().unwrap());
        e.sequence = j.state.sequence;
        e.previous_sha256 = j.state.head.clone();
        e.event = event;
        e
    }
    fn cancellation_intent(j: &Journal) -> Envelope {
        next_event(
            j,
            Event::CancellationAuthorized {
                server_number: 123.try_into().unwrap(),
                authorization: "reviewed cleanup".into(),
            },
        )
    }
    #[test]
    fn low_bytes_or_inodes_use_only_reserve_and_replay_committed_events() {
        for capacity in [(0, u64::MAX), (u64::MAX, 0)] {
            let (dir, mut j) = allocated_journal();
            let before = j.state.sequence;
            SPACE.with(|s| s.set(Some(capacity)));
            assert!(j.admit_allocation().is_err());
            for _ in 0..4 {
                let head = j.state.head.clone();
                let (next, report, reads) = crate::orchestrator::command_tests::watch_journal(j);
                j = next;
                assert!(!report.complete_scan);
                assert_eq!(reads, 0);
                assert_eq!(j.state.head, head);
                assert_eq!(entries(&j.reserve).unwrap().len(), 64);
            }
            j.append(&cancellation_intent(&j)).unwrap();
            assert_eq!(entries(&j.reserve).unwrap().len(), 63);
            assert_eq!(j.state.sequence, before + 1);
            SPACE.with(|s| s.set(None));
            drop(j);
            let j = Journal::open_verified(File::open(dir.path()).unwrap()).unwrap();
            assert_eq!(j.state.sequence, before + 1);
            assert_eq!(
                j.state
                    .operation(&"op".parse().unwrap())
                    .unwrap()
                    .server_number,
                Some(123.try_into().unwrap())
            );
        }
    }
    #[test]
    fn watchdog_at_ordinary_retention_boundary_preserves_cleanup_slots_and_reserve() {
        let (dir, mut j) = allocated_journal();
        while j.state.sequence < MAX_EVENTS - RESERVE_SLOTS {
            let mut event = next_event(
                &j,
                Event::WatchProgress {
                    after: "op".parse().unwrap(),
                },
            );
            event.operation_id = None;
            j.append(&event).unwrap();
        }
        for capacity in [(u64::MAX, u64::MAX), (0, u64::MAX), (u64::MAX, 0)] {
            SPACE.with(|s| s.set(Some(capacity)));
            for _ in 0..4 {
                let before = j.state.clone();
                let (next, report, reads) = crate::orchestrator::command_tests::watch_journal(j);
                j = next;
                assert!(!report.complete_scan);
                assert_eq!(
                    report.unresolved,
                    vec!["op".parse::<OperationId>().unwrap()]
                );
                assert_eq!(reads, 0);
                assert_eq!(
                    canonical::encode(&j.state).unwrap(),
                    canonical::encode(&before).unwrap()
                );
                assert_eq!(j.state.sequence, MAX_EVENTS - RESERVE_SLOTS);
                assert_eq!(entries(&j.reserve).unwrap().len(), 64);
            }
        }
        // No free inodes: cancellation can still publish with its protected inode.
        j.append(&cancellation_intent(&j)).unwrap();
        assert_eq!(j.state.sequence, MAX_EVENTS - RESERVE_SLOTS + 1);
        assert_eq!(entries(&j.reserve).unwrap().len(), 63);
        SPACE.with(|s| s.set(None));
        drop(j);
        let j = Journal::open_verified(File::open(dir.path()).unwrap()).unwrap();
        assert_eq!(j.state.sequence, MAX_EVENTS - RESERVE_SLOTS + 1);
    }
    #[test]
    fn interrupted_emergency_write_consumes_reserve_but_not_a_sequence() {
        let (dir, mut j) = allocated_journal();
        let before = j.state.sequence;
        SPACE.with(|s| s.set(Some((0, 0))));
        FAULT.with(|f| f.set(Some("file-sync")));
        assert!(j.append(&cancellation_intent(&j)).is_err());
        assert_eq!(entries(&j.journal).unwrap().len() as u64, before);
        assert_eq!(entries(&j.reserve).unwrap().len(), 63);
        FAULT.with(|f| f.set(None));
        drop(j);
        let mut j = Journal::open_verified(File::open(dir.path()).unwrap()).unwrap();
        j.append(&cancellation_intent(&j)).unwrap();
        assert_eq!(entries(&j.reserve).unwrap().len(), 62);
        assert_eq!(j.state.sequence, before + 1);
        SPACE.with(|s| s.set(None));
    }
    #[test]
    fn reserve_never_appears_in_replay_namespace() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        assert!(entries(&j.journal).unwrap().is_empty());
        assert_eq!(entries(&j.reserve).unwrap().len(), RESERVE_SLOTS as usize);
        assert!(Journal::open_verified(File::open(dir.path()).unwrap()).is_err());
        drop(j);
        assert_eq!(
            Journal::open_verified(File::open(dir.path()).unwrap())
                .unwrap()
                .state
                .sequence,
            0
        );
    }
    #[test]
    fn subprocess_lock_holder() {
        let Ok(path) = std::env::var("LIFECYCLE_TEST_JOURNAL") else {
            return;
        };
        let _j = Journal::open_verified(File::open(path).unwrap()).unwrap();
        println!("LIFECYCLE_LOCK_HELD");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
    #[test]
    fn process_death_releases_kernel_lock_without_deleting_lock_inode() {
        use std::io::BufRead;
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        j.append(&genesis()).unwrap();
        let inode = io(j.lock.metadata()).unwrap().ino();
        drop(j);
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "journal::tests::subprocess_lock_holder",
                    "--nocapture",
                ])
                .env("LIFECYCLE_TEST_JOURNAL", dir.path())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut output = std::io::BufReader::new(child.0.stdout.take().unwrap());
        let mut held = false;
        for _ in 0..8 {
            let mut line = String::new();
            if output.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if line.contains("LIFECYCLE_LOCK_HELD") {
                held = true;
                break;
            }
        }
        assert!(held);
        assert!(Journal::open_verified(File::open(dir.path()).unwrap()).is_err());
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let recovered = Journal::open_verified(File::open(dir.path()).unwrap()).unwrap();
        assert_eq!(recovered.lock.metadata().unwrap().ino(), inode);
        assert_eq!(recovered.state.sequence, 1);
    }
    #[test]
    fn replaced_lock_and_symlinked_journal_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        std::fs::rename(dir.path().join("lock"), dir.path().join("old-lock")).unwrap();
        let replacement = File::create(dir.path().join("lock")).unwrap();
        drop(replacement);
        assert!(j.append(&genesis()).is_err());
        drop(j);
        std::fs::remove_dir(dir.path().join("journal")).unwrap();
        std::os::unix::fs::symlink("staging", dir.path().join("journal")).unwrap();
        assert!(Journal::open_verified(File::open(dir.path()).unwrap()).is_err());
    }
    #[test]
    fn evidence_is_idempotent_and_revalidated_on_replay() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = Journal::bootstrap_verified(File::open(dir.path()).unwrap()).unwrap();
        j.append(&genesis()).unwrap();
        let bytes = b"synthetic provider confirmation";
        let digest = canonical::sha256(bytes);
        j.retain_evidence(bytes, &digest).unwrap();
        j.retain_evidence(bytes, &digest).unwrap();
        let mut e = genesis();
        e.sequence = 1;
        e.previous_sha256 = j.state.head.clone();
        e.event = Event::AuthenticationResolved {
            evidence: Evidence {
                sha256: digest.clone(),
                bytes: bytes.len() as u64,
                provider_reference: "synthetic-reference".into(),
                reviewer: "test".into(),
                rationale: "synthetic credential correction".into(),
            },
        };
        j.append(&e).unwrap();
        drop(j);
        std::fs::write(dir.path().join("evidence").join(digest), b"corrupt").unwrap();
        assert!(Journal::open_verified(File::open(dir.path()).unwrap()).is_err());
    }
}
