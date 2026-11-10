# Borrowser's cumulative ureq 2.12.1 correction

AI5 [#1125](https://github.com/joris97jansen/borrowser/issues/1125), based on master
`6c2789f3b90041fcc5dec2c53d5d2b1ee1c55194`. Record updated 2026-11-08.
This is the unmerged implementation candidate, not an upstream acceptance,
publication, full HTTP/Fetch compliance, or issue-closure claim.

## Published baseline and historical AI3 correction

The baseline is the exact published package at
<https://static.crates.io/crates/ureq/ureq-2.12.1.crate>:

- Archive SHA-256: `02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d`.
- Published VCS revision: `ba4555d54fd72e16fc737f7e8fcc812efb131260`.
- All 54 published file hashes remain frozen in `UPSTREAM.json`. Four original
  packaging omissions remain: `.github/dependabot.yml`, `.github/workflows/test.yml`,
  `.gitignore`, and the standalone `Cargo.lock`. All four MIT/Apache license files,
  including the chunked decoder licenses, remain byte-identical to the archive.
- AI3 #1123 first corrected `src/header.rs` and `src/response.rs`: reject colonless
  or invalid-name fields before constructing a Header, and propagate parsing
  errors instead of silently omitting fields. `response-header.patch` is retained
  byte-for-byte as **historical AI3 evidence**, with its original patched hashes
  in `historical_ai3_patched_files`. It is no longer the complete current patch.

The cached published archive was independently hashed and its complete file
manifest and VCS record compared with the frozen records. The cumulative diff is
constructed against that archive, not against AI3's already patched tree.

## Complete current patch inventory

`ai5-response-integrity.patch` supersedes the two-file patch as the authoritative
complete diff from published upstream to the selected vendor tree. Its SHA-256 is
`3ad93bedc381f0ce6df55b87c9f31c67b23b1b98775576c87c24b1c8450e6138`.
The ten final source/manifest hashes are recorded in `patched_files`.

| File | Current requirement and correction |
| --- | --- |
| `Cargo.toml`, `Cargo.toml.orig` | Remove rustls `logging` feature activation so debug/trace cannot expose certificate or wire material. Keep TLS, ring, std and TLS 1.2 configuration; do not upgrade packages. |
| `src/header.rs` | Preserve AI3's fallible field construction; validate token names and permitted field-value bytes; trim only boundary SP/HTAB; retain opaque bytes; parse Connection tokens across repeated/list-valued fields. |
| `src/response.rs` | Bound wire lines to 10 KiB and aggregate head to 64 KiB. Validate checked u64 Content-Length equality, Transfer-Encoding grammar and supported capabilities before readers/decoding. Preserve raw field occurrences through a narrow byte accessor. Distinguish 204/205/304; remove eager length-sized allocation/read and ordinary I/O panic paths. Fixed-length readers preserve payload and latch failure; verified completion owns optional pool return. |
| `src/chunked/decoder.rs` | Require payload, CRLF, zero chunk and final trailer termination; validate sizes/extensions/trailers with checked u64 accounting; bound metadata; preserve payload before later framing failures and latch failure. Keep trailers separate from the initial head. |
| `src/error.rs` | Concrete head-limit, unsupported-response and body-metadata-limit errors permit typed mapping without inspecting messages. |
| `src/pool.rs` | Preserve exclusive ownership and indexes; completed readers cannot lose payload to optional maintenance. Capacity decision and eviction share the existing mutex; destroy evicted transports outside it. Include private invariant/order/concurrency tests. |
| `src/stream.rs` | Default-deny observable transport eligibility, boxed delegation, non-consuming TCP inspection, no outer-buffer surplus, blocking/timeout restoration and optional safe pool insertion. Retain typed timeout causes. Debug output excludes endpoint and request material. |
| `src/rtls.rs` | Inspect public rustls plaintext/closure/handshake/write state with a nonempty reader probe; retain the same TLS object. Do not inspect private encrypted buffers or parse TLS twice. Control root-loading diagnostics. |
| `src/unit.rs` | Remove hidden replay after request writes begin; propagate prelude/body/flush/acquisition failures. Qualify checkout before transmission, restore request settings and retain proxy-aware pool identity. Replace raw URL/prelude/header/cookie diagnostics with controlled structural messages. |

The other 40 retained upstream files remain identical. No source or dependency
update beyond this inventory is admitted. The earlier AI3 invariant is retained,
not replaced with panic catching or post-response repair.

## Reproduction and fail-closed checks

`make check-network-dependencies` verifies pinned record digests, exact inventory,
all final content hashes, the actual locked local Cargo selection and resolved
workspace feature graph. It copies the retained source into temporary storage,
reverse-applies the cumulative patch and verifies **every** retained upstream
hash. It then forward-applies the patch and verifies the exact final tree.
Independently, applying the historical patch to the reconstructed clean tree must
reproduce AI3's original two-file hashes without changing other files.

For direct archive qualification (no download performed by the checker):

```sh
python3 tools/ci/check_ureq_patch.py --archive /path/to/ureq-2.12.1.crate
python3 -m unittest discover -s tools/ci/tests -p test_ureq_patch.py -v
```

An independent patch construction is: hash the archive; read its regular files;
compare all published hashes; for each changed retained path in sorted order,
produce `difflib.unified_diff` with two context lines (`n=2`), `a/` and `b/`
labels and original line endings.
Suppress the space marker on empty context lines (the standard
`diff --suppress-blank-empty` presentation), then concatenate those diffs. This
avoids whitespace-only lines in the patch record. `git apply -p1` on a clean retained package must reproduce
the selected files. Archive verification is optional for the routine offline gate;
its SHA, original manifest, patch bytes and final hashes are independently frozen
in the checker. The gate never derives new acceptance hashes from unchecked files.
Controlled mutation tests cover source/manifests, inventory, both patch records,
upstream reconstruction, archive identity, local/registry resolution and features.

## Supported boundary and maintenance limits

See [the authoritative AI5 implementation contract](../../docs/platform/ai5-real-http-response-acquisition-and-bounded-body-integrity.md)
for numeric limits, tests and ownership. These changes correct currently consumed
HTTP/1.x response acquisition; they do not add a general backend framework.
HTTP/1.0 and close-delimited responses are not pooled. Observable eligibility is
momentary, not a guarantee about arbitrary future peer behavior. Incomplete TLS
record input is permitted; queued encrypted socket input conservatively prevents
reuse without being labelled malformed HTTP.

Borrowser disables ureq redirects and automatic content decompression through its
selected configuration. Shared Fetch owns rejection/decoding; ureq's unused public
redirect/compression capabilities are not an alternate supported Borrowser path.
Gzip and encoded limits are in `net`, not additional vendor changes.

Production net tests exercise the selected parser, framing, pool and transports.
At Step G, the five new private pool tests were **not counted as executed**:
an isolated copied vendor tree with the workspace lockfile failed offline
resolution because `hootbin` was unavailable, before compiling tests.
Step H prepared the missing registry metadata and test dependencies outside the
repository, retained the selected runtime package versions and URL patches, then
compiled the actual unchanged `#[cfg(test)]` sources. All ten `pool::tests::`
tests passed offline, including the five new invariant/concurrency tests. The
other 115 compiled library tests were filtered, not claimed as executed.
See the contract's [Step H vendor qualification](../../docs/platform/ai5-real-http-response-acquisition-and-bounded-body-integrity.md#private-vendor-pool-tests)
for commands, isolated lockfile identity and feature differences.
No full upstream test-suite qualification or upstream acceptance is claimed.
