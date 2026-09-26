//! Descriptor-rooted immutable journal. reserve/ and staging/ are never replayed.
use crate::{Error, Result, canonical, model::*, require};
use crate::{deployment::AuthorityRootV2, identity::*};
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

fn read_marker(root: &File, expected: &AuthorityRootV2) -> Result<File> {
    let f = open_at(root, "authority.json", libc::O_RDONLY)?;
    regular(&f)?;
    require(io(f.metadata())?.len() <= 16_384, "root marker bound")?;
    let mut bytes = Vec::new();
    io((&f).take(16_385).read_to_end(&mut bytes))?;
    require(bytes.len() <= 16_384, "root marker grew")?;
    let marker: AuthorityRootV2 = canonical::decode(&bytes)?;
    marker.validate()?;
    require(&marker == expected, "root deployment identity mismatch")?;
    same(&f, &open_at(root, "authority.json", libc::O_RDONLY)?)?;
    Ok(f)
}

/// Owned local lock plus retained directories. No path-based I/O after opening.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) struct Journal {
    root: File,
    marker_file: Option<File>,
    marker: AuthorityRootV2,
    lock: File,
    journal: File,
    staging: File,
    reserve: File,
    state: AuthorityStateV2,
    poisoned: bool,
}
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl Journal {
    /// Production root validation must precede this constructor. Kept crate-private;
    /// alternate roots exist only in unit tests.
    pub(crate) fn open_verified(root: File, expected: &AuthorityRootV2) -> Result<Self> {
        expected.validate()?;
        // Positive recognition only; unsupported roots need no historical parser.
        let marker_file = read_marker(&root, expected)?;
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
            marker_file: Some(marker_file),
            marker: expected.clone(),
            root,
            lock,
            journal,
            staging,
            reserve,
            state: AuthorityStateV2::default(),
            poisoned: false,
        };
        this.replay()?;
        io(this.root.sync_all())?;
        Ok(this)
    }
    pub(crate) fn bootstrap_verified(
        root: File,
        marker: &AuthorityRootV2,
        genesis: &EnvelopeV2,
    ) -> Result<Self> {
        marker.validate()?;
        AuthorityStateV2::default().apply(genesis, marker)?;
        // A fresh dedicated ext4 root may have filesystem-created lost+found.
        // Inspect the entry itself: a symlink/file with that name is not exempt.
        let names = entries(&root)?;
        require(
            names.iter().all(|n| n == "lost+found"),
            "authority already initialized or unexpected entries",
        )?;
        if !names.is_empty() {
            let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
            require(
                unsafe {
                    libc::fstatat(
                        root.as_raw_fd(),
                        c"lost+found".as_ptr(),
                        metadata.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0,
                "filesystem-created directory metadata",
            )?;
            let metadata = unsafe { metadata.assume_init() };
            // Darwin dev_t is signed; MetadataExt exposes device identity as u64.
            #[allow(clippy::unnecessary_cast)]
            let same_device = metadata.st_dev as u64 == io(root.metadata())?.dev();
            require(
                metadata.st_mode & libc::S_IFMT == libc::S_IFDIR
                    && metadata.st_uid == 0
                    && metadata.st_mode & 0o077 == 0
                    && same_device,
                "unexpected filesystem-created material",
            )?;
        }
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
        let mut this = Self {
            marker_file: None,
            marker: marker.clone(),
            root,
            lock,
            journal,
            staging,
            reserve,
            state: AuthorityStateV2::default(),
            poisoned: false,
        };
        checkpoint("bootstrap-reserve")?;
        this.append(genesis)?;
        checkpoint("bootstrap-genesis")?;
        // The marker is the final completion record, published only after genesis
        // and every required directory/reserve object have been synchronized.
        let mut f = open_at(
            &this.staging,
            "authority.json",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        io(f.write_all(&canonical::encode(marker)?))?;
        io(f.sync_all())?;
        checkpoint("bootstrap-marker")?;
        publish_name(
            &this.staging,
            "authority.json",
            &this.root,
            "authority.json",
        )?;
        this.marker_file = Some(read_marker(&this.root, marker)?);
        this.check_objects()?;
        Ok(this)
    }
    fn check_objects(&self) -> Result<()> {
        if let Some(retained) = &self.marker_file {
            same(retained, &read_marker(&self.root, &self.marker)?)?;
        }
        directory(&self.root, "evidence")?;
        same(&self.lock, &open_at(&self.root, "lock", libc::O_RDWR)?)?;
        same(&self.journal, &directory(&self.root, "journal")?)?;
        same(&self.staging, &directory(&self.root, "staging")?)?;
        same(&self.reserve, &directory(&self.root, "reserve")?)
    }
    pub(crate) fn append_launch(
        &mut self,
        event: EventV2,
        time: crate::scheduling::TimeSample,
        tool: ToolIdentityV2,
    ) -> Result<EventDigest> {
        let e = EnvelopeV2 {
            account_id: self.marker.identity.account_id.clone(),
            authority: AUTHORITY.into(),
            authority_id: self.marker.identity.authority_id.clone(),
            region: self.marker.identity.region.clone(),
            root_sha256: self.marker.digest()?,
            event,
            format: FORMAT.into(),
            previous_sha256: self.state.head.clone(),
            schema_version: 2,
            sequence: self.state.sequence,
            time,
            tool,
        };
        self.append(&e)
    }
    pub(crate) fn state(&self) -> &AuthorityStateV2 {
        &self.state
    }
    #[allow(dead_code)] // Retained storage admission primitive; no allocation command in Pass 1.
    pub(crate) fn check_storage_headroom(&self) -> Result<()> {
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
    #[allow(dead_code)] // Retained local storage primitive, not evidence publication authority.
    pub(crate) fn retain_evidence(&mut self, bytes: &[u8], expected: &str) -> Result<()> {
        require(
            !self.poisoned && !bytes.is_empty() && bytes.len() <= 1_048_576,
            "evidence bound/authority",
        )?;
        self.check_objects()?;
        canonical::digest(expected)?;
        require(canonical::sha256(bytes) == expected, "evidence digest")?;
        let dir = directory(&self.root, "evidence")?;
        let names = entries(&dir)?;
        if names.iter().any(|n| n == expected) {
            require(
                self.read_evidence(expected, bytes.len() as u64)? == bytes,
                "evidence replacement forbidden",
            )?;
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
        self.read_evidence(digest, length).map(|_| ())
    }
    fn read_evidence(&self, digest: &str, length: u64) -> Result<Vec<u8>> {
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
        )?;
        same(&f, &open_at(&dir, digest, libc::O_RDONLY)?)?;
        Ok(bytes)
    }
    pub(crate) fn validate_launch_preparation(
        &self,
        prepared: &crate::dispatch::PreparedLaunchV2,
        time: &crate::scheduling::TimeSample,
    ) -> Result<()> {
        require(
            !self.poisoned,
            "journal requires reopen after publication failure",
        )?;
        require(
            self.state.operation.is_none(),
            "unresolved acquisition exists",
        )?;
        prepared.validate(
            &self.marker,
            self.state
                .head
                .as_ref()
                .ok_or(Error("authorization requires head"))?,
            time,
        )
    }
    pub(crate) fn retain_launch_artifacts(
        &mut self,
        prepared: &crate::dispatch::PreparedLaunchV2,
    ) -> Result<()> {
        let refs = prepared.references()?;
        // Each artifact retains the established evidence bounds and poison rules.
        macro_rules! retain {
            ($field:ident, $doc:ident) => {
                self.retain_evidence(
                    &canonical::encode(&prepared.$doc)?,
                    refs.$field.sha256.as_str(),
                )?;
            };
        }
        retain!(deployment, deployment);
        checkpoint("launch-artifact-one")?;
        retain!(approval, approval);
        retain!(trust, trust);
        retain!(specification, specification);
        retain!(request, request);
        retain!(authorization, authorization);
        require(
            self.resolve_launch_artifacts(&refs)? == *prepared,
            "retained launch bytes",
        )?;
        checkpoint("launch-artifacts-complete")
    }
    pub(crate) fn verify_current_launch_artifacts(&self) -> Result<()> {
        let op = self
            .state
            .operation
            .as_ref()
            .ok_or(Error("launch preparation required"))?;
        require(
            self.resolve_launch_artifacts(&op.prepared.references()?)? == op.prepared,
            "retained launch artifacts changed",
        )
    }
    fn resolve_launch_artifacts(
        &self,
        refs: &crate::dispatch::LaunchArtifactRefsV2,
    ) -> Result<crate::dispatch::PreparedLaunchV2> {
        macro_rules! read {
            ($field:ident) => {
                canonical::decode(
                    &self.read_evidence(refs.$field.sha256.as_str(), refs.$field.bytes)?,
                )?
            };
        }
        Ok(crate::dispatch::PreparedLaunchV2 {
            deployment: read!(deployment),
            approval: read!(approval),
            trust: read!(trust),
            specification: read!(specification),
            request: read!(request),
            authorization: read!(authorization),
        })
    }
    fn apply_event(&self, state: &AuthorityStateV2, e: &EnvelopeV2) -> Result<AuthorityStateV2> {
        let retained = match &e.event {
            EventV2::LaunchPrepared(p) => Some(self.resolve_launch_artifacts(&p.artifacts)?),
            _ => None,
        };
        state.apply_retained(e, &self.marker, retained.as_ref())
    }
    fn replay(&mut self) -> Result<()> {
        self.check_objects()?;
        let mut state = AuthorityStateV2::default();
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
            let e: EnvelopeV2 = canonical::decode(&bytes)?;
            if let Some((digest, bytes)) = e.event.evidence() {
                self.verify_evidence(digest, bytes)?;
            }
            state = self.apply_event(&state, &e)?;
            same(&f, &open_at(&self.journal, &n, libc::O_RDONLY)?)?;
        }
        require(state.sequence > 0, "missing authority genesis")?;
        self.check_objects()?;
        // Stabilize entries whose publisher died between rename and directory sync.
        io(self.journal.sync_all())?;
        self.state = state;
        Ok(())
    }
    pub(crate) fn append(&mut self, e: &EnvelopeV2) -> Result<EventDigest> {
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
        if let Some((digest, bytes)) = e.event.evidence() {
            self.verify_evidence(digest, bytes)?;
        }
        let next = self.apply_event(&self.state, e)?;
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
pub(crate) mod tests {
    use super::*;
    // Process creation can transiently inherit another test thread's flock file
    // description before exec closes it. Serialize storage fixtures with the
    // subprocess-lock test; production still rejects genuine contention.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    use crate::test_support::{genesis, marker};
    fn fresh() -> (tempfile::TempDir, Journal) {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
            .unwrap();
        (dir, j)
    }
    pub(crate) fn with_launch_attempt(
        f: impl FnOnce(
            &crate::dispatch::DurableLaunchAttempt,
            &crate::dispatch::PreparedLaunchV2,
            &crate::scheduling::TimeSample,
        ),
    ) {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (_dir, mut j) = fresh();
        launch_prepare(&mut j).unwrap();
        j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
        let attempt = j.begin_attempt(launch_time(0), genesis().tool).unwrap();
        f(
            &attempt,
            &crate::test_support::launch_documents(),
            &launch_time(0),
        );
    }
    fn reopen(dir: &std::path::Path) -> Result<Journal> {
        Journal::open_verified(File::open(dir).unwrap(), &marker())
    }
    fn next(j: &Journal, event: EventV2) -> EnvelopeV2 {
        let mut e = genesis();
        e.sequence = j.state.sequence;
        e.previous_sha256 = j.state.head.clone();
        e.event = event;
        e
    }
    fn launch_vector(name: &str) -> EnvelopeV2 {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dispatch-v2")
            .join(format!("{name}.json"));
        canonical::decode(&std::fs::read(path).unwrap()).unwrap()
    }
    fn launch_time(seconds: u64) -> crate::scheduling::TimeSample {
        let mut t = launch_vector("prepared").time;
        t.boottime_ns += seconds * 1_000_000_000;
        t
    }
    fn launch_prepare(j: &mut Journal) -> Result<()> {
        let e = launch_vector("prepared");
        j.prepare_launch(crate::test_support::launch_documents(), e.time, e.tool)
    }
    #[test]
    fn durable_launch_capabilities_match_independent_receipts_and_restart_state() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        use crate::dispatch::*;
        let (dir, mut j) = fresh();
        launch_prepare(&mut j).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("journal/00000000000000000001.json")).unwrap(),
            canonical::encode(&launch_vector("prepared")).unwrap()
        );
        let prepared = j.state.clone();
        drop(j);
        let mut j = reopen(dir.path()).unwrap();
        assert_eq!(j.state, prepared);
        let dispatch = j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
        assert_eq!(
            dispatch.identity(),
            j.state
                .operation
                .as_ref()
                .unwrap()
                .dispatch
                .as_ref()
                .unwrap()
        );
        let dispatched = j.state.clone();
        drop(j);
        let mut j = reopen(dir.path()).unwrap();
        assert_eq!(j.state, dispatched);
        let a = j.begin_attempt(launch_time(0), genesis().tool).unwrap();
        assert_eq!(a.identity().receipt.head, j.state.head.clone().unwrap());
        assert_eq!(
            std::fs::read(dir.path().join("journal/00000000000000000003.json")).unwrap(),
            canonical::encode(&launch_vector("attempt-intent")).unwrap()
        );
        j.finish_attempt(
            a,
            LaunchAttemptOutcome::DefinitelyNotTransmitted,
            launch_time(0),
            genesis().tool,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join("journal/00000000000000000004.json")).unwrap(),
            canonical::encode(&launch_vector("definitely-not-transmitted")).unwrap()
        );
        for (early, due) in [(1, 2), (9, 10)] {
            let retained = j.state.clone();
            drop(j);
            j = reopen(dir.path()).unwrap();
            assert_eq!(j.state, retained);
            assert!(j.begin_attempt(launch_time(early), genesis().tool).is_err());
            let a = j.begin_attempt(launch_time(due), genesis().tool).unwrap();
            j.finish_attempt(
                a,
                LaunchAttemptOutcome::DefinitelyNotTransmitted,
                launch_time(due),
                genesis().tool,
            )
            .unwrap();
        }
        let retained = j.state.clone();
        drop(j);
        let mut j = reopen(dir.path()).unwrap();
        assert_eq!(j.state, retained);
        assert!(j.begin_attempt(launch_time(119), genesis().tool).is_err());
        assert_eq!(j.state.operation.as_ref().unwrap().attempts.len(), 3);
    }
    #[test]
    fn replay_never_reconstructs_pending_attempt_permission() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        launch_prepare(&mut j).unwrap();
        j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
        let _lost = j.begin_attempt(launch_time(0), genesis().tool).unwrap();
        let pending = j.state.clone();
        drop(j);
        for _ in 0..3 {
            let mut j = reopen(dir.path()).unwrap();
            assert_eq!(j.state, pending);
            assert!(j.begin_attempt(launch_time(10), genesis().tool).is_err());
            assert!(launch_prepare(&mut j).is_err());
            assert!(j.prepare_dispatch(launch_time(10), genesis().tool).is_err());
        }
    }
    #[test]
    fn launch_publication_faults_never_grant_capabilities_and_poison_until_replay() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for stage in 0..4 {
            for point in ["write", "truncate", "file-sync", "rename", "directory-sync"] {
                let (dir, mut j) = fresh();
                if stage > 0 {
                    launch_prepare(&mut j).unwrap();
                }
                if stage > 1 {
                    j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
                }
                let attempt = if stage > 2 {
                    Some(j.begin_attempt(launch_time(0), genesis().tool).unwrap())
                } else {
                    None
                };
                if stage == 0 {
                    j.retain_launch_artifacts(&crate::test_support::launch_documents())
                        .unwrap();
                }
                let before = j.state.clone();
                FAULT.with(|f| f.set(Some(point)));
                let failed = match stage {
                    0 => launch_prepare(&mut j),
                    1 => j
                        .prepare_dispatch(launch_time(0), genesis().tool)
                        .map(|_| ()),
                    2 => j.begin_attempt(launch_time(0), genesis().tool).map(|_| ()),
                    _ => j.finish_attempt(
                        attempt.unwrap(),
                        crate::dispatch::LaunchAttemptOutcome::DefinitelyNotTransmitted,
                        launch_time(0),
                        genesis().tool,
                    ),
                };
                assert!(failed.is_err());
                assert!(j.poisoned);
                assert_eq!(j.state, before);
                FAULT.with(|f| f.set(None));
                assert!(j.begin_attempt(launch_time(10), genesis().tool).is_err());
                drop(j);
                let mut recovered = reopen(dir.path()).unwrap();
                assert_eq!(
                    recovered.state.sequence,
                    before.sequence + u64::from(point == "directory-sync")
                );
                if stage == 2 && point == "directory-sync"
                    || stage == 3 && point != "directory-sync"
                {
                    assert!(
                        recovered
                            .begin_attempt(launch_time(10), genesis().tool)
                            .is_err()
                    );
                }
                if stage == 3 && point == "directory-sync" {
                    assert!(
                        recovered
                            .begin_attempt(launch_time(1), genesis().tool)
                            .is_err()
                    );
                    assert!(
                        recovered
                            .begin_attempt(launch_time(2), genesis().tool)
                            .is_ok()
                    );
                }
            }
        }
    }
    #[test]
    fn launch_low_space_is_ordinary_and_consumed_attempt_outcome_is_recovery() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for stage in 0..3 {
            let (_dir, mut j) = fresh();
            if stage > 0 {
                launch_prepare(&mut j).unwrap();
            }
            if stage > 1 {
                j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
            }
            if stage == 0 {
                j.retain_launch_artifacts(&crate::test_support::launch_documents())
                    .unwrap();
            }
            let event = launch_vector(["prepared", "dispatch-intent", "attempt-intent"][stage]);
            let before = j.state.clone();
            SPACE.with(|s| s.set(Some((0, 0))));
            assert!(j.append(&event).is_err());
            assert_eq!(j.state, before);
            assert_eq!(entries(&j.reserve).unwrap().len(), 64);
            SPACE.with(|s| s.set(None));
        }
        for name in [
            "definitely-not-transmitted",
            "transmission-uncertain",
            "parameter-conflict",
            "access-blocked",
            "throttled-held",
            "response-unresolved",
        ] {
            let (dir, mut j) = fresh();
            launch_prepare(&mut j).unwrap();
            j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
            let _attempt = j.begin_attempt(launch_time(0), genesis().tool).unwrap();
            SPACE.with(|s| s.set(Some((0, 0))));
            j.append(&launch_vector(name)).unwrap();
            assert_eq!(entries(&j.reserve).unwrap().len(), 63);
            assert!(j.begin_attempt(launch_time(10), genesis().tool).is_err());
            SPACE.with(|s| s.set(None));
            drop(j);
            assert_eq!(reopen(dir.path()).unwrap().state.sequence, 5);
        }
    }
    #[test]
    fn final_slots_cannot_fund_another_launch_attempt() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (_dir, mut j) = fresh();
        launch_prepare(&mut j).unwrap();
        j.prepare_dispatch(launch_time(0), genesis().tool).unwrap();
        let attempt = j.begin_attempt(launch_time(0), genesis().tool).unwrap();
        while j.state.sequence < MAX_EVENTS - RESERVE_SLOTS {
            j.append(&next(&j, EventV2::StorageCheckpoint)).unwrap();
        }
        j.finish_attempt(
            attempt,
            crate::dispatch::LaunchAttemptOutcome::DefinitelyNotTransmitted,
            launch_time(0),
            genesis().tool,
        )
        .unwrap();
        let intent = j
            .state
            .operation
            .as_ref()
            .unwrap()
            .next_attempt(&launch_time(2))
            .unwrap();
        let mut event = next(&j, EventV2::LaunchAttemptIntent(intent));
        event.time = launch_time(2);
        assert!(j.append(&event).is_err());
        assert_eq!(j.state.operation.as_ref().unwrap().attempts.len(), 1);
        assert_eq!(entries(&j.reserve).unwrap().len(), 64);
    }
    #[test]
    fn partial_launch_retention_never_establishes_or_adopts_authority() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for (point, count) in [("launch-artifact-one", 1), ("launch-artifacts-complete", 6)] {
            let (dir, mut j) = fresh();
            FAULT.with(|f| f.set(Some(point)));
            assert!(launch_prepare(&mut j).is_err());
            FAULT.with(|f| f.set(None));
            assert_eq!(j.state.sequence, 1);
            assert!(j.state.operation.is_none());
            let files = entries(&directory(&j.root, "evidence").unwrap()).unwrap();
            assert_eq!(files.len(), count);
            drop(j);
            for _ in 0..2 {
                let mut j = reopen(dir.path()).unwrap();
                assert!(j.state.operation.is_none());
                assert_eq!(j.state.sequence, 1);
                assert!(j.prepare_dispatch(launch_time(0), genesis().tool).is_err());
                assert!(j.begin_attempt(launch_time(0), genesis().tool).is_err());
                assert_eq!(
                    entries(&directory(&j.root, "evidence").unwrap()).unwrap(),
                    files
                );
            }
            // Explicit preparation may reuse identical bytes, never status/replay.
            let mut j = reopen(dir.path()).unwrap();
            launch_prepare(&mut j).unwrap();
            j.retain_evidence(
                b"unreferenced extra artifact",
                &canonical::sha256(b"unreferenced extra artifact"),
            )
            .unwrap();
            drop(j);
            let mut j = reopen(dir.path()).unwrap();
            assert!(launch_prepare(&mut j).is_err());
            assert_eq!(j.state.sequence, 2);
        }
    }
    #[test]
    fn every_launch_artifact_is_exact_before_publication_and_replay() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let p = crate::test_support::launch_documents();
        let r = p.references().unwrap();
        let artifacts = [
            (
                canonical::encode(&p.deployment).unwrap(),
                r.deployment.sha256.as_str(),
            ),
            (
                canonical::encode(&p.approval).unwrap(),
                r.approval.sha256.as_str(),
            ),
            (
                canonical::encode(&p.trust).unwrap(),
                r.trust.sha256.as_str(),
            ),
            (
                canonical::encode(&p.specification).unwrap(),
                r.specification.sha256.as_str(),
            ),
            (
                canonical::encode(&p.request).unwrap(),
                r.request.sha256.as_str(),
            ),
            (
                canonical::encode(&p.authorization).unwrap(),
                r.authorization.sha256.as_str(),
            ),
        ];
        for (bytes, digest) in artifacts {
            for missing in [true, false] {
                let (dir, mut j) = fresh();
                launch_prepare(&mut j).unwrap();
                let path = dir.path().join("evidence").join(digest);
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
                if missing {
                    std::fs::remove_file(&path).unwrap();
                } else {
                    let mut corrupt = bytes.clone();
                    corrupt[0] = b' ';
                    std::fs::write(&path, corrupt).unwrap();
                }
                assert!(j.prepare_dispatch(launch_time(0), genesis().tool).is_err());
                drop(j);
                assert!(reopen(dir.path()).is_err());
                assert!(!missing || !path.exists());
            }
        }
    }
    #[test]
    fn retained_reference_lengths_types_canonical_bytes_and_cross_bindings_reject() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for failure in [
            "digest",
            "length",
            "missing",
            "type",
            "noncanonical",
            "inconsistent",
        ] {
            let (dir, mut j) = fresh();
            let p = crate::test_support::launch_documents();
            j.retain_launch_artifacts(&p).unwrap();
            let mut e = launch_vector("prepared");
            let EventV2::LaunchPrepared(ref mut prepared) = e.event else {
                panic!()
            };
            match failure {
                "digest" => prepared.artifacts.request.sha256 = "f".repeat(64).parse().unwrap(),
                "length" => prepared.artifacts.request.bytes += 1,
                "missing" => std::fs::remove_file(
                    dir.path()
                        .join("evidence")
                        .join(prepared.artifacts.request.sha256.as_str()),
                )
                .unwrap(),
                "type" => {
                    prepared.artifacts.request.sha256 = prepared
                        .artifacts
                        .deployment
                        .sha256
                        .as_str()
                        .parse()
                        .unwrap();
                    prepared.artifacts.request.bytes = prepared.artifacts.deployment.bytes;
                }
                _ => {
                    let mut request = p.request.clone();
                    if failure == "inconsistent" {
                        request.spec.launch.instance_type = "different.large".parse().unwrap();
                    }
                    let mut bytes = canonical::encode(&request).unwrap();
                    if failure == "noncanonical" {
                        bytes.push(b' ');
                    }
                    let digest = canonical::sha256(&bytes);
                    j.retain_evidence(&bytes, &digest).unwrap();
                    prepared.artifacts.request.sha256 = digest.parse().unwrap();
                    prepared.artifacts.request.bytes = bytes.len() as u64;
                }
            }
            assert!(j.append(&e).is_err(), "{failure}");
            assert_eq!(j.state.sequence, 1);
            // Even a forged canonical journal record with a fresh event hash cannot
            // make malformed/inconsistent retained bytes become preparation authority.
            let path = dir.path().join("journal/00000000000000000001.json");
            std::fs::write(&path, canonical::encode(&e).unwrap()).unwrap();
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                .unwrap();
            drop(j);
            assert!(reopen(dir.path()).is_err(), "{failure}");
        }
    }
    #[test]
    fn artifact_store_refuses_replacement_and_poisons_failed_retention() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        let p = crate::test_support::launch_documents();
        let bytes = canonical::encode(&p.deployment).unwrap();
        let digest = canonical::sha256(&bytes);
        assert!(j.retain_evidence(&bytes, &"f".repeat(64)).is_err());
        j.retain_evidence(&bytes, &digest).unwrap();
        let path = dir.path().join("evidence").join(&digest);
        let inode = std::fs::metadata(&path).unwrap().ino();
        j.retain_evidence(&bytes, &digest).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        std::fs::write(&path, b"wrong").unwrap();
        assert!(j.retain_evidence(&bytes, &digest).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"wrong");
        drop(j);
        let (dir, mut j) = fresh();
        FAULT.with(|f| f.set(Some("directory-sync")));
        assert!(launch_prepare(&mut j).is_err());
        FAULT.with(|f| f.set(None));
        assert!(j.poisoned);
        assert!(launch_prepare(&mut j).is_err());
        drop(j);
        let j = reopen(dir.path()).unwrap();
        assert!(j.state.operation.is_none());
    }
    #[test]
    fn retained_launch_documents_are_revalidated_on_replay() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        launch_prepare(&mut j).unwrap();
        let refs = crate::test_support::launch_documents()
            .references()
            .unwrap();
        assert_eq!(
            entries(&directory(&j.root, "evidence").unwrap())
                .unwrap()
                .len(),
            6
        );
        drop(j);
        let path = dir
            .path()
            .join("evidence")
            .join(refs.request.sha256.as_str());
        std::fs::write(path, b"corrupt").unwrap();
        assert!(reopen(dir.path()).is_err());
    }
    #[test]
    fn fresh_v2_bootstrap_reopen_and_restart_are_deterministic() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, j) = fresh();
        let state = j.state.clone();
        assert_eq!(state.sequence, 1);
        assert_eq!(
            std::fs::read(dir.path().join("authority.json")).unwrap(),
            canonical::encode(&marker()).unwrap()
        );
        assert_eq!(entries(&j.reserve).unwrap().len(), 64);
        assert_eq!(entries(&j.journal).unwrap(), ["00000000000000000000.json"]);
        assert!(reopen(dir.path()).is_err());
        drop(j);
        for _ in 0..3 {
            assert_eq!(reopen(dir.path()).unwrap().state, state);
        }
        assert!(
            Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
                .is_err()
        );
    }
    #[test]
    fn deployment_identity_must_match_every_marker_field() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, j) = fresh();
        drop(j);
        for field in ["authority", "account", "region", "machine", "filesystem"] {
            let mut m = marker();
            match field {
                "authority" => m.identity.authority_id = "other".parse().unwrap(),
                "account" => m.identity.account_id = "222222222222".parse().unwrap(),
                "region" => m.identity.region = "eu-west-1".parse().unwrap(),
                "machine" => m.identity.controller_machine_id = "2".repeat(32),
                _ => m.identity.filesystem_uuid = "00000000-0000-0000-0000-000000000002".into(),
            }
            assert!(
                Journal::open_verified(File::open(dir.path()).unwrap(), &m).is_err(),
                "{field}"
            );
        }
        assert!(reopen(dir.path()).is_ok());
    }
    #[test]
    fn unsupported_marker_generation_and_noncanonical_bytes_reject_without_changes() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for field in ["authority", "format", "schema", "whitespace"] {
            let (dir, j) = fresh();
            drop(j);
            let mut m = marker();
            match field {
                "authority" => m.authority = "unknown".into(),
                "format" => m.format = "unknown".into(),
                "schema" => m.schema_version = 3,
                _ => (),
            }
            let mut bytes = canonical::encode(&m).unwrap();
            if field == "whitespace" {
                bytes.push(b' ');
            }
            let file = dir.path().join("authority.json");
            std::fs::write(&file, &bytes).unwrap();
            assert!(reopen(dir.path()).is_err());
            assert_eq!(std::fs::read(file).unwrap(), bytes);
        }
    }
    #[test]
    fn historical_or_forged_marker_journals_reject_without_old_event_model() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for case in [
            "missing-marker",
            "v1",
            "future",
            "authority",
            "format",
            "tool",
            "dirty",
            "root",
            "account",
            "region",
            "authority-id",
        ] {
            let (dir, j) = fresh();
            drop(j);
            let mut e = genesis();
            match case {
                "missing-marker" => {
                    std::fs::remove_file(dir.path().join("authority.json")).unwrap()
                }
                "v1" => {
                    e.schema_version = 1;
                    e.format = "borrowser-host-lifecycle-event".into();
                    e.authority = "provider-lifecycle-only".into();
                }
                "future" => e.schema_version = 3,
                "authority" => e.authority = "unknown".into(),
                "format" => e.format = "unknown".into(),
                "tool" => e.tool.schema_version = 1,
                "dirty" => e.tool.source_clean = false,
                "root" => e.root_sha256 = "f".repeat(64).parse().unwrap(),
                "account" => e.account_id = "222222222222".parse().unwrap(),
                "region" => e.region = "eu-west-1".parse().unwrap(),
                _ => e.authority_id = "other".parse().unwrap(),
            }
            let file = dir.path().join("journal/00000000000000000000.json");
            let bytes = canonical::encode(&e).unwrap();
            std::fs::write(&file, &bytes).unwrap();
            assert!(reopen(dir.path()).is_err(), "{case}");
            assert!(
                Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
                    .is_err()
            );
            assert_eq!(std::fs::read(file).unwrap(), bytes);
        }
    }
    #[test]
    fn historical_genesis_shape_is_not_a_v2_event() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, j) = fresh();
        drop(j);
        // Deliberately construct only obsolete envelope syntax, with no old event types.
        let mut value = serde_json::to_value(genesis()).unwrap();
        let fields = value.as_object_mut().unwrap();
        fields.remove("root_sha256");
        fields.remove("region");
        fields.insert("operation_id".into(), serde_json::Value::Null);
        fields.insert("schema_version".into(), 1.into());
        fields.insert("format".into(), "borrowser-host-lifecycle-event".into());
        fields.insert("authority".into(), "provider-lifecycle-only".into());
        fields.get_mut("tool").unwrap()["schema_version"] = 1.into();
        let bytes = canonical::encode(&value).unwrap();
        let file = dir.path().join("journal/00000000000000000000.json");
        std::fs::write(&file, &bytes).unwrap();
        assert!(reopen(dir.path()).is_err());
        assert_eq!(std::fs::read(file).unwrap(), bytes);
    }
    #[test]
    fn marker_symlink_hardlink_and_in_place_modification_reject() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for kind in ["symlink", "hardlink", "in-place"] {
            let (dir, mut j) = fresh();
            let path = dir.path().join("authority.json");
            if kind == "in-place" {
                let mut root = marker();
                root.identity.region = "eu-west-1".parse().unwrap();
                std::fs::write(&path, canonical::encode(&root).unwrap()).unwrap();
            } else {
                std::fs::rename(&path, dir.path().join("other-marker")).unwrap();
                if kind == "symlink" {
                    std::os::unix::fs::symlink("other-marker", &path).unwrap();
                } else {
                    std::fs::hard_link(dir.path().join("other-marker"), &path).unwrap();
                }
            }
            assert!(j.append(&next(&j, EventV2::StorageCheckpoint)).is_err());
            drop(j);
            assert!(reopen(dir.path()).is_err());
        }
    }
    #[test]
    fn bootstrap_rejects_spoofed_filesystem_material_before_writing() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for symlink in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            if symlink {
                std::os::unix::fs::symlink("/", dir.path().join("lost+found")).unwrap();
            } else {
                std::fs::write(dir.path().join("lost+found"), b"not a directory").unwrap();
            }
            assert!(
                Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
                    .is_err()
            );
            assert_eq!(
                entries(&File::open(dir.path()).unwrap()).unwrap(),
                ["lost+found"]
            );
        }
    }
    #[test]
    fn partial_bootstrap_cannot_be_opened_or_repaired() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for point in [
            "bootstrap-reserve",
            "write",
            "truncate",
            "file-sync",
            "rename",
            "directory-sync",
            "bootstrap-genesis",
            "bootstrap-marker",
        ] {
            let dir = tempfile::tempdir().unwrap();
            FAULT.with(|f| f.set(Some(point)));
            assert!(
                Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
                    .is_err()
            );
            FAULT.with(|f| f.set(None));
            let names = entries(&File::open(dir.path()).unwrap()).unwrap();
            assert!(reopen(dir.path()).is_err(), "{point}");
            assert!(
                Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
                    .is_err()
            );
            assert_eq!(entries(&File::open(dir.path()).unwrap()).unwrap(), names);
        }
        let (dir, j) = fresh();
        drop(j);
        std::fs::remove_file(dir.path().join("journal/00000000000000000000.json")).unwrap();
        assert!(reopen(dir.path()).is_err());
    }
    #[test]
    fn replay_rejects_corrupt_sequence_noncanonical_bytes_and_hash_chain() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for failure in ["sequence", "whitespace", "digest", "previous-event"] {
            let (dir, mut j) = fresh();
            j.append(&next(&j, EventV2::StorageCheckpoint)).unwrap();
            drop(j);
            let file = dir.path().join("journal/00000000000000000001.json");
            match failure {
                "sequence" => {
                    std::fs::rename(&file, dir.path().join("journal/00000000000000000002.json"))
                        .unwrap()
                }
                "whitespace" => {
                    let mut bytes = std::fs::read(&file).unwrap();
                    bytes.push(b' ');
                    std::fs::write(&file, bytes).unwrap();
                }
                "digest" => {
                    let mut e: EnvelopeV2 =
                        canonical::decode(&std::fs::read(&file).unwrap()).unwrap();
                    e.previous_sha256 = Some("f".repeat(64).parse().unwrap());
                    std::fs::write(&file, canonical::encode(&e).unwrap()).unwrap();
                }
                _ => {
                    let mut e = genesis();
                    e.time.realtime_ns = 2;
                    std::fs::write(
                        dir.path().join("journal/00000000000000000000.json"),
                        canonical::encode(&e).unwrap(),
                    )
                    .unwrap();
                }
            }
            assert!(reopen(dir.path()).is_err());
        }
    }
    #[test]
    fn failures_never_publish_partial_events_or_reuse_committed_sequence() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for point in ["write", "truncate", "file-sync", "rename", "directory-sync"] {
            let (dir, mut j) = fresh();
            let e = next(&j, EventV2::StorageCheckpoint);
            FAULT.with(|f| f.set(Some(point)));
            assert!(j.append(&e).is_err());
            assert!(j.append(&e).is_err());
            FAULT.with(|f| f.set(None));
            drop(j);
            let mut recovered = reopen(dir.path()).unwrap();
            if point == "directory-sync" {
                assert_eq!(recovered.state.sequence, 2);
                assert!(recovered.append(&e).is_err());
            } else {
                assert_eq!(recovered.state.sequence, 1);
                recovered.append(&e).unwrap();
            }
            assert_eq!(
                entries(&recovered.journal).unwrap(),
                ["00000000000000000000.json", "00000000000000000001.json"]
            );
        }
    }
    #[test]
    fn damaged_reserve_blocks_headroom_without_touching_journal() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, j) = fresh();
        SPACE.with(|s| s.set(Some((u64::MAX, u64::MAX))));
        assert!(j.check_storage_headroom().is_ok());
        std::fs::write(dir.path().join("reserve/0000.slot"), b"short").unwrap();
        assert!(j.check_storage_headroom().is_err());
        SPACE.with(|s| s.set(None));
        assert_eq!(j.state.sequence, 1);
    }
    #[test]
    fn low_bytes_or_inodes_protect_reserve_from_ordinary_publication() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for capacity in [(0, u64::MAX), (u64::MAX, 0)] {
            let (dir, mut j) = fresh();
            SPACE.with(|s| s.set(Some(capacity)));
            assert!(j.check_storage_headroom().is_err());
            for _ in 0..4 {
                let before = j.state.clone();
                assert!(j.append(&next(&j, EventV2::StorageCheckpoint)).is_err());
                assert_eq!(j.state, before);
                assert_eq!(entries(&j.reserve).unwrap().len(), 64);
            }
            j.append(&next(&j, EventV2::StorageRecovery)).unwrap();
            assert_eq!(entries(&j.reserve).unwrap().len(), 63);
            SPACE.with(|s| s.set(None));
            drop(j);
            assert_eq!(reopen(dir.path()).unwrap().state.sequence, 2);
        }
    }
    #[test]
    fn retention_boundary_preserves_final_recovery_slots() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        while j.state.sequence < MAX_EVENTS - RESERVE_SLOTS {
            j.append(&next(&j, EventV2::StorageCheckpoint)).unwrap();
        }
        for capacity in [(u64::MAX, u64::MAX), (0, u64::MAX), (u64::MAX, 0)] {
            SPACE.with(|s| s.set(Some(capacity)));
            let before = j.state.clone();
            assert!(j.append(&next(&j, EventV2::StorageCheckpoint)).is_err());
            assert_eq!(j.state, before);
            assert_eq!(entries(&j.reserve).unwrap().len(), 64);
        }
        j.append(&next(&j, EventV2::StorageRecovery)).unwrap();
        assert_eq!(entries(&j.reserve).unwrap().len(), 63);
        SPACE.with(|s| s.set(None));
        drop(j);
        assert_eq!(
            reopen(dir.path()).unwrap().state.sequence,
            MAX_EVENTS - RESERVE_SLOTS + 1
        );
    }
    #[test]
    fn interrupted_emergency_write_consumes_reserve_not_sequence() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        SPACE.with(|s| s.set(Some((0, 0))));
        FAULT.with(|f| f.set(Some("file-sync")));
        assert!(j.append(&next(&j, EventV2::StorageRecovery)).is_err());
        assert_eq!(entries(&j.journal).unwrap().len(), 1);
        assert_eq!(entries(&j.reserve).unwrap().len(), 63);
        FAULT.with(|f| f.set(None));
        drop(j);
        let mut j = reopen(dir.path()).unwrap();
        j.append(&next(&j, EventV2::StorageRecovery)).unwrap();
        assert_eq!(entries(&j.reserve).unwrap().len(), 62);
        assert_eq!(j.state.sequence, 2);
        SPACE.with(|s| s.set(None));
    }
    #[test]
    fn replaced_marker_lock_and_symlinked_journal_reject() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        for name in ["authority.json", "lock"] {
            let (dir, mut j) = fresh();
            std::fs::rename(
                dir.path().join(name),
                dir.path().join(format!("old-{name}")),
            )
            .unwrap();
            std::fs::copy(
                dir.path().join(format!("old-{name}")),
                dir.path().join(name),
            )
            .unwrap();
            assert!(j.append(&next(&j, EventV2::StorageCheckpoint)).is_err());
        }
        let (dir, j) = fresh();
        drop(j);
        std::fs::rename(dir.path().join("journal"), dir.path().join("old-journal")).unwrap();
        std::os::unix::fs::symlink("old-journal", dir.path().join("journal")).unwrap();
        assert!(reopen(dir.path()).is_err());
    }
    #[test]
    fn evidence_is_idempotent_and_revalidated_on_replay() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, mut j) = fresh();
        let bytes = b"synthetic storage evidence";
        let digest = canonical::sha256(bytes);
        j.retain_evidence(bytes, &digest).unwrap();
        j.retain_evidence(bytes, &digest).unwrap();
        j.append(&next(
            &j,
            EventV2::StorageEvidence {
                sha256: digest.clone(),
                bytes: bytes.len() as u64,
            },
        ))
        .unwrap();
        drop(j);
        assert!(reopen(dir.path()).is_ok());
        std::fs::write(dir.path().join("evidence").join(digest), b"corrupt").unwrap();
        assert!(reopen(dir.path()).is_err());
    }
    #[test]
    fn subprocess_lock_holder() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let Ok(path) = std::env::var("LIFECYCLE_TEST_JOURNAL") else {
            return;
        };
        let _j = Journal::open_verified(File::open(path).unwrap(), &marker()).unwrap();
        println!("LIFECYCLE_LOCK_HELD");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
    #[test]
    fn process_death_releases_kernel_lock_without_deleting_lock_inode() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        use std::io::BufRead;
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::bootstrap_verified(File::open(dir.path()).unwrap(), &marker(), &genesis())
            .unwrap();
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
        assert!(Journal::open_verified(File::open(dir.path()).unwrap(), &marker()).is_err());
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let recovered = Journal::open_verified(File::open(dir.path()).unwrap(), &marker()).unwrap();
        assert_eq!(recovered.lock.metadata().unwrap().ino(), inode);
        assert_eq!(recovered.state.sequence, 1);
    }
}
