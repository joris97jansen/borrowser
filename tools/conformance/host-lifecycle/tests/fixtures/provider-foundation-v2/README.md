# Independent V2 contract vectors

These are supplied synthetic contract examples, not AWS authority or approval.
JSON objects were specified outside Rust, serialized with Python `json.dumps`
using sorted keys, compact separators, literal Unicode and one terminal LF, and
hashed with Python `hashlib.sha256`. No Rust encoder, normalization helper or V1
observation conversion produced their expected bytes or hashes. All strings in
these vectors are ASCII without control characters, so this independent JSON
procedure matches the canonical contract without relying on Rust escaping rules.

Unchanged root/launch/context leaf data comes from the existing synthetic context
fixture; the schema, evidence-policy version, prior state and references were
specified separately. This does not infer V2 evidence from V1 observations.

| Vector | Purpose |
| --- | --- |
| `instance` | managed=false with principal and independently hidden=true |
| `instance-eni` | present empty operator members; source lacks requester fields |
| `standalone-eni` | requester=false with requester identity; independent operator |
| `instance-ebs` | instance-side metadata, signed card index, managed=true without principal |
| `volume` | unavailable operator object |
| `volume-attachment` | missing instance/device despite management metadata; parent/child volume mismatch |
| `operator-states` | unavailable, synthetic absent, present empty, contradictory and partially returned states |
| `coverage` | unchanged V1 read coverage |
| `references` | ObservationV2, InventoryV2, CoverageV1 with exact target hashes/lengths |
| `inventory` | successor references plus duplicated contradictory attachments/history |
| `context` | nonempty prior-provider successor references bound into context identity |

Every `.sha256` is over the corresponding complete `.json` bytes including LF.
`references` identities target `instance`, `inventory`, and `coverage` respectively.
The context reference set contains those same identities in canonical order.
Tests compare supplied bytes and hashes; they do not regenerate expected values.

The Absent example is intentionally constructed contract data. No current audited
AG9g0e1a AWS normalizer produces it. Missing instance/device in the attachment vector
is Unavailable(NotReturned), regardless of the accompanying managed-resource facts.
