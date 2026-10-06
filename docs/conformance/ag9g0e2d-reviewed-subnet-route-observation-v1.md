# A07 reviewed-subnet route observations V1

The frozen infrastructure boundary can describe the reviewed route table exactly,
but that cannot establish whether another table has an explicit association with
the reviewed subnet. This successor adds only that missing A07 observation. It
does not reinterpret V1 exact scopes, modify V4 schemas, or traverse other infrastructure.

`ReviewedSubnetRouteQueryV1` has schema version 1, operation DescribeRouteTables,
retained account/region and `association_subnet`, exactly bound to the reviewed
manifest. The only wire filter is `association.subnet-id=<reviewed subnet>`;
MaxResults is 10 and subsequent requests use the exact returned NextToken. There
are no route-table ID, VPC, owner, route-state or other admission filters. Both
reviewed explicit-subnet and main-inheritance modes require this independent read.
The original exact reviewed table read remains necessary.

`ReviewedSubnetRouteRecordV1` has schema version 1, this successor query, and the
existing `ObservationDataV4::RouteTable` data. Other V4 variants reject. The same
pinned EC2 structural correlation, semantic SDK decoder and route normalizer produce
the returned facts. Returned table IDs, owners, VPCs, associations and routes are
never substituted with reviewed values. The new record is not a V4 record and
cannot use historical query/coverage provenance.

`ReviewedSubnetRouteCoverageV1` has schema version 1, the exact successor query,
required flag, requests, pages, records, terminal flag and existing coverage status
vocabulary. `ReviewedSubnetRouteEvidenceV1` is an inert context-bound in-memory
carrier, not a durable envelope. The canonical encoder is unchanged. No historical
schema, identity, parser, validator, fixture, limit or dependency pin changes.

The private `QueryCore` admits both closed frontends into one shared query ledger
and observation round. It reserves the largest terminal coverage encoding before
I/O; rejected admission produces no coverage. Pagination, occurrence/output charges,
byte accounting, cancellation, session/time checks, first-failure precedence and
duplicate-query prevention share the historical machinery. No second budget or
SDK configuration is created for the successor.

Discovery ingestion counts this family together with all historical families before
encoding or indexing. Each represented route contributes one root plus actual owned
nested list occurrences. Exact count lower than coverage is valid partial evidence;
greater than coverage is inconsistent. Entirely missing roots/nonempty pages are
therefore gaps even without positional provenance. Genuine zero-source terminal
results remain valid. Failed queries retain valid partial route records. No final
route-association/admission verdict is implemented in #1416.

Transport tests compare normalized returned data with the original exact-table reader,
assert the sole subnet filter/MaxResults, test pagination cycles and shared latches,
and reject combined-family request/byte overflow. Final round validation is owned by
the [operation discovery coordinator](ag9g0e2d-operation-discovery.md).
