# AG9g0e2b / #1414: bounded EC2 infrastructure observations

This issue delivers the eleven infrastructure readers together with explicit V4
successor evidence, shared-session composition and pinned-decoder integrity checks.
It belongs to host-lifecycle tooling under **AG — Web Platform Test Harness and
Cross-Engine Conformance Infrastructure**. It does not complete AG, establish
infrastructure admission or change browser-engine behavior.

## Ownership and compatibility

`tools/conformance/host-lifecycle/src/provider/{ec2_observation_v4,
network_observation_v4,endpoint_policy_observation_v4,observation_v4,evidence_v4}.rs`
own SDK-independent representation, structural validation and canonical identity.
`aws/ec2_{infrastructure_reads,observation,network_observation,endpoint_policy,
decode_integrity}.rs` own requests, SDK projection, policy interpretation and protocol
integrity. `aws/observation_session.rs` composes the existing identity readers and
these EC2 readers using one explicit SDK configuration and one observation round.

V1/V2/V3 serialized schemas, identities, validators and fixtures stay frozen. A V4
record has `schema_version: 4`, a V1 query identity and one of the eleven V4 data
variants. It uses the existing canonical encoder and 16 KiB record ceiling; its
observation-only `EvidenceIdentityV4` hashes those bytes. There is no V4 inventory,
context, reference resolution or publication API. The in-memory, nonserializable
`ObservationEntryV4` / `ProviderObservationV4` carrier accepts V2 STS/S3 and other
unsuperseded V2 records, V3 identity records and V4 EC2 records. It rejects superseded
V2 identity/EC2 variants without changing their historical standalone validators.
The aggregate requires sorted canonical records, permits equal duplicate records,
and validates per-query occurrence credit, coverage uniqueness and shared ceilings.

Production construction binds the reviewed manifest to retained deployment authority
with `ReviewedInfrastructureV1::parse_bound`, starts the Linux observation clock
before reading the explicit session file, and uses the existing bounded transport.
It does not expose clients, request callbacks, ambient credentials or endpoint
configuration. Selection is a closed `InfrastructureRead` enum; there is no scheduler
or required-read derivation. Separate DNS selectors share a client and budget but
produce separate query identities and independently returned facts.

The [e2a execution contract](ag9g0e2a-identity-query-execution.md) remains authoritative
for legal typed scopes, request/page accounting, shared limits, cancellation, failure
precedence, query leases, continuation cycles and reserved failure coverage. The only
executor changes are mixed-carrier dispatch and V4 occurrence/canonical accounting.
`ContinuationToken::parse` and generic empty-token rejection remain unchanged.

## Owned response surface and C1–C7

All rows use exact retained/manifest identifiers, except the explicitly regional
Regions/AZ queries. No identity is copied from the request, enclosing resource or
reviewed expectation into returned evidence. Unknown bounded enum literals and
contradictory combinations are observations, not compliance decisions.

| Operation | Request and owned returned facts | Successor correction |
| --- | --- | --- |
| DescribeRegions | Regional, AllRegions=true; region name, opt-in status | C1 partial name |
| DescribeAvailabilityZones | Regional, AllAvailabilityZones=true; name, zone ID, region, state | C1 partial name/ID |
| DescribeSubnets | Exact subnet; ID, owner, VPC, zone/name/ID, IPv4 CIDR, IPv6-native/assignment/public-IPv4 flags, Outpost ARN, customer-owned pool | C1 independent partial identities |
| DescribeVpcs | Exact VPC; ID, owner, tenancy, DHCP options ID | C1 |
| DescribeSecurityGroups | Exact reviewed group IDs; ID, owner, VPC, ingress/egress rule occurrences, protocol and ports; independent group, IPv4, IPv6 and prefix-list collections | C1, C2 independent collection presence; C3 malformed typed peers |
| DescribeRouteTables | Exact table; ID, owner, VPC; every route destination/target, instance owner, state/origin; every association field | C1, C3; C4 association ID, returned table/subnet/gateway, main flag, public IPv4 pool, state and status message |
| DescribeVpcEndpoints | Exact endpoint; ID, owner, VPC, service, type, state, route-table ID occurrences, bounded policy structure | C1, C3, C5 duplicate policy members |
| DescribePrefixLists | Exact prefix list; ID, name, every IPv4/IPv6 CIDR occurrence | C1, C3 |
| DescribeVpcAttribute | Exact VPC plus enableDnsSupport OR enableDnsHostnames; returned VPC and both independently returned BooleanAttribute objects/values | C1; separate invocation provenance; no merge/inference |
| DescribeDhcpOptions | Exact DHCP options; ID, owner, every key/configuration and value-object occurrence | C1, C6 omitted value inside a present value object |
| DescribeNetworkAcls | Exact ACL; ID, owner, VPC; all associations and entries, direction, rule number/action/protocol, both CIDRs, port-range and ICMP objects/members | C1, C3; C7 association ID, independently returned child ACL ID and subnet ID |

Group peers retain returned account, VPC, group ID/name, description, peering ID and
status. CIDR/prefix peers retain descriptions. Routes retain all three destination
fields concurrently and all twelve target fields: gateway, egress-only gateway, NAT,
transit, local and carrier gateways, peering connection, core-network ARN, ODB-network
ARN, instance, network interface and IP address. Multiple destinations/targets,
reversed ports, negative signed values and future enum literals are not normalized
into reviewed route/rule expectations. Fields outside this owned surface (for example
tags and creation timestamps) are not new evidence requirements.

## Field states and coverage

`Ec2MemberV4<T>` is a closed `kind`/`value` enum. Its typed Present payloads use the
existing lexical ID/CIDR types; a Malformed payload must fail that type's parser.
Literal text fields cannot claim Malformed or Present(empty).

| SDK/integrity-qualified fact | Representation | Coverage contribution |
| --- | --- | --- |
| Omitted string member, including a resource ID | NotReturned | None: represented omission, not repaired identity |
| Explicit empty string | Empty | None: distinct returned value, not semantic absence |
| Valid bounded typed/literal value | Present(value) | None, including contradictions/future enum text |
| Bounded nonempty invalid typed value | Malformed(original decoded text) | Incomplete(Malformed); same-record and other valid records survive |
| NUL in a string | Unrepresentable(ContainsNul) | Incomplete(Malformed); marker and fitting siblings survive |
| Overlong field | Unrepresentable(TextBytes) | Incomplete(Limit(RecordBytes)); fitting records retained before round latch |
| Omitted optional object/list/numeric member | Unavailable(NotReturned) | None; never synthesize an empty list or Boolean |
| Present empty collection | Present([]) | None; independent of other collections |
| Present partial object/list item | Present object with member states | Charge and retain the occurrence and valid siblings |
| Missing top-level result collection | No fabricated resource record | Incomplete(Malformed), even if decoded token is terminal |
| Missing selected DNS object/value | Keep actual returned VPC and both attribute states | Incomplete(Malformed) with this invocation's provenance |
| Unsupported policy member/value structure | Closed name/shape or shape marker, with supported siblings | Incomplete(Unsupported) |
| Empty/invalid policy JSON | Empty/Malformed policy state; endpoint siblings retained | Incomplete(Malformed) |
| Unrepresentable collection/record/policy bound | No truncated overflowing record | Incomplete(existing Limit); prior accepted records/pages remain |
| XML integrity or generated scalar-decoder failure | No records from that unqualified invocation | Incomplete(Malformed), nonterminal; prior pages and actual request/body charges remain |
| Ordinary non-success service response | No fabricated success facts | Existing AccessDenied/NotFound/SessionExpired/Service classification |

NotReturned and Empty complete the observation of a member, as in the V3 identity
contract; Complete does not prove resource existence, validity, compliance or admission.
Malformed typed members explicitly differ from valid contradictory facts. A query-local
failure stops that query; a bound/session/time/cancellation latch stops the shared round.
An already latched reason takes precedence. A decoded, integrity-qualified terminal
page can be terminal and incomplete after normalization; a decoder failure cannot.

Page normalization keeps the first pending representation limit, promotes a limit
over a query-local failure, and otherwise keeps the first failure. Conversion errors,
fact-validation errors and the selected DNS attribute check use this same rule.
Combining failures does not latch the round: fitting evidence first passes through
e2a's existing retain-before-normalization-limit path. A later Malformed failure
cannot erase a pending limit; an already latched round/session/time reason still wins.

## Bounded endpoint-policy occurrences

Projection reads sequential borrowed `serde_json::value::RawValue` slices with a
bounded `MapAccess` visitor. It never collects input members into a unique-key map
or retains a general JSON AST. Repeated Version, Id, Statement, Sid, Effect,
Principal/NotPrincipal, Action/NotAction, Resource/NotResource and Condition members
retain their original occurrence order. Principal names, condition operators and
condition keys also retain duplicates. Scalar and array forms remain distinct.
String/Boolean condition values retain their type. Unsupported null, number (including
out-of-f64-range literals), object or array positions keep a closed shape marker.
Unknown member names remain bounded member states beside the value-shape marker;
the unknown subtree's recursive payload is deliberately not retained or evaluated.
The bounded preflight validates string decoding as well as container syntax: the
pinned serde_json RawValue skip path accepts lone UTF-16 surrogates that String
decoding rejects. Executed regressions require these values and keys to become a
Malformed policy while retaining endpoint siblings, never a projection panic.
Malformed JSON has no reliable member structure and retains only the policy-state
marker, not arbitrary raw text disguised as structured evidence.

The public representation is fixed depth. Documents and statements serialize directly
as occurrence arrays. Text/literal and selected collection containers use disjoint
JSON forms: closed member-state objects, arrays, Booleans where allowed, or closed
unsupported-shape strings. Other policy enums have closed kind/value tags. These V4
choices keep the deepest supported policy within the unchanged canonical depth-16
ceiling. Round-trip tests include duplicate nested conditions and unsupported shapes.

The existing aggregate allowances remain: 16 statements across repeated Statement
members; per statement, 16 principal entries/identities, 8 actions including NotAction,
16 resources including NotResource, and 8 conditions across repeated Condition
members. Each repeated structural collection also fits the existing 128-element
ceiling. Sid is at most 128 bytes, ordinary text 2,048 bytes, domain-name DHCP values
253 bytes, and the canonical observed policy 8 KiB. Splitting peer collections shares
one 128-peer allowance per SG rule; repeated policy keys cannot create fresh allowances.
No deduplication, truncation, budget increase or effective-policy evaluation occurs.

The private `check_policy_bytes` boundary encodes only the closed V4 projection.
Its derived serializers have static ASCII schema keys, string/Boolean values and
fixed depth within the frozen encoder's depth allowance. Returned names are values,
not schema keys; arbitrary JSON numbers and recursive structures never enter this
encoding. Consequently its reachable encoder failure is the general 64 KiB byte
ceiling, which already exceeds the policy's 8 KiB ceiling. Both this failure and a
successful encoding larger than 8 KiB yield `Limit(RecordBytes)`. This is a local
representation boundary, not a change to the frozen encoder or a relabeling of raw
JSON syntax or validation failures.

Executed protected-adapter regressions use two, four and forty repeated Version
members containing 2,048 ASCII characters each. Two fit; four exceed 8 KiB; forty
would exceed the encoder's 64 KiB ceiling. Both oversized cases retain earlier-page
evidence and actual occurrence charges, then latch the shared round against a later
identity request. Mixed-record regressions also retain a preceding TextBytes marker
and fitting sibling fields/records. Assertions cover request/page/terminal state,
coverage, exact occurrence and canonical accounting, and the latched reason. A direct
failure-combination regression independently covers a pending limit followed by
Malformed. Empty/invalid JSON remains query-local Malformed without a round latch.

The structural XML pass counts every owned repeated item, including partial/duplicate
items, before accepting a page. A DNS invocation counts one response record. Bounded
policy traversal additionally counts each JSON value and member-name occurrence,
including unsupported subtrees inspected within the limits. It checks the round clock
while traversing. The provider computes a minimum occurrence requirement independently
from retained V4 structure; actual work can exceed that minimum. Counts are submitted
to the original shared Records allowance before canonical records are accepted. If
structural traversal itself exceeds its bound, the invocation fails closed rather than
claiming complete decoded evidence. If later normalization hits a bound, preceding
bounded records remain and no overflowing record is partially retained.

## Pagination: service representation versus logical continuation

The API documentation below was checked on 2026-10-03. Each linked operation describes
its own terminal convention; the seven null descriptions do not specify that an
explicit empty XML element is equivalent to null. Rejecting their empty tokens is the
unchanged local executor rule, not an inference that AWS documentation forbids them.

| Operation / documented terminal | Omitted element | Explicit empty element | Nonempty element | Ambiguous/malformed monitored structure |
| --- | --- | --- | --- | --- |
| [DescribeSubnets](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeSubnets.html): null | SDK None → terminal | SDK Some("") → unchanged Malformed | Unchanged token validation/cycles | Integrity failure, nonterminal |
| [DescribeVpcs](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeVpcs.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |
| [DescribeSecurityGroups](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeSecurityGroups.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |
| [DescribeRouteTables](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeRouteTables.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |
| [DescribeVpcEndpoints](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeVpcEndpoints.html): empty string; example also omits token | None → terminal | Some("") → private helper → None/terminal | Unchanged | Integrity failure |
| [DescribePrefixLists](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribePrefixLists.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |
| [DescribeDhcpOptions](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeDhcpOptions.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |
| [DescribeNetworkAcls](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeNetworkAcls.html): null | None → terminal | Some("") → Malformed | Unchanged | Integrity failure |

The private `vpc_endpoint_continuation` helper runs only for DescribeVpcEndpoints,
after consuming that invocation's integrity-qualified output. It maps only an exactly
empty String to None. There is no global filter, trimming, whitespace/null sentinel,
`xsi:nil` interpretation or token rewriting. An empty intermediate collection with a
valid token continues. A literal `null` or whitespace token is nonempty; it is forwarded
exactly and remains subject to the existing length/NUL/cycle/page rules.

## Decoder-integrity evidence and service-error boundary

Source inspection uses the **locally pinned** Cargo.lock packages, not moving SDK
accessor documentation. Relative to `aws-sdk-ec2-1.237.0/src/protocol_serde/`, each
`shape_describe_{regions,availability_zones,subnets,vpcs,security_groups,route_tables,
vpc_endpoints,prefix_lists,vpc_attribute,dhcp_options,network_acls}.rs` contains the
successful response loop and builder updates. The eight token decoders call
`aws_smithy_xml::decode::try_data` and wrap the returned String in Some; there is no
empty-token normalization. Generated collection decoders start an empty Vec only
when their enclosing field is encountered. An omitted parent stays None, an explicit
empty parent becomes Some(empty), and a partial `item` builds its optional members.

Owned nested decoders are `shape_{region,availability_zone,subnet,vpc,security_group,
ip_permission,user_id_group_pair,ip_range,ipv6_range,prefix_list_id,route_table,route,
route_table_association,route_table_association_state,vpc_endpoint,prefix_list,
attribute_boolean_value,dhcp_options,dhcp_configuration,attribute_value,network_acl,
network_acl_association,network_acl_entry,port_range,icmp_type_code}.rs` and their
referenced `*_list`/`*_set` decoders. `ec2_decode_integrity.rs` explicitly pairs these
owned wire fields with SDK fields; no convenience accessor erases optional presence.

In `aws-smithy-xml-0.60.15/src/decode.rs`, `ScopedDecoder::next_tag` (line 341) calls
`next_start_element`; its attribute handling (line 406) uses
`unescape(value.as_str()).ok()?`. This converts an attribute-unescape error into
end-of-traversal. Generated `while let Some(...)` loops can therefore succeed after
losing a record, nested members or a continuation. `try_data` (line 435) returns empty
text for explicit empty elements. These are **source-inspection findings**.

The executable unprotected probes in `ec2_decode_integrity_tests.rs` independently
reproduce all three losses with the actual pinned VPC-endpoint protocol decoder:
a skipped item, skipped route-table/service members and skipped nextToken. Direct SDK
probes also prove None versus Some("") for omitted versus both empty XML forms, and
exact whitespace/entity-decoded text. The protected regressions run the production
adapter path against those bodies and reject them before records or terminal state
are accepted. These are **executed synthetic protocol results**, not live AWS evidence.

The EC2 guard is a private structural preflight using the existing `xmlparser` pin;
the SDK remains the only semantic XML decoder. It checks the complete balanced XML,
expected successful root, known object/list/scalar nesting, duplicate monitored
members, and all raw attributes (including ignored subtrees). Raw attribute ampersands
are conservatively rejected, including valid references, as in the documented IAM
protection. Monitored attributes other than namespace declarations, scalar children,
CDATA/comments/fragmented scalar text, DTD/processing instructions and malformed XML
fail closed. Ignored metadata subtrees cannot hide attribute traversal errors.
Depth and per-element attribute count are bounded at 128. Structural presence, list
length/order and empty/nonempty scalar state must correlate with the SDK output from
the same invocation. Only a completed successful SDK lifecycle yields a consumable
output; mismatch, reuse, expiry or cancellation cannot yield a terminal page.

**Only successful HTTP responses enter success-root/shape scanning.** Non-success
responses remain with the SDK's service-error decoder and existing static classifier.
Transport buffering, lifecycle, shared time/session and limit checks still apply to
both paths. Regressions prove ordinary access-denied, not-found and expired-session
responses keep their categories on the first or later page, and oversized successful
or service-error responses retain the shared response-limit precedence. Diagnostics
never include raw XML, policy text, tokens or provider error messages.

`Ec2Integrity` has an explicit Debug implementation exposing only the closed operation
name. Its payload-bearing State and Ec2Output have no Debug implementation. Executed
guarded-invocation regression checks both compact and pretty Debug while the completed
guard still owns distinctive token/policy sentinels, before consuming the receiver;
neither sentinel appears. Consuming the receiver afterwards still returns the exact
token and policy evidence. Correlation and receiver lifecycle rules are unchanged.

## Acceptance and validation

Within #1414, completion requires V4 contracts resolving C1–C7, mixed-record validation
and accounting, shared session, all eleven production readers, decoder integrity,
regressions and documentation together. Outputs validate against the explicit V4
successor EC2/network/policy contracts and existing V1 coverage contract. This replaces
the issue's historical V2-only output criterion without modifying historical contracts.
The dependency remains e2a's shared execution contract; there is no e2b1 prerequisite.

`provider_v4_contracts` pins independent canonical JSON/hash vectors, historical
rejection, partial states, duplicate identity, occurrence credit and aggregate bounds.
The `aws::ec2_` suites exercise exact requests, owned responses, all token forms,
empty intermediate pages, cycles/NUL/length/page bounds, collection/normalized bounds,
partial/malformed/contradictory facts, duplicate/unsupported policies, separate DNS
provenance, service classification, actual counts and retained earlier-page evidence.
They also round-trip every accepted V4 record. Existing transport, IAM presence,
identity, shared executor, clock, cancellation, admission and historical contract
suites remain required unchanged in behavior.

Validation commands use Rust 1.92.0, the standalone manifest/lockfile, offline Cargo,
and external target directories on macOS and Linux: targeted tests, full standalone
`cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo build`. Linux also runs the platform-specific confinement/journal/process tests
that cannot execute on macOS. Synthetic SDK production-boundary tests are the runtime
smoke coverage; no browser UI is affected. Root `make ci` does not discover this
standalone tool and remains an integration/MR check rather than replacement evidence.
Execution results and unavailable checks must be reported with the delivery review.

Excluded: new AWS operations, discovery, compliance/admission, effective IAM/network
evaluation, allocation-resource observations, reconciliation/inventory construction,
durable publication, controller integration, dependency upgrades and budget increases.
