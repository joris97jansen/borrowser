# AG9g0e provider evidence vocabulary V1

Historical limitation: V1 `managed_operator` cannot preserve independent operator
management/principal/visibility facts, and its volume attachment requires InstanceId.
[AG9g0e1a V2](ag9g0e1a-management-observation-v2.md) corrects those representations
through explicit successor contracts. This V1 matrix and canonical vocabulary remain
frozen; missing V1 information cannot be reconstructed or relabeled as V2 evidence.

This freezes A01–A38 **inputs and evidence limits**, not an evaluator. E1 defines
SDK-independent representations; e2 supplies adapters, normalization, required-query
coverage and policy. E3 supplies durable evidence/publication/replay; e4 coordinates.

## Expectation / observation / evaluation boundary

The manifest defines reviewed expectations. Observations retain bounded provider
facts before e2 compares them with those expectations. No SDK adapters, permission
evaluation or conversion from observed facts into reviewed types exists in e1.

The complete audit of former manifest-type reuse is:

| Former reuse (including nested types) | Observation-side disposition |
| --- | --- |
| `Ipv4Cidr` | Shared: policy-neutral network/prefix; all canonical IPv4 networks remain representable |
| `Effect` | Shared only as the Allow/Deny literal inside `RuleActionObservation`; unrecognized action literals remain explicit |
| `ReviewedRoute`, `RouteAssociation` | Separate `RouteObservation` and `RouteAssociationObservation`: destinations/targets, exact state/origin, main/subnet/gateway associations; no derived private-route or supported-origin flags |
| `SecurityRule`, `Protocol`, `Peer` | Separate SG permissions/peers: protocol literal, independent signed endpoints, IPv4/IPv6/prefix-list/foreign-group facts, peering metadata; no same-account/VPC or ordered-range prerequisite |
| `EndpointPolicy`, `PolicyStatement`, `Principal`, `S3Action`, `PolicyResource`, `PolicyCondition` | Separate bounded policy structural evidence: literal actions/resources/principals/operators/keys/values, both positive and negated fields, explicit unsupported shapes; no wildcard expansion or condition evaluation |
| `NaclRule` (also `Protocol`, `Effect`, `Ipv4Cidr`) | Separate entries: returned number/direction/action/protocol, IPv4/IPv6, port and ICMP facts; no sorting, deduplication or required default-deny substitution |
| `DnsDhcpExpectation`, `DnsServers` | Separate returned DHCP option keys/values, including custom resolvers and unrecognized keys; VPC DNS attributes remain separate observations |

The shared primitives are lossless at the semantic value level and impose no
reviewed policy. `ProviderI32` also preserves negative sentinels and contradictory
numeric fields without changing the authority canonical encoder. Interface address
and prefix fields now use IPv4/IPv6 semantic types, including both prefix-list
address families; descriptive text, provider enum
spellings, protocol literals and DHCP values remain bounded literal text because
their meaning cannot be reduced to an approved value before evaluation.

Unknown route state/origin/action/principal-kind/DHCP-key variants carry explicit
bounded literals. Unknown protocol and condition-operator spellings remain in their
named fields for e2 classification. Unknown structural fields cannot be inserted as
arbitrary maps: e2 must record unsupported/malformed evidence and incomplete coverage.
Endpoint-policy markers name unsupported document/statement members or value shapes;
they do not retain arbitrary recursive payloads or imply a complete supported policy.
Statement/condition positions are zero-based within the observed vectors, with no
durable index or packing meaning. Unparseable structure uses explicit malformed or
unsupported coverage rather than fabricated positions.
Duplicate or simultaneously contradictory fields remain represented. E2 must not
select a compliant alternative and discard the others.

The [foundation](ag9g0e1-reconciliation-foundation.md) specifies unchanged record,
aggregate and policy bounds. No transport, manifest, context, storage or e3/e4
ownership changes accompany this vocabulary correction.

## Evidence matrix

Sources: D = retained deployment/support; A = approval/review/ceilings;
R = exact retained request/spec/tags/user data; J = replayed launch history;
M = exact digest-bound infrastructure manifest.

All required missing/unavailable evidence means unresolved. Affirmative contradiction
of reviewed infrastructure or operation-linked resources means conflict. Documented
absence of a prohibited feature may satisfy that negative predicate. None of these
dispositions is implemented by e1. No field silently defaults an omitted bool to false.

`Observed<T>` explicitly distinguishes Present, Absent and Unavailable. Unknown provider
enum strings can be preserved as bounded ProviderText; e2 must classify them explicitly.
`Unsupported` records preserve constructs that cannot fit the closed supported schema.
`ReadCoverageV1` binds a typed query and accounts requests/pages/records, terminal
response and incomplete reason. A resource is not unrelated simply because it fails
admission. Matching token, operation tags, retained identity or attachment linkage must
not be discarded to manufacture a singleton. These are requirements for e2.

The [AG9g0e2e0 proof audit](ag9g0e2e0-authoritative-infrastructure-absence.md) qualifies
the A04/A07/A10 negative facts without changing this matrix or its reviewed
expectations. A04 placement negatives and strict A10 IPv6 absence remain unresolved
on the audited evidence; A07 has narrow positive-witness conceptual inferences,
not synthetic member absence. Terminal coverage, `NotReturned`, empty values and
generic historical absence vocabulary must not be substituted for those proofs.

| Fact | Retained source / required predicate | Abstract observation fields | Audited reads |
| --- | --- | --- | --- |
| A01 | D/R account and non-root caller | Caller.account/arn/user_id | STS GetCallerIdentity |
| A02 | D/R region and regional access | Query account/region; Region.opt_in_status | DescribeRegions |
| A03 | D/R AZ name/ID/region | AvailabilityZone | DescribeAvailabilityZones |
| A04 | D/R/M subnet owner/VPC/AZ/IPv4 and compatible addressing/placement | Subnet, Unsupported.Addressing | DescribeSubnets |
| A05 | D/R VPC owner/default tenancy | Vpc.owner/tenancy | DescribeVpcs |
| A06 | D/R/M exact SGs, owners, VPC, complete ingress/egress | SecurityGroup, Unsupported.SecurityRule | DescribeSecurityGroups |
| A07 | D/M effective route table, association, complete private routes | RouteTable; Unsupported.Route | DescribeRouteTables |
| A08 | D/M exact S3 gateway endpoint/service/VPC/owner/routes/policy/prefix list | Endpoint, PrefixList, Unsupported.EndpointPolicy | DescribeVpcEndpoints, DescribePrefixLists |
| A09 | M DNS attributes and DHCP | Dns, Dhcp, Unsupported.DnsDhcp | DescribeVpcAttribute, DescribeVpcs, DescribeDhcpOptions |
| A10 | M effective NACL association and complete ordered rules | Nacl, Unsupported.Nacl | DescribeNetworkAcls |
| A11 | D/R bucket owner/region | Bucket.expected_owner/region | S3 HeadBucket with ExpectedBucketOwner |
| A12 | D/R exact customer-managed compatible KMS key/state | Key | KMS DescribeKey by exact key ARN |
| A13 | A/R AMI ID/owner | Image.id/owner | DescribeImages |
| A14 | A/R x86_64 Linux HVM EBS and root device | Image architecture/platform/virtualization/root fields | DescribeImages |
| A15 | A/R single EBS mapping, no extra/store mappings/product/billing features | Image.mappings/product_codes/platform_details/usage_operation | DescribeImages |
| A16 | R AMI root snapshot matches volume origin | ImageMapping.snapshot, Volume.snapshot | DescribeImages, DescribeVolumes |
| A17 | A/R type architecture/HVM/EBS/On-Demand/nonburstable/no store/accelerators/network topology | InstanceType capability fields | DescribeInstanceTypes |
| A18 | A/R default vCPU/memory and EBS capacities within ceilings | InstanceType CPU/memory/EBS fields | DescribeInstanceTypes |
| A19 | R type offered in exact AZ; absence is unresolved | TypeOffering | DescribeInstanceTypeOfferings |
| A20 | D/A/R exact profile and role ARNs/unique IDs | Profile, RoleIdentity | IAM GetInstanceProfile |
| A21 | R candidate's exact profile association | Instance.profile_arn/profile_id, Attachment.Profile | DescribeInstances, DescribeIamInstanceProfileAssociations |
| A22 | R/J token/account/region/AZ/subnet/VPC/AMI/type | Query context, Instance | DescribeInstances and infrastructure joins |
| A23 | R/A/J exactly one plausible InstanceId; zero unresolved, multiple conflict | Complete coverage and all Instance records | Complete bounded discovery union (e2) |
| A24 | R exact instance/ENI/volume tags | Instance/NetworkInterface/Volume.tags | Resource descriptions |
| A25 | R one ordinary primary ENI, owner/subnet/VPC/SGs/device/card | NetworkInterface, Attachment.NetworkInterface | DescribeInstances, DescribeNetworkInterfaces |
| A26 | R exact ENI attachment and delete-on-termination | Attachment.NetworkInterface | Both instance-side and ENI-side descriptions |
| A27 | R one primary IPv4 and no additional/public/carrier/customer-owned/EIP/IPv6/prefixes | NetworkInterface address/prefix/association fields | DescribeInstances, DescribeNetworkInterfaces |
| A28 | R/A exactly one root EBS at exact device/AZ/instance | Instance.root_device/volumes, Volume.zone, Attachment.Volume | DescribeInstances, DescribeVolumes |
| A29 | R/A gp3 exact size/IOPS/throughput/encryption/key, delete-on-termination, no multi-attach | Volume, Attachment.Volume | DescribeVolumes, instance mapping |
| A30 | R exact applied IMDS settings | InstanceOptions.metadata | DescribeInstances |
| A31 | R shutdown stop and exact stop/termination protection | InstanceAttributeValue | DescribeInstanceAttribute separately per attribute |
| A32 | R monitoring disabled/EBS optimization enabled | InstanceOptions.monitoring/ebs_optimized | DescribeInstances |
| A33 | R On-Demand/default tenancy/no capacity reservation/block | InstanceOptions lifecycle/tenancy/capacity fields | DescribeInstances |
| A34 | R hibernation/enclave/auto-recovery disabled | InstanceOptions | DescribeInstances |
| A35 | R hostname ip-name and A/AAAA disabled | InstanceOptions hostname/DNS fields | DescribeInstances |
| A36 | R/A CPU matches type defaults | Instance.cpu_cores/cpu_threads; InstanceType defaults | DescribeInstances, DescribeInstanceTypes |
| A37 | R exact collector user-data bytes including LF | InstanceAttributeValue.UserData | DescribeInstanceAttribute(userData), decode once in e2 |
| A38 | Frozen exclusions: key/kernel/ramdisk/licenses/accelerators/placement/host/Outpost/secondary ENI/delegation | ExcludedFeatures, instance interfaces, Unsupported.LaunchFeature | Corresponding instance/ENI/volume descriptions |

E2 must retain both sides of attachment observations and distinguish contradictory
duplicates. Record query identity supplies provenance without making response order
authoritative. Inventories distinguish OperationAssociated, SharedInfrastructure and
Reference, separately from Corroborated/Plausible/Conflicted confidence. Original
resource and evidence/history references are data, never cleanup capabilities.

## Explicit nonclaims

AWS descriptions do not return the Borrowser request digest or prove historical wire
omissions. IAM identity does not prove effective permissions or delivered credentials.
AMI metadata does not prove image contents, collector installation, host execution or
signed identity. Network configuration does not prove actual DNS or S3 traffic.
Prices, reviews, provenance and trust digests remain retained local facts. Reboot
migration and omitted ENA/connection-tracking defaults retain AG9g0d's nonclaims.

No observation can restore transmission permission, reset attempts/deadlines or
reconstruct a capability. The future binding transition is only Unbound → Bound(I).
Later disappearance, drift, termination, conflicting identity or another plausible
instance cannot erase/rebind I. Conflict history remains sticky. Those transitions
are deliberately absent from e1 code.
