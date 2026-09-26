# AWS EC2 lifecycle foundation and non-mutating SDK boundary — #1396

Independent Rust operational-tooling workspace. Read the
[contract](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md).
Package `0.2.0` exposes local bootstrap/status only. #1396 adds a private,
non-mutating AWS SDK boundary: explicit temporary-session credentials, STS caller
admission, S3 bucket admission and capability-gated pure EC2 request projection.
There is no production EC2 allocation or termination path. See the
[pinned SDK audit](../../../docs/conformance/ag9g0d-aws-sdk-boundary.md).

## Supported commands

```text
borrowser-host-lifecycle authority bootstrap
borrowser-host-lifecycle status
```

Commands require Linux, a dedicated non-root controller, the protected ext4 mount
`/var/lib/borrowser-host-lifecycle`, and the root-owned canonical deployment document
`/etc/borrowser-host-lifecycle/deployment.json`. There are no path overrides.

Deployment format is `borrowser-aws-ec2-deployment`, schema 2, with an `identity`
object binding exact `account_id`, `authority_id`, `region`, `controller_machine_id`
and `filesystem_uuid`. Optional `reviewed_support` binds reviewed static AWS
identities and is mandatory for launch-contract validation; absence preserves local-only
Pass-1 documents. See the contract for validation rules. Fixtures are synthetic
serialization data, never approved deployment inputs.

Bootstrap requires clean committed compiled provenance and an empty prepared root
(apart from filesystem `lost+found`). It durably publishes genesis and storage before
the final immutable `authority.json` marker. Failed/partial bootstrap is never
silently repaired. Status reopens only a matching completed generation-2 root and
reports its journal sequence/head. It permits development builds for inspection,
but does not append events. Exit zero is only local command success.

Earlier/unknown generation markers, deployment documents or journals reject. A new
marker cannot make historical genesis compatible. No migration or compatibility
runtime exists. Keep historical evidence externally; prepare a new reviewed authority
rather than editing old records or deleting its lock file. Protect against simultaneous
controllers and fence any previous authority before replacement.

## Validation

Use Rust 1.92.0 and set `CARGO_TARGET_DIR` outside the repository:

```sh
rustup run 1.92.0 cargo test --locked --offline --manifest-path tools/conformance/host-lifecycle/Cargo.toml
rustup run 1.92.0 cargo clippy --locked --offline --all-targets --manifest-path tools/conformance/host-lifecycle/Cargo.toml -- -D warnings
rustup run 1.92.0 cargo fmt --manifest-path tools/conformance/host-lifecycle/Cargo.toml -- --check
rustup run 1.92.0 cargo build --locked --offline --manifest-path tools/conformance/host-lifecycle/Cargo.toml
```

Dependencies must already be available for offline checks. This standalone workspace
is not discovered by root engine CI. Linux confinement/mount tests run only on Linux;
macOS storage tests do not prove Linux production authority. No provider integration
tests exist in Pass 3. Synthetic storage-only event variants exist solely under
`cfg(test)` and are rejected by production-schema integration tests.

Allocation, termination, identity evidence, S3 publication,
and infrastructure deployment are unavailable. Later passes must supply their real
contracts before enabling operations. AG9g0a remains independently frozen/open at
`51dd44cafe476581be5f46a6a6fd242197e4e1b0`.


## Pass-2 data contracts

The library provides `LaunchApprovalV2`, `LaunchSpecV2`, deterministic `ClientToken`,
`RunInstancesRequestV2`, `CollectorConfigV2` and `IdentityTrustV2`. Call their explicit
validators; parsing/hashing alone is never admission or authority. The supported
launch shape has one private IPv4 primary ENI and one encrypted gp3 root. Capacity,
AMI, type, network/profile IDs, key, tags and ceilings must be exact reviewed values.
No fixture represents production approval or deployed support.

The [projection policy](../../../docs/conformance/ag9g0d-run-instances-projection-v2.md)
defines the frozen logical fields. The pinned SDK audit documents their pure
projection; provider corroboration is not implemented. The collector configuration is non-executable JSON; there is no
collector binary. Trust checks encoding/digest/metadata only, never X.509 or CMS.
The local marker/genesis and bootstrap/status CLI are unchanged. Additional files
cannot make an authority capable of network access or mutation.

Targeted contracts: `cargo test --locked --offline --test contracts` from this
workspace with an external target directory. `tests/fixtures/README.md` documents
the independent synthetic vectors and their non-authoritative trust bytes.


Pass-2 cost review binds an explicit planned duration of 1–604,800 seconds and a
checked, rounded-up total hourly estimate (compute + root EBS + applicable other),
plus fixed operation costs, against the total ceiling. A distinct pricing-evidence
digest binds the reviewed rates and exact gp3 configuration; no AWS pricing formulas
or live pricing are implemented. Planned runtime is not timeout, stop or termination
authority, nor a spending guarantee. Cleanup ownership belongs to the operational
acceptance package. IAM unique IDs retain 16–128 ASCII letters/digits/underscore
bytes without prefix inference. EC2 resource suffixes retain their bounded opaque
lexical rules. The pinned maintenance input exposes no reboot-migration field;
there is no post-launch maintenance mutation fallback.


## Pass-3 local dispatch authority

`dispatch.rs` implements one unresolved operation with an explicit human
`LaunchAuthorizationV2`, one logical dispatch and at most three attempt receipts.
Before preparation publication, the controller retains all six canonical documents
in the protected local evidence store. The event holds only typed digest/length
references and compact bindings. Replay resolves and validates those exact bytes;
missing or corrupt evidence rejects. Orphans confer no authority and are never
auto-adopted or deleted. No external mutable source is needed for replay.

The dispatch window is derived as exactly 120 seconds from the preparation
envelope’s controller clock sample. Human authorization supplies audit metadata,
never the operational clock, deadline or retry timestamp. Only a durably
recorded `definitely-not-transmitted` outcome can permit another attempt, after
2 seconds for attempt 2 or 8 seconds for attempt 3. Pending/uncertain attempts,
parameter conflict, access failure, throttling hold and unresolved responses block
transmission. No outcome establishes an EC2 identity. Restart preserves the budget
and delays; reboot/time-namespace change invalidates remaining retry permission.
An intent without outcome never recreates a capability after restart.

Private, non-cloneable, non-serializable capability types are created only after
successful durable append. Raw request/receipt/token objects cannot substitute for
an attempt capability. The CLI remains bootstrap/status only; these are internal
library/storage paths with deterministic tests, not operational launch commands.
The #1396 SDK boundary consumes these identities for pure projection and adds no
allocation transmission path. #1402 has not started.

Preparation/dispatch/attempt intents require ordinary storage. Outcomes of consumed
attempts may use protected recovery capacity; this does not let retry intent consume
cleanup reserves. There is no automatic retry loop, migration, close, replacement,
provider mutation or S3 publication in this durable state machine. The separate
SDK boundary supplies explicit credentials only to read-only STS/S3 admission.

Targeted checks: `cargo test --locked --offline --test dispatch` and
`cargo test --locked --offline journal::tests` using an external target directory.
See the contract for exact timing, outcome and publication rules.


Pass-3 artifact/bound hardening keeps all journal shapes below an 8-KiB regression
budget. Maximum-value valid documents can collectively exceed 65,536 bytes, but
live as six individually bounded retained artifacts. The largest tested journal
shape is 4,023 bytes; conservative numeric-width expansion reaches 4,103 bytes.


## #1396 SDK boundary validation

Use Rust 1.92 with an external target directory. Targeted tests are
`cargo test --locked --offline aws::`; complete tests/build, formatting and Clippy
remain required. The synthetic HTTP connector never accesses AWS. All service
versions and 157 audited SDK members are frozen in the linked audit. #1402 is not
implemented. Historical pass descriptions above describe the frozen foundation;
current SDK behavior is specified by the #1396 audit.
