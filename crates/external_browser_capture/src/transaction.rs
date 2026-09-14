//! Internal dedicated-collector transaction. Never call from an aggregate process.
//! One retained verified distribution per bounded workload; one fresh isolated
//! browser per input. Fail-stop can terminate the collector. No AG or corpus ownership.
#![cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "Real capture is Linux-only; deterministic workload tests run on other hosts."
    )
)]
use crate::chromium::{delivery::StaticDomPolicyRejection, session::CaptureOutcome};
use crate::{CaptureError as E, Result, limits::*};
#[cfg(target_os = "linux")]
use crate::{
    chromium::{protocol::PipeTransport, session::PreparedSession},
    configuration::Configuration,
    distribution::{DistributionManifest, VerifiedDistribution},
    isolation::IsolatedBrowser,
    packaging::InspectorExpressionV1,
    source_identity,
};
#[cfg(target_os = "linux")]
use std::path::Path;

/// Construction is confined to checked outer finalization.
pub(crate) struct CompletedWorkload(Vec<CaptureOutcome>);
impl CompletedWorkload {
    pub(crate) fn into_outcomes(self) -> impl ExactSizeIterator<Item = CaptureOutcome> {
        self.0.into_iter()
    }
}

#[cfg(target_os = "linux")]
struct VerifiedCaptureEnvironment {
    configuration: Configuration,
    expression: InspectorExpressionV1,
    distribution: VerifiedDistribution,
}

/// No caller-managed finalizer or exposed browser handles. The sole caller today
/// is qualification running in the dedicated single-threaded collector executable.
#[cfg(target_os = "linux")]
pub(crate) fn capture_static_dom_workload(
    root: &Path,
    configuration: Configuration,
    supplied: &Path,
    inputs: &[&[u8]],
) -> Result<CompletedWorkload> {
    validate_workload(inputs)?;
    crate::isolation::require_supported_host()?;
    let environment = VerifiedCaptureEnvironment::prepare(root, configuration, supplied)?;
    run_environment(environment, inputs, capture_attempt)
}

// Concrete environment ownership with a private attempt seam for Linux object-
// retention/cleanup tests. Production has only the capture_attempt implementation.
#[cfg(target_os = "linux")]
fn run_environment(
    environment: VerifiedCaptureEnvironment,
    inputs: &[&[u8]],
    mut attempt: impl FnMut(&VerifiedCaptureEnvironment, &[u8]) -> Result<CaptureOutcome>,
) -> Result<CompletedWorkload> {
    // Keep the shared snapshot outside the unwind boundary so even a panicking
    // attempt cannot bypass its explicit disposal. Never normalize a panic.
    let work = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_inputs(inputs, |input| attempt(&environment, input))
    }));
    let cleanup = environment.distribution.close();
    finalize_workload(work, cleanup)
}

#[cfg(target_os = "linux")]
impl VerifiedCaptureEnvironment {
    fn prepare(root: &Path, c: Configuration, supplied: &Path) -> Result<Self> {
        source_identity::verify_manifest(
            root,
            &c.collector_source_manifest_path,
            &c.collector_source_manifest_sha256,
            source_identity::SourceSet::Collector,
        )?;
        let expression = InspectorExpressionV1::load(
            root,
            &c.capture_algorithm_source_sha256,
            &c.packaging_source_sha256,
            &c.executed_expression_sha256,
        )?;
        let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map_err(|_| E::UnsupportedHost)?;
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
        Ok(Self {
            configuration: c,
            expression,
            distribution,
        })
    }
}

#[cfg(target_os = "linux")]
fn capture_attempt(
    environment: &VerifiedCaptureEnvironment,
    input: &[u8],
) -> Result<CaptureOutcome> {
    let deadline = crate::deadline::AttemptDeadline::new();
    let watchdog = crate::isolation::AttemptWatchdog::arm(deadline)?;
    let attempt = (|| {
        let mut browser = IsolatedBrowser::launch(
            &environment.configuration,
            &environment.distribution,
            deadline,
        )?;
        let _profile_identity = browser.profile_identity();
        let prepared = (|| {
            let transport = PipeTransport::new(browser.transport()?)?;
            PreparedSession::prepare(transport, &environment.configuration, deadline)
        })();
        let (result, cleanup) = match prepared {
            Ok(mut session) => {
                let result =
                    session.capture(&environment.configuration, input, &environment.expression);
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
        after_cleanup(result, cleanup)
    })();
    let watchdog_cleanup = watchdog.finish();
    after_cleanup(attempt, watchdog_cleanup)
}

fn checked_total(total: usize, additional: usize, maximum: usize) -> Result<usize> {
    let next = total.checked_add(additional).ok_or(E::Limit)?;
    if next > maximum {
        Err(E::Limit)
    } else {
        Ok(next)
    }
}

fn validate_workload(inputs: &[&[u8]]) -> Result<()> {
    if inputs.is_empty() || inputs.len() > WORKLOAD_INPUTS {
        return Err(E::Limit);
    }
    let mut total = 0;
    for input in inputs {
        crate::chromium::delivery::validate_input(input)?;
        total = checked_total(total, input.len(), WORKLOAD_INPUT_BYTES)?;
    }
    Ok(())
}

fn retained_bytes(outcome: &CaptureOutcome) -> Result<usize> {
    let fields: &[&[u8]] = match outcome {
        CaptureOutcome::Observation(o) => {
            if o.bytes.len() > ARTIFACT_BYTES {
                return Err(E::Limit);
            }
            &[&o.bytes, o.document.as_bytes(), o.realm.as_bytes()]
        }
        CaptureOutcome::Rejected(StaticDomPolicyRejection::MetaRefresh {
            frame,
            loader,
            destination,
        }) => &[frame.as_bytes(), loader.as_bytes(), destination.as_bytes()],
        CaptureOutcome::Rejected(StaticDomPolicyRejection::ChildFrame {
            parent,
            loader,
            child,
        }) => &[parent.as_bytes(), loader.as_bytes(), child.as_bytes()],
    };
    fields.iter().try_fold(0, |total, field| {
        checked_total(total, field.len(), WORKLOAD_OUTPUT_BYTES)
    })
}

// A narrow private test seam for workload accounting/order; production always
// supplies capture_attempt borrowing the ONE retained environment. Not a backend API.
fn run_inputs(
    inputs: &[&[u8]],
    mut attempt: impl FnMut(&[u8]) -> Result<CaptureOutcome>,
) -> Result<Vec<CaptureOutcome>> {
    validate_workload(inputs)?;
    let mut outcomes = Vec::new();
    outcomes
        .try_reserve_exact(inputs.len())
        .map_err(|_| E::Allocation)?;
    let mut total = 0;
    for input in inputs {
        let outcome = attempt(input)?;
        total = checked_total(total, retained_bytes(&outcome)?, WORKLOAD_OUTPUT_BYTES)?;
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

// Each cleanup has already run when this is called. Repeated nesting preserves
// browser < watchdog < outer-distribution precedence without early cleanup skips.
fn after_cleanup<T>(result: Result<T>, cleanup: Result<()>) -> Result<T> {
    cleanup.and(result)
}

fn finalize_workload(
    work: std::thread::Result<Result<Vec<CaptureOutcome>>>,
    cleanup: Result<()>,
) -> Result<CompletedWorkload> {
    match work {
        Ok(result) => after_cleanup(result, cleanup).map(CompletedWorkload),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chromium::session::Observation;

    fn observation(bytes: Vec<u8>) -> CaptureOutcome {
        CaptureOutcome::Observation(Observation {
            bytes,
            denied_requests: 0,
            document: "document".into(),
            realm: "realm".into(),
        })
    }
    fn rejection() -> CaptureOutcome {
        CaptureOutcome::Rejected(StaticDomPolicyRejection::ChildFrame {
            parent: "parent".into(),
            loader: "loader".into(),
            child: "child".into(),
        })
    }

    #[test]
    fn workload_preflight_rejects_before_attempt_and_accepts_full_input_budget() {
        let full = vec![b'x'; FIXTURE_BYTES];
        assert!(validate_workload(&[full.as_slice(); WORKLOAD_INPUTS]).is_ok());
        let oversized = vec![b'x'; FIXTURE_BYTES + 1];
        for inputs in [
            vec![],
            vec![b"".as_slice(); WORKLOAD_INPUTS + 1],
            vec![oversized.as_slice()],
            vec![b"\xef\xbb\xbf".as_slice()],
            vec![b"\xff".as_slice()],
        ] {
            assert!(run_inputs(&inputs, |_| panic!("invalid input reached attempt")).is_err());
        }
        assert_eq!(
            checked_total(WORKLOAD_INPUT_BYTES, 1, WORKLOAD_INPUT_BYTES),
            Err(E::Limit)
        );
        assert_eq!(checked_total(usize::MAX, 1, usize::MAX), Err(E::Limit));
    }

    #[test]
    fn exact_ordered_bytes_and_rejections_share_one_workload() {
        let inputs: &[&[u8]] = &[
            b"<!doctype html>\r\n<p>first",
            "é水🙂".as_bytes(),
            b"<iframe>",
        ];
        let mut seen = Vec::new();
        let result = run_inputs(inputs, |input| {
            seen.push(input.to_vec());
            Ok(if input == b"<iframe>" {
                rejection()
            } else {
                observation(input.to_vec())
            })
        })
        .unwrap();
        let complete = finalize_workload(Ok(Ok(result)), Ok(())).unwrap();
        assert_eq!(complete.into_outcomes().len(), inputs.len());
        assert_eq!(seen, inputs);
        assert_eq!(
            run_inputs(&inputs[..1], |i| Ok(observation(i.to_vec())))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn per_output_and_accumulated_payload_limits_never_return_partial_results() {
        assert_eq!(
            retained_bytes(&observation(vec![0; ARTIFACT_BYTES + 1])),
            Err(E::Limit)
        );
        let exact = observation(vec![0; ARTIFACT_BYTES]);
        assert_eq!(
            retained_bytes(&exact),
            Ok(ARTIFACT_BYTES + "documentrealm".len())
        );
        let mut attempts = 0;
        let result = run_inputs(&[b"".as_slice(); 5], |_| {
            attempts += 1;
            Ok(observation(vec![0; ARTIFACT_BYTES]))
        });
        assert!(matches!(result, Err(E::Limit)));
        // Identity payload also counts: four maximum artifacts already exceed 32 MiB.
        assert_eq!(attempts, 4);
        assert_eq!(retained_bytes(&rejection()), Ok("parentloaderchild".len()));
        assert_eq!(
            checked_total(WORKLOAD_OUTPUT_BYTES - 1, 1, WORKLOAD_OUTPUT_BYTES),
            Ok(WORKLOAD_OUTPUT_BYTES)
        );
    }

    #[test]
    fn infrastructure_failure_stops_workload_and_outer_cleanup_overrides_it() {
        let mut attempts = 0;
        let result = run_inputs(&[b"".as_slice(); 3], |_| {
            attempts += 1;
            if attempts == 1 {
                Ok(rejection())
            } else {
                Err(E::Launch)
            }
        });
        assert_eq!(attempts, 2);
        assert!(matches!(
            finalize_workload(Ok(result), Err(E::Cleanup)),
            Err(E::Cleanup)
        ));
        assert!(matches!(
            finalize_workload(Ok(Ok(vec![rejection()])), Err(E::Cleanup)),
            Err(E::Cleanup)
        ));
    }

    #[test]
    fn all_cleanup_layers_take_precedence_without_erasing_capture_errors_on_success() {
        // Distinct test error values make precedence observable even though real
        // cleanup failures may share CaptureError::Cleanup.
        for capture in [Ok(rejection()), Err(E::Protocol)] {
            let browser = after_cleanup(capture, Err(E::ProcessIdentity));
            let watchdog = after_cleanup(browser, Err(E::Deadline));
            assert!(matches!(
                after_cleanup(watchdog, Err(E::Cleanup)),
                Err(E::Cleanup)
            ));
        }
        assert_eq!(
            after_cleanup::<()>(Err(E::Protocol), Ok(())),
            Err(E::Protocol)
        );
        assert_eq!(
            after_cleanup::<()>(Err(E::Protocol), Err(E::ProcessIdentity)),
            Err(E::ProcessIdentity)
        );
        assert_eq!(
            after_cleanup::<()>(Err(E::ProcessIdentity), Err(E::Deadline)),
            Err(E::Deadline)
        );
    }

    #[test]
    fn panic_is_resumed_after_outer_cleanup_and_never_completed() {
        let mut disposed = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let work = std::panic::catch_unwind(|| -> Result<Vec<CaptureOutcome>> {
                panic!("attempt panic")
            });
            let cleanup = {
                disposed = true;
                Ok(())
            };
            finalize_workload(work, cleanup)
        }));
        assert!(disposed);
        assert!(result.is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod environment_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    // Authored object-lifetime test data, never a Chromium distribution or GO.
    fn environment() -> (tempfile::TempDir, VerifiedCaptureEnvironment) {
        let supplied = tempfile::tempdir().unwrap();
        std::fs::set_permissions(supplied.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let executable = supplied.path().join("chrome");
        std::fs::write(&executable, b"x").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let digest = external_test_provenance::sha256(b"x").to_string();
        let bytes = format!(
            "format = \"borrowser-chromium-distribution-manifest-v1\"\nroot_mode = 493\ndirectories = []\n[[entries]]\npath = \"chrome\"\nkind = \"regular\"\nmode = 493\nexecutable = true\nbyte_length = 1\nsha256 = \"{digest}\"\nfile_capabilities = \"absent\"\n"
        );
        let manifest = DistributionManifest::parse(bytes.as_bytes()).unwrap();
        let distribution =
            VerifiedDistribution::create(&manifest, supplied.path(), "chrome", &digest).unwrap();
        (
            supplied,
            VerifiedCaptureEnvironment {
                configuration: crate::configuration::specimen(),
                expression: crate::packaging::specimen(),
                distribution,
            },
        )
    }

    #[test]
    fn one_real_snapshot_survives_supplied_replacement_across_inputs_then_is_disposed() {
        let (supplied, environment) = environment();
        let root = environment.distribution.root().to_owned();
        let original = std::fs::metadata(environment.distribution.executable()).unwrap();
        let mut calls = 0;
        let completed = run_environment(environment, &[b"first", b"second"], |env, _| {
            calls += 1;
            std::fs::write(supplied.path().join("chrome"), b"changed supplied bytes").unwrap();
            let metadata = std::fs::metadata(env.distribution.executable()).unwrap();
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                (original.dev(), original.ino())
            );
            assert_eq!(std::fs::read(env.distribution.executable()).unwrap(), b"x");
            Ok(CaptureOutcome::Rejected(
                StaticDomPolicyRejection::ChildFrame {
                    parent: "p".into(),
                    loader: "l".into(),
                    child: "c".into(),
                },
            ))
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(completed.into_outcomes().len(), 2);
        assert!(!root.exists());
    }

    #[test]
    fn real_snapshot_is_disposed_after_attempt_error_or_unwind() {
        for panic in [false, true] {
            let (_supplied, environment) = environment();
            let root = environment.distribution.root().to_owned();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_environment(environment, &[b"fixture"], |_, _| {
                    assert!(!panic, "attempt panicked");
                    Err(E::Protocol)
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(matches!(result.unwrap(), Err(E::Protocol)));
            }
            assert!(!root.exists());
        }
    }
}
