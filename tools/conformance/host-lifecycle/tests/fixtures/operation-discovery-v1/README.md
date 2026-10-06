# Operation discovery test inputs

These are test scenarios, not a durable discovery format. The empty reservation
fixture is hand authored and parsed into the frozen V5 record type. Its canonical
bytes are still owned by that type's existing encoder.

`aws/operation_discovery_tests.rs` supplies scripted EC2 responses through the
real bounded SDK reader/executor. Independently authored expectations count:

- three ENIs as three source occurrences and nine projections;
- three volumes with one attachment each as six occurrences and nine projections;
- three reservations with one instance each as six occurrences and twelve projections.

Omission cases retain the original executor coverage and accounting. Removing
the middle resource removes the only reference to `i-cccccccc`; the surviving
partial bundle passes the frozen aggregate validator but fails discovery's
representation audit. First, middle, final, parent, child, sibling, whole-page,
and whole-query omissions are tested separately from genuine empty pages.

The coordinator scenario independently returns A through ClientToken, B through
the operation tag, C through an operation-tag ENI, and additional distinct IDs
through the authority and volume paths. Request assertions prohibit combined
tag replacement and admission filters. Pure recomputation uses the retained
results and explicit final facts, with outer-order permutations preserving
every provider source position.

`aws/operation_discovery_review_tests.rs` adds a fully described foreign
instance/ENI/volume/profile-association component. Individual ownership, attachment,
profile and opaque-reference mutations preserve query coverage while blocking
exclusion. It also checks both selected DNS attributes against live reader outcomes,
seed/depth request ordering, shared request exhaustion, and combined report metadata
exhaustion below the former individual allowances.

`aws/operation_discovery_parent_tests.rs` reproduces the positional-parent defect
from that complete foreign component: an authority-query parent and its sibling
projections identify token-linked A while nested ENI/EBS projections still name B.
Eight additional reads run through the existing reader on the same session (four
empty lists and four attributes); supplied-record byte changes are charged
explicitly. The corrected result remains complete, retains A/B/ENI/volume and
positive linkage, and prevents exclusion. The unmodified foreign baseline remains
eligible for exclusion.

Focused fixtures distinguish same-source sibling edges, typed positional parent
edges, copied enclosing IDs and provider attachment references. They assert both
occurrence indices, exact query/page/path isolation, read reasons, source-derived
depths, independent VolumeIds, sibling-instance separation, missing actual parents
despite available sibling projections, unavailable IDs, and unsupported secondary
references. Outer permutations preserve source positions and inner order. Forty
ENIs account for 42 source occurrences and 84 projections before graph expansion;
the additional positional facts/links exhaust the existing allowance without
discarding evidence or enabling exclusions. These expectations come from the
authored responses and real reader/executor, not the discovery audit's counts.
