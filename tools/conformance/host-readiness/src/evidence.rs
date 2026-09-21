//! Versioned, bounded evidence indexing. Structural validation is not review authority.
use crate::{Result, require};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT: &str = "borrowser-ag9g0a-host-readiness-evidence-v1";
pub const AUTHORITY: &str = "host-readiness-only-not-chromium-qualification-not-mechanism-go";
pub const COLLECTOR: &str = "04d22c360c34c891a66400f31afcf3c4a4146973";
pub const HELPER: &str = "763159c513e5a0d2c68ceff927b702ca4aa97351";
pub const HELPER_DESCRIPTOR_TEST: &str = "probe_linux::tests::retained_worker_survives_path_replacement_and_closes_unrelated_descriptors";
pub const HELPER_NAMESPACE_TEST: &str = "probe_linux::tests::namespace_prerequisites_runtime";
pub const IMMUTABLE_TREE_TEST: &str =
    "distribution::linux::replacement_tests::immutable_tree_rejects_main_and_helper_replacement";
pub const PRIVATE_PROFILE_TEST: &str =
    "profile::private_runtime_tests::private_profile_old_creation_race";
pub const PRIVATE_PROC_TEST: &str =
    "isolation::procfs::tests::browser_visible_private_proc_runtime";
pub const POPULATION_TEST: &str =
    "isolation::population::runtime_tests::quiescent_namespace_population_runtime";
pub const ATTEMPT_FAILURE_TEST: &str =
    "isolation::attempt_failure_runtime_tests::actual_attempt_failure_paths_reap_owned_children";
pub const DESCENDANT_CAPABILITY_TEST: &str =
    "isolation::attempt_failure_runtime_tests::sys_admin_is_local_to_descendant_user_namespace";
pub const BOOTSTRAP_ROLE_TEST: &str =
    "isolation::attempt_failure_runtime_tests::bootstrap_role_uses_live_process_objects";
pub const REWRITTEN_TITLE_TEST: &str =
    "isolation::attempt_failure_runtime_tests::rewritten_title_is_not_exec_argv";
pub const WATCHDOG_TEST: &str =
    "isolation::cleanup_runtime_tests::watchdog_success_failure_and_terminal_expiry";
pub const FROZEN_RUNTIME_TESTS: &[&str] = &[
    IMMUTABLE_TREE_TEST,
    PRIVATE_PROFILE_TEST,
    PRIVATE_PROC_TEST,
    POPULATION_TEST,
    ATTEMPT_FAILURE_TEST,
    DESCENDANT_CAPABILITY_TEST,
    BOOTSTRAP_ROLE_TEST,
    REWRITTEN_TITLE_TEST,
    WATCHDOG_TEST,
];
pub const MANIFEST_BYTES: usize = 4 * 1024 * 1024;
pub const ARTIFACTS: usize = 4096;
pub const DEFINITIONS: usize = 256;
pub const ATTEMPTS: usize = 64;
pub const RESULTS: usize = 4096;
pub const ARTIFACT_BYTES: u64 = 1 << 40;
pub const TOTAL_BYTES: u64 = 16 << 40;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub id: String,
    pub path: String,
    pub byte_length: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub commit: String,
    pub source: String,
    pub lockfile: String,
    pub clean_checkout: String,
    pub review: String,
    pub build: String,
    pub executable: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub helper: SourceIdentity,
    pub collector: SourceIdentity,
    pub host_probes: SourceIdentity,
    pub collector_manifests: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub platform_provenance: String,
    pub image_snapshot: String,
    pub kernel: String,
    pub security: String,
    pub tools: String,
    pub libraries: String,
    pub resources: String,
    pub offline: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofAuthority {
    IndependentHostInventory,
    PinnedHelperDeterministic,
    PinnedHelperNamespaceSmoke,
    HostPrerequisiteOnly,
    FrozenCollectorRuntime,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Property {
    NativePlatform,
    ImageSnapshotKernel,
    UnprivilegedHost,
    SecurityPolicy,
    Resources,
    OfflineExecution,
    FrozenSourcesTools,
    NamespaceMappingsOwnership,
    PrivateProcfs,
    PrivateMountProfile,
    NetworkClosure,
    PrivateIpc,
    PidfdSignalsReaping,
    ProcessInspection,
    SeccompNoNewPrivs,
    RetainedExecutable,
    HelperDescriptorImplementation,
    HelperNamespaceSmoke,
    UtilLinuxDescriptorBoundary,
    CollectorPrelaunch,
    CollectorRuntime,
}
pub const PROPERTIES: &[Property] = &[
    Property::NativePlatform,
    Property::ImageSnapshotKernel,
    Property::UnprivilegedHost,
    Property::SecurityPolicy,
    Property::Resources,
    Property::OfflineExecution,
    Property::FrozenSourcesTools,
    Property::NamespaceMappingsOwnership,
    Property::PrivateProcfs,
    Property::PrivateMountProfile,
    Property::NetworkClosure,
    Property::PrivateIpc,
    Property::PidfdSignalsReaping,
    Property::ProcessInspection,
    Property::SeccompNoNewPrivs,
    Property::RetainedExecutable,
    Property::HelperDescriptorImplementation,
    Property::HelperNamespaceSmoke,
    Property::UtilLinuxDescriptorBoundary,
    Property::CollectorPrelaunch,
    Property::CollectorRuntime,
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestOnlyPrerequisite {
    pub id: String,
    pub frozen_source: String,
    pub source_location: String,
    pub explanation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Requirement {
    Production,
    FrozenRegression {
        test_only: Option<TestOnlyPrerequisite>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeDefinition {
    pub id: String,
    pub authority: ProofAuthority,
    pub gate: u32,
    pub properties: Vec<Property>,
    pub requirement: Requirement,
    pub exact_selection: String,
    pub execution: ExecutionKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExecutionKind {
    Tests { expected_top_level_tests: u32 },
    HostCommand,
    RecordedReview,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExecutionEvidence {
    Tests {
        executed_tests: u32,
        exit_status: u32,
    },
    HostCommand {
        invocations: u32,
        exit_status: u32,
    },
    RecordedReview {
        accepted: bool,
    },
}
impl ExecutionKind {
    fn validate(self) -> Result<()> {
        match self {
            Self::Tests {
                expected_top_level_tests,
            } => require(
                (1..=65536).contains(&expected_top_level_tests),
                "nonzero test count bound",
            ),
            Self::HostCommand | Self::RecordedReview => Ok(()),
        }
    }
    fn accepts(self, actual: ExecutionEvidence) -> bool {
        match (self, actual) {
            (
                Self::Tests {
                    expected_top_level_tests,
                },
                ExecutionEvidence::Tests {
                    executed_tests,
                    exit_status,
                },
            ) => {
                executed_tests > 0 && executed_tests == expected_top_level_tests && exit_status == 0
            }
            (
                Self::HostCommand,
                ExecutionEvidence::HostCommand {
                    invocations,
                    exit_status,
                },
            ) => invocations == 1 && exit_status == 0,
            (Self::RecordedReview, ExecutionEvidence::RecordedReview { accepted }) => accepted,
            _ => false,
        }
    }
}
pub const COLLECTOR_PRELAUNCH: &str = "isolation::linux::prelaunch_cleanup_tests::prelaunch_partial_cleanup_covers_socket_namespace_and_fork_failure";
pub const HELPER_SUITE: &str = "qualification-prep:complete-native-suite";
pub const COLLECTOR_SUITE: &str = "external-browser-capture:non-ignored-library-suite";
pub const PLATFORM_INVENTORY: &str = "host-platform-inventory";
pub const SECURITY_INVENTORY: &str = "host-security-inventory";
pub const TOOLING_INVENTORY: &str = "host-offline-tooling-inventory";

fn route(p: &ProbeDefinition, authority: ProofAuthority, gate: u32, selectors: &[&str]) -> bool {
    p.authority == authority && p.gate == gate && selectors.contains(&p.exact_selection.as_str())
}
impl Property {
    /// Exhaustive reviewed policies: adding a Property requires an explicit decision.
    /// Definition validation separately binds execution kind/count and requirement kind.
    pub fn allows(self, p: &ProbeDefinition) -> bool {
        use crate::report::HostProbe;
        use ProofAuthority::*;
        let host = |probe: HostProbe| {
            route(
                p,
                HostPrerequisiteOnly,
                host_gate(probe),
                &[probe.command()],
            )
        };
        let collector = |selectors: &[&str]| route(p, FrozenCollectorRuntime, 9, selectors);
        let helper = || route(p, PinnedHelperDeterministic, 5, &[HELPER_SUITE]);
        match self {
            Self::NativePlatform => route(p, IndependentHostInventory, 1, &[PLATFORM_INVENTORY]),
            Self::ImageSnapshotKernel => {
                route(p, IndependentHostInventory, 1, &[PLATFORM_INVENTORY])
            }
            Self::UnprivilegedHost => route(p, IndependentHostInventory, 2, &[SECURITY_INVENTORY]),
            Self::SecurityPolicy => route(p, IndependentHostInventory, 2, &[SECURITY_INVENTORY]),
            Self::Resources => route(p, IndependentHostInventory, 2, &[SECURITY_INVENTORY]),
            Self::OfflineExecution => route(p, IndependentHostInventory, 3, &[TOOLING_INVENTORY]),
            Self::FrozenSourcesTools => route(p, IndependentHostInventory, 3, &[TOOLING_INVENTORY]),
            Self::NamespaceMappingsOwnership => {
                host(HostProbe::UnshareFd)
                    || collector(&[
                        PRIVATE_PROC_TEST,
                        ATTEMPT_FAILURE_TEST,
                        DESCENDANT_CAPABILITY_TEST,
                    ])
            }
            Self::PrivateProcfs => {
                host(HostProbe::UnshareFd) || collector(&[PRIVATE_PROC_TEST, ATTEMPT_FAILURE_TEST])
            }
            Self::PrivateMountProfile => collector(&[
                IMMUTABLE_TREE_TEST,
                PRIVATE_PROFILE_TEST,
                ATTEMPT_FAILURE_TEST,
            ]),
            Self::NetworkClosure => {
                host(HostProbe::UnshareFd) || collector(&[ATTEMPT_FAILURE_TEST])
            }
            Self::PrivateIpc => {
                host(HostProbe::UnshareFd) || helper() || collector(&[ATTEMPT_FAILURE_TEST])
            }
            Self::PidfdSignalsReaping => {
                host(HostProbe::ProcessInspection)
                    || helper()
                    || collector(&[
                        COLLECTOR_SUITE,
                        POPULATION_TEST,
                        ATTEMPT_FAILURE_TEST,
                        WATCHDOG_TEST,
                    ])
            }
            Self::ProcessInspection => {
                host(HostProbe::ProcessInspection)
                    || collector(&[
                        PRIVATE_PROC_TEST,
                        POPULATION_TEST,
                        BOOTSTRAP_ROLE_TEST,
                        REWRITTEN_TITLE_TEST,
                    ])
            }
            Self::SeccompNoNewPrivs => host(HostProbe::Seccomp),
            Self::RetainedExecutable => collector(&[COLLECTOR_SUITE, BOOTSTRAP_ROLE_TEST]),
            Self::HelperDescriptorImplementation => {
                helper() || route(p, PinnedHelperDeterministic, 5, &[HELPER_DESCRIPTOR_TEST])
            }
            Self::HelperNamespaceSmoke => {
                route(p, PinnedHelperNamespaceSmoke, 6, &[HELPER_NAMESPACE_TEST])
            }
            Self::UtilLinuxDescriptorBoundary => host(HostProbe::UnshareFd),
            Self::CollectorPrelaunch => route(p, FrozenCollectorRuntime, 8, &[COLLECTOR_PRELAUNCH]),
            Self::CollectorRuntime => collector(&[COLLECTOR_SUITE, ATTEMPT_FAILURE_TEST]),
        }
    }
}
pub const fn host_gate(probe: crate::report::HostProbe) -> u32 {
    match probe {
        crate::report::HostProbe::Seccomp | crate::report::HostProbe::ProcessInspection => 4,
        crate::report::HostProbe::UnshareFd => 7,
    }
}
pub fn host_properties(probe: crate::report::HostProbe) -> &'static [Property] {
    use Property::*;
    match probe {
        crate::report::HostProbe::Seccomp => &[SeccompNoNewPrivs],
        crate::report::HostProbe::UnshareFd => &[
            NamespaceMappingsOwnership,
            PrivateProcfs,
            NetworkClosure,
            PrivateIpc,
            UtilLinuxDescriptorBoundary,
        ],
        crate::report::HostProbe::ProcessInspection => &[PidfdSignalsReaping, ProcessInspection],
    }
}
pub fn regression_properties(selector: &str) -> Option<&'static [Property]> {
    use Property::*;
    Some(match selector {
        IMMUTABLE_TREE_TEST | PRIVATE_PROFILE_TEST => &[PrivateMountProfile],
        PRIVATE_PROC_TEST => &[NamespaceMappingsOwnership, PrivateProcfs, ProcessInspection],
        POPULATION_TEST => &[PidfdSignalsReaping, ProcessInspection],
        ATTEMPT_FAILURE_TEST => &[
            NamespaceMappingsOwnership,
            PrivateProcfs,
            PrivateMountProfile,
            NetworkClosure,
            PrivateIpc,
            PidfdSignalsReaping,
            CollectorRuntime,
        ],
        DESCENDANT_CAPABILITY_TEST => &[NamespaceMappingsOwnership],
        BOOTSTRAP_ROLE_TEST => &[ProcessInspection, RetainedExecutable],
        REWRITTEN_TITLE_TEST => &[ProcessInspection],
        WATCHDOG_TEST => &[PidfdSignalsReaping],
        _ => return None,
    })
}
impl ProbeDefinition {
    fn validate_policy(&self) -> Result<()> {
        use ProofAuthority::*;
        self.execution.validate()?;
        let production = matches!(self.requirement, Requirement::Production);
        let test_one = self.execution
            == ExecutionKind::Tests {
                expected_top_level_tests: 1,
            };
        let tests = matches!(self.execution, ExecutionKind::Tests { .. });
        let selection = self.exact_selection.as_str();
        let valid = match self.authority {
            IndependentHostInventory => {
                production
                    && self.execution == ExecutionKind::RecordedReview
                    && match self.gate {
                        1 => selection == PLATFORM_INVENTORY && !self.properties.is_empty(),
                        2 => selection == SECURITY_INVENTORY && !self.properties.is_empty(),
                        3 => selection == TOOLING_INVENTORY && !self.properties.is_empty(),
                        10 => selection == "evidence-review" && self.properties.is_empty(),
                        11 => selection == "host-ready-review" && self.properties.is_empty(),
                        _ => false,
                    }
            }
            PinnedHelperDeterministic => {
                production
                    && self.gate == 5
                    && ((selection == HELPER_DESCRIPTOR_TEST && test_one)
                        || (selection == HELPER_SUITE && tests))
            }
            PinnedHelperNamespaceSmoke => {
                production && self.gate == 6 && selection == HELPER_NAMESPACE_TEST && test_one
            }
            HostPrerequisiteOnly => {
                let probe = crate::report::HostProbe::parse(selection)?;
                production
                    && self.execution == ExecutionKind::HostCommand
                    && self.gate == host_gate(probe)
                    && self.properties == host_properties(probe)
            }
            FrozenCollectorRuntime => {
                if let Some(properties) = regression_properties(selection) {
                    self.gate == 9
                        && test_one
                        && self.properties == properties
                        && matches!(self.requirement, Requirement::FrozenRegression { .. })
                } else {
                    production
                        && ((selection == COLLECTOR_PRELAUNCH && self.gate == 8 && test_one)
                            || (selection == COLLECTOR_SUITE && self.gate == 9 && tests))
                }
            }
        };
        require(valid, "proof selection/execution policy")?;
        for property in &self.properties {
            require(property.allows(self), "production property proof policy")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlternativeProof {
    pub property: Property,
    pub proof: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Outcome {
    RequiredExecutedPass {
        execution: ExecutionEvidence,
    },
    NotApplicableTestOnlyPrerequisite {
        prerequisite: String,
        measured_absence: String,
        raw_evidence: String,
        alternatives: Vec<AlternativeProof>,
        unavailable_coverage: String,
        review: String,
    },
    Failed {
        reason: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeResult {
    pub probe: String,
    pub execution_ordinal: u32,
    pub command: String,
    pub raw_result: String,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub ordinal: u32,
    pub id: String,
    pub observed_at: Option<String>,
    pub results: Vec<ProbeResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub ordinal: u32,
    pub id: String,
    pub proofs: Vec<String>,
}
pub const GATES: &[&str] = &[
    "environment-native-identity",
    "privilege-security",
    "frozen-offline-tooling",
    "host-runtime-prerequisites",
    "approved-helper-suite",
    "helper-namespace-smoke",
    "util-linux-descriptor-proof",
    "frozen-prelaunch-regression",
    "frozen-runtime-regressions",
    "evidence-review",
    "host-ready-review",
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostReadinessManifest {
    pub format: String,
    pub authority: String,
    pub run_id: String,
    pub sources: Sources,
    pub environment: Environment,
    pub artifacts: Vec<ArtifactRef>,
    pub probes: Vec<ProbeDefinition>,
    pub gates: Vec<Gate>,
    pub attempts: Vec<Attempt>,
    pub selected_attempt: u32,
    pub review: String,
}
pub fn identifier(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .next()
                .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)),
        "identifier",
    )
}
pub fn description(s: &str) -> Result<()> {
    require(
        !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control),
        "bounded description",
    )
}
pub fn hex(s: &str, length: usize) -> Result<()> {
    require(
        s.len() == length
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "lowercase hex digest",
    )
}
pub fn artifact_path(s: &str) -> Result<()> {
    require(
        !s.is_empty()
            && s.len() <= 256
            && s.split('/').all(|c| {
                !c.is_empty()
                    && c.len() <= 64
                    && c != "."
                    && c != ".."
                    && c.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            }),
        "confined artifact path",
    )
}
pub fn total_lengths(lengths: impl IntoIterator<Item = u64>) -> Result<u64> {
    let mut total = 0u64;
    for n in lengths {
        require(n <= ARTIFACT_BYTES, "artifact length bound")?;
        total = total.checked_add(n).ok_or("artifact total overflow")?;
        require(total <= TOTAL_BYTES, "artifact total bound")?;
    }
    Ok(total)
}
fn sorted_unique<T: Ord>(items: impl IntoIterator<Item = T>) -> Result<()> {
    let mut last = None;
    for item in items {
        require(
            last.as_ref().is_none_or(|p| p < &item),
            "duplicate or unordered collection",
        )?;
        last = Some(item);
    }
    Ok(())
}
fn timestamp(s: &str) -> Result<()> {
    // Observational UTC timestamp, no time-based acceptance decisions.
    let b = s.as_bytes();
    require(
        b.len() == 20
            && b[4] == b'-'
            && b[7] == b'-'
            && b[10] == b'T'
            && b[13] == b':'
            && b[16] == b':'
            && b[19] == b'Z',
        "UTC timestamp",
    )?;
    require(
        b.iter()
            .enumerate()
            .all(|(i, c)| [4, 7, 10, 13, 16, 19].contains(&i) || c.is_ascii_digit()),
        "timestamp digits",
    )?;
    let month: u32 = s[5..7].parse()?;
    let day: u32 = s[8..10].parse()?;
    let year: u32 = s[..4].parse()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    require(
        year > 0
            && day > 0
            && day <= days
            && s[11..13].parse::<u32>()? < 24
            && s[14..16].parse::<u32>()? < 60
            && s[17..19].parse::<u32>()? < 60,
        "timestamp range",
    )
}
impl HostReadinessManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= MANIFEST_BYTES, "manifest size")?;
        let m: Self = serde_json::from_slice(bytes)?;
        require(m.canonical()? == bytes, "noncanonical manifest")?;
        Ok(m)
    }
    pub fn canonical(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = serde_json::to_vec(self)?;
        bytes.push(b'\n');
        require(bytes.len() <= MANIFEST_BYTES, "manifest size")?;
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<()> {
        require(
            self.format == FORMAT && self.authority == AUTHORITY,
            "host-only evidence authority",
        )?;
        identifier(&self.run_id)?;
        require(
            !self.artifacts.is_empty() && self.artifacts.len() <= ARTIFACTS,
            "artifact population",
        )?;
        require(
            !self.probes.is_empty() && self.probes.len() <= DEFINITIONS,
            "proof population",
        )?;
        require(
            self.gates.len() == GATES.len() && self.gates.len() <= 32,
            "required gates",
        )?;
        require(
            !self.attempts.is_empty() && self.attempts.len() <= ATTEMPTS,
            "attempt population",
        )?;
        sorted_unique(self.artifacts.iter().map(|a| &a.path))?;
        let mut artifacts = BTreeSet::new();
        for a in &self.artifacts {
            identifier(&a.id)?;
            artifact_path(&a.path)?;
            hex(&a.sha256, 64)?;
            require(
                artifacts.insert(a.id.as_str()),
                "duplicate artifact identifier",
            )?;
        }
        total_lengths(self.artifacts.iter().map(|a| a.byte_length))?;
        let reference = |id: &str| -> Result<()> {
            identifier(id)?;
            require(artifacts.contains(id), "missing artifact reference")
        };
        for source in [
            &self.sources.helper,
            &self.sources.collector,
            &self.sources.host_probes,
        ] {
            hex(&source.commit, 40)?;
            for id in [
                &source.source,
                &source.lockfile,
                &source.clean_checkout,
                &source.review,
                &source.build,
                &source.executable,
            ] {
                reference(id)?;
            }
        }
        require(
            self.sources.helper.commit == HELPER && self.sources.collector.commit == COLLECTOR,
            "frozen source identity",
        )?;
        require(
            self.sources.host_probes.commit != HELPER
                && self.sources.host_probes.commit != COLLECTOR,
            "host support absent from historical commits",
        )?;
        reference(&self.sources.collector_manifests)?;
        let e = &self.environment;
        for id in [
            &e.platform_provenance,
            &e.image_snapshot,
            &e.kernel,
            &e.security,
            &e.tools,
            &e.libraries,
            &e.resources,
            &e.offline,
            &self.review,
        ] {
            reference(id)?;
        }
        sorted_unique(self.probes.iter().map(|p| &p.id))?;
        let definitions: BTreeMap<_, _> = self.probes.iter().map(|p| (p.id.as_str(), p)).collect();
        for p in &self.probes {
            identifier(&p.id)?;
            description(&p.exact_selection)?;
            require((1..=11).contains(&p.gate), "proof gate/test bounds")?;
            require(
                p.properties.len() <= PROPERTIES.len(),
                "property population",
            )?;
            sorted_unique(p.properties.iter())?;
            if let Requirement::FrozenRegression { test_only } = &p.requirement {
                require(
                    !p.properties.is_empty(),
                    "regression production-property mapping required",
                )?;
                require(
                    p.authority == ProofAuthority::FrozenCollectorRuntime,
                    "regression authority",
                )?;
                if let Some(t) = test_only {
                    identifier(&t.id)?;
                    reference(&t.frozen_source)?;
                    require(
                        t.frozen_source == self.sources.collector.source,
                        "test-only citation must bind frozen collector source artifact",
                    )?;
                    description(&t.source_location)?;
                    description(&t.explanation)?;
                }
            }
            p.validate_policy()?;
        }
        // A reviewed proof mapping cannot omit the issue's mandated test inventory.
        for selection in FROZEN_RUNTIME_TESTS {
            require(
                self.probes
                    .iter()
                    .filter(|p| {
                        p.exact_selection == *selection
                            && p.authority == ProofAuthority::FrozenCollectorRuntime
                            && p.gate == 9
                            && p.execution
                                == (ExecutionKind::Tests {
                                    expected_top_level_tests: 1,
                                })
                            && matches!(p.requirement, Requirement::FrozenRegression { .. })
                    })
                    .count()
                    == 1,
                "missing/duplicate frozen runtime regression",
            )?;
        }
        for (selection, authority, gate) in [
            (
                HELPER_DESCRIPTOR_TEST,
                ProofAuthority::PinnedHelperDeterministic,
                5,
            ),
            (
                HELPER_NAMESPACE_TEST,
                ProofAuthority::PinnedHelperNamespaceSmoke,
                6,
            ),
        ] {
            require(
                self.probes
                    .iter()
                    .filter(|p| {
                        p.exact_selection == selection
                            && p.authority == authority
                            && p.gate == gate
                            && p.execution
                                == (ExecutionKind::Tests {
                                    expected_top_level_tests: 1,
                                })
                            && matches!(p.requirement, Requirement::Production)
                    })
                    .count()
                    == 1,
                "missing pinned helper implementation/smoke test",
            )?;
        }
        for (selection, authority, gate) in [
            (
                "qualification-prep:complete-native-suite",
                ProofAuthority::PinnedHelperDeterministic,
                5,
            ),
            (
                "external-browser-capture:non-ignored-library-suite",
                ProofAuthority::FrozenCollectorRuntime,
                9,
            ),
        ] {
            require(
                self.probes
                    .iter()
                    .filter(|p| {
                        p.exact_selection == selection
                            && p.authority == authority
                            && p.gate == gate
                            && matches!(
                                p.execution,
                                ExecutionKind::Tests {
                                    expected_top_level_tests: 1..=65536
                                }
                            )
                            && matches!(p.requirement, Requirement::Production)
                    })
                    .count()
                    == 1,
                "missing complete native suite",
            )?;
        }
        for probe in crate::report::HostProbe::ALL {
            require(
                self.probes
                    .iter()
                    .filter(|p| p.exact_selection == probe.command())
                    .count()
                    == 1,
                "missing/duplicate mandatory host command",
            )?;
        }
        let mut all_proofs = BTreeSet::new();
        for (index, gate) in self.gates.iter().enumerate() {
            require(
                gate.ordinal as usize == index + 1 && gate.id == GATES[index],
                "gate identity/order",
            )?;
            require(
                !gate.proofs.is_empty() && gate.proofs.len() <= DEFINITIONS,
                "gate proof population",
            )?;
            sorted_unique(gate.proofs.iter())?;
            for id in &gate.proofs {
                let p = definitions.get(id.as_str()).ok_or("unknown gate proof")?;
                require(
                    p.gate == gate.ordinal && all_proofs.insert(id.as_str()),
                    "gate proof binding",
                )?;
            }
        }
        require(all_proofs.len() == self.probes.len(), "ungated proof")?;
        sorted_unique(self.attempts.iter().map(|a| a.ordinal))?;
        let mut attempt_ids = BTreeSet::new();
        let mut result_count = 0usize;
        for attempt in &self.attempts {
            identifier(&attempt.id)?;
            require(
                attempt.ordinal > 0 && attempt_ids.insert(&attempt.id),
                "attempt identity",
            )?;
            if let Some(t) = &attempt.observed_at {
                timestamp(t)?;
            }
            result_count = result_count
                .checked_add(attempt.results.len())
                .ok_or("result overflow")?;
            require(result_count <= RESULTS, "result population")?;
            sorted_unique(attempt.results.iter().map(|r| &r.probe))?;
            let mut execution = BTreeSet::new();
            for r in &attempt.results {
                require(
                    r.execution_ordinal > 0 && execution.insert(r.execution_ordinal),
                    "execution ordinal",
                )?;
            }
            let mut ordered = attempt.results.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|r| r.execution_ordinal);
            let mut previous_gate = 0;
            let mut failure = false;
            for r in ordered {
                let p = definitions
                    .get(r.probe.as_str())
                    .ok_or("unknown result proof")?;
                require(
                    p.gate >= previous_gate && !failure,
                    "execution crossed failed or out-of-order gate",
                )?;
                previous_gate = p.gate;
                failure = matches!(r.outcome, Outcome::Failed { .. });
            }
            for r in &attempt.results {
                let p = definitions
                    .get(r.probe.as_str())
                    .ok_or("unknown result proof")?;
                reference(&r.command)?;
                reference(&r.raw_result)?;
                match &r.outcome {
                    Outcome::RequiredExecutedPass { execution } => require(
                        p.execution.accepts(*execution),
                        "execution kind/count/status",
                    )?,
                    Outcome::Failed { reason } => description(reason)?,
                    Outcome::NotApplicableTestOnlyPrerequisite {
                        prerequisite,
                        measured_absence,
                        raw_evidence,
                        alternatives,
                        unavailable_coverage,
                        review,
                    } => {
                        let Requirement::FrozenRegression { test_only: Some(t) } = &p.requirement
                        else {
                            return Err("production prerequisite cannot be not applicable".into());
                        };
                        require(prerequisite == &t.id, "test-only prerequisite binding")?;
                        for id in [measured_absence, raw_evidence, review] {
                            reference(id)?;
                        }
                        description(unavailable_coverage)?;
                        require(
                            alternatives.len() == p.properties.len(),
                            "incomplete alternative properties",
                        )?;
                        sorted_unique(alternatives.iter().map(|a| a.property))?;
                        for (a, property) in alternatives.iter().zip(&p.properties) {
                            require(
                                a.property == *property && a.proof != p.id,
                                "alternative property binding",
                            )?;
                            let other = definitions
                                .get(a.proof.as_str())
                                .ok_or("unknown alternative proof")?;
                            require(
                                other.properties.contains(property),
                                "alternative property absent",
                            )?;
                            require(
                                attempt.results.iter().any(|r| {
                                    r.probe == a.proof
                                        && matches!(r.outcome, Outcome::RequiredExecutedPass { .. })
                                }),
                                "alternative must actually pass in same attempt",
                            )?;
                        }
                    }
                }
            }
        }
        let selected = self
            .attempts
            .iter()
            .find(|a| a.ordinal == self.selected_attempt)
            .ok_or("selected attempt missing")?;
        require(
            self.attempts
                .last()
                .is_some_and(|a| a.ordinal == self.selected_attempt),
            "cannot select earlier attempt over later failure",
        )?;
        require(
            selected.results.len() == self.probes.len(),
            "incomplete selected attempt",
        )?;
        require(
            selected
                .results
                .iter()
                .all(|r| !matches!(r.outcome, Outcome::Failed { .. })),
            "failed required gate",
        )?;
        let proved: BTreeSet<_> = selected
            .results
            .iter()
            .filter(|r| matches!(r.outcome, Outcome::RequiredExecutedPass { .. }))
            .flat_map(|r| definitions[r.probe.as_str()].properties.iter().copied())
            .collect();
        require(
            PROPERTIES.iter().all(|p| proved.contains(p)),
            "missing production property",
        )?;
        Ok(())
    }
}
