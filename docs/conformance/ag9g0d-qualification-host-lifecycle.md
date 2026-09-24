# AG9g0d Pass 1: AWS EC2 authority-generation foundation

This independent external-conformance operational subsystem establishes a durable
local AWS lifecycle authority generation. **Pass 1 has no provider clients,
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

`DeploymentV2` contains exactly `format`, `identity`, `schema_version`.
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
allocation approval. Launch approvals/configuration are not implemented in Pass 1.

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
Pass 1 has no production recovery event, command, or caller-selectable reserve flag.
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

Launch approval, SDK clients, token generation, reconciliation, identity collection/
verification, ingress, termination, cloud evidence publication and static qualification
support remain later passes. Real deployment and separately authorized lifecycle
acceptance remain mandatory before AG9g0d can close. No allocation or termination
can be exercised with Pass 1.

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
