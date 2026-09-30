# Partial identity V3 independent vectors

These synthetic canonical JSON documents and SHA-256 files were specified independently
of the Rust encoder using sorted compact JSON and a terminal LF. Tests must compare
against these bytes, not regenerate expectations from the implementation.

`member-states` covers every scalar state. `key` has an omitted returned ARN and
a contradictory account/state, independently of its exact query. `profile` has an
explicitly empty ARN, usable ID, malformed role ID, partial role and a duplicate role.
`coverage` counts the profile plus all three roles and remains schema 1.
`references` distinguishes V2/V3 observations and inventories plus V1 coverage.
`inventory` reuses unchanged V2 attachment bytes; it is a supplied container example,
not an inventory-construction result. `context` keeps generation-2 authority/launch
facts but binds evidence-policy/schema 3 and successor references.

Fixtures grant no authority. They neither migrate historical evidence nor claim
provider authenticity, admission, reconciliation or qualification.
