# AI5: Real HTTP Response Acquisition And Bounded Body Integrity

Current source management: [the AI subtree maintenance contract](ai-dependency-subtree-maintenance.md) governs Borrowser-owned ureq development, current qualification and upstream history. The dated AI3/AI5 package and patch records below remain historical evidence; HTTP behavior is unchanged.

Issue: [#1125](https://github.com/joris97jansen/borrowser/issues/1125).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation baseline: master `6c2789f3b90041fcc5dec2c53d5d2b1ee1c55194`.
Implementation qualification: 2026-11-08, unmerged feature branch
`feat/1125-harden-http-response-acquisition-and-body-integrity`.

## Authority and status

This describes the implemented AI5 candidate, superseding conflicting legacy
execution qualifications in [AI1](ai1-fetch-http-networking-response-contract.md),
[AI2](ai2-typed-fetch-request-construction.md),
[AI3](ai3-typed-fetch-response-status-headers-body.md), and
[AI4](ai4-network-backend-and-deterministic-fake-network.md). Their dated records
remain historical evidence. Steps A–G are accepted implementation checkpoints;
Step G records provenance, contracts and CI wiring. The Step H local evidence
below awaits final review. Hosted CI, publication, merge and issue closure are
separate; none is implied here.

Normative references are [RFC 9110 field syntax/list recipient rules](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.6),
[RFC 9112 framing and completion](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3),
[chunked coding](https://www.rfc-editor.org/rfc/rfc9112.html#section-7.1),
[persistence](https://www.rfc-editor.org/rfc/rfc9112.html#section-9.3), and
[WHATWG Fetch](https://fetch.spec.whatwg.org/#http-network-fetch).
The implementation is a bounded supported subset, not complete Fetch or HTTP
conformance. Numeric budgets, strict TE/CL rejection, unsupported coding rejection,
HTTP/1.0 pooling exclusion and temporary redirect rejection are Borrowser policy.

## Ownership and execution

- Addressing owns `BrowserUrl`, `ResourceAddress`, retrieval identity and native
  file conversion. AI2 request admission remains authoritative: GET without a
  body, admitted fields and initial credential rejection. HEAD and other methods
  remain unsupported. HTTP(S) use the HTTP backend; direct `file:` retains its
  separate existing loader. Other execution schemes fail request admission.
- `net::backend::HttpResponseBackend::acquire` returns existing immutable
  `NetworkResponseInfo` plus an owned fallible `Read`. `UreqBackend` acquires one
  selected response without automatic redirect execution or content decoding.
  Neither the backend nor a consumer owns a second event lifecycle.
- Shared `fetch_http_with_backend` performs response validation, redirect/content
  admission, Start, body handling and exactly one terminal Done or Error.
  `stream_reader` delivers route-bounded decoded bytes. HTTP framing belongs to
  the patched backend readers, not a second parser in Fetch.
- Browser owns consumer admission, document lifetime, stylesheet slots, navigation
  retirement and image decode/install. Runtime/bus only route existing typed
  commands/events. CSS, Layout and Paint own no HTTP or transport policy.

`FakeNetwork` uses that same shared admission, decoding, limits, cancellation and
terminal path. `FakeResponse` bytes now mean **content-encoded HTTP content after
transfer framing**, not necessarily decoded bytes. A gzip fixture must provide
actual gzip bytes. The fake does not parse HTTP framing or prove socket/TLS
correctness. It remains memory-only with no real-backend fallback; direct files
are not fake HTTP routes.

## Immutable response facts and acquisition

`FetchRequest` retains the AH original address, request ID, method, current target,
initiator/origin facts and existing issued provenance. The original requested URL
and selected reported response URL remain distinct; neither is reconstructed from
body length or diagnostic output. No redirect chain is invented.

`NetworkResponseInfo` shares immutable facts across Start, Done and contextual
Error. Ordered `HttpHeaders` retain repeated/interleaved occurrences, canonical
lowercase names and admitted opaque field-value bytes. Wire-parser boundary
SP/HTAB is trimmed according to field parsing; this is not an exact raw-head
capture. `headers_bytes` avoids ureq's lossy string-value projection.
Content-Encoding and Content-Length are preserved after admission and never
removed/replaced by gzip decoding. The latter describes encoded HTTP content
where applicable, not decoded progress. Body acquisition is not body completion.

404 and 500 are HTTP responses, not transport failures. Whether a completed
representation is suitable for a document, stylesheet or image remains consumer
policy. In particular, response completion alone does not make a 206 response a
complete representation or perform a 304 cache merge.

## Response head and framing selection

The pinned parser validates fields before selecting a body reader, decoding,
allocating from a length, or returning any stream to a pool:

- Token field names, permitted field-value bytes and mandatory delimiters are
  validated; colonless/malformed fields fail rather than disappear. Diagnostics
  exclude field contents. This preserves the AI3 malformed-header correction.
- Each wire head line is at most **10 KiB including CRLF**. Aggregate head is at
  most **64 KiB**, including status line and final empty line. The parser also
  caps field occurrences at 100. These precede separate typed metadata limits:
  64 fields, 256-byte names, 8 KiB values and 64 KiB reconstructed field storage.
- Content-Length is checked decimal **u64**. Identical repeated and comma-list
  members agree numerically, including values above u32::MAX. Empty, signed,
  malformed, conflicting and overflowing declarations fail, including invalid
  later occurrences. Missing length remains valid for close-delimited responses.
  A declaration never drives allocation; usize conversion bounds actual slices.
- Transfer-Encoding grammar includes parameters, quoted values and recipient
  empty-list tolerance across repeated fields. The bounded head limits the
  allocation-free scan. A present all-empty list fails. Repeated chunked and
  prohibited coding parameters fail. Missing separators/invalid grammar are
  malformed; otherwise valid unsupported stacks (including non-final chunked)
  are UnsupportedResponse. Body-bearing execution supports only one
  unparameterized chunked coding. No transfer decompression was added.
- Any TE/CL combination fails under the approved strict policy, even where an RFC
  recipient could choose transfer-coding precedence. HTTP/1.0 TE is rejected.
- HTTP/1.0 and HTTP/1.1 responses are supported; unsupported versions and all
  informational responses are rejected explicitly. No 1xx continuation, upgrade,
  HTTP/2 or HTTP/3 support is claimed.

Framing selection returns `Result`. Reader construction does not eagerly read a
Content-Length-sized body. Ordinary network I/O failures propagate without
`expect`, panic catching or release-unwind assumptions.

## Verified body completion and null-body statuses

Fixed-length readers retain u64 position/length accounting, deliver known payload
before optional maintenance, and fail on premature EOF. Chunked decoding has
explicit size, payload, CRLF, trailer, End and Failed transitions. A nonempty read
returns successful EOF only after complete validated framing. Missing payload,
zero chunk, delimiters or trailer termination fails. A framing error after a
payload read is reported on a later read so delivered bytes are not erased.
Failed readers remain failed. Empty-buffer reads do not advance state or pool.

Close-delimited HTTP completion requires transport EOF. It cannot prove semantic
application completeness: without a declared size, a peer closing early can be
indistinguishable from its intended end. Image/content decoders may independently
reject an incomplete representation. Close-delimited connections are not reused.

| Status | Implemented behavior |
| --- | --- |
| 204 | Reject Content-Length/Transfer-Encoding; head-terminated, no drain. |
| 205 | Reject nonzero declared length. Keep the actual fixed/chunked/close-delimited reader; verify complete framing and zero content before decoding. Any content byte fails before consumer delivery, retaining zero delivered progress. An open close-delimited 205 must time out, not emit Done. |
| 304 | Head-terminated, no drain. Admit valid Content-Length and syntactically valid Transfer-Encoding representation metadata without selecting those body decoders. Strict TE/CL conflict rejection remains. |

An empty library reader is not proof that no unsolicited wire bytes exist.
Connection eligibility independently checks known buffer/transport state.

## Chunk metadata and resource budgets

| Budget | Maximum, inclusive |
| --- | ---: |
| Chunk-size/extension wire line, including CRLF | 8 KiB |
| Individual trailer wire line, including CRLF | 8 KiB |
| Trailer section, including final CRLF | 64 KiB |
| Nonempty trailer field lines | 64 |
| Cumulative chunk framing metadata, excluding payload | 1 MiB |

Checked u64 hexadecimal parsing does not allocate from chunk size. Extensions
validate supported token/quoted-string grammar. Trailers are validated and
discarded separately, never merged into immutable initial metadata. The explicit
prohibited subset in `vendor/ureq/src/chunked/decoder.rs` rejects framing, routing,
connection, authentication, cookie, content-processing, response-control,
conditional and range fields. This is not a general trailer API. Independent
boundary tests prevent one budget from accidentally qualifying another.
`BodyMetadataTooLarge` propagates as ResourceLimit, including through gzip.

| Resource route | Encoded content | Decoded delivery |
| --- | ---: | ---: |
| HTML/document | 20 MiB | 10 MiB |
| CSS/stylesheet | 4 MiB | 2 MiB |
| Image | 40 MiB | 20 MiB |

Encoded bytes are counted **after transfer framing, before content decoding**;
TLS records, HTTP head and chunk metadata are not that byte unit. Decoded bytes
are the bytes delivered in Chunk events. Exact encoded budget plus valid EOF
succeeds; observing an actual byte beyond it fails. Budget exhaustion alone does
not prove excess. Neither declared length nor compressed size permits excessive
decoded delivery. Streaming does not buffer an entire representation; these are
per-operation bounds, not a process-wide memory/CPU/concurrency quota.

## Shared content decoding and redirect admission

Absent Content-Encoding means identity. Exactly one actual coding supports
identity, gzip or x-gzip, ASCII case-insensitively. RFC recipient empty list
positions are ignored within existing header bounds. A present all-empty list or
invalid token grammar is MalformedResponse. Unknown, stacked or repeated actual
codings are UnsupportedResponse, including across field occurrences.

Body-bearing coding rejection precedes Start/Chunk, preserving response facts and
zero progress in one Error. 204/304 ignore representation coding for body reads;
205 verifies raw zero content first. Shared `EncodedReader` and `GzipReader` use
pinned flate2 1.1.2. CRC, truncated input, concatenated members and invalid trailing
content are qualified. Empty concatenated members still consume encoded budget.

The private concrete causes `HttpBodyError::{EncodedLimit, SourceRead, Gzip}`
retain owned errors through nested decoder processing. Typed inspection, not
string matching, maps encoded/decoded/metadata excess to ResourceLimit and source
or gzip corruption/truncation to Read. Delivered decoded progress survives later
checksum/framing failure. Diagnostic projection does not expose decoder text.

The real agent hard-disables redirects independently of request intent. Shared
Fetch rejects 301/302/303/307/308 with **any Location occurrence**, including empty
or repeated values: UnsupportedResponse, available metadata, zero progress, no
Start/Chunk/Done and one Error. No target is contacted, including userinfo-bearing
targets. The request model's Follow intent remains representable but unsupported
for an actual redirect operation. Missing Location and other 3xx remain selected
responses. The misleading HttpClientPolicy redirect knob was removed. AI6 #1126
owns explicit per-hop admission/traversal; no partial redirect algorithm exists.

## Persistence, timeouts and TLS

Existing exclusive stream ownership provides one outstanding request and no
pipelining. Only verified framing completion can consider optional pool return.
HTTP/1.0, close-delimited, failed and abandoned incomplete transfers are unpoolable.
Repeated/list-valued request and response Connection fields honor close tokens,
case-insensitively, with recipient whitespace/empty-member handling. x-close is
not close. Pool identity retains origin/proxy route and agent-owned TLS settings.

Eligibility is checked before insertion and immediately before checkout, before
new request bytes. Ureq's outer buffer must be empty. ReadWrite inspection is
positive/default-deny, including boxed delegation. TCP uses a non-consuming
nonblocking peek and restores blocking mode; queued bytes, EOF, errors or failed
restoration reject reuse. Rustls 0.23.32 inspection uses public plaintext,
closure, handshake/write state and a **nonempty** reader probe. WouldBlock means
no plaintext currently available. It retains the same TLS connection/transport.
Private encrypted-input emptiness is not required: incomplete TLS records can
legitimately survive HTTP boundaries. Queued encrypted socket input conservatively
rejects reuse without classifying it as malformed HTTP.

These checks are momentary observations, not a guarantee against arbitrary
future peer behavior. Positive HTTP/HTTPS persistence, TLS 1.2/1.3, controlled
fragmented KeyUpdate records and known plaintext/ciphertext isolation are tested.
A completed response stays successful when optional pool maintenance fails; an
incomplete framing/transport failure does not become success. Atomic pool add and
eviction keep LRU/recycle consistent under the same mutex and destroy evicted
streams after unlocking. No payload is lost to optional maintenance.

Fresh and checked-out sockets restore blocking mode and configured read/write
limits. Defaults are connect **15 s**, read **30 s**, write **30 s**. Deadlines do
not override stricter configured limits or leak a prior request's deadline into
another. Setup failure prevents transmission on an unqualified socket. These are
not a whole-fetch deadline, bounded DNS cancellation or instantaneous I/O abort.
No hidden replay follows prelude/body writes, flush or response acquisition.
Discarding an unused stale candidate before transmission is not request replay.

`TlsTrustStore` preserves native roots, native plus explicit roots, or explicit
custom roots, with certificate validation and ring/TLS 1.2/1.3. No trust bypass,
TLS policy UI, client-certificate/authentication UI or revocation-policy claim is
introduced. Rustls logging is disabled in the resolved workspace feature graph.
Root diagnostics contain fixed messages/counts, not certificate details.

## Outcomes, cancellation and Browser settlement

An accepted operation emits at most one Start; Chunk requires Start; one terminal
Done or Error settles it. Admission failure can carry acquired metadata without
Start. Post-Start failure retains the immutable snapshot and actual decoded
BodyProgress. No Done follows framing, decoding, limit, read or cancellation
failure. Public NetworkErrorKind categories are unchanged; AI7 #1127 owns broader
staged taxonomy and cancellation/delivery policy.

Cancellation after Start directly constructs StreamFailure with Cancelled and
route-correct zero progress before gzip header acquisition. Midstream and 205
checks retain existing semantics. A token does not interrupt a currently blocked
OS read; the configured timeout/next checkpoint governs observation. Runtime
channel delivery is not a downstream acknowledgement protocol.

Network diagnostics retain request ID, resource kind/role, operation stage and
stable category, excluding raw URLs, credentials, field values, request preludes,
reason phrases and arbitrary decoder/source messages. Backend/TLS/body causes
remain concrete internally until controlled projection. Best-effort stderr writes
cannot prevent terminal delivery merely because the sink rejects writes. Typed
semantic response metadata is not redacted or rewritten.

Actual runtime/bus tests prove partial document failure prevents successful parse
completion, partial CSS is aborted and cannot be revived by delayed CssDone, and
sibling resources remain independent. Replacement navigation retires old Loading
images after successful dispatch and before generation advancement. Request-ID
checks prevent late cancellation from removing a new same-URL operation. Image
DecodeResult retains image::ImageError; consumer status is controlled and failed
decoding cannot install a Ready texture. This does not implement the full AJ/AK
navigation/resource lifecycle or active-document preservation on 204/205.

## Provenance, CI and qualification limits

[The vendor record](../../vendor/ureq/BORROWSER-PATCH.md) explains the exact ten-file
cumulative correction, unchanged 54-file published manifest, four omissions and
historical AI3 patch. `make check-network-dependencies` verifies exact inventory,
source/patch hashes, reverse/forward reconstruction, locked local ureq selection,
net-owned flate2 1.1.2 and workspace all-feature/unfiltered-target metadata.
Independent `cargo tree --workspace --all-features --target all -e features`
qualification confirms no ureq automatic gzip or rustls logging activation.

The existing Linux Rust CI job retains URL/network verification, formatting,
all previous lint/test/build lanes and its 30-minute job deadline. It prepares
all-feature metadata before offline verifier tests and additionally lints the
observation-qualified Browser example. `make qualify-network-release` builds the
actual example with release/conformance-observation, reuses release dependency
artifacts, then executes it. Its hosted step has a six-minute outer deadline.

`ai5_abort_probe` verifies cfg(panic = "abort") and actual SIGABRT, executes 30
HTTP/HTTPS failure workers, two persistence/concurrent-completion workers, two
closed-stderr workers and two malformed-image workers. Six additional supervisor
controls distinguish ready timeout, missing readiness, premature exit, exact
64 KiB output, one-byte excess and larger-output drainage. The helper owns finite
execution, termination/reaping and drainage deadlines; only finished drainers
are joined. Output retention is 64 KiB per pipe; excess continues draining and
fails qualification. Workers spawn threads, not descendant processes; drainage
still has an independent failure deadline. Failed OS cleanup is explicit failure,
not an assertion that a child was reaped.

Local accepted A–F evidence: 230 net tests and 394 Browser/runtime tests passed;
all 36 workers, abort and supervisor controls passed on macOS under the real
release profile. Step G reruns the affected lanes, provenance negative cases,
feature graph, formatting, Clippy and the same Make release target. These results
are local evidence, not executed hosted CI, Linux/Windows qualification, a green
full workspace CI aggregate, merge, or issue closure. Hosted execution and final
cross-lane evidence remain Step H.

At the Step G checkpoint, private vendor tests remained a qualification gap:
a copied standalone vendor plus workspace lockfile could not resolve unavailable
offline `hootbin`, before compiling tests. The five new pool tests were not
counted as passing. Production-path net tests could not establish their
compilation. The Step H record below resolves this gap without modifying the
selected vendor source or production dependency selection.

## Step G local qualification record (2026-11-08)

Rust/Cargo 1.92.0, local macOS execution. These are candidate checks, not hosted
Linux qualification or Step H final acceptance.

| Check | Observed local result |
| --- | --- |
| Published archive identity, VCS and original manifest | Pinned SHA-256 and all 54 published hashes match. |
| Independent forward application to archive-derived clean source | All 50 retained files match selected bytes; 40 unchanged, ten patched; all license files retained. |
| `make check-network-dependencies` and optional `--archive` checker | Pass: original manifest, both patch records, reverse/forward reconstruction, locked package and actual features. |
| `python3 -m unittest discover -s tools/ci/tests -p test_ureq_patch.py -v` | 32 passed, including controlled negative mutations. |
| Complete CI-tool discovery and existing URL gate | 64 tests passed; URL source/graph gate passed. |
| Focused `net --lib ai5` / Browser `ai5f_` tests | 92 / 4 passed; these are subsets of the totals below. |
| `cargo test -p net --no-fail-fast --locked --offline` | 230 passed, no failures or ignores. |
| Browser/runtime lane (`browser`, `runtime_net`, `bus`, `runtime_css`, observation feature) | 394 passed, no failures or ignores. |
| Net and observation-qualified Browser all-target Clippy, `-D warnings` | Pass; existing dependency warning remains visible. |
| `make fmt-check`, targeted vendor rustfmt and whitespace checks | Pass; new untracked records are checked separately. |
| `CARGO_NET_OFFLINE=true RUST_BACKTRACE=1 make qualify-network-release` | Pass: actual release build; 36 original workers, SIGABRT and all six supervisor controls. Ready stall is SIGKILL/reaped/drained; only intentional controls time out. |
| Workflow wiring | YAML parsed locally with Ruby/Psych; enabled Linux matrix, existing gates and timeouts checked. actionlint is unavailable locally; no hosted run is claimed. |
| Standalone copied-vendor `cargo test --lib --locked --offline pool::tests::` | Blocked at unavailable hootbin resolution before compilation; zero private vendor tests qualified. |

The all-feature metadata and independent all-feature/all-target cargo-tree results
agree on ureq `charset,json,native-certs,tls`, rustls `ring,std,tls12`, and shared
flate2 1.1.2. Flate2's default/rust_backend unification also comes from image's
existing PNG dependency; it does not reactivate ureq's automatic gzip.
All 52 accepted A–F files and the index fingerprint remain unchanged by Step G.
No full `make ci`, hosted CI, merge or issue closure is claimed. Historical native
macOS Chromium CI failures remain historical evidence, not a new Step G pass.

## Regression map and explicit remaining ownership

| Evidence | Current invariant |
| --- | --- |
| `net/tests/backend_response_fields.rs`, `backend_response_integrity.rs` | Actual production field/framing parser, exact head budgets, large u64 declarations, recipient list grammar and status distinctions. |
| `net/tests/backend_body_completion.rs`, `net/src/tests/ai5.rs` | Production reader completion, chunk/trailer boundaries, progress, 205 verification and HTTP/HTTPS timeout behavior. |
| `net/src/tests/ai5_connections.rs`, `ai5_pool.rs`, `ai5_tls_reuse.rs` | Actual reuse/isolation, pool identity/order/concurrent completion, faults, timeout restoration and no replay. |
| `net/src/tests/ai5_decoding.rs`, `stream.rs`, `net/tests/fake_network.rs` | Shared real/fake admission, raw metadata, gzip completeness and both byte budgets. |
| `net/src/tests/ai5_diagnostics.rs`, `browser/src/tab/tests/ai5_failures.rs`, `browser/src/resources.rs` | Controlled output, concrete causes, real runtime settlement, image navigation and decoder failures. |
| `browser/examples/ai5_abort_probe.rs` | Actual release-abort, supervised process liveness, network outcomes, Browser settlement, attempts and persistence. |
| `tools/ci/tests/test_ureq_patch.py` | Fail-closed source/record/selection mutation checks and actual selected feature graph. |

Paths in that table are relative to `crates/`, except `tools/`.
AI6 #1126 owns redirect traversal and per-hop admission; AI7 #1127 owns broader
errors, cancellation and timeout policy; AI8 #1128 owns aggregate loader metadata;
AI9 #1129 owns AG networking integration. AJ/AK own full document/subresource
semantics and image lifetimes; AT owns cache/304 merging. Cookies, CORS/CSP,
mixed-content/security-policy enforcement, authentication UI, JS fetch/XHR,
service workers, WebSockets, HTTP/2/3 and new loading/rendering features remain
outside this issue. No separate implementation-phase issue is required.

## Step H final local qualification (2026-11-08)

Steps A–G are formally accepted. Step H performed final integrated local
qualification on macOS arm64 with Rust/Cargo 1.92.0. Final review, publication,
hosted qualification and merge are still pending. This record does not mark
issue #1125 closed. No networking, Browser, test, dependency, provenance or CI
implementation was changed during Step H; only this contract, the tracker and
the vendor maintenance note received qualification updates.

The qualified implementation has 65 staged paths, based on master
`6c2789f3b90041fcc5dec2c53d5d2b1ee1c55194`. HEAD, merge base, local master,
cached origin/master and live remote master all matched that commit.
The accepted staged binary diff SHA-256 is
`c566671457aa91de9b37b38807a02931b1a7d8998555d3b9afea1595dd2b7e4b`;
its candidate tree is `44469466e7d13ea5a15c1f98e2c1a7026eb8009b`.
These identify the accepted implementation before the three unstaged Step H
documentation updates. The final review packet records the resulting combined
diff/tree separately; a document cannot contain its own final content hash.

### Requirements and evidence

The net and Browser totals below are complete lanes, not sums of overlapping
focused reruns. A source guarantee, a test assertion, an executed local lane and
a hosted result are different evidence. Paths in this table are relative to
`crates/`, except `vendor/`, `tools/`, `.github/` and `Makefile`.

| Requirement | Production owner | Executed evidence | Limit or remaining owner |
| --- | --- | --- | --- |
| Real HTTP/HTTPS GET; typed requests/responses | `net/src/{request,backend,fetch}.rs` | Net real HTTP/HTTPS, request/backend adapters; release workers | GET only; HEAD/other methods remain unsupported. |
| Immutable original metadata and identity | `net/src/{backend,headers,response}.rs` | Response adapters, fields, decoding and event tests; net lane | Acquired facts remain distinct from consumer interpretation; AI8 #1128 owns aggregate metadata. |
| Head/length/transfer validation | `vendor/ureq/src/{header,response}.rs` | `net/tests/backend_response_{fields,integrity}.rs`; release malformed-head workers | Explicit HTTP/1.0/1.1 subset and strict TE/CL rejection; numeric budgets above. |
| Fixed/chunked completion; trailers | `vendor/ureq/src/response.rs`, `chunked/decoder.rs` | `backend_body_completion.rs`, `net/src/tests/ai5.rs`; truncation/metadata release workers | Exact metadata/field-count boundaries covered; close-delimited EOF cannot prove application completeness. |
| 204/205/304 | Backend framing plus `net/src/fetch.rs` | Head/status tests, 205 raw-zero verification and HTTP/HTTPS timeout tests | No cache merge or full active-document semantics; AT and AJ/AK retain those concerns. |
| Encoded/decoded limits | `net/src/{limits,stream}.rs` | Identity/gzip route-boundary tests for all three routes; typed progress checks | Per-request bytes, not global CPU/memory/concurrency limits. |
| Gzip completeness/corruption | `net/src/stream.rs` | `ai5_decoding.rs`, private reader tests; gzip release workers | CRC, truncation, trailing data and concatenated members covered; no other content formats. |
| Real/fake admission parity | `net/src/{backend,fake,fetch}.rs` | Real transport, fake-network and shared decoding tests | Fake content is after transfer framing; it is not transport qualification. |
| Redirect rejection/attempt visibility | `net/src/{agent,fetch}.rs` | All supported redirect statuses/Location cases, credential target, real/fake tests; release request counts | No target contact or traversal; AI6 #1126 owns per-hop processing. |
| No hidden replay | `vendor/ureq/src/unit.rs` | Write/flush/acquisition faults, request counts, release failures | Discard before transmission is allowed; no general retry policy. |
| HTTP/HTTPS persistence/isolation | Vendor stream/readers/pool/rustls | `ai5_connections.rs`, `ai5_pool.rs`, `ai5_tls_reuse.rs`; release persistence workers; private pool tests below | Actual reused identities asserted; observable eligibility is momentary. |
| TLS state/trust | `net/src/tls.rs`, `vendor/ureq/src/rtls.rs` | Trust failures, TLS 1.2/1.3, buffered plaintext/ciphertext and fragmented KeyUpdate tests | Public APIs only; no private encrypted-buffer attestation or broader TLS UI. |
| Timeouts/maintenance | Vendor stream/unit/pool/readers | Fresh/reused timeout, configured-vs-deadline and maintenance faults; private eviction tests | Optional maintenance cannot fail a completed response; full policy/DNS cancellation remains AI7 #1127. |
| Controlled diagnostics/typed causes | `net/src/{backend,stream,log,tls}.rs`; vendor logging | Four net diagnostic tests, nested body-error tests, closed-stderr release workers | No raw URL/header/decoder strings in diagnostics; authorized typed metadata retained. |
| Cancellation/terminal identity | `net/src/fetch.rs` and existing stream/events | Pre-decoder, midstream and 205 cancellation; one terminal outcome/progress assertions | No interruption of a blocked OS read or downstream acknowledgement; AI7 #1127. |
| Document/CSS/image settlement | Browser handlers and existing runtime/bus | Complete 394-test lane, `ai5_failures.rs`; release Browser observations | Failure cannot publish partial success; broader AJ/AK consumer semantics remain deferred. |
| Navigation image retirement | `browser/src/{resources,tab/nav,tab/image}.rs` | Generation retirement, same-URL stale cancellation and typed image decode tests | Current supported navigation behavior, not a complete new lifecycle. |
| Actual release panic-abort | `browser/examples/ai5_abort_probe.rs` | Release build, real SIGABRT, 36 workers and six supervisor controls | Local macOS evidence only; bounded supervision, not panic catching. |
| Dependency/source provenance | `tools/ci/check_ureq_patch.py`, frozen vendor records | 38 negative/positive provenance tests, 70 CI-tool tests, optional archive reconstruction, all-target feature graph | No digest or source change; exact pinned local selection remains enforced. |
| CI enforcement | `.github/workflows/ci.yml`, `Makefile` | Actionlint 1.7.7 and local equivalents below | Hosted Linux execution pending; no enabled hosted Windows/macOS lane. |

### Private vendor pool tests

The previously uncompiled five-test gap is resolved, without changing accepted
vendor source, manifests, lockfile, patch bytes or trusted digests. A temporary
copy of `vendor/ureq` received the workspace lockfile and only the existing root
URL/percent-encoding `[patch.crates-io]` entries. Both retain fork revision
`92f7fb395f9ffb2f706e7c664283781fd3b294a2`; no self-patch for ureq is needed when
the copied package is the root under test.

Dependency preparation was online, separate from execution. Cargo needed metadata
for optional inactive `hootbin` 0.1.5, plus standalone dev dependencies including
env_logger 0.9.3 and humantime 2.4.0. `testdeps` was not enabled, so hootbin was
not compiled. No public HTTP service was used by the tests.

For the isolated copy only, preparation used:

```sh
cargo metadata --manifest-path "$ai5_testdir/ureq/Cargo.toml" \
  --no-default-features --features tls,json,charset,native-certs --format-version 1
cargo update --manifest-path "$ai5_testdir/ureq/Cargo.toml" \
  -p native-tls --precise 0.2.14 --offline
cargo update --manifest-path "$ai5_testdir/ureq/Cargo.toml" \
  -p security-framework-sys --precise 2.15.0 --offline
cargo update --manifest-path "$ai5_testdir/ureq/Cargo.toml" \
  -p bitflags@2.13.2 --precise 2.9.4 --offline
cargo metadata --manifest-path "$ai5_testdir/ureq/Cargo.toml" \
  --no-default-features --features tls,json,charset,native-certs \
  --locked --offline --format-version 1
cargo test --manifest-path "$ai5_testdir/ureq/Cargo.toml" --lib \
  --no-default-features --features tls,json,charset,native-certs \
  --locked --offline pool::tests:: -- --test-threads=1
```

Here `ai5_testdir` is an isolated temporary directory, not a repository path.
The precise updates undo newer shared package selections introduced by resolving
the standalone package's inactive optional native-tls dependency. All normal
runtime dependency package versions, sources and registry checksums were compared
with the actual selected workspace ureq closure and match. Build feature
unification is not identical: standalone cc lacks workspace parallel/jobserver
and their transitive edges. This is a library test-profile qualification, not a
claim of byte-identical workspace artifacts. Production release behavior is
qualified separately by the actual release example.

The isolated manifest SHA-256 is
`35d771ddc05a1d599e3983904548fb5b43ba902d9385aa5c16f6588774bafbb3`;
its lockfile SHA-256 is
`3fa3c5fae6cfc83dd17162bdd058970d4f12b0e26a61dc86738d6e059a67ad87`.
The local evidence bundle retains both files, metadata and command logs.
Every copied `src/` file, including concrete `#[cfg(test)]` implementations,
was checked byte-for-byte against selected vendor source.

Result: **ten pool tests passed, zero failed/ignored, 115 other compiled library
tests filtered**. All five new tests executed:

- `concurrent_add_and_checkout_preserve_pool_invariants`
- `evicted_transport_destruction_happens_outside_the_pool_lock`
- `global_eviction_and_checkout_preserve_both_indexes_and_order`
- `per_host_eviction_preserves_the_oldest_surviving_entries`
- `zero_capacity_never_leaves_stale_index_entries`

The other five existing pool tests also passed. The inherited unused-parentheses
warning in upstream middleware stayed visible. No full upstream suite execution
or unselected gzip/hootbin test configuration is claimed.

### Integrated local execution

All commands below executed during Step H. Dependency acquisition for the
isolated vendor lane and actionlint installation was distinct from offline test
execution. Cargo commands used `--locked`; Make checks ran with
`CARGO_NET_OFFLINE=true`.

| Check | Actual result |
| --- | --- |
| `cargo test -p net --no-fail-fast --locked --offline` | 230 passed, zero failed/ignored. Original 77 plus A 64, B 22, C 18, D 27, E 18, F 4. |
| `cargo test -p browser -p runtime_net -p bus -p runtime_css --features browser/conformance-observation --no-fail-fast --locked --offline` | 394 passed, zero failed/ignored. Actual runtime/bus paths and existing document/CSS/image/frame smoke. |
| `make check-network-dependencies`; checker `--archive` against cached published package | Pass, including frozen 54-file inventory, four omissions, licenses, ten patched files, historical AI3 and cumulative replay, local selection/features. |
| `python3 -m unittest discover -s tools/ci/tests -v` | 70 passed, including all 38 provenance tests. |
| `cargo tree --workspace --all-features --target all -e features --locked --offline` | Pass; ureq 2.12.1 with charset/json/native-certs/tls, rustls 0.23.32 ring/std/tls12, flate2 1.1.2. No ureq gzip or rustls logging. |
| `make ci` | **Failed**, exit 2 at default workspace tests; no aggregate pass claimed. Earlier dependency, addressing/conformance, formatting and three workspace Clippy gates passed. |
| Complete default and HTML5 workspace test lanes, independently with `--no-fail-fast --locked --offline` | Each: 2,929 passed, nine failed, 18 ignored. Only failing target: borrowser-conformance native Chromium. No other target failure; expensive parser linear-streaming guard passed. |
| Clean-master and candidate serial `chromium::tests::native_` comparison | Both: one passed, nine failed, one ignored; same exact wrapper failure set. See below. |
| Every later Make CI target independently | Pass: parser/external/fatal fixtures, DOM/patch goldens, local pages, rawtext, six HTML fuzz-smoke/pipeline lanes, pipeline regressions, WPT, default/HTML5 debug and release builds, release probe, HTML/CSS benchmark compilation and entity generation check. |
| Workspace default, HTML5 and hardening/fuzz Clippy, warnings denied | Passed in `make ci`; no suppression added. |
| Observation-qualified Browser all-target Clippy, warnings denied | Pass; includes the release example. |
| `make qualify-network-release` | Pass; all 36 ordinary workers, intentional SIGABRT and all six supervisor controls. No ordinary worker timeout. |
| `actionlint .github/workflows/ci.yml` | Pass, actionlint 1.7.7 built with Go 1.25.0 in an isolated tool directory. |
| Formatting and `git diff --check HEAD` | Pass. |

The original net inventory is preserved. Two original test names were replaced
under accepted Step E to assert the corrected contract: legacy redirect-follow
and credential-target-authentication characterizations now assert selected
redirect rejection without traversal/authentication. No other original test
disappeared. Final accepted test/source bytes remained unchanged throughout the
broad qualification runs.

The 18 workspace ignores are inherited: private subprocess entry points,
pinned-Chromium/native-platform opt-in scenarios, isolated packaging rebuilds,
and explicit billable AWS acceptance. No new ignore/filter was added for AI5.
The independent full lanes did not apply the workflow's existing Linux-only
duplicate-native-wrapper skip. Local macOS execution cannot qualify Linux procfs
or the pinned Linux real-Chromium scenarios.

### Baseline failures remain failures

Clean unchanged master was extracted with `git archive` into a temporary
directory, without creating a branch or modifying the repository index. Both
it and the candidate ran:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance \
  --locked --offline chromium::tests::native_ -- --test-threads=1
```

Both failed these exact nine wrappers:

```text
native_capture_failure_diagnostics
native_cleanup_diagnostic_failures
native_lifecycle_regressions
native_macos_bsd_ownership_states
native_process_and_pipe_contract
native_resource_completion_and_shutdown
native_shutdown_uses_capture_deadline
native_startup_exit_event_classification
native_temporary_artifact_containment
```

Observed causes include unsupported live-process executable inspection during
ownership scans and consequent cleanup identity/timeouts. This is independently
reproduced baseline evidence, not an AI5 regression, an automatic waiver, or a
green CI result. Native reference/process qualification remains with the existing
AG conformance implementation and hosted native lanes. No AI5 source correction
or separate implementation-phase issue is justified by this result.

### Publication and issue-close conditions

The final local result is **PASS WITH CONDITIONS**: the vendor-test gap is
resolved, affected lanes and all remaining independent gates pass, and no new
AI5 correctness defect was found. The full local CI aggregate is still red at
the independently reproduced native baseline failures. Final diff review and
an explicit publication decision remain required; nothing was committed,
staged by Step H, pushed or published.

After publication is separately authorized, record the exact approved commit
and tree, then reconcile the PR head and synthetic integration commit tested
by GitHub Actions. Required qualification includes the enabled **Rust** Linux
job (all existing gates plus six-minute AI5 release-abort step), **AG2 Chromium
(Linux x86-64)**, **AG4 credential-free synthesis and snapshots**, the six HTML5
fuzz-smoke jobs, and the three CSS regression/syntax/pipeline jobs. The special
AG4 proxy timing job is push-only for its dedicated qualification branch, not
this ordinary PR. This inventory describes configured qualification jobs;
repository branch-protection requirements were not inferred or changed.

The workflow's 30-minute Rust job, dependency-preparation ordering and six-minute
AI5 step were statically checked; actionlint and local equivalents passed except
the recorded native aggregate failures. Only actual hosted execution can prove
Linux runner behavior, cache-miss timing and successful completion within those
budgets. No hosted Linux or Windows/macOS result exists for this unpublished
candidate. Full upstream vendor-suite execution is also not claimed.

Issue #1125 is **not currently closeable**. Closure requires final review of the
precisely identified candidate, authorized publication, reconciled hosted checks
and any failures, merge evidence, and an explicit issue-owner closeout decision
against the exit criteria. The original GitHub title/body still describe backend
foundation; this reviewed contract captures the approved hardened scope without
silently modifying the issue. AI6–AI9, AJ/AK, AT and later security/platform work
retain the ownership listed above. No independently unowned new deliverable was
identified and no new issue is needed.
