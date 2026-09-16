use crate::{
    Result,
    canonical::{self, Writer},
    distribution::CandidateDistributionManifest,
    error::require,
    identity::{AUTHORITY, BrowserIdentity},
    source_identity as source,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRecord {
    pub format: String,
    pub authority: String,
    pub publisher: String,
    pub release: String,
    pub architecture: String,
    pub upstream_artifact: String,
    pub upstream_sha256: String,
    pub provenance_record_sha256: String,
    pub source_build_provenance: String,
    pub vendor_patches: String,
    pub extraction_record_sha256: String,
    pub frozen_tree_identity: String,
    pub executable_path: String,
    pub browser_identity_sha256: String,
    pub host_record_sha256: String,
    pub distribution_manifest_sha256: String,
    pub review_record_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostRecord {
    pub format: String,
    pub authority: String,
    pub image_identity: String,
    pub image_sha256: String,
    pub snapshot_identity: String,
    pub pretty_name: String,
    pub architecture: String,
    pub kernel_release: String,
    pub uid: u32,
    pub gid: u32,
    pub security_policy_record_sha256: String,
    pub libraries_record_sha256: String,
    pub resources_record_sha256: String,
}
impl HostRecord {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == "borrowser-preparation-host-v1"
                && self.authority == AUTHORITY
                && self.architecture == "x86_64"
                && self.uid != 0
                && self.gid != 0,
            "host identity",
        )?;
        for s in [
            &self.image_identity,
            &self.snapshot_identity,
            &self.pretty_name,
            &self.kernel_release,
        ] {
            canonical::identity(s)?;
        }
        for d in [
            &self.image_sha256,
            &self.security_policy_record_sha256,
            &self.libraries_record_sha256,
            &self.resources_record_sha256,
        ] {
            canonical::digest(d)?;
        }
        Ok(())
    }
    pub fn verify_actual(&self) -> Result<()> {
        self.validate()?;
        #[cfg(target_os = "linux")]
        {
            let os = crate::probe_linux::read_proc("/etc/os-release", 65536)?;
            let pretty = os
                .lines()
                .find_map(|s| s.strip_prefix("PRETTY_NAME="))
                .map(|s| s.trim_matches('"'));
            // procfs reports size zero, so use its bounded stream rather than regular-file length checks.
            let kernel = crate::probe_linux::read_proc("/proc/sys/kernel/osrelease", 4096)?;
            require(
                pretty == Some(self.pretty_name.as_str())
                    && kernel.trim_end() == self.kernel_release
                    && std::env::consts::ARCH == self.architecture
                    && unsafe { libc::getuid() } == self.uid
                    && unsafe { libc::geteuid() } == self.uid
                    && unsafe { libc::getgid() } == self.gid
                    && unsafe { libc::getegid() } == self.gid,
                "actual host mismatch",
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(crate::Error::Unsupported)
        }
    }
}
impl PinRecord {
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == "borrowser-preparation-pin-v1"
                && self.authority == AUTHORITY
                && self.architecture == "x86_64",
            "pin format",
        )?;
        for s in [
            &self.publisher,
            &self.release,
            &self.source_build_provenance,
            &self.vendor_patches,
            &self.frozen_tree_identity,
        ] {
            canonical::identity(s)?;
        }
        require(
            !self.upstream_artifact.is_empty()
                && self.upstream_artifact.len() <= 1024
                && !self.upstream_artifact.chars().any(char::is_control),
            "upstream artifact",
        )?;
        canonical::relative(&self.executable_path)?;
        for d in [
            &self.upstream_sha256,
            &self.provenance_record_sha256,
            &self.extraction_record_sha256,
            &self.browser_identity_sha256,
            &self.host_record_sha256,
            &self.distribution_manifest_sha256,
            &self.review_record_sha256,
        ] {
            canonical::digest(d)?;
        }
        Ok(())
    }
}
pub const ARGUMENTS: [&str; 9] = [
    "--headless=new",
    "--remote-debugging-pipe",
    "--user-data-dir=profile",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--enable-automation",
    "--enable-features=NetworkServiceSandbox",
    "about:blank",
];
pub const DISTRIBUTION_PATH: &str =
    "tools/conformance/static-dom-capture-chromium-linux-v1.distribution.toml";
pub fn produce(
    root: &Path,
    pin: &PinRecord,
    host: &HostRecord,
    browser: &BrowserIdentity,
    manifest: &CandidateDistributionManifest,
) -> Result<Vec<u8>> {
    source::verify_checkout(root)?;
    host.verify_actual()?;
    let bytes = configuration_bytes(pin, host, browser, manifest)?;
    source::verify_checkout(root)?;
    Ok(bytes)
}
/// Serialization is deterministic; successful production additionally requires actual source/host checks.
pub fn configuration_bytes(
    pin: &PinRecord,
    host: &HostRecord,
    b: &BrowserIdentity,
    m: &CandidateDistributionManifest,
) -> Result<Vec<u8>> {
    pin.validate()?;
    host.validate()?;
    b.validate()?;
    let mh = canonical::hash(&m.canonical_bytes()?);
    require(
        mh == pin.distribution_manifest_sha256 && mh == b.distribution_manifest_sha256,
        "manifest binding",
    )?;
    require(
        canonical::hash(&canonical::json(b)?) == pin.browser_identity_sha256
            && canonical::hash(&canonical::json(host)?) == pin.host_record_sha256,
        "record binding",
    )?;
    require(
        m.executable_digest(&pin.executable_path)? == b.executable_sha256,
        "executable binding",
    )?;
    let mut w = Writer::new(65536);
    for (k, v) in [
        ("format", "borrowser-static-dom-capture-chromium-linux-v1"),
        ("capture_mechanism", "borrowser-chromium-cdp-static-dom"),
        ("capture_mechanism_version", "17"),
        ("browser_product", &b.browser_product),
        ("browser_version", &b.browser_version),
    ] {
        w.field(k, v)?;
    }
    if let Some(r) = b.build_revision() {
        w.field("browser_build_revision", r)?;
    }
    for (k, v) in [
        ("browser_executable_path", pin.executable_path.as_str()),
        ("browser_executable_sha256", &b.executable_sha256),
        ("browser_distribution_manifest_path", DISTRIBUTION_PATH),
        ("browser_distribution_sha256", &mh),
        ("platform_os_family", "linux"),
        ("platform_os_version", &host.pretty_name),
        ("platform_architecture", "x86_64"),
        ("kernel_release", &host.kernel_release),
        ("cdp_protocol_version", &b.protocol_version),
        ("cdp_contract", "ag9g-chromium-cdp-static-dom-v5"),
        ("capture_algorithm", "web-observable-dom-tree-v1-inspector"),
        ("capture_algorithm_version", "1"),
        ("capture_algorithm_path", source::INSPECTOR_PATH),
        ("capture_algorithm_source_sha256", source::INSPECTOR_HASH),
        (
            "packaging",
            "web-observable-dom-tree-v1-isolated-expression",
        ),
        ("packaging_version", "1"),
        ("packaging_source_path", source::PACKAGING_PATH),
        ("packaging_source_sha256", source::PACKAGING_HASH),
        ("executed_expression_sha256", source::EXPRESSION_HASH),
        ("collector_source_manifest_path", source::COLLECTOR_PATH),
        ("collector_source_manifest_sha256", source::COLLECTOR_HASH),
        (
            "qualification_suite",
            "ag9g-static-dom-chromium-qualification-v15",
        ),
        ("qualification_manifest_path", source::QUALIFICATION_PATH),
        ("qualification_manifest_sha256", source::QUALIFICATION_HASH),
        (
            "isolation_profile",
            "linux-rootless-user-net-pid-mount-quiescent-proc-v12",
        ),
        ("target_url", "http://ag9g.invalid/fixture.html"),
    ] {
        w.field(k, v)?;
    }
    w.field("response_status", 200u64)?;
    for (k, v) in [
        ("content_type_header", "text/html; charset=utf-8"),
        ("decoding_policy", "utf8-no-bom-v1"),
        (
            "target_parser_input_context",
            "static-text-html-utf8-scripting-disabled-v1",
        ),
        ("resource_network_policy", "offline"),
        ("collection_policy", "ag9g-reviewed-pinned-static-dom"),
        ("collection_policy_version", "1"),
    ] {
        w.field(k, v)?;
    }
    w.field("invocation_arguments", ARGUMENTS)?;
    for (k, v) in [
        ("fixture_max_bytes", 1048576u64),
        ("artifact_max_bytes", 8388608),
        ("protocol_message_max_bytes", 16777216),
        ("protocol_total_max_bytes", 67108864),
        ("outstanding_requests_max", 4),
        ("processed_events_max", 4096),
        ("diagnostic_max_bytes", 262144),
        ("command_timeout_ms", 30000),
        ("attempt_timeout_ms", 120000),
    ] {
        w.field(k, v)?;
    }
    Ok(w.finish())
}
