# Provider lifecycle only

Independent Rust workspace for AG9g0d Stage 1. Read the
[contract](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md)
before installation. Provider allocation never proves host readiness.

Build/test with Rust 1.92, this lockfile and a separate external target directory:

```sh
cargo +1.92.0 build --locked --offline --manifest-path tools/conformance/host-lifecycle/Cargo.toml
cargo +1.92.0 test --locked --offline --manifest-path tools/conformance/host-lifecycle/Cargo.toml
cargo +1.92.0 clippy --locked --offline --all-targets --manifest-path tools/conformance/host-lifecycle/Cargo.toml -- -D warnings
cargo +1.92.0 fmt --manifest-path tools/conformance/host-lifecycle/Cargo.toml -- --check
```

Set `CARGO_TARGET_DIR` outside the repository. Root workspace CI does not discover
this package. Linux-specific tests require a Linux test environment; never claim
native qualification from their results. The two ignored provider tests require
explicit test selection and a matching `BORROWSER_ROBOT_INTEGRATION` value of
`read-only` or `test-order`. No provider credentials are required by normal tests. `tests/commands.rs` is loaded
as a crate-internal test module so its fake store can implement the sealed persistence
boundary; production builds expose no fake-store or synthetic-capability feature.

The ignored `provider_nonprocessing_order_validation` test (`test-order` gate)
checks the configured catalogue and sends the allocation form with `test=true`
through a test-only transport path, with no billable fallback. It provides
request-shape/catalogue integration preflight only, not production lifecycle
authority or real allocation acceptance. It does not exercise durable journal
dispatch, production scheduling, transaction/server reconciliation, cancellation,
release or AG9g0a readiness. No production CLI preflight command exists; production
allocation never sends `test`.

## Installation inputs

Before authorizing any real allocation, verify Robot Webservice ordering is enabled
for the intended account/user under
`Administration → Settings → Web Service Settings → Ordering`. Retain a dated,
reviewed, credential-free activation attestation binding that context to deployment;
exclude passwords, authentication headers, cookies and other credentials. Read-only
Robot success does not prove activation. Missing or uncertain activation blocks the
billable gate. This is an operator prerequisite, not allocation authority; the
controller does not verify or enable activation. See the authoritative
[deployment contract](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md#authority-and-deployment).

An administrator supplies a dedicated local ext4 mount at
`/var/lib/borrowser-host-lifecycle`, private to a dedicated non-root controller
identity, and installs a root-owned non-writable deployment document at
`/etc/borrowser-host-lifecycle/deployment.json`.

The document uses canonical V1 encoding and exactly these fields:
`account_id`, `approved_account_currency` (`EUR`), `approved_class` (`AX42-1`), `approved_monthly_gross_units`,
`approved_setup_gross_units`, `authority_id`, `catalogue_evidence_sha256`,
`controller_machine_id`, `filesystem_uuid`, `product_id`.

Stage 1 requires an externally reviewed EUR Robot account. Verify its currency
through Robot `GET /order/currency` during deployment approval; catalogue prices
are account-currency values and do not themselves attest EUR. Retain that review
with the catalogue approval. Gross ceilings are integer units of EUR 1/10,000,
including primary IPv4. Obtain
the actual Robot product ID from the catalogue; no real ID or price is supplied
by repository fixtures. Install the reviewed, canonical typed approval artifact at
`/etc/borrowser-host-lifecycle/product-approval.json`, root-owned and not writable
by the controller. Its exact SHA-256 must equal `catalogue_evidence_sha256`.
The [contract](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md#reviewed-product-and-account-binding)
defines all fields, including catalogue ID/name/description, reviewed server-product
spelling, AX42-1 mapping, FSN1/IPv4, ceilings, currency attestation and reviewer/reference.
The live catalogue must match that binding and validate current addon quantity/location
capabilities. The binding is retained in the journal. Document the
account binding and exclusive controller ordering policy; labels are not remote
account attestations.

The credential file `/etc/borrowser-host-lifecycle/robot.credentials` is a private
regular single-link file owned by the controller identity. It contains UTF-8
`username:password` without a trailing newline, at most 8 KiB. Install it through
the external secret mechanism, never through command-line secret values. It is
not part of the authority, repository, evidence or deployment JSON.

The reviewed executable must come from clean committed tool source and its own
lockfile. Dirty development builds cannot write production history. Source review
and deployment are external responsibilities; this tool does not self-approve.

The journal is authoritative lifecycle state, not the complete acceptance package.
Retain the externally reviewed build/executable, deployment/product approval,
account/activation/currency, journal/head and transaction/server bindings required
by [operational acceptance evidence](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md#operational-acceptance-evidence).
Include cancellation/effective release, IPv4/associated-resource disposition and
separate billing disposition; V1 never asserts `billing_settled`. Preserve typed
resolution evidence and secret minimization rather than adding raw provider responses
to the journal. This evidence establishes no host readiness, AG9g0a Phase B,
Chromium/static DOM qualification or mechanism GO.

## Commands

All paths below are external input documents, not credentials. Ordinary input
documents can use normal JSON whitespace; persisted events cannot.

```text
host-lifecycle authority bootstrap
host-lifecycle allocate --request FILE
host-lifecycle status
host-lifecycle reconcile --operation OPERATION_ID
host-lifecycle watch
host-lifecycle cancel --authorization FILE
host-lifecycle resolve --resolution FILE
```

The installed binary name is `borrowser-host-lifecycle`; `host-lifecycle` above
denotes that executable. Bootstrap is explicit; a missing root or wrong filesystem
is never repaired automatically. No root override exists in the production CLI.

Allocation input fields: `operation_id`, `authorization`, `request`. The request
has exactly `product_id`, `location: "FSN1"`, `addons: ["primary_ipv4"]`. Unknown
fields—including provisioning inputs—reject. Reusing an operation ID cannot
create another submission.

Cancellation input fields: `operation_id`, `server_number`, `authorization`.
Use the exact retained server number. A lost response requires readback on a
later explicit invocation. A negative GET alone cannot authorize a second POST:
`CancellationRetryResolved` must retain reviewed provider evidence about the prior
request, followed by a fresh explicit invocation and consistent negative readback.
Acknowledged, incomplete, reserved or contradictory results never permit retry.
No schedule performs cancellation.

Resolution input fields: `operation_id`, `expected_head`, `event`, `evidence_path`.
`event` must be an allowed reviewed resolution variant from `model.rs`; its
evidence includes `sha256`, `bytes`, `provider_reference`, `reviewer`, `rationale`.
Evidence must contain no credentials. Original provider evidence is retained by
digest in the private authority. The operator verifies authenticity and attribution;
structural validation alone is not that review. There is no force-clear command.

Reports preserve identities, state, derived provider phases, obligations and journal head.
Watch scans at most two due obligations, reports every unresolved/remaining obligation,
and uses an ordinary-capacity durable cursor. Cursor writes cannot spend the final
64 journal slots or filesystem cleanup reserve. If ordinary publication is blocked,
watch reports incomplete work; fairness then requires operator attention.
An incomplete scan exits one after printing the report/state.
Recovery and cancellation do not need the external product-approval file.
Exit zero means command execution completed; callers must inspect the report for
uncertainty and pending obligations. Exit one means the command failed or was held. An accepted
command is never qualification success. Pending/uncertain states require follow-up.

## Recovery operator obligations

Before allocation, demonstrate off-controller missing-heartbeat alert delivery and
independent Robot access. Keep the product/account approval and cancellation path
available outside the controller. A process, network, disk or credential failure
must not silently remove the operator's obligation to locate/release the server.

Do not delete lock files, edit journal events, restore a backup as a fresh authority,
or order replacement compute while ownership is unresolved. Fence the previous
controller before activating a replacement and reconcile potentially missing events.
Manual emergency cancellation remains available through Robot when durable local
publication cannot be completed; retain its facts externally for later resolution.

No OS installation, keys, rescue configuration, KVM, guests, qualification gates,
Chromium, auction route, automatic destruction or generic provider/storage framework
is implemented here. Primary IPv4 obligations require explicit release evidence.

## Internal Rust boundary

The binary calls `run_cli()`, the sole supported production entry point. Authority,
clock/provider assembly, credentials, HTTP constructors and dispatch capabilities
are crate-private; Controller fields are private. Production construction derives
and validates compiled tool provenance, never a caller-supplied ToolIdentityV1.
Pure public serialization types do not authorize operations. Tests inject fakes
only inside the crate, without a production bypass feature.

Transaction responses retain allocation/targeted-read/history-item provenance.
Malformed no-identity history items produce TransactionHistory protocol failures
and incomplete scans; they do not alter the individual Transaction endpoint's
failure counters or holds. Partial history IDs remain unattributed candidates.

Completed allocation baselines constrain all automatic identity binding, including
late server discovery and partial-ID recovery. A baseline contradiction retains
facts and blocks ownership/cancellation until an explicit provider-backed
`BaselineAttributionResolved` review of the exact candidate; established identities
cannot be replaced. A transaction-only exception never approves a later baseline
server. History responses coalesce identical same-ID facts and retain conflicting
normalized/partial facts in canonical order, independent of provider array order.

Watch distinguishes not-due work from exhausted automatic recovery. At four
unknown-history rounds or 576 known-transaction rounds, `requires_operator_recovery`
is reported and `complete_scan` is false (nonzero CLI exit). Replay and repeated
watch never renew allowances; other due obligations remain eligible.

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

Resource attribution identity (transaction ID and optional server) is separate from
exact candidate snapshot identity (all normalized transaction fields). Reviews and
viable/disqualified dispositions match the full retained canonical typed snapshot.
A disposition of T/123/ready does not affect T/123/cancelled. Distinct snapshots
sharing a resource remain visible and blocking, including unreviewed alternatives;
V1 does not infer provider progression to discard them. An exact snapshot must have
been retained before it can be reviewed or disqualified.

Only one exact admissible retained snapshot with its required reviews and no other
blocking conflict can bind ownership. Canonical ordering makes retained collections
deterministic; no tie breaker selects ownership. Exact disposition cannot affect
another snapshot, established ownership, or history/server/cancellation conflicts.

Allocation and cancellation uncertainty are separate retained domains. The global
report is their union plus conflicts. Transaction/server reconciliation cannot clear
a lost cancellation outcome or consume reviewed retry readiness. A fresh explicit
cancel invocation and consistent readback are still required to dispatch again.
