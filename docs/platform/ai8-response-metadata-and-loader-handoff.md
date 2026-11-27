# AI8: Response Metadata And Resource-Loader Handoff

> AJ3 current qualification: [AJ3](aj3-main-document-response-preparation.md) adds immutable ResponseContentDisposition type-only classification in net: Missing, Inline, Attachment, Other and Unusable. One field's ASCII token is classified without filename/parameter interpretation; malformed, empty, repeated or ambiguous types remain Unusable facts. Raw fields survive unchanged across real/fake/supplied response construction. Browser owns download rejection and ignores Unusable for disposition applicability before applying its status/MIME policy. No transport error or download capability is added.

> Current-status qualification (2026-11-21): AI8 is merged through PR #1482 and
> present in master `f77a6834`. The working-tree/uncommitted and pending-hosted
> statements below retain their historical validation scope. They are not a
> current claim that aggregate metadata remains unimplemented or unmerged.
> [AI10](ai10-fetch-http-networking-response-handling-foundation-closeout.md)
> records current scope and exact-source baseline CI. Its own documentation
> qualification and formal milestone closure remain pending; prior results and
> source identities are preserved.

Issue: [#1128](https://github.com/joris97jansen/borrowser/issues/1128).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation base: `cc65bc134f21c31e3e34937132082c583b2de4de`.
This describes an uncommitted working-tree implementation. Validation evidence
is recorded below; committed-candidate qualification, hosted CI, publication and
issue/milestone closure are separate.
The original qualification history is preserved separately from the focused
review corrections and their new working-tree evidence at the end of this record.

## Browser completion admission

[AJ7](aj7-deterministic-stylesheet-acquisition.md) preserves the MIME subset above
and admits CSS only after authenticated complete acquisition and strict usable
final provenance. Complete rejected bodies and legitimate failed prefixes survive
on attachments. Error may supply contextual redirect/admission/coding facts before
Start; those are not a selected final response. Wrong-key/conflicting optional
facts are omitted, independently of primary failure precedence. Present progress
must be CSS-route and equal the legitimately observed count, including before
Start (absent or zero is legitimate). Producer error kind remains separate.
Charset is retained without adding decoding or broader nosniff/quirks enforcement.

## Ownership and handoff

[AI1](ai1-fetch-http-networking-response-contract.md) governs ownership;
[AI2](ai2-typed-fetch-request-construction.md) governs request admission;
[AI3](ai3-typed-fetch-response-status-headers-body.md) governs immutable response
facts. [AI4](ai4-network-backend-and-deterministic-fake-network.md),
[AI5](ai5-real-http-response-acquisition-and-bounded-body-integrity.md),
[AI6](ai6-redirect-handling-limits-final-url.md), and
[AI7](ai7-network-errors-cancellation-timeouts-resource-failures.md) retain shared
execution, framing/decoding, redirects and failure authority. AH URL, origin,
issuance/attachment identity and retained stylesheet-base contracts remain intact.

Fetch/Networking owns response-header interpretation. Browser retains resource
applicability, document lifetime, stylesheet attachment/installation and image
decode decisions. CSS owns syntax/cascade/computed style; Layout and Paint gain
no HTTP responsibility. No new crate, event envelope, loader framework or metadata
wrapper is introduced.

`NetworkResponseInfo` remains the handoff:

```text
actual FetchRequest.address()
  -> real / fake / local / supplied response construction
  -> Arc<ResponseFacts>: exact acquired facts + cached metadata
  -> NetEvent -> runtime_net -> CoreEvent -> Browser consumer
```

Both constructors borrow the original `ResourceAddress`, retaining its URL and
producer-known `addressing::ResourceKind` together. Production and supplied paths
pass the actual request's address. Purpose is never inferred from MIME or mapped
back from `core_types::ResourceKind::{Html,Css,Image}`. Hyperlink purpose remains
Hyperlink on Html execution and through redirects. Representing Font/Media facts
does not add execution routes or loaders.

Public read-only accessors are `content_type() -> &ResponseContentType`,
`declared_content_length() -> DeclaredContentLength`, and
`resource_kind() -> addressing::ResourceKind`, alongside existing status, headers,
URL and redirect accessors. `ContentType::{mime_type,charset}` borrow the selected
essence and declared label. The cached interpretation is constructed once; private
facts have no mutation API. Original ordered header bytes and raw local MIME are
retained independently and remain equality facts even if canonical metadata agrees.
The existing copy-on-write redirect-history attachment preserves retained snapshots.

## Content-Type interpretation

Authorities inspected for the approved design on 2026-11-20:
[Fetch MIME extraction](https://fetch.spec.whatwg.org/#concept-header-extract-mime-type),
[Fetch get/decode/split](https://fetch.spec.whatwg.org/#concept-header-list-get-decode-and-split),
and [MIME parsing](https://mimesniff.spec.whatwg.org/#parse-a-mime-type).

The internal temporary logical combination joins same-name occurrences with
comma/space in original order. A byte-slice scanner implements quote/escape-aware
splitting, retaining empty members and preserving quotes for the existing MIME
parser. Quote state crosses original field boundaries. Isomorphic decoding does
not change delimiter decisions; charset bytes are decoded by the existing parser,
never lossy UTF-8. Neither this operation nor selection changes stored headers.
Admitted header budgets bound scratch storage and parsing work; component slices
borrow the temporary combination rather than allocating a string for every comma.

Extraction follows Fetch's algorithm order. Failed MIME candidates and exact
`*/*` essence are ignored. Separate essence/charset carry state is reset when
the selected essence changes. A later explicit same-essence charset affects that
candidate, but does not replace the run's carry state. The existing per-field
WHATWG parser and its recovery rules remain the authority. Non-charset parameters
are scanned but not retained in a speculative parameter registry.
Discarded same-essence candidates do not materialize inherited charset strings.
Only after selection does the final candidate without an explicit charset take
ownership of the saved run charset. An explicit final charset stays authoritative.

| ResponseContentType | Meaning |
| --- | --- |
| Missing | No declaration. |
| Empty | Every original declaration contains only HTTP whitespace. This is a Borrowser compatibility distinction. |
| Unusable | Present, nonempty declarations produce no selected MIME. Includes exact wildcard-only declarations. |
| Parsed(ContentType) | Selected normalized essence and optional declared charset string. |

Missing/empty/unusable never fabricate a MIME default. `text/*` and `*/css` remain
parsed literal essences, without wildcard matching. Unknown MIME and charset labels
remain facts; an explicitly quoted empty charset stays present-empty. The API does
not resolve charset labels, decode text, read BOM/meta declarations, sniff bytes,
or claim a complete serialized MIME record. `declared_content_types()` remains
per-field inspection only; loaders use aggregate `content_type()`.

Independent literal cases in `net/tests/response_metadata.rs` cover HTML, CSS,
images, malformed/recoverable declarations, quoted commas/semicolons/escapes,
unfinished quotes spanning occurrences, duplicate charset parameters, unknown
labels, obs-text, wildcard handling, conflicting order and charset carryover.
For example, CSS charset A / CSS charset B / CSS without charset selects A;
CSS without charset / CSS charset B / CSS without charset selects no charset.

## Declared length and framing

Authorities:
[Fetch length extraction](https://fetch.spec.whatwg.org/#concept-header-extract-content-length),
[RFC 9110 section 8.6](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6),
and [RFC 9112 section 6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3).

`DeclaredContentLength` compares all logically split textual candidates before
syntax or range interpretation. Equal nonempty ASCII-digit candidates project to
a checked u64. Missing and unusable distinguish absent fields from Fetch's null
result; Conflicting represents differing candidates; OutOfRange records the
bounded projection of a syntactically numeric declaration.

| Declarations | Metadata result |
| --- | --- |
| absent | Missing |
| `0` | Value(0) |
| `007`, or repeated `007` | Value(7) |
| `7, 7` | Value(7) |
| `7, 007` | Conflicting |
| `7, 8`, `bad, 7`, `7,,7` | Conflicting |
| empty, `-7`, `+7`, `7x`, quoted `7`, repeated identical `bad` | Unusable |
| `18446744073709551615` | Value(u64::MAX) |
| `18446744073709551616` | OutOfRange |
| overflowing numeral followed by a different candidate | Conflicting, before numeric interpretation |

AI5's private backend parser continues comparing numeric framing values, so
`7` and `007` can frame seven bytes while metadata reports Conflicting. This is
deliberate, not a new networking error. No vendor API or second framing decision
is added. Real malformed/conflicting/overflowing wire framing still fails before
response adaptation. Metadata extraction never selects a reader, allocates from
a declaration, changes a budget, or proves completion.

Declared length describes encoded content where applicable. Observed encoded
bytes remain internal to the existing bounded body reader, after transfer framing
and before content decoding. `BodyProgress` counts delivered decoded bytes.
A 304 can carry representation-length metadata while delivering zero bytes;
existing 204/205/304 framing and completion rules are unchanged. Existing gzip
decoding retains the original length/encoding fields and does not rewrite metadata.

Fake and supplied responses use the same metadata construction, but do not parse
wire framing. Their finite reader can complete independently of the declaration;
even Conflicting, Unusable or OutOfRange metadata does not manufacture wire
validation or a fake framing error. Real/fake parity is asserted for equivalent
admitted facts; real malformed-head tests separately exercise the actual parser.

## Provenance, classification and body outcomes

Original URL, exact issued correlation string, exact reported response spelling,
cached AH parse result and actual redirect chain retain their distinct meanings.
`response_url()` is final for a selected Start/Done response. A contextual Error
after Start retains that snapshot. Before selection, a failed target acquisition
can retain the last acquired redirect response and entered history; no response
or final URL is invented for the failed destination. Absent/invalid reported URL
states remain explicit, with no requested/document-URL fallback.

`HttpStatus::class()` derives Informational/Successful/Redirection/ClientError/
ServerError from the existing valid numeric code, preserving unknown valid codes.
Existing `is_success()` and `is_http_error()` are specifically status predicates.
Response constructors still reject informational responses. Local results have
no HTTP status/class. HTTP 404/500, transport/body failure, resource applicability,
parsing/decoding and installation remain independent outcomes. There is no
response-wide success boolean. Acquired metadata is never a complete body.

## Current Browser CSS policy

Browser now consumes typed metadata rather than splitting a first-field string.
It accepts Missing/Empty for existing compatibility, accepts Parsed `text/css`,
and rejects Unusable or another essence. This compatibility policy is not a claim
of complete stylesheet MIME/security-policy conformance.

| Case | Previous policy | AI8 policy |
| --- | --- | --- |
| absent / empty / SP or HTAB only | accept | accept |
| `TEXT/CSS; charset=UTF-8`, recoverable `text/css;` | accept | accept |
| image MIME / broken / exact wildcard | reject | reject |
| `;charset=UTF-8` | accept | reject |
| unusable non-UTF-8 field / Unicode whitespace around CSS essence | accept | reject |
| separate CSS then image declarations | accept | reject |
| separate image then CSS declarations | reject | accept |
| empty then image declarations | accept | reject |
| invalid then CSS declarations | reject | accept |
| CSS then invalid/wildcard declarations | accept | accept |
| combined image then CSS | reject | accept |
| combined CSS charset A then image | accept | reject |

`normalized_content_type()` and `legacy_content_type()` are removed. Fixed
rejection wording no longer interpolates the raw first field. Existing exact
issuance, slot/generation, duplicate-start, immutable completion, CSS assembly,
attachment order, retained response base and installation invariants remain.
The regression matrix traverses actual Browser commands, CSS runtime assembly
and stylesheet installation, including rejection settlement and duplicate Start.

Documents/images already retain the response and need no artificial adapter or
new MIME acceptance rule. Local-file MIME remains one local-loader fact parsed
as a single value, with raw spelling retained. No HTTP fields/status or inferred
filesystem Content-Length are fabricated.

## Diagnostics, extensions and exclusions

Debug uses fixed structural order and bounded, redacted metadata. Selected MIME,
charset and declared length values do not appear; outcome categories, purpose,
status/class and existing delivered-byte accounting do. Ordinary/pretty/nested
literal expectations and private-content sentinels cover direct metadata,
response/event envelopes and failed sends. This is an internal regression contract,
not a public serialization schema or request identity.

Future MIME sniffing must distinguish declared from computed MIME and justify its
body access. Encoding extensions belong at the existing Fetch/body boundary and
must preserve declaration versus delivery facts. Integrity needs concrete
verification and terminal semantics. Policy checks require their actual owning
capability and captured context, not empty modes or placeholder success decisions.

No new body parsers, charset decoding, sniffing, loader framework, execution route,
Font/Media loader, integrity pipeline, cache/cookie/security enforcement, or
unrelated lifecycle refactor is included. AI9 retains broader AG networking
fixture/discovery/reporting work. Required AI8 behavior and tests are not deferred.

## Historical working-tree validation before focused review corrections

This section preserves the earlier candidate's commands, failures, baseline
reproduction, smoke observations and review assessment. These executions predate
the charset-allocation and hyperlink-fixture corrections recorded below. They
are historical evidence and do not qualify the corrected working-tree candidate.

All results below are local working-tree evidence on macOS on 2026-11-20, not
committed-candidate or hosted qualification. No temporary commit was used for a
gate. Detailed command/exit records and logs are retained locally under
`/private/tmp/borrowser-ai8-validation/`; they are not repository workflow files.

### Targeted and subsystem execution

| Exact command | Observed result |
| --- | --- |
| `cargo test -p net --test response_metadata --locked` | 8 passed, including the final cross-occurrence escape case. |
| `cargo test -p net --test fetch_response --locked` | 7 passed. |
| `cargo test -p net --lib ai8_ --locked` | 2 passed: real/fake projection and redirect/failure context. |
| `cargo test -p browser --lib ai8_ --locked` | 3 passed: full CSS matrix, exact completion facts and document retention. |
| `cargo test -p runtime_net --lib --locked` | 5 passed. |
| `cargo test -p net --locked` | 272 unit/integration tests and 1 doctest passed. |
| `cargo test -p browser --lib --locked` | 380 passed. |
| `cargo test -p browser --lib --features conformance-observation --locked` | 390 passed. |

The 14 new AI8 tests are registered and executed by Cargo. Table-driven cases use
literal independent expectations rather than outputs from the extraction code.
The full workspace runs below also executed the affected Browser, runtime,
net and supplied-response consumers after constructor/API migration. Final
review corrected one CSS-returning-HTML fixture's purpose from Document to
Stylesheet. After that test-only correction, metadata tests (8), both Browser
lanes (380 / 390), strict workspace lint and formatting all passed again.

Initial sandboxed loopback attempts failed at `TcpListener::bind` with
PermissionDenied. Socket-enabled reruns passed. An initial strict lint run found
test-table type complexity; a test-local `MimeCase` alias resolved it without an
allowance, assertion change or gate change. Subsequent strict lint lanes passed.

### Aggregate CI and independent remaining gates

`make ci` exited 2 in its existing `make test` step. The conformance binary
reported 189 passed / 9 failed / 13 existing ignored tests. The nine failures were:

```text
chromium::tests::native_capture_failure_diagnostics
chromium::tests::native_cleanup_diagnostic_failures
chromium::tests::native_lifecycle_regressions
chromium::tests::native_macos_bsd_ownership_states
chromium::tests::native_process_and_pipe_contract
chromium::tests::native_resource_completion_and_shutdown
chromium::tests::native_shutdown_uses_capture_deadline
chromium::tests::native_startup_exit_event_classification
chromium::tests::native_temporary_artifact_containment
```

Current failure evidence includes `UnsupportedPlatform("process ownership: live
process executable unavailable during ownership scan")` and incomplete native
cleanup. The same nine failures were reproduced on a fresh, untouched detached
worktree of `cc65bc134f21c31e3e34937132082c583b2de4de` with:

```sh
cargo test -p borrowser-conformance --bin borrowser-conformance native_ --locked
```

That baseline run used the main worktree's existing Cargo target directory and
reported 5 passed / 9 failed / 2 ignored / 195 filtered. Its worktree was verified
clean and removed afterward. This is current baseline evidence, not an assumption
from earlier milestones. No native assertion or qualification gate was altered.

Before stopping, aggregate CI passed `make check-url-dependencies` (including all
245 tooling qualification tests), `make check-network-dependencies`,
`make conformance-ci` (7 PASS), `make conformance-addressing` (22 PASS, 2 explicitly
UNSUPPORTED), `make fmt-check`, `make lint`, `make lint-html5`, and
`make lint-html5-hardening`. Dependency checks qualify their unchanged dependency
inputs; they do not qualify the uncommitted AI8 implementation as an exact HEAD.

Every unreached top-level CI target was then executed independently:

| Exact command | Result |
| --- | --- |
| `make test-html5-runtime` | Failed on the same nine native checks; 189 passed / 9 failed / 13 ignored in that binary. |
| `make test-html5-parser-conformance` | Passed. |
| `make test-html5-external-fixtures` | Passed. |
| `make test-html5-parser-fatal-failures` | Passed. |
| `make test-html5-dom-golden` | Passed. |
| `make test-html5-patch-golden` | Passed. |
| `make test-html5-smoke-real-pages` | Passed. |
| `make test-html5-rawtext-script-regressions` | Passed. |
| `make test-html5-tokenizer-fuzz-smoke` | Passed. |
| `make test-html5-tokenizer-script-data-fuzz-smoke` | Passed. |
| `make test-html5-tokenizer-rawtext-fuzz-smoke` | Passed. |
| `make test-html5-tokenizer-rcdata-fuzz-smoke` | Passed. |
| `make test-html5-tree-builder-token-fuzz-smoke` | Passed. |
| `make test-html5-pipeline-regressions` | Passed. |
| `make test-html5-pipeline-fuzz-smoke` | Passed. |
| `make test-wpt-tree-builder` | Passed. |
| `make build` | Passed. |
| `make build-html5` | Passed. |
| `make build-release` | Passed. |
| `make build-release-html5` | Passed. |
| `make qualify-network-release` | Passed, including abort control, 30 failure processes and 6 persistence/closed-sink/image processes. |
| `make compile-html5-benches` | Passed. |
| `make compile-css-benches` | Passed. |
| `make html-entities-check` | Passed. |

These separate successes do not turn aggregate CI into a pass. Existing fuzz-smoke
and release warnings remain visible in their logs, without broader instrumentation
or conformance claims.

To let a failing target finish without preventing other workspace targets from
executing, both complete configurations were additionally run with Cargo's
no-fail-fast option:

```sh
cargo test --workspace --lib --bins --tests --examples --locked --no-fail-fast
cargo test --workspace --lib --bins --tests --examples --features html5 --locked --no-fail-fast
```

Both exited 101, reporting only the same conformance binary as a failed target;
all other test targets completed successfully. No tests were filtered or newly
ignored in these executions, and failed statuses were retained.

Final `make fmt-check` and `make lint` passed after the Rust fixture migrations.
`git diff --check` and changed-document local-link verification passed. The
supported local toolchain was `rustc 1.92.0 (ded5c06cf 2025-12-08)` (Homebrew).

### GUI smoke observation

`make run` launched the current candidate. A temporary loopback server supplied
`http://127.0.0.1:51534/` with accepted, rejected, missing-header and repeated-header
stylesheets. UI input and a captured window established all five resource
requests. Visual inspection showed accepted text green, rejected text default
black, missing-header text blue and repeated image-then-CSS text purple. Loading
settled. This observation is separate from deterministic automated correctness
coverage. The supervised Borrowser process and loopback server were stopped.

Earlier accessibility/process-directed input attempts launched the app but did
not navigate its custom address field; they were not counted as successful smoke
tests. Normal mouse events at the verified foreground window enabled the final
successful scenario. No engine test switch or permission setting was added.

### Architecture review and readiness

Review found no outstanding implementation corrections or material departure from
the approved design. Extraction stays in Fetch, CSS acceptance stays in Browser,
and all added representations serve current requirements. Immutable raw facts,
purpose/route separation, URL/failure provenance, body accounting, bounded
processing and nested redaction are covered. No shortcut, placeholder loader,
generic framework, speculative policy object or vendor change was introduced.

The implementation is ready for human review as an uncommitted candidate.
Aggregate CI remains failed on the reproduced native checks. Issue closure,
committed-candidate qualification and hosted success are not established; no new
follow-up issue is proposed for implementation or qualification phases.

## Focused review corrections and current working-tree evidence

### Charset allocation work

Review identified repeated copying of the saved charset into discarded MIME
candidates. Eight admitted Content-Type fields of 8,000 bytes account for 64,128
header bytes (64,147 bytes for the HTTP/1.1 200 OK head including the final CRLF).
An unfinished quoted charset spans the first four fields and produces a 63,978-byte
UTF-8 label. Four subsequent fields each contain 2,000 `a/a,` candidates. The
previous loop copied the inherited label 8,000 times: 511,824,000 cumulative
bytes, not peak memory. Header-storage bounds alone did not prevent this work.

`ResponseContentType::extract` now retains essence/run-charset state while selecting
the final candidate, then moves the saved charset into it only when it lacks an
explicit charset. No discarded candidate receives an inherited string. The one
saved charset copy on an essence transition preserves the original algorithm's
run state; a later explicit same-essence charset does not overwrite that state.
There is no new production limit, rejected declaration, shared-string abstraction,
candidate filtering, header mutation or change to Fetch MIME semantics.

The new adversarial semantic regression asserts the independently constructed
charset, MIME essence, exact admitted field order/bytes and response equality.
Existing and expanded literal cases cover A/B/absent carryover, explicit final B,
essence resets, ignored invalid/wildcard candidates and cross-occurrence quoting
and escapes. The semantic regression also passes against the preserved prior
source; output correctness by itself is not an allocation guard.

`net/tests/mime_extraction_allocations.rs` is a separate one-test Cargo integration
binary. A test-only System allocator counts successful allocation/reallocation
request sizes on the measuring thread, only around production response construction.
Inputs, setup and assertions are outside measurement. Its 2,052,096-byte envelope
is a test guard, not a production input limit. It measures cumulative requested
allocation bytes, not peak/live storage, exact allocator copying or elapsed time.
It neither measures other threads nor claims general networking performance.

### Request-purpose fixtures

The fragment-free replacement history test now captures the issued request and
asserts Hyperlink purpose after link activation. Its response helper takes explicit
purpose and receives the captured address's kind for that response. The helper
checks retained purpose at Start and after completion/publication. Address-bar,
refresh, back and forward traversal requests retain explicit Document assertions.
The cross-document addressing test constructs response facts directly from the
captured issued address and checks outgoing and retained Hyperlink purpose, also
after DOM publication. No purpose is inferred from Html execution or MIME.
These corrections are confined to fixtures/assertions; production admission,
attachment, invalidation and document lifetime behavior are unchanged.

### Corrected-candidate commands and results

The following executions used the corrected working tree on 2026-11-20. Detailed
command/exit records and complete logs are retained under
`/private/tmp/borrowser-ai8-review-corrections/`, separately from the unchanged
historical records under `/private/tmp/borrowser-ai8-validation/`. All final
automated regressions use local fixtures, with no public internet dependency.

| Exact command | Actual result |
| --- | --- |
| `cargo test -p net --test response_metadata --locked` | 9 passed, including the new adversarial semantic regression and expanded explicit-final-charset expectations. |
| `cargo test -p net --test mime_extraction_allocations --locked --offline -- --nocapture` | 1 passed; 497,964 cumulative requested allocation bytes, below the 2,052,096-byte guard. |
| `cargo test -p browser --lib navigation_history_back_from_fragment_free_replacement_never_relabels_new_document --locked` | Exactly 1 passed; 379 filtered. |
| `cargo test -p browser --lib document_addressing_fragment_cross_document_navigation_replaces_address_and_associations --locked` | Exactly 1 passed; 379 filtered. |
| `cargo test -p browser --lib navigation_history_fragments_share_document_and_refresh_repopulates_the_group --locked` | Exactly 1 passed; 379 filtered, including explicit Document purpose for refresh. |
| `cargo test -p net --locked` | Rebuilt final execution: 274 unit/integration tests and 1 doctest passed; no ignored or filtered tests. |
| `cargo test -p browser --lib --locked` | 380 passed; no ignored or filtered tests. |
| `cargo test -p browser --lib --features conformance-observation --locked` | 390 passed; no ignored or filtered tests. |
| `cargo test -p runtime_net --lib --locked` | Socket-enabled final execution: 5 passed; no ignored or filtered tests. |
| `make fmt-check` | Passed (`cargo fmt --all -- --check`). |
| `make lint` | Passed (`cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings`). |
| `make lint-html5` | Passed (`cargo clippy -j 4 --workspace --all-targets --features html5 --locked -- -D warnings`). |
| `git diff --check` and `git diff --cached --check` | Passed; original index retained. |

Cargo discovered and executed both new tests; the allocation probe is an automatic
integration-test target, not an unregistered helper. The final net log contains
both test names. Both complete Browser logs contain the two corrected scenarios,
the refresh scenario, and the existing AI8 CSS acceptance/completion/retention tests.
The 19-case CSS matrix still traverses commands, assembly and installation. No CSS
acceptance policy changed during these focused corrections.

An isolated negative control copied the preserved staged candidate with
`git checkout-index --all --prefix=/private/tmp/borrowser-ai8-review-corrections/prior-candidate/`
and included the identical allocation probe and adversarial semantic regression.
Its production Content-Type source
was checked byte-for-byte against the saved pre-correction source. From that
temporary workspace the exact control commands were:

```sh
CARGO_TARGET_DIR=/private/tmp/borrowser-ai8-review-corrections/prior-target CARGO_BUILD_JOBS=2 cargo test -p net --test mime_extraction_allocations --locked --offline -- --nocapture
CARGO_TARGET_DIR=/private/tmp/borrowser-ai8-review-corrections/prior-target CARGO_BUILD_JOBS=2 cargo test -p net --test response_metadata ai8_large_run_charset_survives_eight_thousand_later_candidates --locked --offline
```

The allocation control exited 101 as expected: 0 passed / 1 failed, requesting
512,321,964 bytes against the same guard. The semantic control passed exactly
1 test (8 filtered). The measured difference is 511,824,000 requested bytes,
matching the removed inherited-charset copies for this input. This is specific
allocation-work evidence, not a throughput benchmark or general memory bound.

Failed/intermediate attempts are retained, not relabeled as final passes:

- The first control builds shared the main Cargo target directory. After those
  builds, the first `cargo test -p net --locked` reused the prior-source allocation
  binary and exited 101 on that guard with the old 512,321,964-byte measurement.
  The control was rebuilt in the separate target directory above, and
  `cargo clean -p net` removed net's cached artifacts in the main worktree. A fresh
  complete net compilation/execution then passed, and the rebuilt standalone
  measurement returned 497,964 bytes. No production source was swapped, no index
  entry changed, and no test assertion or gate was weakened to resolve this setup
  error. The isolated and rebuilt executions above are the evidence used here.
- The first runtime_net execution inside the restricted sandbox exited 101:
  3 passed / 2 failed at local socket binds with PermissionDenied. The same command
  with loopback permission passed all 5 tests. Net and Browser full regression
  lanes ran with loopback permission. The existing vendor `ureq` parentheses
  warning remains visible in logs; vendor code and lint gates were not changed.

### Architecture review and qualification boundary

The corrected candidate still matches the approved architecture and scope. The
current CSS caller justifies central metadata cached with immutable ResponseFacts;
there is no separate metadata wrapper. The allocation fix keeps all selection
state local, using owned strings and one final move. The allocator probe is isolated
test instrumentation, not a production allocation abstraction or a generic test
framework. Fixture purpose comes from the issued address or explicit checked
context. Fetch, Addressing, Browser, CSS, Layout and Paint boundaries remain intact.
No shortcuts, new routes, placeholder loaders, speculative policy objects, changed
framing/decoding/completion semantics or outstanding code corrections were found.
The feature-gap tracker already describes this working-tree implementation and
the remaining networking responsibilities; these corrections do not change gaps.

The earlier GUI observation, release/build/qualification runs, full-workspace runs
and aggregate CI/base-native reproduction above were not repeated for these focused
corrections. They remain historical. In particular, aggregate local CI previously
failed on nine native Chromium checks reproduced on the base. That result remains
a failure; no corrected-candidate full-CI pass is established or inferred from it.
No new visual behavior is introduced here; deterministic Browser and runtime
regressions provide the current integration evidence. A new GUI observation and
aggregate qualification remain unperformed for this corrected candidate.

This candidate is ready for human review, with the qualification limitation above.
Committed-candidate qualification, hosted CI and issue closeability are not claimed.

**Historical Git handoff — 2026-11-20:**

The original 42-file staged candidate is preserved; focused corrections and the
new allocation test remain outside the index. The branch is
`feat/1128-response-metadata-extraction-and-loader-handoff`. HEAD, its base, local
master and origin/master are `cc65bc134f21c31e3e34937132082c583b2de4de`; a fresh
GitHub ref lookup confirms the same live master (0 ahead / 0 behind), with no
published feature-branch ref. No commit, push, PR mutation, merge,
issue closure, milestone closure, new issue or repo-local AI workflow file was made.
