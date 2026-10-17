use super::{
    artifact::Prepared,
    cloudformation::Aws,
    command::{Commands, Failure, Output, Spec},
    config::{AwsEnvironment, Config},
    eice::{self, VerifiedEndpoint, VerifiedTarget},
    state::{Record, Store},
};
use base64::Engine;
use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) fn key(
    commands: &mut impl Commands,
    config: &Config,
    work: &Path,
) -> Result<PathBuf, String> {
    let key = work.join("id_ed25519");
    let output = commands
        .run(Spec::new(
            &config.ssh_keygen,
            vec![
                "-q".into(),
                "-t".into(),
                "ed25519".into(),
                "-N".into(),
                "".into(),
                "-C".into(),
                "ag4".into(),
                "-f".into(),
                key.as_os_str().to_owned(),
            ],
        ))
        .map_err(|e| e.to_string())?;
    if output.code != 0 {
        return Err("operation SSH key generation failed".into());
    }
    Ok(key)
}
pub(super) struct Host {
    pub target: VerifiedTarget,
    pub endpoint: VerifiedEndpoint,
    pub known_hosts: PathBuf,
    pub key: PathBuf,
}
// Reviewed alongside bootstrap.sh's entire phase bounds. Do not shorten this
// window to fit a late deployment; preserve a full window and capture reserve.
const READINESS_SECONDS: u64 = 1080;
const CAPTURE_RESERVE_SECONDS: u64 = 540;

fn bootstrap_failure<'a>(text: &'a str, run_id: &str) -> Result<Option<(&'a str, u8)>, String> {
    let mut failure = None;
    for line in text.lines().map(|line| line.trim_end_matches('\r')) {
        if !line.starts_with("BORROWSER_AG4_BOOTSTRAP_FAILED") {
            continue;
        }
        let fields: Vec<_> = line.split(' ').collect();
        if line.len() > 128
            || fields.len() != 4
            || fields[0] != "BORROWSER_AG4_BOOTSTRAP_FAILED"
            || fields[1] != run_id
            || !matches!(
                fields[2],
                "setup"
                    | "apt_update"
                    | "apt_install"
                    | "chromium_download"
                    | "archive"
                    | "sandbox"
                    | "ssh_prepare"
                    | "ssh_config"
                    | "ssh_validate"
                    | "ssh_restart"
                    | "ssh_probe_loopback"
                    | "ssh_probe_private"
                    | "ready"
            )
        {
            return Err("malformed or unrelated bootstrap failure marker".into());
        }
        let status = fields[3]
            .parse::<u8>()
            .ok()
            .filter(|status| *status != 0 && status.to_string() == fields[3])
            .ok_or("invalid bootstrap failure status")?;
        let observed = (fields[2], status);
        if failure.is_some_and(|previous| previous != observed) {
            return Err("conflicting bootstrap failure markers".into());
        }
        failure = Some(observed);
    }
    Ok(failure)
}

pub(super) fn ready<C: Commands>(
    aws: &mut Aws<'_, C>,
    r: &mut Record,
    store: &Store,
    work: &Path,
    key: PathBuf,
    endpoint: &VerifiedEndpoint,
) -> Result<Host, String> {
    let work_deadline = aws.deadline;
    let start = aws.commands.elapsed();
    if work_deadline.saturating_sub(start)
        < Duration::from_secs(READINESS_SECONDS + CAPTURE_RESERVE_SECONDS)
    {
        return Err(
            "insufficient work budget for full bootstrap readiness and capture reserve".into(),
        );
    }
    let deadline = start + Duration::from_secs(READINESS_SECONDS);
    aws.deadline = deadline;
    for _ in 0..READINESS_SECONDS / 5 {
        if aws.commands.elapsed() >= deadline {
            break;
        }
        let instance = aws
            .instance(r, store)?
            .ok_or("instance disappeared during readiness")?;
        if instance["State"]["Name"] != "running" {
            aws.pause(Duration::from_secs(5))?;
            continue;
        }
        let console = aws.call(
            "ec2",
            "get-console-output",
            &[
                "--instance-id".into(),
                r.instance_id.clone().ok_or("missing instance id")?,
                "--latest".into(),
            ],
        )?;
        if console["InstanceId"] != r.instance_id.as_deref().unwrap() {
            return Err("console instance identity mismatch".into());
        }
        let Some(text) = console.get("Output").and_then(serde_json::Value::as_str) else {
            aws.pause(Duration::from_secs(5))?;
            continue;
        };
        // The pinned AWS CLI decodes EC2's base64 console string itself.
        if let Some((phase, status)) = bootstrap_failure(text, &r.run_id)? {
            return Err(format!(
                "guest bootstrap failed: phase={phase}, exit-status={status}"
            ));
        }
        let prefix = format!("BORROWSER_AG4_READY {} ", r.run_id);
        let keys: Vec<_> = text
            .lines()
            .filter_map(|line| line.trim_end_matches('\r').strip_prefix(&prefix))
            .collect();
        if keys.is_empty() {
            aws.pause(Duration::from_secs(5))?;
            continue;
        }
        if keys.iter().any(|other| *other != keys[0]) {
            return Err("conflicting console SSH host keys".into());
        }
        let fields: Vec<_> = keys[0].split_whitespace().collect();
        if fields.len() != 2
            || fields[0] != "ssh-ed25519"
            || base64::engine::general_purpose::STANDARD
                .decode(fields[1])
                .map_err(|_| "invalid host key encoding")?
                .len()
                != 51
        {
            return Err("invalid authenticated SSH host key".into());
        }
        let target = endpoint.verify_target(aws, r, store, &instance, keys[0])?;
        let known_hosts = work.join("known_hosts");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&known_hosts)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{} {}", target.instance_id, target.host_key).map_err(|e| e.to_string())?;
        aws.deadline = work_deadline;
        return Ok(Host {
            target,
            endpoint: endpoint.clone(),
            known_hosts,
            key,
        });
    }
    Err("host/bootstrap readiness deadline exceeded".into())
}
fn spec(
    config: &Config,
    host: &Host,
    environment: &AwsEnvironment,
    remote: &str,
    timeout: Duration,
    limit: usize,
) -> Result<Spec, String> {
    let descriptor = eice::TunnelDescriptor::create(
        config,
        &host.endpoint,
        &host.target,
        host.known_hosts
            .parent()
            .ok_or("missing invocation directory")?,
        timeout,
    )?;
    let controller = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|_| "controller executable unavailable")?;
    let mut s = Spec::new(
        &config.ssh,
        [
            "-F",
            "/dev/null",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "IdentityAgent=none",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "GlobalKnownHostsFile=/dev/null",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "ServerAliveInterval=5",
            "-o",
            "ServerAliveCountMax=2",
            "-o",
            "HostKeyAlgorithms=ssh-ed25519",
            "-o",
            "UpdateHostKeys=no",
        ],
    );
    s.args.extend([
        "-o".into(),
        eice::proxy_command(&controller, &descriptor)?.into(),
        "-o".into(),
        // OpenSSH takes the first proxy mechanism, even ProxyJump=none.
        // Install our fixed ProxyCommand before explicitly disabling jumps.
        "ProxyJump=none".into(),
        "-o".into(),
        format!("HostKeyAlias={}", host.target.instance_id).into(),
        "-o".into(),
        format!("UserKnownHostsFile={}", host.known_hosts.display()).into(),
        "-i".into(),
        host.key.as_os_str().to_owned(),
        format!("ag4@{}", host.target.instance_id).into(),
        remote.into(),
    ]);
    s.timeout = timeout;
    s.stdout_limit = limit;
    s.failure_stderr_limit = 2048;
    s.eice_proxy = true;
    environment.apply(&mut s);
    Ok(s)
}
// Upload-only diagnostics. Never format Spec/input/stdout or read key material.
const UPLOAD_PRIMARY_LIMIT: usize = 8192;
const UPLOAD_CLEANUP_LIMIT: usize = 2048;
const UPLOAD_STDERR_LIMIT: usize = 2048;

fn upload_excerpt(environment: &AwsEnvironment, text: &str, limit: usize) -> String {
    // Redact before truncation so a credential crossing the boundary cannot leak.
    let mut text = environment.redact(text);
    if text.len() > limit {
        const TRUNCATED: &str = "...[truncated]";
        let mut end = limit - TRUNCATED.len();
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(TRUNCATED);
    }
    text
}

fn upload_failure(
    attempt: usize,
    result: Result<Output, Failure>,
    environment: &AwsEnvironment,
) -> (String, bool) {
    match result {
        Ok(output) => (
            format!(
                "upload attempt {attempt}: result=Output; ssh_exit={}; stderr={}",
                output.code,
                upload_excerpt(
                    environment,
                    &String::from_utf8_lossy(&output.stderr),
                    UPLOAD_STDERR_LIMIT,
                )
            ),
            false,
        ),
        Err(failure) => (
            format!(
                "upload attempt {attempt}: result=Failure; primary={}; cleanup={}; ssh_exit=unavailable",
                upload_excerpt(environment, &failure.primary, UPLOAD_PRIMARY_LIMIT),
                failure.cleanup.as_ref().map_or_else(
                    || "None".into(),
                    |error| format!(
                        "Some({})",
                        upload_excerpt(environment, error, UPLOAD_CLEANUP_LIMIT)
                    )
                ),
            ),
            failure.cleanup.is_some(),
        ),
    }
}

pub(super) fn capture<C: Commands>(
    aws: &mut Aws<'_, C>,
    config: &Config,
    host: &Host,
    artifact: &Prepared,
    test_id: Option<&str>,
    diagnostics: &mut impl Write,
) -> Result<Vec<u8>, String> {
    let work_deadline = aws.deadline;
    let commands = &mut *aws.commands;
    let upload_started = commands.elapsed();
    let upload_deadline = work_deadline.min(commands.elapsed() + Duration::from_secs(300));
    let mut uploaded = false;
    let mut failures = Vec::with_capacity(2);
    let mut attempts = 0;
    for attempt in 1..=2 {
        let remaining = upload_deadline.saturating_sub(commands.elapsed());
        if remaining.is_zero() {
            break;
        }
        let mut upload = spec(
            config,
            host,
            &aws.environment,
            "umask 077; cat > /home/ag4/run/harness.part && chmod 700 /home/ag4/run/harness.part",
            remaining,
            1024,
        )?;
        upload.input = artifact.bytes.clone();
        attempts = attempt;
        match commands.run(upload) {
            Ok(o) if o.code == 0 => {
                uploaded = true;
                break;
            }
            result => {
                let (failure, unresolved) = upload_failure(attempt, result, &aws.environment);
                failures.push(failure);
                if unresolved {
                    return Err(format!("artifact transfer failed; {}", failures.join("; ")));
                }
            }
        }
    }
    if !uploaded {
        return Err(format!("artifact transfer failed; {}", failures.join("; ")));
    }
    // A recovered upload failure is operational context, never comparison data.
    for failure in failures {
        writeln!(diagnostics, "AG4 {failure}")
            .map_err(|e| format!("upload succeeded but diagnostic write failed: {e}"))?;
    }
    writeln!(
        diagnostics,
        "AG4 artifact upload: bytes={}, elapsed_ms={}, attempts={attempts}",
        artifact.bytes.len(),
        commands
            .elapsed()
            .saturating_sub(upload_started)
            .as_millis()
    )
    .map_err(|e| format!("upload diagnostic write failed: {e}"))?;
    let verify = format!(
        "cd /home/ag4/run && printf '%s  harness.part\\n' '{}' | sha256sum --check --strict >/dev/null && mv harness.part harness && ./harness --ag4-artifact-info",
        artifact.sha256
    );
    let identity = commands
        .run(spec(
            config,
            host,
            &aws.environment,
            &verify,
            work_deadline
                .saturating_sub(commands.elapsed())
                .min(Duration::from_secs(20)),
            16384,
        )?)
        .map_err(|e| e.to_string())?;
    if identity.code != 0 {
        return Err("remote artifact verification failed".into());
    }
    artifact.verify_remote(&identity.stdout)?;
    // IDs originate in the fixed registry, never untrusted shell text.
    let selected = test_id
        .map(|id| format!(" '{}'", id.replace('\'', "'\\''")))
        .unwrap_or_default();
    let execute = format!(
        "cd /home/ag4/run && TMPDIR=/tmp/ag4 /usr/bin/python3 /opt/borrowser-ag4/guest_capture.py --harness ./harness --chromium /opt/borrowser-ag2/chrome-linux64/chrome{selected}"
    );
    let result = commands.run(spec(
        config,
        host,
        &aws.environment,
        &execute,
        work_deadline
            .saturating_sub(commands.elapsed())
            .min(Duration::from_secs(120)),
        1024,
    )?);
    let (primary, command_unresolved) = match result {
        Ok(result) if result.code == 0 => (None, false),
        Ok(result) => (
            Some(format!(
                "remote Chromium execution exited {}; capture will not be retried; SSH stderr prefix: {}",
                result.code,
                upload_excerpt(
                    &aws.environment,
                    &String::from_utf8_lossy(&result.stderr),
                    2048
                )
            )),
            false,
        ),
        Err(error) => (
            Some(format!(
                "remote Chromium execution failed; capture will not be retried: {error}"
            )),
            error.cleanup.is_some(),
        ),
    };
    if let Some(primary) = primary {
        let remaining = work_deadline
            .saturating_sub(commands.elapsed())
            .saturating_sub(Duration::from_secs(5));
        let diagnostic = if command_unresolved || commands.interrupted(false) || remaining.is_zero()
        {
            "guest diagnostics unavailable: interruption, exhausted work budget, or unresolved command".into()
        } else {
            // One best-effort read, charged to capture work. Never borrows the
            // separate stack cleanup budget and never re-executes the harness.
            match commands.run(spec(
                config,
                host,
                &aws.environment,
                "head -c 8192 /home/ag4/run/capture.stderr",
                remaining.min(Duration::from_secs(10)),
                8192,
            )?) {
                Ok(output) if output.code == 0 && !output.stdout.is_empty() => format!(
                    "guest diagnostic prefix (at most 8192 bytes): {}",
                    aws.environment
                        .redact(&String::from_utf8_lossy(&output.stdout))
                ),
                Ok(output) => format!(
                    "guest diagnostics unavailable: read exited {} or empty",
                    output.code
                ),
                Err(error) => format!("guest diagnostics unavailable: {error}"),
            }
        };
        return Err(format!("{primary}; {diagnostic}"));
    }
    let deadline = work_deadline.min(commands.elapsed() + Duration::from_secs(60));
    for _ in 0..2 {
        let remaining = deadline.saturating_sub(commands.elapsed());
        if remaining.is_zero() {
            break;
        }
        match commands.run(spec(
            config,
            host,
            &aws.environment,
            "cat /home/ag4/run/capture.json",
            remaining,
            crate::chromium::ingest::MAX_CAPTURE_BYTES,
        )?) {
            Ok(o) if o.code == 0 => return Ok(o.stdout),
            Err(e) if e.cleanup.is_some() => return Err(e.to_string()),
            _ => {}
        }
    }
    Err("capture retrieval failed".into())
}

#[cfg(test)]
mod tests {
    // Explicit native qualification only: the source-stamped production helper
    // execs a test-only argv-checking stream server in place of AWS. This proves
    // SSH/key/upload/quoting/ownership behavior, not corporate EICE connectivity.
    #[test]
    #[ignore = "requires pinned OpenSSH client/server and a source-stamped controller; no AWS"]
    fn native_eice_stream_proxy() {
        use crate::aws::{command::Owner, eice};
        use std::os::unix::{fs::PermissionsExt, process::CommandExt};
        let case = match std::env::var("BORROWSER_AG4_LOCAL_SSH_CASE") {
            Ok(case) => case,
            Err(_) => {
                for case in [
                    "success",
                    "wrong-host-key",
                    "tunnel-failure",
                    "banner-timeout",
                ] {
                    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            "aws::ssh::tests::native_eice_stream_proxy",
                            "--ignored",
                            "--nocapture",
                            "--test-threads=1",
                        ])
                        .env("BORROWSER_AG4_LOCAL_SSH_CASE", case)
                        .process_group(0)
                        .spawn()
                        .unwrap();
                    let bound = std::time::Instant::now() + Duration::from_secs(30);
                    loop {
                        if let Some(status) = child.try_wait().unwrap() {
                            assert!(status.success(), "{case}");
                            break;
                        }
                        if std::time::Instant::now() >= bound {
                            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                            child.wait().unwrap();
                            panic!("native SSH test exceeded bound: {case}");
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                return;
            }
        };
        let mut owner = Owner::enter().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut host = eice::tests::host(&work);
        let root = work.path().canonicalize().unwrap();
        let ssh = PathBuf::from(std::env::var_os("BORROWSER_AG4_TEST_SSH").unwrap());
        let sshd = PathBuf::from(std::env::var_os("BORROWSER_AG4_TEST_SSHD").unwrap());
        let controller = PathBuf::from(std::env::var_os("BORROWSER_AG4_TEST_CONTROLLER").unwrap());
        let mut c = eice::tests::config();
        c.ssh = ssh.clone();
        c.ssh_keygen = ssh.parent().unwrap().join("ssh-keygen");
        let version = owner.run(Spec::new(&ssh, ["-V"])).unwrap();
        assert!(String::from_utf8_lossy(&version.stderr).starts_with("OpenSSH_10.2p1"));
        host.key = key(&mut owner, &c, &root).unwrap();
        let server_key = root.join("host_key");
        let out = owner
            .run(Spec::new(
                &c.ssh_keygen,
                vec![
                    "-q".into(),
                    "-t".into(),
                    "ed25519".into(),
                    "-N".into(),
                    "".into(),
                    "-f".into(),
                    server_key.clone().into_os_string(),
                ],
            ))
            .unwrap();
        assert_eq!(out.code, 0);
        let public = fs::read_to_string(root.join("host_key.pub")).unwrap();
        host.target.host_key = public
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        fs::write(
            &host.known_hosts,
            format!("{} {}\n", host.target.instance_id, host.target.host_key),
        )
        .unwrap();
        let user = unsafe {
            let p = libc::getpwuid(libc::geteuid());
            assert!(!p.is_null());
            std::ffi::CStr::from_ptr((*p).pw_name)
                .to_str()
                .unwrap()
                .to_owned()
        };
        let server_config = root.join("sshd_config");
        fs::write(&server_config, format!("HostKey {}\nAuthorizedKeysFile {}\nStrictModes yes\nUsePAM no\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\nAllowUsers {user}\nAllowAgentForwarding no\nAllowTcpForwarding no\nX11Forwarding no\nLogLevel ERROR\n", server_key.display(), host.key.with_extension("pub").display())).unwrap();
        let environment = AwsEnvironment::from_lookup(|k| match k {
            "AWS_ACCESS_KEY_ID" => Some("synthetic-access".into()),
            "AWS_SECRET_ACCESS_KEY" => Some("synthetic-secret".into()),
            "AWS_SESSION_TOKEN" => Some("synthetic-token".into()),
            _ => None,
        })
        .unwrap();
        let odd = root.join("quote ' percent %h %d space $HOME");
        fs::create_dir(&odd).unwrap();
        fs::set_permissions(&odd, fs::Permissions::from_mode(0o700)).unwrap();
        let copied_controller = odd.join("controller %h '");
        fs::copy(controller, &copied_controller).unwrap();
        fs::set_permissions(&copied_controller, fs::Permissions::from_mode(0o700)).unwrap();
        {
            let mode = case.as_str();
            let program = root.join(format!("stream-{mode}"));
            let body = match mode {
                "success" | "wrong-host-key" => format!(
                    "exec '{}' -i -e -f '{}'",
                    sshd.display(),
                    server_config.display()
                ),
                "tunnel-failure" => "printf 'proxy connection failure\\n' >&2; exit 23".into(),
                _ => "exec /bin/sleep 30".into(),
            };
            let expected = "--region eu-west-1 --no-cli-pager --no-cli-auto-prompt ec2-instance-connect open-tunnel --instance-connect-endpoint-id eice-0123456789abcdef0 --instance-connect-endpoint-dns-name eice-0123456789abcdef0.b67b86ba.ec2-instance-connect-endpoint.eu-west-1.amazonaws.com --private-ip-address 10.77.0.10 --remote-port 22 --max-tunnel-duration 15";
            fs::write(
                &program,
                format!(
                    "#!/bin/sh\n[ \"$#\" = 16 ] && [ \"$*\" = '{expected}' ] || exit 91\n{body}\n"
                ),
            )
            .unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
            c.aws_cli = program;
            c.aws_cli_identity = Some(eice::cli_identity(&c.aws_cli).unwrap());
            if mode == "wrong-host-key" {
                fs::write(
                    &host.known_hosts,
                    format!(
                        "{} {}\n",
                        host.target.instance_id,
                        eice::tests::host(&work).target.host_key
                    ),
                )
                .unwrap();
            }
            let payload = b"bounded local AG4 artifact transfer\n".repeat(8192);
            let remote = format!("cat > '{}/uploaded'", root.display());
            let mut s = spec(
                &c,
                &host,
                &environment,
                &remote,
                Duration::from_secs(15),
                1024,
            )
            .unwrap();
            // All production SSH options remain intact. Only local sshd's
            // current-UID account replaces ag4; no user is created on this Mac.
            let destination = format!("ag4@{}", host.target.instance_id);
            *s.args
                .iter_mut()
                .find(|v| **v == destination.as_str())
                .unwrap() = format!("{user}@{}", host.target.instance_id).into();
            let descriptor = eice::TunnelDescriptor::create(
                &c,
                &host.endpoint,
                &host.target,
                &odd,
                Duration::from_secs(15),
            )
            .unwrap();
            *s.args
                .iter_mut()
                .find(|v| v.to_string_lossy().starts_with("ProxyCommand="))
                .unwrap() = eice::proxy_command(&copied_controller, &descriptor)
                .unwrap()
                .into();
            s.input = payload.clone();
            let start = std::time::Instant::now();
            let result = owner.run(s);
            let mut poisoned = false;
            let diagnostic = match result {
                Ok(out) if mode == "success" => {
                    assert_eq!(out.code, 0, "{}", String::from_utf8_lossy(&out.stderr));
                    assert_eq!(fs::read(root.join("uploaded")).unwrap(), payload);
                    assert!(out.stdout.is_empty());
                    String::from_utf8(out.stderr).unwrap()
                }
                Ok(out) => {
                    assert_ne!(out.code, 0);
                    String::from_utf8(out.stderr).unwrap()
                }
                Err(e) => {
                    assert_ne!(mode, "success", "{e}");
                    if let Some(cleanup) = &e.cleanup {
                        // A failed SSH and its HUP-terminated proxy can race
                        // Darwin group signaling. EPERM is never cleared by a
                        // later absence observation; this owner stays poisoned.
                        assert!(
                            cleanup.contains("SIGTERM") && cleanup.contains("raw errno=Some(1)"),
                            "{e}"
                        );
                        poisoned = true;
                        assert!(owner.quiescent().is_err());
                        let mut cleanup_command = Spec::new("/bin/sh", ["-c", "exit 0"]);
                        cleanup_command.cleanup = true;
                        assert!(owner.run(cleanup_command).unwrap_err().cleanup.is_some());
                    }
                    e.to_string()
                }
            };
            match mode {
                "wrong-host-key" => assert!(
                    diagnostic.contains("HOST IDENTIFICATION HAS CHANGED"),
                    "{diagnostic}"
                ),
                "tunnel-failure" => assert!(
                    diagnostic.contains("proxy connection failure"),
                    "{diagnostic}"
                ),
                "banner-timeout" => assert!(diagnostic.contains("banner exchange"), "{diagnostic}"),
                _ => (),
            }
            eprintln!(
                "pinned SSH {mode}: elapsed={:?}; ownership_poisoned={poisoned}; transferred={} bytes; {diagnostic}",
                start.elapsed(),
                if mode == "success" { payload.len() } else { 0 }
            );
            if mode == "success" {
                let root_exit = owner
                    .timing
                    .iter()
                    .find(|(name, _)| *name == "root_exit_observed")
                    .unwrap()
                    .1;
                let reaped = owner
                    .timing
                    .iter()
                    .find(|(name, _)| *name == "owner_quiescent")
                    .unwrap()
                    .1;
                eprintln!(
                    "pinned SSH root-exit observation to full owned quiescence: {:?}",
                    reaped.duration_since(root_exit)
                );
            }
            if poisoned {
                assert!(owner.finish().is_err());
                return;
            }
        }
        owner.finish().unwrap();
    }
    use super::*;
    use crate::aws::{artifact, template};
    use std::collections::VecDeque;
    struct Transport {
        responses: VecDeque<Result<Output, Failure>>,
        requests: Vec<String>,
    }
    impl Commands for Transport {
        fn run(&mut self, s: Spec) -> Result<Output, Failure> {
            for setting in [
                "StrictHostKeyChecking=yes",
                "IdentityAgent=none",
                "ProxyJump=none",
                "ControlMaster=no",
            ] {
                assert!(s.args.iter().any(|a| a == setting));
            }
            let proxy = s
                .args
                .iter()
                .position(|a| a.to_string_lossy().starts_with("ProxyCommand="))
                .unwrap();
            let jump = s.args.iter().position(|a| a == "ProxyJump=none").unwrap();
            assert!(
                proxy < jump,
                "OpenSSH first proxy selection must be the fixed helper"
            );
            assert!(s.timeout <= Duration::from_secs(300));
            self.requests
                .push(s.args.last().unwrap().to_string_lossy().into_owned());
            self.responses.pop_front().expect("unexpected SSH attempt")
        }
        fn elapsed(&self) -> Duration {
            Duration::ZERO
        }
        fn pause(&mut self, _: Duration, _: bool) -> Result<(), String> {
            Ok(())
        }
        fn interrupted(&self, _: bool) -> bool {
            false
        }
    }
    fn ok(bytes: Vec<u8>) -> Result<Output, Failure> {
        Ok(Output {
            stdout: bytes,
            stderr: vec![],
            code: 0,
        })
    }
    fn failure() -> Result<Output, Failure> {
        Err(Failure {
            primary: "transport failure".into(),
            cleanup: None,
        })
    }
    fn transport_aws(transport: &mut Transport) -> Aws<'_, Transport> {
        Aws {
            commands: transport,
            executable: "/not-invoked/aws".into(),
            environment: AwsEnvironment::from_lookup(|key| match key {
                "AWS_ACCESS_KEY_ID" => Some("synthetic-access".into()),
                "AWS_SECRET_ACCESS_KEY" => Some("synthetic-secret".into()),
                "AWS_SESSION_TOKEN" => Some("synthetic-token".into()),
                _ => None,
            })
            .unwrap(),
            deadline: Duration::from_secs(1800),
            cleanup: false,
        }
    }
    fn upload_inputs() -> (tempfile::TempDir, Config, Host, Prepared) {
        let work = tempfile::tempdir().unwrap();
        let config = eice::tests::config();
        let host = eice::tests::host(&work);
        let mut identity = artifact::identity();
        identity.platform = "linux-x86_64".into();
        let bytes = b"private artifact input sentinel".to_vec();
        let prepared = Prepared {
            sha256: template::sha256(&bytes),
            bytes,
            identity,
        };
        (work, config, host, prepared)
    }
    #[test]
    fn upload_failure_then_success_is_diagnostic_before_verification() {
        let (_work, config, host, prepared) = upload_inputs();
        let mut transport = Transport {
            requests: vec![],
            responses: VecDeque::from([
                failure(),
                ok(vec![]),
                ok(serde_json::to_vec(&prepared.identity).unwrap()),
                ok(vec![]),
                ok(b"capture bytes".to_vec()),
            ]),
        };
        let mut diagnostics = vec![];
        let result = capture(
            &mut transport_aws(&mut transport),
            &config,
            &host,
            &prepared,
            Some("canvas/cascade"),
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(result, b"capture bytes");
        assert!(transport.responses.is_empty());
        assert_eq!(transport.requests.len(), 5);
        assert_eq!(transport.requests[0], transport.requests[1]);
        assert!(transport.requests[0].contains("cat > /home/ag4/run/harness.part"));
        assert!(transport.requests[2].contains("sha256sum --check --strict"));
        assert!(transport.requests[3].contains("guest_capture.py"));
        let diagnostics = String::from_utf8(diagnostics).unwrap();
        assert!(diagnostics.contains("upload attempt 1: result=Failure"));
        assert!(diagnostics.contains("primary=transport failure; cleanup=None"));
        assert!(!diagnostics.contains("upload attempt 2"));
        assert!(!diagnostics.contains("artifact transfer failed"));
    }
    #[test]
    fn both_upload_failures_retain_distinct_outcomes_without_execution() {
        let (_work, config, host, prepared) = upload_inputs();
        let mut transport = Transport {
            requests: vec![],
            responses: VecDeque::from([
                Err(Failure {
                    primary: "input closed; command stderr prefix: first transport detail".into(),
                    cleanup: None,
                }),
                Err(Failure {
                    primary: "command deadline; command stderr prefix: second transport detail"
                        .into(),
                    cleanup: None,
                }),
            ]),
        };
        let mut diagnostics = vec![];
        let error = capture(
            &mut transport_aws(&mut transport),
            &config,
            &host,
            &prepared,
            None,
            &mut diagnostics,
        )
        .unwrap_err();
        assert_eq!(transport.requests.len(), 2);
        assert!(transport.responses.is_empty());
        assert!(transport.requests.iter().all(|r| r.contains("cat >")));
        for (attempt, detail) in [
            (1, "first transport detail"),
            (2, "second transport detail"),
        ] {
            assert!(error.contains(&format!("upload attempt {attempt}: result=Failure")));
            assert!(error.contains(detail));
        }
        assert_eq!(error.matches("ssh_exit=unavailable").count(), 2);
        assert!(diagnostics.is_empty()); // The caller reports the returned failure once.
    }
    #[test]
    fn unresolved_upload_cleanup_aborts_immediately_and_keeps_previous_attempt() {
        let (_work, config, host, prepared) = upload_inputs();
        for fatal_attempt in [1, 2] {
            let mut responses = VecDeque::new();
            if fatal_attempt == 2 {
                responses.push_back(failure());
            }
            responses.push_back(Err(Failure {
                primary: "input broken pipe".into(),
                cleanup: Some("SIGTERM failed: Operation not permitted; raw errno=Some(1)".into()),
            }));
            responses.push_back(ok(vec![])); // Must never consume the next command.
            let mut transport = Transport {
                requests: vec![],
                responses,
            };
            let error = capture(
                &mut transport_aws(&mut transport),
                &config,
                &host,
                &prepared,
                None,
                &mut vec![],
            )
            .unwrap_err();
            assert_eq!(transport.requests.len(), fatal_attempt);
            assert_eq!(transport.responses.len(), 1);
            assert!(error.contains(&format!("upload attempt {fatal_attempt}: result=Failure")));
            assert!(error.contains("primary=input broken pipe"));
            assert!(error.contains(
                "cleanup=Some(SIGTERM failed: Operation not permitted; raw errno=Some(1))"
            ));
            if fatal_attempt == 2 {
                assert!(
                    error.contains("upload attempt 1: result=Failure; primary=transport failure")
                );
            }
        }
    }
    #[test]
    fn nonzero_upload_outputs_retain_exit_codes_and_stderr() {
        let (_work, config, host, prepared) = upload_inputs();
        let mut transport = Transport {
            requests: vec![],
            responses: [(255, "transport detail"), (7, "remote command detail")]
                .into_iter()
                .map(|(code, text)| {
                    Ok(Output {
                        code,
                        stderr: text.as_bytes().to_vec(),
                        stdout: b"never report stdout".to_vec(),
                    })
                })
                .collect(),
        };
        let error = capture(
            &mut transport_aws(&mut transport),
            &config,
            &host,
            &prepared,
            None,
            &mut vec![],
        )
        .unwrap_err();
        assert_eq!(transport.requests.len(), 2);
        assert!(
            error
                .contains("upload attempt 1: result=Output; ssh_exit=255; stderr=transport detail")
        );
        assert!(
            error.contains(
                "upload attempt 2: result=Output; ssh_exit=7; stderr=remote command detail"
            )
        );
        assert!(!error.contains("never report stdout"));
    }
    #[test]
    fn upload_failure_fields_are_bounded_and_credentials_redacted_before_truncation() {
        let mut transport = Transport {
            requests: vec![],
            responses: VecDeque::new(),
        };
        let aws = transport_aws(&mut transport);
        let (detail, unresolved) = upload_failure(
            1,
            Err(Failure {
                primary: format!(
                    "synthetic-access synthetic-secret synthetic-token {}",
                    "é".repeat(20000)
                ),
                cleanup: Some(format!("synthetic-token {}", "界".repeat(20000))),
            }),
            &aws.environment,
        );
        assert!(unresolved);
        assert!(detail.contains("result=Failure; primary=[credential redacted]"));
        assert!(detail.contains("cleanup=Some([credential redacted]"));
        assert_eq!(detail.matches("...[truncated]").count(), 2);
        assert!(detail.len() < UPLOAD_PRIMARY_LIMIT + UPLOAD_CLEANUP_LIMIT + 160);
        for secret in ["synthetic-access", "synthetic-secret", "synthetic-token"] {
            assert!(!detail.contains(secret));
        }
        // A whole credential spanning the truncation point is redacted first.
        let prefix = "x".repeat(UPLOAD_STDERR_LIMIT - 4);
        let text = upload_excerpt(
            &aws.environment,
            &format!("{prefix}synthetic-secret"),
            UPLOAD_STDERR_LIMIT,
        );
        assert!(!text.contains("synt"));
        assert!(text.len() <= UPLOAD_STDERR_LIMIT);
    }
    #[test]
    fn upload_output_diagnostics_are_bounded_and_exclude_inputs_stdout_and_credentials() {
        let (_work, config, host, prepared) = upload_inputs();
        let mut transport = Transport {
            requests: vec![],
            responses: VecDeque::new(),
        };
        for _ in 0..2 {
            let mut stderr = b"synthetic-access synthetic-secret synthetic-token ".to_vec();
            stderr.extend(vec![0xff; 8192]);
            transport.responses.push_back(Ok(Output {
                code: 255,
                stderr,
                stdout: [prepared.bytes.as_slice(), b"PRIVATE KEY MATERIAL SENTINEL"].concat(),
            }));
        }
        let error = capture(
            &mut transport_aws(&mut transport),
            &config,
            &host,
            &prepared,
            None,
            &mut vec![],
        )
        .unwrap_err();
        assert_eq!(transport.requests.len(), 2);
        assert_eq!(error.matches("...[truncated]").count(), 2);
        assert_eq!(error.matches("result=Output").count(), 2);
        assert!(error.len() < 2 * UPLOAD_STDERR_LIMIT + 256);
        for excluded in [
            "private artifact input sentinel",
            "PRIVATE KEY MATERIAL SENTINEL",
            "synthetic-access",
            "synthetic-secret",
            "synthetic-token",
        ] {
            assert!(!error.contains(excluded), "retained {excluded}");
        }
    }
    #[test]
    fn execution_failure_keeps_ssh_and_best_effort_guest_diagnostics_before_cleanup() {
        use crate::aws::{
            capture_then_cleanup,
            state::Creation,
            tests::{Script, aws, record, stack_id, store},
        };
        for available in [true, false] {
            let work = tempfile::tempdir().unwrap();
            let config = eice::tests::config();
            let host = eice::tests::host(&work);
            let mut identity = artifact::identity();
            identity.platform = "linux-x86_64".into();
            let prepared = Prepared {
                bytes: vec![],
                sha256: template::sha256(&[]),
                identity,
            };
            let mut transport = Transport {
                requests: vec![],
                responses: VecDeque::from([
                    ok(vec![]),
                    ok(serde_json::to_vec(&prepared.identity).unwrap()),
                    Ok(Output {
                        stdout: vec![],
                        stderr: b"SSH execution context".to_vec(),
                        code: 7,
                    }),
                    if available {
                        ok(b"guest harness failure".to_vec())
                    } else {
                        failure()
                    },
                ]),
            };
            let (_dir, store) = store();
            let mut r = record();
            r.creation = Creation::Bound;
            r.stack_id = Some(stack_id());
            store.save(&r).unwrap();
            let mut lifecycle = Script::default();
            lifecycle.ready_cleanup();
            let (primary, cleanup) =
                capture_then_cleanup(&mut aws(&mut lifecycle), &mut r, &store, |_, _| {
                    capture(
                        &mut transport_aws(&mut transport),
                        &config,
                        &host,
                        &prepared,
                        None,
                        &mut vec![],
                    )
                    .map(|_| vec![])
                });
            let error = primary.unwrap_err();
            assert!(error.contains("exited 7"));
            assert!(error.contains("SSH execution context"));
            assert!(error.contains(if available {
                "guest harness failure"
            } else {
                "guest diagnostics unavailable"
            }));
            assert!(cleanup.is_ok());
            assert_eq!(
                transport
                    .requests
                    .iter()
                    .filter(|s| s.contains("guest_capture.py"))
                    .count(),
                1
            );
            assert_eq!(
                transport.requests.last().unwrap(),
                "head -c 8192 /home/ag4/run/capture.stderr"
            );
        }
    }
    #[test]
    fn upload_execution_and_retrieval_have_distinct_retry_budgets() {
        let work = tempfile::tempdir().unwrap();
        let config = eice::tests::config();
        let host = eice::tests::host(&work);
        let mut identity = artifact::identity();
        identity.platform = "linux-x86_64".into();
        let prepared = Prepared {
            bytes: vec![1, 2, 3],
            sha256: template::sha256(&[1, 2, 3]),
            identity,
        };
        for phase in ["upload", "identity", "execution", "retrieval", "success"] {
            let mut responses = VecDeque::new();
            if phase == "upload" {
                responses.extend([failure(), failure()]);
            } else {
                responses.push_back(ok(vec![]));
                responses.push_back(if phase == "identity" {
                    ok(b"{}".to_vec())
                } else {
                    ok(serde_json::to_vec(&prepared.identity).unwrap())
                });
                if phase != "identity" {
                    responses.push_back(if phase == "execution" {
                        failure()
                    } else {
                        ok(vec![])
                    });
                }
                if phase == "execution" {
                    responses.push_back(failure());
                }
                if phase == "retrieval" {
                    responses.extend([failure(), failure()]);
                }
                if phase == "success" {
                    responses.push_back(ok(b"existing capture wire bytes".to_vec()));
                }
            }
            let mut transport = Transport {
                responses,
                requests: vec![],
            };
            let result = capture(
                &mut transport_aws(&mut transport),
                &config,
                &host,
                &prepared,
                Some("canvas/cascade"),
                &mut vec![],
            );
            assert_eq!(result.is_ok(), phase == "success");
            assert!(transport.responses.is_empty());
            assert!(
                transport
                    .requests
                    .iter()
                    .filter(|r| r.contains("guest_capture.py"))
                    .count()
                    <= 1
            );
            assert_eq!(
                transport.requests.len(),
                match phase {
                    "upload" => 2,
                    "identity" => 2,
                    "execution" => 4,
                    "retrieval" => 5,
                    _ => 4,
                }
            );
        }
    }
}
