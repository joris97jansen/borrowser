# AG9g0e2a1: partial identity-service observations V3

This corrects the partial KMS/IAM identity representation needed by #1413.
Implementation belongs exclusively to `tools/conformance/host-lifecycle`.
These are inert provider contracts and private normalization/presence helpers.
There is no production read entry point, query executor, admission, reconciliation,
inventory construction, journal integration or provider authority.

## Versions and ownership

V1 and V2 source contracts, canonical bytes, validators, identities and fixtures
remain unchanged. In particular, `RoleIdentity` and `ObservationDataV2` retain their
historical complete-identity requirement. Missing information cannot be recovered
by relabeling those records. No conversion/migration API exists.

`ObservationRecordV3` has schema 3 and only two data variants:

* `Key { metadata: ObservationValueV2<KmsKeyEvidenceV3> }`;
* `Profile { profile: ObservationValueV2<IamProfileEvidenceV3> }`.

The reused object/collection `ObservationValueV2` has unchanged semantics. No new
normalizer emits `Absent`. Missing parents/collections use
`Unavailable(NotReturned)`; present objects retain independently observed members.
STS, S3 and EC2 observations remain schema 2. Coverage/query/limit contracts remain
V1. Generation-2 root, deployment, launch and journal contracts are unchanged.

The query holds the requested key/profile identity. Key records require
DescribeKey with exactly one `ResourceIdentity::Key`; profile records require
GetInstanceProfile with exactly one `ResourceIdentity::Profile`. These checks
validate evidence provenance shape, not correspondence with returned identity.
Normalization receives no reviewed identifiers. A different returned ARN/account
is evidence, never a generic read failure or a reason to substitute request data.

## Independent member states

`IdentityMemberV3<T>` uses `kind` and, where applicable, `value`:

| Input | State |
| --- | --- |
| Omitted member | `not-returned` |
| Present SDK-decoded empty string | `empty` |
| Valid typed identifier / nonempty bounded text | `present` with typed value |
| Nonempty bounded identifier rejected by its lexical type | `malformed` with original SDK-decoded `ProviderText` |
| More than 2,048 decoded UTF-8 bytes | `unrepresentable` / `text-bytes` |
| Contains NUL, within the byte bound | `unrepresentable` / `contains-nul` |

Byte overflow takes precedence over NUL when both apply. No strings are trimmed,
truncated, lowercased or filled from requested values. Whitespace identifiers are
malformed. Unknown enum literals remain present bounded text, not compliant defaults.
`Malformed` must contain a nonempty value that fails the field's typed parser.
Literal text fields reject Malformed and reject Present(empty); Empty has its own
representation. Identity lexical validators are reused unchanged.

Unrepresentable markers retain no offending text; valid siblings survive. They
are explicit incomplete-normalization evidence, not successful truncated values.
The later #1413 executor must apply the foundation's stop-on-bound and incomplete
coverage requirements. This issue does not implement that executor or coverage
finalization. Record/list/aggregate overflow rejects; no fitting prefix is returned.

KMS owns six independently observed members: ARN, AWS account, manager, key spec,
usage and state. `KeySpec` is not filled from deprecated CustomerMasterKeySpec.
The pinned SDK preserves optionality of these fields, so KMS needs no presence
inspector. Missing/null optional values remain unavailable; no semantic absence is
inferred. Missing metadata is distinct from a present metadata object with missing
members.

IAM profile ARN and ID are independent. Roles is either unavailable or a present
bounded list. Each returned member produces one `IamRoleEvidenceV3` with independent
ARN and ID. Empty, malformed, duplicate and contradictory roles remain occurrences.
Returned list order is preserved, so permuting a list may change record identity.
Later policy must be independent of response ordering; this is not a policy verdict.

## Pinned correction audit

IAM stays at 1.113.0. Its generated sources are:

* `protocol_serde/shape_get_instance_profile.rs`;
* `protocol_serde/shape_instance_profile.rs`;
* `protocol_serde/shape_role_list_type.rs`;
* `protocol_serde/shape_role.rs`;
* `serde_util.rs`: `get_instance_profile_output_output_correct_errors`,
  `instance_profile_correct_errors`, `role_correct_errors`.

They synthesize an empty profile when the parent is omitted, empty strings for
omitted profile ARN/ID and role ARN/ID, and an empty roles vector for an omitted
collection. The role-list decoder preserves `member` order and occurrences.
SDK required-member declarations therefore do not prove wire presence.

KMS stays at 1.111.0; `types/_key_metadata.rs` and
`protocol_serde/shape_key_metadata.rs` preserve the relevant optional members.
The scalar SDK decoder owns XML unescaping/JSON decoding and enum spellings.

## Private IAM structural presence

`aws/iam_presence.rs` exposes only an AWS-private invocation factory and a consuming
receiver. Its scanner and output/presence pairing function are module-private.
There is no supplied-path selector, XML tree API, body retention, filesystem/network
access, identifier parsing or provider-policy logic in the scanner.

`read_before_deserialization` sees only the completed SdkBody returned by the
existing `BoundedHttp` connector. Non-success status produces no successful
presence observation. A streaming/unbuffered body rejects. Inspection is limited to:

```
GetInstanceProfileResponse/GetInstanceProfileResult/InstanceProfile
  Arn
  InstanceProfileId
  Roles/member
    Arn
    RoleId
```

Only booleans and an ordered vector of role presence pairs survive inspection.
Parent presence is required because the SDK corrects the parent too. Unrelated
subtrees cannot contribute flags. Local-name path matching agrees with the pinned
Smithy decoder; prefixes are checked for balanced closing tags but do not become
provider namespace assertions. No semantic values are read by the inspector.

The direct `xmlparser=0.13.6` dependency is the already-resolved tokenizer used by
Smithy XML. Its archive SHA-256 is
`66fee0b777b0f5ac1c69bb06d361268faafa61cd4682ae064a171c16c433e9e4`.
Only the host-lifecycle dependency edge changes; existing dependency versions and
checksums do not. The scanner adds explicit structural checks because this tokenizer
does not itself validate balanced tags or duplicate attributes.

Memory/work bounds are the existing 1-MiB body, a 128-frame borrowed-name stack,
128 attributes for the current start tag, and 128 role-presence pairs. Traversal is
linear, without a DOM or recursive descent. Clock/session/latched-round checks occur
before scanning, every 64 tokens and after scanning. These are conservative private
inspection limits, not increases to any provider/transport contract.

Mismatched/unclosed tags, duplicate monitored singletons/parents/collections,
duplicate attributes and ambiguous paths reject. Repeated role members are not
singletons. DTD/entity declarations, non-UTF-8 declarations, scalar child elements,
monitored CDATA, non-whitespace container text, fragmented scalar text and non-namespace attributes on monitored
elements reject explicitly. The scanner does not invent semantics for xsi:nil.
Ordinary escaped text is decoded only by the SDK; unsupported scalar forms are
never reconstructed by a second decoder. Error diagnostics contain no body text.

Every raw attribute value must contain no ampersand (`&`). This restriction applies
before SDK deserialization to all attributes on all elements: response/result
wrappers, profile/role containers, identity fields, ignored siblings and ignored
subtrees, including namespace declarations and otherwise permitted attributes.
The existing attribute-name, duplicate, count, nesting and monitored-element policy
checks still apply. Plain and empty values are accepted where those checks permit.
Named and numeric references are unsupported in attributes, even when valid XML
(for example `&amp;`, `&#38;` or `&#x41;`). This is an intentionally narrower input
grammar, not a claim that every rejected attribute is malformed XML. Escaped
element text remains supported and SDK-owned; `<Arn/>` remains Empty and an
omitted Arn remains NotReturned.

The pinned `aws-smithy-xml 0.60.15` `decode.rs::next_start_element` evaluates
attribute values using `unescape(value.as_str()).ok()?`. An unescape error becomes
None, terminating generated IAM traversal without necessarily returning an SDK
error. IAM correction can then manufacture empty defaults for unread identifiers.
`xmlparser 0.13.6` tokenization alone does not validate attribute references.
The pinned `unescape.rs::unescape` returns borrowed input immediately when no `&`
is present, so the accepted attribute grammar excludes this silent-error path.
The scanner only tests for the forbidden syntax; it never unescapes, reconstructs,
retains or interprets an attribute value. The rejection diagnostic is static and
contains no attribute/response text. The existing invocation failure state prevents
capture reuse, SDK retries remain disabled, and already charged request/body bytes
are retained after rejection.

Each invocation has fresh state and a single consuming receiver. The interceptor
verifies the GetInstanceProfile input type, captures once, then uses the generated
output from the same deserialization context. Role cardinality and corrected
defaults must agree with the presence evidence. Normalization occurs inside that
context; no API accepts an independently supplied presence map or SDK output.
Unexpected repeated invocation/response, type/cardinality disagreement, poisoned
state or unfinished lifecycle rejects. The receiver is consumed only after the
corresponding SDK send succeeds; its own round-time check remains mandatory.
Errors/cancellation cannot be retried by resetting this capture. No SDK client or
request builder is exposed by the capture.

## Canonical identity and containers

The original canonical encoder remains authoritative: sorted ASCII keys, exact
escaping, unsigned numbers, compact UTF-8 and one final LF. New records reject
unknown fields, incompatible schema versions and noncanonical input.

`EvidenceIdentityV3` covers Observation or Inventory with schema 3 and exact bytes.
`EvidenceReferenceV3` explicitly distinguishes ObservationV2, ObservationV3,
InventoryV2, InventoryV3 and CoverageV1, validating inner kind/version/length.
An old identity cannot acquire new semantics through the new reference type.

`ProviderObservationV3` is an in-memory, nonserializable mixture of V2 records and
the narrow V3 records with V1 coverage. It orders actual record canonical bytes,
allows equal duplicates, rejects superseded V2 Key/Profile records in current
observations, and checks per-query occurrence credit. One successor response record
requires one enclosing occurrence plus each role occurrence. Pure normalizers
report this count; #1413 must charge runtime occurrences before any filtering.

The aggregate limit includes the original record encodings and coverage, not a
new serialized wrapper. V3 inventories/context exist solely because their reference
fields must accept successor identities. They retain V2 attachments and unchanged
authority/launch/artifact facts. Historical V2 references are explicit, not migrated.
No inventory resource is fabricated for a partial returned identity.

Unchanged ceilings: 16-KiB record, 256-KiB normalized aggregate, 4,096 occurrences,
128 repeated elements, 64-KiB complete inventory, 32-KiB context and existing
reference/count bounds. V3 record accounting shares the existing round and latched
failure state with V1/V2, without refunds or resets. Canonicalization/identity are
integrity operations, never evidence authenticity or durable publication.

## Validation and exclusions

`provider_v3_contracts` uses independently specified JSON/SHA-256 fixtures. Unit
tests under `aws::identity_observation_tests` and `aws::iam_presence::tests` exercise
real pinned SDK deserialization with synthetic replay transport, including omissions,
empty values, independent malformed siblings, duplicates, bound failures and
correlation rejection. Existing V1/V2 fixture suites remain mandatory.

Validate with Rust 1.92, locked/offline dependencies and an external target directory,
including Linux clock/confinement checks. No real AWS traffic is needed.
No #1413 executor, production IAM/KMS adapter, EC2 normalization, discovery,
A01–A38 evaluator, reconciliation, inventory construction, durable publication,
controller, mutation, cleanup, host readiness or Chromium qualification is added.
