# AG9g0e1 bounded read SDK audit

AG9g0e2a1 adds [narrow V3 partial identity observations and private IAM presence capture](ag9g0e2a1-partial-identity-observation-v3.md). V1/V2 contracts and SDK pins remain unchanged; production adapters and query execution remain #1413 work.

AG9g0e1a adds [pinned EC2 management/delegation protocol regressions and pure field normalization](ag9g0e1a-management-observation-v2.md).
The pins, read inventory and transport configuration below are unchanged. No current
AG9g0e1a normalizer emits semantic Absent, including for managed EBS attachments.
Full service adapters and query execution remain e2-owned.

This extends the [AG9g0d audit](ag9g0d-aws-sdk-boundary.md) without changing its
RunInstances projection or enabling transmission. E1 adds private clients and
synthetic protocol tests only; production service adapters belong to e2.

## Exact sources and dependency consequences

| Addition | Version | Archive SHA-256 |
| --- | --- | --- |
| aws-sdk-iam | 1.113.0 | 90e401e73e324d4b5ce6f435cf913fdadc86e99785f2020251bccc6ece45f7d6 |
| aws-sdk-kms | 1.111.0 | c62d6d9b9c60e08be622a6bfc98e483d75683795caaf9704886970aa07ed8529 |

Both declare Rust 1.91.1, compatible with the required Rust 1.92.0 compiler. Both use
`default-features=false`, `rt-tokio`; neither introduces a default client factory.
Cargo resolution adds only these two packages: every pre-existing locked version and
checksum remains unchanged. Both use the existing aws-runtime 1.7.5, Smithy runtime
1.11.3, runtime-api 1.12.3, types 1.5.0, async 1.2.14, HTTP client 1.1.13, HTTP 0.63.6,
JSON 0.62.7, observability 0.2.6 and aws-types 1.3.16. IAM additionally uses existing
query/XML 0.60.15. Existing EC2/STS/S3 pins and BehaviorVersion v2026_01_12 remain fixed.

Existing transitive `bytes=1.12.1`, `http-body=1.1.0`, `http-body-util=0.1.5` become
direct exact dependencies. Tokio 1.53.1 gains direct production rt/time use, while
macros and deterministic paused-time support remain test features. Smithy types
explicitly enables http-body-1-x, already required by the locked service graph.
No aws-config, new TLS backend, policy evaluator
or network stack is added. Rust 1.92 locked/offline validation remains mandatory;
declared MSRV alone is not build evidence.

## Endpoint, signing and credentials

Private IAM/KMS clients derive from the existing explicit SdkConfig; FIPS and dual-stack
are explicitly false. Synthetic protocol tests verify these ordinary endpoints:

| Region | IAM endpoint / signing region | KMS endpoint / signing region |
| --- | --- | --- |
| eu-central-1 | iam.amazonaws.com / us-east-1 | kms.eu-central-1.amazonaws.com / eu-central-1 |
| us-gov-west-1 | iam.us-gov.amazonaws.com / us-gov-west-1 | kms.us-gov-west-1.amazonaws.com / us-gov-west-1 |
| cn-north-1 | iam.cn-north-1.amazonaws.com.cn / cn-north-1 | kms.cn-north-1.amazonaws.com.cn / cn-north-1 |

Signing names are iam and kms. Generated endpoint rules live in each exact crate's
`src/config/endpoint.rs` and endpoint-library sources; operation setup lives in
`src/operation/get_instance_profile.rs` and `src/operation/describe_key.rs`.
These transport checks do not grant region/partition admission. E2 must corroborate
the reviewed account/region and reject unsupported evidence; no alternate region,
partition or endpoint fallback is authorized by this contract.

Credentials remain the explicit, expiring, protected external session file from
AG9g0d. There is no ambient aws-config/profile/endpoint/proxy discovery, AssumeRole,
refresh or S3 CreateSession. TLS verification and disabled proxy discovery remain in
the explicit AWS-LC/rustls HTTPS builder. SDK max attempts remains one, standard mode;
connect 5 seconds, operation/attempt 30 seconds. The shared observation round further
restricts remaining time and expiry; extending a session cannot reset a round.

## Audited operation inventory and members

The exact allowlist is retained independently in
`tests/fixtures/read-sdk-surface-v1.json` and compared with `aws/read_surface.rs`.
It comprises STS GetCallerIdentity, S3 HeadBucket, IAM GetInstanceProfile, KMS DescribeKey
and the EC2 Describe operations listed in the [evidence matrix](ag9g0e-provider-evidence-matrix-v1.md).
The inventory is not a dispatcher or a general arbitrary-action transport interface.

| New request member | Disposition |
| --- | --- |
| GetInstanceProfileInput.instance_profile_name | Exact final profile-name component derived from reviewed ARN in e2; never name-based discovery |
| DescribeKeyInput.key_id | Exact reviewed key ARN; aliases and discovery forbidden |
| DescribeKeyInput.grant_tokens | Explicit omission; no grant authority |

Those are the complete request structs in the pinned sources. IAM's response profile
ARN/ID and Roles ARN/ID are the corroboration inputs; profile name/path are returned
identity metadata, not substitutes for exact ARN. CreateDate, tags, role trust policy,
permissions boundaries and other role metadata do not establish effective permissions.
KMS KeyMetadata ARN/account/manager/spec/usage/state are inputs; alias, grant, rotation,
key-policy and cryptographic operations are excluded. Unknown enums remain unsupported
evidence rather than compliant defaults. Additional response members do not become
authority by being present.

IAM is AWS Query XML (Action=GetInstanceProfile, Version=2010-05-08). KMS is AWS JSON
with X-Amz-Target=TrentService.DescribeKey. Synthetic tests assert exact request scope,
signing, no GrantTokens, successful generated deserialization and service-error handling
with one request. No SDK output is converted into provider observations here.

For EC2, e2 must use exact reviewed IDs and the separately specified discovery scopes,
not over-constrained admission filters. DescribeInstanceAttribute allows userData,
instanceInitiatedShutdownBehavior, disableApiTermination and disableApiStop. VPC
attributes are enableDnsSupport/enableDnsHostnames. Pagination follows only returned
tokens under the per-query/round bounds; API-valid page-size selection and execution
are e2-owned. No autogenerated paginator is invoked in e1.

## Executable body boundary

Pinned source evidence:

* runtime-api 1.12.3 `client/http.rs`: HttpClient selects a SharedHttpConnector whose
  `call` returns HttpResponse; this permits a private decorator before SDK processing.
* types 1.5.0 `body/http_body_1_x.rs`: SdkBody implements Body and permits frame polling.
* runtime 1.11.3 `client/orchestrator.rs`: response processing invokes streaming
  deserialization or calls `read_body` before nonstreaming deserialization.
* runtime 1.11.3 `client/orchestrator/http.rs`: `read_body` collects the supplied body.

Borrowser therefore returns **no response to Smithy until successful EOF**:

```text
original HTTP stream → connector frame polling → check capacity → bounded copy
→ successful EOF/clock check → completed bounded SdkBody → Smithy deserialization
```

`BoundedHttp` is the only argument accepted by application SDK configuration. It wraps
both production and synthetic connectors and delegates connector settings, validation
and metadata. The underlying connection pool/TLS behavior remains SDK-owned.

Before copying every data frame, checked accounting enforces 1 MiB per response and
8 MiB per round. All service clients share the same round; creating another connector
does not refund previous bytes. Buffer reservation depends only on checked accepted
lengths, with capacity capped at the per-response budget. No unbounded collect precedes
the check. Oversized successful and error responses fail at the connector and cannot
reach protocol deserialization, including operations that might ignore a body.

Content-Length and body size hints can conservatively reject but never accept or
drive allocation. Malformed/duplicate lengths, conflicting transfer framing,
non-identity content encoding, unsupported transfer encoding and trailers reject.
Absent or understated lengths cannot bypass actual frame accounting. An exact-limit
body succeeds only after valid EOF. Rejected/error/cancelled streams are dropped,
never drained. Async cancellation latches the round so it cannot be reused as a fresh
budget; accepted bytes remain charged. Bounded processing yields periodically so
an endless immediately-ready stream cannot starve cancellation.

The guarantee concerns bytes accepted/copied here. TLS/HTTP may already have buffered
an offending frame before presenting it. This is not a bound on all network-stack
allocations or headers. Smithy's subsequent collection operates only on the completed
bounded byte sequence; the original streaming body never escapes this boundary.

## Validation evidence required

Frame probes check successful EOF, exact/one-over bodies, misleading/absent length and
size hints, shared budgets, body errors, cancellation, trailers and no drain. The
pinned SdkBody adapter polls data EOF and trailer EOF separately; tests distinguish
that from consuming additional data. SDK tests assert DispatchFailure rather than
protocol/service parsing for oversized success/error bodies across all five services.
Small IAM XML/KMS JSON responses demonstrate working generated protocol paths.
A separate test exhausts the shared round with bounded KMS responses and proves
the next IAM response cannot enter deserialization. The deserialization interceptor
also has a positive control, so a zero count cannot pass through an inert probe.

These tests use no real credentials or AWS network. Linux/container tests and native
tests use the standalone lockfile and Rust 1.92. E3/e4 must separately prove actual
lock release, fresh replay, stale rejection and end-to-end concurrency. No such
publication/controller implementation exists in this issue.

## Infrastructure observation successor

[AG9g0e2b / #1414](ag9g0e2b-ec2-infrastructure-observations.md) implements the eleven
closed infrastructure readers on this bounded transport and e2a execution machinery.
It adds SDK-independent V4 facts, invocation-scoped EC2 structural correlation and
shared-session composition. No pins, transport budgets, admission or mutation surface
change. The linked contract records generated-decoder source evidence separately
from executed unprotected probes and protected protocol regressions.

## Allocation observation successor

[AG9g0e2c / #1415](ag9g0e2c-ec2-allocation-observations.md) adds eight closed
allocation readers, V5 source-specific facts and opaque user-data bytes on the same
pinned transport/session. Its contract specifies exact request scopes, the 100-name
instance-type boundary, visibility flags, invocation-qualified positional source
credit, independent output/byte bounds and the complete owned SDK member inventory.
The SDK remains the semantic decoder; the userData AttributeValue String receives
one API Base64 decode at the adapter. No historical schema, SDK pin, shared evidence
ceiling, launch authority or controller/publication surface changes.
