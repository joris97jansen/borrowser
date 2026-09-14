use crate::{CaptureError as E, Result, limits::*, wire};
use external_test_provenance::sha256;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FORMAT: &str = "borrowser-static-dom-capture-chromium-linux-v1";

macro_rules! fields {
    ($($(#[$meta:meta])* $name:ident: $ty:ty),* $(,)?) => {
        #[derive(Clone, Debug, Deserialize, Serialize)]
        #[cfg_attr(test, derive(Default))]
        #[serde(deny_unknown_fields)]
        pub struct Configuration { $($(#[$meta])* pub(crate) $name: $ty,)* }
        impl Configuration {
            pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
                let mut w = wire::Writer::new(CONFIG_BYTES);
                $(if stringify!($name) != "browser_build_revision" || self.browser_build_revision.is_some() {
                    w.field(stringify!($name), &self.$name)?;
                })*
                Ok(w.finish())
            }
        }
    }
}
fields! {
    format: String,
    capture_mechanism: String,
    capture_mechanism_version: String,
    browser_product: String,
    browser_version: String,
    browser_build_revision: Option<String>,
    browser_executable_path: String,
    browser_executable_sha256: String,
    browser_distribution_manifest_path: String,
    browser_distribution_sha256: String,
    platform_os_family: String,
    platform_os_version: String,
    platform_architecture: String,
    kernel_release: String,
    cdp_protocol_version: String,
    cdp_contract: String,
    capture_algorithm: String,
    capture_algorithm_version: String,
    capture_algorithm_path: String,
    capture_algorithm_source_sha256: String,
    packaging: String,
    packaging_version: String,
    packaging_source_path: String,
    packaging_source_sha256: String,
    executed_expression_sha256: String,
    collector_source_manifest_path: String,
    collector_source_manifest_sha256: String,
    qualification_suite: String,
    qualification_manifest_path: String,
    qualification_manifest_sha256: String,
    isolation_profile: String,
    target_url: String,
    response_status: u64,
    content_type_header: String,
    decoding_policy: String,
    target_parser_input_context: String,
    resource_network_policy: String,
    collection_policy: String,
    collection_policy_version: String,
    #[serde(deserialize_with="wire::arguments")]
    invocation_arguments: Vec<String>,
    fixture_max_bytes: u64,
    artifact_max_bytes: u64,
    protocol_message_max_bytes: u64,
    protocol_total_max_bytes: u64,
    outstanding_requests_max: u64,
    processed_events_max: u64,
    diagnostic_max_bytes: u64,
    command_timeout_ms: u64,
    attempt_timeout_ms: u64,
}
impl Configuration {
    pub fn load(root: &Path, path: &str) -> Result<Self> {
        Self::parse(&wire::read(root, path, CONFIG_BYTES)?)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let c: Self = wire::parse(bytes, CONFIG_BYTES)?;
        c.validate()?;
        if c.canonical_bytes()? != bytes {
            return Err(E::NonCanonical);
        }
        Ok(c)
    }
    pub fn sha256(&self) -> Result<String> {
        Ok(sha256(&self.canonical_bytes()?).to_string())
    }
    fn validate(&self) -> Result<()> {
        for (actual, expected) in [
            (&self.format, FORMAT),
            (&self.capture_mechanism, "borrowser-chromium-cdp-static-dom"),
            (&self.capture_mechanism_version, "17"),
            (&self.platform_os_family, "linux"),
            (&self.cdp_contract, "ag9g-chromium-cdp-static-dom-v5"),
            (
                &self.capture_algorithm,
                "web-observable-dom-tree-v1-inspector",
            ),
            (&self.capture_algorithm_version, "1"),
            (
                &self.capture_algorithm_path,
                crate::packaging::INSPECTOR_PATH,
            ),
            (&self.packaging, crate::packaging::FORMAT),
            (&self.packaging_version, "1"),
            (&self.packaging_source_path, crate::packaging::PACKAGER_PATH),
            (
                &self.qualification_suite,
                "ag9g-static-dom-chromium-qualification-v15",
            ),
            (
                &self.isolation_profile,
                "linux-rootless-user-net-pid-mount-quiescent-proc-v12",
            ),
            (&self.target_url, "http://ag9g.invalid/fixture.html"),
            (&self.content_type_header, "text/html; charset=utf-8"),
            (&self.decoding_policy, "utf8-no-bom-v1"),
            (
                &self.target_parser_input_context,
                "static-text-html-utf8-scripting-disabled-v1",
            ),
            (&self.resource_network_policy, "offline"),
            (&self.collection_policy, "ag9g-reviewed-pinned-static-dom"),
            (&self.collection_policy_version, "1"),
        ] {
            if actual != expected {
                return Err(E::Field);
            }
        }
        for v in [
            &self.browser_product,
            &self.browser_version,
            &self.platform_os_version,
            &self.platform_architecture,
            &self.kernel_release,
            &self.cdp_protocol_version,
        ] {
            wire::identity(v)?;
        }
        if let Some(v) = &self.browser_build_revision {
            wire::identity(v)?;
        }
        for v in [
            &self.browser_executable_sha256,
            &self.browser_distribution_sha256,
            &self.capture_algorithm_source_sha256,
            &self.packaging_source_sha256,
            &self.executed_expression_sha256,
            &self.collector_source_manifest_sha256,
            &self.qualification_manifest_sha256,
        ] {
            wire::digest(v)?;
        }
        for v in [
            &self.browser_executable_path,
            &self.browser_distribution_manifest_path,
            &self.collector_source_manifest_path,
            &self.qualification_manifest_path,
        ] {
            wire::path(v)?;
        }
        for (a, e) in [
            (self.response_status, 200),
            (self.fixture_max_bytes, FIXTURE_BYTES as u64),
            (self.artifact_max_bytes, ARTIFACT_BYTES as u64),
            (self.protocol_message_max_bytes, MESSAGE_BYTES as u64),
            (self.protocol_total_max_bytes, PROTOCOL_BYTES as u64),
            (self.outstanding_requests_max, 4),
            (self.processed_events_max, EVENTS as u64),
            (self.diagnostic_max_bytes, DIAGNOSTIC_BYTES as u64),
            (self.command_timeout_ms, COMMAND_MS),
            (self.attempt_timeout_ms, ATTEMPT_MS),
        ] {
            if a != e {
                return Err(E::Limit);
            }
        }
        validate_arguments(&self.invocation_arguments)
    }
}

/// V1 has a closed launch vocabulary. Flags are identity-bearing, never a network boundary.
fn validate_arguments(args: &[String]) -> Result<()> {
    const REQUIRED: &[&str] = &[
        "--headless=new",
        "--remote-debugging-pipe",
        "--user-data-dir=profile",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--enable-automation",
        "--enable-features=NetworkServiceSandbox",
    ];
    const OPTIONAL: &[&str] = &[
        "--disable-background-networking",
        "--disable-component-update",
        "--disable-sync",
        "--disable-default-apps",
        "--metrics-recording-only",
        "--password-store=basic",
    ];
    if args.len() > 16 {
        return Err(E::Limit);
    }
    for (i, arg) in args.iter().enumerate() {
        if arg.len() > 1024
            || args[..i].contains(arg)
            || !(REQUIRED.contains(&arg.as_str())
                || OPTIONAL.contains(&arg.as_str())
                || arg == "about:blank")
        {
            return Err(E::Sandbox);
        }
    }
    if REQUIRED.iter().any(|a| !args.iter().any(|b| b == a))
        || args.last().map(String::as_str) != Some("about:blank")
    {
        return Err(E::Sandbox);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn specimen() -> Configuration {
    // Synthetic configuration for contract tests only; never a recognized browser pin.
    let mut c = Configuration {
        format: FORMAT.into(),
        ..Default::default()
    };
    for value in [
        &mut c.browser_product,
        &mut c.browser_version,
        &mut c.platform_os_version,
        &mut c.platform_architecture,
        &mut c.kernel_release,
        &mut c.cdp_protocol_version,
        &mut c.browser_executable_path,
        &mut c.browser_distribution_manifest_path,
        &mut c.collector_source_manifest_path,
        &mut c.qualification_manifest_path,
    ] {
        *value = "x".into();
    }
    c.capture_mechanism = "borrowser-chromium-cdp-static-dom".into();
    c.capture_mechanism_version = "17".into();
    c.platform_os_family = "linux".into();
    c.cdp_contract = "ag9g-chromium-cdp-static-dom-v5".into();
    c.capture_algorithm = "web-observable-dom-tree-v1-inspector".into();
    c.capture_algorithm_version = "1".into();
    c.capture_algorithm_path = crate::packaging::INSPECTOR_PATH.into();
    c.packaging = crate::packaging::FORMAT.into();
    c.packaging_version = "1".into();
    c.packaging_source_path = crate::packaging::PACKAGER_PATH.into();
    c.qualification_suite = "ag9g-static-dom-chromium-qualification-v15".into();
    c.isolation_profile = "linux-rootless-user-net-pid-mount-quiescent-proc-v12".into();
    c.target_url = "http://ag9g.invalid/fixture.html".into();
    c.content_type_header = "text/html; charset=utf-8".into();
    c.decoding_policy = "utf8-no-bom-v1".into();
    c.target_parser_input_context = "static-text-html-utf8-scripting-disabled-v1".into();
    c.resource_network_policy = "offline".into();
    c.collection_policy = "ag9g-reviewed-pinned-static-dom".into();
    c.collection_policy_version = "1".into();
    for v in [
        &mut c.browser_executable_sha256,
        &mut c.browser_distribution_sha256,
        &mut c.capture_algorithm_source_sha256,
        &mut c.packaging_source_sha256,
        &mut c.executed_expression_sha256,
        &mut c.collector_source_manifest_sha256,
        &mut c.qualification_manifest_sha256,
    ] {
        *v = "0".repeat(64);
    }
    c.response_status = 200;
    c.fixture_max_bytes = FIXTURE_BYTES as u64;
    c.artifact_max_bytes = ARTIFACT_BYTES as u64;
    c.protocol_message_max_bytes = MESSAGE_BYTES as u64;
    c.protocol_total_max_bytes = PROTOCOL_BYTES as u64;
    c.outstanding_requests_max = 4;
    c.processed_events_max = EVENTS as u64;
    c.diagnostic_max_bytes = DIAGNOSTIC_BYTES as u64;
    c.command_timeout_ms = COMMAND_MS;
    c.attempt_timeout_ms = ATTEMPT_MS;
    c.invocation_arguments = [
        "--headless=new",
        "--remote-debugging-pipe",
        "--user-data-dir=profile",
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-extensions",
        "--enable-automation",
        "--enable-features=NetworkServiceSandbox",
        "about:blank",
    ]
    .map(String::from)
    .to_vec();
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonicality_schema_and_bounds() {
        let c = specimen();
        let b = c.canonical_bytes().unwrap();
        Configuration::parse(&b).unwrap();
        let s = String::from_utf8(b).unwrap();
        for bad in [
            format!("{s}unknown = 1\n"),
            format!("{s}format = \"x\"\n"),
            s.replace("response_status = 200", "response_status = \"200\""),
        ] {
            assert_eq!(Configuration::parse(bad.as_bytes()).unwrap_err(), E::Schema);
        }
        assert_eq!(
            Configuration::parse(format!("# comment\n{s}").as_bytes()).unwrap_err(),
            E::NonCanonical
        );
        let mut c = specimen();
        c.invocation_arguments.insert(0, "--no-sandbox".into());
        assert_eq!(c.validate(), Err(E::Sandbox));
        c = specimen();
        c.command_timeout_ms += 1;
        assert_eq!(c.validate(), Err(E::Limit));
    }

    #[test]
    fn network_sandbox_feature_is_exact_required_and_identity_bearing() {
        let valid = specimen();
        let bytes = valid.canonical_bytes().unwrap();
        Configuration::parse(&bytes).unwrap();
        let flag = "--enable-features=NetworkServiceSandbox";
        for replacement in [
            None,
            Some("--enable-features=Other"),
            Some("--enable-features=NetworkServiceSandbox,Other"),
            Some("--enable-features=NetworkServiceSandbox<Trial"),
        ] {
            let mut c = valid.clone();
            c.invocation_arguments.retain(|a| a != flag);
            if let Some(value) = replacement {
                c.invocation_arguments.insert(0, value.into());
            }
            assert_eq!(
                Configuration::parse(&c.canonical_bytes().unwrap()).unwrap_err(),
                E::Sandbox
            );
            assert_ne!(c.sha256().unwrap(), valid.sha256().unwrap());
        }
        for extra in [
            flag,
            "--disable-features=NetworkServiceSandbox",
            "--enable-features=NetworkServiceInProcess",
        ] {
            let mut c = valid.clone();
            c.invocation_arguments.insert(0, extra.into());
            assert_eq!(
                Configuration::parse(&c.canonical_bytes().unwrap()).unwrap_err(),
                E::Sandbox
            );
        }
    }
}
