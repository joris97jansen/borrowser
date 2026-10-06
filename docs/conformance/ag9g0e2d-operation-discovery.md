# AG9g0e2d bounded operation discovery

AG9g0e2d / #1416 adds discovery and relationship attribution to the standalone
`tools/conformance/host-lifecycle` workspace. It builds on e2a–e2c and the frozen
[A01–A38 evidence matrix](ag9g0e-provider-evidence-matrix-v1.md). It does not
evaluate those admission predicates, decide A23 cardinality, build final inventory,
bind or reconcile resources, publish/replay evidence, capture controller context,
or authorize cleanup. Neither parent AG9g0e2 nor Milestone AG is complete.

## Boundaries and supplied inputs

`provider::discovery::DiscoveryInputs::new` checks retained `PreparedLaunchV2`
artifacts against immutable `ReconciliationContextV3`, including artifact hashes
and lengths, authority root, request binding, and digest-bound manifest. Each
artifact is size-preflighted before existing validation. This verifies retained
identity; it does not renew launch authorization. Optional already-resolved prior
resources and relationships carry the context's prior-state digest and an explicit
fully-supplied flag. The context's bound InstanceId is always a seed. Missing prior
evidence remains a closure gap. Authenticating/resolving prior references belongs
to e3, not this API.

`ingest` owns a `DiscoveryEvidence`: the existing inert `ProviderObservationV5`
and optional [reviewed-subnet route successor](ag9g0e2d-reviewed-subnet-route-observation-v1.md).
It returns a validated carrier or an error containing the supplied evidence. The
carrier/report have no serde envelope, durable identity, publication or replay API.
Records retain their existing observation versions and canonical identities.

Offline `derive_offline` additionally requires `DiscoveryExecutionFacts`:
accepted request/source/output/response-byte/normalized-byte counters, the shared
failure latch, bounded pre-admission rejections, coordinator stop disposition,
and final round/session check. These facts are consistency-checked, not treated
as assertions of completeness. They do not authenticate HTTP history. There is
no offline dependency on a clock, credentials, AWS, or hidden coordinator state.

The pure layers own required reads (`requirements`), allocation references and
attribution (`relationships`), family-specific representation (`representation`
and `policy_count`), and the four completeness gates (`completeness`). Private
`aws::operation_discovery` selects and executes closed reads through the existing
`ObservationSession`. Shared `QueryCore` owns admission/page/time mechanics;
the historical and A07 frontends own their distinct records and coverage.

## Required-read traceability

Every row below contributes reasons to the same required-query entry. Sharing a
read unions its `FactSet` and source-occurrence triggers; it never replaces an
independent discovery path. Coverage `required` flags are not inputs to obligation
derivation. Exact infrastructure identities below always mean reviewed retained
targets, not arbitrary returned VPC/route/key/profile traversal.

I = each retained or observed plausible instance; E/V/P = each observed or prior
ENI/volume/profile-association identity, including contradictory returned IDs.
All rows require exact matching actual coverage, representation accounting, and
finished expansion; a failed, rejected, omitted or unterminated read cannot satisfy
an obligation. Empty lists can exhaust discovery without satisfying admission.

| Fact | Retained input / observed reference | Required read and scope | Trigger / regression |
| --- | --- | --- | --- |
| A01 | account/caller expectation | STS Caller, regional | always; mixed singleton accounting |
| A02 | region | Regions, regional/all regions | always; historical root accounting |
| A03 | AZ name/ID | AvailabilityZones, regional/all AZs | always; fixed reviewed seeds |
| A04 | subnet | Subnets, exact reviewed subnet | always; fixed reviewed seeds |
| A05 | VPC | Vpcs, exact reviewed VPC | always; shared A09/A22 reasons |
| A06 | SG set | SecurityGroups, exact reviewed set | always; same read also A25 |
| A07 | subnet/route table/association mode | exact reviewed RouteTables plus independent subnet-association query | always in both modes; successor transport/normalizer tests |
| A08 | endpoint/prefix list | exact VpcEndpoints and PrefixLists | always; policy scanner work tests |
| A09 | VPC/DNS/DHCP | exact Vpcs, both separate VpcAttributes, exact DHCP | always; distinct attribute provenance |
| A10 | NACL | NetworkAcls, exact reviewed NACL | always; fixed reviewed seeds |
| A11 | bucket/owner | HeadBucket with retained ExpectedBucketOwner | always; singleton and final-time tests |
| A12 | KMS key | DescribeKey, exact reviewed ARN | always; mixed singleton accounting |
| A13 | AMI/owner | Images, exact retained AMI | always; shared A13–A16 reasons |
| A14 | architecture/platform/root | same Images | always |
| A15 | image mappings/products | same Images | always; extracted/inline accounting |
| A16 | image origin and V snapshots | Images plus exact/attached Volumes | retained image and every V/I reference |
| A17 | instance type capabilities | InstanceTypes, exact retained type | always; no capability admission |
| A18 | type capacities | same InstanceTypes | always |
| A19 | type/AZ | InstanceTypeOfferings, retained type+AZ | always |
| A20 | profile/role identities | GetInstanceProfile, exact reviewed ARN | always; source role multiplicity |
| A21 | I/profile association references | exact Instances; profile associations attached to I and exact P | every I/P; transitive association test |
| A22 | token/placement/AMI/type | token discovery, exact Instances, reviewed subnet/VPC joins | retained token and every I |
| A23 | all possible instance references | all independent roots and allocation closure | seven-path test; **no cardinality verdict** |
| A24 | operation tags | independent instance/ENI/volume tag paths and exact descriptions | every allocation identity; missing tags retain linkage |
| A25 | I/ENI topology | Instances, exact ENIs and ENIs attached to I; reviewed SGs | every I/E; both perspectives |
| A26 | ENI attachments | same instance/ENI reads | enclosing and independently returned IDs |
| A27 | ENI addresses/associations | same instance/ENI reads | no address/admission filter |
| A28 | I/EBS mappings/V attachments | Instances, exact Volumes, Volumes attached to I | every I/V; parent/child disagreement retained |
| A29 | volume properties/attachments | same volume and instance reads | every I/V |
| A30 | I metadata options | Instances | every I; sibling projection audit |
| A31 | protection/shutdown | three separate InstanceAttributes | every I; selected attribute coverage |
| A32 | monitoring/EBS optimization | Instances | every I |
| A33 | lifecycle/placement/capacity | Instances | every I |
| A34 | hibernation/enclave/maintenance | Instances | every I |
| A35 | hostname/DNS options | Instances | every I |
| A36 | CPU/type defaults | Instances plus retained InstanceTypes | every I; shared type reasons |
| A37 | exact retained user data | InstanceAttribute(userData) | every I; existing single API Base64 decode |
| A38 | owned exclusion/secondary-interface facts | image, instance, ENI and volume projections | every corresponding identity; unsupported expansion remains incomplete |

## Independent discovery and closure

The seven roots are Instances by ClientToken, Instances by authority tag, Instances
by operation tag, ENIs by authority tag, ENIs by operation tag, Volumes by authority
tag, and Volumes by operation tag. Combined tags cannot substitute. Queries contain
no launch-property, owner, state, VPC or other admission filters.

Every returned allocation identity receives an exact singleton read. Every I also
requires ENI, volume and profile-association attachment queries and all four instance
attributes. Instance-side ENIs/mappings and standalone attachment perspectives add
both endpoints independently. Volume attachments retain enclosing and independently
returned volume IDs. Profile associations retain their returned instance. Sibling
projections are connected by exact query and source position, not by choosing one
identity. Requested targets remain separate query references; they never replace
returned fields. Malformed or unavailable relationship identities remain explicit
closure gaps. Opaque service references and unsupported secondary interfaces do not
open arbitrary service/infrastructure traversal.

Actual positional parent/child linkage is distinct from same-source sibling
linkage, a child's copied enclosing identity, and provider attachment perspectives.
For each extracted instance ENI, EBS mapping and volume attachment, the pure graph
locates the actual `Instance` or `Volume` projection using the exact query identity,
recorded page and typed `containing_list()` parent path. An options/exclusion sibling,
a parent on another page/query, or a matching enclosing ID cannot substitute.
The actual parent's identity is linked to all valid identities in that child
projection, preserving both projections as bounded occurrence references. These
`PositionalParent` edges express structural containment, not confirmed attachment.
`PositionalParentFact` records agreement, contradiction or unavailability between
the actual parent and the child's enclosing identity, including consistent self
references for which no graph edge is needed. Independently returned volume IDs
also remain connected and are compared with the actual parent.

Parent identity disagreements remain valid attribution evidence. No observation,
source position or query coverage is rewritten. Links are constructed before
positive propagation, contradiction classification, work derivation and exclusion.
Positive linkage is retained as a monotone flag even on contradictory nodes. Both
parent and child occurrences contribute to required-read reasons; query work alone
is deduplicated. Their common query determines discovery depth; containment adds
no extra read hop and does not give a newly observed child its prior parent's depth
zero. Complete contradictory discovery is permitted; the connected component is
ineligible for affirmative unrelatedness.

Missing actual parent projections, unavailable parent IDs and unavailable enclosing
IDs are explicit closure gaps. Unsupported secondary-interface projections retain
their known instance-parent/enclosing references and their unsupported gap, without
inventing a secondary allocation identity or traversal. An instance's reservation
parent supplies existing presence/ownership checks but no allocation identity: it
does not connect sibling instances. Root source paths have no parent and inline
collections introduce no new positional allocation nodes. Lookup scans the already
bounded canonical evidence; it allocates no additional parent index.

Query work is deduplicated by canonical identity; evidence is never deduplicated.
Cycles terminate because identities and query obligations are monotone bounded
sets. The coordinator uses the pure report's `next_pending` ordering: fixed seed
stages (ClientToken, authority tags, operation tags, reviewed infrastructure, prior
identities), canonical query key within each stage, then relationship reads ordered
by depth, canonical query key and triggering occurrence. A follow-up cannot displace
an outstanding seed. Independent observations introduce depth one; retained prior
identities have depth zero. A read of a known identity introduces new references at
the next depth. Repeated pure relaxation computes minimum depths from exact supplied
query/source provenance; it needs no hidden execution history. Unrooted supplied
references sort after known depths and remain represented.

The bounded frontier is constructed in fixed seed/typed-identity order; execution
uses the explicit stage/depth order. Exhaustion stops coordination and records
incompleteness, including any unexecuted seeds. It does not bypass a mandatory stop
to promise that every path executes. Outer-order permutations cannot affect either
selection or truncation. Distinct occurrences and provider collection order remain
intact. A profile association discovered through a depth-one attachment read is a
depth-two exact read, even when its query sorts before other depth-one work.

## Representation completeness is additional to execution coverage

Frozen aggregate validation intentionally permits partial evidence. Its minimum
source credit, including index-prefix credit, is not used as a completeness proof.

For V5, audit each exact query independently: represented roots on each represented
page must have dense indices starting at zero; each child needs its actual parent
projection; each source needs all required siblings; each extracted `Present(n)`
collection needs actual children `0..n`. Images require image+excluded projections;
instances require instance+options+excluded; nested ENIs require interface+instance
attachment; standalone ENIs require interface+standalone attachment+excluded;
volumes require volume+excluded and their declared attachment children. Other
sources require their named single projection. Only after these checks pass is the
exact count of actual unique source nodes plus owned inline list occurrences compared
with coverage.records. Declared-but-missing children and index prefixes add no credit.

| Family / operation | Exact represented-source rule |
| --- | --- |
| V2 STS / S3 | one enclosing singleton record; complete success must retain required successful fields |
| V3 KMS | one enclosing response, even when metadata is not returned |
| V3 IAM | one enclosing response plus each retained role occurrence, including duplicates |
| V4 Regions / AZs / Subnets / Vpcs | each root occurrence; no identity deduplication |
| V4 SG / routes / prefix lists / DHCP / NACL | each root plus every owned nested list occurrence |
| V4 VPC attribute | one enclosing singleton per distinct attribute query |
| V4 endpoint | endpoint and XML list occurrences plus supported policy JSON scanner work: each value/container and each object key |
| V5 allocation | actual positional source nodes plus inline owned list occurrences after structural audit |
| A07 successor | route root plus owned nested occurrences, under its own exact matching successor coverage |

For V4 DescribeVpcAttribute, complete coverage also requires the selected DNS object
and its Boolean value. The unsolicited sibling does not satisfy this adapter rule;
both explicit false and true do. Missing selected evidence with truthful incomplete
coverage is usable; pairing it with Complete coverage rejects as inconsistent input.
Other observable mandatory failures remain enforced by V2 successful identity/region
checks, V3 malformed/unrepresentable identity checks, V4 generic facts failures, and
the unchanged V5 aggregate `record.failure` / `required_failure` path (including
selected instance attributes and reservation collection presence). An omitted wire
result set that produces no record is not reconstructible from supplied records;
coverage remains the supplied execution contract, not authenticated HTTP history.

Policy `{}` adds one scanner-work occurrence despite historical minimum policy credit
being zero. Unsupported/malformed policy markers can discard subtree cardinality;
they therefore produce `UnverifiableSourceRepresentation`, not guessed equality.
No new occurrence ledger, SDK capture mechanism or frozen validator change is needed.

Missing first/middle V5 roots fail density even when prefix credit still equals
coverage.records. Missing final roots, all projections of a root, a nonempty page,
or all records for a positive-count query cause a count deficit. Missing siblings
fail structure even if occurrence counts match. Historical nonpositional families
can prove a deficit but cannot locate its page or missing resource. We do not invent
that identity or unseen descendants. Empty list results with zero source count are
valid; singleton success cannot be empty. Gaps between represented page numbers may
be genuinely empty pages and do not themselves fail the audit. NotReturned and
NotExposedBySource remain field states, not assertions of semantic emptiness.

## Ingestion, accounting and bounded reports

Ingestion checks combined collection counts first, then streams serialization into
a bounded counter before canonical JSON tree creation, cloning, sorting or indexing.
The counter accounts canonical control escaping and terminal LF. Materialized
canonical byte counts must agree with it. Existing records and route records jointly
fit 4096 retained outputs, 128 coverage entries, 128 accepted requests, 4096 source
occurrences, 16 KiB per record and 256 KiB normalized canonical evidence. Only outer
record/coverage order is normalized; the existing aggregate validator still runs
unchanged on canonical order. No inner provider order or source position is rewritten.

Every record requires exact matching coverage, account/region/context bindings, and
the successor requires the reviewed subnet. The shared snapshot cannot understate
supplied requests, source counts, retained outputs, retained bytes or known coverage
reservations. Unaccounted larger request/source/output counts cause an accounting
gap. Normalized charges can legitimately exceed final retained bytes: coverage
reservations and accepted charges before later failure are not refunded. Equality
with retained canonical bytes is neither required nor sufficient. A passed final
check cannot coexist with a failure latch. Shared limit/session query failures must
agree with that latch (the existing poisoned-lock boundary reports `Clock`);
query-local service, malformed-data and cycle failures do not require a latch.
Pre-admission rejection cannot coexist
with coverage for that query and never fabricates a coverage record.

Missing projections, roots, descendants and coverage remain useful partial evidence
with an incomplete result. Impossible source shapes, duplicate positional projections,
wrong provenance, exact represented counts exceeding coverage, successful coverage
paired with adapter-mandated normalization failure, and contradictory accounting are
rejected while returning supplied evidence. Historical validators are unchanged.

Derived report metadata has **one 65,536-byte allowance**, shared by graph, work,
status/audit and exclusion construction. Independent counters do not establish
additional capacity. `ReportBudget` charges before insertion and never refunds;
`metadata_bytes` includes conservative charges for temporary source/order indexes.
The platform-independent logical ownership rule below counts every copied payload
and uses fixed-width logical slots; it is not a claim about Rust allocator overhead.
Borrowed evidence remains in the separately bounded input carrier. No report member
owns a clone of an observation payload.

| Owned metadata | Logical charge |
| --- | --- |
| Report/container controls, counters and fixed failure flags | 256 bytes reserved initially |
| Allocation identity | 16 bytes plus UTF-8 identity length, per owned copy |
| Graph node | two identity copies (map key and node identity) plus 64 bytes for flags, depth, agreement and collection controls |
| Resource/query-trigger occurrence reference | 8 bytes each |
| Relationship occurrence | both identity copies plus 24 bytes for provenance kind and child/parent occurrence slots |
| Source-aware fact or relationship gap | 16 bytes each |
| Positional parent comparison | 32 bytes, including optional parent, child and agreement |
| Source/sibling scratch insertion | 64 bytes plus each copied identity |
| Contradiction scratch identity | identity plus 16 bytes |
| Prior ordering scratch | 8 bytes per possible borrowed identity/edge reference, reserved before construction |
| Required query | twice canonical key length (key and conservative owned query allowance), plus 64 bytes for reasons/stage/container controls |
| Execution entry | 24 bytes |
| Representation audit entry | 32 bytes plus 16 per gap |
| Exclusion component/visited scratch | 16 bytes per graph node plus 32, reserved before construction |
| Exclusion candidate | identity plus 16 bytes |

Query-local representation scratch retains its separately approved 64 KiB bound;
the resulting retained audit entry is charged to the combined report allowance.
The existing component ceilings (128 queries, 4096 graph nodes/edges, 32 KiB graph
accounting, 64 KiB keys, 16 KiB triggers) also apply; none adds capacity to the shared
allowance. On any allocation refusal,
fixed-size exhaustion flags remain available without another insertion, no exclusion
becomes effective, and discovery is incomplete. Useful supplied evidence stays in
the carrier. The frontier describes known work, never unseen descendants. Regression
fixtures combine 123 possible reads with 100 retained relationship occurrences: the
former individual ceilings fit, while the aggregate allowance is exhausted.
Positional facts/edges use these same report and graph allowances, charged before
insertion. A forty-ENI fixture retains all 84 projections while positional expansion
exhausts the graph allowance; missing links cannot make exclusions effective.

## Attribution and finalization

A discovery hit is a returned allocation reference. Matching token, operation-tag
query/returned tag, retained prior identity, or connected attachment supplies plausible
operation linkage. Missing tags or admission mismatches cannot undo it. Conflicting
sibling identities, requested/returned allocation identities, or competing attachment
instances remain represented as contradictory linkage. Actual positional parents
that disagree with child enclosing/returned identities are retained in the same
component with both positive provenance and contradiction visible.

The sole exclusion rule is an entirely closed foreign component after complete
execution, representation, expansion and final validation. Every allocation needs
standalone identity/tag evidence with exactly one valid authority/operation pair,
consistent across the component and explicitly naming another operation. Every
instance needs an explicit nonempty different token, consistent across observations;
instance reservation ownership must agree with the actual parent reservation;
instance-side and standalone ENI ownership, standalone attachment instance ownership,
and instance-side EBS volume ownership must all be present and agree with the query
account. Source-aware fact references retain each perspective without copying fields.
Both ENI attachment objects and IDs must be available, agree across perspectives,
and name the same endpoints. EBS mappings and volume attachments must agree on the
enclosing/returned volume, instance and device. Profile associations must agree with
the instance's returned profile ARN and unique ID; this does not admit that profile
against the reviewed launch profile or open arbitrary profile reads.

Missing prerequisites block exclusion; available conflicting facts set contradictory
attribution. Missing relationship objects remain closure gaps. Opaque associated
resource, owning-service, operator principal/managed or requester identity/managed
evidence receives `OpaqueManagedReference`: retained, unresolved at this reader
boundary, and ineligible for exclusion. No semantic absence or arbitrary traversal
is inferred from those markers. None of these checks evaluates launch admission,
attachment delete-on-termination, protection settings or other nonrelationship
properties. Actual query coverage is never rewritten by these decisions.
Positive target linkage, prior identities, missing ownership/tags/tokens, conflicting
observations, or any incomplete gate prevent exclusion. Excluded resources and their
evidence stay in the report. This is not deletion authority.

`derive` prepares all report data and exclusion candidates. The live coordinator then
checks the original round's monotonic clock and session expiry after that substantive
processing, and performs constant-size finalization. Effective exclusion is exposed
through `DiscoveryReport::attribution`, gated on the final result. Query-local errors
permit other independent reads; shared cancellation/time/session/byte/request/record
latches stop the round with existing precedence and no reset. All four conditions
are necessary for `complete`: required execution, sufficient supplied representation,
finished relationship expansion, and final round/session validation.

Equivalent evidence orderings mean permutations of supplied outer records/coverage
and resolved prior inputs with the same facts, multiplicities, source positions and
execution facts. They yield equivalent reports. Provider responses that change recorded
positions legitimately change provenance and may change evidence hashes. Inner
collections are not sorted to manufacture hash equality.

## Validation lane

Use Rust 1.92.0, the standalone lockfile, and an external `CARGO_TARGET_DIR`:

```sh
cargo test --locked --offline --test provider_discovery
cargo test --locked --offline --lib operation_discovery
cargo test --locked --offline --lib query_execution
cargo test --locked --offline --lib reviewed_subnet_route
cargo test --locked --offline --lib response_limits
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo build --locked --offline
cargo fmt --check
```

Run the full standalone suite on Linux as well, including
`production_round_uses_linux_boot_clock` and the existing Linux authority/filesystem
tests. Root-workspace CI does not cover this workspace. Scripted transports need no
AWS access. Browser smoke tests do not apply to this tooling-only change.
