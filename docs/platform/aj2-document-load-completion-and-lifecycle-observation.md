# AJ2: Document Load Completion, Typed Failures, And Lifecycle Observation

> AJ4 refinement: [document character decoding](aj4-document-character-decoding.md) selects the declared/default UTF-8 subset before activation and transports typed terminal decoder evidence. Silent cancellation, unavailable interruption evidence, and explicit BOM/meta deferral remain qualified limitations. Earlier charset-selection gaps below describe their historical checkpoints.

> AJ3 current qualification: [AJ3](aj3-main-document-response-preparation.md) refines response admission and concurrent navigation lifetime. Active body/parser/stylesheet settlement remains independent of a pending replacement. Pending failures and 204/205 outcomes have their own issued-attempt terminal; the active first-failure latch is preserved. A settled active frame remains observable while a replacement waits. Earlier negative response fixtures and eligibility qualifications below are historical AJ2 scope, superseded only by AJ3's explicit subset.

Issue: [#1133](https://github.com/joris97jansen/borrowser/issues/1133).
Milestone AJ. Implementation base: `fe8a154545ec126bc32a8c48900be30474d5ad2a`.
This describes the implementation candidate; validation and review qualification
are recorded separately. It does not close AJ or establish complete loading.

## Authority and request context

[AJ1](aj1-static-document-loading-and-page-assembly-contract.md) remains the
architecture contract. This change extends Browser's existing lifetime, not a
second loader, request wrapper, state machine, assembled-page object, or renderer.

`Tab::prepare_document_request` constructs AI2's admitted `FetchRequest` from
AH's `ResourceAddress`. Its initiator URL and requesting origin capture actual
source context when available, including hyperlink activation. Missing context
is not guessed. Resource purpose, Html execution route, and `HistoryTarget`'s
append/existing action are different facts with existing consumers. Refresh and
history traversal need no new stored load-kind discriminator. Viewport inputs
remain at AC frame execution. Static/no-JS is the supported capability; it is
not a new mode selector and grants no script scheduling or execution.

Foundations: AH4 document/frozen-base lifetime, AH5 origins, AH6 issuance and
retirement; AI2 requests, AI3 responses, AI4 fake/shared acquisition, AI5 body
integrity, AI6 redirects, AI7 failures, AI8 metadata; AE parser/publication
identity; AF stylesheet order and computed-style ownership; AC accepted frame
and artifact validity; AG5 production observations. Their semantic owners and
existing qualifications remain unchanged.

## Accepted body completion and terminal ordering

`DocumentLoadState::body_completion: Option<net::BodyProgress>` retains one new
fact: Browser accepted a matching main-document `NetworkDone`. Admission of the
immutable response, delivered bytes, body completion, accepted parser completion,
and committed DOM remain distinguishable.

Acceptance requires the existing response snapshot equality, Html route, and
delivered-byte accounting checks. `Some` is installed before attempting
`ParseHtmlDone`, and only once. It is not an acknowledgement that the parser
received EOF, a complete-representation verdict, or HTTP applicability policy.
Existing ignored downstream command-send errors remain a limitation; missing
parser completion never becomes successful settlement.

| Event | Browser outcome |
| --- | --- |
| First valid NetworkDone | Retain completion and attempt one ParseHtmlDone. |
| First invalid NetworkDone | Typed fatal failure, without EOF. |
| Chunk, Done or network error after accepted Done | Inert; no extra bytes, EOF, provenance replacement or reversal of the network terminal. |
| Parser success before accepted body completion | Typed ParserCompletionBeforeBody failure and existing publication invariant error. |
| Parser failure before EOF | Retain owning typed parser failure and perform existing cleanup. |
| Valid parser completion after EOF | Require current committed handle/version/mode/DOM before Complete. |
| Duplicate successful parser completion | Preserve existing invariant rejection and fatal-failure behavior. |
| Parser error after Complete | Preserve existing fatal parser-error behavior. |
| Any later failure after Failed | Preserve the first authoritative cause. |
| Older-generation or unmatched route/slot events | Existing admission guards remain authoritative. |

Navigation's existing generation replacement resets completion/failure evidence.
Failed admission/enqueue does not replace the current load. Same-document
changes, intermediate publications and frozen-base changes do not clear it.

## Typed failure and provenance

The existing `DocumentParseStatus::Failed` now carries `DocumentLoadFailure`.
There is no second failure flag or separately synchronized optional cause.
Browser wraps existing `NetworkErrorKind`, `HtmlParseFailure`, document creation
and `DocumentPublicationFailure` payloads. Browser-only variants identify the
actual admission/input invariants previously reduced to status strings.

Network failures retain available `NetworkResponseInfo` and `BodyProgress`.
Contextual acquired response facts remain in the failure payload; they never
populate `DocumentLoadState.response` as an admitted document response. Before
initial acquisition these facts can be absent. After a failed redirect-target
acquisition, context can describe the acquired redirect source and entered
history, never an invented final response at the failed destination.

Original/final response URLs and redirect history remain owned by immutable AI
facts. Committed document URL/origin and fallback/selected frozen base remain in
`DocumentAddressState`. No URL copies, origin regeneration, guessed final URL or
request-relative stylesheet-base fallback are introduced.

`LoadStateInvariantViolation` retains a typed relationship category and optional
offending slot identity and slot/request counts at failure for investigation. Its
categories are only those checked by the current query, not a generalized error
taxonomy. Context-free failure Debug and UI text expose the category, slot
presence and captured counts. Tab Debug derives the offending slot's current
document-order ordinal from `DocumentStyleSet`, even beyond the row limit, and
marks whether its row is included or omitted. An identity absent from current
order is reported as such; an unspecified identity is not guessed. The ordinal
is local to that snapshot, not a retained identity or new slot registry.

## Borrowed stylesheet inspection

`DocumentStyleSet` still owns its slot map, order vector and available CSS parses.
`StylesheetLifecycleView` privately borrows one existing slot and its map key.
Its crate-private accessors expose source kind, selected address, and existing
pending/loaded/failed/aborted facts. Private slot types, text and parses do not
escape. The exact-size iterator follows document order without another registry,
parse clone or persistent observation state.

Installed CSS can coexist with
`StylesheetLoadPhase::AwaitingRuntime { progress: BlockReceived, .. }`.
Consequently zero pending slots does not establish runtime settlement. Terminal
external installation also requires the current issued record to be Complete.
Failed/aborted slots retain identity and order but supply no CSS parse. Sibling
requests stay independent. Retirement removes installation authority; late
events cannot recreate it. Eligible completed work may still finish for a failed
document's retained preview under the existing guards.

## Derived settlement and coherent boundaries

The tab-private query returns `Result<StaticInputSettlement, DocumentLoadFailure>`.
It performs no mutation, I/O, decoding, CSS computation, installation or rendering.

Precedence:

1. Latched failure returns Failed and preserves its cause.
2. Contradictory current body/commit/slot/request relationships return an invariant
   error, even if ordinary work also remains pending.
3. The initial state with no accepted load/document/work is Idle.
4. Known unfinished document or stylesheet network/runtime work is Pending.
5. Quiescent work with unavailable required evidence is EvidenceUnavailable.
6. Otherwise supported current inputs are SettledInputsOnly.

Normal missing response/body facts during an active load are Pending. Parser
Complete without accepted body completion is an invariant violation. Missing or
invalid external stylesheet response-base provenance, permitted by existing CSS
admission, is unavailable evidence; usable CSS is retained and ordinary preview
rendering remains available. Completed slot rows classify their retained response
base as parsed, missing or invalid. Snapshot-wide missing/invalid counts also
explain limitations beyond the 64 displayed rows. These classifications inspect
`StylesheetLoadPhase::Complete { response }` without parsing URLs again. Deferred
imports and unrelated image/font/script work do not create new stylesheet waits.

The full consistency query runs only after coherent operations. Discovery first
creates all pending slots, then admits/issues or settles every request. Its
failure branches no longer update loading halfway through that transaction; the
existing publication boundary evaluates once after discovery finishes. No
in-progress flag is needed. CSS handlers likewise finish related slot/request
updates before an operational evaluation.

`update_loading` consumes the query. Pending shows loading; Idle, Failed,
EvidenceUnavailable and SettledInputsOnly do not. Unavailable evidence retains
an incomplete status, rather than a success claim or endless spinner. Invariant
errors latch through the existing fatal cleanup. Progress/status formatting
reuses that call-local result and cannot overwrite a fatal/incomplete status.
An individual CSS runtime completion reports "Stylesheet completed" while
supported input work remains pending, even when every CSS parse is installed.
"Stylesheet work settled" requires SettledInputsOnly from that same evaluation;
zero pending attachment slots is insufficient. Recoverable CSS failure messages
retain their separate path.

## Rendering and applicability qualification

SettledInputsOnly proves accepted response/body/parser/committed-document facts
and terminal current supported stylesheet work. It proves neither complete
response eligibility nor complete AJ1 assembly. Every lifecycle snapshot states:

```text
response_installation_eligibility: not-established
full_page_assembly: not-certified
```

`render_observed_frame` rejects pending/unavailable/failed inputs using its
existing IncompleteDocument error; invariant errors become InvalidArtifacts.
SettledInputsOnly allows the existing rendering attempt. All actual frame,
unchanged identity, follow-up/discard and current retained-artifact checks remain.
An accepted frame proves rendering of current committed inputs, not that their
HTTP response was eligible to install. Normal preview rendering has no new
settlement gate.

HTTP status/MIME response policy, 204/205 active-document preservation, and 304/206
applicability remain AJ3 #1134. Controlled negative fixtures explicitly retain
the uncertified qualification even when those current consumer paths become
mechanically quiescent. They do not bless current replacement behavior. No new
response-policy variant, sniffing, decoding, stylesheet selection/import, script,
security, navigation-history or event-loop behavior is implemented here.

## Deterministic safe diagnostics and cost

Tab's Debug uses a private bounded lifecycle formatter. No structured public
settlement API is added. Load state, parser status and fatal failure use explicit
safe Debug formatting. Arbitrary nested parser/backend messages, including
PatchValidation strings, never enter diagnostic or UI text. Owning typed payloads
remain available internally. UI failure text contains fixed bounded categories.

Snapshots use fixed field order, current document-order sheet ordinals, structural
provenance classifications and explicit missing facts. They emit at most 64 sheet
rows and 16 KiB, including the omitted count. No raw URL/header/body/charset text,
pointer, elapsed time, hash-map iteration or raw parsed CSS is serialized.
Admitted and contextual failure-response summaries separately show up to 20
ordered redirect statuses from typed redirect hops, with an omitted count.
Retained network failure progress is projected as route and delivered-byte count,
or explicitly absent. Missing acquired responses remain absent, not invented
empty redirect histories. Neither projection includes response or error text.
Truncation never limits the settlement audit: omitted dependencies can still
produce Pending or an invariant error. Observations cannot request repaint,
enqueue commands, change identity/generations or mutate retained artifacts.

One query visits S slots with expected O(1) issued-record lookups and O(1)
additional working memory. Matching-record counts detect orphans without nested
searches. It reparses no URLs and reconstructs no retrieval keys. Snapshots add
at most 64 formatted rows. Across P publications and K terminal sheet updates,
worst-case work is O((P + K)S); this is not a linear-total-loading claim. No query
is added to streamed chunks, ignored duplicate terminals or normal preview frames.
Test-only visit/call accounting qualifies these bounds without a production cache.
Debug formatting additionally makes one linear read-only pass over authoritative
slots for unavailable-base counts and failure correlation, formatting at most 64
rows. It adds no operational query or retained synchronization state.

## Future readiness correspondence

Fetching/parsing facts are evidence relevant to future loading. Accepted parser
completion is relevant to future interactive. Supported input settlement is only
part of future complete requirements. Rendering and fatal failure are independent
dimensions. No Document.readyState, DOMContentLoaded, load event or browser event
loop is implemented or implied.

## Regression and qualification boundary

AJ2 tests exercise production Browser events, AI's shared fake Fetch path, existing
CSS installation, preview rendering and feature-gated accepted-frame observation.
They cover ordering/terminal guards, contextual redirects, coherent dispatch,
failure isolation, unavailable provenance, invariant errors, output bounds,
redaction, purity and traversal frequency. Success fixtures deliver known bytes
and NetworkDone through event admission rather than writing completion fields.
Test results, source identity, host limits and CI qualification belong to the
implementation review packet; compilation alone does not establish closeability.

## Initial candidate validation (2026-11-22)

These are fresh local results on the initial uncommitted AJ2 candidate, before
the diagnostic review corrections, not inherited baseline passes. That candidate's
changed Browser source/fixture fingerprint is
`969609e923cb522f313758296396969a7cafb8a703fb02b5bca90773eaccf7f4`:
SHA-256 over sorted changed Browser relative paths, NUL, file bytes, NUL. Docs
are excluded from that fingerprint. The feature branch starts at the base above.

| Validation | Actual result |
| --- | --- |
| Targeted `cargo test -p browser --lib aj2_ --features conformance-observation --locked` | 17 passed. The final full suites also execute these cases. |
| Final `cargo test -p browser --locked` | 396 unit and 5 integration tests passed, no failures/ignores. |
| Final Browser suite with `--features conformance-observation` | 407 unit and 5 integration tests passed, no failures/ignores. |
| `cargo test -p runtime_parse -p runtime_css -p runtime_net --locked` | 37 unit/integration tests passed. |
| Required AG CI/addressing/networking suites | 7 / 22 / 16 pass, with 0 / 2 / 2 explicitly unsupported cases; no failures/errors. |
| Formatting and three workspace Clippy configurations | Passed. Browser observation-feature all-target Clippy also passed. |
| Dependency qualification | Integrity checks and 246 qualification tests passed. |
| Parser conformance, external fixtures, fatal failures, DOM/patch goldens, real pages, rawtext regressions, tokenizer/tree/pipeline fuzz smoke and WPT tree builder | All independently passed after aggregate CI stopped. |
| Debug/HTML5/release/release-HTML5 builds, release network probe, HTML/CSS bench compilation, entity generation check | Passed. Release warnings remain in unchanged HTML/CSS/runtime-parser code. |
| Full `make ci` | Failed at workspace tests: nine native macOS Chromium ownership tests; not a green aggregate. |
| Complete default and HTML5 workspace tests with `--no-fail-fast` | Each failed only the conformance binary target with the same nine failures; other targets passed. |

The nine failures were reproduced on a clean detached checkout of exact base
`fe8a154545ec126bc32a8c48900be30474d5ad2a` using
`cargo test -p borrowser-conformance --bin borrowser-conformance native_ --locked`.
The base selected run reported 5 pass, 9 fail and 2 existing ignores. The failed
cases are native capture diagnostics, cleanup diagnostics, lifecycle, macOS BSD
ownership, process/pipe, resource completion/shutdown, shutdown deadline, startup
exit classification and temporary artifact containment. Diagnostics include live
process executable unavailability during ownership scanning and cleanup timeouts.
Matching base failures establish an inherited host/platform qualification issue;
they neither waive it nor make the candidate's CI aggregate successful.

Both existing deterministic local-file and redirected HTTP/CSS/image runtime
smoke tests passed, including actual accepted frames. `make run` launched and its
initial window was inspected. The subsequent local-page visual attempt could not
be qualified because window captures became black; no manual page-rendering pass
is claimed. Automated production-path evidence remains separate from that limit.

The implementation is left for owner review without staging, commit, push or
issue closure. Full-CI/platform qualification and manual visual limits remain
explicit; Milestone AJ and AJ3 response-policy gaps are not closed.

## Diagnostic review correction validation

The subsequent diagnostic corrections retain the same base and operational
lifecycle. The changed Browser source/fixture fingerprint, using the definition
above, is `074430d70cff2d87127f0a2cc02a8aca3eab20c341240c9ba43ecc6c757f6644`.
The original idle snapshot fixture remains unchanged.

| Fresh validation after correction | Actual result |
| --- | --- |
| Targeted AJ2 tests with conformance observation | 19 passed, no failures or ignores. |
| `cargo test -p browser --locked` | 398 unit and 5 integration tests passed, no failures or ignores. |
| `cargo test -p browser --features conformance-observation --locked` | 409 unit and 5 integration tests passed, no failures or ignores. |
| `cargo fmt --all -- --check` | Passed. |
| Browser all-target Clippy with conformance observation and `-D warnings` | Passed. |

These tests compare equivalent request maps inserted in opposite orders, retain
distinct document-order slot phases, explain unavailable bases and correlate
invariant failures beyond the row limit before and after failure latching. They
also exercise two- and twenty-hop contextual redirects, absent/pre-acquisition
and partial-body progress, nested/pretty redaction, output bounds, observational
purity, query frequency, usable CSS, ordinary previews and checked frames.

Full CI was not rerun for these diagnostic corrections. Its earlier nine
baseline-matching native Chromium ownership failures remain unresolved, not
waived or converted to a passing result. The prior manual visual qualification
limit also remains. AG native tests and their assertions were not changed.

## Stylesheet status correction validation

The final targeted status correction reuses the existing settlement result;
it adds no state or additional operational query. Its changed Browser
source/fixture fingerprint, using the definition above, is
`64177937952c29b24977ce24df21b4496a101e7a4e5e5a0c758c0a5cf8e60283`.

The production-path regression first failed on the prior code: with two installed
CSS parses, the first runtime completion falsely reported settled work while the
second remained AwaitingRuntime/BlockReceived. After the correction, the first
completion keeps loading active and reports only that stylesheet's completion;
the second establishes SettledInputsOnly and clears loading. Each handler still
performs exactly one settlement evaluation. Fatal and unavailable-evidence guards
and recoverable failure messages are unchanged.

| Fresh validation after status correction | Actual result |
| --- | --- |
| Targeted AJ2 tests with conformance observation | 20 passed, no failures or ignores. |
| `cargo test -p browser --locked` | 399 unit and 5 integration tests passed, no failures or ignores. |
| `cargo test -p browser --features conformance-observation --locked` | 410 unit and 5 integration tests passed, no failures or ignores. |
| `cargo fmt --all -- --check` | Passed. |
| Browser all-target Clippy with conformance observation and `-D warnings` | Passed. |
| `git diff --check` | Passed. |

Full CI was not rerun. The earlier nine baseline-reproduced native Chromium
ownership failures and manual visual qualification limit remain unresolved.
These results do not certify response applicability, complete assembly or
Milestone AJ completion.
