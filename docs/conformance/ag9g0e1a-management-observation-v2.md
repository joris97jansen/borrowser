# AG9g0e1a: provider management/delegation evidence V2

This corrects the loss of independent management facts in the frozen
[V1 evidence vocabulary](ag9g0e-provider-evidence-matrix-v1.md). It extends the
[foundation](ag9g0e1-reconciliation-foundation.md) within the independent
`tools/conformance/host-lifecycle` workspace. This is an inert observation contract,
not an A38 evaluator, reconciliation decision, or provider-authority representation.

## Frozen V1 and explicit successors

V1 source contracts, validators, canonical bytes, fixture hashes and rejection rules
remain unchanged. There is no conversion from V1 to V2, including no reconstruction
of operator evidence from `managed_operator`. An explicitly new record is required.

| Contract | Version and representation |
| --- | --- |
| `ObservationRecordV2` / `ObservationDataV2` | Schema 2; closed successor enum, no embedded unrestricted V1 payload |
| `EvidenceIdentityV2` | Schema 2, Observation or Inventory, SHA-256 and exact canonical byte length |
| `EvidenceReferenceV2::ObservationV2` | Requires inner Observation kind and schema 2 |
| `EvidenceReferenceV2::InventoryV2` | Requires inner Inventory kind and schema 2 |
| `EvidenceReferenceV2::CoverageV1` | Requires unchanged `EvidenceIdentityV1`, Coverage kind and schema 1 |
| `InventoryEntryV2` | Explicit successor reference list |
| `ProviderResourceInventoryV2` | Schema 2; successor entries, attachments and history |
| `PriorProviderIdentityV2` | Successor evidence references; unchanged full prior-state digest meaning |
| `ContextFieldsV2` / `ReconciliationContextV2` | Schema 2 and evidence-policy version 2; immutable fields |
| `ProviderObservationV2` | In-memory V2 records and V1 coverage, bound to a V2 context digest; no serde aggregate |

V2 references do not accept V1 Observation or Inventory identities, even through
the CoverageV1 variant. Kind/version mismatches reject at every container validator.
An identity is an integrity reference; validation does not fetch or authenticate its
target. Referenced bytes still require later verification. Inventory identities name
the complete canonical typed inventory bytes, without defining a durable envelope.
`ProviderResourceInventoryV2::canonical_bytes()` is the authoritative inventory
validation/encoding path. Both `validate()` and `identity()` use that path; identity
covers the exact complete bytes, including container syntax and terminal LF.
Oversized inventories reject before an identity can be produced, without truncation,
chunking or partial hashing.

Query identities/scopes, read operations/coverage, limit version 1, digest syntax,
resource identities, list/text primitives and unchanged network observation leaves
are reused without reinterpretation. Existing launch/root generation 2 is independent
of observation schema 2. Context validation preserves root/artifact/binding/receipt,
capture-time and prior-evidence set checks; it grants neither locked capture nor a lease.

V2 parse/canonical APIs validate schema, shape and bounds. Direct serde decoding is
not contract validation; typed inventories and reference values require `validate()`.
No decoder falls back to another version. Unknown fields reject and corrected members
have no deserialization defaults. The existing canonical encoder and its 64-KiB
decoder ceiling are unchanged; no large-document reader or durable packing is added.

## Independent nested states

`ObservationValueV2<T>` contains `Present(T)`, `Absent`, or
`Unavailable(UnavailableEvidenceV2)`. Unavailable reasons are `NotReturned`,
`NotExposedBySource`, or an unchanged `ReadFailureV1` under `Read`.

`OperatorEvidenceV2` contains independently observed `managed`, `principal` and
`hidden_by_default`. The operator itself is an observation value. `RequesterEvidenceV2`
similarly separates requester management and identity. Unchanged observation fields
continue using the existing `Observed<T>` vocabulary with its original semantics.

| Decoded response condition | Normalized evidence |
| --- | --- |
| Omitted operator object | `Unavailable(NotReturned)` |
| Present operator, including an empty XML object | `Present(OperatorEvidenceV2)` |
| Omitted member of a present operator | That member is `Unavailable(NotReturned)` |
| Explicit false / true | `Present(false)` / `Present(true)` |
| Returned principal | Its bounded literal value, independently of either boolean |
| Omitted instance/device in standalone EBS attachment | `Unavailable(NotReturned)`, regardless of sibling metadata |
| Field not exposed by this source shape | `Unavailable(NotExposedBySource)` where represented |
| Invalid typed resource/account identifier or unrepresentable timestamp | `Unavailable(Read(Malformed))`; never an invented identifier |
| Text exceeding its existing bound or containing NUL | Normalization error; never truncation or a compliant default |
| Failed read/protocol decoding | No successful field normalization; caller must retain the applicable failure/coverage evidence |

**No currently audited normalization path emits `ObservationValueV2::Absent`.**
It remains a separately encoded contract state requiring authoritative semantic
absence. Omission, optionality, explicit false, missing parents, and correlations with
sibling fields do not establish that absence. The synthetic Absent vector exercises
the vocabulary only; it is not claimed to be an AWS adapter output.

AWS's volume attachment documentation says managed-resource attachments return null
instance/device fields. That describes returned values, not positive proof that an
underlying instance/device does not exist or is semantically inapplicable. Neither
`associated_resource` nor `instance_owning_service` changes those fields' observation
states. Later e2 evaluation may interpret the complete combination.

`managed=true` with missing principal, `managed=false` with a principal, independently
hidden resources and conflicting requester/operator facts all remain representable.
Literal text is not a structured subprotocol. An explicitly returned empty text value
is not an absence marker; empty typed identifiers are malformed. No placeholder value
is synthesized for any field.

## Exact pinned SDK evidence and source locations

The lockfile pins `aws-sdk-ec2=1.237.0`, archive SHA-256
`bfe29481f63a118c80f6bbded678711367bfc2b23f59b4351e7dfa6f439c3881`.
The generated `types/_operator_response.rs` has independent optional bool/string/bool
members. `protocol_serde/shape_operator_response.rs` independently sets each returned
member; omission leaves None. The parent decoders below construct Some for a present
operator object, even when its members are omitted.

| Read / location | Pinned `types/` source | Pinned `protocol_serde/` source |
| --- | --- | --- |
| DescribeInstances / instance | `_instance.rs` | `shape_instance.rs` |
| DescribeInstances / ENI | `_instance_network_interface.rs` | `shape_instance_network_interface.rs` |
| DescribeInstances / block mapping EBS | `_ebs_instance_block_device.rs` | `shape_ebs_instance_block_device.rs` |
| DescribeNetworkInterfaces / ENI | `_network_interface.rs` | `shape_network_interface.rs` |
| DescribeVolumes / volume | `_volume.rs` | `shape_volume.rs` |

`types/_volume_attachment.rs` and `protocol_serde/shape_volume_attachment.rs` expose
and decode instance/device and management metadata independently. No null/semantic
absence discriminator or cross-field validation is supplied by that decoder.
These claims are pinned-source observations, corroborated by synthetic protocol tests,
not an inference that optional SDK fields represent absence.

API references corroborating field meanings:
[OperatorResponse](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_OperatorResponse.html),
[VolumeAttachment](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_VolumeAttachment.html),
[EbsInstanceBlockDevice](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_EbsInstanceBlockDevice.html),
[NetworkInterface](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_NetworkInterface.html).
The live references are not a substitute for the locked source and protocol regression.

## Source-specific evidence

`NetworkInterfaceSourceV2` distinguishes Standalone from Instance with observed parent
instance identity. Both retain operator facts. Standalone ENIs expose requester bool
and identity independently. Instance-side ENIs expose neither requester field: both
normalize to NotExposedBySource, without joining a separate response.

`AttachmentV2::InstanceEbsMapping` retains the observed enclosing instance, mapping
device and nested EBS object state. Its EBS object owns observed volume ID, status,
attachment time, delete-on-termination, signed card index, associated resource,
volume-owner account and operator. Missing EBS objects remain unavailable objects.

`AttachmentV2::VolumeAttachment` retains the observed enclosing volume separately
from the returned attachment volume ID. Its attachment owns observed instance/device,
state, attachment time, delete-on-termination, signed card index, associated resource
and instance-owning service. The standalone volume's operator remains on the volume
observation, not fabricated into each attachment. Instance-side volume owner and
standalone instance-owning service are not substituted for one another.

Enclosing/child identity mismatches, simultaneously contradictory fields and repeated
attachments survive. No ordinary/managed discriminator forces a compliant choice.
Unchanged ENI/profile attachment semantics remain explicit variants in AttachmentV2.

## Bounds, determinism and ownership

The existing bounds remain: 2,048-byte ProviderText, 128-element EvidenceList,
16-KiB canonical record, 256-KiB normalized aggregate, 4,096 occurrences/records,
128 requests and 16 pages per query. Inventory entry/attachment counts remain 4,096,
entry references 128, history references 256, context prior references 128 and context
canonical bytes 32 KiB. Limits compose: a record's byte limit can bind before its
collection limit. The signed card index uses unchanged ProviderI32 decimal strings.

Three distinct byte contracts apply and must not be used interchangeably:

- **Per-record canonical limit:** `RECORD_BYTES` is 16 KiB. Observation identities
  require a byte length in `1..=RECORD_BYTES`.
- **Complete inventory canonical-container limit:** `INVENTORY_CANONICAL_BYTES_V2`
  is exactly 65,536 bytes (64 KiB), including all container overhead and the terminal
  LF. Inventory identities require a byte
  length in `1..=INVENTORY_CANONICAL_BYTES_V2`. The existing inventory child/count/
  reference bounds and child accounting remain enforced in addition to this limit.
- **Aggregate observation-round accounting limit:** `NORMALIZED_BYTES` is 256 KiB
  across charged canonical records and coverage. It is not an observation-record
  identity ceiling or a substitute for the complete inventory-container check.

V2 identity validation selects its ceiling by evidence kind and still requires
schema 2. Every accepted V2 inventory can produce a valid Inventory identity.
These V2 checks do not alter V1 inventory or identity behavior.

The frozen V1 inventory validator accounts for individual children and their summed
bytes; it can accept a structurally valid inventory larger than one 64-KiB canonical
document. V1 introduced no whole-inventory canonicalization/identity guarantee.
AG9g0e1a does not retroactively change that behavior. V2 deliberately adds the stronger
invariant that every accepted inventory has complete canonical bytes and a valid
matching identity within the existing document envelope. The shared encoder, decoder
and `EVENT_BYTES` remain unchanged. There is no larger-document encoding API.

The complete-container ceiling can bind before child/count/accounting limits. Such
an inventory rejects; evidence is never truncated, deduplicated, chunked or partially
hashed to make it fit. The 256-KiB observation-round allowance does not promise that
one inventory document can retain that entire amount.

Records are canonically ordered with equal duplicates allowed. Coverage credit is
checked per exact query, including duplicates. A terminal response is still necessary
for complete read coverage; completion does not imply every optional member was
returned or that admission evidence is sufficient. Prior-context references retain
their existing sorted unique reference-set semantics; this does not deduplicate
observation occurrences. No durable ordering/index/envelope scheme is introduced.

`ObservationRound::canonical_record_v2` validates before shared canonical accounting.
The V1 entry point is unchanged; both share latched round limits without refunds.
Decoded occurrence charging remains e2-owned and must precede filtering/deduplication.

Provider modules contain no AWS types or client imports. Private pure helpers under
`aws/management_observation.rs` interpret only the audited fields and actual enclosing
response context. They perform no reads, discovery, pagination, reviewed comparison,
admission, reconciliation or inventory construction. Other service adapters remain e2.
Malformed/unsupported normalization must later produce incomplete evidence as required
by the foundation; these pure helpers neither execute queries nor decide coverage.

## Regression evidence and exclusions

`provider_v2_contracts` tests independent fixtures/hashes, nested states, contradictory
and duplicate records, identity propagation, versions, shapes and bounds. New protocol
tests use synthetic XML through the locked SDK for all five locations, all independent
operator bool/omission combinations, requester fields and both EBS source shapes.
Omitted instance/device fields remain unavailable with neither, either or both managed
metadata fields. V1 fixtures and validators are exercised unchanged. Compile-fail
examples retain immutable contexts and nonserializable observation aggregates.

No publication, allocation, mutation, cleanup, journal/replay provider authority,
controller integration, new read operation, A01–A38 evaluator, reconciliation decision,
host readiness, signed IID verification or Chromium qualification is implemented.
This issue does not close AG9g0e2, parent #1402, or Milestone AG.
