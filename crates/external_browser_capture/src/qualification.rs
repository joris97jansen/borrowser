//! Mechanism feasibility only. No collection/admission evidence or AG registry writes.
use crate::{
    CaptureError as E, Result, configuration::Configuration, packaging::InspectorExpressionV1,
    source_identity,
};
use std::path::Path;

const CORPUS: &str = "tests/contract-vectors/static-dom-capture-qualification-v1";
const VECTORS: [(&str, &[u8]); 6] = [
    (
        "noscript.html",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/noscript.html"
        ),
    ),
    (
        "authored-effects.html",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/authored-effects.html"
        ),
    ),
    (
        "utf8.html",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/utf8.html"
        ),
    ),
    (
        "redirect.html",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/redirect.html"
        ),
    ),
    (
        "child-frame.html",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/child-frame.html"
        ),
    ),
    (
        "external-script.js",
        include_bytes!(
            "../../../tests/contract-vectors/static-dom-capture-qualification-v1/external-script.js"
        ),
    ),
];

pub struct MechanismQualification {
    configuration: String,
    collector: String,
    observations: Vec<(String, String)>,
}
impl MechanismQualification {
    pub fn diagnostic(&self) -> String {
        let mut s = format!(
            "AG9g Stage 0 mechanism feasibility: GO\nAdmission-grade qualification: unavailable\nconfiguration-sha256: {}\ncollector-executable-sha256: {}\n",
            self.configuration, self.collector
        );
        for (name, digest) in &self.observations {
            s.push_str(&format!("{name}: {digest}\n"));
        }
        s
    }
}
pub fn mechanism(
    root: &Path,
    configuration_path: &str,
    distribution: &Path,
) -> Result<MechanismQualification> {
    crate::isolation::require_supported_host()?;
    let c = Configuration::load(root, configuration_path)?;
    source_identity::verify_manifest(
        root,
        &c.collector_source_manifest_path,
        &c.collector_source_manifest_sha256,
        source_identity::SourceSet::Collector,
    )?;
    source_identity::verify_manifest(
        root,
        &c.qualification_manifest_path,
        &c.qualification_manifest_sha256,
        source_identity::SourceSet::Qualification,
    )?;
    let expression = InspectorExpressionV1::load(
        root,
        &c.capture_algorithm_source_sha256,
        &c.packaging_source_sha256,
        &c.executed_expression_sha256,
    )?;
    for (name, expected) in VECTORS {
        if crate::wire::read(
            root,
            &format!("{CORPUS}/{name}"),
            crate::limits::FIXTURE_BYTES,
        )? != expected
        {
            return Err(E::Source);
        }
    }
    #[cfg(target_os = "linux")]
    {
        run_linux(root, c, expression, distribution)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (c, expression, distribution);
        Err(E::UnsupportedHost)
    }
}

#[cfg(target_os = "linux")]
fn run_linux(
    root: &Path,
    c: Configuration,
    expression: InspectorExpressionV1,
    supplied: &Path,
) -> Result<MechanismQualification> {
    use crate::{
        chromium::{protocol::PipeTransport, session::PreparedSession},
        distribution::{DistributionManifest, VerifiedDistribution},
        isolation::IsolatedBrowser,
    };
    use std::io::Read;
    let release =
        std::fs::read_to_string("/proc/sys/kernel/osrelease").map_err(|_| E::UnsupportedHost)?;
    let os = std::fs::read_to_string("/etc/os-release").map_err(|_| E::UnsupportedHost)?;
    let os_id = os
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|s| s.trim_matches('"'))
        .ok_or(E::UnsupportedHost)?;
    if release.trim_end() != c.kernel_release
        || os_id != c.platform_os_version
        || std::env::consts::ARCH != c.platform_architecture
    {
        return Err(E::UnsupportedHost);
    }
    let manifest = DistributionManifest::load(
        root,
        &c.browser_distribution_manifest_path,
        &c.browser_distribution_sha256,
    )?;
    let distribution = VerifiedDistribution::create(
        &manifest,
        supplied,
        &c.browser_executable_path,
        &c.browser_executable_sha256,
    )?;
    // /proc/self/exe is an intentional kernel handle to the running executable, not repository pathname traversal.
    let mut exe = std::fs::File::open("/proc/self/exe").map_err(|_| E::ProcessIdentity)?;
    let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
    let mut chunk = [0u8; 65536];
    let mut size = 0u64;
    loop {
        let n = exe.read(&mut chunk).map_err(|_| E::Read)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > 2 * 1024 * 1024 * 1024 {
            return Err(E::Limit);
        }
        digest.update(&chunk[..n]);
    }
    let collector = digest
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut observations = Vec::new();
    for (name, input) in VECTORS
        .into_iter()
        .filter(|(name, _)| name.ends_with(".html"))
    {
        let deadline = crate::deadline::AttemptDeadline::new();
        let watchdog = crate::isolation::AttemptWatchdog::arm(deadline)?;
        let attempt = (|| {
            let mut browser = IsolatedBrowser::launch(&c, &distribution, deadline)?;
            let _profile_identity = browser.profile_identity();
            let prepared = (|| {
                let transport = PipeTransport::new(browser.transport()?)?;
                PreparedSession::prepare(transport, &c, deadline)
            })();
            let (result, cleanup) = match prepared {
                Ok(mut session) => {
                    let result = session.capture(&c, input, &expression);
                    let cleanup = if result.is_ok() {
                        browser.finish_with_event_check(|| session.verify_quiescent_events())
                    } else {
                        browser.abort()
                    };
                    (result, cleanup)
                }
                Err(e) => (Err(e), browser.abort()),
            };
            // Cleanup failure always takes precedence over the capture failure.
            cleanup?;
            qualify_outcome(name, result)
        })();
        let watchdog_cleanup = watchdog.finish();
        watchdog_cleanup?;
        observations.push((name.into(), attempt?));
    }
    distribution.close()?;
    Ok(finalize_mechanism(c.sha256()?, collector, observations))
}
// Called only after transaction, final live-population validation and cleanup.
// Historical Network Service PIDs are intentionally not qualification inputs.
#[cfg(any(target_os = "linux", test))]
fn finalize_mechanism(
    configuration: String,
    collector: String,
    observations: Vec<(String, String)>,
) -> MechanismQualification {
    MechanismQualification {
        configuration,
        collector,
        observations,
    }
}

#[cfg(any(target_os = "linux", test))]
fn qualify_outcome(
    name: &str,
    result: Result<crate::chromium::session::CaptureOutcome>,
) -> Result<String> {
    use crate::chromium::{delivery::StaticDomPolicyRejection, session::CaptureOutcome};
    match (name, result?) {
        (
            "redirect.html",
            CaptureOutcome::Rejected(StaticDomPolicyRejection::MetaRefresh {
                frame,
                loader,
                destination,
            }),
        ) if !frame.is_empty()
            && !loader.is_empty()
            && destination == "http://ag9g.invalid/redirected.html" =>
        {
            Ok("rejected-as-required".into())
        }
        (
            "child-frame.html",
            CaptureOutcome::Rejected(StaticDomPolicyRejection::ChildFrame {
                parent,
                loader,
                child,
            }),
        ) if !parent.is_empty() && !loader.is_empty() && !child.is_empty() && child != parent => {
            Ok("rejected-as-required".into())
        }
        ("redirect.html" | "child-frame.html", _) | (_, CaptureOutcome::Rejected(_)) => {
            Err(E::Qualification)
        }
        (_, CaptureOutcome::Observation(observation)) => {
            validate_observation(name, &observation.bytes, observation.denied_requests)?;
            if observation.document.is_empty() || observation.realm.is_empty() {
                return Err(E::Qualification);
            }
            Ok(external_test_provenance::sha256(&observation.bytes).to_string())
        }
    }
}
#[cfg(any(target_os = "linux", test))]
fn validate_observation(name: &str, bytes: &[u8], denied: usize) -> Result<()> {
    let output = std::str::from_utf8(bytes).map_err(|_| E::Artifact)?;
    let contains = |line: &str| output.lines().any(|l| l == line);
    let valid = match name {
        "noscript.html" => {
            contains("local-name = \"strong\"") && contains("data = \"NOSCRIPT-PARSED\"")
        }
        "authored-effects.html" => {
            !contains("local-name = \"b\"")
                && !contains("local-name = \"em\"")
                && contains("data = \"STATIC-SENTINEL\"")
                && denied > 0
        }
        "utf8.html" => contains("data = \"é水🙂\""),
        _ => false,
    };
    if valid { Ok(()) } else { Err(E::Qualification) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_sensitive_assertion_is_not_source_text_presence() {
        assert_eq!(
            validate_observation(
                "noscript.html",
                b"data = \"<strong>NOSCRIPT-PARSED</strong>\"\n",
                0
            ),
            Err(E::Qualification)
        );
        assert!(
            validate_observation(
                "noscript.html",
                b"local-name = \"strong\"\ndata = \"NOSCRIPT-PARSED\"\n",
                0
            )
            .is_ok()
        );
    }
    #[test]
    fn authored_effect_and_resource_assertions() {
        let output = b"data = \"STATIC-SENTINEL\"\n";
        assert_eq!(
            validate_observation("authored-effects.html", output, 0),
            Err(E::Qualification)
        );
        assert!(validate_observation("authored-effects.html", output, 1).is_ok());
        assert_eq!(
            validate_observation(
                "authored-effects.html",
                b"data = \"STATIC-SENTINEL\"\nlocal-name = \"b\"\n",
                1
            ),
            Err(E::Qualification)
        );
    }
}

#[cfg(test)]
mod attribution_tests {
    use super::*;
    use crate::chromium::{delivery::StaticDomPolicyRejection as R, session::CaptureOutcome as O};
    fn redirect() -> O {
        O::Rejected(R::MetaRefresh {
            frame: "f".into(),
            loader: "l".into(),
            destination: "http://ag9g.invalid/redirected.html".into(),
        })
    }
    fn child() -> O {
        O::Rejected(R::ChildFrame {
            parent: "f".into(),
            loader: "l".into(),
            child: "child".into(),
        })
    }
    #[test]
    fn exact_rejection_matches_only_its_authored_vector() {
        assert!(qualify_outcome("redirect.html", Ok(redirect())).is_ok());
        assert!(qualify_outcome("child-frame.html", Ok(child())).is_ok());
        assert_eq!(
            qualify_outcome("redirect.html", Ok(child())),
            Err(E::Qualification)
        );
        assert_eq!(
            qualify_outcome("child-frame.html", Ok(redirect())),
            Err(E::Qualification)
        );
    }
    #[test]
    fn infrastructure_and_preparation_errors_are_never_negative_evidence() {
        for name in ["redirect.html", "child-frame.html"] {
            for error in [
                E::UnexpectedEvent,
                E::UnexpectedNavigation,
                E::DocumentIdentity,
                E::Protocol,
                E::ScriptingControl,
                E::Completion,
                E::RealmIdentity,
            ] {
                assert_eq!(qualify_outcome(name, Err(error)), Err(error));
            }
        }
    }
}

#[cfg(test)]
mod completion_tests {
    #[test]
    fn validated_completion_has_no_unconditional_lifetime_rejection() {
        let result = super::finalize_mechanism(
            "config".into(),
            "collector".into(),
            vec![("fixture".into(), "observation".into())],
        );
        assert!(
            result
                .diagnostic()
                .starts_with("AG9g Stage 0 mechanism feasibility: GO\n")
        );
    }
}
