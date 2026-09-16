# AG9g0: reproducible qualification preparation

**qualification-prep is a producer, not an authority.** Preparation output never
establishes mechanism GO. AG9g0 remains open; AG9g1 remains blocked. This procedure
adds no capture/admission/registry behavior and does not select a browser or host.

The historical review base is `619828f08fc97f3aa0f5d1913802dd8e08a00759`.
The frozen reviewed collector source is
`04d22c360c34c891a66400f31afcf3c4a4146973`: mechanism 17, qualification suite v15,
47 collector source entries and nine qualification source entries. Preparation
changes do not change that identity. Build the collector from that exact commit.

Authorities: [frozen capture contract](ag9g-admitted-static-dom-capture.md),
`configuration.rs`, `distribution.rs`, `distribution_linux.rs`, `wire.rs`,
`source_set.rs`, `source_identity.rs`, `packaging.rs`, `transaction.rs` and
`qualification.rs` at the frozen revision. The helper independently produces their
wire artifacts; it neither imports private source nor replaces their verifiers.
See [helper README](../../tools/conformance/qualification-prep/README.md) for
commands, schemas, resource bounds, publication and implementation differences.

## Three locations

1. Helper checkout/build: independently reviewed preparation source, separate
   workspace, Cargo.lock, target directory and executable digest.
2. Collector checkout/build: clean exact `04d22c36`, original root Cargo files,
   reviewed manifests and Rust 1.92.0; independent new build directory.
3. Qualification input root: exact exported frozen source tree plus the reviewed
   configuration and browser distribution manifest at their confined relative
   paths. This explicitly recorded input tree is not claimed to be a clean Git
   checkout. Its source files must equal the frozen Git tree.

Browser payloads, host/build inventories, archives, logs, output and absolute
machine-local paths stay outside the repository. Eventually reviewed pin metadata,
canonical config and distribution manifest may be committed separately; doing so
never changes the collector revision. Do not check in the synthetic test golden as
a real config or populate a pin with placeholder values.

## Selection and acquisition gate

Select one native non-root x86-64 Linux VM/host and one explicit browser release.
Do not infer ARM Chromium support from cross-compilation or filesystem tests.

Record publisher/distribution, exact release/version, x86-64 architecture,
immutable upstream artifact locator/object identity, artifact SHA-256 and available
publisher signature/checksum/attestation. Record source/build revision, downstream
patches and build provenance. Keep missing provenance visibly unmet; do not invent
it. Prefer immutable release artifacts over moving system installations.

Review the extraction tool/version and exact procedure before extracting into a
fresh external directory as a non-root user. Preserve the supplied population and
metadata; reject malicious archive paths/types. Identify the precise extracted
root and main regular executable. Do not remove a setuid helper, repair permissions,
rewrite links or omit files to force verification. An unsuitable distribution
requires a different reviewed pin, not silent normalization.

Freeze the tree on immutable/read-only backing storage with no writable alias
available to the probe/browser. Record the original artifact separately from the
extracted-tree snapshot. Read-only presentation must preserve recorded modes.
Record and control backing storage outside the guest; namespace/mount checks alone
do not establish that external storage administrators cannot change a disk.

## Host gate

Record exact upstream OS image/release and digest, provisioned snapshot identity,
PRETTY_NAME, kernel release, native architecture, nonzero UID/GID, and detailed
security/library/resource inventory digests. Recheck actual values on the machine
that generates configuration and runs qualification.

Require unprivileged user namespaces with current nonzero mappings; new user,
network, PID and mount namespaces; namespace ownership queries; pidfds and signals;
execveat; descriptor filtering; private procfs and no inherited proc aliases;
private tmpfs mounts/read-only remounts; readable process metadata and Chromium's
nested user-namespace sandbox. Record AppArmor/SELinux/seccomp/container policy.
An unavailable capability is an unsupported host, never permission to use root,
map-root-user, --no-sandbox or host networking.

Record native loader and host library versions outside the distribution manifest,
util-linux path/version/digest, available RAM/disk/tmpfs, file descriptor and process
limits. Account for staged and per-attempt copies of distributions up to 8 GiB,
plus browser and build headroom. Provision dependencies before host freeze; no
implicit network installation belongs to preparation or ordinary CI.

## Candidate production and review gates

1. Build/review the standalone helper and run its deterministic and applicable
   Linux tests. Record helper source, lockfile, executable and build identities.
2. Run `manifest` on the frozen explicitly supplied tree. Independently review
   full population, modes, link resolution, capability absence and executable.
3. Run `identity` using the same frozen tree/manifest/executable and a pinned
   util-linux executable/version. Retain actual raw product/revision/protocol
   candidate data and helper logs. This is not qualification evidence.
4. Author host and pin records from actual measured/reviewed facts. Use `record`
   to validate and serialize them. The human review record must bind the inputs
   without hashing itself/the final pin, avoiding circular identities.
5. Run `configuration` against the clean exact frozen source checkout. It verifies
   actual host identity and all record/source/distribution bindings. Review the
   resulting canonical bytes; do not add optional browser flags automatically.
6. Freeze the candidate inputs after review. The collector must still independently
   verify them during genuine qualification. Passing the producer is insufficient.

The source-manifest digests are:

```text
collector:     0eebd26f1beaeca68c0b9830b25ca70388bca2ff4aa512fa497fd6fd04b2c732
qualification: eaea0ed1353cae955aa54fe3e9317ac7477bc225dce564211b0b3228d8c6c2c3
```

The helper recomputes the inspector, packaging-source and exact executed-expression
hashes. No optional argv, default browser version, guessed protocol version or
manifest regeneration is part of configuration generation. Pin/host/review data
remain preparation metadata and never add fields to collector wire formats.

## Collector preflight and build gate

Obtain the already-pushed exact collector commit in a separate clean Linux checkout.
Verify HEAD, tracked/index cleanliness and absence of non-ignored untracked files.
Verify both raw source-manifest digests, their membership and every declared file
against the committed tree. Record compiler, native linker and Cargo configuration;
use Rust/Cargo 1.92.0 and the original Cargo.lock. Record all build-affecting flags.

Before real Chromium work, execute on the selected x86-64 Linux host:

```sh
cargo test -p external-browser-capture --features chromium-cdp --locked --lib isolation::linux::prelaunch_cleanup_tests::prelaunch_partial_cleanup_covers_socket_namespace_and_fork_failure -- --exact --nocapture
```

Require exactly one executed passing test. Zero tests or successful linking does
not satisfy this gate. Run the applicable existing ignored Linux mount/profile,
process/reap and watchdog tests explicitly; record test-only prerequisites such as
namespaced ns_last_pid separately from collector prerequisites.

Build into a new explicit directory with pre-provisioned dependencies:

```sh
cargo +1.92.0 build --locked --offline -p conformance-runner --no-default-features --features external-capture --bin conformance-capture --target-dir /absolute/local/ag9g-collector-build
sha256sum /absolute/local/ag9g-collector-build/debug/conformance-capture
```

Record exact command/profile, source revision, Cargo.lock, compiler/linker,
configuration/environment and executable digest. This is the frozen collector
build, not the helper build. Do not rebuild between executable freeze and run.

Materialize location 3 from the frozen Git tree and add only the independently
reviewed runtime files:

```text
tools/conformance/static-dom-capture-chromium-linux-v1.distribution.toml
tools/conformance/static-dom-capture-chromium-linux-v1.config.toml
```

Verify source bytes/modes against the frozen tree and the two additions against
reviewed hashes, then freeze that root. Runtime absolute paths stay external.

## Genuine mechanism qualification

From the qualification input root, execute the frozen executable directly:

```sh
/absolute/local/ag9g-collector-build/debug/conformance-capture qualify --purpose mechanism --configuration tools/conformance/static-dom-capture-chromium-linux-v1.config.toml --browser-distribution /absolute/local/pinned-chromium-distribution --output /absolute/outside-input-root/ag9g0-run-001
```

The output parent must exist outside the input root; the final directory must not
exist. Retain exit status without hiding it behind a pipeline, stdout/stderr,
mechanism.txt, exact config/manifest bytes and digests, collector executable/digest,
source revision, browser artifact/snapshot identities and build/host/kernel/policy
records. Retain helper provenance and preflight results separately. No automatic
retry is permitted to turn a failed run into an unexplained successful record.

GO requires successful collector termination and its genuine mechanism GO
diagnostic, matching config/executable digests, all three positive vector hashes
and both correctly attributed expected rejections. Every sandbox/process, document,
realm, late-event and terminal cleanup invariant must succeed.

Browser launch, identity probing, deterministic tests, cross-compilation, scripted
CDP, synthetic processes, partial vectors, generic failures on rejection vectors,
panics, abnormal exits and cleanup failures are not GO. Even genuine mechanism GO
is not admission-grade qualification or evidence. AG9g1 remains blocked until the
actual mechanism GO and applicable closeout review.

## Failure and requalification

Stop, retain diagnostics and classify the failure as bad preparation input,
unsuitable host/browser pin, or a trust-bearing collector defect. Inspect residual
resources after abnormal termination; no returned cleanup error is not proof that
cleanup completed. Never relax sandbox/isolation/verification, omit vectors, change
limits or substitute root/host networking.

Preparation fixes require regenerated/reviewed inputs and a complete genuine run.
A collector defect requires updated source identities/manifests, architecture
review as appropriate, a new frozen commit/build and complete qualification rerun.
Do not continue using evidence from a modified collector source tree.
