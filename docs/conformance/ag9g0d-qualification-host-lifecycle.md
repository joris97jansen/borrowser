# AG9g0d: AWS EC2 authority foundation and reviewed contracts (Passes 1–2)

This independent external-conformance operational subsystem establishes a durable
local AWS lifecycle authority generation plus closed reviewed data contracts. **Pass 2 has no provider clients,
credentials, network transport, allocation or termination capability.** It supports
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

The only production event is `authority-initialized`, at sequence zero with a null
previous digest. Production rejects further events until later reviewed passes
introduce actual AWS semantics. Storage tests use compile-time-only test events to
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
Passes 1–2 have no production recovery event, command, or caller-selectable reserve flag.
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
can be exercised with Pass 2.

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
There is no retry implementation or retained dispatch event in this pass.

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
