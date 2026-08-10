# AE13e pinned external parser proof

This directory contains a deliberately small, pinned proof of the external
adapter boundary. The vendored `.dat` files are exact upstream WPT source
files. `allowlist.toml` is authoritative for the upstream project, exact
revision, source-file hashes, licence/attribution metadata, and selected
record identities; it is not a complete WPT import.

`allowlist.toml` selects three records by source-file ordinal and record
SHA-256. The adapter reads those records, preserves the upstream error-count
semantics, and emits ordinary Borrowser fixture-v3 declarations. The emitted
declarations are then loaded by the canonical fixture validator and executed
by the production parser observation runner.

The generated fixture bundles under
`crates/html/tests/fixtures/html5/external-wpt/` are derived artifacts. Do
not edit them by hand; use the explicit external-fixture update/check command
documented in the AE13e contract.

WPT source material is distributed under the included 3-Clause BSD licence.
The adapter's provenance records preserve the source revision, path, record
identity, hashes, attribution, and representation-only adaptation.
