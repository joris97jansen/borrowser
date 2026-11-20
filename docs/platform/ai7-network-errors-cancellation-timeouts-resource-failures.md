# AI7: Network Errors, Cancellation, Timeouts, And Resource Failure

Issue: [#1127](https://github.com/joris97jansen/borrowser/issues/1127).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation base: `01827c613876e61929a92b429c7e99ccaca25f20`.
Date: 2026-11-20. Uncommitted implementation; qualification is recorded below.

## Ownership and supported contract

[AI1](ai1-fetch-http-networking-response-contract.md) owns architecture and
diagnostics; [AI2](ai2-typed-fetch-request-construction.md) owns admission;
[AI3](ai3-typed-fetch-response-status-headers-body.md) owns immutable response
facts and delivered progress; [AI4](ai4-network-backend-and-deterministic-fake-network.md)
owns shared fake acquisition; [AI5](ai5-real-http-response-acquisition-and-bounded-body-integrity.md)
owns framing/decoding integrity; [AI6](ai6-redirect-handling-limits-final-url.md)
owns redirect traversal, history and checkpoint precedence. AI7 refines failure
classification without replacing those lifecycles or their event envelopes.

Addressing remains the URL parser/resolver. Fetch owns error interpretation;
runtime/bus preserve the typed outcome and tab/generation/slot/route/issued-URL
correlation. Browser owns document failure, attachment settlement, buffers,
installation and decode. CSS, Layout and Paint acquire no network policy.

Synchronous AH URL failures and `RequestAdmissionError` remain separate from
failures of admitted operations. Unsupported scheme/method, credentials/headers
and invalid route/slot combinations do not manufacture workers, resource events
or HTTP responses. There is no universal error wrapper. Existing unsupported
request capabilities are explicit admission categories; absent CORS/CSP and other
security-policy APIs do not create a pretend policy decision or placeholder enum.

HTTP 404/500 remain immutable response facts on every route. Their bodies can
complete or independently fail. Metadata acquisition is not body completion,
resource acceptance, parsing, stylesheet installation, or successful image decode.

## Taxonomy and concrete evidence

`net::NetworkErrorKind` remains a small Copy/Eq enum. `ResourceLimitKind` identifies
an existing enforcement domain, not a configurable resource framework.

| Boundary/evidence | Outcome |
| --- | --- |
| `RequestAdmissionError`, AH `UrlFailure` | Existing synchronous typed rejection, no network event. |
| Backend resolver `Dns` marker | DnsFailure, unless an inspectable more specific cause applies. |
| TCP connect marker or concrete refused/reset/aborted/not-connected/broken-pipe I/O | ConnectionFailure. |
| Concrete rustls error or backend TLS-operation marker | TlsFailure; specific timeout/connection sources precede the TLS fallback. |
| Concrete TimedOut, including retained nested normalization | Timeout. |
| Existing cooperative token observed at a checkpoint | Cancelled. |
| Backend BadHeader/BadStatus or typed malformed response admission | MalformedResponse. |
| Positively detected BadBody framing marker | MalformedResponse, including local premature EOF. |
| HTTP SourceRead wrapper with known cause | Inspect the actual timeout/connection/TLS/framing/metadata cause. |
| Otherwise unclassified HTTP source read or bare local-file reader error | Read. |
| Gzip wrapper's own corruption/truncation cause | ContentDecoding. |
| Unsupported Content-Encoding token or coding stack | UnsupportedEncoding. |
| Invalid/all-empty Content-Encoding grammar | MalformedResponse. |
| Unsupported protocol/transfer capability or response-admission capability | UnsupportedResponse. |
| EncodedReader excess | ResourceLimit(EncodedBody). |
| Delivered-byte excess or invalid over-limit BodyProgress construction | ResourceLimit(DecodedBody). |
| Wire head or admitted response-header budget | ResourceLimit(ResponseHead). |
| Chunk/trailer framing metadata budget | ResourceLimit(FramingMetadata). |
| Existing RedirectError | Redirect(reason), including LimitExceeded. |
| Local URL conversion/open failure | LocalFile; no fabricated HTTP status. |
| Genuinely unclassified acquisition cause | Backend. |

Classification borrows the retained concrete source before projecting fixed safe
text. It inspects `io::Error::get_ref()` as well as known source relationships;
generic source traversal alone can skip a payload. It never parses Display text,
infers TLS from an HTTPS URL, or treats generic InvalidData/UnexpectedEof as proof
of HTTP framing failure. Unknown wrappers remain unclassified. Specific I/O
timeout/connection evidence is considered before a TLS-operation fallback.

Ordinary HTTP bodies preserve `HttpBodyError::SourceRead` through identity/gzip
processing. The 205 verification path adds the same provenance before classifying
its source error. Bare shared readers also serve files: they retain Read rather
than acquiring HTTP timeout/connection/TLS meaning. Pairwise tests protect this.

## Narrow backend corrections

Previously, rustls setup/handshake I/O and TCP connect shared ConnectionFailed.
The backend now marks TLS server-name, client creation and handshake failures
with Tls, retaining the original source. This identifies the operation without
erasing an underlying timeout, reset or certificate error.

Fixed-length readers mark only locally observed premature EOF with BadBody.
Chunked readers mark positive syntax/termination faults and locally observed
missing bytes. Reading one metadata byte explicitly distinguishes Ok(0) from
an underlying Err, preserving Interrupted retry and incrementing metadata counts
only after a byte is obtained. Arbitrary source UnexpectedEof/InvalidData stays
unchanged. Generic already-failed states return an unclassified failure, not a
new framing diagnosis; no failed reader can subsequently claim completion.

Controlled probes demonstrated two additional gaps on the implementation host:
an actual stalled TLS handshake surfaced WouldBlock beneath Tls, and a controlled
request-write WouldBlock surfaced as an unclassified acquisition failure. The
existing successful blocking socket setup is the evidence for normalization:

- TLS acquisition normalizes a direct WouldBlock source only after connect_host
  configured the owned blocking TCP socket and both read/write limits are finite.
- Request prelude/body writes and flush normalize WouldBlock only after successful
  fresh/reused socket preparation and with a finite configured write timeout.
  Body copies qualify only calls on their destination writer: a private
  RequestBodyWriter borrows the prepared Stream and preserves caller-reader
  errors unchanged through both io::copy and copy_chunked. The aggregate
  send_body result cannot establish error origin and is not normalized.
- Existing DeadlineStream response-head/body read normalization is preserved.
- Generic readers, fake faults and idle nonblocking probes are not normalized.

Normalization retains the original error beneath TimedOut. It does not introduce
retry, change timeout values, or restart a request. Replacement of an unused stale
pool candidate before transmission remains distinct from replay after writes.

Source management follows [the subtree maintenance contract](ai-dependency-subtree-maintenance.md).
The seven deliberate vendor changes are error.rs, rtls.rs, response.rs,
chunked/decoder.rs, stream.rs, unit.rs and body.rs. Existing history/import anchors,
historical patch records, file modes, dependency versions, features and Cargo
graphs remain unchanged. Proposed inventory hashes and the independently pinned
policy digest advance together; verifier behavior does not change.

## Limits, facts and terminal context

| Route | Encoded content | Delivered decoded bytes |
| --- | ---: | ---: |
| Document | 20 MiB | 10 MiB |
| Stylesheet | 4 MiB | 2 MiB |
| Image | 40 MiB | 20 MiB |

Encoded bytes are after transfer framing and before content decoding. Response
head and chunk/trailer budgets remain separate AI5 bounds. No numeric budget or
EOF/integrity rule changes. Exact budget plus verified completion succeeds;
observing another byte fails. A declared Content-Length proves neither complete
delivery nor decoded size and never authorizes a full-body allocation.

Acquired immutable metadata, headers, original/final provenance and AI6 entered
history survive failure. Delivered progress counts actual Chunk bytes. Before
initial acquisition, response/progress are None. Failed redirect-target acquisition
preserves source response facts and entered history rather than inventing target
metadata. Admission/body progress absence follows AI5/AI6's existing distinctions.
Framing, decoding, timeout, source, limit or cancellation failure cannot emit Done.

## Cancellation, timeouts and delivery

The runtime retains one Arc<AtomicBool> per (TabId, RequestId) navigation generation.
Associated operations share the flag. This is not per-resource cancellation, and
retained cancellation records are not redesigned. Checks remain before initial
and subsequent acquisition, at AI6 redirect checkpoints, after Start before decoder
construction, and before each outer body-reader call in the shared stream loop.
Internal gzip, framing, or TLS work and Interrupted retries can perform additional
source reads before the next cancellation checkpoint. Raw 205 verification checks
at each iteration of its own reader loop.

A checkpoint observing cancellation settles Cancelled there. A pending redirect
is not entered history. Entry into backend acquisition commits that transition,
not DNS success, transmission or receipt. After acquisition returns, concrete
failure/admission decisions settle without a retrospective cancellation override.
Cancellation after the final checkpoint may allow acquisition or completion.
No wall-clock race ordering is promised.

Defaults remain 15 seconds connect, 30 seconds read and 30 seconds write per
backend operation. Acquisition and body processing expose inspectable timeout
causes. DNS remains synchronous without a bounded resolver deadline/cancellation
guarantee. There is no whole-Fetch deadline or immediate interruption guarantee
for blocked DNS/connect/TLS/read/write. Gzip can perform source reads while
constructing its header; its existing pre-construction checkpoint remains.

The worker invokes one terminal callback on its supported ordinary failure/success
paths. This is producer emission, not an acknowledgement protocol or exactly-once
end-to-end delivery. Existing ignored channel-send errors remain a limitation.

Browser document failure retains authority to cancel dependent generation work.
CSS/image-local failure clears partial state without cancelling siblings. Existing
stale-generation/slot/issued-key checks, completed-resource protection and image
navigation retirement remain in force. Presentation formatting may reduce a typed
outcome to fixed text; text never selects cancellation or other semantic outcomes.

## Fake versus production evidence

The immutable fake acquisition table accepts validated DnsFailure,
ConnectionFailure, TlsFailure, Timeout and Backend outcomes. Unsupported fixture
categories reject atomically. Missing routes are Backend with a fixed diagnostic.
There is no real fallback or I/O during fake registration/acquisition.

Typed reader faults use finite encoded offsets and admitted io::ErrorKind values.
Each acquisition owns fresh reader state. Offset zero and the EOF offset fail
when read/probed; out-of-range offsets and persistent Interrupted configurations
reject. Empty-buffer reads succeed without consuming or settling. Bare WouldBlock,
UnexpectedEof and InvalidData remain Read, not timeout/framing evidence.

Fake failures prove shared lifecycle and consumer handling, not DNS/TLS/OS timeout
behavior. Fake bytes use actual shared admission, decoding and body budgets;
redirects use actual traversal and cancellation uses real token checkpoints.
Production evidence separately exercises actual resolver acquisition through the
existing resolver seam, concrete causes, real local TLS certificate/handshake
failure, response-head/body timeouts, framing readers and connection/request counts.
No public network, artificial sleep or generic scheduler is required.

## Diagnostics and exclusions

Stable output means bounded, redacted structural Rust Debug for the supported
implementation/toolchain baseline. Exact surfaces are NetworkErrorKind and its
limit domain, NetEvent::Error and CoreEvent::NetworkError ordinary/pretty/nested
formatting, and fixed network-error log field order/stage/category. Existing
response/body Debug retains its structural contract. Literal expectations and
PRIVATE URL/credential/header/body/source sentinels protect those surfaces.
The contextual CoreEvent::NetworkError regression independently pins field order,
ResourceLimit(EncodedBody), Some(response), Some(body), status/progress, redaction
placeholders and non-exhaustive markers in ordinary and pretty literal forms.
It also pins ordinary nesting inside Some(Err(...)); pretty nesting is checked
for boundedness and redaction. Private URL/header/error contents and correlation
IDs are absent. This is representative structural stability for the supported
implementation/toolchain, not exhaustive enum mirroring or a public wire schema.

No arbitrary OS/backend/decoder/source text enters diagnostic projection. Semantic
response facts remain available to authorized code; redacted output is never a
semantic key. No serde dependency, public schema or persistent identifier is added.

No automatic retry/cache fallback, new UI, policy enforcement, scheduler, generic
persistent failure hierarchy, runtime acknowledgement protocol or lifecycle
framework is introduced. AI8 #1128 retains aggregate metadata; AI9 #1129 retains
AG networking integration. AJ/AK retain full document/subresource lifecycles;
AT retains cache behavior. No separate implementation/test/docs issue is needed.

## Qualification and closeability

Current command evidence is appended after qualification. Source/Cargo function
checks over the proposed working tree are labelled as such. Committed-candidate
dependency gates and the complete make ci aggregate remain pending a separately
authorized commit. No temporary/disposable commit, gate change, ignore or bypass
is used to turn an expected uncommitted-input rejection into a pass.

This uncommitted implementation does not establish issue closure, hosted
qualification, owner acceptance, publication or merge.

### Initial reviewed candidate execution

This section preserves the evidence for reviewed patch
`a47a1d9326a6c76bd23b49c32bbfe3426f079d998fde0064bbfe7ee59c891d1a`.
It is historical evidence for that candidate, not qualification of later corrected
code. The review-correction qualification below records the fresh results.

Current execution used Rust/Cargo 1.92.0 on macOS arm64. Cargo commands were
locked/offline; Make execution used CARGO_NET_OFFLINE=true. Counts describe
separate executed selections, not a deduplicated aggregate. Loopback fixtures
needed native socket access: an initial restricted run failed to bind and the
actual tests were rerun with socket access. No public network service was used.

The final AI7 net-lib selection (`cargo test -p net --lib ai7_ --locked --offline`)
passed all 12 tests. Targeted fake tests passed 2/2 and the actual framing-reader
regression passed 1/1. The affected observation-qualified selection passed 659
tests (including one net doctest), zero failures/ignores. Current complete default
and HTML5 workspace selections each executed 84 binaries: 2,990 passed, nine
failed, 18 inherited ignored; only borrowser-conformance native wrappers failed.
No AI7 ignore, skip, suppression or assertion weakening was introduced.

The changed vendor files separately pass Rust 2018 rustfmt; root make fmt-check
does not cover that excluded workspace. Formatting altered only the intentional
vendor changes. The existing middleware unused-parentheses warning and existing
release parser warnings remain visible. Fuzz results qualify the existing smoke
commands, not broad conformance or coverage-guided/sanitizer qualification.

| Exact command | Current result |
| --- | --- |
| `cargo test -p net -p runtime_net -p bus -p runtime_css -p browser --features browser/conformance-observation --no-fail-fast --locked --offline` | PASS: 659 passed, zero failures/ignores. |
| `make fmt-check` | PASS (0). |
| `rustfmt --check --edition 2018 vendor/ureq/src/error.rs vendor/ureq/src/rtls.rs vendor/ureq/src/response.rs vendor/ureq/src/chunked/decoder.rs vendor/ureq/src/stream.rs vendor/ureq/src/unit.rs` | PASS (0). |
| `cargo clippy -p browser --all-targets --features conformance-observation --locked --offline -- -D warnings` | PASS (0). |
| `cargo test -p browser --features conformance-observation ai6_runtime_redirected_document_stylesheet_image_smoke --locked --offline` | PASS (0). |
| `make ci` | REJECTED (2): uncommitted qualification input; pending committed candidate, not a pass. |
| `make qualify-network-release` | PASS: actual release abort, 36 ordinary workers and all supervisor controls. |
| `make check-network-dependencies` | REJECTED (2): uncommitted qualification input; pending committed candidate, not a pass. |
| `python3 -I -B tools/ci/check_dependency_subtrees.py --tests` | REJECTED (1): uncommitted qualification input; pending committed candidate, not a pass. |
| `make conformance-ci` | PASS (0). |
| `make conformance-addressing` | PASS (0). |
| `make lint` | PASS (0). |
| `make lint-html5` | PASS (0). |
| `make lint-html5-hardening` | PASS (0). |
| `make test-html5-parser-conformance` | PASS (0). |
| `make test-html5-external-fixtures` | PASS (0). |
| `make test-html5-parser-fatal-failures` | PASS (0). |
| `make test-html5-dom-golden` | PASS (0). |
| `make test-html5-patch-golden` | PASS (0). |
| `make test-html5-smoke-real-pages` | PASS (0). |
| `make test-html5-rawtext-script-regressions` | PASS (0). |
| `make test-html5-tokenizer-fuzz-smoke` | PASS (0). |
| `make test-html5-tokenizer-script-data-fuzz-smoke` | PASS (0). |
| `make test-html5-tokenizer-rawtext-fuzz-smoke` | PASS (0). |
| `make test-html5-tokenizer-rcdata-fuzz-smoke` | PASS (0). |
| `make test-html5-tree-builder-token-fuzz-smoke` | PASS (0). |
| `make test-html5-pipeline-regressions` | PASS (0). |
| `make test-html5-pipeline-fuzz-smoke` | PASS (0). |
| `make test-wpt-tree-builder` | PASS (0). |
| `make build` | PASS (0). |
| `make build-html5` | PASS (0). |
| `make build-release` | PASS (0). |
| `make build-release-html5` | PASS (0). |
| `make compile-html5-benches` | PASS (0). |
| `make compile-css-benches` | PASS (0). |
| `make html-entities-check` | PASS (0). |
| `cargo test --workspace --lib --bins --tests --examples --locked --offline --no-fail-fast` | FAIL (101): 2,990 passed; nine native wrapper failures; 18 inherited ignores. |
| `cargo test --workspace --lib --bins --tests --examples --features html5 --locked --offline --no-fail-fast` | FAIL (101): 2,990 passed; nine native wrapper failures; 18 inherited ignores. |
| `cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline chromium::tests::native_ -- --test-threads=1` | FAIL (101): one passed, same nine failed, one inherited ignored; serial exact-base comparison. |
| `cargo test -p borrowser-conformance --bin borrowser-conformance --locked --offline chromium::tests::native_ -- --test-threads=1` (untouched base archive) | FAIL (101): one passed, same nine failed, one inherited ignored; serial exact-base comparison. |

`make lint`, `make fmt-check`, `git diff --check`, and supported source/Cargo
function verification were also rerun after the final test assertion changes.
The 12-test targeted selection includes actual bare-versus-HTTP shared-reader
execution, source handling throughout gzip, TLS marker precedence, actual stalled
TLS/head timeouts, and the controlled blocking-write timeout/no-replay probe.

Before the conditional stream/unit corrections, the unrestricted TLS stall probe
returned TlsFailure and the blocking-write probe returned Backend. Both now pass
with Timeout while retaining source errors. The release probe now authors an
explicit expected Start flag per case, since MalformedResponse can occur before
or after Start; category alone no longer determines the phase. Its full final
release execution passed all existing controls and workers.

### Initial reviewed candidate source and Cargo evidence

`python3 -I -B /tmp/ai7-source-qualification.py` passed supported library functions:
check_python_import_artifacts, load_qualification, check_history, verify_source
for both maintained dependencies, check_historical_snapshot, check_resolution
for current ureq, and check_url_dependencies.check_repo. The latter verifies main
default/html5/hardening/conformance-observation/all-feature and fuzz default/all-feature
Cargo graphs, exact source routing, features and registry origins. This is
explicitly WORKING-TREE evidence, not a committed-candidate gate. No verifier
behavior, historical anchor, file mode, manifest, lock or feature changed.

The policy digest advances from
`e9057883fbd9510f2c8547197fa32b404f481941243c6c8059ed136428703b61` to
`eacca93ded48beda9b9e6ef0eaf26af95ef7d1f50fce4e915c631336a85c7280`.
The JSON diff contains only the six deliberate source hash entries. The Python
verifier diff contains only its independent POLICY_SHA256 constant. Historical
ureq records/patches remain unchanged.

### Initial reviewed candidate native baseline comparison

The exact base was extracted with git archive into a temporary directory, without
creating a Git commit, worktree or branch. Both base and candidate ran the serial
chromium::tests::native_ command above, with a shared build cache but independent
source trees. Both failed exactly these wrappers:

- native_capture_failure_diagnostics
- native_cleanup_diagnostic_failures
- native_lifecycle_regressions
- native_macos_bsd_ownership_states
- native_process_and_pipe_contract
- native_resource_completion_and_shutdown
- native_shutdown_uses_capture_deadline
- native_startup_exit_event_classification
- native_temporary_artifact_containment

Current diagnostics include unavailable live executable identity during ownership
scans and consequent cleanup identity/timeouts. This reproduces an inherited
platform qualification failure; it is not a waiver, a green workspace aggregate,
or a borrowed AI5/AI6 result. No conformance implementation change addresses it.

### Remaining final-candidate work

The unchanged gates reject the uncommitted Python qualification input before
reaching their remaining checks. They must be rerun after a separately authorized
commit, together with the complete make ci aggregate and applicable hosted checks.
The guarded Python qualification-test suite remains pending; it was not run via
raw discovery or a temporary commit. All applicable independent remaining CI
lanes above executed with their actual exit status retained.

Architecture review: ownership, enum/source handling, deterministic fixtures,
terminal context, resource guards and no-replay boundaries are preserved. No
shortcut, demo behavior or unrelated abstraction was found. The implementation
meets functional AI7 criteria in working-tree evidence, but #1127 is **not
closeable yet**: exact committed-candidate qualification, unresolved native CI
failures and final owner review/acceptance remain outstanding. AI8/AI9, AJ/AK and
AT retain their existing work; no new follow-up issue is proposed.

### Changed-file map

Paths are repository-relative. CSS, Layout, Paint, Addressing and runtime/bus
production ownership and event envelopes remain unchanged.

| File | Purpose |
| --- | --- |
| `crates/browser/examples/ai5_abort_probe.rs` | Precise failure expectations and independently authored Start expectations for release qualification. |
| `crates/browser/src/resources.rs` | Test-only delivered-limit category expectation; production settlement unchanged. |
| `crates/browser/src/tab/status.rs` | Fixed presentation labels for refined categories; no new UI flow. |
| `crates/browser/src/tab/tests/ai3_responses.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/browser/src/tab/tests/ai5_failures.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/browser/src/tab/tests/network.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/browser/src/tab/tests/resource_associations.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/backend.rs` | Concrete source classification with honest acquisition fallback and fixed head diagnostics. |
| `crates/net/src/body.rs` | Unsupported content encoding and delivered-body limit categories. |
| `crates/net/src/event.rs` | Typed failure categories, four limit domains, fixed diagnostics and literal redacted Debug tests. |
| `crates/net/src/fake.rs` | Validated acquisition/reader faults, independent readers and empty-read/EOF tests. |
| `crates/net/src/fetch.rs` | Response-head limit mapping and fixed diagnostic coverage at checkpoints/admission. |
| `crates/net/src/file.rs` | Fixed acquisition/body logging without changing local-file failure semantics. |
| `crates/net/src/lib.rs` | Export ResourceLimitKind with the existing event types. |
| `crates/net/src/log.rs` | Literal stage/category output regression tests. |
| `crates/net/src/stream.rs` | HTTP source provenance, 205 mapping, decoding/limit distinctions and paired/gzip regressions. |
| `crates/net/src/tests/ai5.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/ai5_connections.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/ai5_decoding.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/ai5_diagnostics.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/ai5_tls_reuse.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/backend.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/limits.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/redirects.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/response_adapter.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/response_body.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/src/tests/tls.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/tests/backend_body_completion.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/tests/fake_network.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/net/tests/fetch_response.rs` | Extend or refine deterministic failure/category assertions while preserving existing lifecycle and integrity coverage. |
| `crates/runtime_net/src/tests.rs` | Actual production forwarding, categories, correlation and partial-progress regression. |
| `docs/architecture/ARCHITECTURE.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/engine-feature-gap-tracker.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/platform/ai-dependency-subtree-maintenance.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/platform/ai1-fetch-http-networking-response-contract.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/platform/ai4-network-backend-and-deterministic-fake-network.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/platform/ai5-real-http-response-acquisition-and-bounded-body-integrity.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `docs/platform/ai6-redirect-handling-limits-final-url.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |
| `tools/ci/check_dependency_subtrees.py` | Only the independently pinned policy digest changes. |
| `tools/ci/dependency-subtrees.json` | Seven deliberately changed source hashes; no mode/history/graph changes. |
| `vendor/ureq/src/chunked/decoder.rs` | Positive framing markers, source preservation, Interrupted accounting and honest failed states. |
| `vendor/ureq/src/error.rs` | Narrow Tls and BadBody markers in the existing error enum. |
| `vendor/ureq/src/response.rs` | Mark only locally observed premature fixed-length EOF. |
| `vendor/ureq/src/rtls.rs` | Retain TLS operation identity and original sources. |
| `vendor/ureq/src/stream.rs` | Demonstrated TLS blocking-timeout normalization with retained cause. |
| `vendor/ureq/src/unit.rs` | Demonstrated prepared-socket write/flush normalization without replay. |
| `vendor/ureq/src/body.rs` | Destination-only body-write normalization preserving caller-reader error provenance in both copy paths. |
| `docs/platform/ai7-network-errors-cancellation-timeouts-resource-failures.md` | AI7 contract, qualification, ownership or feature-gap status reference; earlier dated records preserved. |

## Review corrections and fresh qualification

### Candidate identity and preservation

The reviewed content was unchanged when corrections began: reconstructing the
original complete-patch order reproduced
`a47a1d9326a6c76bd23b49c32bbfe3426f079d998fde0064bbfe7ee59c891d1a`.
HEAD/base remained `01827c613876e61929a92b429c7e99ccaca25f20`, on
`feat/1127-network-error-taxonomy-cancellation-timeouts-resource-failures`.
The original candidate had been staged since review; its staged patch was preserved
byte-for-byte and review corrections remain unstaged. No unrelated content
change, branch change, commit, push or GitHub mutation was performed.

### Findings and regression evidence

The aggregate body::send_body error could originate in the caller reader or the
destination. A finite destination timeout did not identify its source. unit.rs
now passes the prepared-socket qualification into body.rs without normalizing the
aggregate result. A private RequestBodyWriter borrows the existing Stream and
normalizes only its Write/flush calls; io::copy and copy_chunked preserve caller
read errors and their concrete source chains. This adapter has that single
current provenance requirement. No general error framework or Borrowser Fetch
request-body capability is introduced. Prelude, handshake, response reads,
connection ownership and no-replay semantics remain unchanged.

The new caller-body regression first failed on the reviewed code: TimedOut was
observed where WouldBlock was expected (exit 101). The passing corrected test uses
a healthy loopback server, finite destination write timeout, and explicit
request-head acknowledgement before injecting caller WouldBlock with a nested
PermissionDenied cause. Both explicit-content-length and chunked paths retain
that exact chain, one body read, one server request and no replay.

Positive controls inject actual destination body-write and flush WouldBlock at the
existing transport seam, for both copy paths and fresh/reused connections. They
retain TimedOut -> original WouldBlock, map to Timeout, and preserve exactly one
failed attempt without replay. The existing prelude-write and real handshake/head/
body timeout controls also passed in the fresh affected lane. Controlled socket
fault evidence and actual loopback/OS timeout evidence remain distinguished.

The added runtime_net test independently authors literal contextual
CoreEvent::NetworkError ordinary/pretty output and ordinary nesting, with a
ResourceLimit(EncodedBody), Some(response), Some(body), status and progress.
Private URL/header/error contents and correlation identifiers are excluded;
pretty nesting is also checked for redaction and a finite output bound. Existing
NetEvent/log literal and CoreEvent redaction tests remain. No serializer, new
formatting abstraction, public schema or exhaustive enum mirror is added.

Cancellation implementation is unchanged. Its corrected checkpoint wording is:

> Checks remain before initial and subsequent acquisition, at AI6 redirect
> checkpoints, after Start before decoder construction, and before each outer
> body-reader call in the shared stream loop. Internal gzip, framing, or TLS work
> and Interrupted retries can perform additional source reads before the next
> cancellation checkpoint. Raw 205 verification checks at each iteration of its
> own reader loop.

### Fresh corrected-code commands and results

The following commands executed after the corrections, using Rust/Cargo 1.92.0,
locked/offline Cargo and CARGO_NET_OFFLINE=true for Make. Controlled loopback tests
ran with native socket access. Earlier candidate results above are preserved as
historical evidence, not substituted for the following fresh qualification.

| Exact command | Result |
| --- | --- |
| `cargo test -p net -p runtime_net ai7_ --locked --offline -- --nocapture` | PASS: 19 tests, zero failures/ignores. |
| `cargo test -p net -p runtime_net -p bus -p runtime_css -p browser --features browser/conformance-observation --no-fail-fast --locked --offline` | PASS: 662 tests including one net doctest, zero failures/ignores. |
| `make fmt-check` | PASS (0). |
| `rustfmt --check --edition 2018 vendor/ureq/src/body.rs vendor/ureq/src/unit.rs vendor/ureq/src/error.rs vendor/ureq/src/rtls.rs vendor/ureq/src/response.rs vendor/ureq/src/chunked/decoder.rs vendor/ureq/src/stream.rs` | PASS (0). |
| `make lint` | PASS (0). |
| `cargo clippy -p browser --all-targets --features conformance-observation --locked --offline -- -D warnings` | PASS (0). |
| `cargo test -p browser --features conformance-observation ai6_runtime_redirected_document_stylesheet_image_smoke --locked --offline` | PASS (0). |
| `make qualify-network-release` | PASS: all 36 ordinary workers and existing abort/supervisor controls. |
| `python3 -I -B /tmp/ai7-source-qualification.py` | PASS: working-tree source/history/features/main-and-fuzz Cargo verification only. |
| `make ci` | REJECTED (2): uncommitted qualification input; not a passing gate. |
| `make check-network-dependencies` | REJECTED (2): uncommitted qualification input; not a passing gate. |
| `python3 -I -B tools/ci/check_dependency_subtrees.py --tests` | REJECTED (1): uncommitted qualification input; not a passing gate. |
| `git diff --check` | PASS (0). |

The new focused tests are:

- ai7_caller_body_would_block_is_not_a_destination_timeout_in_either_copy_path
- ai7_destination_body_write_and_flush_timeouts_preserve_sources_without_replay
- ai7_contextual_core_error_debug_has_independent_literals_and_redacts_nested_contents

Exact command output is retained in /tmp/ai7-review-*.log and the command ledger
/tmp/ai7-review-results.jsonl. The reproducing failure is retained separately in
/tmp/ai7-review-source-provenance-before.log. The existing qualification helper
/tmp/ai7-source-qualification.py and initial baseline/candidate comparison logs
/tmp/ai7-native-baseline.log and /tmp/ai7-native-candidate.log are preserved.
The earlier untouched-base archive and original complete patch are retained too.

### Corrected inventory and remaining qualification

Relative to the reviewed inventory, only these entries advance, preserving mode
100644:

| File | Reviewed SHA-256 | Corrected SHA-256 |
| --- | --- | --- |
| src/body.rs | 58c9464760578ee695e53647c7de3f37509f123776db34bdbfd7ad9fb3c83866 | 10a4ef3c7a54944769695e81e44bf87d4d23e2b3c72f90bc14bcda4f1163ed81 |
| src/unit.rs | a7ff65b672f5a1f166b6b7d07699eb27c5dd0b386ae0b5434df8105b553c9a6a | 49ba2ca0bd4d0b6f4c5e5702c0e741dbbd0d8689521fa04a4f7694fe8d6aa287 |

The independently pinned policy digest advances from
`eacca93ded48beda9b9e6ef0eaf26af95ef7d1f50fce4e915c631336a85c7280` to
`05bf5be4ff5c681178fe7519d030197927b11e08b84c0bc155c19609ac5130f3`.
Verifier behavior, historical anchors/records, other inventory entries, modes,
versions, features, manifests and locks remain unchanged. Rust 2018 formatting
passes for all seven intentionally changed vendor Rust files.

Architecture review found the corrections consistent with the approved AI7 plan:
error origin is resolved at the owned destination boundary, diagnostics remain
structural, and the documentation matches existing cooperative checkpoints. No
shortcut, demo-only behavior or unrelated framework was introduced. All requested
review findings have working-tree regression evidence.

The correction did not rerun unrelated parser/fuzz/build selections or the full
workspace/native comparisons: their earlier failures and logs remain intact, not
waived or relabeled as current qualification. Final committed-candidate source
and guarded qualification-test gates, complete make ci and applicable hosted
qualification remain pending a separately authorized commit. The existing nine
native failures still require resolution/qualification; no skip or waiver was
added. Final owner review/acceptance also remains outstanding. #1127 is **not
closeable yet**. No new issue or milestone is warranted.
