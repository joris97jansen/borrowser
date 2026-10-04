# AG9g0e1 / #1406: reconciliation contracts and bounded read foundation

AG9g0e2a1 adds [narrow V3 partial identity observations and private IAM presence capture](ag9g0e2a1-partial-identity-observation-v3.md). V1/V2 contracts and SDK pins remain unchanged; production adapters and query execution remain #1413 work.

AG9g0e1a adds [explicit V2 management/delegation observation contracts](ag9g0e1a-management-observation-v2.md).
The V1 representations below remain frozen, including their management-information
limitations. V2 has separate identities, references, inventory and context containers;
it does not migrate V1 evidence or add durable provider authority.

This extends the [AG9g0d authority](ag9g0d-qualification-host-lifecycle.md) and
[SDK audit](ag9g0d-aws-sdk-boundary.md). It establishes inert, SDK-independent
contracts and bounded transport, not provider admission or durable provider state.
Implementation lives exclusively in the independent `tools/conformance/host-lifecycle`
workspace. Root engine CI does not discover that workspace.

## Ownership and versions

`provider/manifest.rs` owns the [reviewed infrastructure manifest](ag9g0e-reviewed-infrastructure-v1.md).
`provider/observation.rs`, `network_observation.rs`, `coverage.rs` and `inventory.rs` own the abstract
[evidence vocabulary](ag9g0e-provider-evidence-matrix-v1.md). `context.rs` owns
immutable context data. `limits.rs` owns counters and representation limits;
`storage.rs` owns arithmetic over supplied storage facts. None imports an AWS SDK,
journal, network client or filesystem API. `aws/response_limits.rs` owns the
[bounded connector](ag9g0e-read-sdk-boundary.md).

New contracts have schema version 1. The generation-2 authority, root, deployment,
launch artifacts, event variants, capabilities, dispatch budget/deadline and existing
vectors remain unchanged. New contracts reject unknown versions and fields. Parsing,
hashing or cloning data grants neither publication nor launch authority.

The observation aggregate deliberately has no Serialize/Deserialize implementation.
Records have their own canonical contract representation. There is no durable
aggregate, evidence envelope, chunk, index, packing algorithm or retained layout here.
Records are ordered by canonical bytes; equal duplicates remain represented, and
contradictions cannot overwrite one another. Query identities bind operation,
account, region and typed scope. E2 must validate operation/scope compatibility and
normalize service responses. Complete coverage requires a terminal response; counts
alone never establish completeness or an atomic AWS snapshot.

Reviewed expectations and observed facts have distinct semantic ownership. The
data flow is AWS response → provider observation → e2 comparison with the reviewed
expectation → evaluation. `network_observation.rs` retains route destinations,
targets, state, origin and associations; SG permissions and peers; NACL entries;
DHCP options; and bounded endpoint-policy structural facts. It does not import
reviewed rule/policy types or decide whether a provider fact is allowed.
There are no derived `all_routes_active` or `supported_origins` observation flags.
DNS support/hostname attributes remain independent VPC facts, not DHCP expectations.

Only `manifest::Ipv4Cidr` and `manifest::Effect` remain shared primitives. The former
identifies a canonical IPv4 network without restricting which network is allowed;
the latter names the literal Allow/Deny action without granting permission. Unknown
actions have a separate `RuleActionObservation::Unrecognized` variant. No reviewed
validator is applied to observations. IPv4/IPv6 address types preserve address
semantics; canonical spelling is not a claim of acceptable addressing. Signed SDK
integers such as ICMP -1 use a bounded canonical decimal-string type, preserving the
existing unsigned-only authority encoder unchanged.

Provider enum/protocol literals, foreign identities, wildcard/negated policy fields,
custom DHCP values and contradictory ranges remain evidence. E2 must classify them
after recording them. The endpoint-policy vocabulary has fixed fields, bounded
literal lists and explicit unsupported-structure markers; it is neither recursive
JSON nor an IAM evaluator. Unrepresented structures or malformed semantic values
require explicit unsupported/malformed evidence and incomplete coverage in e2,
never omission, absence or compliant defaults. Unsupported structure markers do not
claim to preserve arbitrary nested payloads or prove policy completeness.

The 8-KiB observed-policy bound, 16 statements, 8 action literals, 16 resource
literals, 16 principal identities, 8 conditions per statement and 128-byte Sid bound
remain enforced. Positive/negated fields share the corresponding collection budget.
Full resource literals use the existing 2,048-byte ProviderText bound rather than
being mistaken for the manifest's relative object-prefix representation. All nested
lists remain bounded at 128; the existing 16-KiB record and 256-KiB aggregate budgets
still apply. DHCP domain values retain their 253-byte bound.

## Limits and accounting (ObservationLimitsV1)

All arithmetic is checked. Limits are fixed by version, not caller-selectable
unlimited options. Exhaustion latches; no counter refund or reset is available.

| Boundary | V1 limit / accounting |
| --- | --- |
| Pages per logical query | 16 page attempts, initial request included; outstanding continuation means incomplete |
| Requests per round | 128 actual connector calls across all services, admission, attributes, relationships and errors |
| Decoded records | 4,096 occurrences including nested repeated semantic elements, charged before filtering/deduplication |
| Response body | 1 MiB of accepted data-frame bytes for every status |
| Response bytes per round | 8 MiB, shared across clients; accepted bytes remain charged after later failure |
| Normalized evidence | 256 KiB: sum of canonical e1 record and coverage encodings, including escaping and LF |
| Single e1 record | 16 KiB canonical encoding |
| Provider text | 2,048 UTF-8 bytes, no NUL; identifiers have their own stricter types |
| Repeated observation field | 128 elements, checked during decoding; the smaller record/aggregate byte limits still apply |
| Continuation token | 4 KiB, never logged; cycles mean incomplete |
| Elapsed observation | 300 seconds, starting before credential loading/session admission, ending after normalization |
| Automatic retry/poll | None; SDK max attempts remains one |

These bounds support one reviewed allocation and its infrastructure, not exhaustive
account-wide inventory. Each limit applies independently; bytes or time may exhaust
before request/record counts. In e2 a bound breach stops new reads and yields explicit
incomplete coverage, never truncated success. Partial bounded evidence can still
describe a conflict; it cannot establish complete singleton discovery.

Transport uses a shared `ObservationRound`, including all STS/S3 reads. Production
time uses the existing Linux boot ID, CLOCK_BOOTTIME and time-namespace identity.
Backward time, domain changes, expiry and checked-deadline overflow reject. A Tokio
timer cancels pending I/O at the lesser of 30 seconds and remaining round/session
time; clock checks before accepting each frame and EOF prevent suspend/resume or
delayed scheduling from extending acceptance. Realtime is used only for the explicit
credential expiry. Neither clock nor read results change the launch's 120-second
deadline or three-attempt budget. Non-Linux production observation has no fallback;
test clocks are compile-time test-only.

E1 executes request/body/time accounting and supplies pure page/record/coverage
contracts. E2 executes pagination, records traversal, normalization and its final
round-time check. Service errors remain static bounded categories, never retained
raw SDK errors, secret text or response bodies.

## Evidence allocation is not a durable representation

Existing authority limits remain 65,536 bytes per canonical document/event and
1 MiB per evidence-store object. The stricter following allowances are independent:

* 256 KiB of e1 normalized/canonical observation and coverage data;
* 32 KiB of **complete retained bytes** per future reconciliation evidence object,
  not 32 KiB of payload;
* a shared 256 KiB additional allowance for manifest/context retention and future
  metadata, envelope or representation overhead;
* 512 KiB total complete referenced evidence bytes per publication;
* 35 referenced objects per publication, without assigning counts to any object kind.

Manifest and context have their own 64-KiB and 32-KiB bounds. Other future canonical
metadata must fit the existing 64-KiB ceiling and the shared allocation budget.
These allowances do not promise that an arbitrary e3 encoding fits. E3 owns all
envelopes and packing and must prove its complete representation fits. E1's
`canonical_record` only measures e1-owned representations; it is not a durable writer.

## Ordinary storage and headroom

All future reconciliation artifacts/events, including first binding, are ordinary
publications. They cannot consume the final 64 journal slots or preallocated recovery
files. This classification adds no current event or change to `publication.rs`.

`ReconciliationStorageBudgetV1::check` accepts supplied complete object lengths,
verified-reuse flags, event length, normalized length and storage facts. It requires:

* total publication evidence and object counts within the above allowances, including
  reused objects; overhead is total retained bytes minus normalized bytes, floored at zero;
* projected ordinary evidence inventory at most 192 objects / 48 MiB, counting all
  current objects including orphans;
* next journal sequence below 16,320;
* capacity for new objects and the event rounded to the supplied filesystem allocation
  unit, plus a conservative extra 64-KiB staging allocation;
* at least 1 MiB and 128 inodes remaining, also allowing event and staging inodes.

Identical object reuse requires byte/hash/length verification by the future caller.
The calculation does not scan storage, reserve physical space or guarantee against
later ENOSPC. E3 must check fresh capacity under the publication lock. It must refuse
insufficient headroom before retention; no durable success is claimed. Later write
failures preserve writer poisoning, orphan non-adoption and verified reopen/replay.
Repeated observations eventually exhaust ordinary space; no pruning, recovery budget
or hidden cleanup workflow is implied.

## Optimistic context and publication contract

`ReconciliationContextV1` owns immutable data: root identity/digest, head and next
sequence, exact launch binding and six artifact references, preparation/dispatch/
attempt identities, prior provider-state identity/summary/evidence, manifest identity,
policy/limit versions and capture time. Prior provider-state identity must cover the
entire state, not merely its displayed binding/conflict summary. Owned validated
input documents may accompany the context later without being embedded into its
compact identity representation.

There is no Journal, descriptor, client, mutable authority reference or launch
capability in the context. Context construction is structural validation, not verified
authority capture. Its digest is an integrity identity, not a lease.

The mandatory e3/e4 lifecycle is:

1. Open/replay/validate authority and capture context under the existing lock.
2. Drop **all** lock-owning handles before credential/session admission or any AWS call.
3. Observe with immutable original context and one bounded round.
4. Reacquire through fresh verified open/replay, without network access.
5. Require identical root, head **and** next sequence, operation/request/artifact and
   attempt identities, prior provider state, manifest and policy/limit identities.
   Revalidate referenced bytes even if head/sequence match.
6. Reject any intervening journal advancement as stale, before new evidence retention.
7. Recompute policy under the lock, retain/verify evidence and atomically publish.
   Binding exists only after synchronized publication succeeds.

No stale rebase, merge, retarget, sequence reservation, automatic retry with old
observations, second lock or reconciliation database is permitted. Stale-publication
rejection is not a new provider observation against current state.

A data-only context cannot prove that a caller retains no separate authority handle.
Network I/O while retaining any such handle is explicitly invalid. E1 exposes no
production reconciliation entry point; actual capture/release/freshness enforcement
belongs to e3, and process/barrier-based end-to-end proof belongs to e4. Source scans
or context serialization tests are not substitutes for that later proof.

## Exposure and exclusions

Production CLI remains `authority bootstrap` and `status`, without network access.
No new status fields, credential/manifest CLI ingestion or background controller.
Manifest parsing accepts bounded supplied bytes; future ingestion must use protected
authority rules. Synthetic vectors are neither approval nor provider authenticity.

E2 owns service adapters, SDK normalization, pagination/discovery, A01–A38 evaluation,
candidate decisions and inventory construction. E3 owns durable envelopes/packing,
events/reducers, context capture/freshness enforcement, binding and replay/publication
faults. E4 owns controller/status integration and complete concurrency/restart tests.
None of this establishes signed identity, host readiness, Chromium qualification,
cleanup authority, live allocation or mechanism GO.

## Allocation observation successor (e2c)

[AG9g0e2c / #1415](ag9g0e2c-ec2-allocation-observations.md) implements the eight
launch/allocation read operations with explicit V5 observations. Frozen V1–V4
representations remain historical contracts; V5 preserves partial returned IDs,
independent attachment perspectives, qualified source positions and bounded opaque
user-data bytes. Its mixed carrier is in-memory only. Source, output and canonical
byte limits share the existing round; discovery selection, admission, binding and
durable provider publication remain separate work.
