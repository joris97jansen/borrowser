use crate::{
    Error, Result,
    canonical::{self, Writer},
    error::require,
};
use serde::Deserialize;
use std::{path::Path, process::Command};
pub const REVISION: &str = "04d22c360c34c891a66400f31afcf3c4a4146973";
pub const COLLECTOR_PATH: &str = "tools/conformance/static-dom-capture-source-manifest-v1.toml";
pub const QUALIFICATION_PATH: &str =
    "tools/conformance/static-dom-qualification-source-manifest-v1.toml";
pub const COLLECTOR_HASH: &str = "0eebd26f1beaeca68c0b9830b25ca70388bca2ff4aa512fa497fd6fd04b2c732";
pub const QUALIFICATION_HASH: &str =
    "eaea0ed1353cae955aa54fe3e9317ac7477bc225dce564211b0b3228d8c6c2c3";
pub const INSPECTOR_PATH: &str = "tools/conformance/web-observable-dom-tree-v1.mjs";
pub const PACKAGING_PATH: &str = "crates/external_browser_capture/src/packaging.rs";
pub const INSPECTOR_HASH: &str = "e3fc2de4fae55c4d14e8eae0520362a2d29b0789174858d3f8837c7c4cd5e209";
pub const PACKAGING_HASH: &str = "03fb42cb7b6708adc9e40d142b33a8be046c27bd16417ad1316160c810473100";
pub const EXPRESSION_HASH: &str =
    "316a83bad2374261833aa9890399f8fae6417742b0350406f452c52cd3936f0b";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    #[serde(deserialize_with = "entries")]
    entries: Vec<Source>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    path: String,
    sha256: String,
}
// Closed source-verification vocabulary; callers cannot supply arbitrary Git arguments.
#[derive(Clone, Copy)]
enum GitQuery<'a> {
    Version,
    Filters,
    Head,
    IndexClean,
    Status(&'a FilterDrivers),
}
const FILTER_QUERY: &str = r"^filter\..*\.(clean|smudge|process|required)$";
const FILTER_KEYS: usize = 1024;
const FILTER_DRIVERS: usize = 128;
const FILTER_DRIVER_BYTES: usize = 128;
const FILTER_KEY_BYTES: usize = 144;
#[derive(Debug, PartialEq, Eq)]
struct FilterDrivers(Vec<String>);
impl FilterDrivers {
    fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= 65536, "filter query bytes")?;
        if bytes.is_empty() {
            return Ok(Self(Vec::new()));
        }
        require(bytes.last() == Some(&0), "filter key framing")?;
        let mut drivers = Vec::new();
        for (index, raw) in bytes[..bytes.len() - 1].split(|b| *b == 0).enumerate() {
            require(
                index < FILTER_KEYS && raw.len() <= FILTER_KEY_BYTES,
                "filter key bounds",
            )?;
            let key = std::str::from_utf8(raw).map_err(|_| Error::Invalid("filter key UTF-8"))?;
            let rest = key
                .strip_prefix("filter.")
                .ok_or(Error::Invalid("filter key prefix"))?;
            let driver = [".clean", ".smudge", ".process", ".required"]
                .iter()
                .find_map(|suffix| rest.strip_suffix(suffix))
                .ok_or(Error::Invalid("filter key suffix"))?;
            // Deliberately supported subset of Git subsection names; dots belong
            // to the complete case-sensitive driver, not to a split key grammar.
            require(
                !driver.is_empty()
                    && driver.len() <= FILTER_DRIVER_BYTES
                    && driver
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
                "filter driver grammar",
            )?;
            if !drivers.iter().any(|d| d == driver) {
                require(drivers.len() < FILTER_DRIVERS, "filter driver count")?;
                drivers
                    .try_reserve(1)
                    .map_err(|_| Error::Invalid("allocation"))?;
                drivers.push(driver.to_owned());
            }
        }
        drivers.sort();
        Ok(Self(drivers))
    }
}
fn git_command(root: &Path, query: GitQuery<'_>) -> Command {
    let mut command = Command::new("git");
    command.arg("--no-optional-locks").arg("--no-pager");
    if !matches!(query, GitQuery::Version) {
        command.args(["--no-lazy-fetch", "-c", "core.fsmonitor=false"]);
    }
    if let GitQuery::Status(drivers) = query {
        for driver in &drivers.0 {
            for field in ["clean", "smudge", "process"] {
                command.arg("-c").arg(format!("filter.{driver}.{field}="));
            }
            command
                .arg("-c")
                .arg(format!("filter.{driver}.required=false"));
        }
    }
    command.arg("-C").arg(root);
    match query {
        GitQuery::Version => {
            command.arg("--version");
        }
        GitQuery::Filters => {
            command.args([
                "config",
                "--includes",
                "--null",
                "--name-only",
                "--get-regexp",
                FILTER_QUERY,
            ]);
        }
        GitQuery::Head => {
            command.args(["rev-parse", "HEAD"]);
        }
        GitQuery::IndexClean => {
            command.args([
                "diff-index",
                "--cached",
                "--quiet",
                "--no-ext-diff",
                "--ignore-submodules=none",
                "HEAD",
                "--",
            ]);
        }
        GitQuery::Status(_) => {
            command.args([
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ]);
        }
    }
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    #[cfg(test)]
    git_tests::environment(&mut command);
    if matches!(query, GitQuery::Filters) {
        // GIT_CONFIG redirects only `git config`, not `git status`. Ignoring
        // that selector makes discovery observe the same sources as status.
        command.env_remove("GIT_CONFIG");
    }
    if !matches!(query, GitQuery::Version) {
        command.env("GIT_NO_LAZY_FETCH", "1");
    }
    command
}
fn supported_git_version(bytes: &[u8]) -> Result<&str> {
    let raw = std::str::from_utf8(bytes).map_err(|_| Error::Invalid("Git version UTF-8"))?;
    let version = raw
        .strip_prefix("git version ")
        .and_then(|s| s.strip_suffix('\n'))
        .ok_or(Error::Invalid("Git version shape"))?;
    let mut fields = version.split('.');
    let mut numbers = [0u32; 3];
    for number in &mut numbers {
        let field = fields.next().ok_or(Error::Invalid("Git version shape"))?;
        require(
            !field.is_empty()
                && field.bytes().all(|b| b.is_ascii_digit())
                && (field == "0" || !field.starts_with('0')),
            "Git version shape",
        )?;
        *number = field
            .parse()
            .map_err(|_| Error::Invalid("Git version number"))?;
    }
    require(fields.next().is_none(), "Git version shape")?;
    require(numbers >= [2, 45, 0], "Git >= 2.45.0 required")?;
    Ok(version)
}
struct VerificationGit<'a> {
    root: &'a Path,
}
impl<'a> VerificationGit<'a> {
    fn open(root: &'a Path) -> Result<Self> {
        let git = Self { root };
        let version = git.run(GitQuery::Version)?;
        let exact = supported_git_version(&version)?;
        eprintln!("Preparation environment: git version {exact}");
        Ok(git)
    }
    fn filters(&self) -> Result<FilterDrivers> {
        FilterDrivers::parse(&self.run(GitQuery::Filters)?)
    }
    fn status(&self) -> Result<Vec<u8>> {
        self.run(GitQuery::IndexClean)?;
        let before = self.filters()?;
        let result = self.run(GitQuery::Status(&before))?;
        #[cfg(test)]
        git_tests::after_status(self.root);
        let after = self.filters()?;
        require(before == after, "filter driver set changed")?;
        self.run(GitQuery::IndexClean)?;
        Ok(result)
    }
    fn run(&self, query: GitQuery<'_>) -> Result<Vec<u8>> {
        let mut command = git_command(self.root, query);
        #[cfg(unix)]
        {
            run_git_query(
                &mut command,
                std::time::Duration::from_secs(30),
                if matches!(query, GitQuery::Filters) {
                    GitExit::EmptyFilterSet
                } else if matches!(query, GitQuery::IndexClean) {
                    GitExit::IndexClean
                } else {
                    GitExit::Success
                },
            )
        }
        #[cfg(not(unix))]
        {
            let _ = command;
            Err(Error::Unsupported)
        }
    }
}
#[cfg(unix)]
struct GitChild {
    child: std::process::Child,
    reaped: bool,
}
#[cfg(unix)]
impl GitChild {
    fn cleanup(&mut self) -> Result<()> {
        if self.reaped {
            return Ok(());
        }
        // An unreaped direct child cannot have its PID recycled.
        let signal = self.child.kill();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.reaped = true;
                    return Ok(());
                }
                Ok(None) => {
                    if signal.is_err() {
                        return Err(Error::Cleanup);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
                Err(_) => return Err(Error::Cleanup),
            }
            if std::time::Instant::now() >= end {
                return Err(Error::Cleanup);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
#[cfg(unix)]
impl Drop for GitChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.try_wait();
        }
    }
}
#[cfg(unix)]
#[derive(Clone, Copy)]
enum GitExit {
    Success,
    IndexClean,
    EmptyFilterSet,
}
#[cfg(all(test, unix))]
fn run_git(command: &mut Command, timeout: std::time::Duration) -> Result<Vec<u8>> {
    run_git_query(command, timeout, GitExit::Success)
}
#[cfg(unix)]
fn run_git_query(
    command: &mut Command,
    timeout: std::time::Duration,
    policy: GitExit,
) -> Result<Vec<u8>> {
    use std::{io::Read, os::fd::AsRawFd, process::Stdio, time::Instant};
    let end = Instant::now() + timeout;
    let mut owner = GitChild {
        child: command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
        reaped: false,
    };
    let operation = (|| {
        let mut stdout = owner
            .child
            .stdout
            .take()
            .ok_or(Error::Invalid("git stdout"))?;
        let mut stderr = owner
            .child
            .stderr
            .take()
            .ok_or(Error::Invalid("git stderr"))?;
        for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            require(
                flags >= 0
                    && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0,
                "git nonblocking pipes",
            )?;
        }
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut eof = [false; 2];
        let mut status = None;
        loop {
            if Instant::now() >= end {
                return Err(Error::Timeout);
            }
            for (index, reader, bytes, limit) in [
                (0, &mut stdout as &mut dyn Read, &mut out, 65536usize),
                (1, &mut stderr as &mut dyn Read, &mut err, 16384usize),
            ] {
                // One bounded read per stream per iteration prevents a noisy peer starving the deadline.
                let mut buffer = [0u8; 4096];
                match reader.read(&mut buffer) {
                    Ok(0) => eof[index] = true,
                    Ok(n) => {
                        require(bytes.len() + n <= limit, "git output limit")?;
                        bytes.extend_from_slice(&buffer[..n]);
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            if status.is_none() {
                match owner.child.try_wait() {
                    Ok(Some(s)) => {
                        owner.reaped = true;
                        status = Some(s);
                    }
                    Ok(None) => (),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => (),
                    Err(e) => return Err(e.into()),
                }
            }
            if let Some(status) = status
                && eof.iter().all(|v| *v)
            {
                if matches!(policy, GitExit::EmptyFilterSet)
                    && status.code() == Some(1)
                    && out.is_empty()
                    && err.is_empty()
                {
                    return Ok(out);
                }
                if matches!(policy, GitExit::IndexClean) && status.code() == Some(1) {
                    return Err(Error::Invalid("index differs from HEAD"));
                }
                if !status.success() {
                    return Err(Error::Io(format!(
                        "git failed ({status}): {}",
                        String::from_utf8_lossy(&err)
                    )));
                }
                return Ok(out);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    })();
    let cleanup = owner.cleanup();
    #[cfg(test)]
    let cleanup = git_tests::cleanup(cleanup, owner.child.id(), owner.reaped);
    crate::error::after_cleanup(operation, cleanup)
}

pub fn verify_checkout(root: &Path) -> Result<()> {
    require(root.is_absolute(), "absolute source root")?;
    let git = VerificationGit::open(root)?;
    verify_revision(&git.run(GitQuery::Head)?)?;
    require(git.status()?.is_empty(), "clean frozen checkout")?;
    verify_sources(root)
}
pub fn verify_sources(root: &Path) -> Result<()> {
    for (path, digest, count) in [
        (COLLECTOR_PATH, COLLECTOR_HASH, 47),
        (QUALIFICATION_PATH, QUALIFICATION_HASH, 9),
    ] {
        let bytes = canonical::read_confined(root, path, 65536)?;
        require(canonical::hash(&bytes) == digest, "source manifest digest")?;
        let m: Manifest = toml::from_str(
            std::str::from_utf8(&bytes).map_err(|_| Error::Invalid("manifest UTF-8"))?,
        )
        .map_err(|_| Error::Invalid("source manifest schema"))?;
        require(
            m.format == "borrowser-capture-source-manifest-v1" && m.entries.len() == count,
            "source membership",
        )?;
        let mut w = Writer::new(65536);
        w.field("format", &m.format)?;
        let mut last = String::new();
        let mut total = 0usize;
        for e in m.entries {
            canonical::relative(&e.path)?;
            canonical::digest(&e.sha256)?;
            require(e.path > last, "source order")?;
            let source = canonical::read_confined(root, &e.path, 1_048_576)?;
            total = total
                .checked_add(source.len())
                .ok_or(Error::Invalid("source total"))?;
            require(
                total <= 4 * 1_048_576 && canonical::hash(&source) == e.sha256,
                "source bytes",
            )?;
            w.raw("[[entries]]\n")?;
            w.field("path", &e.path)?;
            w.field("sha256", &e.sha256)?;
            last = e.path;
        }
        require(w.finish() == bytes, "canonical source manifest")?;
    }
    let inspector = canonical::read_confined(root, INSPECTOR_PATH, 65536)?;
    let packaging = canonical::read_confined(root, PACKAGING_PATH, 65536)?;
    require(
        canonical::hash(&inspector) == INSPECTOR_HASH,
        "inspector source",
    )?;
    require(
        canonical::hash(&packaging) == PACKAGING_HASH,
        "packaging source",
    )?;
    require(
        canonical::hash(&expression(&inspector)?) == EXPRESSION_HASH,
        "expression digest",
    )
}
/// Independently implements the published expression transform, not imported code.
pub fn expression(raw: &[u8]) -> Result<Vec<u8>> {
    require(raw.len() <= 65536, "inspector size")?;
    let source = std::str::from_utf8(raw).map_err(|_| Error::Invalid("inspector UTF-8"))?;
    let sites = [
        "export const algorithm =",
        "export const MAX_BYTES =",
        "export class InspectionFailure",
        "export function inspectWebObservableDomTreeV1(",
        "export function captureWebObservableDomTreeV1(",
    ];
    let mut seen = [false; 5];
    let mut out = String::from("(() => {\n\"use strict\";\n");
    for line in source.split_inclusive('\n') {
        if let Some(i) = sites.iter().position(|p| line.starts_with(p)) {
            require(!seen[i], "duplicate inspector export")?;
            seen[i] = true;
            out.push_str(&line[7..]);
        } else {
            out.push_str(line);
        }
    }
    require(seen.iter().all(|b| *b), "missing inspector export")?;
    out.push_str("\nif (document.readyState !== \"complete\" ||\n    document.contentType !== \"text/html\" ||\n    document.characterSet !== \"UTF-8\") {\n  throw new InspectionFailure(\"capture-document-context\");\n}\nreturn new TextDecoder(\"utf-8\", { fatal: true })\n  .decode(inspectWebObservableDomTreeV1(document));\n})()\n");
    Ok(out.into_bytes())
}

fn entries<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<Source>, D::Error> {
    canonical::sequence::<D, Source, 128>(d)
}

pub fn verify_revision(head: &[u8]) -> Result<()> {
    require(
        head == format!("{REVISION}\n").as_bytes(),
        "frozen revision",
    )
}

#[cfg(all(test, unix))]
mod git_tests {
    use super::*;
    use std::{io::Write, time::Duration};
    thread_local! {static FAIL:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
    pub(super) fn cleanup(result: Result<()>, pid: u32, reaped: bool) -> Result<()> {
        assert!(reaped);
        let mut status = 0;
        assert_eq!(
            unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
        if FAIL.get() {
            Err(Error::Cleanup)
        } else {
            result
        }
    }
    #[test]
    fn child_entry() {
        let Ok(mode) = std::env::var("AG9G_GIT_TEST_CHILD") else {
            return;
        };
        match mode.as_str() {
            "small" => {
                print!("git-small-output");
            }
            "stdout" => {
                std::io::stdout().write_all(&vec![b'x'; 70000]).unwrap();
            }
            "stderr" => {
                std::io::stderr().write_all(&vec![b'x'; 20000]).unwrap();
            }
            "stall" => {
                std::thread::sleep(Duration::from_secs(10));
            }
            "failure" => {
                eprintln!("bounded diagnostic");
                std::process::exit(7);
            }
            _ => unreachable!(),
        }
    }
    fn command(mode: &str) -> Command {
        let mut c = Command::new(std::env::current_exe().unwrap());
        c.args([
            "--exact",
            "source_identity::git_tests::child_entry",
            "--nocapture",
        ])
        .env("AG9G_GIT_TEST_CHILD", mode);
        c
    }
    type Environment = Box<dyn FnMut(&mut Command)>;
    type Mutation = Box<dyn FnMut(&Path)>;
    thread_local! {
        static ENVIRONMENT: std::cell::RefCell<Option<Environment>> = std::cell::RefCell::new(None);
        static AFTER_STATUS: std::cell::RefCell<Option<Mutation>> = std::cell::RefCell::new(None);
    }
    pub(super) fn environment(command: &mut Command) {
        ENVIRONMENT.with_borrow_mut(|h| {
            if let Some(h) = h {
                h(command);
            }
        });
    }
    pub(super) fn after_status(root: &Path) {
        AFTER_STATUS.with_borrow_mut(|h| {
            if let Some(h) = h {
                h(root);
            }
        });
    }
    struct FilterRepo(tempfile::TempDir);
    impl FilterRepo {
        fn new(driver: &str) -> Self {
            ENVIRONMENT.set(Some(Box::new(|c| {
                c.env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_COUNT", "0")
                    .env_remove("GIT_CONFIG_PARAMETERS");
            })));
            let r = Self(tempfile::tempdir().unwrap());
            r.setup(&["init", "-q"]);
            std::fs::write(
                r.0.path().join(".gitattributes"),
                format!("tracked filter={driver}\n"),
            )
            .unwrap();
            std::fs::write(r.0.path().join("tracked"), b"INDEX\n").unwrap();
            r.setup(&["add", ".gitattributes", "tracked"]);
            r.setup(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-qm",
                "test",
            ]);
            r
        }
        fn setup(&self, args: &[&str]) -> Vec<u8> {
            let mut c = Command::new("git");
            c.arg("--no-optional-locks")
                .arg("-C")
                .arg(self.0.path())
                .args(args);
            c.env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_COUNT", "0")
                .env_remove("GIT_CONFIG_PARAMETERS");
            run_git(&mut c, Duration::from_secs(5)).unwrap()
        }
        fn hook(&self, body: &str) -> String {
            use std::os::unix::fs::PermissionsExt;
            let path = self.0.path().join(".git/filter-hook");
            std::fs::write(
                &path,
                format!("#!/bin/sh\nprintf invoked > .git/filter-ran\n{body}\n"),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            // Fixed relative command avoids shell quoting any generated path.
            ".git/filter-hook".into()
        }
        fn dirty(&self) {
            std::fs::write(self.0.path().join("tracked"), b"DIRTY\n").unwrap();
            std::fs::File::options()
                .write(true)
                .open(self.0.path().join("tracked"))
                .unwrap()
                .set_times(
                    std::fs::FileTimes::new()
                        .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1)),
                )
                .unwrap();
        }
        fn marker(&self) -> std::path::PathBuf {
            self.0.path().join(".git/filter-ran")
        }
    }
    impl Drop for FilterRepo {
        fn drop(&mut self) {
            ENVIRONMENT.set(None);
            AFTER_STATUS.set(None);
        }
    }
    #[test]
    fn index_clean_and_staged_change() {
        let r = FilterRepo::new("none");
        let git = VerificationGit::open(r.0.path()).unwrap();
        assert!(git.run(GitQuery::IndexClean).unwrap().is_empty());
        r.dirty();
        // Cached comparison ignores unstaged worktree content.
        assert!(git.run(GitQuery::IndexClean).unwrap().is_empty());
        r.setup(&["add", "tracked"]);
        assert!(matches!(
            git.status(),
            Err(Error::Invalid("index differs from HEAD"))
        ));
    }
    #[test]
    fn index_stability_after_status() {
        let r = FilterRepo::new("none");
        let git = VerificationGit::open(r.0.path()).unwrap();
        let head = String::from_utf8(r.setup(&["rev-parse", "HEAD"])).unwrap();
        AFTER_STATUS.set(Some(Box::new(move |root| {
            let mut c = Command::new("git");
            c.arg("-C").arg(root).args([
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{},late-link", head.trim()),
            ]);
            run_git(&mut c, Duration::from_secs(5)).unwrap();
        })));
        assert!(matches!(
            git.status(),
            Err(Error::Invalid("index differs from HEAD"))
        ));
    }
    #[test]
    fn index_gitlink_rejected_and_status_never_recurses() {
        let r = FilterRepo::new("none");
        r.setup(&["init", "-q", "sub"]);
        std::fs::write(r.0.path().join("sub/file"), "original").unwrap();
        r.setup(&["-C", "sub", "add", "file"]);
        r.setup(&[
            "-C",
            "sub",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "nested",
        ]);
        let head = String::from_utf8(r.setup(&["-C", "sub", "rev-parse", "HEAD"])).unwrap();
        r.setup(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},sub", head.trim()),
        ]);
        std::fs::write(r.0.path().join("sub/file"), "modified").unwrap();
        let trace = r.0.path().join(".git/trace.json");
        let mut ordinary = Command::new("git");
        ordinary
            .arg("-C")
            .arg(r.0.path())
            .args(["status", "--porcelain", "--ignore-submodules=none"])
            .env("GIT_TRACE2_EVENT", &trace);
        run_git(&mut ordinary, Duration::from_secs(5)).unwrap();
        assert!(
            std::fs::read_to_string(&trace)
                .unwrap()
                .contains("\"event\":\"child_start\"")
        );
        std::fs::remove_file(&trace).unwrap();
        let trace_copy = trace.clone();
        ENVIRONMENT.set(Some(Box::new(move |c| {
            c.env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_COUNT", "1")
                .env("GIT_CONFIG_KEY_0", "diff.ignoreSubmodules")
                .env("GIT_CONFIG_VALUE_0", "all")
                .env_remove("GIT_CONFIG_PARAMETERS")
                .env("GIT_TRACE2_EVENT", &trace_copy);
        })));
        let git = VerificationGit::open(r.0.path()).unwrap();
        assert!(matches!(
            git.status(),
            Err(Error::Invalid("index differs from HEAD"))
        ));
        // Exercise the actual hardened argv separately even with the dirty gitlink.
        let drivers = git.filters().unwrap();
        let command = git_command(r.0.path(), GitQuery::Status(&drivers));
        assert!(
            command
                .get_args()
                .any(|arg| arg == "--ignore-submodules=all")
        );
        git.run(GitQuery::Status(&drivers)).unwrap();
        let trace = std::fs::read_to_string(&trace).unwrap();
        assert!(!trace.contains("\"event\":\"child_start\""));
    }
    #[test]
    fn filter_clean_required_and_dotted_drivers_are_inert() {
        for driver in ["simple", "case.Sensitive-driver"] {
            let r = FilterRepo::new(driver);
            let hook = r.hook("printf 'INDEX\\n'");
            r.setup(&["config", &format!("filter.{driver}.clean"), &hook]);
            r.setup(&["config", &format!("filter.{driver}.required"), "true"]);
            r.dirty();
            assert!(
                r.setup(&["status", "--porcelain", "--untracked-files=all"])
                    .is_empty()
            );
            assert!(r.marker().exists());
            std::fs::remove_file(r.marker()).unwrap();
            let git = VerificationGit::open(r.0.path()).unwrap();
            assert_eq!(git.filters().unwrap(), FilterDrivers(vec![driver.into()]));
            let status = git.status().unwrap();
            assert_eq!(status, b" M tracked\n"); // No filter compensation: the raw checkout is dirty.
            assert!(!r.marker().exists());
            std::fs::write(r.0.path().join("tracked"), b"INDEX\n").unwrap();
            assert!(git.status().unwrap().is_empty());
            assert!(!r.marker().exists());
        }
    }
    #[test]
    fn filter_process_is_not_started_even_when_required() {
        let r = FilterRepo::new("long.running");
        let hook = r.hook("exit 0"); // Invocation witness, deliberately not a valid filter-protocol peer.
        r.setup(&["config", "filter.long.running.process", &hook]);
        r.dirty();
        let mut ordinary = Command::new("git");
        ordinary
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(r.0.path())
            .args(["status", "--porcelain", "--untracked-files=all"]);
        // The witness exits instead of speaking the long-running protocol, so
        // Git may fail its handshake. The marker is the invocation proof.
        let _positive_result = run_git(&mut ordinary, Duration::from_secs(5));
        assert!(r.marker().exists());
        std::fs::remove_file(r.marker()).unwrap();
        r.setup(&["config", "filter.long.running.required", "true"]);
        let git = VerificationGit::open(r.0.path()).unwrap();
        assert_eq!(git.status().unwrap(), b" M tracked\n");
        assert!(!r.marker().exists());
    }
    #[test]
    fn filter_environment_discovery_and_set_stability() {
        let r = FilterRepo::new("env.driver");
        let hook = r.hook("exit 0");
        r.setup(&["config", "filter.local.only.required", "false"]);
        ENVIRONMENT.set(Some(Box::new(move |c| {
            c.env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG", "/dev/null")
                .env("GIT_CONFIG_COUNT", "2")
                .env("GIT_CONFIG_KEY_0", "filter.env.driver.clean")
                .env("GIT_CONFIG_VALUE_0", &hook)
                .env("GIT_CONFIG_KEY_1", "filter.env.driver.required")
                .env("GIT_CONFIG_VALUE_1", "true")
                .env(
                    "GIT_CONFIG_PARAMETERS",
                    "'filter.env.driver.process=.git/filter-hook'",
                );
        })));
        r.dirty();
        let git = VerificationGit::open(r.0.path()).unwrap();
        assert_eq!(
            git.filters().unwrap(),
            FilterDrivers(vec!["env.driver".into(), "local.only".into()])
        );
        assert_eq!(git.status().unwrap(), b" M tracked\n");
        assert!(!r.marker().exists());
        AFTER_STATUS.set(Some(Box::new(|root| {
            std::fs::OpenOptions::new()
                .append(true)
                .open(root.join(".git/config"))
                .unwrap()
                .write_all(b"\n[filter \"new.driver\"]\nrequired = false\n")
                .unwrap();
        })));
        assert_eq!(
            git.status(),
            Err(Error::Invalid("filter driver set changed"))
        );
    }
    #[test]
    fn filter_discovery_bounds_framing_and_empty_results() {
        assert_eq!(FilterDrivers::parse(b"").unwrap(), FilterDrivers(vec![]));
        assert_eq!(
            FilterDrivers::parse(b"filter.a.b.clean\0filter.a.b.required\0").unwrap(),
            FilterDrivers(vec!["a.b".into()])
        );
        for raw in [
            b"filter.a.clean".as_slice(),
            b"filter.a.clean\0\0",
            b"filter..clean\0",
            b"other.a.clean\0",
            b"filter.a.unknown\0",
            b"filter.a=b.clean\0",
            b"filter.a\n.clean\0",
            b"filter.\xff.clean\0",
        ] {
            assert!(FilterDrivers::parse(raw).is_err(), "{raw:?}");
        }
        assert!(
            FilterDrivers::parse(format!("filter.{}.clean\0", "x".repeat(129)).as_bytes()).is_err()
        );
        assert!(FilterDrivers::parse(&vec![b'x'; 65537]).is_err());
        assert!(FilterDrivers::parse(&b"filter.a.clean\0".repeat(1025)).is_err());
        let many: String = (0..129).map(|i| format!("filter.d{i}.clean\0")).collect();
        assert!(FilterDrivers::parse(many.as_bytes()).is_err());
        let r = FilterRepo::new("none");
        let git = VerificationGit::open(r.0.path()).unwrap();
        assert_eq!(git.filters().unwrap(), FilterDrivers(vec![])); // Git exit 1: no matches.
        let mut config = std::fs::OpenOptions::new()
            .append(true)
            .open(r.0.path().join(".git/config"))
            .unwrap();
        for i in 0..700 {
            writeln!(
                config,
                "\n[filter \"{}{}\"]\nrequired = false",
                "d".repeat(120),
                i
            )
            .unwrap();
        }
        drop(config);
        assert_eq!(git.filters(), Err(Error::Invalid("git output limit")));
    }
    #[test]
    fn missing_promisor_object_never_fetches() {
        let r = FilterRepo::new("none");
        let tree = String::from_utf8(r.setup(&["rev-parse", "HEAD^{tree}"])).unwrap();
        let tree = tree.trim();
        let missing =
            r.0.path()
                .join(".git/objects")
                .join(&tree[..2])
                .join(&tree[2..]);
        std::fs::remove_file(&missing).unwrap();
        r.setup(&["config", "remote.origin.promisor", "true"]);
        r.setup(&[
            "config",
            "remote.origin.url",
            "file:///nonexistent/ag9g-local-only",
        ]);
        let trace = r.0.path().join(".git/lazy-trace.json");
        let mut ordinary = Command::new("git");
        ordinary
            .arg("--no-optional-locks")
            .arg("--no-pager")
            .arg("-C")
            .arg(r.0.path())
            .args([
                "-c",
                "core.fsmonitor=false",
                "diff-index",
                "--cached",
                "--quiet",
                "--no-ext-diff",
                "--ignore-submodules=none",
                "HEAD",
                "--",
            ])
            .env("GIT_NO_LAZY_FETCH", "0")
            .env("GIT_TRACE2_EVENT", &trace);
        assert!(run_git(&mut ordinary, Duration::from_secs(5)).is_err());
        let events = std::fs::read_to_string(&trace).unwrap();
        let children: Vec<serde_json::Value> = events
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|v: &serde_json::Value| v["event"] == "child_start")
            .collect();
        assert!(
            children
                .iter()
                .any(|v| v["argv"].as_array().unwrap().iter().any(|a| a == "fetch"))
        );
        assert!(
            children
                .iter()
                .any(|v| v["argv"].to_string().contains("upload-pack"))
        );
        std::fs::remove_file(&trace).unwrap();
        let trace_copy = trace.clone();
        ENVIRONMENT.set(Some(Box::new(move |c| {
            c.env("GIT_NO_LAZY_FETCH", "0")
                .env("GIT_TRACE2_EVENT", &trace_copy);
        })));
        let git = VerificationGit::open(r.0.path()).unwrap();
        let drivers = git.filters().unwrap();
        for query in [
            GitQuery::Filters,
            GitQuery::Head,
            GitQuery::IndexClean,
            GitQuery::Status(&drivers),
        ] {
            let c = git_command(r.0.path(), query);
            assert!(c.get_args().any(|a| a == "--no-lazy-fetch"));
            assert!(
                c.get_envs()
                    .any(|(k, v)| k == "GIT_NO_LAZY_FETCH" && v == Some(std::ffi::OsStr::new("1")))
            );
        }
        assert!(git.run(GitQuery::Head).is_ok());
        assert!(matches!(git.status(), Err(Error::Io(_))));
        assert!(!missing.exists());
        let events = std::fs::read_to_string(trace).unwrap();
        assert!(
            !events
                .lines()
                .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
                .any(|v| v["event"] == "child_start")
        );
    }
    #[test]
    fn supported_version_is_unambiguous_and_modern() {
        for version in ["2.45.0", "2.45.1", "2.50.0"] {
            let raw = format!("git version {version}\n");
            assert_eq!(supported_git_version(raw.as_bytes()).unwrap(), version);
        }
        for raw in [
            "git version 2.35.1\n",
            "git version 2.36.0\n",
            "git version 2.44.9\n",
            "git version 1.9.0\n",
            "git version 2.36\n",
            "git version 2.36.0.rc1\n",
            "git version 2.36.0 (vendor)\n",
            "git version 02.36.0\n",
            "git version 2.36.0\nextra",
            "git version 2.36.0\r\n",
            "2.36.0\n",
            "git version 2.36.0",
        ] {
            assert!(supported_git_version(raw.as_bytes()).is_err(), "{raw:?}");
        }
    }
    #[test]
    fn configured_fsmonitor_never_executes() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        // Setup commands are test-only; production vocabulary remains closed.
        let setup = |args: &[&str]| {
            let mut c = Command::new("git");
            c.arg("-C").arg(d.path()).args(args);
            run_git(&mut c, Duration::from_secs(5)).unwrap()
        };
        setup(&["init", "-q"]);
        std::fs::write(d.path().join("tracked"), b"tracked").unwrap();
        setup(&["add", "tracked"]);
        setup(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "test",
        ]);
        std::fs::write(d.path().join("tracked"), b"unstaged modification").unwrap();
        let hook = d.path().join(".git/fsmonitor-hook");
        let marker = d.path().join(".git/fsmonitor-ran");
        std::fs::write(&hook, b"#!/bin/sh\nprintf invoked > .git/fsmonitor-ran\n").unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).unwrap();
        setup(&["config", "core.fsmonitor", hook.to_str().unwrap()]);
        // Positive control proves this repository would execute the configured hook.
        setup(&["status", "--porcelain", "--untracked-files=all"]);
        assert!(marker.exists());
        std::fs::remove_file(&marker).unwrap();
        let git = VerificationGit::open(d.path()).unwrap();
        assert!(!git.status().unwrap().is_empty());
        assert!(!marker.exists());
        let filters = git.filters().unwrap();
        let mut c = git_command(d.path(), GitQuery::Status(&filters));
        c.env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "core.fsmonitor")
            .env("GIT_CONFIG_VALUE_0", hook.as_os_str())
            .env("GIT_CONFIG_PARAMETERS", "'core.fsmonitor=true'");
        run_git(&mut c, Duration::from_secs(5)).unwrap();
        assert!(!marker.exists());
        // The same false setting overrides the built-in-monitor request too.
        setup(&["config", "core.fsmonitor", "true"]);
        assert!(!git.status().unwrap().is_empty());
        assert!(!marker.exists());
    }
    #[test]
    fn bounded_outputs_deadline_exit_and_checked_reaping() {
        assert!(
            String::from_utf8(run_git(&mut command("small"), Duration::from_secs(2)).unwrap())
                .unwrap()
                .contains("git-small-output")
        );
        for mode in ["stdout", "stderr"] {
            assert_eq!(
                run_git(&mut command(mode), Duration::from_secs(2)),
                Err(Error::Invalid("git output limit"))
            );
        }
        assert_eq!(
            run_git(&mut command("stall"), Duration::from_millis(100)),
            Err(Error::Timeout)
        );
        assert!(
            matches!(run_git(&mut command("failure"), Duration::from_secs(2)), Err(Error::Io(s)) if s.contains("bounded diagnostic"))
        );
        FAIL.set(true);
        let result = run_git(&mut command("stall"), Duration::from_millis(100));
        FAIL.set(false);
        assert_eq!(result, Err(Error::Cleanup));
    }
}
