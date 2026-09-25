# Synthetic V2 contract vectors

These files freeze reviewed serialization examples, not production approvals.
`authority-v2`, `deployment-v2` and `genesis-v2` remain byte-identical to Pass 1.
`reviewed-deployment-v2` adds the static support required by Pass-2 constructors.

The Pass-2 JSON and SHA-256 vectors were independently authored using explicit
synthetic values and a separate Python canonical encoder/hashlib, without importing
or executing the Rust implementation. They are checked in as exact bytes, including
one terminal LF. Normal builds/tests never generate or update them. In particular,
the embedded collector LF is escaped as `\u000a`, per the frozen canonical contract.

`client-token-binding-v2.json` freezes the exact domain-separated hash input;
`client-token-v2.txt` is its expected lowercase SHA-256 token plus terminal LF.
The final request binds the independently retained spec digest and token. Contract
tests compare constructors with these independent typed values and hashes.

`identity-trust-v2.json` intentionally uses `3000` (an empty ASN.1 sequence), **not**
an AWS certificate. Pass 2 establishes only byte/digest/metadata consistency. It must
not masquerade as cryptographic verification coverage; Pass 6 needs independent
AWS-format signed fixtures and must reject these synthetic certificate bytes.
Fixture capacity/type, cost, IDs, references, reviews and times have no admission,
pricing, host-readiness or provider-authenticity authority.


`dispatch-v2/` contains independently authored Pass-3 authorization and journal
vectors with retained SHA-256 digests. The event chain begins at the frozen Pass-1
genesis; the preparation references the exact frozen Pass-2 synthetic documents by typed
digest/length, plus the separately retained Pass-3 authorization artifact. Human
authorization has audit time only; dispatch clocks derive from the preparation
envelope. The
six outcome fixtures are alternatives at sequence 4, not consecutive events.
No Rust serializer generates these fixtures during tests. They are never production
approvals; reviewer, clock, source provenance and deployed identities are synthetic.
