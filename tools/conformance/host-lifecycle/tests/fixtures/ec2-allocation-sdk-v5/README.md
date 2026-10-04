# V5 allocation protocol fixtures

Eight synthetic EC2 Query success bodies for pinned aws-sdk-ec2 1.237.0. No body is
live AWS evidence. The production ObservationSession tests use the bounded connector,
SDK decoder, invocation integrity guard, normalizers and shared query executor.

The rich bodies exercise the owned surface documented in
`docs/conformance/ag9g0e2c-ec2-allocation-observations.md`: partial/contradictory IDs,
unknown enum literals, signed numeric values, nested objects, both attachment sides,
management/requester fields and source-specific collection occurrences. Instances
includes a secondary interface and therefore has incomplete unsupported coverage.
Inference device entries deliberately use the pinned decoder's `member` spelling;
other owned list entries use `item`. InstanceAttribute user data decodes to the
literal bytes `TQ==`, so an accidental second decode changes the expected value.

Instance private/public IPv4 and IPv6 summaries deliberately disagree with embedded
ENI address facts. Assertions check their independent values and the owned fields
of every observation variant, not only variant counts or canonical round trips.
Image EBS fixtures own mapping/object-presence/snapshot facts only. AMI provisioning,
Reservation requester metadata and DNS display names are excluded; dedicated wire
regressions inject them, including oversized requester/DNS strings, and prove they
do not change canonical evidence or create field-specific limit failures. Volume
provisioning, ENI requester/operator facts and private-DNS configuration remain owned.

Boundary tests construct additional omission/empty/malformed/duplicate/oversized
variants from these bodies or explicit small response strings. They assert request
shapes, source credit, retained evidence and coverage separately. Historical SDK and
provider fixtures are not rewritten by this issue.
