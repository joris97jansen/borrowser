# AWS EC2 authority foundation and reviewed contracts — Passes 1–2

Independent Rust operational-tooling workspace. Read the
[contract](../../../docs/conformance/ag9g0d-qualification-host-lifecycle.md).
Package `0.2.0` exposes generation-2 local bootstrap/status only. Pass 2 adds pure
reviewed deployment/launch/token/trust data contracts to the library. It contains no
provider client, credential loader, network transport or resource mutation path.
AG9g0d remains operationally incomplete; no host readiness or qualification is proven.

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
tests exist in Pass 2. Synthetic storage-only event variants exist solely under
`cfg(test)` and are rejected by production-schema integration tests.

Allocation, termination, SDKs, identity evidence, S3 publication,
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
classifies fields for the future SDK audit. No SDK projection or provider validation
is implemented. The collector configuration is non-executable JSON; there is no
collector binary. Trust checks encoding/digest/metadata only, never X.509 or CMS.
The local marker/journal and bootstrap/status CLI are unchanged. Additional files
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
lexical rules. Reboot migration is excluded pending the Pass-4 pinned SDK input audit;
there is no post-launch maintenance mutation fallback.
