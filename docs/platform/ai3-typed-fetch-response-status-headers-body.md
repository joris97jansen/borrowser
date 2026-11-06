# AI3: Typed Response, HTTP Status, Headers, And Body Delivery

Issue: [#1123](https://github.com/joris97jansen/borrowser/issues/1123).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation base: master `809a9b1ec115192e9b94411271397b8f54426e5f`.

## Ownership and production boundary

`net` owns the evolved `NetworkResponseInfo`, `HttpStatus`, `HttpHeaders`,
`BodyProgress`, response admission, per-field declared MIME parsing, and coarse
`NetworkErrorKind`. The former response/error definitions are removed from
`core_types`; no reverse dependency, compatibility re-export, new crate, or
parallel response model is introduced. `ResourceKind` remains the existing
execution/limit classification, not a new Fetch destination.

Browser, bus and network runtime already depend on `net`. The conformance driver
now has a direct internal dependency on `net` and retains actual issued requests
when constructing its supplied responses. No external package or serialization
dependency is added. No fixture category or observation schema changes.

The later response-header crash correction pins the existing `ureq 2.12.1`
package to a narrowly patched local source. Its provenance, two-file diff and
unchanged transitive dependency graph are documented below.

The [AI1 architecture contract](ai1-fetch-http-networking-response-contract.md)
and [AI2 issuance contract](ai2-typed-fetch-request-construction.md) govern this
change. [AH6](ah6-resource-addresses-and-identity-keys.md) exact issuance and
immutable response admission, [AH8](ah8-css-resource-reference-discovery.md)
stylesheet bases, and [AH9](ah9-addressing-conformance-fixtures-and-snapshots.md)
independent supplied-response observations remain in force.

Production delivery is still incremental:

```text
FetchRequest -> net HTTP/file execution -> NetEvent -> runtime_net -> CoreEvent
             -> Browser admission -> ordered byte chunks -> Done | Error
```

A response is acquired metadata, not a complete body or a consumer success.
Private immutable facts are shared through an `Arc`: cloning Start/Done/failure
snapshots cannot mutate them and does not copy the bounded header payload again.
Equality compares their content, including exact reported provenance; pointer
identity is never a semantic identifier. This keeps the existing platform event
and image-state envelopes compact without changing their ownership.

## HTTP status and failures

`HttpStatus` validates `100..=599`; unknown valid codes retain their exact value
and numerical class. There is no enum of known codes, reason-phrase table, or
fabricated zero/500 status. Numerical 3xx classification differs from the
redirect set 301/302/303/307/308. In particular 304 is not a redirect to follow.

A selected final-response constructor rejects informational statuses. The current
backend adapter fails explicitly if it returns 1xx instead of a selected final
response; consuming interim heads remains AI5. HTTP 404/500 are ordinary response
facts on every route. The image-specific `HttpStatus` network failure category
and transport conversion are removed.

RFC 9110 recommends 5xx-like recipient handling for invalid status values. AI3's
explicit malformed-response failure is a stricter supported-subset decision,
not that recommended recovery and not a claim of complete recipient conformance.
No valid HTTP status is changed to represent a transport or consumer failure.
Only concrete malformed/unsupported response-admission categories are added;
AI7 retains detailed staged failure and timeout/cancellation policy.

Local-file construction requires a file request origin URL and carries no HTTP
status or fabricated HTTP fields. Its existing extension-derived content type
remains a local-loader fact.

## Original, issued and selected-response URLs

The immutable facts retain:

- The complete original `BrowserUrl` from the request.
- The exact issued correlation string. AI2's HTML/image complete spelling and
  CSS fragmentless issued spelling remain unchanged.
- `reported_response_url: Option<String>`: the exact selected-response spelling.
- A cached `Option<Result<UrlParseOutcome, UrlFailure>>`, derived once through AH
  from that optional string during construction.

`None` means absent provenance. `Some("")` is an explicit empty report and parses
as invalid without a base. Invalid nonempty reports retain their typed AH failure;
valid reports retain their URL and recoverable diagnostics. Neither raw nor parsed
provenance can be set independently after construction. Equality retains raw
spelling even where AH normalization produces equal URLs.

Metadata construction preserves acquired facts. Actual HTTP backend admission
requires a present, parsed HTTP(S) selected URL before Start. Rejection retains
available metadata as failure context without publishing chunks or Done.

HTML consumes the retained AH result and rejects missing/invalid provenance
instead of silently substituting the requested URL. This removes the former
empty-final-URL compatibility fallback; it does not restore an already retired
active document. Images also require usable provenance for their actual source.

AH8's supplied CSS metadata can still explicitly retain unavailable or invalid
response bases without failing otherwise accepted CSS. Relative references stay
unresolved; request/document URLs are not guessed as bases. Existing AH8/AH9 empty
sentinel inputs are translated to absence at their documented fixture ingestion
boundary, not by a general URL-normalization rule.

The AH9 `document-default` missing-provenance row now asserts production
cancellation and terminal rejection with no committed addressing snapshots.
Its former request-URL fallback expectation is intentionally replaced; the two
valid-provenance snapshots and CSS missing/invalid-base snapshots are preserved.

The current response URL is the final selected response URL. No second copy,
current-hop URL, guessed redirect chain, or URL-inequality redirect history is
introduced. AI6 owns observable attempts and redirect-chain orchestration.

## Ordered headers and backend fidelity

`HttpHeaders` remains one bounded insertion-ordered list. Names are canonical
lowercase ASCII tokens, matching is ASCII case-insensitive, values retain exact
admitted bytes, and repeated fields retain occurrence order. No sorting,
universal comma joining, map abstraction, or lossy UTF-8 conversion is introduced.
Equality supports immutable snapshot comparison; `values(name)` retains same-name
order without replacing the underlying list.

Existing limits remain: 64 fields, 256 name bytes, 8192 value bytes, and 65536
aggregate bytes including `name: value\r\n` overhead. Append validates before
copying and leaves the list unchanged on failure.

The locked `ureq 2.12.1` adapter reconstructs the backend-exposed occurrence order
using names plus per-name occurrence positions. It verifies that every exposed
occurrence has an accessible value and applies the shared limits. Detectable
missing values, invalid fields and excess limits fail rather than disappearing.

**This is not a wire capture.** The backend trims exposed values, hides some
value bytes, and removes
Content-Encoding/Content-Length during automatic gzip decoding. Public APIs
cannot recover those facts. The admitted list preserves what this adapter can
actually obtain; tests characterize upstream transformations. No fidelity flag
claims otherwise. AI5 owns complete malformed-head/framing/encoding qualification
and any required backend adaptation; this change adds no HTTP parser.

The pinned parser correction rejects colonless fields and invalid field names
before framing or adapter access. Header-construction failures now reject the
whole head instead of being silently discarded. This narrow syntax correction
does not establish full header-value or framing conformance.

## Declared Content-Type and charset

`ContentType` is a read-only projection of one field's MIME essence and optional
charset. The single parser in `net/src/content_type.rs` implements WHATWG MIME
§4.4 scanning and recovery, including byte-isomorphic input, HTTP whitespace,
ASCII token validation, case normalization, quoted strings/escapes, parameter
position, and the first valid duplicate charset parameter. Parameters not needed
by this projection are scanned without creating a speculative parameter registry.
Original fields and bytes remain available independently.

A missing field produces no iterator item. Fatal MIME parsing failure produces a
fixed per-field error. Prescribed recoveries produce parsed metadata rather than
an invented strict-validation failure. Unknown MIME and charset labels remain
valid declaration facts, not default values or encoding decisions.

Tests use independently expected WHATWG/WPT cases; the reviewed WPT MIME fixture
blob is `8692d77ebf4f5d28b1358cf65b1fe89e60f8a77e`. This is a scoped essence/charset
projection, not full WPT execution or MIME sniffing qualification.

AI8 will reuse this parser for aggregate repeated-header interpretation and loader
handoff. The existing first-field access and CSS MIME compatibility policy remain
explicitly legacy; they are not a second long-term parsing authority. No sniffing,
charset-label resolution, BOM/meta scanning, or character decoding is added.

## Body bytes, limits and terminal accounting

Chunks remain owned `Vec<u8>` octets, delivered in order through the existing
32 KiB reader. No universal body buffer, streaming framework, or complete-body
wrapper is introduced. Existing images own their buffer; HTML remains incremental.

`BodyProgress` has a private route and delivered count. The route derives its cap:
HTML 10 MiB, CSS 2 MiB, image 20 MiB. A caller cannot supply an independent limit.
Construction rejects an over-limit count. Runtime forwarding and Browser
completion check the route against the actual command/consumer; HTML and image
completion also check observed delivered-byte accounting.

Done carries the same response metadata and progress. Error carries acquired
metadata and known progress where available, rather than an independently mutable
status code. Before acquisition/counting those facts are absent, not invented.
Read, cancellation and resource-limit failures cannot publish Done. Exact-limit
EOF succeeds; observing another byte fails even when the configured prefix was
already delivered. Complete-resource consumers discard failed partial buffers.

Counts refer to bytes returned by the reader (decoded gzip output when ureq's
decoder is active), not encoded length. Content-Length is neither a completeness
proof nor an allocation authorization. Separate encoded accounting remains
unavailable where the backend does not expose it.

204/205/304 expose null-body semantics: no chunks and zero delivered bytes. Done
here describes absence of a consumer body, not independent validation of wire
framing, trailing bytes, or connection reuse. Body-bearing responses still require
reader completion. A fully transferred 206 is still partial content, not proof of
a complete representation.

## Narrow image integration

The existing image entry records its issuing generation and one optional admitted
`NetworkResponseInfo` while Loading. No separate image networking model, fetch-job
hierarchy or full AK4 load-result model is added. Tab/generation/route, exact URL,
first-start admission, immutable completion, byte count and image route guard the
existing decoder entry point. Unusable provenance, null bodies, unmerged 304 and
unsupported 206 settle locally without decoding. Empty image bytes also fail.

404/500 do not by themselves invalidate an image. Complete supported image bytes
can reach Ready; non-image bytes fail through the existing decoder. Image 206
rejection is the current lack of range reconstruction, not a general invalid-image
rule. Fetch preserves the status and bytes independently of consumer acceptance.

`NetworkErrorKind::Cancelled` controls cancellation of a matching unfinished image
load. Error text never decides cancellation. Existing forgetting behavior remains
for such cancellation; other failures clear buffers and settle Error. Rejected,
decoding and ready entries ignore late network events. Event handling reports
actual presentation state changes so inert events do not request new render work.

Image cache identity, same-generation retry identity, and stranded old-generation
Loading entries are not redesigned. AI7/AK4 retain broader cancellation/lifetime
work. Existing decode formats and resource keys are unchanged.

## Debug and explicit exclusions

Custom Rust Debug on response metadata, content type and network events emits
bounded structural facts, status and necessary byte counts. It withholds URL
contents, field values/arbitrary names, body bytes, server reason text, backend
error strings, private IDs, pointers and timings, including nested, pretty and
failed-send formatting. Existing non-network event variants also avoid exposing
CSS payloads through the custom CoreEvent formatter. These are internal regression
contracts, not public wire formats. No Serde or diagnostics framework is added.
Legacy backend logs and UI error strings are not claimed globally sanitized.

AI3 deliberately preserves the eager reset in `Tab::start_fetch`, history timing,
and existing generation routing. It **does not implement 204/205 active-document
preservation**. AJ2 #1133 / AJ3 #1134 must test actual navigation from a populated
page and preserve document/origin/history/input/styles/rendering. An isolated
response-handler test cannot establish that guarantee.

HTML 304/206 preparation remains AJ3; CSS HTTP-status and partial-response policy
remains AJ7 #1138. In particular current CSS can still accept a 404/500 text/css
body, and existing HTML/CSS may consume partial content inappropriately. No AI3
test labels those behaviors conformant. AT5 #1245 owns 304 merge/revalidation;
AT6 #1246 owns cache-aware loader integration. The image lookup table is not an
HTTP cache. AK4 #1145 owns broader image load results.

AI4 #1124 retains the backend/fake seam; AI5 #1125 real backend qualification;
AI6 #1126 redirects; AI7 #1127 staged failures; AI8 #1128 aggregate metadata;
AI9 #1129 networking conformance integration. No JavaScript networking APIs,
HTTP cache, cookies, security-policy enforcement, navigation rewrite or new
compression/decoding behavior is claimed.

Extension points are documented responsibilities, not placeholder fields:
filtered responses and security filtering need implemented policy; MIME sniffing
needs its own concrete algorithm; streaming evolution must preserve ordering and
terminal facts; compression needs encoded/decoded provenance; trailers need
backend support and a lifecycle distinct from immutable initial headers.

## Requirement and exit traceability

| AI3 requirement / exit | Implementation and regression evidence |
| --- | --- |
| Typed response/status/header/body | `net::NetworkResponseInfo`, validated `HttpStatus`, existing `HttpHeaders`, chunks and route-bound `BodyProgress` in production events. |
| Response URL and final URL distinct from original | Exact optional report plus cached AH result, complete original and exact issued spelling; absent/empty/invalid/valid and equality tests. |
| Header ordering, repetitions, casing | Shared list/equality/values; model and exposed-backend interleaving/byte/boundary tests. |
| Body bytes and explicit guardrails | Empty/binary delivery, every-route exact/over-limit tests, partial interruption and cancellation, actual loopback read failure. |
| Truncation/rejection metadata | Error category plus retained immutable facts and known delivered count/cap; no successful truncation. |
| Content type and charset without sniffing | Per-field declared projection, independent MIME recovery/charset/unknown-label/byte cases; original values retained. |
| Normal, redirect, HTTP error, network failure | Model tests for 200/404/500/3xx/unknown codes; 404/500 streams on all routes; separate terminal failures. |
| Safe deterministic metadata/debug | No timing/reason fields; literal, ordinary, pretty, nested and failed-send assertions. |
| All named acceptance cases | `net/tests/fetch_response.rs`, header tests, reader/adapter/status tests, Browser event tests. |
| Production integration | Runtime local-file handoff, real loopback 404/500 image decode-to-Ready, immutable HTML/CSS completion and AH6/AH8/AH9 regressions. |
| Closeable before new backend execution | Pure constructors/in-memory reader tests establish the model; current-backend characterization is separate from AI5 qualification. |
| Future extensions | Responsibilities above, without inactive modes or speculative framework. |

Implementation, local validation, hosted qualification and GitHub closure remain
separate. This document does not claim publication, issue closure, or milestone
completion.

## Local validation checkpoint (2026-11-06)

This subsection records the original candidate before the later pinned-backend
header correction. Its historical results remain separate from correction
qualification below.

The branch is based on synchronized master
`809a9b1ec115192e9b94411271397b8f54426e5f`. Validation ran with Rust/Cargo
1.92.0 on Darwin arm64, macOS 27.0.1. Socket-dependent tests ran outside the
execution sandbox; no public-internet resource tests are required.

- The final affected-crate command, `cargo test -p addressing -p core_types
  -p net -p bus -p runtime_net -p browser --locked --offline`, passed **478 tests**
  with no failures or ignores. This includes 370 Browser unit tests, all current
  AH addressing suites and the request/header/file/network regression suites.
- Direct AI3 runs passed seven response-model tests, fourteen net reader/adapter/
  status tests and seven Browser production-event tests. The real image runtime
  test also verifies that a truncated HTTP 500 body preserves metadata/counts and
  never enters successful decoding.
- The final `make ci` passed dependency/source graph checks, CI conformance
  (7 PASS), addressing conformance (22 PASS, 2 explicitly unsupported), formatting
  and all three lint lanes, then **failed** in the existing native macOS Chromium
  ownership tests. No full-CI pass is claimed.
- Independently executed remaining CI targets passed: parser conformance,
  external fixtures, fatal failures, DOM/patch goldens, real-page/rawtext
  regressions, all six fuzz smoke runners, pipeline regressions, WPT tree builder,
  default/HTML5 debug and release builds, both benchmark compilations and entity
  generation checks. These individual passes do not turn the failed CI aggregate
  into a pass.

Supplemental default and HTML5 workspace commands used Cargo's `--no-fail-fast`
with the same `--lib --bins --tests --examples --locked --offline` selection.
Every other target completed successfully; both commands returned failure solely
for the native conformance binary. Those runs preceded the final null-body
cancellation regression, which the final affected-crate run and all three lint
lanes requalified. The deterministic production local-file document/CSS/image/
frame smoke also passed with `conformance-observation` enabled.

Native failures are the existing process-ownership qualification limitation,
outside AI3. Base and candidate both reproduce **1 passed, 9 failed, 1 ignored**
with the same serial native-test command:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline \
  chromium::tests::native_ -- --test-threads=1
```

The final comparison ran sequentially, base then candidate, after the other
qualification runners finished. The base is an unmodified `git archive` with
isolated build artifacts. All twelve
native Chromium source files and all 415 locked package coordinates/checksums
match the candidate. Failure diagnostics include unavailable live-process
executable identity, cleanup identity/timeouts and expected launch-error
classification. No native test assertion, ignore or CI gate was changed.

Release builds retain twelve HTML, two CSS and four parser-runtime warnings,
reproduced by a release check of those unchanged crates on the base. Fuzz runners
retain host warnings about missing sanitizer callbacks, no coverage-interesting
inputs and absent `timeout`/`gtimeout`; the 128-run smoke exits are evidence of
runner execution, not sanitizer or coverage qualification. No fresh supported
Linux ownership qualification or hosted CI result is claimed.

## Pinned response-header crash correction (#1123)

The original 2.12.1 `HeaderLine::into_header` could admit a token-only line
without a colon. `name()` could then expose that token while `value()` and
`value_raw()` indexed one byte beyond its end. Arbitrary field enumeration
exposed this in AI3; `Content-Length`, `Transfer-Encoding`, `Content-Encoding`
and `Connection` could already trigger it inside response construction, before
Borrowser could inspect a `Response`.

Isolated local reproducers confirmed both paths against the unchanged pinned
library. All five malformed examples exited 101 with unwinding and SIGABRT with
`panic=abort`; `X-Empty:` completed normally. Core dumps were disabled and only
child processes were allowed to abort. No release abort ran in the main test
runner. The existing release profile is unchanged.

The public response API cannot intervene before backend framing/compression
access, and raw header storage is private. The chosen boundary is a local Cargo
patch of the checksum-verified published 2.12.1 package, with two source changes:

1. Locate a colon and validate a nonempty token name before constructing a
   Header. The delimiter is strictly inside the byte line; an empty value is
   valid. Construction diagnostics contain no supplied field contents.
2. Propagate construction failure from `Response::do_from_stream` before any
   internal accessor runs, rather than silently dropping invalid fields.

The exact dependency diff is retained in
[`vendor/ureq/response-header.patch`](../../vendor/ureq/response-header.patch).
The package archive SHA-256 is
`02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d`, with published
VCS revision `ba4555d54fd72e16fc737f7e8fcc812efb131260`. Licenses, original file
hashes, four packaging omissions and the two modified hashes are retained in
[`BORROWSER-PATCH.md`](../../vendor/ureq/BORROWSER-PATCH.md) and `UPSTREAM.json`.
Root Cargo pins `=2.12.1`, selects this local patch, and excludes it from workspace
membership. Only ureq's registry source/checksum entry changes; all package versions, other
checksums, features, transitive edges and the frozen AH URL graph remain unchanged.

A dependency upgrade or remote fork would require additional compatibility and
source qualification without helping this two-function fix. No safe public API
can repair an already unsafe internal parse. No `catch_unwind`, raw-socket
pre-parser, middleware or competing HTTP parser is used.

Fetch maps existing typed `ureq::ErrorKind::BadHeader` directly to
`NetworkErrorKind::MalformedResponse`. The terminal diagnostic is the fixed
`malformed HTTP response headers`; raw backend error strings are neither logged
nor copied into this event. Correlation remains exact, response/body context is
absent before admission, and no fake HTTP status is introduced. A rejected head
emits one Error and no Start/Chunk/Done. Existing consumer failure paths settle
HTML, CSS and images; cancellation and valid HTTP-error response behavior remain
unchanged.

Permanent direct-backend tests cover colonless arbitrary/internal fields,
invalid names, empty values, repeated/interleaved order and 200/404/500 bodies.
The real loopback Browser test exercises all three issued routes, typed errors,
event-channel termination, redaction and resource settlement. The previous
malformed-name omission characterization now asserts rejection; whitespace
normalization characterization remains independent.

`make check-network-dependencies` checks the vendored source inventory/hashes,
recorded patch and actual `--locked --offline` Cargo selection. Five integrity
tests exercise changed/unchanged source tampering, unexpected files and attempts
to certify modified source by rewriting its patch record.
Isolated optimized `panic=abort` probes against the corrected library reject all
five malformed examples with typed BadHeader and accept `X-Empty:` with exit 0.

AI5 #1125 retains complete response/framing/value/encoding qualification, interim
handling, compression and wire fidelity. This correction establishes only safe
delimiter construction, deterministic propagation and the described production
failure handoff. The vendored middleware source is unchanged and exposes an
existing `unused_parens` compiler warning; it is not suppressed or repaired here.

### Correction validation checkpoint (2026-11-06)

- `cargo test -p net --test backend_response_headers --locked --offline` passed
  both permanent direct-parser tests (ten malformed-field cases and valid
  empty/repeated/interleaved fields with three response statuses).
- The permanent production-loopback regression passed all eighteen combinations
  of malformed field and HTML/CSS/image route. The complete affected-crate run
  (`net`, `bus`, `runtime_net`, `browser`, `addressing`) passed **481 tests**, with
  no failures or ignores, including existing cancellation, 404/500 image decode,
  partial-body, header-order and AH regression behavior.
- The deterministic document/CSS/image/frame runtime smoke passed. Formatting,
  all three workspace lint lanes, URL graph checks, the new network dependency
  gate and five integrity tests passed. The dependency's existing parentheses
  warning remains visible; no allowance or unrelated source fix was introduced.
- Full `make ci` again stopped at the nine native macOS ownership failures.
  Both workspace feature lanes completed with `--no-fail-fast` and failed only
  the native conformance binary. All remaining independent CI targets passed,
  including parser/golden/WPT/fuzz runners, default/HTML5 debug/release builds,
  benchmark compilation and entity generation. The earlier release/fuzz warnings
  remain qualified rather than being treated as sanitizer or coverage evidence.
- Optimized isolated probes also linked the actual corrected release-built
  backend with `panic=abort` and fat LTO. All five malformed field examples
  rejected with BadHeader and exit 0; the empty-value example completed with
  exit 0. This verifies the identified release crash path, not every potential
  backend panic or all HTTP framing behavior.

After the other qualification runners finished, the exact native test command
was repeated sequentially on the unchanged archived base and corrected candidate.
Both returned **1 passed, 9 failed, 1 ignored**, with the same nine failures as the
established baseline. All twelve native Chromium Rust source files remain
byte-identical. This is reproduced baseline failure, not native qualification.

These results replace the earlier untested accessor-panic risk with a corrected,
reproducible parser boundary. They do not constitute a green CI aggregate or
fresh supported Linux/hosted ownership qualification. No issue closure or
publication is claimed.

## Hosted network dependency gate correction (#1123)

The existing `rust` job in `.github/workflows/ci.yml` now runs **Verify pinned
network dependency source and locked selection** with
`make check-network-dependencies`, immediately after
`make check-url-dependencies` and before formatting/lint/build/test gates. The
step has no additional condition or failure allowance. The earlier workflow
discovered the five source-integrity tests through the URL target, but did not
invoke the complete network checker's locked Cargo-resolution validation.

The preceding URL check calls online `cargo metadata --locked` for the main and
fuzz workspaces. Its successful main-workspace resolution prepares the same
dependency graph that the network checker then inspects with
`cargo metadata --locked --offline`. This order supplies dependency availability
on a cache miss; the cache action is only an optimization. Both existing URL
checks and all other workflow gates remain unchanged. The checker, dependency
versions/features, lockfiles and two-file backend patch are unchanged.

Local validation (2026-11-06):

- `make check-url-dependencies check-network-dependencies` passed, including all
  37 CI-tool tests. A separate `test_ureq_patch.py` run passed all five tests.
- With a newly created, empty `CARGO_HOME`, the network checker failed before
  preparation as expected. Running the exact hosted sequence, URL target then
  network target, passed both targets and all 37 discovered tests. The temporary
  Cargo home acquired 401 crate archives; offline metadata then selected local
  `ureq 2.12.1` with no registry source. Both workspace lockfiles stayed identical.
- Pinned actionlint v1.7.7 passed the complete workflow. A YAML structural
  comparison verified that only the new step was added, immediately after URL
  preparation, with every existing job and gate preserved. `make fmt-check`
  passed. No Rust production code or dependency source changed in this correction.
- This verifies cold-cache preparation on local macOS, not execution on a hosted
  Linux runner. Full CI was not rerun for this workflow/documentation-only change;
  the earlier failed aggregate and nine reproduced native baseline failures
  remain qualified above. Required hosted CI has not run for this candidate.

## Standards and independent evidence

- [RFC 9110 status codes](https://www.rfc-editor.org/rfc/rfc9110.html#section-15)
  and [HTTP fields](https://www.rfc-editor.org/rfc/rfc9110.html#section-5).
- [RFC 9112 body framing](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3)
  and [incomplete messages](https://www.rfc-editor.org/rfc/rfc9112.html#section-8).
- [Fetch statuses](https://fetch.spec.whatwg.org/#statuses).
- [WHATWG MIME parsing](https://mimesniff.spec.whatwg.org/#parsing-a-mime-type)
  and [WPT MIME cases](https://github.com/web-platform-tests/wpt/blob/master/mimesniff/mime-types/resources/mime-types.json).
- [HTML image status independence](https://html.spec.whatwg.org/multipage/images.html#updating-the-image-data),
  [linked-resource processing](https://html.spec.whatwg.org/multipage/semantics.html#fetching-and-processing-a-resource-from-a-link-element),
  and [navigation](https://html.spec.whatwg.org/multipage/browsing-the-web.html#navigating-across-documents).
