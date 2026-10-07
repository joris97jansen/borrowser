# AG9g0e2e0: authoritative infrastructure absence audit

This audit formalizes the evidence available for the A04, A07 and A10 negative
facts in the [provider evidence matrix](ag9g0e-provider-evidence-matrix-v1.md).
It belongs to the standalone host-lifecycle tooling in Milestone AG. It adds
regression coverage and interpretation constraints, **no production schema, adapter,
read or admission evaluator**. AG9g0e2e, full A01–A12 admission, parent AG9g0e2 and
Milestone AG remain outstanding.

The decision is to retain [V4](ag9g0e2b-ec2-infrastructure-observations.md) and the
[reviewed-subnet route successor](ag9g0e2d-reviewed-subnet-route-observation-v1.md)
unchanged. No newly established service fact requires an additional representation.
An inability to prove absence is a valid audit result, not permission to manufacture
positive evidence. The downstream evaluator must leave unsupported negative claims
unresolved; affirmative contradictory evidence remains independently usable.

## Source and implementation basis

Audited 2026-10-06 against the standalone workspace's locked `aws-sdk-ec2 1.237.0`,
`aws-smithy-xml 0.60.15`, EC2 Query API version `2016-11-15` and SDK behavior version
`2026-01-12`. No dependency was upgraded. Service documentation is an interpretation
basis, not a replacement for the pinned implementation or a captured AWS response.

In that EC2 crate, inspect `src/protocol_serde/shape_{subnet,route,network_acl_entry,
route_table,network_acl,route_list,network_acl_entry_list,describe_subnets,
describe_route_tables,describe_network_acls}.rs` and the corresponding `src/types/`
builders. Returned strings are independent `Option<String>` members: builders start
with `None`; a matched XML element sets `Some(try_data(...))`. Smithy XML's
`src/decode.rs::try_data` returns empty text for an empty element. A present list
element decodes to a vector, including an empty vector. These mechanics explain
representation, not infrastructure absence. The
[EC2 Query protocol](https://smithy.io/2.0/aws/protocols/aws-ec2-query-protocol.html)
does not turn these independent structure members into a service-level union or
certify that an unreturned member denotes an absent infrastructure feature.

Borrowser's protected path is `aws/observation_session.rs` →
`ec2_infrastructure_reads.rs` (or `reviewed_subnet_route_reads.rs`) →
`query_execution{,_core}.rs` → the bounded transport, pinned SDK and
`ec2_decode_integrity.rs` → `ec2_observation.rs` / `ec2_network_observation.rs`.
The integrity guard correlates owned XML occurrences with the decoded structure,
rejects duplicate monitored scalar members and unsupported monitored attributes
(including `xsi:nil`), and prevents lossy decoder behavior from certifying a page.
Earlier valid pages survive a rejected invocation; the rejected page is not
partially certified. These tests exercise that path, not an alternate decoder.

| Input/evidence | Preserved interpretation |
| --- | --- |
| Omitted string member / SDK `None` | `Ec2MemberV4::NotReturned`; no semantic absence claim |
| Explicit empty string, either XML spelling | `Empty`, distinct from `NotReturned` |
| Bounded interpretable string | `Present`; lexical validity alone is not service validity |
| Bounded lexically invalid typed string | `Malformed(raw)`; sibling facts survive |
| NUL / overlong string | `Unrepresentable(ContainsNul / TextBytes)`; independent siblings survive within existing bounds |
| Omitted nested collection/object/value | `ObservationValueV2::Unavailable(NotReturned)` |
| Explicit empty collection | `Present([])`; returned emptiness, not proof of the expected resource/configuration |
| Malformed XML / unsupported monitored attribute | Failed invocation with incomplete coverage, not an absent member |
| Complete execution and represented occurrence count | Coverage/accounting facts only; not semantic completeness of each record |

`ProviderText` deliberately accepts bounded nonempty literals, including unknown
ARN-like values, whitespace and the text `null`. Such strings must not acquire new
lexical restrictions for this issue. Malformed syntax at the XML boundary is
different from lexical `Malformed` on a typed CIDR or identifier. Historical
`ObservationValueV2::Absent` is not emitted by these projections and is not a
certificate for any fact in this audit. Historical validators remain unchanged.

## A04: no Outpost placement

**Claim and proof level:** the exact returned subnet is a regional subnet, with no
Outpost placement. This is a resource-level placement claim, not merely a missing
`outpostArn` member.

**Service basis:** AWS's
[data-residency guidance](https://docs.aws.amazon.com/wellarchitected/latest/data-residency-hybrid-cloud-services-lens/drhcsec05-bp01.html)
describes a resource-level null `OutpostArn` as indicating placement in the Region.
It does not define omission in this qualified `DescribeSubnets` XML response as
that authoritative resource-level null. The
[Subnet response contract](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_Subnet.html)
describes optional `outpostArn` but supplies no such bridge. An Outpost is homed to
an Availability Zone, so regional/VPC/AZ identity alone does not prove non-Outpost
placement ([Outposts architecture](https://docs.aws.amazon.com/outposts/latest/server-userguide/how-outposts-works.html)).

**Provider decision:** `Subnet.outpost` preserves omitted, empty, returned and
unrepresentable strings. Neither omission, empty text, literal `null`, nor rejected
`xsi:nil` is a resource-null certificate. Complete exact-subnet coverage, a matching
identity and all other subnet fields do not repair this gap. An affirmative Outpost
literal survives missing or malformed pool/other evidence; interpretation of the
literal belongs downstream.

**Outcome:** authoritative negative placement is **unproven** on this read surface.
V4 already expresses all available facts; no new type is justified. Downstream A04
must leave this negative check unresolved. Any proposed resource-null bridge or
additional read needs a separate source-backed contract issue first.

## A04: no customer-owned IPv4 pool placement

**Claim and proof level:** the exact subnet has no customer-owned IPv4 pool
association/placement, a resource-level fact about `customerOwnedIpv4Pool`.

**Service basis:** [Subnet](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_Subnet.html)
documents the optional pool and the launch-assignment flag separately.
[ModifySubnetAttribute](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_ModifySubnetAttribute.html)
treats the pool and assignment flag as one modifiable attribute and requires a pool
when enabling assignment. It does not guarantee the converse that disabling
assignment removes the pool. No audited source certifies omitted/empty pool as
authoritative absence.

**Provider decision and completeness:** `Subnet.customer_owned_pool` retains its
independent literal state even when the wire has `mapCustomerOwnedIpOnLaunch=false`.
That Boolean is not a V4-owned observation; adding it would not establish the
converse. Exact identity, terminal coverage and complete retained occurrences do
not upgrade `NotReturned` or `Empty`. An affirmative pool survives incomplete
Outpost evidence and vice versa.

**Outcome:** **unproven**, downstream unresolved. Existing V4 is sufficient to
retain the limitation. No new API, Boolean field or absence state is introduced.

## A07: exclusion of alternative route destinations

**Claim and proof level:** a represented route's destination is the affirmative
supported IPv4 CIDR or reviewed prefix list, excluding a different *conceptual*
destination for that same route. This is a record-level inference, distinct from
literal absence of other response members and from exclusion across a table.

**Service basis:** a route has a destination and target
([route-table concepts](https://docs.aws.amazon.com/vpc/latest/userguide/subnet-route-tables.html));
[CreateRoute](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_CreateRoute.html)
requires a destination CIDR or prefix list. IPv4 and IPv6 routes are separate.
These service concepts support a narrow positive-witness interpretation, not a
blanket optional-output-member rule. The
[Route response](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_Route.html)
and pinned decoder retain all three destination members independently:

| Wire | V4 destination |
| --- | --- |
| `destinationCidrBlock` | `ipv4` |
| `destinationIpv6CidrBlock` | `ipv6` |
| `destinationPrefixListId` | `prefix_list` |

**Provider decision:** a usable affirmative destination can identify the conceptual
category without rewriting competing members. Missing-only, empty-only, malformed,
unrepresentable or incompatible positive destinations cannot supply that witness.
An affirmative competing IPv6/CIDR/prefix-list member must survive, including beside
otherwise incomplete evidence. A prefix-list identifier alone does not certify
IPv4 contents; gateway endpoint prefix lists can be family-specific or dualstack
([gateway endpoints](https://docs.aws.amazon.com/vpc/latest/privatelink/gateway-endpoints.html)).

**Completeness and outcome:** subject identity, returned route fields and an
interpretable, nonconflicting record are required. To exclude prohibited destinations
throughout the relevant route collection, additionally satisfy every collection gate
below and classify every occurrence. V4 already carries the positive witness and
competing evidence. **Narrow conceptual inference is supported; literal unreturned
member absence is not.** No successor is required. Admission application remains
AG9g0e2e work; unsupported literal exclusions remain unresolved.

## A07: exclusion of alternative route targets

**Claim and proof level:** a represented route targets the actual supported local
destination or independently identified gateway endpoint, excluding a different
conceptual forwarding target for that route. Record-level target classification
may require a second, independently returned endpoint resource record.

**Service basis:** [CreateRoute](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_CreateRoute.html)
requires exactly one target resource in its input; this is not a tagged output
union. The route-table guide documents local routes, and
[ReplaceRoute](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_ReplaceRoute.html)
documents restoring the `local` target. A route's existence or origin alone is not
proof of a current local target. The gateway-endpoint guide documents automatic
service-prefix-list → endpoint routes. A `vpce-` prefix alone does not identify the
endpoint type, service, VPC, state or route-table association.

All twelve retained target members remain independent:

| Wire | V4 target |
| --- | --- |
| `gatewayId` | `gateway` (including actual `local` or endpoint literal) |
| `egressOnlyInternetGatewayId` | `egress_only_internet_gateway` |
| `natGatewayId` | `nat_gateway` |
| `transitGatewayId` | `transit_gateway` |
| `localGatewayId` | `local_gateway` |
| `carrierGatewayId` | `carrier_gateway` |
| `vpcPeeringConnectionId` | `vpc_peering_connection` |
| `coreNetworkArn` | `core_network_arn` |
| `odbNetworkArn` | `odb_network_arn` |
| `instanceId` | `instance` |
| `networkInterfaceId` | `network_interface` |
| `ipAddress` | `ip_address` |

The [Route API](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_Route.html)
also includes `instanceOwnerId`, state and origin, retained outside `targets`.
They are auxiliary metadata. Instance and network-interface references can describe
associated aspects of forwarding; `ipAddress` describes a Route Server next hop.
Do not count every nonempty field as a different mutually exclusive target, or
erase any of them using conceptual exclusivity.

**Provider decision, completeness and outcome:** a returned `gatewayId=local`, or
an endpoint-target literal plus independently returned endpoint classification,
provides a positive witness; preserve all competing/associated fields for subsequent
interpretation. Unknown literal targets, missing classification, malformed or
incompatible evidence must not be turned into a supported target. Same collection
gates as destinations apply to whole-table exclusion. **Narrow conceptual inference
is supported; blanket literal member absence is not.** Existing V4 and both route
carriers suffice. This issue tests witness preservation, not target admission.

## A10: per-entry IPv6 member absence

**Claim and proof level:** one NACL entry has no prohibited IPv6 field/evidence.
This is a member-level claim; positive classification of an IPv4 rule is a distinct
record-level fact.

**Service basis:** the
[NetworkAclEntry API](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_NetworkAclEntry.html)
has independent optional CIDR fields. AWS describes IPv4 and IPv6 rules as separate
([custom ACLs](https://docs.aws.amazon.com/vpc/latest/userguide/custom-network-acl.html)).
That does not make omission an authoritative literal absence certificate. Even
protocol interpretation must remain independent:
[CreateNetworkAclEntry](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_CreateNetworkAclEntry.html)
documents protocol 58 with IPv4 CIDR as well. Do not derive IP family solely from
protocol, nor absence of IPv6-related evidence from an IPv4 CIDR.

**Provider decision and completeness:** `NaclEntryV4.ipv4`, `ipv6`, `protocol` and
`icmp` remain independent. `ipv6=NotReturned`, `Empty`, typed `Malformed` and
`Unrepresentable` differ even beside a valid IPv4 rule. Missing ICMP objects and
empty objects with unavailable children also differ. Complete collection accounting
does not fill these member gaps; both CIDRs and returned ICMP evidence survive.

**Outcome:** literal negative IPv6-member proof is **unproven**, downstream unresolved.
No new representation is justified; positive IPv4 family semantics do not answer
this stronger claim.

## A10: collection-wide exclusion of IPv6 entries/evidence

**Claim and proof level:** no prohibited IPv6 entry/evidence occurs anywhere in the
effective ACL's relevant complete rule collection, in both directions. This is
collection-level; an IPv4 entry says nothing about its siblings or later pages.

**Service basis:** [NetworkAcl](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_NetworkAcl.html)
exposes entries and subnet associations. AWS documents separate IPv6 rules,
including IPv6 default-deny entries in applicable configurations
([default ACLs](https://docs.aws.amazon.com/vpc/latest/userguide/default-network-acl.html)).
Their presence is evidence, even when the same rule number occurs for IPv4 and IPv6.

**Provider decision and completeness:** apply all collection gates below to the
actual effective ACL and its returned associations. Retain every entry, direction,
number, protocol, action and both CIDRs. Neither one IPv4 entry, terminal pagination,
nor equal source/retained counts proves the claim. An omitted `entrySet` is
unavailable; `entrySet` with zero items is returned emptiness, not a valid witness
for the retained nonempty default-deny expectation. If classifying any entry would
require the unsupported member-absence inference above, the strict negative claim
remains unresolved. Positive IPv6 evidence remains visible beside unknown entries,
malformed siblings, duplicate rules or incomplete later pages.

**Outcome:** **no general positive proof for the strict retained exclusion** is
established by this audit. Existing V4 faithfully retains the limitation and
contradictions; no absence state or normalizing filter is added.

## Collection gates and route-carrier compatibility

Any future collection-wide conclusion needs all of the following, not just a
successful SDK call:

1. Correct returned subject identities and associations, with each observation's
   actual query provenance. Request/manifest identities never fill returned gaps.
2. Bounded, structurally qualified decoding and terminal, complete query coverage
   without failed/rejected pages. `nextToken` is an execution fact, not feature
   absence ([DescribeSubnets](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeSubnets.html),
   [DescribeRouteTables](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeRouteTables.html),
   [DescribeNetworkAcls](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeNetworkAcls.html)).
3. Retained occurrence accounting: one root plus each route/association or
   entry/association occurrence. Duplicates and empty items consume credit. V4's
   frozen minimum-count validator intentionally permits partial evidence;
   discovery's exact retained-count audit detects deficits against supplied coverage.
4. Available nested collections and sufficient facts to classify **every** occurrence.
   Count equality can hold with missing fields or a missing nested collection, which
   contributes zero known occurrences. Representation completeness is not semantic
   completeness, authenticity or admission.
5. Independent handling of duplicates, ordering and incompatible repeated observations.
   Nested ordering is retained, outer carrier ordering is canonicalized, and no
   latest-wins, set collapse or substitution of a compatible observation is allowed.

The exact-table path retains `ObservationRecordV4` with its exact query; the
reviewed-subnet path retains `ReviewedSubnetRouteRecordV1` and its filtered query.
Identical payload data does not make these provenances interchangeable. Both use
the same decoder/normalizer and existing bounds (128 nested members, 16 KiB record,
16 pages/query; shared 128 requests/coverage, 4096 source/output occurrences and
256 KiB normalized evidence). Their coverage and deficit checks remain separate.
Mixed V2/V3/V4/V5 carriers, successor encodings and frozen V4 bytes/hashes/validators
are unchanged. Neither path authenticates an atomic resource snapshot; EC2 is
[eventually consistent](https://docs.aws.amazon.com/ec2/latest/devguide/eventual-consistency.html).

## A10 expectation compatibility and follow-up boundary

The [reviewed V1 manifest](ag9g0e-reviewed-infrastructure-v1.md) uses `NaclRule`
with `Ipv4Cidr`, rule number, effect and protocol, and `NaclExpectation` with ordered
ingress/egress lists. Each direction has 1–128 entries, unique increasing numbers
1–32767 and a final 32767 deny/all/0.0.0.0/0 rule. It has no IPv6-entry representation
or exception for AWS IPv6 default deny. A strict complete-rule comparison therefore
cannot silently accommodate such returned IPv6 entries.

Smallest separate contract issue: **Define reviewed A10 treatment of AWS IPv6
default-deny entries.** Decide whether strict IPv4 structural exclusion is intentional
or a precisely specified IPv6 deny exception is required; version the reviewed
expectation if necessary. Do not resolve this through observation normalization,
rule-number deduplication, ignoring IPv6 or implementing traffic/effective-policy
analysis. This audit records the mismatch without revising the expectation.

For A04, any attempt to make positive admission possible needs a separately justified
service/protocol resource-absence contract (and a separate read issue if needed),
or reconsideration of the retained expectation. No known additional read is claimed
to solve the gap. Broader observation redesign and generic AWS absence abstractions
are outside this issue.

## Regression coverage and deliberate exclusions

`aws/ec2_infrastructure_absence_tests.rs` uses the actual pinned decoder and protected
readers for omitted/empty/present/malformed/unrepresentable members, permissive text,
unsupported XML, positive route witnesses and retained competing evidence, missing
versus empty collections, IPv6 default deny, duplicates, permutations and failed
later pages. Test source comments link to the relevant service basis. These tests
assert provider evidence, never a test-only admission evaluator.

`aws/reviewed_subnet_route_reads_tests.rs` compares exact/successor data while
preserving query identities and incomplete coverage. `aws/operation_discovery_boundary_tests.rs`
checks infrastructure occurrence deficits, complete-but-unavailable evidence and
incompatible repetitions without semantic promotion. `tests/provider_v4_contracts.rs`
checks independent canonical state identity, literal permissiveness and contradiction
retention; existing frozen fixtures and mixed-carrier tests remain unchanged.

Excluded: AG9g0e2e A01–A12 evaluation; A13–A38; candidate selection; reconciliation;
durable provider identity; publication/replay; effective permissions; traffic proof;
manifest revision; new reads/discovery/retries; SDK upgrades; generalized absence
schemas; CSS, layout, paint, parser and browser runtime changes.
