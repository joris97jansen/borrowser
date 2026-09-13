# Frozen AG9g evidence/admission V1 contracts

Status: **documentation only in Stage 0**. No evidence/index loader, candidate
publication, registry population, or advisory/historical integration is implemented.
The [Stage 0 collection contracts](ag9g-admitted-static-dom-capture.md) define
canonical encoding, configuration, packaging and qualification ownership.

## Collection evidence

Format `borrowser-external-capture-collection-evidence-v1`. One capture and exactly
two independent collection attempts. All fields are required in this exact order:

```text
format
capture_id
configuration_path
configuration_sha256
collector_source_revision
collector_executable_sha256
browser_executable_sha256
browser_distribution_sha256
qualification
attempts
```

`capture_id` is the existing canonical `sha256:` plus 64 lowercase hex digits.
Configuration path is confined and its digest is the raw canonical configuration
digest, equal to capture provenance. Collector revision is the full immutable
clean Git revision. Executable digests name the actual collector/browser. The
distribution digest names the exact canonical distribution manifest.

Qualification has exactly these fields, in order:

```text
suite
suite_source_sha256
collector_source_revision
collector_source_manifest_sha256
configuration_sha256
browser_executable_sha256
collector_executable_sha256
checks
```

Suite is `ag9g-static-dom-chromium-qualification-v15`. Revision/source, executable,
browser, configuration, and qualification manifest must agree with the actual
collection implementation/configuration. Checks are exactly this ordered sequence:

```text
pre-navigation-script-disable
noscript-parser-state
authored-inline-script-disabled
authored-external-script-disabled
script-disable-preserved-through-observation
isolated-world-exact-document
inspector-executes-without-page-world-evaluation
exact-response-bytes-and-headers
non-document-requests-denied
linux-network-isolation
correlated-parser-navigation-completion
frame-loader-context-binding
read-only-inspection
fresh-process-profile-and-cache
sandbox-preserved
```

Each attempt has exactly, in order:

```text
ordinal
nonce
host_boot_id
supervisor_pid
supervisor_start_ticks
browser_pid
browser_start_ticks
user_namespace_inode
network_namespace_inode
pid_namespace_inode
profile_device
profile_inode
document_instance
inspection_realm
capture_id
artifact_sha256
artifact_byte_length
milestones
```

Ordinals are exactly 1 then 2. Nonces are separately generated 32-byte OS-random
values, encoded as 64 lowercase hex digits. Boot ID is a canonical lowercase
Linux boot UUID. PIDs are positive host-visible PIDs; start values are Linux
process start-time ticks. Namespace/profile values identify opened objects.
Document/realm bindings are adapter-produced opaque printable ASCII, 1–128 bytes.
Capture ID equals the top-level existing validated ID. Artifact digest/length
describe the actual independently received bytes, at most the existing 8 MiB.

Each milestone has exactly `kind`, `sequence`; sequence is positive u64 and
strictly increases in the unified collector command/ack/supervisor journal.
It is neither timestamp nor CDP request ID. Kind order is exactly:

```text
isolation-verified
browser-identity-verified
pre-navigation-controls-acknowledged
navigation-issued
document-response-fulfilled
document-response-completed
document-parser-completed
inspection-realm-bound
observation-received
post-observation-controls-verified
browser-reaped
```

These facts record a trusted reviewed collector's observations. Their presence,
nonces, mechanism names, and valid hashes do not independently confer admission
or cryptographically prove a dishonest author's execution history.

## Qualification and independent collection

The same final `conformance-capture` executable performs admission-grade
qualification and A/B. Its source must be clean, committed, contained in the
declared Git revision and match the source manifest. Review, then **user commit**,
then clean locked build, qualification, and A/B without rebuilding/changing inputs.
The agent stops before committing. Stage 0 feasibility, including dirty-tree
development runs, is never automatically historical admission-grade evidence.

Any trust-bearing source, executable, source membership, browser distribution,
environment, configuration, inspector, packaging, protocol, isolation, delivery,
inspection, cleanup or evidence-generation change invalidates qualification.
Unchanged public APIs/version labels do not preserve it.

A/B require separate nonces, fresh processes, pipes, profiles, and user/net/PID
namespaces. Retain A's profile/namespace handles until B is established to avoid
object-identity reuse. A is reaped before B starts. Require distinct process
identity tuples, profile objects, namespaces and nonces. Receive each output
independently through its own inspector before equality comparison. A copied
artifact cannot satisfy the collector state machine.

Require equality of actual observation bytes, canonical provenance/identity-bearing
fields, exact configuration bytes, and validated ExternalCaptureId. There is no
`capture_preimage_sha256` field or duplicate capture-ID algorithm/API.
`ValidatedExternalCaptureV1` remains the capture identity authority. Transient
attempt values never enter that identity or existing AG fingerprints.

## Admission index

Path `tests/conformance/external/capture-admissions.toml`, not created in Stage 0.
Exact top-level order: `format = "borrowser-external-capture-admission-index-v1"`,
then `admissions`. Each of zero to 256 records has exactly, in order:

```text
capture_id
evidence_path
evidence_sha256
```

Path is exactly
`tests/conformance/external/capture-evidence/<64-digit-capture-digest>/collection-evidence.toml`.
Evidence digest covers exact canonical file bytes. Sort by decoded capture digest;
duplicate IDs/paths fail even when identical. No additional admission ID exists.

Embed the exact reviewed index bytes in the verifier build. Runtime confined
same-object index reads must match those bytes. Repository review and rebuilding
are the trust decision; modifying an on-disk list alone cannot admit a capture.
This does not claim protection against building maliciously changed source.
No public trust flag, unchecked constructor or admission callback is introduced.

## Parsing, bounds, confinement and diagnostics

Use the Stage 0 canonical typed TOML rules: bounded same-object read, unknown-field
rejection, explicit validation, deterministic canonical serialization and exact
input equality. No custom parser. Index max 131,072 bytes; evidence max 65,536;
exactly two attempts, fifteen checks, eleven milestones per attempt. Identities
max 128 bytes; immutable revision and relative paths max 256; path components
max 64. U64 numeric fields with stated narrower predicates. Digests exactly
lowercase SHA-256; no unknown/missing/duplicate fields, wrong types, unsupported
versions, duplicate records or noncanonical representation.

Cumulative evidence max 16 MiB. Configuration/source-manifest files max 64 KiB,
cumulative configuration/manifest bytes max 4 MiB. New loader-owned retained
data/workspace max 32 MiB; existing AG registry/artifact limits remain separate.
Checked arithmetic and fallible reservation precede retention. These are not
false byte-exact claims about all third-party parser temporary allocations.

Every trust-bearing repository read reuses AG9b's same-opened-object confinement:
component traversal, regular-file validation, bounds, hashes and parsing refer to
opened objects. Retain verified bytes/state; never reopen a validated pathname.
Reject symlinks, escapes, unsupported confinement, and wrong prefixes.

Validation phases, in order:

1. Bounded index read and exact build-reviewed byte agreement.
2. Index schema and canonical encoding.
3. Index fields/order/uniqueness/resources.
4. Resolve IDs against existing validated registry captures.
5. Same-object evidence reads/digests in capture-ID order.
6. Evidence schema/canonical encoding.
7. Evidence fields/order/cardinality/resources.
8. Configuration and source-manifest verification.
9. Cross-record qualification/independence/provenance/artifact relationships.
10. Private admitted-context construction.

Rank diagnostics by phase, global before record, capture digest, attempt ordinal,
field ordinal, explicit error-code ordinal. Schema diagnostics use the first
reported parser offset, not parser prose as classification. Field/error ordinals
are explicit enums, not memory layout/string sorting. Subjects stay bounded.

Missing registry references or referenced evidence/configuration, malformed or
changed bytes, and inconsistent indexed populations fail the explicit advisory
operation before comparison. An unindexed registry capture remains a valid
declaration but unadmitted; in-scope comparison fails admission. Unreferenced
filesystem evidence is never discovered/trusted. Empty index admits nothing.

## Publication and existing AG identities

Stage 2 uses private staging, exclusive creation, complete validation, file/directory
synchronization and atomic no-replace directory publication. Partial publication
cannot be a valid candidate. Collection success never updates admission/registry
automatically. Stage 3 is a reviewed multi-file change with index last; loaders
reject inconsistent intermediate state rather than assuming multi-file atomicity.

Evidence/index bytes do not enter ExternalCaptureId or existing AG fingerprints.
Configuration identity already participates through the existing configuration
digest. Evidence digests identify exact review inputs, not another capture-ID
system. Existing baseline/trend grammars record existing advisory evaluation
types. Historical baseline loading does not re-admit captures. Removing admission
prevents future evaluation without rewriting history. Local detailed admission
errors map to existing `UnsupportedCaptureContext` at the AG9c gate.

The verifier can be newer than the collector. Historical admission resolves the
exact reviewed historical source/configuration set, never silently substitutes
current files, and never qualifies a changed collector for new captures. Ordinary
aggregate/CI execution remains independent of these evidence inputs.
