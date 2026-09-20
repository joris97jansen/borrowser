# AG9g0d Stage 1: Hetzner provider lifecycle

This independent workspace acquires provider compute only. It produces no native
host-readiness, provisioning, Chromium, qualification or mechanism-GO evidence.
AG9g0a Phase A remains frozen at
`51dd44cafe476581be5f46a6a6fd242197e4e1b0`; genuine native Phase B is still required.

Implementation: `tools/conformance/host-lifecycle`. Its dependencies never include
the engine, collector, qualification-prep or host-readiness. Its CLI requires Linux;
portable deterministic tests do not establish a supported production controller.

## Provider contract

The only production allocation route is Hetzner Robot's regular
`POST /order/server/transaction`. The exact form contains `product_id`,
`location=FSN1`, and `addon[]=primary_ipv4`. The product ID comes from reviewed
deployment configuration for AX42-1, verified against the current catalogue.
It is never inferred from the marketing name. Catalogue gross monthly/setup prices,
including primary IPv4, must fit explicit deployment ceilings in EUR 1/10,000 units.
Robot prices are in the account currency. V1 requires a reviewed EUR account,
explicitly pinned as `approved_account_currency=EUR` in deployment configuration.
The deployment review must check `GET /order/currency`; catalogue values alone
do not verify currency. These admission ceilings are not a provider-enforced
total-spending cap.

No `dist`, `lang`, `authorized_key[]`, password, comment, rescue or boot fields
are sent. No fallback chooses another product/location, inserts provisioning
parameters or retries using a different request. An unexpected required provider
field is an unsupported contract discrepancy requiring review.

Reference: [Robot Webservice](https://robot.hetzner.com/doc/webservice/en.html),
reviewed 2026-09-19. The API documents no allocation idempotency key. Comments cause
manual processing and are excluded. Transaction listing covers the previous 30
days; absence, including history expiry, does not establish non-allocation.

Endpoint-class budgets live together in `scheduling.rs`:

| Endpoint class | Documented ceiling |
| --- | --- |
| Allocation POST | 20/day |
| Cancellation POST | 200/hour |
| Transaction history GET | 500/hour |
| Individual transaction GET | 500/hour |
| Server GET, conservatively combined list/detail | 200/hour |
| Cancellation GET | 200/hour |
| Catalogue GET | 500/hour |

Every request consumes a durable rolling-window budget entry before transport. Runtime
`RATE_LIMIT_EXCEEDED` observations impose additional holds and permanently tighten
the retained count/interval ceiling for this V1 authority. Budgets never confer
permission to resubmit a possibly-transmitted mutation. Provider quotas may also
be affected by external clients; this tool cannot fence Robot's web interface.

## Reviewed product and account binding

The root-owned `/etc/borrowser-host-lifecycle/product-approval.json` is a bounded,
canonical `ProductApproval` V1 artifact. `catalogue_evidence_sha256` is the SHA-256
of its exact bytes, including final LF, and is checked before allocation. The
artifact must agree with deployment account scope, exact Robot product ID,
AX42-1 class, price ceilings and externally attested EUR currency. Its complete
typed contents and digest are retained in `CatalogueObserved`, so the review
binding remains reconstructible without retaining arbitrary provider JSON.

The artifact contains `schema_version`, `account_scope`, `hardware_class`,
`catalogue` (`product_id`, exact `name`, ordered `description` strings),
`server_product` (the independently reviewed Robot server-product spelling),
`location`, `addon`, `monthly_gross_ceiling`, `setup_gross_ceiling`,
`attested_account_currency`, `reviewer` and `provider_reference`. The reference
identifies the external review materials, including the class mapping and
credential/account/currency check. Root-controlled review supplies that mapping;
Robot does not expose a Borrowser hardware-class attestation. No marketing-name
heuristic, repository fixture or API string alone approves AX42-1.

Before dispatch, authenticated catalogue data must match the reviewed ID, name
and complete description exactly; list FSN1 as available; contain exactly one
`primary_ipv4` ordering entry; permit quantity one (`min <= 1 <= max`); agree
with any explicit addon location restriction; and provide unambiguous FSN1
server/addon prices within both approved combined ceilings. A missing, changed,
duplicate or incompatible capability rejects admission. An absent optional addon
`location` is acceptable only with its FSN1-specific pricing entry. Limits and
locations are validated from current provider data, not inferred from the class.

`server_confirmed` means **current accepted provider-resource corroboration
only**: the observed server number equals the retained number, `product` equals
the reviewed server spelling, datacenter is FSN1 or an FSN1-prefixed datacenter,
provider status is `ready`, and `cancelled` is false. The latest accepted typed
server metadata remains in state and history. Wrong number/product/datacenter
sets conflict and clears confirmation. `in process` or cancellation cannot set
confirmation; unsupported statuses reject normalization. Cancellation-state
contradictions reopen attention. This predicate establishes no OS, native x86-64,
host-readiness, guest, qualification or mechanism-GO fact.

V1 deliberately keeps currency and credential-to-account association as external
deployment attestations. The operator checks authenticated `GET /order/currency`
and retains that review before approving EUR. This tool does not call that endpoint
or claim to detect later currency/account changes. Local `AccountScopeId` is an
operating scope, never remote account proof. Credential replacement requires
renewed external account/currency review. Recovery/cancellation use retained
operation facts and account scope; losing the external approval artifact blocks
new allocation, not emergency cleanup of a known resource.

## Authority and deployment

One persistent Linux controller owns `/var/lib/borrowser-host-lifecycle`, a
dedicated locally backed ext4 filesystem. The filesystem and device must honor
flushes and survive process death/reboot. No runner workspace, overlay, network
filesystem, shared multi-host mount or automatic failover is supported.

A protected root-owned deployment file binds the controller machine ID, volume
UUID, account/authority IDs, approved hardware class and exact product ID. The
controller runs under a dedicated non-root identity. All provider commands use
the same permanent lock-file inode and kernel `flock`. Never delete that file to
recover a lock; terminate a stuck owner through the external operator procedure.
Replacement requires fencing the previous controller/provider access first.

Root acquisition uses Linux `openat2` with BENEATH/NO_SYMLINKS/NO_MAGICLINKS,
descriptor `statx` mount ID, mount inventory, filesystem/device identity and private ownership checks. The
verified directory descriptors are retained. The supported kernel must provide
`openat2`, `STATX_MNT_ID` (Linux 5.8+) and time-namespace identity; missing
facilities reject rather than falling back to weaker pathname access. Journal, reserve and lock operations
use single-component descriptor-relative `openat2` with NO_XDEV and exclusive
renames; the macOS test adapter is not a production authority implementation. Child
directory identity and device checks detect replacement; absolute authority paths
are not repeatedly reopened for operations.

The threat boundary covers accidental pathname/symlink substitution and
cooperating-process races. It does not protect against a malicious privileged
administrator, forged deployment configuration, compromised kernel/storage, or
an undetected full-volume rollback. Filesystem type does not prove physical
flush behavior. Deployment must establish that separately.

`authority bootstrap` is explicit and local-only. The administrator first mounts
and protects the volume and installs deployment configuration. Ordinary commands
never create a missing root. Partial bootstrap is not silently repaired. A
reviewed clean committed build is required for all production journal-writing
commands; development tests use explicitly synthetic provenance.

## Facts and transitions

The reducer stores allocation progress, retained identity, cancellation progress,
candidate/conflict/uncertainty facts, endpoint holds, observation scheduling and
billing disposition separately. Its external phase is derived. Robot's `ready`
is a provider status only; Borrowser uses `allocated` after server corroboration.

Allocation records authorization, exact canonical request/fingerprint and a fixed
deadline, then a complete bounded transaction/server baseline. Baseline items
are individual events; a completion event binds their counts. No partial baseline
permits dispatch. Catalogue verification and storage admission precede mutation.

A pending request retains its endpoint and owning operation. A crashed mutation
may be marked response-lost to enable reads; a crashed read requires reviewed
provider-access resolution because an unrecorded authentication error is possible.
Neither path grants mutation retry permission.

A synchronized dispatch intent precedes one allocation POST. A valid returned
transaction identity is retained before acceptance is reported. A failure between
intent and durable response remains uncertain. Repeating `allocate` with the same
operation ID never reconstructs permission to submit again; a different request
under that ID is rejected. Another unresolved operation blocks new acquisition.

Recovery queries an established transaction directly or collects new history
candidates against the baseline. A unique compatible candidate is not sufficient
attribution: an identical manual order or delayed history may be indistinguishable.
V1 therefore requires reviewed provider/operator evidence for lost-response binding.
It never chooses the newest candidate. Known IDs remain retained during failures
and contradictions. Conflict resolution cannot rebind an established server.

Completed pre-submission transaction and server baselines are negative attribution
constraints. Every automatic binding, including later targeted reads and partial-ID
recovery, checks both identities against them. A baseline identity is retained with
its observation and `BaselineContradiction`, but cannot establish ownership or
cancellation authority. Absence is necessary, never sufficient, attribution.
`BaselineAttributionResolved` requires reviewed provider-backed evidence and an
exact compatible retained candidate associated with a contradiction. It cannot
replace an established transaction/server or approve an unrelated history candidate.
A transaction-only exception does not approve a later baseline server: that exact
server requires its own reviewed exception. Original contradictions remain retained.

Candidate ambiguity is an ownership constraint independent of pre-submission
baseline contradictions. `AllocationResolved` requires an exact retained,
non-disqualified snapshot. With alternatives or blocking conflicts, its reviewed
event is retained but does not bind ownership. It never discards alternatives.
For an unowned, uncertain dispatched allocation without a response identity,
`BaselineCandidateDisqualified` also supports exact recovery-candidate disposition
when the baseline is empty; its historical event name does not create a baseline
contradiction. It cannot clear unrelated conflicts or itself supply allocation
authority. For baseline-free recovery, after dispositions leave one candidate,
submit a fresh `AllocationResolved`.
One compatible retained candidate with no blocking conflict can be recovered by
that explicit review without an artificial `BaselineAttributionResolved` event.
Normal direct-response attribution keeps its existing strict rules.

Candidate snapshots are retained losslessly across provider observations and rounds.
The candidate set is historical attribution evidence, not a latest-state cache.
Exact duplicates coalesce; a newer snapshot never replaces a distinct earlier
snapshot because transaction or server identity matches. This also applies to
successive history responses containing only one transaction each. A distinct
snapshot exceeding the 1,024-candidate bound fails closed without eviction; an
exact duplicate at capacity remains admissible.

Resource attribution identity (`ResourceAttributionIdentity`) is the transaction ID
and optional server number. Exact candidate snapshot identity is the complete typed
`Transaction`: ID, server number, status, date, product, location and addons. Full
payload equality, not resource identity or provider array position, identifies a
review subject. Candidate and review collections are ordered by canonical payload
bytes; this ordering is only for deterministic retention, never ownership selection.

`BaselineAttributionResolved` reviews exact snapshot viability; it does not select
an owner. Each `ReviewedBaselineAttribution` retains that complete snapshot and a
`Viable` or `Disqualified` disposition. Distinct non-disqualified snapshots remain
blocking even when they share a transaction/server identity, including alternatives
not yet reviewed. V1 does not infer valid provider progression between these facts.
A full normalized identity-only transaction is also a separate snapshot requiring
its own disposition; a partial-response ID alone is not a normalized snapshot.

`BaselineCandidateDisqualified` requires the exact snapshot to have been retained
by this operation. Its envelope binds operation/account/authority, current expected
journal head and reviewed evidence. Disposing of T/123/ready cannot dispose of
T/123/cancelled or any other changed field. It cannot disqualify established server
ownership, revive a disqualified snapshot, or dismiss another conflict category.
Original observations and review events remain immutable.

`unique_bindable_candidate()` requires exactly one non-disqualified retained
snapshot, its exact viability review, compatibility with the allocation request,
all required baseline reviews, no unrelated blocking conflict and no replacement
of established ownership. Zero or multiple snapshots cannot bind. No first/last,
maximum or other tie breaker selects ownership. This proves both unique resource
attribution and unique supporting facts, independently of review/disposition order.
This is neither cancellation nor release evidence.


Reviewed resolution is scoped to the exact retained facts it covers. Resolving one
conflict never clears an unrelated conflict. `Operation::conflict` is a report
cache recomputed after every accepted event from unresolved baseline facts,
retained history conflicts and typed transaction-identity, transaction, server,
server-cancellation and cancellation contradictions. Blocking facts also preserve
reported uncertainty. New ownership binding requires that predicate to be clear.
Other contradiction facts are deduplicated and canonically ordered, bounded to
1,024 per operation, with their resolved status retained. Recurrence reopens a
previously reviewed fact; journal history preserves both reviews and observations.

A baseline review covers the exact retained snapshot and may cover an earlier
partial-response identity contradiction for that transaction. It never covers a different
nonempty server number, history, server corroboration or cancellation facts.
`IdentityConflictResolved` covers only retained transaction facts exactly equal to
its reviewed transaction, while still requiring the established transaction and
server identity. It cannot clear different transaction facts or another conflict
category. Allocation/non-allocation, release, cancellation-retry and access
resolutions do not dismiss unrelated conflicts. Release still requires no conflict.
Where V1 has no applicable scoped resolution for a retained contradiction, the
operation remains blocked for operator/provider recovery and further reviewed
protocol work; the generic identity resolution is not a force-clear escape hatch.


Interrupted pre-dispatch/baseline work also remains explicit; no hidden restart
submits it. An operator may resolve non-allocation using the retained no-dispatch
history and appropriate external evidence, then authorize a new operation.

## Typed durable dispatch boundary

`AuthorityId`, `AccountScopeId` and `OperationId` are distinct nonempty ASCII
identifiers of at most 128 bytes (`A-Z`, `a-z`, digits, `.`, `_`, `:`, `-`).
`RobotTransactionId` allows alphanumerics/hyphens; `ProductId` allows alphanumerics,
periods, underscores and hyphens. `ServerNumber` is a nonzero u64. Event, request
and evidence digest types accept exactly 64 lowercase hexadecimal characters.
Constructors and deserialization validate these bounds; their JSON string/u64
representations preserve existing canonical bytes.

A `MutationDescriptor` contains `method`, `endpoint`, exact encoded `body` and
`body_sha256` (SHA-256 of body bytes without an added newline). Its fingerprint
hashes the canonical descriptor bytes including final LF. Allocation uses exactly
`POST /order/server/transaction` and the three approved inputs; cancellation uses
`POST /server/<retained-number>/cancellation` and the fixed cancellation body.
Neither credentials nor Authorization headers are descriptor fields.

The dispatch-intent event retains the descriptor. Replay checks equality against
the exact request or retained server before permitting the transition. Only after
successful durable publication can the orchestrator create a private, non-cloneable
`DurableAllocationDispatch` or `DurableCancellationDispatch`. Each contains the
mutation kind, authority/account/operation identities, immutable intent sequence
(the attempt identity), event-head receipt, descriptor and descriptor fingerprint.
Allocation also retains its original fixed authorization deadline, checked again
against Linux boot/time identity before transport. Cancellation never inherits
that expired acquisition deadline. The provider trait consumes the matching capability by value and accepts no
replacement operation, body or server argument. Transport checks account/authority
scope and kind, then sends the retained descriptor endpoint/body directly.

The production `EventStore` is sealed: external callers cannot substitute an
in-memory implementation to fabricate durable permission. Only the verified
journal implements it outside `cfg(test)`. Internal fake stores and explicitly
synthetic transport capabilities exist only in unit-test builds. Public descriptor
construction or deserialization does not grant a capability. No capability can be
reconstructed from a replayed intent to resend an uncertain allocation.

The genesis golden is unchanged. This pre-deployment V1 correction deliberately
adds descriptor payloads to dispatch-intent events and reviewed product/server
metadata and typed transaction-response sources to observations; these are authorization requirements, not serializer
churn. Earlier draft payloads missing these fields reject. No historical files are
rewritten and no migration/fallback weakens them. Future deployed schema upgrades
still require explicit old-version replay support and unchanged byte digests.

## Supported production entry point

The binary delegates to the sole supported operational library entry, `run_cli()`.
It accepts no injected store, clock, provider, credential or build identity. Journal
opening, Linux deployment/credential loading, Robot HTTP construction, mutation
traits/capabilities and orchestration are crate-private. Controller fields are
private; `Controller::production` obtains compiled build provenance itself,
validates it and requires clean committed source. CLI bootstrap applies the same
build policy. Status remains the documented read-only exception for a dirty build.
Pure public model/serialization types, including historical `ToolIdentityV1`,
confer no authority and cannot be passed into the production constructor.
Clock/provider/store injection and synthetic capabilities remain crate-internal
`cfg(test)` boundaries, with no production bypass feature. These are compile-time
ownership restrictions, not protection against malicious code running as the
controller OS identity or against a privileged administrator.

## Partial transaction responses and multiple obligations

Provider transaction responses have three outcomes: no trustworthy identity,
one validated identity with unsupported surrounding data, or a normalized
transaction. A typed `RawValue` envelope and a separate duplicate-detecting ID
struct extract identity before full transaction normalization. Duplicate envelope
or ID fields, invalid IDs and invalid JSON do not yield an identity. Secret
screening remains separate; a generic JSON value never chooses an identity.

A partial allocation response commits its ID and typed failure immediately,
clears the pending transport request and leaves allocation uncertain. It cannot
bind a server. Reconciliation targets that exact ID; only a later fully compatible
transaction can establish allocation through this direct response attribution.
Every normalized read and partial-identity event retains a typed source:
`allocation-response`, `transaction-read`, or `transaction-history-item` (full
allocation responses already have their distinct event kind). A no-identity item
is a protocol `FailureObserved` for that source's actual endpoint. For history,
this deliberately chooses **TransactionHistory protocol failure**, making the
scan incomplete; it is not a transport failure of the individual Transaction
endpoint. The successful HTTP/read observation and subsequent protocol failure
can coexist. Only the actual source endpoint's failure/backoff accounting changes;
normal charging still accounts for the actual GET once. Partial identities retain
their source and failure without inventing a transport failure or endpoint hold.
Partial history identities do not gain direct allocation-response attribution. Contradictory IDs are
retained without replacement; a unique history candidate is still insufficient.

Each history response is grouped by validated transaction ID before reduction.
Identical normalized occurrences coalesce. Distinct typed facts for the same ID,
including normalized/partial combinations, become `HistoryConflictObserved`
events in ascending ID and canonical-byte order. Every distinct fact remains
retained, no member binds ownership, and the scan is incomplete. Provider array
ordering cannot choose current facts. No-identity items retain the existing
TransactionHistory protocol-failure semantics and are canonically ordered too.

Acquisition requires all previous operations closed, but recovery admits up to
the existing 64-operation bound, including later-reopened releases. `watch` selects
at most two due operations per invocation in sorted cyclic ID order, using a
persisted account-level cursor for fairness. It respects each operation's schedule
and shared endpoint holds. One failure does not erase other obligations or stop
the bounded scan; an authority publication failure stops progress safely.

The report lists attempted, failed/held, remaining, remaining-due and all unresolved
operation IDs, `requires_operator_recovery` with typed exhaustion reasons, plus
`complete_scan`. Exhaustion makes the scan incomplete even if no reads are due. CLI watch prints that report and retained state;
it exits nonzero for an incomplete scan. A complete scan is never resource release.
A held operation does not consume an observation round. Cursor publication is
ordinary only. Failure to persist it stops the bounded scan, reports incomplete
work and requires operator attention; it cannot spend protected recovery capacity.
Without durable cursor progress, fairness may require operator intervention.
Cursor publication does
not change ownership, release authorization, ambiguity or provider state. Watch
and reconcile retain only provider-read interfaces and perform no provider mutation.

## Cancellation and release

Cancellation targets only the exact bound server number. Durable explicit release
authorization precedes the request. The request body is exactly:

`cancellation_date=now&reserve_location=false`

Reservation eligibility never enables reservation. There is no cancellation
withdrawal surface. A later explicit cancellation invocation performs readback
before considering resubmission. Readback is semantic, not proof that a GET ran:

| Provider observation | Retained readback | Retry permission |
| --- | --- | --- |
| `cancelled=true`, date present, `reserved=false` | Acknowledged | None |
| `cancelled=false`, no date, `reserved=false` | NoCancellationScheduled | None by itself |
| `cancelled=true`, no date | Inconclusive | None |
| `cancelled=false`, date present | Inconclusive | None |
| `reserved=true` or wrong server | Inconclusive/conflict | None |

V1 does not claim Robot read-after-write consistency or that a negative GET proves
an uncertain earlier cancellation cannot later take effect. After a dispatch,
resubmission therefore additionally requires `CancellationRetryResolved`: reviewed
provider-backed evidence that the prior request is no longer pending and no
cancellation is scheduled, for the exact retained server, with no identity or
reservation conflict. This moves a consistent negative readback to
`ReviewedNotScheduled`; it cannot approve an inconclusive or acknowledged result.
A fresh explicit cancel invocation must obtain another consistent negative GET,
record fresh authorization and durably publish a new dispatch intent/capability.
An inconsistent observation invalidates the review; each dispatch consumes it.
For the first dispatch only, a consistent negative readback plus initial explicit
authorization suffices. No scheduler can resolve or resubmit cancellation.
Authentication, rate and retry holds still apply.
Conflicts and lost responses are retained; neither is success by itself.

Cancellation acknowledgement is distinct from service termination. Neither a
successful POST, `cancelled=true`, elapsed time nor repeated 404s creates `released`.
V1 release requires a reviewed resolution binding provider-issued effective
termination evidence to account/operation/server, absence of location reservation,
and closure of the approved IPv4 resource obligation. The tool verifies retained
artifact bytes/digests at publication and replay, plus typed subjects; it does not authenticate correspondence or
replace the reviewer's assessment. New contradictory evidence reopens attention.

Allocation and cancellation uncertainty are orthogonal. `AllocationUncertainty`
tracks unresolved allocation facts; `CancellationUncertainty::OutcomeUncertain`
tracks the cancellation dispatch outcome. The report-level `uncertain` field is
recomputed as their union plus unresolved conflicts after every accepted event.
Transaction binding/reconciliation can clear allocation uncertainty only. It cannot
clear a lost cancellation outcome or invalidate `ReviewedNotScheduled` readiness.
Cancellation readback, reviewed retry and release transitions govern cancellation
uncertainty; acknowledgement clears that domain only and never establishes release.
Contradictory cancellation observations reopen it. Both cancellation retry gates
use cancellation-specific uncertainty, prior dispatch, semantic readback/review
and conflict checks, not the report-level union. Reconcile/watch remain GET-only;
a fresh explicit cancel invocation still reads back and durably authorizes dispatch.

`billing_settled` remains false in V1: invoice settlement is not inferred or
implemented by service cancellation. No unrelated qualification-evidence handoff
blocks explicitly authorized emergency cancellation. Provider ownership and
cancellation facts must still be retained durably.

## Journal V1

`journal/` contains only immutable committed events named with a 20-digit unsigned
sequence plus `.json`, starting at zero. `staging/`, `reserve/` and `evidence/` are
distinct namespaces and are never replayed as journal events.

Envelope fields, in ascending ASCII key order:
`account_id`, `authority`, `authority_id`, `event`, `format`, `operation_id`,
`previous_sha256`, `schema_version`, `sequence`, `time`, `tool`.

`format` is `borrowser-host-lifecycle-event`, `authority` is
`provider-lifecycle-only`, and `schema_version` is 1. Event variants use a `kind`
tag and a typed `data` object when they have payload; unit events have only `kind`.
All nested object keys also sort by ascending ASCII bytes. No arbitrary JSON maps
or raw provider bodies are retained. Schema arrays preserve their specified order;
provider add-ons are sorted, unique strings. Baseline collection is sorted by ID.

The custom encoder, not serde_json's serializer, defines these bytes:

- UTF-8 without BOM; compact JSON; exactly one final LF and no trailing bytes.
- Quote/backslash escape as `\"` and `\\`; U+0000–U+001F as lowercase `\u00xx`.
  Other Unicode scalars are emitted literally; there is no Unicode normalization
  or slash escaping. Field validators reject controls where prohibited.
- Unsigned u64 decimal integers only. No signs, fractions, exponents, leading
  zeros or floating-point values. Absent optional fields are explicit `null`.
- Unknown/duplicate fields, illegal transitions and noncanonical encodings reject.

SHA-256 covers **exact persisted bytes including the final LF**. Genesis has
`previous_sha256=null`; later events reference the preceding event's byte digest.
The event does not contain its own digest. Reports expose the retained head digest
for independent retention. A digest chain does not independently authenticate a
reviewer or prevent rollback of an entire valid chain.

Tool provenance V1 contains package name/version, schema version, source revision,
dependency-lock SHA-256 and clean-source flag. Revision/clean status are captured
at build time; lock bytes are embedded in the executable. Production writes reject
dirty builds. V1 does not claim reproducible-binary attestation: compiler/linker
identity and executable digest are outside its supported provenance contract.
Review/deployment must preserve the reviewed build. Historical tool versions are
retained per event. Upgrades must never rewrite them.

Golden bytes/digests are independent protocol vectors. Library upgrades must
preserve them. Unknown schema versions reject; no migration/rewriting command
exists. A future encoder/schema must explicitly support old replay and link new
events to the old byte digest.

## Publication and recovery reserve

Publication creates a private staging object, writes it, synchronizes its bytes,
uses atomic no-replace rename into the journal, then synchronizes both directories.
Only then may its receipt authorize mutation. Errors poison the current writer;
it must reopen/replay before proceeding. Recovery validates and synchronizes a
published event whose writer may have died after rename but before directory sync.

Publication priority is derived centrally from the typed event in `publication.rs`;
callers cannot supply an emergency flag. Exhaustive matching requires every new
event/endpoint class to be classified explicitly. The reviewed recovery allowlist is:

- `AllocationResponse`, `PartialTransactionIdentity`, `TransactionObserved`,
  `ServerObserved`, `HistoryConflictObserved`: retain mutation outcomes and resource/identity observations.
- `CancellationAuthorized`, `CancellationDispatchIntent`, `CancellationObserved`,
  `CancellationResubmissionAuthorized`: explicit cancellation and its results.
- `AllocationResolved`, `IdentityConflictResolved`, `NonAllocationResolved`,
  `ReleaseResolved`, `AuthenticationResolved`, `ProviderAccessResolved`,
  `BaselineAttributionResolved`, `BaselineCandidateDisqualified`,
  `CancellationRetryResolved`: reviewed
  recovery facts/access required for cleanup.
- `MutationResponseLost`: retain uncertainty before recovery.
- `EndpointCharged`, `ReadSucceeded`, `BudgetRebootHold` only for Cancellation or
  CancellationRead, preserving cancellation prerequisites.
- `FailureObserved` only for Allocation, Cancellation or CancellationRead,
  retaining mutation outcomes and cancellation-read failures.

All other events are ordinary, including `WatchProgress`, observation-round
bookkeeping, allocation admission/dispatch and non-cancellation read accounting.
At sequence 16,320, ordinary publication stops; only recovery events may consume
the final 64 journal slots. Ordinary events never claim a reserve file, including
under low free bytes or inodes. Background reconciliation can therefore stop before
performing provider reads at this boundary; explicit cancellation and reviewed
recovery publication retain access to protected capacity. Reserve is finite and
still requires operator intervention if genuine cleanup exhausts it.

Bootstrap allocates 64 separate 64-KiB reserve files outside the journal. When
ordinary free bytes/inodes are low, emergency publication first exclusively moves
one reserve object into staging and synchronizes that move, then writes the genuine
event. Failed/interrupted writes leave staging artifacts. They do not become
placeholders, reusable events or committed sequence numbers. Already published
events are never overwritten. Replenishment/retention maintenance is external and
requires review; the tool does not silently recycle its audit history.

## Bounds and scheduling

Provider-derived limits are the endpoint quota table and history horizon above.
Defensive limits are 64 KiB/event, 128-byte identifiers, 1-KiB descriptive fields,
16-MiB HTTP bodies, depth 16 for provider JSON screening, 1,024 history items,
256 server-inventory entries and 1-MiB external evidence artifacts. Retained
baseline contradictions, reviewed baseline exceptions and distinct conflicting
history facts each use the existing 1,024-identity defensive ceiling; excess
requires operator recovery without dropping earlier facts. Evidence
also has an operational ceiling of 256 artifacts / 64 MiB; acquisition requires
at most 192 artifacts / 48 MiB so reviewed recovery can still add evidence. These are
supported implementation bounds, not claims about maximum provider populations.
Excess input rejects without truncating matching facts.

The 64-KiB event ceiling exceeds the supported typed payloads including worst-case
JSON escaping; large baselines are split into item records. The journal ceiling is
16,384 events (at most 1 GiB of event bytes). New acquisition additionally requires
1 GiB free space, 16,384 free inodes, fewer than one quarter of the journal slots used and
all 64 emergency slots present with their full byte capacity. These conservative operational thresholds leave
over 12,288 slots: up to 1,280 baseline items, 4,096 unbound history
observations, 6,336 known-resource observation/cursor events and room for intent,
failures, resolutions and cancellation. Retries and unexpected external activity
still consume finite headroom and can require operator recovery.
Filesystem metadata needs additional deployment headroom.

Observation scans run no faster than every five minutes. Unknown allocations get
at most four automatic history scans; known operations get at most 576 rounds.
The typed eligibility is `NotDue`, `Due`, or `RequiresOperatorRecovery` with a
history/transaction-round reason. A safely retained direct-response ID selects
the known-transaction allowance. At exactly four/576 rounds, including a scan
that consumes the final round, watch reports operator recovery and exits incomplete.
Exhaustion is checked before cadence; later watches/replay never reset it or read
that operation. Other due obligations can still be scanned. A routine scheduling
hold alone is not exhaustion. Reaching these limits requires operator recovery,
not a new allocation. Storage
headroom and emergency slots are finite; independent alerting/manual cancellation
remain required for disk failure or prolonged outages.

Timing uses Linux boot ID, CLOCK_BOOTTIME nanoseconds, CLOCK_REALTIME audit
nanoseconds and time-namespace identity. std::time::Instant is never persisted.
The 120-second acquisition deadline is fixed across processes. Reboot invalidates
it. Endpoint accounting restarts only behind a persisted full-quota-interval hold;
provider rate holds also wait their full observed interval in the new boot.

401 suppresses subsequent authentication until explicit reviewed correction.
Rate-limit observations retain max_request/interval; invalid fields fail closed.
Other failures have persisted 30/60/120/240/300-second deferrals and require review
after five failures. Mutation uncertainty is never a reason to resubmit. HTTP
calls use a five-second DNS wait, 10-second connect and 30-second total limits.
The DNS worker owns only a public endpoint name, cannot dispatch HTTP, and may
finish its system lookup after the caller times out; process exit does not wait
for it. The pinned fresh-connection adapter classifies DNS/connect/TLS-init
failures before HTTP writes as definitely not transmitted; other transport errors
remain uncertain. Neither classification automatically resubmits allocation.
No blocking poll loop exists.

## Secrets and operations

Credentials are loaded from a protected, bounded external file owned by the
controller identity; never argv/environment/journal. Credential-using execution
sets RLIMIT_CORE=0 and PR_SET_DUMPABLE=0. Owned secret buffers use best-effort
zeroization. The private transport disables redirects, proxies, connection reuse,
implicit retry paths and library logging, and emits only static diagnostics.
JSON credential echoes are rejected. HTTP-library copies and privileged compromise
are outside any complete memory-erasure guarantee.

`status` is local replay only. `reconcile` and one-shot `watch` have only the
RobotReader interface. They cannot allocate, cancel, resolve ambiguity or withdraw
cancellation. An external scheduler invokes `watch`; an independent monitor must
alert on its missing heartbeat. GitHub Actions may later call the same controller,
never copy the authority to a runner.

Before any billable run, demonstrate the ext4 deployment/flush assumptions,
concurrent-call exclusion, killed-process recovery, external heartbeat alert and
independent operator access to Robot. This repository implementation alone does
not establish those operational prerequisites or any qualification result.

## Validation boundary

Normal tests are credential-free. Pure transition/format tests, scripted provider
failures and actual local HTTP fixtures are separate from Linux filesystem tests.
Publication failure and low-space/inode tests use deterministic injected faults;
they do not prove a particular disk's power-loss durability. Linux container tests
do not qualify the container as a production ext4 controller or AG9g0a host.

Real GET integration and nonprocessing `test=true` order validation are ignored,
explicit opt-in tests with distinct gates. The latter has no billable fallback.
Real allocation/release, operational controller verification and AG9g0a Phase B
remain unperformed until explicitly authorized on an approved deployment.
