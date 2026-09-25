# AG9g0d: AWS EC2 durable launch authority (Passes 1–3)

This independent external-conformance operational subsystem establishes a durable
local AWS lifecycle authority, closed reviewed data contracts and durable launch
authorization/dispatch/attempt state. **Pass 3 has no provider clients, credentials,
network transport or ability to allocate or terminate resources.** Its CLI supports
only local bootstrap and status. AG9g0d is operationally incomplete.

```text
AWS resource lifecycle
≠ AG9g0a host readiness
≠ Chromium qualification
≠ static DOM mechanism qualification
≠ mechanism GO
```

AG9g0a remains independently frozen/open at
`51dd44cafe476581be5f46a6a6fd242197e4e1b0`. No engine or readiness authority is
introduced. Implementation: `tools/conformance/host-lifecycle`, an independent
Rust workspace with no engine dependencies.

## Generation contract

| Artifact | Identifier | Schema |
| --- | --- | --- |
| Authority discriminator | `aws-ec2-lifecycle-only` | generation 2 |
| Journal event | `borrowser-aws-ec2-lifecycle-event` | 2 |
| Deployment | `borrowser-aws-ec2-deployment` | 2 |
| Root marker | `borrowser-aws-ec2-authority-root` | 2 |
| Tool provenance | `ToolIdentityV2` | 2 |

Package version is `0.2.0`. Prior and unknown future generations are unsupported.
There is no compatibility runtime, automatic migration, journal rewriting, or
conversion of historical authority into AWS authority. Historical evidence belongs
in external retention and Git history. Any earlier promise of replay compatibility
is superseded by this explicit generation break.

`DeploymentV2` contains `format`, `identity`, `schema_version`, and an optional
`reviewed_support` object. Absence preserves the frozen local-only Pass-1 document
bytes and is rejected by every launch-contract constructor. Explicit null is
noncanonical. Support is never inferred from files or AWS configuration.
Its nested identity contains exactly `account_id`, `authority_id`,
`controller_machine_id`, `filesystem_uuid`, `region`. The account is exactly twelve
ASCII decimal digits. Region is an explicit 1–32-byte lexical identifier using only ASCII lowercase
letters, digits and hyphens, with alphanumeric endpoints and no empty hyphen-separated
components. There is no geographic prefix, component-count or partition taxonomy.
Lexical validity does not attest AWS existence, availability, account access or
deployment approval; these belong to later reviewed AWS admission. Authority IDs
are 1–128 ASCII alphanumeric/`._:-` bytes. Machine identity is 32 lowercase hex digits;
filesystem UUID is lowercase hex in 8-4-4-4-12 form. No production values are supplied
by fixtures, defaults, AWS profiles or environment discovery.

`authority.json` is the immutable canonical root marker. It contains exactly
`authority`, `format`, `identity`, `schema_version`, with the same identity as the
reviewed deployment. It is completion evidence for local bootstrap, not a cloud
allocation approval. Pass-2 reviewed documents do not modify the root marker,
genesis or journal schema, add events, or enable any operational command.

## Persistent Linux authority

One dedicated non-root controller owns `/var/lib/borrowser-host-lifecycle`, a
private dedicated locally backed ext4 mount. The volume must honor flushes and
survive process death/reboot. No runner workspace, overlay/network filesystem,
automatic failover, or shared multi-host authority is supported. Replacement
requires external fencing and review; never delete a lock to defeat a live owner.

Install canonical `/etc/borrowser-host-lifecycle/deployment.json` as a root-owned,
non-group/world-writable regular single-link file. Deployment binds the actual
machine ID and filesystem UUID. `openat2` confinement, descriptor `statx` mount ID,
mount inventory, device identity, private ownership and retained directory descriptors
remain mandatory. Linux needs `openat2`, `STATX_MNT_ID` and time-namespace support.
There is no weaker production pathname or unsupported-platform fallback.

All access uses the same permanent lock inode and exclusive nonblocking `flock`.
Descriptor-relative child opens reject symlinks/magic links and mount crossings.
Object replacement is checked against retained descriptors. The macOS adapter exists
only to exercise storage tests; it is not a production authority implementation.

This boundary protects against accidental substitution and cooperating-process
races, not malicious privileged administrators, compromised kernels/storage,
forged root-owned deployment, or full-volume rollback. Hash chains do not prove
physical flush behavior or prevent replacing an entire coherent history.

## Bootstrap and opening

Only explicit `authority bootstrap` can initialize storage. It first validates
identity, generation and clean compiled tool provenance. The root must be empty
apart from a same-device, root-owned private directory named `lost+found`
(symlinks and regular files are not exempt). Existing or partially initialized
roots reject; bootstrap never removes, repairs or adopts their artifacts.

Under the permanent lock, bootstrap creates and synchronizes journal/staging/
reserve/evidence directories and 64 physically written 64-KiB reserve files. It
publishes and synchronizes canonical genesis, then publishes `authority.json`
last by synchronized exclusive rename from staging. Success is reported only after
publication durability completes. A crash before the marker leaves an incomplete
root that ordinary commands and bootstrap both reject.

Opening first requires a private canonical marker of exactly the new generation,
then acquires the permanent lock and revalidates marker bytes and inode. Every
identity field must match deployment. Replay independently validates every event
against the marker, including genesis. A marker beside an empty journal, a historical
genesis, an incompatible event, or a mismatched root digest cannot grant authority.
No rejected root is appended to or repaired. A fully published valid marker/genesis
whose final directory synchronization was interrupted may be synchronized on open;
this stabilizes complete artifacts and never synthesizes missing bootstrap records.

## Journal and provenance

Committed events use zero-based, contiguous 20-digit sequence filenames under
`journal/`. Staging/reserve/evidence files are never replayed as events. The envelope
contains exactly `account_id`, `authority`, `authority_id`, `event`, `format`,
`previous_sha256`, `region`, `root_sha256`, `schema_version`, `sequence`, `time`,
`tool`. Root SHA-256 covers exact canonical marker bytes including final LF. Rust uses
`AuthorityRootDigest` for that identity and `EventDigest` for previous-event/head
identities, with no implicit cross-conversion and unchanged SHA-256 wire encoding.

Genesis remains `authority-initialized`, at sequence zero with a null previous
digest. Pass 3 adds `launch-prepared`, `launch-dispatch-intent`,
`launch-attempt-intent` and `launch-attempt-outcome` to the same V2 envelope.
The frozen genesis/root and all Pass-2 contract bytes remain unchanged. Storage tests use compile-time-only test events to
exercise multievent chains, publication faults and reserves. Those variants cannot
be parsed by the production library or reached from its CLI.

Canonical encoding remains UTF-8, compact JSON, ASCII-sorted object keys, unsigned
u64 integers, explicit nulls, precisely escaped controls and one final LF. Unknown/
duplicate fields, noncanonical bytes, invalid identities and unsupported versions
reject. Events are bounded at 65,536 bytes; markers and deployment at 16,384 bytes.
SHA-256 links exact committed bytes. State exposes next sequence and retained head;
reopening deterministically reconstructs both. Genesis must exist.

Provenance records package/version, schema 2, clean-source flag, source revision and
embedded lockfile SHA-256. Bootstrap requires an actual clean committed build; tests
use synthetic provenance unavailable in production. Status can inspect a valid root
using a development binary but cannot append events. Provenance does not replace
external review of executable hash, compiler/linker and deployment identity.

## Publication and recovery reserve

Retain exclusive staging creation, file synchronization, no-replace rename and
source/destination directory synchronization. Publication errors poison the writer;
only reopen/replay can restore usable storage. Incomplete writes never become
committed events or reusable sequence numbers. Replay stabilizes valid published
journal entries after interrupted synchronization.

The storage bound remains 16,384 events with the last 64 slots protected from
ordinary publication. Low-byte/inode recovery uses preallocated reserve files,
claimed by synchronized rename before writing; interruption consumes the reserve
object but does not fabricate an event. Event-derived classification remains private.
Pass-3 outcome events are recovery-class; preparation, dispatch and attempt intents
are ordinary-class. There is no caller-selectable reserve flag or recovery command.
Provider-independent evidence retention/headroom primitives remain internal and
covered by storage tests, with no CLI exposure or cloud publication.

## Commands and limitations

```text
borrowser-host-lifecycle authority bootstrap
borrowser-host-lifecycle status
```

Both require Linux and the fixed deployment/root paths. Unknown/removed commands
reject before opening an authority. Neither command loads credentials or calls any
provider. JSON output reports authority, root identity, and reconstructed sequence/
head; success means local command completion only.

S3/Object-Locked storage is a separate retained evidence plane, not local sequencing,
locking or latest-state authority. The Stage A builder and qualification instance
must never become this controller. Existing staged infrastructure is not evidence
of deployed/reviewed support. Neither its older frozen controller executable nor
its build instructions authorize this generation's operations.

SDK clients, reconciliation, identity collection/verification, ingress, termination,
cloud evidence publication and static qualification support remain later passes. Real deployment and separately authorized lifecycle
acceptance remain mandatory before AG9g0d can close. No allocation or termination
can be exercised with Pass 3.

## Validation

Use the standalone workspace's locked test/build/clippy/fmt checks and external
Cargo target directory. Root engine CI does not discover this workspace. Targeted
checks cover generation/identity rejection, bootstrap failure stages, exact marker
vectors, replay, hash corruption, object replacement and locking. Preserve storage
fault-injection, evidence-integrity and reserve-exhaustion coverage without old
provider fixtures. Linux-only confinement/mount tests require Linux. Portable tests
and cross-compilation cannot qualify the production controller or AG9g0a host.

The independently authored `genesis-v2.json`/`.sha256` vector freezes exact genesis
bytes separately from the production constructor. Tests verify its marker binding,
replay head, canonical encoding, provenance rejection and constructor agreement.


## Pass 2: reviewed static support and approval

`reviewed_support` binds a reviewed deployed infrastructure reference and SHA-256,
review interval, exact AZ name/ID, VPC/subnet/route-table/S3 gateway-endpoint IDs,
1–5 sorted unique SG IDs, profile ARN/unique ID, role ARN/unique ID, evidence bucket,
regions, exact customer-managed KMS key ARN and trust-artifact digest. Region must
match both bucket and gateway-endpoint region. IAM/KMS ARN accounts must match the
authority account, KMS region must match, and ARN partitions must agree. These are
local binding checks, not AWS existence/account-access validation. The infrastructure
manifest referenced by its digest must describe route binding, VPC DNS support,
S3 DNS resolution, SG egress, endpoint policy and NACL behavior. Later admission
must corroborate it; no second bucket, Internet/NAT route or public-address fallback
is permitted when the existing evidence region is incompatible.

Deployment SHA-256 covers the entire canonical document including local authority
identity and static support. The immutable marker continues to bind only local
identity; a reviewed support change cannot alter established root identity. Loading
support does not grant mutation authority. The complete staged `infrastructure/`
work is neither consumed nor treated as deployed/reviewed identity by this code.

`LaunchApprovalV2` uses format `borrowser-aws-ec2-launch-approval`, schema 2. It binds
deployment, infrastructure and trust digests; exact launch parameters; independent
resource/cost ceilings; and reviewer/reference/validity metadata. Reviews have a
positive reviewed-at Unix-second value, reviewed-at <= valid-from < valid-until
(with an exclusive end and a maximum year-9999 timestamp). The launch approval
interval must fit within the deployment, AMI, cost and trust approval intervals.
`ReviewV2::validate_at` accepts explicit caller-supplied time for a pure interval
check; no clock, reviewer authentication or current admission is implied.

The approval carries exact AMI ID, owner account, root-device name, provenance
reference/digest, and exact instance type. Those reviewed facts must later be
corroborated. Native Linux/x86_64/HVM/EBS, nonburstable, EBS-only type suitability,
reviewed vCPU/memory ceilings, no extra AMI disks or paid product codes, and a
preinstalled minimal identity collector remain future admission obligations.
No moving image lookup, bake, installation, provisioning or host readiness exists.

## Closed launch policy versus reviewed capacity

The [field-disposition contract](ag9g0d-run-instances-projection-v2.md) is normative
for the future SDK projection; it does not claim that projection is implemented.
Every serialized `LaunchPolicyV2` field must equal the fixed V2 policy. This means
one On-Demand instance, one new primary ENI, private subnet-assigned IPv4 only,
no public address/IPv6/key pair, and one encrypted delete-on-termination root EBS.
IMDS is enabled with required tokens, hop limit 1, IPv6 and metadata tags disabled.
Shutdown stops rather than terminates; termination protection is false and stop
protection true. Detailed monitoring, hibernation, enclaves and automatic recovery are disabled.
Reboot-migration configuration is outside this contract pending the Pass-4 pinned
SDK input audit; no qualification conclusion may depend on that state. EBS optimization is true, capacity reservations
are excluded, and tenancy is shared. No alternate launch mode or replacement exists.

Capacity has **no defaults**. The sole supported storage variant is
`RootVolumeV2::Gp3 { size_gib, iops, throughput_mib_s, kms_key_arn }`. Structural
validation uses the reviewed [gp3 API limits](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_EbsBlockDevice.html):
1–65,536 GiB, 3,000–80,000 IOPS, 125–2,000 MiB/s, at most
one MiB/s per four IOPS and max(3,000, 500 × size) IOPS. These are structural bounds,
not AG9g0a sizing, regional/type availability or deployment approval. Exact values
must also fit the reviewed resource ceilings. No key alias or implicit KMS default
is allowed. Instance type remains an exact bounded lexical identifier; no
hand-authored instance-family catalogue determines eligibility.

Cost uses a closed `ReviewedHourlyCostV2` breakdown: compute, root EBS and
applicable-other allocations, all already normalized by the reviewer to currency
microunits/hour. All three amounts must be explicitly present; applicable-other
may be zero if evidence explicitly explains why static/shared costs are not
attributable to this operation. `fixed_operation_microunits` separately records
reviewed one-off attributable costs (including an explicit zero when none apply).

A distinct `PricingEvidenceDigest` in `pricing_evidence_sha256` binds exact retained
pricing evidence. Its review/reference metadata and the enclosing launch approval
bind these amounts to the exact approved instance type, region, gp3 size/IOPS/
throughput, currency, attribution decisions and review interval. Evidence must cover
priced EBS storage, IOPS and throughput; explain normalization into hourly units;
and identify applicable shared/support and fixed costs or justify their exclusion.
Pass 2 retains the digest and reviewed values only: it does not fetch, interpret or
validate pricing evidence content, calculate AWS prices, or implement a billing engine.

```text
hourly_total = compute + root_ebs + applicable_other
planned_variable_cost = ceil(hourly_total × planned_seconds / 3600)
planned_operation_cost = planned_variable_cost + fixed_operation_cost
hourly_total > 0
hourly_total <= max_hourly_microunits
planned_operation_cost <= max_operation_microunits
```

Currency remains three uppercase ASCII letters. Planned duration remains explicitly
reviewed and bounded to 1–604,800 seconds (seven days), not a production default.
Longer planning horizons require contract review. All sums, multiplication, division,
remainder and rounding addition use checked u64 arithmetic; overflow rejects.
Hourly rates are never compared directly with currency-denominated operation totals.

```text
planned runtime ≠ timeout authority ≠ automatic stop ≠ automatic termination ≠ spending guarantee
```

This is admission/accountability information, not live pricing, billing settlement
or expenditure enforcement. It neither bounds actual runtime nor accounts by itself
for unreviewed charges, changed rates or resources retained after planned execution.
Cleanup/operator ownership must be explicitly assigned in the later operational acceptance package;
a cost reviewer is not implicitly the cleanup owner. No scheduler, stop or
destructive authorization is introduced.

Tags are explicit on instance, volume and ENI. Each list has 1–16 reviewed entries,
strictly byte-sorted unique keys, nonempty bounded text (128/256 bytes). AWS-reserved
and `borrowser:` keys reject in approvals. The spec adds exactly authority-ID and
operation-ID tags to all three lists and sorts them (at most 18 entries).

## Canonical specification, token and final request

`LaunchSpecV2` (`borrowser-aws-ec2-launch-spec` / 2) binds authority, operation,
deployment digest, approval digest and the full approved launch parameters including
exact user-data bytes and derived tags. It contains no token, provider observations,
credentials, SDK objects or arbitrary maps. Its constructor validates all reviewed
bindings. `LaunchSpecDigest` hashes canonical bytes (including terminal LF).
Fingerprinting alone never establishes validity or approval.

`ClientToken::derive` first validates the spec and then hashes canonical JSON:

```text
{
  domain: "borrowser-aws-ec2-client-token-v2",
  authority_id, operation_id, account_id, region, availability_zone_id,
  spec_hash: SHA256(canonical LaunchSpecV2 bytes)
}
```

The token is exactly 64 lowercase hexadecimal characters, with no randomness,
credentials, ambient configuration or circular token input. Parsing a retained token
checks its syntax only; final request construction always recomputes and compares it.
No caller-selected token can override the derived binding. Account/region/AZ identity
are retained because EC2 subnet-based idempotency is zonal, not globally scoped.
Pass 2 defines only request identity; Pass 3 below adds local dispatch and retry
eligibility. Neither implements wire attempts.

`RunInstancesRequestV2` (`borrowser-aws-ec2-run-instances-request` / 2) contains explicit
min/max counts of 1, token, spec digest and the complete spec. Construction/revalidation
requires exact deployment/approval/trust/spec/token agreement. `LaunchRequestDigest`
hashes its canonical bytes separately from `LaunchSpecDigest`, `LaunchApprovalDigest`,
`DeploymentDigest`, `IdentityTrustDigest`, `CertificateDigest` and root/event identities.
These documents are data, never sealed mutation capabilities or authorization events.

## Collector configuration and identity trust

`CollectorConfigV2` (`borrowser-aws-ec2-identity-collector-config` / 2) contains only
account, region, role unique ID, bucket and the fixed principal-derived ingress scheme
`ag9g0d/aws-ec2-v2/identity-ingress/<aws-userid>`. Its exact canonical UTF-8 bytes,
including LF, are stored as launch `user_data` and therefore participate in all
fingerprints. The bound is 1,024 bytes. Future SDK projection encodes these bytes once
as required by its API. No operation identity, script, shell, cloud-init, executable,
URL, presigned capability or credential field exists. The approved AMI must already
contain the collector. Its implementation, role permissions and S3 transport remain
later passes, including deterministic object-version attribution.

`IdentityTrustV2` (`borrowser-aws-ec2-identity-trust` / 2) binds the exact region,
profile `rsa2048-cms-signed-data-sha256-rsa-pkcs1-v1_5`, certificate bytes represented
losslessly as lowercase even-length hex (1–16,384 decoded bytes), their SHA-256,
expected RSA-2048/exponent-65537 characteristics, reviewed source reference, review
metadata and an explicit null or previous trust-artifact digest for replacement.
Changed material always changes the artifact digest and requires a new bound review.
There is no runtime certificate retrieval, system-root trust or implicit update.

Pass 2 verifies the byte encoding/digest and structural metadata **only**. It does
not assert that DER is a certificate or that its key/region matches the declaration.
Pass 6 must parse/validate the reviewed certificate and the AWS `rsa2048` endpoint's
base64 CMS/PKCS#7 SignedData, pin the exact region certificate/key, verify the supported
single-signer SHA-256/RSA profile, and compare embedded signed content to the exact
retained IID bytes. Alternate signers/algorithms, detached content, unsupported
attributes/structures, malformed encodings and trailing data must reject then.
The signature is provider/platform evidence, never readiness or native-execution proof.

All new artifacts use strict unknown/duplicate-field rejection, bounded canonical
encoding and explicit validators. There are no dependency additions. Independent
synthetic vectors cover deployment, approval, spec, final request, collector config,
trust and token-domain input. Tests consume frozen bytes/digests, never regenerate
vectors from the production serializer. The synthetic trust vector is deliberately
not an AWS certificate and cannot satisfy future cryptographic acceptance.


### Opaque reviewed AWS identifiers

`IamRoleId` and `InstanceProfileId` remain distinct Rust types. Each retains exact
16–128 bytes using only ASCII letters (either case), digits or underscore. Exact
bytes are preserved; historical AROA/AIPA prefixes have no authority semantics. Role/profile ARNs still identify resource
classes; exact ARN/unique-ID relationships require later provider corroboration.
The collector uses the exact reviewed role unique ID, never an inferred prefix,
ARN-derived ID or friendly name.

AMI/VPC/subnet/SG/VPC-endpoint/route-table IDs require their exact typed prefix plus
1–64 lowercase hexadecimal suffix bytes. Neither eight nor seventeen characters
is a permanent V2 naming rule. These bounds protect local representation, not an
AWS resource catalogue. Safe opaque syntax does not establish existence, ownership,
relationship or approval; exact deployment/approval bindings still apply.


## Pass 3: durable launch authorization and attempts

The implemented boundaries are:

| Pass | Authority implemented |
| --- | --- |
| 1 | Durable local generation/root/bootstrap/replay |
| 2 | Exact reviewed deployment, approval, specification, token and request contracts |
| 3 | Local human authorization, logical dispatch, attempt receipts and retry eligibility |
| 4 | Future AWS client, wire projection and transport normalization; not implemented |

There is at most one unresolved acquisition operation. Preparation, dispatch,
pending attempt, blocked response and even dispatch expiry retain that operation.
No close/replacement/reset event exists in this pass. Reviewed documents and parsed
IDs alone never grant an attempt capability. No provider observations, ownership,
termination, resource closeout or network capabilities are introduced.

### Human authorization and retained artifacts

`LaunchAuthorizationV2` uses format `borrowser-aws-ec2-launch-authorization`, schema 2.
It binds authority, account, region, AZ ID, operation, deployment/approval/trust/spec/
request digests, exact ClientToken, expected current journal head, reviewer,
reference, rationale and human `authorized_at_unix_seconds` audit metadata.
`LaunchAuthorizationDigest` is distinct from event and request digests. The human
artifact contains no boot clock, operational deadline or retry timestamp. Future
operator admission must use the protected controller; no authorization-ingestion
CLI exists in Pass 3. Reviewer text is not cryptographic reviewer authentication.

`launch-prepared` contains only a compact `LaunchBindingV2` and exactly six typed
references (`LaunchArtifactRefsV2`). Each field fixes its semantic digest type and
exact canonical byte length: deployment, launch approval, identity trust,
specification, final request and authorization. `RetainedArtifactV2<D>` is a storage
reference, not a capability; there are no implicit conversions between its digest
types. All six identities remain independently retained, even where request/spec
content overlaps. No complete reviewed document is embedded in a journal record.

Preparation first validates all reviewed documents, their frozen Pass-2 constructors,
root identity, authorization/head bindings, review admission and controller time.
It then retains each exact canonical document under its digest in the protected
local `evidence/` namespace, verifies all retained bytes, and publishes the compact
preparation event. Only successful synchronized journal publication establishes
preparation authority. Partial or complete unreferenced retention creates harmless
storage orphans: status/replay neither adopts nor removes them. Explicit preparation
may reuse identical retained bytes only after full validation and a new successful
journal publication; matching filenames alone never authorize any operation.

Evidence access retains descriptor-relative confinement, private single-link regular
files, SHA-256 content addressing, exact length/hash checks, no-replace publication,
idempotence for identical bytes only, aggregate bounds and publication-poisoning
rules. No external source file, S3 object or network lookup contributes replay
inputs. Replay resolves all six exact references, checks length/digest, canonical-
decodes each expected type, re-runs all frozen Pass-2 validators and checks every
reference against the reconstructed documents and authorization. Missing, malformed,
wrong-type, wrong-length, noncanonical, corrupt or inconsistent artifacts fail closed.
Dispatch/attempt capability creation also rechecks the current retained artifacts.
The pure reducer accepts separately resolved inputs; production journal application
supplies these solely from confined evidence storage. References alone cannot apply
preparation. Failed validation cannot partially modify state.

Large artifacts retain their existing individual canonical bounds and the evidence
store's existing 256-object / 64-MiB aggregate limits. The journal remains bounded
at 65,536 bytes per event. A maximum-value test uses 128-byte authority/operation IDs,
32-byte region/AZ identities, maximally escaped bounded text, full SG/tag collections,
and the maximum 16,384-byte trust-certificate representation. Each document validates
individually, while encoding their aggregate exceeds the journal ceiling. The compact
preparation is 2,902 bytes; dispatch is 2,922; attempt is 3,458; the largest outcome is
4,023. A separate conservative expansion to 20-digit numeric fields yields at most
4,103 bytes. All shapes must remain below an 8-KiB regression budget, leaving more
than 56 KiB of structural headroom below the unchanged event ceiling. The numeric
expansion is an upper-bound calculation, not a claim that all maximum integers can
coexist in a reachable journal. No normal test regenerates golden vectors.

### Controller-derived operational timing

The preparation envelope's controller `TimeSample` is the sole start of operational
authority. Reducer state derives `LaunchDispatchDeadline` as exactly that sample's
`boottime_ns + 120_000_000_000`, with checked arithmetic. Human audit time cannot move
the start or choose a deadline; future audit timestamps reject. Realtime is used for
review admission/audit, never for operational deadline/delay arithmetic. The derived
window is not reset after artifact retention, restart, retry, expiry or reboot.

### One logical dispatch, distinct attempts

`launch-dispatch-intent` binds the authorization digest, preparation sequence/head,
complete launch binding and controller-derived preparation timing context. Its resulting sequence/head
and sampled time become `DispatchIdentityV2`. Exactly one dispatch is allowed per
operation. Retries cannot create another dispatch or change region, AZ/AZ ID,
subnet, request, specification or token.

`launch-attempt-intent` binds that exact dispatch, an attempt number in 1–3, and the
envelope timing context. Its sequence/head form the individual attempt receipt.
`DurableLaunchDispatch` and `DurableLaunchAttempt` have private fields, no public
constructor and no Clone/Serialize/Deserialize implementation. Only successful
synchronized journal publication returns a capability; a parsed receipt is data.
The attempt capability has immutable identity access and is consumed by value when
recording its outcome. Future transport must likewise consume it by value, and
recheck its carried deadline/clock immediately before transmission. Pass 3 has no
transport consumer or request-only mutation interface.

An attempt intent without a durable outcome is already uncertainty. Replay never
reconstructs a lost attempt capability, even if the process died before handing it
to transport. This conservative boundary sacrifices a possible retry rather than
inventing proof that nothing was transmitted. Later reconciliation is required.

### Outcomes and bounded scheduling

| Retained outcome | Retry consequence |
| --- | --- |
| `definitely-not-transmitted` | May become eligible after its durable observation plus the minimum delay |
| `transmission-uncertain` | Blocked pending future reconciliation |
| `parameter-conflict` | Permanent retry block for this operation; parameters/token cannot be regenerated |
| `access-blocked` | Blocked; no credential-correction/revalidation transition exists yet |
| `throttled-held` | Attempt consumed, progress held; no automatic retry/release |
| `response-unresolved` | Response requires later retention/normalization; no inferred provider identity or retry |

Outcomes bind the exact last pending attempt and can be recorded only once. They
are authority-relevant categories, not AWS error mappings. A future transport must
supply trustworthy normalization; callers cannot infer non-transmission from a
missing response. No response body or credential is retained here.

Eligibility is derived from the authorization, dispatch and bounded attempt history:
maximum three attempts, within 120 seconds of the controller preparation sample, two seconds
from outcome 1 before attempt 2, eight seconds from outcome 2 before attempt 3.
Delays start at the recorded outcome observation, conservatively after attempt
intent. All arithmetic is checked integer nanoseconds. The deadline is exclusive;
retry-delay thresholds are inclusive. Preparation/dispatch/attempt clock samples
cannot move backwards. Elapsed time grants eligibility only, never execution.

The timing domain is boot ID, `CLOCK_BOOTTIME` and time-namespace identity. Reboot
or namespace change invalidates remaining transmission permission, without resetting
operation, request, token, attempt count or obligations. Same-boot restart preserves
the exact deadline and delay. Outcome recovery remains possible after reboot or
expiry, but cannot restore transmission permission. Realtime clock changes do not
shorten delays or extend deadlines. There is no `Instant`, background loop, timeout
cleanup, replacement operation or automatic dispatch.

Only unresolved provider ownership exists in Pass 3; no ownership observation can
be appended. Future ownership/reconciliation semantics must remain a separate gate
above retry eligibility. A response, uncertainty or conflict already blocks retry.
AWS zonal ClientToken idempotency supplements this local authority; it never grants
cross-AZ permission or justifies changing the exact retained request.

### Recovery classification and test boundary

Preparation, dispatch intent and every attempt intent are ordinary publications.
They cannot consume the final 64 journal slots or preallocated recovery inodes.
The internal capability-producing paths also require acquisition headroom.
An outcome for an already-consumed attempt is recovery-class because losing it can
lose the only retained account of a possible mutation. All six outcome categories
receive that treatment, including definitive non-transmission. This classification
never authorizes another attempt: that requires a separate ordinary publication.
At most three outcome events can be recorded for the unresolved operation. The
protected reserve remains principally available for recovery and future cleanup.

Publication failure poisons the writer. Reopen/replay can stabilize a fully renamed
entry, but cannot synthesize an absent entry, reset a budget, or return its lost
capability. Independent Pass-3 fixtures in `tests/fixtures/dispatch-v2/` freeze the
authorization, preparation, dispatch, attempt and six alternative outcome envelopes.
They were composed independently of the Rust serializer; ordinary tests never
regenerate them. The preparation vector references the frozen Pass-2 files and the
independent Pass-3 authorization artifact rather than embedding their contents. Filesystem tests inject failures at write/truncate/file-sync/rename/
directory-sync boundaries and check low-space, final-slot and restart behavior.
Retention crash tests cover the first artifact, all six before publication, failed
journal writes, interrupted directory sync, and missing/corrupt committed evidence.
Orphans confer no authority and never bypass the one-unresolved-operation gate.
Compile-fail tests enforce capability construction, copying, deserialization and
retargeting boundaries. Production-schema and CLI tests continue to reject test-only
events and unimplemented commands.
