# Reviewed infrastructure manifest V1

`provider/manifest.rs` owns this closed reviewed-expectation representation. It is not
AWS policy JSON, a Terraform/CDK document or a general network/IAM evaluator.
The format is `borrowser-reviewed-infrastructure`, schema 1.

## Byte identity and binding

Maximum input is 65,536 bytes. Strict typed decoding rejects unknown/duplicate fields
and unknown variants. Encoding reuses the authority canonical codec: unsigned integers,
ASCII-sorted keys, compact UTF-8 JSON, exact control escaping and terminal LF. Input
must already equal that encoding. No whitespace or permissive JSON equivalence is
used when verifying the retained `InfrastructureDigest`.

`parse_bound` validates the complete manifest and deployment, hashes exact canonical
bytes, compares `reviewed_support.infrastructure_sha256`, and checks exact account,
region, VPC, subnet, route table, S3 endpoint, bucket and SG-set agreement. Role-session,
principal-role and encryption-key conditions also match reviewed support. The manifest
contains neither its own digest nor the enclosing deployment digest. The existing
review reference and review metadata remain in deployment.

## Closed supported representations

| Area | V1 representation |
| --- | --- |
| IPv4 CIDR | Unsigned network integer and prefix 0–32; host bits must be zero |
| Protocol | `all`; TCP/UDP inclusive u16 port range; ICMP optional type/code, where wildcard type requires wildcard code |
| Route association | `explicit-subnet` or `main-inheritance`, for the exact manifest subnet/table |
| Routes | Nonempty canonical set, at most 128: local IPv4 route(s) and exactly one reviewed S3 prefix-list/gateway-endpoint route |
| Route state/origin | Supported local route means active/CreateRouteTable; S3 route active/CreateRoute; other observed states/origins cannot normalize into these expectations |
| SGs | 1–5 unique SGs in ID order; complete ingress/egress canonical sets, at most 128 rules each |
| SG peers | IPv4 CIDR, exact same-account/same-VPC SG reference, or exact reviewed S3 prefix-list ID |
| DNS/DHCP | Exact DHCP-options ID, explicit VPC DNS support/hostname booleans, AmazonProvidedDNS and one lowercase DNS domain (253-byte total / 63-byte labels) |
| NACL | Exact NACL and subnet association; 1–128 entries per direction ordered by unique rule number; IPv4/protocol/allow-or-deny fields |
| NACL default | Last entry exactly 32767, deny all protocols, 0.0.0.0/0 |

Routes and rule/resource/condition/statement sets use ascending canonical-byte order;
SG IDs and ARN lists use lexical order. No constructor silently removes duplicates.
NACL ordering is rule-number ordering, not an unordered set or policy evaluation.
All collection limits also remain subject to the aggregate manifest byte limit.

The [AG9g0e2e0 absence audit](ag9g0e2e0-authoritative-infrastructure-absence.md)
records a compatibility decision still needed for A10: AWS can return IPv6
default-deny entries in applicable configurations, while this V1 expectation has
only IPv4 entries and no IPv6 exception. Observations must retain those entries;
they must not be filtered or merged by rule number to fit this manifest. A separate
contract issue must decide whether strict exclusion is intentional or a versioned
exception is needed. This note changes neither the expectation nor its validator.

## Endpoint policy representation

At most 16 canonical-order statements; whole typed policy at most 8 KiB. The target
bucket is the manifest bucket. Policy version corresponds to AWS `2012-10-17`.
Every statement has explicit `sid` (null or 1–128 alphanumeric bytes), effect,
principal, actions, resources and conditions. Empty action/resource/principal-role
sets reject. Sid is comparison metadata, not authority.

Principals are `any` or a sorted set of at most 16 same-account role ARNs.
Actions are exclusively:

* `s3:GetObject`, `s3:GetObjectVersion`, `s3:PutObject`;
* `s3:AbortMultipartUpload`, `s3:ListMultipartUploadParts`;
* `s3:ListBucket`, `s3:ListBucketMultipartUploads`, `s3:GetBucketLocation`.

These action names describe reviewed infrastructure; they do not enable any SDK call.
Only HeadBucket is executable in this issue's S3 boundary.

Resources are the bucket itself, a literal nonempty slash-terminated prefix (at most
256 ASCII alphanumeric/`/._-` bytes with no `.`/`..` path segment) followed by one
terminal wildcard, or the exact principal ingress resource
`ag9g0d/aws-ec2-v2/identity-ingress/${aws:userid}/*`.

Conditions are a closed set (at most eight):

| Typed condition | AWS structural meaning |
| --- | --- |
| secure-transport | Bool `aws:SecureTransport`, explicit true/false |
| principal-account | StringEquals `aws:PrincipalAccount`, exact account |
| principal-role | ArnEquals `aws:PrincipalArn`, exact reviewed role ARN |
| source-vpc | StringEquals `aws:SourceVpc`, exact VPC |
| source-endpoint | StringEquals `aws:SourceVpce`, exact endpoint |
| aws-kms-encryption | StringEquals `s3:x-amz-server-side-encryption`, `aws:kms` |
| encryption-key | StringEquals `s3:x-amz-server-side-encryption-aws-kms-key-id`, exact key ARN |
| role-session | StringLike `aws:userid`, exact role unique ID followed by `:*` |

The frozen ownership flow is AWS SDK response → e2 normalization into
`EndpointPolicyObservation` → comparison with reviewed `EndpointPolicy` → e2
evaluation. `EndpointPolicyObservation` retains provider facts; `EndpointPolicy`
remains the reviewed expectation. Normalization, including any explicitly documented
scalar/array conventions, does not establish compliance. E1 never parses an AWS
response. Negated fields, condition operators, variable forms, principal kinds and
wildcard expressions cannot be normalized away. Unsupported or malformed provider
structures remain explicit evidence with incomplete coverage according to the
observation contracts. Structural agreement with reviewed policy does not prove
effective IAM authorization, DNS resolution, traffic reachability or host credentials.

## Unsupported constructs and evidence

Reviewed IPv6, Internet/NAT/transit/peering/propagated routes, alternative route targets,
custom DNS resolvers and additional DHCP option families reject. This is a versioned
supported subset; it is not a provider catalogue or an assertion that those AWS
constructs are universally invalid.

Observed unsupported constructs must remain explicitly unsupported/incomplete evidence
in e2, with affirmative contradictions retained for policy evaluation. Missing fields
must never become an empty compliant set. No synthetic manifest or current deployed
infrastructure supplies implicit expectations.
