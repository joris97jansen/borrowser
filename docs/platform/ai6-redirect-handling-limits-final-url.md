# AI6: Redirect Handling, Limits, And Final Response Provenance

Issue: [#1126](https://github.com/joris97jansen/borrowser/issues/1126).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation base: `20b7a31042e33f488db66e76a1f167fec674b3a7`.
Date: 2026-11-20. Local uncommitted implementation; validation is recorded below.

## Authority and ownership

[AI1](ai1-fetch-http-networking-response-contract.md) controls ownership,
lifecycle, redirects and diagnostics. [AI2](ai2-typed-fetch-request-construction.md),
[AI3](ai3-typed-fetch-response-status-headers-body.md),
[AI4](ai4-network-backend-and-deterministic-fake-network.md) and
[AI5](ai5-real-http-response-acquisition-and-bounded-body-integrity.md) retain
request admission, response/body facts, shared fake acquisition and qualified
HTTP acquisition. [AH resolution](ah2-typed-url-parsing-and-diagnostics.md) and
[identity](ah6-resource-addresses-and-identity-keys.md) remain Addressing-owned.

Fetch coordinates individual attempts. The backend does not follow redirects;
the agent remains configured with `redirects(0)`. Runtime/bus carry existing
correlation and events. Browser admits and installs selected document/CSS/image
results; CSS, Layout and Paint acquire no redirect or transport policy.

GitHub closure is separate evidence: #1126 was closed by imported rust-url commit
`00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a`, a file-URL change containing an unrelated
`Fixes #1126`. This local implementation does not reopen, close, publish or merge
the issue. Code readiness, review, hosted qualification and completion differ.

## Supported decisions

| Response/target | Fetch behavior |
| --- | --- |
| 301/302/303/307/308 with one Location | Resolve, admit and follow, preserving GET and its absent body. |
| Missing Location | Select the acquired response, including its ordinary body behavior. |
| Empty Location | Resolve normally; it can cause another acquisition of the same retrieval address. |
| Repeated Location occurrences | Redirect(RepeatedLocation), even if identical; no next acquisition. |
| Other 3xx, including 300/304/305/306/399 | Select, without following Location. Existing 304 body rules remain. |
| Fatal AH resolution failure | Redirect(InvalidLocation); recoverable diagnostics remain successful. |
| Non-HTTP(S) target | Redirect(UnsupportedScheme), without dispatching file/other loaders. |
| Normalized username or password present | Redirect(UrlCredentialsUnsupported), before acquisition/authentication. |
| Another follow after twenty entered redirects | Redirect(LimitExceeded), before acquisition 22. |

HEAD, other methods, manual/error modes and authentication remain unsupported.
The initial acquisition plus twenty followed acquisitions can select a response
successfully. Cycles use the same budget; repeated URL identity alone is not a
loop verdict. A stateful endpoint may legitimately redirect to itself then finish.

Resolve against the acquired current response URL, never the document base or
original request. If the resolved target fragment is absent, inherit the current
request fragment through AH fragment-only resolution. `#` remains present-empty;
an explicit replacement remains that replacement. No Addressing mutation API,
string URL joining or alternate parser is introduced.

## Header-byte adaptation and its qualification

The [Fetch location-URL algorithm](https://fetch.spec.whatwg.org/#concept-response-location-url)
describes resolution and fragment inheritance. The byte-to-string boundary remains
unsettled in [Fetch #883](https://github.com/whatwg/fetch/issues/883) and
[#814](https://github.com/whatwg/fetch/issues/814). The corresponding
[WPT](https://github.com/web-platform-tests/wpt/blob/master/fetch/api/redirect/redirect-location-escape.tentative.any.js)
is tentative. Neither strict UTF-8 rejection nor isomorphic decoding is claimed
as an unambiguous normative rule. WPT's isomorphic encoding supplies test bytes;
it does not direct this implementation to reinterpret those bytes as Unicode.

The private Fetch-to-Addressing helper copies ASCII bytes unchanged and escapes
each non-ASCII byte as uppercase `%HH` using its original value. It performs no
Unicode decoding, replacement, percent-decoding, or comma splitting. Existing
percent escapes retain their spelling. Original admitted header bytes remain
unchanged in metadata. Existing wire field validation/OWS processing and typed
8 KiB value bounds remain; the adapted reference is bounded to at most 24 KiB.

Independently authored literal expectations include:

| Location bytes | Reference passed to AH |
| --- | --- |
| `../next?\xFF` | `../next?%FF` |
| `../next?\xE2\x98\x83` | `../next?%E2%98%83` |
| `../next?%ff%E2%98%83%e2%98%83` | unchanged |
| `../next?\xFF%ff` | `../next?%FF%ff` |
| `../next?%\xFF` | `../next?%%FF` |
| `../next?%\xE2\x98\x83` | `../next?%%E2%98%83` |
| `../p,a?x=%2f&y=a,b#` | unchanged, one URL reference |

ASCII syntax and its subsequent normalization remain AH-owned. A stray percent
can produce a recoverable diagnostic; it is not an automatic failure. Invalid
header construction/wire syntax fails before this helper. A valid field whose
URL has an invalid port still fails AH parsing.

## Targeting, correlation and immutable facts

`FetchRequest::for_redirect` derives a checked next request, preserving the
original ResourceAddress, purpose, request ID, initiator, requesting origin and
admitted ordinary headers. `current_url` is the complete current target.
`transport_target` projects it through AH without its fragment. Fake registration
and lookup use that projection; the real builder executes the current complete
URL, with fragments excluded by the pinned wire serializer.

`legacy_issued_url` remains original correlation: HTML/image complete original
spelling, CSS original fragmentless spelling. It never selects the next target.
The real response retains the complete current reported URL, including CSS
fragment provenance. Intermediate URL inequality cannot fabricate a history.

Each hop contains source request URL, acquired response URL, status and admitted
target URL. At most twenty hops are retained in the existing immutable response
facts. History attachment is copy-on-write: a separately retained acquired
snapshot cannot change. Start and terminal events share the selected snapshot,
including exact original/reported provenance, headers and history.

Ordinary admitted Accept/Accept-Language survive authority/scheme changes. Fresh
backend requests regenerate Host, request target and execution-derived fields.
Authorization/Cookie/Proxy-Authorization/Referer remain outside initial admission;
userinfo cannot generate Basic authentication. Set-Cookie remains inert.
HTTP-to-HTTPS and HTTPS-to-HTTP do not implement mixed-content/HSTS/CORS policy.

## Pending transitions, cancellation and failure context

Before each acquisition, Fetch checks its existing cooperative token. A target
resolved/admitted from a redirect is only pending. After intermediate-reader
release, the next pre-acquisition checkpoint can cancel it without adding a hop.
Entry into `backend.acquire` commits the transition, not DNS success, connection
establishment, transmission or server receipt. After the call returns, Fetch
records that entered transition before processing success or failure. No new
history is published while acquisition is blocked.

Valid redirect responses with Location also have a checkpoint before Location
interpretation. A checkpoint that observes cancellation settles Cancelled there.
Otherwise a concrete acquisition/admission failure settles without retrospective
conversion to cancellation. Changes after the final checkpoint may permit an
acquisition to begin. Final body checkpoints retain AI5 behavior, including Start
before the pre-decoder cancellation check. Defaults remain 15 s connect and 30 s read/write per backend operation. No
instantaneous abort, total Fetch deadline, bounded DNS cancellation or wall-clock
race ordering is promised.

| Outcome | Retained response/history | Body progress |
| --- | --- | --- |
| Cancellation before initial acquisition | None; empty history | None |
| Invalid/repeated/forbidden target or budget exhaustion | Current redirect facts; no rejected hop | Route-correct zero |
| Cancellation after target admission, before next entry | Source facts; pending hop absent | Route-correct zero |
| Entered next acquisition fails without new metadata | Latest acquired source facts; entered hop included | Route-correct zero |
| Newly acquired response fails URL/provenance admission | New acquired facts and entered history | None, preserving AI5's admission representation |
| Selected coding admission fails | Selected facts and history | Route-correct zero |
| Selected body fails/cancels | Exact Start snapshot/history | Actual delivered decoded bytes |
| Selected body completes | Exact Start snapshot/history | Actual delivered decoded bytes |

When an adapter cannot construct new metadata, preserve previous context rather
than inventing a response for the failed destination. One terminal Done/Error
settles the operation. No intermediate Start/Chunk/Done is consumer-visible.
HTTP 404/500 remain response statuses; their selected bodies can complete or fail.

## Disposal and shared-pool evidence

Fetch drops intermediate readers before the next attempt. It performs no further
body-reader consumption, content decoding or consumer delivery. Header acquisition
may already have buffered payload. No arbitrary drain is used to recover reuse;
unread nonempty readers retain unfinished framing and drop their connections.
Zero-body head-completed responses retain existing independently checked pooling.
Discarding intermediate payload is not a claim of its verified body completion.
Acquired head/framing validation still precedes Fetch decisions.

Production `agent_for_policy` shares the default agent but builds a fresh agent
for each custom-policy acquisition. Therefore custom-root facade HTTPS tests
qualify functionality, not same-pool exclusion. This ownership is unchanged.

AI6 disposal qualification reuses the retained AgentBackend/capacity_agent seam,
delegating to the production current-target builder and metadata extraction and
preserving ureq Status responses. A single agent/pool runs complete-response
reuse controls, an unread redirect, the selected response, and a subsequent reuse
control. Socket/request IDs prove fixed, chunked and close-delimited behavior on
HTTP and HTTPS. Body-reader probes separately prove zero reads and release order.
The existing real-rustls controlled TLS seam adds actual session/request identity,
buffered plaintext isolation and positive TLS 1.2/1.3 controls through Fetch.
Synthetic transport fixtures are not relabeled as HTTPS evidence.

## Tests, diagnostics and exclusions

Production fake routes exercise actual chains, exact limits/cycles, byte cases,
response-relative resolution, fragments, rejected targets, checkpoints and
failure context. Actual loopback HTTP/HTTPS acquisition exercises raw field bytes,
headers, scheme changes and selected HTTP errors. Literal expectations are not
generated by the helper under test. Observers retain original snapshot clones.

Runtime/bus tests preserve tab/generation/slot/route and original issued strings.
Browser runtime smoke asserts final document/fallback bases, the final external
stylesheet base and resolved CSS reference, original image keys, and a completed
frame. Rejection tests settle HTML/CSS/image consumers while preserving siblings.

Debug/log output uses bounded ordered structural fields, status, count and fixed
decisions/categories, without URL/header/body contents or arbitrary errors.
Internal Debug is a regression surface, not a public serialization schema.

AI7 retains broader failure taxonomy/cancellation/timeouts and delivery policy.
AI8 owns aggregate metadata; AI9 owns AG networking integration. Cache, cookies,
authentication, security policy, navigation redesign, JS networking, HEAD, retry
frameworks, schedulers, new backend hierarchies and public serializers are excluded.
No production agent/pool redesign or vendor/dependency change is introduced.

## Validation record

Current local execution (2026-11-20, Rust/Cargo 1.92.0, macOS arm64):

| Level / command | Current result |
| --- | --- |
| `cargo test -p net --lib ai6_ --locked --offline` | 18 passed; zero failures/ignores. |
| `cargo test -p net --test fake_network --locked --offline` | 22 passed. |
| `cargo test -p net --test fetch_response --locked --offline` | 7 passed. |
| `cargo test -p runtime_net ai6_ --locked --offline` | 1 passed. |
| `cargo test -p browser --features conformance-observation ai6_ --locked --offline` | 2 passed. |
| `cargo test -p net -p runtime_net -p bus -p addressing -p browser --locked --offline` | 680 passed; zero failures/ignores. |
| `cargo test -p browser --features conformance-observation --locked --offline` | 391 passed; zero failures/ignores. |
| Explicit redirected document/CSS/image smoke | 1 passed with final bases, original image key and observed frame assertions. |
| Existing explicit AI2 local-file runtime smoke | 1 passed. |
| `make fmt-check`, `make lint`, observation-qualified Browser Clippy | Passed. Initial test-module ordering lint was corrected and rerun. |
| `make check-network-dependencies` | Passed; committed vendor history/source, locked selection and features verified. |
| `make qualify-network-release` | Passed: 36 production workers and intentional abort/supervisor controls. |
| `make ci` | Failed at nine native macOS Chromium ownership wrappers. The same serial selection reproduces all nine on untouched base and candidate; all remaining independent gates completed as recorded below. |

Socket fixtures required execution with local socket access beyond the restricted
execution environment. The initial restricted run failed to bind loopback; the unrestricted
runs above executed the actual tests. Existing vendored middleware warnings remain
visible, with no source normalization or warning suppression.

The counts describe separate executed lanes, not a deduplicated aggregate.
No hosted CI, commit, merge or issue-completion claim.

### Current baseline comparison

An untouched `git archive` of the exact base and the AI6 candidate each ran:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline \
  chromium::tests::native_ -- --test-threads=1
```

Both returned exit 101: **1 passed, 9 failed, 1 inherited ignored**. Matching failures:

- `native_capture_failure_diagnostics`
- `native_cleanup_diagnostic_failures`
- `native_lifecycle_regressions`
- `native_macos_bsd_ownership_states`
- `native_process_and_pipe_contract`
- `native_resource_completion_and_shutdown`
- `native_shutdown_uses_capture_deadline`
- `native_startup_exit_event_classification`
- `native_temporary_artifact_containment`

Diagnostics include unavailable executable identity during native ownership scans
and consequent cleanup failures. This demonstrates an inherited qualification
failure, not a waiver or a green CI aggregate. No ignore, assertion weakening,
workflow bypass or conformance implementation change addresses it in AI6.

### Complete independent CI execution

After the failed aggregate, both complete workspace selections ran with
`--no-fail-fast` to execute every target without masking their failing exit:

```sh
cargo test --workspace --lib --bins --tests --examples --locked --offline --no-fail-fast
cargo test --workspace --lib --bins --tests --examples --features html5 --locked --offline --no-fail-fast
```

Each returned exit 101, with **2,973 passed, 9 failed, 18 inherited ignored** across
84 binaries. The only failing target was the conformance binary's same nine native
wrappers. The additional executions do not turn `make ci` or either workspace lane
green. All AI6 targeted/affected/observation tests above passed without ignores.

The actual `make ci` run passed 245 CI-tool tests, dependency/source checks, seven
CI fixtures, 22 addressing fixtures (plus two explicitly unsupported), formatting
and all three workspace Clippy feature lanes before the native failure.

All subsequent independent targets completed successfully:

- `make test-html5-parser-conformance`
- `make test-html5-external-fixtures`
- `make test-html5-parser-fatal-failures`
- `make test-html5-dom-golden`
- `make test-html5-patch-golden`
- `make test-html5-smoke-real-pages`
- `make test-html5-rawtext-script-regressions`
- `make test-html5-tokenizer-fuzz-smoke`
- `make test-html5-tokenizer-script-data-fuzz-smoke`
- `make test-html5-tokenizer-rawtext-fuzz-smoke`
- `make test-html5-tokenizer-rcdata-fuzz-smoke`
- `make test-html5-tree-builder-token-fuzz-smoke`
- `make test-html5-pipeline-regressions`
- `make test-html5-pipeline-fuzz-smoke`
- `make test-wpt-tree-builder`
- `make build`
- `make build-html5`
- `make build-release`
- `make build-release-html5`
- `make compile-html5-benches`
- `make compile-css-benches`
- `make html-entities-check`

The separately executed `make qualify-network-release` also passed as recorded
above; its gate was not dropped. The six fuzz smokes each executed 128 runs.
Their existing coverage/instrumentation warnings and absent timeout/gtimeout
wrapper remain visible; these runs do not establish sanitizer, coverage-guided
or outer wall-timeout qualification. Existing release-only HTML/CSS/runtime-parser
warnings remain unsuppressed. No new ignore or workflow exception was added.

Self-review found no material deviation from the approved AI6 design. The local
implementation is available for review, with the scoped validation complete.
The full CI aggregate remains failed and hosted Linux/Windows qualification,
review approval, publication, merge and GitHub completion remain separate.
