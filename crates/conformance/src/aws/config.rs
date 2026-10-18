use super::{
    command::{Commands, Spec},
    template,
};
use serde::Deserialize;
use std::{
    ffi::OsString,
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

// No provider discovery: callers authenticate before entering AG4. Kept only in
// memory, without Debug/Serialize, and shared by normal and recovery invocations.
pub(super) struct AwsEnvironment {
    env: Vec<(OsString, OsString)>,
    // Unsetting HOME still lets Python consult the password database. Botocore
    // also searches ~/.aws/models independently of AWS_DATA_PATH. Keep a private
    // empty home alive across version checks and every command in this operation.
    _home: tempfile::TempDir,
}
impl AwsEnvironment {
    pub fn retain_unresolved_home(&mut self) {
        self._home.disable_cleanup(true);
    }
    pub fn load() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var_os(key))
    }
    pub fn from_lookup(mut get: impl FnMut(&str) -> Option<OsString>) -> Result<Self, String> {
        let mut env: Vec<(OsString, OsString)> = [
            ("PATH", "/usr/bin:/bin"),
            ("LC_ALL", "C"),
            ("SHELL", "/bin/sh"),
            ("AWS_CONFIG_FILE", "/dev/null"),
            ("AWS_SHARED_CREDENTIALS_FILE", "/dev/null"),
            ("BOTO_CONFIG", "/dev/null"),
            ("AWS_IGNORE_CONFIGURED_ENDPOINT_URLS", "true"),
            ("AWS_MAX_ATTEMPTS", "1"),
            ("AWS_RETRY_MODE", "standard"),
            ("AWS_EC2_METADATA_DISABLED", "true"),
            ("AWS_PAGER", ""),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        for key in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
        ] {
            match get(key) {
                Some(value)
                    if value.to_str().is_some_and(|s| {
                        !s.is_empty() && s.len() <= 16384 && !s.chars().any(char::is_control)
                    }) =>
                {
                    env.push((key.into(), value))
                }
                None if key == "AWS_SESSION_TOKEN" => {}
                _ => {
                    return Err(format!(
                        "pre-authenticated {key} required (nonempty, bounded UTF-8)"
                    ));
                }
            }
        }
        if let Some(ca) = get("AWS_CA_BUNDLE") {
            let path = PathBuf::from(&ca);
            if !path.is_absolute() {
                return Err("AWS_CA_BUNDLE must be an absolute public CA file".into());
            }
            let bytes = read(&path, 1024 * 1024)?;
            if !bytes
                .windows(27)
                .any(|w| w == b"-----BEGIN CERTIFICATE-----")
                || bytes.windows(11).any(|w| w == b"PRIVATE KEY")
            {
                return Err("AWS_CA_BUNDLE must contain only reviewed public certificates".into());
            }
            env.push(("AWS_CA_BUNDLE".into(), ca));
        }
        let home = tempfile::Builder::new()
            .prefix("borrowser-ag4-aws-home-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .map_err(|e| format!("private AWS home: {e}"))?;
        env.push(("HOME".into(), home.path().as_os_str().to_owned()));
        Ok(Self { env, _home: home })
    }
    pub fn apply(&self, spec: &mut Spec) {
        spec.clear_env = true;
        spec.env = self.env.clone();
    }
    pub fn redact(&self, message: &str) -> String {
        sanitize(&self.env, message.as_bytes(), false)
    }
    pub fn helper_environment() -> Result<Vec<(OsString, OsString)>, String> {
        // The parent owns this HOME. A helper must not create another TempDir
        // that exec would strand, nor read profiles or inherit arbitrary inputs.
        let mut env = Vec::new();
        for (key, expected) in [
            ("PATH", "/usr/bin:/bin"),
            ("LC_ALL", "C"),
            ("SHELL", "/bin/sh"),
            ("AWS_CONFIG_FILE", "/dev/null"),
            ("AWS_SHARED_CREDENTIALS_FILE", "/dev/null"),
            ("BOTO_CONFIG", "/dev/null"),
            ("AWS_IGNORE_CONFIGURED_ENDPOINT_URLS", "true"),
            ("AWS_MAX_ATTEMPTS", "1"),
            ("AWS_RETRY_MODE", "standard"),
            ("AWS_EC2_METADATA_DISABLED", "true"),
            ("AWS_PAGER", ""),
        ] {
            if std::env::var_os(key).as_deref() != Some(std::ffi::OsStr::new(expected)) {
                return Err("EICE helper requires the parent controlled environment".into());
            }
            env.push((key.into(), expected.into()));
        }
        for key in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
        ] {
            match std::env::var_os(key) {
                Some(value)
                    if value.to_str().is_some_and(|s| {
                        !s.is_empty() && s.len() <= 16384 && !s.chars().any(char::is_control)
                    }) =>
                {
                    env.push((key.into(), value))
                }
                None if key == "AWS_SESSION_TOKEN" => (),
                _ => return Err("invalid parent-supplied AWS credentials".into()),
            }
        }
        let home = std::env::var_os("HOME").ok_or("parent AWS home missing")?;
        let path = Path::new(&home);
        let meta = fs::symlink_metadata(path).map_err(|_| "parent AWS home unavailable")?;
        use std::os::unix::fs::MetadataExt;
        if !path.is_absolute()
            || !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.mode() & 0o777 != 0o700
            || meta.uid() != unsafe { libc::geteuid() }
        {
            return Err("invalid private parent AWS home".into());
        }
        env.push(("HOME".into(), home));
        if let Some(ca) = std::env::var_os("AWS_CA_BUNDLE") {
            let path = PathBuf::from(&ca);
            if !path.is_absolute() {
                return Err("invalid parent public CA path".into());
            }
            let bytes = read(&path, 1024 * 1024)?;
            if !bytes
                .windows(27)
                .any(|w| w == b"-----BEGIN CERTIFICATE-----")
                || bytes.windows(11).any(|w| w == b"PRIVATE KEY")
            {
                return Err("invalid parent public CA bundle".into());
            }
            env.push(("AWS_CA_BUNDLE".into(), ca));
        }
        Ok(env)
    }
}

// Concrete AWS/EICE diagnostics only. Never used for captured artifact/stdout
// data. Streams are bounded first; an overflow discards the whole diagnostic
// because its last authentication context may be incomplete.
pub(super) fn sanitize(env: &[(OsString, OsString)], bytes: &[u8], overflow: bool) -> String {
    if overflow || bytes.len() > 256 * 1024 {
        return "[transport diagnostics withheld: retention limit exceeded]".into();
    }
    let secrets: Vec<_> = env
        .iter()
        .filter(|(key, _)| {
            matches!(
                key.to_str(),
                Some("AWS_ACCESS_KEY_ID" | "AWS_SECRET_ACCESS_KEY" | "AWS_SESSION_TOKEN")
            )
        })
        .filter_map(|(_, v)| v.to_str())
        .filter(|s| !s.is_empty())
        .collect();
    fn literals(text: &str, secrets: &[&str]) -> String {
        let mut mask = vec![false; text.len()];
        // CLI formatting may wrap a value between captured chunks or lines.
        // Match known values across ASCII whitespace while retaining original
        // byte offsets, so the whole wrapped value is removed together.
        let positions: Vec<_> = text
            .bytes()
            .enumerate()
            .filter_map(|(i, b)| (!b.is_ascii_whitespace()).then_some(i))
            .collect();
        let compact = positions
            .iter()
            .map(|i| text.as_bytes()[*i])
            .collect::<Vec<_>>();
        let compact =
            std::str::from_utf8(&compact).expect("removing ASCII whitespace preserves UTF-8");
        for secret in secrets {
            for (start, _) in text.match_indices(secret) {
                mask[start..start + secret.len()].fill(true);
            }
            // EOF may split a credential even before the stream reaches its cap.
            for n in (1..secret.len()).rev() {
                if secret.is_char_boundary(n) && text.ends_with(&secret[..n]) {
                    mask[text.len() - n..].fill(true);
                    break;
                }
            }
            let value: String = secret
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect();
            if !value.is_empty() {
                for (start, _) in compact.match_indices(&value) {
                    mask[positions[start]..=positions[start + value.len() - 1]].fill(true);
                }
                for n in (1..value.len()).rev() {
                    if value.is_char_boundary(n) && compact.ends_with(&value[..n]) {
                        mask[positions[compact.len() - n]..].fill(true);
                        break;
                    }
                }
            }
        }
        let mut out = Vec::with_capacity(text.len());
        let mut i = 0;
        while i < text.len() {
            if mask[i] {
                out.extend_from_slice(b"[credential redacted]");
                while i < mask.len() && mask[i] {
                    i += 1;
                }
            } else {
                out.push(text.as_bytes()[i]);
                i += 1;
            }
            if out.len() > 256 * 1024 {
                return "[transport diagnostics withheld: sanitization size limit]".into();
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    let mut text = literals(&String::from_utf8_lossy(bytes), &secrets);
    // URL quoting (including a quoted URL embedded in request context) is
    // decoded only for this bounded diagnostic. Repeated encoding beyond two
    // levels is withheld, not treated as benign arbitrary text.
    for _ in 0..2 {
        let mut decoded = Vec::with_capacity(text.len());
        let mut i = 0;
        let b = text.as_bytes();
        while i < b.len() {
            if b[i] == b'%'
                && i + 2 < b.len()
                && b[i + 1].is_ascii_hexdigit()
                && b[i + 2].is_ascii_hexdigit()
            {
                let h = |v: u8| {
                    if v.is_ascii_digit() {
                        v - b'0'
                    } else {
                        v.to_ascii_lowercase() - b'a' + 10
                    }
                };
                decoded.push(h(b[i + 1]) * 16 + h(b[i + 2]));
                i += 3;
            } else {
                decoded.push(b[i]);
                i += 1;
            }
        }
        text = literals(&String::from_utf8_lossy(&decoded), &secrets);
    }
    if text.contains('%') || text.contains("\\u") || text.contains("\\x") {
        return "[transport diagnostics withheld: ambiguous encoded context]".into();
    }
    // Wrapped authentication field names must not evade line matching. A CLI
    // failure is not a specified HTTP/JSON serialization, so ambiguous wrapping
    // is withheld rather than parsed as a new diagnostic protocol.
    let lower_text = text.to_ascii_lowercase();
    let compact: String = lower_text.chars().filter(|c| !c.is_whitespace()).collect();
    for field in ["x-amz-", "authorization", "credential=", "signature="] {
        if compact.contains(field) && !lower_text.contains(field) {
            return "[transport diagnostics withheld: wrapped authentication context]".into();
        }
    }
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        let auth = [
            "x-amz-",
            "authorization",
            "awsaccesskeyid",
            "aws_access_key_id",
            "aws_secret_access_key",
            "aws_session_token",
            "credential=",
            "credential\"",
            "credential'",
            "signature=",
            "signature\"",
            "signature'",
            "signedheaders=",
        ]
        .iter()
        .filter_map(|s| lower.find(s))
        .min();
        if let Some(at) = auth {
            // Retain benign transport context preceding the URL/header only.
            let start = ["wss://", "https://", "http://"]
                .iter()
                .filter_map(|s| lower[..at].rfind(s))
                .min()
                .unwrap_or(0);
            out.push_str(&line[..start]);
            out.push_str("[AWS authentication context redacted]\n");
            // No trustworthy delimiter proves where a malformed/wrapped URL,
            // header or request context ends. Withhold the remaining bounded
            // diagnostic, including unindented continuation lines. Earlier
            // benign failure context remains useful; safety wins over tail detail.
            break;
        } else {
            out.push_str(line);
        }
    }
    out
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prices {
    pub reviewed_utc: String,
    pub compute_hour_usd_micros: u64,
    pub gp3_gib_month_usd_micros: u64,
    pub ipv4_hour_usd_micros: u64,
    pub egress_gib_usd_micros: u64,
    pub request_allowance_usd_micros: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub account: String,
    pub vpc_id: String,
    pub subnet_id: String,
    pub shared_stack_id: String,
    pub shared_template_sha256: String,
    pub cloudformation_role_arn: String,
    pub aws_cli: PathBuf,
    pub ssh: PathBuf,
    pub ssh_keygen: PathBuf,
    pub python: PathBuf,
    pub node: PathBuf,
    pub cdk_app: PathBuf,
    pub prices: Prices,
    // Parent-qualified AWS executable identity, never accepted from operator JSON.
    #[serde(skip)]
    pub aws_cli_identity: Option<(u64, u64, u64, String)>,
}
pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    if !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("expected regular input file".into());
    }
    let mut bytes = vec![];
    f.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("input size limit exceeded".into());
    }
    Ok(bytes)
}
impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let c: Self = serde_json::from_value(template::json(&read(path, 16384)?)?)
            .map_err(|e| e.to_string())?;
        if c.account.len() != 12
            || !c.account.bytes().all(|b| b.is_ascii_digit())
            || !super::eice::id(&c.vpc_id, "vpc-")
            || !super::eice::id(&c.subnet_id, "subnet-")
            || !c
                .cloudformation_role_arn
                .starts_with(&format!("arn:aws:iam::{}:role/", c.account))
        {
            return Err("invalid account/network/role inputs".into());
        }
        super::eice::shared_authority(&c.account, &c.shared_stack_id, &c.shared_template_sha256)?;
        for p in [
            &c.aws_cli,
            &c.ssh,
            &c.ssh_keygen,
            &c.python,
            &c.node,
            &c.cdk_app,
        ] {
            if !p.is_absolute() || !p.is_file() {
                return Err("absolute existing tool paths required".into());
            }
        }
        c.admit_cost()?;
        Ok(c)
    }
    pub fn admit_cost(&self) -> Result<u64, String> {
        let p = &self.prices;
        if ![
            p.compute_hour_usd_micros,
            p.gp3_gib_month_usd_micros,
            p.ipv4_hour_usd_micros,
            p.egress_gib_usd_micros,
            p.request_allowance_usd_micros,
        ]
        .iter()
        .all(|x| *x > 0 && *x < 100_000_000)
        {
            return Err("positive reviewed regional price inputs required".into());
        }
        // UTC date parsing through a fixed YYYY-MM-DD format; no local timezone.
        let parts: Vec<_> = p.reviewed_utc.split('-').collect();
        if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
            return Err("price review date required".into());
        }
        let year: i32 = parts[0].parse().map_err(|_| "invalid price date")?;
        let month: i32 = parts[1].parse().map_err(|_| "invalid price date")?;
        let day: i32 = parts[2].parse().map_err(|_| "invalid price date")?;
        if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day)
        {
            return Err("invalid price review date".into());
        }
        let mut date = unsafe { std::mem::zeroed::<libc::tm>() };
        date.tm_year = year - 1900;
        date.tm_mon = month - 1;
        date.tm_mday = day;
        let seconds = unsafe { libc::timegm(&mut date) };
        if date.tm_year != year - 1900 || date.tm_mon != month - 1 || date.tm_mday != day {
            return Err("invalid price review date".into());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        // reviewed_utc is date-only: the entire thirtieth UTC calendar day is valid.
        let current_utc_day = now / 86400;
        let reviewed_utc_day = seconds as u64 / 86400;
        if seconds < 0
            || reviewed_utc_day > current_utc_day
            || current_utc_day - reviewed_utc_day > 30
        {
            return Err("regional prices must be reviewed within 30 days".into());
        }
        let estimate = 2
            * (p.compute_hour_usd_micros
                + (16 * p.gp3_gib_month_usd_micros).div_ceil(730)
                + p.ipv4_hour_usd_micros
                + p.egress_gib_usd_micros.div_ceil(4)
                + p.request_allowance_usd_micros);
        if estimate > 500_000 {
            return Err(format!(
                "cost admission refused: estimate {estimate} micro-USD exceeds 500000"
            ));
        }
        Ok(estimate)
    }
    pub fn tools(
        &mut self,
        commands: &mut impl Commands,
        aws_env: &AwsEnvironment,
    ) -> Result<(), String> {
        let pins: serde_json::Value =
            serde_json::from_str(include_str!("../../aws/toolchain.json"))
                .map_err(|e| e.to_string())?;
        let cli_identity = super::eice::cli_identity(&self.aws_cli)?;
        for (path, args, needle) in [
            (
                &self.aws_cli,
                vec!["--version"],
                format!("aws-cli/{} ", pins["aws_cli"].as_str().unwrap()),
            ),
            (
                &self.python,
                vec!["--version"],
                format!("Python {}", pins["python"].as_str().unwrap()),
            ),
            (
                &self.node,
                vec!["--version"],
                format!("v{}", pins["node"].as_str().unwrap()),
            ),
            (
                &self.ssh,
                vec!["-V"],
                format!("OpenSSH_{}", pins["openssh"].as_str().unwrap()),
            ),
        ] {
            let mut spec = Spec::new(path, args);
            if path == &self.aws_cli {
                aws_env.apply(&mut spec);
            }
            let o = commands.run(spec).map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            if o.code != 0 || !text.starts_with(&needle) {
                return Err(format!("toolchain pin mismatch: {}", path.display()));
            }
        }
        if super::eice::cli_identity(&self.aws_cli)? != cli_identity {
            return Err("AWS CLI changed during parent qualification".into());
        }
        self.aws_cli_identity = Some(cli_identity);
        Ok(())
    }
}
