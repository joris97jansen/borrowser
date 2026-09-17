# AG9g0a: native Linux host-readiness prerequisites

Status: **Phase A repository-side support only. No supported host established.**
AG9g0a remains open. No actual host/provider has been selected or invented here.
Chromium pinning, genuine mechanism qualification and mechanism GO remain
outstanding in AG9g0; AG9g1 remains blocked.

## Authority and lifecycle

This contract adds host-prerequisite evidence, never browser qualification,
Chromium sandbox compatibility, capture/admission evidence or mechanism GO.
The frozen collector authority is
`04d22c360c34c891a66400f31afcf3c4a4146973` (mechanism 17, suite v15).
The approved qualification-prep execution baseline is
`763159c513e5a0d2c68ceff927b702ca4aa97351` with an empty source diff.
A commit's existence alone does not confer review approval.

Read [qualification preparation](ag9g0-qualification-preparation.md), the
[helper README](../../tools/conformance/qualification-prep/README.md), and
[the capture contract](ag9g-admitted-static-dom-capture.md) **at the frozen
collector commit**. In particular its rootless isolation profile, private procfs,
V5/closed-role capability policies and mechanism 17 cleanup remain authoritative.
This document does not modify those production predicates.

Phase A implements docs, the separate host-readiness workspace, deterministic
format tests and bounded host probes; runs available development checks and
architecture review; produces the review packet; then stops **before committing**.
The user reviews and commits the implementation. No result produced from dirty or
uncommitted host-readiness source can become qualification evidence.

Phase B starts only with the exact reviewed host-readiness commit and a separately
verified clean checkout. Bind its source, own Cargo.lock, build configuration and
executable digest. Supply and provision one actual environment before recorded
validation. Complete the ordered gates below and review the retained bundle.
The Phase A completion statement is:

> repository-side AG9g0a qualification support implemented; AG9g0a remains open pending committed source freeze and genuine native host validation

## One actual environment, not a theoretical platform

An x86-64 Linux bare-metal host or x86-64 VM/cloud VM executing natively on an
x86-64 CPU/hypervisor may qualify. Cross-architecture ISA emulation/translation
is prohibited, including an x86-64 Linux guest on ARM. Guest `uname -m`, Rust
architecture or CPUID alone is insufficient. Retain independent physical-machine
or provider/platform/hypervisor evidence establishing the underlying ISA and
execution mode. The utility's Linux/x86-64 compile gate is not that evidence.

The external provisioning artifact must identify one tuple, retaining:

- Upstream image publisher/release, immutable locator, SHA-256 and available
  publisher signature/checksum provenance; retained image or immutable retrievable
  artifact. A mutable distribution name plus installation commands is insufficient.
- Provisioned snapshot identity/export/retention, provisioning inputs and logs,
  exact package versions/artifact identities and backing-storage controls.
- Physical host or provider instance/platform identity, architecture and VM
  configuration; exact kernel release/package/build and relevant kernel config.
- Non-root execution account; effective security policy; resources; exact tools,
  runtime loader/libraries, pre-provisioned dependencies and local Git objects.
- A real durable evidence destination meeting the retention requirements below.

Until this tuple exists it is an unmet execution prerequisite. Do not fill records
with placeholder machine identities or treat a candidate image as a passing host.

## Privilege, security and resources

Require nonzero matching real/effective UID/GID. Record saved IDs and supplementary
groups, UID/GID maps, namespace identities/ancestry/ownership, CapEff, CapPrm,
CapInh, CapAmb, CapBnd, NoNewPrivs, seccomp state, active LSM and container policy.
The probe launcher rejects nonzero execution capability sets; a nonempty bounding
set alone is not execution authority and is not universally prohibited.
Independent inventory/review must also exclude privileged containers, host
CAP_SYS_ADMIN, privileged mappings, `--privileged`, root execution, host PID/net
substitution, sandbox disabling and security-policy disabling to obtain a pass.
A nonzero UID inside a privileged container is not sufficient.

The frozen setup may acquire capabilities inside its new user namespace for
private mounts and namespace setup. This confers no ancestor/host authority.
Chromium's specifically reviewed descendant namespace zygote capability rule is
preserved; this issue does not execute or qualify that Chromium behavior.

The existing host gate requires user/PID/network/mount namespaces, current-user
nonzero mappings and setgroups denial, namespace ownership queries, pidfds/signals,
execveat, descriptor filtering, process inspection, private procfs with no inherited
aliases, tmpfs/read-only mounts and nested user-namespace support. Record applicable
namespace sysctls/quotas and AppArmor/SELinux/seccomp/container configuration. There
is no invented universal distro sysctl list, required LSM disablement, or minimum
kernel inferred solely from a version number. Actual runtime operations must work
under the recorded policy.

The disposable seccomp child sets/reads NoNewPrivs=1, installs an x86-64-checked
filter rejecting only getppid with EPERM, observes Seccomp=2 and verifies rejection
against its positive control. It proves kernel/policy support, **not Chromium's
seccomp policy**. The parent remains outside that filter.

Inventory RAM/disk/tmpfs and FD/process limits, cgroup constraints and loader/native
libraries. Account for the frozen up-to-8-GiB distribution staging and per-attempt
copies plus build/browser headroom. Retain actual bounded capacity measurements
against the chosen allocation; an inventory label alone cannot prove capacity.
No browser-specific library compatibility is asserted before browser selection.

## Provisioning versus offline execution

All package/tool/Rust-component/Cargo dependency/Git-object acquisition precedes
snapshot freeze and recorded validation. Rust/Cargo 1.92.0, rustfmt/clippy,
Git >=2.45.0, util-linux/unshare, linker and native build tools must already exist.
The helper additionally requires canonical `git version MAJOR.MINOR.PATCH` output;
vendor suffixes are not accepted. Record versions and executable digests.

Use locked/offline builds with each original lockfile and separate target directory.
Git verification uses the pinned helper's no-lazy-fetch restrictions. Missing
inputs fail; no fetch, hydration, package repair or retry to install dependencies.
Cargo offline mode alone does not constrain every subprocess. Run with an externally
disconnected VM NIC or equivalent evidenced external network disconnection, retaining
platform configuration and in-guest network measurements. Console/local execution
must not depend on a live SSH/network session. This does not grant runtime privilege
or disable guest security policy. Retain all build-affecting configuration/environment.

## Four distinct evidence authorities

| Source | What it proves | What it does not prove |
| --- | --- | --- |
| Pinned helper deterministic tests | WorkerExecutable identity, deliberate FD bridge, CLOEXEC and unrelated-FD closure across test execs | Actual util-linux behavior or complete identity-path qualification |
| Pinned helper ignored namespace smoke | Actual `/usr/bin/unshare ... /bin/true` basic namespace setup | Full worker state or retained-FD transfer |
| New host-only probes | External kernel/util-linux prerequisites on the exact recorded host/binary | An alternate helper worker, collector verifier or Chromium sandbox |
| Frozen collector runtime tests | Collector-owned operations/predicates from the frozen source | Real Chromium qualification |

The final review composes all four. The host utility does not call the helper's
`identity` command or prove its complete execution path. Repeated namespace/syscall
setup is the minimum external fixture, labelled
`host-kernel-util-linux-prerequisite-only`; it is not a second WorkerExecutable,
QuiescentPopulation or process-role implementation.

The util-linux probe requires explicit absolute executable path, SHA-256 and exact
version line. The recorded executable must also match `/usr/bin/unshare` used
by the pinned helper namespace smoke and frozen tests; retain that object/digest
binding in the tooling inventory. It hashes a retained regular no-capability executable and executes
that object. A deliberately inherited fixture executable descriptor traverses the
actual util-linux exec/namespace/re-exec boundary; unrelated descriptors are seeded
and filtered, then audited in the worker. Record namespace identities/ownership,
nonzero maps, PID 1, child-death signal, private procfs identity/flags/propagation,
absence of aliases and initial private process population.

Network proof is actual state, not namespace creation or a failed Internet request:
only **down loopback**, no IPv4 routes and only IPv6 reject routes according to the
frozen collector's stricter predicate, no unexpected TCP/UDP/Unix endpoints or
inherited descriptors, and successful private socketpair IPC. There is no TCP
loopback service, debug listener or host socket forwarding. Matching this fixture
is an external prerequisite observation, not an alternative production predicate.

The process-inspection fixture observes exactly two owned children: one forked from
a worker thread is enumerated, SIGSTOPed through a pidfd and inspected via retained
proc/start/executable objects; a second appears/disappears; both are killed/reaped;
the first retained pidfd remains exited and its old proc stat is unavailable. It
never forces PID reuse or implements convergence/quiescence/Chromium role policy.

Public probes execute in a disposable process group under a 15-second supervisor,
with separately bounded 64-KiB stdout/stderr captures, null stdin,
and inner util-linux deadline 10 seconds, version deadline 2 seconds and process
inspection deadline 4 seconds. Cleanup has a separate 2-second bound. Success is
returned only after checked child completion/reaping. On timeout/error, terminate
owned processes; cleanup failure overrides the operation failure. Drop is emergency
fallback, not evidence of checked cleanup. Preserve diagnostics after abnormal
termination and inspect residual resources independently. No automatic retry.

## Runtime applicability

Every frozen regression has exactly one final classification:

- `required-executed-pass`: exact selected test count and exit status recorded.
- `not-applicable-test-only-prerequisite`: frozen source explicitly says the missing
  prerequisite is test-only, measured absence/raw evidence is retained, every
  production property maps to an actually passed alternative proof in the same
  attempt, unavailable regression coverage is acknowledged and review is retained.
- `failed`: unsuccessful execution; incomplete required proof blocks finalization.

Ignored/skipped output never automatically establishes not-applicable. A production
requirement cannot use that state. Applicability is classified before closeout;
previous failures stay immutable in attempt history. Do not weaken policy to make
an otherwise inapplicable test run.

The frozen contract identifies namespaced `ns_last_pid` as test-only scaffolding.
If unavailable, `quiescent_namespace_population_runtime` can be inapplicable only
with this production-host proof mapping:

| Property | Alternative |
| --- | --- |
| Private procfs/owned descendant visibility | Frozen `browser_visible_private_proc_runtime` |
| Collector launch and terminal teardown/reaping | Frozen `actual_attempt_failure_paths_reap_owned_children` |
| pidfds/signals/exit observation and bounded cleanup | Helper and frozen cleanup runtime tests |
| Executable/process metadata | Frozen retained-object exec and live process-role runtime tests |
| Worker-thread child enumeration, stopping, retained start identity, appearing/disappearing children | New narrow host process-inspection probe |

The frozen launch/failure test's expected-error assertion alone does not prove
all quiescence behavior. Forced PID reuse and the mixed regression's precise
algorithmic scenarios remain unexecuted coverage, not equivalent evidence supplied
by this probe. Any still-unproved required host property blocks closeout. The schema
models applicability generically, without automatically exempting any named test.

## Canonical evidence V1

Format: `borrowser-ag9g0a-host-readiness-evidence-v1`.
Authority: `host-readiness-only-not-chromium-qualification-not-mechanism-go`.
The utility validates a completed authored index plus retained artifacts; it does
not infer provenance, attest signatures, discover a host or confer human approval.
Production property claims and their mapping to raw evidence must be reviewed.

Canonical encoding is compact UTF-8 JSON, no BOM, exactly one final LF. Object fields
follow the Rust schema declaration order (below); serde_json's JSON escaping is
canonical: quote/backslash and controls escaped, other UTF-8 emitted directly.
Integers are unsigned canonical decimals, no fractions/exponents/signs/leading zeros.
Persisted input must equal canonical reserialization byte for byte. Unknown/duplicate
fields, duplicate identifiers/paths/ordinals and out-of-order collections reject.
No arbitrary JSON maps or free-form embedded JSON observations occur in the manifest.

Top-level order: `format`, `authority`, `run_id`, `sources`, `environment`,
`artifacts`, `probes`, `gates`, `attempts`, `selected_attempt`, `review`.
Nested field order/types are the explicit deny-unknown-fields structs in
`tools/conformance/host-readiness/src/evidence.rs`:

- Sources: helper, collector, host_probes, collector_manifests. Each SourceIdentity:
  commit, source, lockfile, clean_checkout, review, build, executable. Source and
  review artifacts must bind actual clean committed checkouts; an authored string
  cannot establish this. Helper/collector commits are checked against frozen constants.
- Environment: platform_provenance, image_snapshot, kernel, security, tools,
  libraries, resources, offline; all are artifact IDs.
- ArtifactRef: id, path, byte_length, sha256.
- ProbeDefinition: id, authority, gate, properties, requirement, exact_selection,
  execution. ExecutionKind is tagged by kind: tests carries expected_top_level_tests;
  host-command and recorded-review carry no count fields. Properties enumerate required host facilities and the
  four evidence authorities. Requirements are production or frozen-regression;
  regressions require a nonempty production-property mapping; only the latter can carry TestOnlyPrerequisite (id, frozen_source,
  source_location, explanation). Its frozen_source must reference the collector
source artifact, not an unrelated note. Exact test-only status is established by review
  of the cited frozen artifact, not an arbitrary exemption name.
- Gate: ordinal, id, proofs. Exactly the eleven gates below, each with nonempty
  proof membership; every proof belongs to exactly one matching gate.
- Attempt: ordinal, id, observed_at, results. ProbeResult: probe, execution_ordinal,
  command, raw_result, outcome. Outcome is tagged by state: passed carries
  execution, a separately typed record tagged by kind. Tests carries
  executed_tests/exit_status; host-command carries invocations/exit_status;
  recorded-review carries accepted. Failed carries reason; not-applicable carries
  prerequisite/measured_absence/raw_evidence/alternatives/unavailable_coverage/review.
  AlternativeProof contains property, proof. Alternatives must actually pass in
  the same attempt; no circular chains of exemptions.

Artifact arrays sort by path; probe definitions/results and gate proof IDs sort
lexically by identifier; gates/attempts sort by ordinal; property/alternative lists
sort by the schema's Property enum declaration order. Execution ordinals are unique
within each attempt and encode nondecreasing gate order separately from canonical
result ordering. No later result may follow a failed result. The selected attempt
must be the latest, complete and free of failures. Earlier failed attempts may be
partial and remain retained. Required production properties must all have actually
passed proofs; not-applicable records do not satisfy the global production set.
The exact frozen prelaunch selector must declare and execute one test. Definitions
must include all nine frozen runtime selectors below, the exact pinned helper
retained-worker and namespace-smoke selectors, and the suite inventory markers
`qualification-prep:complete-native-suite` and
`external-browser-capture:non-ignored-library-suite`. Suite markers refer to the
recorded complete Cargo command/test inventory, not individual Rust test selectors;
expected top-level totals must be nonzero and checked against that inventory.
Native execution and suite completeness still require review of the raw results.

| Bound | Value |
| --- | --- |
| Manifest including LF | 4 MiB |
| Artifacts | 4,096 |
| Gates | maximum 32; V1 requires exactly the eleven defined gates |
| Probe/test definitions | 256 |
| Attempts | 64 |
| Results across attempts | 4,096 |
| Ordinary identifier | 1–128 ASCII bytes, `[a-z0-9][a-z0-9._-]*` |
| Description/selector | 1–1,024 UTF-8 bytes without controls |
| Artifact path | 1–256 ASCII bytes; components 1–64 bytes |
| Artifact SHA-256 | exactly 64 lowercase hexadecimal characters |
| Source commit | exactly 40 lowercase hexadecimal characters |
| Artifact bytes | unsigned u64, at most 1 TiB |
| Aggregate artifact bytes | checked addition, at most 16 TiB |
| Gate/attempt/execution ordinals | unsigned u32, nonzero; gate 1–11 |
| Test counts | unsigned u32, 1–65,536; must match declared nonzero count |
| Host-command invocations | unsigned u32, exactly one |
| Recorded review | accepted must be true; cannot substitute for runtime execution |
| Successful exit_status | unsigned u32, exactly zero |

Array reference populations cannot exceed their definition populations. Optional
observational timestamps use valid Gregorian `YYYY-MM-DDTHH:MM:SSZ` (years 0001–9999,
no leap-second spelling). They are not ordering or trust authority.

Paths consist of `/`-separated `[A-Za-z0-9._-]+` components. No absolute path, empty
component, `.`/`..`, backslash, trailing separator or symlink is accepted. Absolute
machine-local working paths occur only in retained external command artifacts.

The Unix verifier retains the explicit bundle root and traverses each descendant
component with directory-relative no-follow opens. It accepts regular artifacts
only, hashes/counts the same opened file, checks its identity/metadata before and
after reading, and checks final file and ancestor bindings through retained parents.
A pathname is reopened only to compare binding, never as the object to hash.
Missing, malformed, replaced, mutated or mismatched artifacts fail closed. Verification
has a 24-hour overall deadline checked between reads, fixed-size read buffers and
explicit byte bounds. Use responsive controlled local storage; these sequential
checks are not an atomic snapshot against a privileged storage administrator or a
promise to interrupt an unresponsive kernel filesystem syscall.

`verify MANIFEST BUNDLE_ROOT` emits the identical canonical manifest to stdout only
on success; it never rewrites artifacts or publishes files. The caller captures
stdout/stderr and exit status without hiding failures behind pipelines, exclusively
creates destinations and freezes the completed bundle. There is no archive format,
extraction service, replacement of retained artifacts or automatic upload.

## Closed proof policies and execution semantics

Every Property has an exhaustive reviewed authority/gate/selector policy in
`Property::allows`. There is no unconstrained production-property fallback.
Definitions also validate execution kind and requirement kind. Applicable frozen
regressions must name their full reviewed production-property set, so an exemption
cannot omit properties merely to simplify its alternatives. Global completeness
uses only successful results that passed these policies.

The selectors for independent inventory are `host-platform-inventory` (gate 1),
`host-security-inventory` (gate 2), and `host-offline-tooling-inventory` (gate 3).
These are recorded-review records whose retained artifacts include the actual
measurements/commands; they are not zero-test runs. `evidence-review` (gate 10)
and `host-ready-review` (gate 11) are recorded-review-only and carry no production
properties. No inventory or review may claim runtime properties.

In the following complete policy table, I means independent host inventory;
D means pinned helper deterministic evidence; N means pinned helper namespace
smoke; H means host-prerequisite-only; C means frozen collector runtime. The number
is the gate. `helper-suite` and `collector-suite` refer to the two exact suite
markers above. Frozen test short names refer to their exact selectors in the
runtime inventory below, not arbitrary suffix or substring matching.

| Property | Allowed authority/gate/selection |
| --- | --- |
| NativePlatform | I/1/host-platform-inventory |
| ImageSnapshotKernel | I/1/host-platform-inventory |
| UnprivilegedHost | I/2/host-security-inventory |
| SecurityPolicy | I/2/host-security-inventory |
| Resources | I/2/host-security-inventory |
| OfflineExecution | I/3/host-offline-tooling-inventory |
| FrozenSourcesTools | I/3/host-offline-tooling-inventory |
| NamespaceMappingsOwnership | H/7/host-unshare-fd-prerequisite; C/9/private-proc, attempt-failure, descendant-capability tests |
| PrivateProcfs | H/7/host-unshare-fd-prerequisite; C/9/private-proc or attempt-failure test |
| PrivateMountProfile | C/9/immutable-tree, private-profile or attempt-failure test |
| NetworkClosure | H/7/host-unshare-fd-prerequisite; C/9/attempt-failure test |
| PrivateIpc | H/7/host-unshare-fd-prerequisite; D/5/helper-suite; C/9/attempt-failure test |
| PidfdSignalsReaping | H/4/host-process-inspection-prerequisite; D/5/helper-suite; C/9/collector-suite, population, attempt-failure or watchdog test |
| ProcessInspection | H/4/host-process-inspection-prerequisite; C/9/private-proc, population, bootstrap-role or rewritten-title test |
| SeccompNoNewPrivs | H/4/host-seccomp-prerequisite |
| RetainedExecutable | C/9/collector-suite or bootstrap-role test |
| HelperDescriptorImplementation | D/5/helper-suite or exact retained-worker deterministic test |
| HelperNamespaceSmoke | N/6/exact namespace smoke |
| UtilLinuxDescriptorBoundary | H/7/host-unshare-fd-prerequisite |
| CollectorPrelaunch | C/8/exact prelaunch cleanup test |
| CollectorRuntime | C/9/collector-suite or attempt-failure test |

Exactly one definition for each host command is mandatory. Each must be a
production requirement with authority host-prerequisite-only, execution kind
host-command, the following exact gate/property set, and a result recording
**one invocation and exit status zero**:

| Command | Gate | Exact properties, in schema order |
| --- | --- | --- |
| host-seccomp-prerequisite | 4 | SeccompNoNewPrivs |
| host-unshare-fd-prerequisite | 7 | NamespaceMappingsOwnership, PrivateProcfs, NetworkClosure, PrivateIpc, UtilLinuxDescriptorBoundary |
| host-process-inspection-prerequisite | 4 | PidfdSignalsReaping, ProcessInspection |

An arbitrary authored selector cannot replace a required host command. Adding a
property outside its exact set fails, even if another source may legitimately
prove that property. Rust tests use execution kind tests and require a matching
nonzero top-level count; one test cannot be replaced by a command or review.
Recorded-review results require accepted=true and do not report test counts.
All kinds retain their raw artifacts; typed success does not authenticate the
contents. Not-applicable alternatives obey the same policies and must have a
successful matching execution kind in the same attempt.

## Host-probe observation reports

`report.rs` defines a closed, platform-independent wire contract for exactly the
three host commands. This enables deterministic parser tests on Darwin without
claiming Linux execution. Format is exactly
`borrowser-ag9g0a-host-probe-observations-v1`; authority is exactly
`host-kernel-util-linux-prerequisite-only`. Fields remain ordered as format,
authority, probe, observations; each observation is name, value. Probe is a closed
three-variant enum serialized to the exact command names above.

Reports require 1–64 observations, unique names in strict lexical order using the
existing identifier grammar, values at most 16,384 UTF-8 bytes, and at most 65,536
canonical bytes including final LF. Values can contain escaped procfs tabs/newlines.
The producer may sort observations before serialization but still rejects duplicate
names. Readers never normalize: exact canonical bytes, format, authority and
observation order must validate. `Report::parse_expected` additionally binds probe
to the requested command; the public Linux `run()` uses this function. Canonical
bytes for one command cannot be accepted as another command's response.
No extensible protocol, browser identity or qualification authority is introduced.

## Synthetic V1 golden and compatibility

`tools/conformance/host-readiness/tests/fixtures/synthetic-host-readiness-v1.golden.json`
is the exact canonical synthetic specimen. It includes test, command and review
success records, a failed historical attempt, nullable timestamp, and a documented
test-only not-applicable outcome with alternative proofs. Its identities and data
are synthetic format-test values, never qualification evidence.

The test compares all canonical bytes against the fixture and parses the fixture
back under the same strict contract. There is no automatic update flag or test-time
regeneration. Changes to field ordering, enum tags or representation require explicit
review; incompatible changes after this V1 freeze require a format-version decision.
The typed-execution corrections finalize the still-uncommitted Phase A V1 schema;
no prior dirty-tree output is grandfathered as evidence.

## External evidence layout and review

Repository docs are normative. Run-specific inventories, logs, binaries, archives,
absolute paths and output remain outside the repository:

```text
ag9g0a/<run-id>/
  manifest.json
  environment/{provisioning,platform,image-snapshot,kernel-policy,identities-namespaces,libraries-resources}/
  sources/{helper,frozen-collector,host-probes}/
  builds/{helper,frozen-collector,host-probes}/
  attempts/<attempt-id>/{commands,test-inventories,results,stdout,stderr,cleanup}/
  review/
```

Retain image/snapshot/platform proof, kernel, IDs/security/namespaces, helper source,
lockfile/build/binary, frozen collector source/manifests/build/binary, host-probe
source/lockfile/build/binary, Rust/Cargo/Git/util-linux/linker/native tooling and
loader/libraries/resources. Source artifacts include all necessary Git objects or
source exports and reproducible identity inventories. Tooling inventories include
executable hashes, not merely version labels.

Commands retain exact argv, working directory, environment/configuration, executable
digest, selected tests, top-level and nested subprocess counts, exit/signal status,
raw stdout/stderr, timestamps and cleanup results. Review must confirm that listed
counts represent actual execution, not build success or zero matching tests.

Review artifacts bind underlying inputs/results independently of the final manifest
hash, avoiding circular references. Retain the manifest digest in the final external
review/issue reference. The manifest does not reference itself. Revalidate artifacts
when reviewing; hashes without retained artifacts are insufficient. The actual durable
destination must have stable retrieval identity, reviewer access, retention and
protection against silent replacement. No external service is assumed or invented.
Failed attempts remain retained, including any applicability reclassification record.
The test fixture is synthetic format data and must never be used as real evidence.

## Phase B ordered gates and commands

Each gate requires all earlier required gates. The recorded environment is already
provisioned/offline before execution; no output from Phase A can be promoted.

| Gate | Operation/evidence | Pass or failure |
| --- | --- | --- |
| 1 environment-native-identity | Independently bind actual image/snapshot/kernel/platform | Native x86-64 tuple proven; otherwise unsupported/unproven host |
| 2 privilege-security | Runtime IDs/capabilities/policy/sysctls/resources | No host authority/bypass; otherwise unsuitable host or wrong provisioning |
| 3 frozen-offline-tooling | Verify clean frozen sources, lockfiles, local objects/dependencies, offline builds/binary digests | Reproducible tooling; missing input is preparation failure |
| 4 host-runtime-prerequisites | Seccomp and narrow process/syscall/filesystem proofs plus recorded capacity checks | Required production facilities work; unavailable facility fails host |
| 5 approved-helper-suite | Complete native helper suite/build/lint/format and retained-worker deterministic test | Actual pinned implementation passes; investigate helper defect versus environment |
| 6 helper-namespace-smoke | Explicit exact ignored namespace smoke | One executed passing test; otherwise host/helper failure |
| 7 util-linux-descriptor-proof | Actual recorded util-linux host probe | FD/namespace/proc/network state proven; otherwise host/tool/probe defect |
| 8 frozen-prelaunch-regression | Exact frozen regression | Exactly one test passes; investigate frozen collector versus host/preparation |
| 9 frozen-runtime-regressions | All frozen runtime tests below with explicit applicability | Required tests pass; reviewed test-only N/A satisfies applicability only |
| 10 evidence-review | Canonical index, actual retained artifacts, sources/results reviewed | Complete consistent evidence; otherwise preparation/evidence failure |
| 11 host-ready-review | Reviewed conclusion on one actual tuple | Host prerequisites only; Chromium qualification remains outstanding |

Missing runtime coverage is a blocker, not a documented substitute for proof.
A helper/collector behavioral defect stops qualification and requires a separate
GitHub issue, review and corrected source freeze. Never patch either inside AG9g0a.
After helper changes, preserve old attempts but invalidate helper-dependent results
and the host-ready conclusion; rebuild and rerun the full helper suite and dependent
gates under a new coherent bundle. Independent immutable provisioning artifacts may
be referenced after re-verification. Collector repair requires separate source/
manifest review and re-freeze; do not silently replace AG9g0a's frozen authority.

From the independent host-readiness workspace (all paths are externally supplied):

```sh
cargo +1.92.0 test --locked --offline --manifest-path tools/conformance/host-readiness/Cargo.toml --target-dir "$AG9G_HOST_TARGET"
cargo +1.92.0 clippy --locked --offline --all-targets --manifest-path tools/conformance/host-readiness/Cargo.toml --target-dir "$AG9G_HOST_TARGET" -- -D warnings
cargo +1.92.0 fmt --manifest-path tools/conformance/host-readiness/Cargo.toml -- --check
"$AG9G_HOST_BINARY" host-seccomp-prerequisite
"$AG9G_HOST_BINARY" host-unshare-fd-prerequisite "$AG9G_UNSHARE" "$AG9G_UNSHARE_SHA256" "$AG9G_UNSHARE_VERSION"
"$AG9G_HOST_BINARY" host-process-inspection-prerequisite "$AG9G_UNSHARE" "$AG9G_UNSHARE_SHA256" "$AG9G_UNSHARE_VERSION"
"$AG9G_HOST_BINARY" verify "$AG9G_MANIFEST" "$AG9G_BUNDLE"
```

If Cargo is not the rustup proxy, use `rustup run 1.92.0 cargo` and explicitly ensure
its selected rustc/rustdoc are also 1.92.0. No implicit toolchain installation.
Run development Linux tests explicitly by exact name, not blanket `--ignored`
(the private test-subprocess entry is not a standalone runtime gate). Tests requiring
util-linux take explicit `AG9G0A_UNSHARE`, `AG9G0A_UNSHARE_SHA256` and
`AG9G0A_UNSHARE_VERSION`; absence fails rather than skips.

From the separately verified clean helper checkout, run its complete README suite
with `--locked --offline` and its own target directory. Also execute:

```sh
cargo +1.92.0 test --locked --offline --manifest-path tools/conformance/qualification-prep/Cargo.toml --target-dir "$AG9G_HELPER_TARGET" --lib probe_linux::tests::namespace_prerequisites_runtime -- --exact --ignored --nocapture --test-threads=1
```

From the separately verified clean **04d22c36** collector checkout, retain HEAD,
clean index/worktree, original Cargo.toml/Cargo.lock and both manifest byte digests/
member verifications. Run its non-ignored library suite with chromium-cdp and:

```sh
cargo +1.92.0 test -p external-browser-capture --features chromium-cdp --locked --offline --target-dir "$AG9G_COLLECTOR_TARGET" --lib isolation::linux::prelaunch_cleanup_tests::prelaunch_partial_cleanup_covers_socket_namespace_and_fork_failure -- --exact --nocapture --test-threads=1
```

Zero selected tests fails. Run applicable ignored tests individually with the same
collector prefix and `--exact --ignored --nocapture --test-threads=1`:

```text
distribution::linux::replacement_tests::immutable_tree_rejects_main_and_helper_replacement
profile::private_runtime_tests::private_profile_old_creation_race
isolation::procfs::tests::browser_visible_private_proc_runtime
isolation::population::runtime_tests::quiescent_namespace_population_runtime
isolation::attempt_failure_runtime_tests::actual_attempt_failure_paths_reap_owned_children
isolation::attempt_failure_runtime_tests::sys_admin_is_local_to_descendant_user_namespace
isolation::attempt_failure_runtime_tests::bootstrap_role_uses_live_process_objects
isolation::attempt_failure_runtime_tests::rewritten_title_is_not_exec_argv
isolation::cleanup_runtime_tests::watchdog_success_failure_and_terminal_expiry
```

Retain `--list` inventories. The non-ignored library suite includes retained-object
exec, deadline, partial-acquisition and cleanup tests. Build/hash conformance-capture
using the preparation contract's offline command, but do not run qualification.
A same-named test from current HEAD is never frozen collector evidence.

## Development validation and closeout

Targeted deterministic tests cover canonical bytes, duplicate/unknown fields,
ordering/bounds/arithmetic, invalid identities/paths, mutation/replacement/symlinks,
missing artifacts/proofs, failed gates and invalid applicability. Linux tests cover
host probes and lifecycle/error paths explicitly. Unsupported platforms reject
runtime probe commands. Cross-compilation and Darwin/ARM tests are development
checks only; root CI does not discover this independent workspace.

Run standalone format/test/clippy/build checks and applicable development runtime
checks; root full CI remains MR/milestone validation, not native host evidence.
Architecture review and review packet complete Phase A before the user commits.
AG9g0a closes only after an actual tuple passes every production prerequisite,
all regressions have justified final applicability and the immutable evidence bundle
is reviewed. No Chromium input/identity/configuration, qualification tree or mechanism
GO is created here; AG9g0 and AG9g1 statuses remain unchanged.
