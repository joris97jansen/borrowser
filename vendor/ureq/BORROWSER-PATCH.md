# Borrowser's pinned ureq response-header correction

Borrowser issue #1123 vendors the published `ureq 2.12.1` package from
`https://static.crates.io/crates/ureq/ureq-2.12.1.crate`.

- Archive SHA-256: `02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d`.
- Published VCS revision: `ba4555d54fd72e16fc737f7e8fcc812efb131260`.
- Original MIT and Apache-2.0 license files are retained.
- `UPSTREAM.json` records all 54 published file hashes, the four packaging
  omissions, and the two corrected source hashes. Upstream CI workflows,
  `.gitignore`, and its standalone lockfile are omitted; Borrowser's root
  manifest and lockfile select this dependency. No upstream dependency or
  feature declaration is changed.

`response-header.patch` is the complete source diff from that archive:

1. Require a colon and a nonempty token field name before constructing a Header.
   The delimiter index is inside the line, including when its value is empty.
   Error messages contain no supplied field contents.
2. Propagate header-construction errors from response-head parsing before any
   compression, connection or framing accessor can inspect the fields.

This rejects malformed field names and colonless fields with existing
`ureq::ErrorKind::BadHeader`. Borrowser maps that category to its controlled,
redacted `MalformedResponse` terminal event. No accessor panic is caught or
converted after the fact. The HTTP backend, dependencies, value-access rules,
framing, decoding and redirect behavior otherwise remain unchanged.

`make check-network-dependencies` verifies the retained source hashes, patch
record and actual locked Cargo selection. `crates/net/tests/backend_response_headers.rs`
tests the real backend parser directly; Browser AI3 tests also exercise actual
loopback execution and resource settlement on all three routes. These checks
do not qualify the complete upstream test suite or all HTTP framing behavior.

AI5 #1125 owns broader backend qualification and any eventual replacement of
this local patch. Such a replacement must preserve the rejection, ordering,
empty-value and error-mapping regressions; it must not silently restore the
unsafe invariant or malformed-field omission.
