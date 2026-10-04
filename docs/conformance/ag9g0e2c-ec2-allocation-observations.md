# AG9g0e2c / #1415: bounded EC2 launch and allocation observations

This contract covers eight private EC2 readers in the standalone host-lifecycle
workspace. It builds on [e2a execution](ag9g0e2a-identity-query-execution.md),
[e1a management semantics](ag9g0e1a-management-observation-v2.md), and the completed
[e2b infrastructure boundary](ag9g0e2b-ec2-infrastructure-observations.md).
It does not complete the parent issue or Milestone AG, establish provider admission,
or change CSS, layout, paint, parser or browser behavior.

## Ownership and frozen contracts

`provider/{allocation_value_v5,ec2_allocation_observation_v5,source_occurrence_v5,
observation_v5,evidence_v5}.rs` own SDK-independent representation, validation,
canonical identity and minimum source credit. `aws/ec2_allocation_reads.rs` owns
closed selectors and explicit requests; `ec2_allocation_observation.rs` projects
qualified SDK values. `ec2_decode_integrity/{allocation,allocation_shapes}.rs`
extends the existing EC2 integrity boundary with consumable invocation receipts and
a temporary, bounded source layout. `query_execution.rs`, `response_limits.rs` and
`observation_session.rs` own the shared query/session/round mechanics.

All V1/V2/V3/V4 serialized contracts, bytes, digests, identities, validators, fixtures
and rejection behavior remain frozen. Historical closed enums are not widened.
`ObservationRecordV5` contains `schema_version: 5`, `query: QueryIdentityV1`,
`source: SourceOccurrenceV5` and `data: ObservationDataV5`. Its canonical JSON uses
the existing sorted-key encoder, integer restrictions and terminal LF. Its
observation-only `EvidenceIdentityV5` records version 5, SHA-256 and complete byte
length. Source positions participate in identity: duplicate wire items at distinct
positions remain distinct evidence.

`ObservationEntryV5` carries V2 Caller/Bucket, V3 identity, V4 infrastructure and V5
allocation records. It rejects superseded V2 records without changing their own
historical standalone validators. `ProviderObservationV5` is an inert in-memory
aggregate, without serialization or publication APIs. It validates canonical order,
unique matching coverage, per-query credit and aggregate ceilings. There is no V5
inventory, context, durable index, replay format or reference-resolution scheme.

## Requests and visibility

The caller selects queries. No reader derives discovery work, filters returned
candidates, substitutes request identities for facts, splits a query, or adds
admission filters. Every request uses the pinned EC2 Query protocol, Action matching
the operation and Version=2016-11-15. Exact collections retain canonical input order.
The table lists all operation-specific parameters except a returned `NextToken`.
`T` means exactly one of the three independent tag scopes described below.

| Operation | Permitted scope and exact request shape | Page size / response |
| --- | --- | --- |
| DescribeImages | Exact `ImageId.N`; `IncludeDeprecated=true`, `IncludeDisabled=true` | MaxResults omitted; `imagesSet/item`, nextToken |
| DescribeInstanceTypes | Exact `InstanceType.N`; `IncludeUnsupportedInRegion=true` | MaxResults omitted; `instanceTypeSet/item`, nextToken |
| DescribeInstanceTypeOfferings | One type and AZ: `LocationType=availability-zone`, Filter.1 `instance-type`, Filter.2 `location`, one value each | MaxResults=10; `instanceTypeOfferingSet/item`, nextToken |
| DescribeIamInstanceProfileAssociations | Exact `AssociationId.N`, or attached-instance Filter.1 `instance-id` | Exact omits MaxResults; attachment uses 10; `iamInstanceProfileAssociationSet/item`, nextToken |
| DescribeInstances | Exact `InstanceId.N`, client-token Filter.1, or T; `IncludeManagedResources=true` | Exact omits MaxResults; discovery uses 10; `reservationSet/item/instancesSet/item`, nextToken |
| DescribeNetworkInterfaces | Exact `NetworkInterfaceId.N`, T, or attached-instance Filter.1 `attachment.instance-id`; `IncludeManagedResources=true` | Exact omits MaxResults; discovery/attachment uses 10; `networkInterfaceSet/item`, nextToken |
| DescribeVolumes | Exact `VolumeId.N`, T, or attached-instance Filter.1 `attachment.instance-id`; `IncludeManagedResources=true` | Exact omits MaxResults; discovery/attachment uses 10; `volumeSet/item`, nextToken |
| DescribeInstanceAttribute | Exactly `InstanceId` and `Attribute`: userData, instanceInitiatedShutdownBehavior, disableApiTermination, or disableApiStop | No MaxResults or continuation; returned instanceId and all four independently returned attribute objects |

Combined-tag scope sends Filter.1 `tag:borrowser:authority-id` and Filter.2
`tag:borrowser:operation-id`. Individual authority/operation scopes each send only
Filter.1 with their own key. Every filter has only Value.1. Client-token and
attachment scopes send only their named filter, never additional tags/state/owner
constraints. DryRun, Owner, ExecutableBy and all other unlisted inputs are omitted.

The generic frozen `QueryIdentityV1` exact bound is 128 unique identities. The private
`ExactInstanceTypes` request type narrows this to **100** names, as required by the
[service's InstanceType.N maximum](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeInstanceTypes.html).
101–128 names still pass historical generic query validation, but are deterministically
rejected as `Malformed` before SDK I/O, query reservation, coverage, any charge or
round latch. The reader independently checks this restriction before `LogicalQuery::begin`.
The 100/101/128 regression includes a deliberately invalid internal wrapper to test
that second boundary. No other owned exact collection has a documented narrower
cardinality limit in the inspected pinned input sources and linked API references;
128 remains local policy, not an asserted AWS service maximum.

[Instances](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeInstances.html)
and [ENIs](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeNetworkInterfaces.html)
explicitly prohibit MaxResults with explicit IDs. Exact reads omit it for every
owned operation as a consistent local execution policy; they still follow returned
tokens. Documented MaxResults ranges are 5–100 for types and 5–1000 for
[offerings](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeInstanceTypeOfferings.html),
[profile associations](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeIamInstanceProfileAssociations.html)
and ENIs. The inspected Images/Instances/[Volumes](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeVolumes.html)
API/input descriptions do not establish narrower exact-ID cardinalities or numeric
page-size ranges. Ten is the local discovery page size, not a completeness claim.

All seven paginated adapters preserve opaque tokens exactly, including whitespace
and the literal string `null`. An omitted token is terminal; present empty is
Malformed; NUL is invalid; 4,096 bytes fits and 4,097 produces Limit(RecordBytes).
There is no borrowed e2b endpoint empty-token exception. Empty intermediate pages
continue. Cycles and the 16-page limit stop with earlier evidence preserved.

Visibility flags broaden only the visibility of the approved scope. Types may
report unsupported-in-region types; `supported_in_region` remains a returned fact.
Managed-resource inclusion does not change account settings or imply that omitted
operator/requester fields are false. AMI deprecated/disabled inclusion does not
bypass launch permissions or account Allowed AMIs filtering. The
[Images reference](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeImages.html)
also documents eventual consistency and disappearing deregistered images. Neither
an empty successful response nor a terminal query establishes global existence,
complete discovery, an atomic snapshot, or a compliant singleton. No dependency or
account setting is changed to strengthen those claims.

## Owned variants, field states and source separation

There are sixteen closed data variants. Source/projection compatibility is explicit:

| Operation | SourcePathV5 (zero-based positions) | Permitted projections of that source |
| --- | --- | --- |
| Images | Image { image } | Image, ExcludedFeatures |
| InstanceTypes | InstanceType { instance_type } | InstanceType |
| InstanceTypeOfferings | TypeOffering { offering } | TypeOffering |
| Profile associations | ProfileAssociation { association } | ProfileAssociation |
| Instances | Reservation { reservation } | Reservation |
| Instances | Instance { reservation, instance } | Instance, InstanceOptions, ExcludedFeatures |
| Instances | InstanceNetworkInterface { reservation, instance, interface } | NetworkInterface, InstanceEniAttachment |
| Instances | InstanceEbsMapping { reservation, instance, mapping } | InstanceEbsMapping |
| Instances | InstanceSecondaryInterface { reservation, instance, interface } | UnsupportedSecondaryInterface |
| NetworkInterfaces | NetworkInterface { interface } | NetworkInterface, StandaloneEniAttachment, ExcludedFeatures |
| Volumes | Volume { volume } | Volume, ExcludedFeatures |
| Volumes | VolumeAttachment { volume, attachment } | VolumeAttachment |
| InstanceAttribute | InstanceAttribute | InstanceAttributes |

Request account, region, IDs, filters and selected attribute belong to `query`, not
returned identity fields. Reservation context, enclosing instance/interface/volume
IDs and independently returned child IDs remain separate. Missing parents are never
repaired from children, or vice versa. Instance-side ENI attachments do not expose a
child instance ID in this SDK; standalone attachment instance/owner members remain
independent. Standalone volume attachment volumeId may disagree with its enclosing
volume; both survive. Profile association ID, instance and partial profile ARN/ID
remain independent of the instance profile and GetInstanceProfile sources.

Operator object presence and its managed/principal/hidden-by-default members remain
independent at all five e1a locations: instance, both ENI perspectives, volume and
instance EBS mapping. Standalone ENI requesterManaged/requesterId are independent of
each other and operator. The instance-side ENI has no requester/tag fields in this
SDK; these use NotExposedBySource, never an inferred false/empty. Instance-side
public-address association allocation/association IDs, Neuron manufacturer and
source-inapplicable excluded-feature fields likewise explicitly use NotExposedBySource.
No normalizer emits semantic `Absent`.

The disposition table applies to **every** owned field in the inventory below.
Nested objects use `ObservationValueV2<T>` and members use `MemberV5<T>` unless a
more specific collection or byte type is shown. Unknown bounded enum text is
retained verbatim; negative integers and finite floating values remain facts.

| Qualified returned state | Representation / coverage consequence |
| --- | --- |
| Omitted string or primary identity | NotReturned; no automatic failure and no identity repair |
| Present empty string | Empty; distinct from omission and semantic absence |
| Valid typed identity, CIDR/address or nonempty bounded literal | Present; contradictions do not cause rejection |
| Invalid bounded nonempty typed text | Malformed(original SDK-decoded text), Incomplete(Malformed); fitting siblings remain |
| Ordinary omitted optional object, Boolean, number or list | Unavailable(NotReturned); no fabricated false/zero/object/list |
| Present empty object | Present object with explicit omitted member states |
| Present empty list | Present([]); extracted child collections use Present { count: 0 } |
| Source has no such member | NotExposedBySource, not inferred absence |
| Text above 2,048 bytes | Unrepresentable(TextBytes), Incomplete(Limit(RecordBytes)) |
| NUL in ordinary text | Unrepresentable(ContainsNul), Incomplete(Malformed); opaque user data is exempt |
| More than 128 items in an owned nested collection | Count all source items first; inline collection gets Unavailable(Read(Limit(Records))); extracted parent preserves actual count; no truncated child list; fitting independent projections remain |
| Missing top-level result collection | Incomplete(Malformed), no fabricated empty/resources |
| Missing Reservation.instances | Retain reservation identity/context/count state; Incomplete(Malformed) |
| Missing selected attribute object/value | Retain actual instance identity and all qualified sibling attribute facts; Incomplete(Malformed) |
| Returned secondary interface occurrence | Retain its partial ID/type and enclosing instance; Incomplete(Unsupported) |
| Nonfinite SDK f64 | Unavailable(Read(Malformed)); finite f64 uses exact 16-hex-digit bits, including negative zero |
| Malformed XML, wrong success root, duplicate monitored singleton, failed SDK scalar decode or correlation | No qualified records or terminality from that invocation; earlier accepted evidence remains |
| Non-success service/transport response | Existing bounded AccessDenied/NotFound/Service/Transport/SessionExpired categories; never successful empty output |
| Record/aggregate/time/session/cancellation limit | Preserve previously accepted evidence and no-refund charges; reserved incomplete coverage remains available |

The selected attribute remains invocation provenance even if a different instance or
additional attributes return. Only the selected attribute's missing object/value is
required to make coverage incomplete; malformed returned siblings also contribute
their own failure. Present empty user-data bytes are successful data. Present empty
shutdown text is a represented literal, not an invented missing value. Boolean values
must come from SDK decoding. A qualified terminal page with missing owned data can
have `terminal_page=true` and Incomplete; an unqualified invocation cannot establish
terminality. Complete means this logical query met its execution/evidence contract,
not operation-wide completeness, admission, A01–A38 evaluation or binding.

## Source occurrence credit and bounded ingestion

`SourceOccurrenceV5` serializes `{ page, path }`. Page is 1–16; root list indices
are 0–4095; nested indices are 0–127; collection counts are 0–4096. The closed path
variants above use the normal canonical kebab-case `kind` and named integer fields.
`CollectionShapeV5` is NotReturned, NotExposedBySource, or Present { count }.
Projection kind is derived from the data enum, not independently supplied evidence.
All limits are checked on deserialization and validation.

For one exact query, `SourceCreditV5` maintains temporary collection demands keyed by
(page, enclosing source path, closed list family), plus singleton-page credit and
(source, projection-kind) uniqueness. Every positional index i requires at least i+1
occurrences of its containing list, recursively through its ancestors. Combine demands
by **maximum for the same collection**, and **addition for distinct collections**.
A retained collection shape contributes its actual count and constrains children:
missing/empty cannot have child paths, and count n cannot admit index n. Resource
identity disagreement is unrelated to this structural consistency check and survives.

The closed list families are root Images, InstanceTypes, TypeOfferings,
ProfileAssociations, Reservations, NetworkInterfaces and Volumes; extracted Instances,
InstanceNetworkInterfaces, InstanceEbsMappings, InstanceSecondaryInterfaces and
VolumeAttachments; inline ImageMappings, ImageProductCodes, TypeArchitectures,
TypeVirtualization, TypeRootDevices, TypeUsageClasses, Gpu/Fpga/Inference/Media/NeuronDevices,
Tags, SecurityGroups, PrivateIpv4, Ipv6, Ipv4Prefixes, Ipv6Prefixes, Licenses,
ElasticGpuAssociations and ElasticInferenceAssociations. Only the matching owning
source may claim a list family. Inline arrays retain order and duplicates. Optional
scalar objects are not list occurrences. InstanceAttribute contributes one singleton.

Examples:

- reservation[7]/instance[63] needs 8 reservations + 64 instances = **72** credits.
  Its Instance, InstanceOptions and ExcludedFeatures projections share those prefixes.
  Instance[62] in the same reservation does not increase that minimum.
- Also retaining reservation[6]/instance[63] needs 8 + 64 + 64 = **136**.
- Adding ENI[2] and mapping[4] under the first instance adds 3 + 5 = **8**; the ENI
  base and attachment projections share the ENI's three credits. A distinct page
  starts distinct collections and adds its own prefixes.
- Volume[0] with two attachment occurrences costs 1 + 2 = **3**, even if the two
  attachments have identical fields. Its Volume/ExcludedFeatures projections share
  root credit; reusing the same source/projection kind is rejected.
- A retained reservation count of five requires five instance occurrences even if
  only instance[2] is retained. Count zero plus that path is structurally invalid.

Record validation checks scope/operation/path/projection compatibility, field validity,
canonical size and its own minimum credit. Aggregate validation independently rebuilds
the per-query ledger, requires source.page <= matching coverage.pages, and adds the
unchanged V2/V3/V4 per-record minima to V5 credit. Historical records cannot share or
borrow V5 credit; queries cannot borrow from each other. A general bounded page number
alone does not prove page provenance. The private ingestion boundary supplies that proof:

1. `LogicalQuery` starts one attempt and mints a fresh private identity token. The
   bounded connector charges the actual polled request and body frames. The guard
   verifies the operation and consumes the success XML structure under existing bounds.
2. The pinned SDK remains the semantic decoder. Guard shape/member/list presence must
   correlate exactly with that invocation's successfully decoded output, and the SDK
   lifecycle must finish successfully. The receiver consumes this state once.
3. The receiver builds a bounded temporary layout from that same output and returns a
   noncloneable qualified receipt retaining the attempt token, occurrences and token.
   The query consumes the receipt only if its active attempt matches, accounts one
   actual request/page and derives the source page from its attempted-page counter.
4. Terminality follows the qualified token. Charge **all** qualified owned source
   occurrences before projection, including duplicates and items in overflowing lists.
   A rejected whole charge is not partially accepted; earlier accepted charges remain.
5. Preflight the bounded projections for field/normalization failures before retaining
   output, so a pending limit cannot be lost behind a later provenance/token failure.
   User-data bytes are decoded once and cached for the retention traversal. At most
   one bounded projection is constructed at a time in each traversal.
6. For each projection, check output capacity, actual qualified source membership and
   collection counts, record validation, shared prefix credit, full canonical bytes,
   then increment retained output count and retain it. Failure stops incremental
   retention; earlier records remain. No record prefix or shortened collection is
   manufactured to fit.
7. Validate exact continuation/cycle/size, combine pending failures with the existing
   first-latched round precedence, perform final time/session checks, and finalize
   reserved coverage. A qualified terminal page can still have incomplete evidence.

Source occurrences (4,096), retained output records (4,096) and canonical evidence
bytes are independent shared counters across identity, infrastructure and allocation
readers. Output capacity is enforced both incrementally and on aggregates. The
16-KiB complete-record and 256-KiB aggregate ceilings include full canonical keys,
query, source, value encoding, punctuation and LF; coverage reservation consumes the
same byte budget. Other existing limits remain 128 requests, 16 pages/query,
1 MiB/response, 8 MiB/round, 4,096 token bytes and 300 seconds. Existing connector
expiry/deadline/cancellation checks, exclusive query leases, no refunds and reserved
failure coverage remain in force. SDK buffers, structural traversal and decoded
representations are bounded before retained evidence; no unbounded paginator collects
pages. This temporary layout is neither durable nor a cross-run resource index.

## Opaque user-data bytes

The API userData value becomes a String in pinned EC2 1.237.0:
`protocol_serde/shape_describe_instance_attribute.rs` dispatches userData to
`shape_attribute_value::de_attribute_value`, which uses Smithy XML `try_data` for
`value`. That generated path does **not** Base64-decode the String. The sole API
Base64 decode is `ec2_allocation_observation::user_data`, after integrity qualification.
No text conversion, trimming, line-ending normalization, payload parsing/rewriting,
decompression or second API decode occurs. The byte result is cached across preflight
and retention; original bytes `TQ==` remain `TQ==`, not `M`.

`UserDataBytesV5` supports 0–8,192 decoded bytes, including NUL, invalid UTF-8,
CRLF/LF and trailing bytes. It canonically serializes as standard padded Base64.
Parsing that canonical storage encoding reconstructs bytes and verifies canonical
re-encoding; it does not interpret the recovered bytes as another API encoding.
Byte changes, including terminal LF, change canonical identity. Debug prints length,
not contents.

Encoded API input is limited separately to **10,924 bytes** (4*ceil(8192/3)). A fixed
8,195-byte decode scratch buffer avoids unbounded decoded allocation. Check encoded
length before decoding; malformed Base64 uses MalformedBase64/Incomplete(Malformed).
8,192 bytes fits; 8,193 decoded bytes can still fit the encoded ceiling and yield
DecodedLimit; 8,194 decoded bytes require 10,928 encoded bytes and yield EncodedLimit.
Both limit markers contribute Limit(RecordBytes), retaining fitting identity/sibling
facts before latching. Omitted userData object, present object without value, and a
present empty value remain three distinct states. Valid supported binary data is
never classified unrepresentable because ProviderText cannot store it.

This observation ceiling does not expand launch authority. Existing
[`CollectorConfigV2::user_data_bytes`](../../tools/conformance/host-lifecycle/src/collector_config.rs)
requires canonical collector configuration <=1,024 bytes, and
[`LaunchParametersV2::validate`](../../tools/conformance/host-lifecycle/src/launch.rs)
independently enforces <=1,024 bytes and collector/deployment binding. The larger
8-KiB ceiling observes unexpected data while comfortably representing all currently
authorized payloads. It is a local observation ceiling, not the EC2 user-data maximum.
It preserves the shared record ceiling: an 8-KiB payload needs 10,924 Base64 bytes,
leaving room for the complete envelope. Extra sibling facts can still make a record
exceed 16 KiB; the full-record check rejects it without truncation or raising a limit.
The production-boundary regression recovers the legitimate retained 282-byte collector
fixture byte-for-byte, including terminal LF; the observation path never parses it.

## Pinned integrity and complete owned-field inventory

SDK sources are the locked `aws-sdk-ec2-1.237.0/src/{operation,types,protocol_serde}`
files. `allocation_shapes.rs` maps each field below to its exact wire name and SDK
member. Unlisted SDK members are **outside this observation surface**, not implicitly
compliant, absent, or required. Examples include descriptive names, AMI tags, creation
metadata, detailed accelerator memory/topology, and unrelated networking performance
fields. They do not grant query authority. The existing scanner still checks the
well-formedness and traversal bounds of ignored subtrees, but does not assign their
unowned values evidence meaning or source credit.

The approved field exclusions are explicit, even where the SDK returns the members:

| Unowned metadata | Retained ownership boundary |
| --- | --- |
| AMI `EbsBlockDevice.volume_size`, `volume_type`, `iops`, `throughput`, `encrypted`, `kms_key_id`, `delete_on_termination` and other provisioning members | Image mapping device/virtual/no-device facts, EBS object presence and `snapshot_id` remain owned. Standalone **Volume** provisioning and source-specific attachment delete-on-termination remain owned. |
| `Reservation.requester_id` | Reservation ID, owner and instance collection shape remain owned. Standalone **NetworkInterface** requester identity/managed and all owned operator states remain independent evidence. |
| DNS display names, including `Instance.private_dns_name`/`public_dns_name`, ENI/private-address `private_dns_name` and both association types' `public_dns_name` | Returned IP addresses remain owned. `PrivateDnsNameOptionsResponse` hostname type and A/AAAA configuration remain owned; they are configuration, not DNS display metadata. |

These members have no V5 field, member-specific normalization or singleton monitoring.
Bounded unowned text, including text above the ProviderText ceiling and repeated
unmonitored members, cannot create a field-specific `RecordBytes` failure or change
canonical evidence. This does not bypass the shared whole-response byte/traversal
bounds, XML structural safeguards or SDK decoding: malformed SDK primitive data in
an excluded member can still fail the entire invocation before qualification.

Instance address summaries are independent scalar evidence:

| SDK `Instance` member | Pinned XML singleton | `InstanceV5` member |
| --- | --- | --- |
| `private_ip_address` | `privateIpAddress` | `private_ipv4: MemberV5<Ipv4Addr>` |
| `public_ip_address` | `ipAddress` | `public_ipv4: MemberV5<Ipv4Addr>` |
| `ipv6_address` | `ipv6Address` | `ipv6: MemberV5<Ipv6Addr>` |

The pinned `_instance.rs` and `protocol_serde/shape_instance.rs` expose these three
optional strings; the projector normalizes only their actual returned values.
No ENI, request, association or sibling supplies a missing value. Absent and empty
summaries remain `NotReturned` and `Empty`; malformed bounded address text remains
`Malformed` with incomplete evidence, while valid contradictions remain facts.
They use the existing address-member bounds and canonical IP representation.
Each singleton participates in SDK presence correlation and duplicate-singleton
integrity rejection. They add canonical bytes, but no source occurrence or output
record. Changing a summary changes the instance evidence independently of ENI facts.
The larger instance payload is boxed inside `ObservationDataV5`; this Rust memory
layout choice does not introduce a serialized wrapper or change source accounting.

All listed list items, including duplicates and empty items, count as occurrences.
Monitored singleton duplicates reject the whole invocation, even if values agree.
Owned lists use `item` except InferenceAcceleratorInfo.accelerators: pinned
`shape_inference_device_info_list.rs` consumes **member**. The integrity shape checks
that exact spelling; it rejects item there because the SDK otherwise silently discards
it. This was verified through the production-boundary regression. Numeric/Boolean/time
values remain SDK-decoded, and unknown bounded enum strings are preserved.

The following exhaustive inventory uses SDK member names; nested types have their own
rows. Every field follows the common dispositions above, with the required root,
Reservation.instances, selected-attribute and unsupported-secondary exceptions already
specified. The eight output structs contain their tabled result collection and
next_token, or instance_id and the four attribute objects for InstanceAttribute.

| Pinned SDK type | Owned members |
| --- | --- |
| Image | `image_id`, `owner_id`, `state`, `architecture`, `platform`, `platform_details`, `usage_operation`, `virtualization_type`, `root_device_type`, `root_device_name`, `block_device_mappings`, `product_codes`, `kernel_id`, `ramdisk_id` |
| BlockDeviceMapping | `device_name`, `virtual_name`, `no_device`, `ebs` |
| EbsBlockDevice | `snapshot_id` (containing EBS object presence is independently retained) |
| ProductCode | `product_code_id`, `product_code_type` |
| InstanceTypeInfo | `instance_type`, `processor_info`, `supported_virtualization_types`, `supported_root_device_types`, `supported_usage_classes`, `burstable_performance_supported`, `instance_storage_supported`, `supported_in_region`, `v_cpu_info`, `memory_info`, `ebs_info`, `network_info`, `gpu_info`, `fpga_info`, `inference_accelerator_info`, `media_accelerator_info`, `neuron_info` |
| ProcessorInfo | `supported_architectures` |
| VCpuInfo | `default_v_cpus`, `default_cores`, `default_threads_per_core` |
| MemoryInfo | `size_in_mib` |
| EbsInfo | `ebs_optimized_support`, `ebs_optimized_info` |
| EbsOptimizedInfo | `maximum_iops`, `maximum_throughput_in_m_bps` |
| NetworkInfo | `maximum_network_interfaces`, `maximum_network_cards` |
| GpuInfo | `gpus` |
| FpgaInfo | `fpgas` |
| MediaAcceleratorInfo | `accelerators` |
| NeuronInfo | `neuron_devices` |
| GpuDeviceInfo | `name`, `manufacturer`, `count` |
| FpgaDeviceInfo | `name`, `manufacturer`, `count` |
| InferenceDeviceInfo | `name`, `manufacturer`, `count` |
| MediaDeviceInfo | `name`, `manufacturer`, `count` |
| NeuronDeviceInfo | `name`, `count` |
| InstanceTypeOffering | `instance_type`, `location_type`, `location` |
| IamInstanceProfileAssociation | `association_id`, `instance_id`, `iam_instance_profile`, `state`, `timestamp` |
| IamInstanceProfile | `arn`, `id` |
| Reservation | `reservation_id`, `owner_id`, `instances` |
| Instance | `private_ip_address`, `public_ip_address`, `ipv6_address`, `instance_id`, `client_token`, `placement`, `subnet_id`, `vpc_id`, `image_id`, `instance_type`, `state`, `iam_instance_profile`, `network_interfaces`, `block_device_mappings`, `secondary_interfaces`, `root_device_name`, `root_device_type`, `tags`, `cpu_options`, `operator`, `metadata_options`, `monitoring`, `ebs_optimized`, `instance_lifecycle`, `capacity_reservation_specification`, `capacity_reservation_id`, `capacity_block_id`, `hibernation_options`, `enclave_options`, `maintenance_options`, `private_dns_name_options`, `key_name`, `kernel_id`, `ramdisk_id`, `licenses`, `elastic_gpu_associations`, `elastic_inference_accelerator_associations`, `outpost_arn` |
| InstanceState | `code`, `name` |
| Placement | `availability_zone`, `availability_zone_id`, `tenancy`, `group_name`, `group_id`, `host_id`, `host_resource_group_arn` |
| CpuOptions | `core_count`, `threads_per_core` |
| OperatorResponse | `managed`, `principal`, `hidden_by_default` |
| InstanceMetadataOptionsResponse | `http_endpoint`, `http_tokens`, `http_put_response_hop_limit`, `http_protocol_ipv6`, `instance_metadata_tags`, `state` |
| Monitoring | `state` |
| CapacityReservationSpecificationResponse | `capacity_reservation_preference`, `capacity_reservation_target` |
| CapacityReservationTargetResponse | `capacity_reservation_id`, `capacity_reservation_resource_group_arn` |
| HibernationOptions | `configured` |
| EnclaveOptions | `enabled` |
| InstanceMaintenanceOptions | `auto_recovery` |
| PrivateDnsNameOptionsResponse | `hostname_type`, `enable_resource_name_dns_a_record`, `enable_resource_name_dns_aaaa_record` |
| LicenseConfiguration | `license_configuration_arn` |
| ElasticGpuAssociation | `elastic_gpu_id`, `elastic_gpu_association_id`, `elastic_gpu_association_state`, `elastic_gpu_association_time` |
| ElasticInferenceAcceleratorAssociation | `elastic_inference_accelerator_arn`, `elastic_inference_accelerator_association_id`, `elastic_inference_accelerator_association_state`, `elastic_inference_accelerator_association_time` |
| Tag | `key`, `value` |
| InstanceNetworkInterface | `network_interface_id`, `owner_id`, `vpc_id`, `subnet_id`, `interface_type`, `status`, `groups`, `private_ip_address`, `private_ip_addresses`, `ipv6_addresses`, `ipv4_prefixes`, `ipv6_prefixes`, `association`, `operator`, `attachment` |
| NetworkInterface | `network_interface_id`, `owner_id`, `vpc_id`, `subnet_id`, `interface_type`, `status`, `groups`, `private_ip_address`, `private_ip_addresses`, `ipv6_addresses`, `ipv4_prefixes`, `ipv6_prefixes`, `association`, `tag_set`, `operator`, `requester_managed`, `requester_id`, `attachment`, `outpost_arn` |
| GroupIdentifier | `group_id`, `group_name` |
| InstanceNetworkInterfaceAssociation | `ip_owner_id`, `public_ip`, `carrier_ip`, `customer_owned_ip` |
| NetworkInterfaceAssociation | `allocation_id`, `association_id`, `ip_owner_id`, `public_ip`, `carrier_ip`, `customer_owned_ip` |
| InstancePrivateIpAddress | `private_ip_address`, `primary`, `association` |
| NetworkInterfacePrivateIpAddress | `private_ip_address`, `primary`, `association` |
| InstanceIpv6Address | `ipv6_address`, `is_primary_ipv6` |
| NetworkInterfaceIpv6Address | `ipv6_address`, `is_primary_ipv6` |
| InstanceIpv4Prefix | `ipv4_prefix` |
| InstanceIpv6Prefix | `ipv6_prefix` |
| Ipv4PrefixSpecification | `ipv4_prefix` |
| Ipv6PrefixSpecification | `ipv6_prefix` |
| InstanceNetworkInterfaceAttachment | `attachment_id`, `device_index`, `network_card_index`, `status`, `attach_time`, `delete_on_termination` |
| NetworkInterfaceAttachment | `attachment_id`, `device_index`, `network_card_index`, `status`, `attach_time`, `delete_on_termination`, `instance_id`, `instance_owner_id` |
| InstanceBlockDeviceMapping | `device_name`, `ebs` |
| EbsInstanceBlockDevice | `volume_id`, `status`, `attach_time`, `delete_on_termination`, `ebs_card_index`, `associated_resource`, `volume_owner_id`, `operator` |
| Volume | `volume_id`, `availability_zone`, `availability_zone_id`, `snapshot_id`, `volume_type`, `state`, `size`, `iops`, `throughput`, `encrypted`, `kms_key_id`, `multi_attach_enabled`, `tags`, `attachments`, `operator`, `outpost_arn` |
| VolumeAttachment | `volume_id`, `instance_id`, `device`, `state`, `attach_time`, `delete_on_termination`, `ebs_card_index`, `associated_resource`, `instance_owning_service` |
| InstanceSecondaryInterface | `secondary_interface_id`, `interface_type` |
| InferenceAcceleratorInfo | `accelerators` |
| AttributeValue | `value` |
| AttributeBooleanValue | `value` |

## Regression and validation boundary

`tests/fixtures/ec2-allocation-sdk-v5/` contains eight synthetic rich wire responses,
including contradictory attachments, partial identities, future enum literals and
negative signed values. They are protocol fixtures, not live AWS observations.
The field audit reconciles the eight operations' inventory with the V5 types,
projections and monitored SDK shapes. Rich fixture assertions check expected values
for every owned variant: image mapping/snapshot/billing facts; type CPU, memory,
network/EBS and accelerator capabilities; offering location; profile association;
reservation/instance identities and addresses; instance options/excluded features;
both ENI and EBS perspectives; volume provisioning; secondary interfaces; and all
four attributes. Excluded metadata is exercised separately from the owned fixtures.
`tests/fixtures/provider-foundation-v5/reservation.{json,sha256}` is an independently
constructed canonical vector; all historical vectors remain unchanged.

- `source_occurrence_v5_tests.rs` and `provider_v5_contracts.rs` cover shared fan-out,
  positional prefixes, distinct collections/pages, sparse indices, incompatible and
  duplicate projections, parent/path consistency, matching coverage pages, query and
  historical-version credit isolation, aggregate output/byte bounds and canonical
  identity/round trips/rejection.
- `ec2_allocation_reads_tests.rs` covers all eight operations and sixteen variants,
  exact request parameters/visibility, 100/101/128 type bounds without side effects,
  qualified missing data versus unqualified invocations, duplicate records and
  empty intermediate pages, independent attachments and management/requester facts.
- `ec2_allocation_owned_fields_tests.rs` checks expected rich-fixture values and
  ownership boundaries, contradictory instance/ENI summaries, absent/empty ENI lists,
  canonical address changes without extra source credit, address omission/empty/
  malformed and duplicate-singleton dispositions, and preservation of earlier pages.
  Excluded AMI provisioning/requester/DNS metadata leaves evidence and coverage
  unchanged, including oversized but response-bounded text. SDK primitive decoding
  failures and whole-response limits continue to reject unqualified invocations.
- `ec2_allocation_boundary_tests.rs` covers every discovery scope, token bytes/cycles,
  page/request/source/output/canonical/response limits, shared reader-family budgets,
  earlier evidence, service failures, missing-result/continuation failure precedence,
  all primary identity states, nested object
  presence and the pinned inference-list spelling.
- `ec2_instance_attribute_tests.rs` covers all four exact requests, missing selected
  objects/values and returned siblings, malformed singleton/SDK decoding, the retained
  collector payload, empty/binary/NUL/non-UTF-8/CRLF/trailing bytes, the double-decode
  sentinel, data above the old text ceiling, encoded/decoded limits and the exact
  complete 16,384-byte canonical record boundary (then one byte over).
- `ec2_decode_integrity/allocation_tests.rs` exercises actual SDK lifecycle receipt
  consumption, attempt mismatch, positional/count/projection forgery, incremental
  retention, pending-limit precedence, cancellation and reserved failure coverage.
  Existing e2a/e2b transport/deadline/session tests remain part of the full lane.

Use the standalone workspace and pinned toolchain, independently of root browser CI:

```sh
rustup run 1.92.0 cargo test --manifest-path tools/conformance/host-lifecycle/Cargo.toml --locked --offline --lib ec2_allocation_reads
rustup run 1.92.0 cargo test --manifest-path tools/conformance/host-lifecycle/Cargo.toml --locked --offline --test provider_v5_contracts
rustup run 1.92.0 cargo test --manifest-path tools/conformance/host-lifecycle/Cargo.toml --locked --offline
rustup run 1.92.0 cargo clippy --manifest-path tools/conformance/host-lifecycle/Cargo.toml --locked --offline --all-targets -- -D warnings
rustup run 1.92.0 cargo build --manifest-path tools/conformance/host-lifecycle/Cargo.toml --locked --offline
rustup run 1.92.0 cargo fmt --manifest-path tools/conformance/host-lifecycle/Cargo.toml --check
```

Linux runs additionally exercise the production clock/filesystem paths. Synthetic
production-boundary tests prove request/decoder/normalizer/executor behavior; they
are not live AWS qualification or browser-runtime tests. Exact executed commands and
results belong to the implementation review report, not inferred from this recipe.

Discovery coordination, admission filters, candidate rejection, A01–A38 evaluation,
binding, durable publication/replay, controller/CLI integration, AWS mutation and
browser-engine behavior remain excluded. No completed query or implemented adapter
alone makes #1415, its parent or AG complete.
