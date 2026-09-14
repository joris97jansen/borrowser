//! Mechanism feasibility only. No collection/admission evidence or AG registry writes.
use crate::{CaptureError as E, Result, configuration::Configuration, source_identity};
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
/// Dedicated single-threaded collector entry point. Fork-based isolation and
/// watchdog/terminal fail-stop may terminate this process. Never invoke from an
/// aggregate runner or multithreaded test harness; execute conformance-capture.
pub fn mechanism(
    root: &Path,
    configuration_path: &str,
    distribution: &Path,
) -> Result<MechanismQualification> {
    crate::isolation::require_supported_host()?;
    let c = Configuration::load(root, configuration_path)?;
    source_identity::verify_manifest(
        root,
        &c.qualification_manifest_path,
        &c.qualification_manifest_sha256,
        source_identity::SourceSet::Qualification,
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
        run_linux(root, c, distribution)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (c, distribution);
        Err(E::UnsupportedHost)
    }
}

#[cfg(target_os = "linux")]
fn run_linux(root: &Path, c: Configuration, supplied: &Path) -> Result<MechanismQualification> {
    use std::io::Read;
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
        size = size.checked_add(n as u64).ok_or(E::Limit)?;
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
    let configuration = c.sha256()?;
    let mut vectors = Vec::new();
    vectors
        .try_reserve_exact(VECTORS.len())
        .map_err(|_| E::Allocation)?;
    vectors.extend(
        VECTORS
            .into_iter()
            .filter(|(name, _)| name.ends_with(".html")),
    );
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(vectors.len())
        .map_err(|_| E::Allocation)?;
    inputs.extend(vectors.iter().map(|(_, bytes)| *bytes));
    let workload = crate::transaction::capture_static_dom_workload(root, c, supplied, &inputs)?;
    let outcomes = workload.into_outcomes();
    if outcomes.len() != vectors.len() {
        return Err(E::Qualification);
    }
    let mut observations = Vec::new();
    observations
        .try_reserve_exact(vectors.len())
        .map_err(|_| E::Allocation)?;
    for ((name, _), outcome) in vectors.into_iter().zip(outcomes) {
        let mut retained_name = String::new();
        retained_name
            .try_reserve_exact(name.len())
            .map_err(|_| E::Allocation)?;
        retained_name.push_str(name);
        observations.push((retained_name, qualify_outcome(name, Ok(outcome))?));
    }
    Ok(finalize_mechanism(configuration, collector, observations))
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
