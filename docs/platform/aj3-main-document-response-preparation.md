# AJ3: Main Document Response Preparation

Issue: [#1134](https://github.com/joris97jansen/borrowser/issues/1134).
Milestone AJ. Implementation base: `efdcc3ca07825fff667225dcb3243ef3d9e5528c`.
This contract refines [AJ1](aj1-static-document-loading-and-page-assembly-contract.md)
and [AJ2](aj2-document-load-completion-and-lifecycle-observation.md). It does not
establish full navigation, character decoding, page assembly, or AJ completion.

## Ownership and existing path

Browser owns document applicability, navigation/history intent, active document
lifetime and parser handoff. AI owns typed request admission, HTTP acquisition,
redirect execution, immutable response facts, metadata interpretation, body
integrity, limits and transport failures. AH owns URL/origin semantics. AE owns
parsing and DOM semantics. CSS and AC retain style and retained-render ownership.

The path remains ResourceAddress → FetchRequest → CoreCommand::FetchStream →
runtime_net → net::fetch_stream → NetEvent/CoreEvent → Browser admission →
existing ParseHtml commands → AE publication → Browser commit. Fake networking
uses the same Fetch lifecycle. No additional loader, request model, resource
registry, complete-body buffer or navigation framework is introduced.

## Request identity and submission

The private Tab reservation counter uses checked addition. Every reservation is
consumed, including failed request construction/enqueue. Invalid URL input and
same-document actions that require no reservation issue no request ID. No reset,
wrapping, cancellation, history operation, terminal-record removal or document
replacement can decrease the counter. Exhaustion is a typed submission rejection.

There is no writable `Tab.nav_gen`. `active_document_request_id()` borrows its
answer from the actual installed FetchRequest, or returns None. The request field
is private to the state owner. Activation is its sole installation operation;
there is no arbitrary ID setter or counter synchronization operation. Request IDs
retain their existing per-tab navigation-generation meaning for subresources.

Navigation methods return Result<(), NavigationError>. Ok denotes an accepted
submission or same-document action, never network/parser completion. Unsupported
schemes, credentials, route admission and absent/disconnected command queues
retain their real typed cause. Status text is only a presentation. A rejected
submission neither fails the active document nor supersedes an existing pending
request; no NetworkErrorKind is fabricated for an unexecuted Fetch.

Pre-dispatch rejection returns its original NavigationError to the caller. It
never overwrites active-document progress/failure or an issued attempt's status.
Every existing submission entry also projects that result into one bounded,
redacted `navigation_notice` for the existing page status display. This is UI
presentation only: it owns no request, response, terminal or typed failure record
and participates in no loading, event-admission or frame-validity decision.
The separate string is needed to show rejected C alongside settled A and pending
B without attributing C to either lifecycle. Retained terminal attempts cannot
suppress a later notice, including on an otherwise empty tab.

The notice is replaced by a later rejection and cleared by the next successful
submission/action, including a successful no-op. Network/parser events from an
older submission cannot erase newer feedback. Shell toolbar and page-link UI
entries use this same report boundary; discarding their returned Result does not
lose feedback or create a second UI failure registry. The typed result remains
unchanged for non-UI callers. `visible_load_status()` composes the separately
labeled submission notice, actual pending/terminal attempt and active-document
progress/failure. The notice is not part of deterministic load-state authority.

## Pending navigation and active document

A bounded NavigationAttempt retains the actual issued request, existing
Append/Existing history intent, optional non-replacing response and one terminal
Result<BodyProgress, DocumentLoadFailure>. It owns no DOM, parser, stylesheet set,
render artifact or second page. Terminal retention is diagnostic, not allocation.

Successful fetch enqueue supersedes only an earlier pending navigation. The actual
active document, document URL/origin/frozen base, committed history association,
input, parser, stylesheet/runtime work, images and retained artifacts remain live.
The toolbar can display the pending destination; it is not document authority.

For an eligible HTML response all fallible preparation and parser-start command
enqueue precede replacement effects. Exclusive mutable Tab access makes this
commit coherent with event delivery. Only then are the old generation and CSS
buffers retired, deferred history applied, old input/DOM/render structures reset,
and the pending request, response and prepared addresses installed. A valid AE
publication subsequently makes the new DOM authoritative under existing atomic
handle/version/mode/base rules. Failure after activation retains the new load's
permitted preview; it does not restore the old page.

Browser-side parser-command enqueue failure is typed. Queue acceptance is not
parser initialization or downstream router delivery acknowledgement. Existing
unacknowledged downstream channel forwarding remains a qualification limit.

204/205 never activate a document. Their selected response is retained until a
matching zero-byte Html completion or typed failure. They send no parser input or
EOF and alter no committed history/document identity. Completion restores the
committed toolbar URL. When no active request was ever installed, presentation
says "no document installed" rather than claiming to preserve an active document.
A failed pending navigation also preserves any existing active page. No snapshot-based reconstruction or rollback of request IDs occurs.

Refresh and cross-document history traversal defer the existing history action.
Same-document actions cancel pending replacement before updating their own
history/document URL. Disabled traversal stays a no-op. Supersession/cancellation
and tab closure cancel the correct request generations; late events cannot
resurrect retired work. Closing a tab retires CSS buffers and cancels both active
and distinct pending requests.

Cancellation retains the selected 204/205 response context, if acquired, but
leaves body progress absent: a response-start event is not transfer progress or
zero-body completion evidence. Only an actual matching NetworkDone supplies
that completion. Cancellation remains a typed Cancelled terminal for diagnostics;
later completion cannot overwrite it. The presentation omits Cancelled attempts
from navigation-failure notices, so successful same-document navigation does not
display an error for the fetch it intentionally cancelled. Other terminal
failures remain visible and retain their first cause.

## Cancellation provenance audit

The current production Browser/Fetch path has one cancellation cause: an explicit
Browser command affecting that request's cancellation token. This justifies
omitting its Cancelled terminal from failure overlays without another cancellation
flag, reason enum or registry. This is an audited producer invariant, not a claim
about arbitrary injected CoreEvents or future independent abort producers.

- `net/src/fetch.rs` emits Cancelled at local-file acquisition, HTTP acquisition
  and redirect checkpoints, or the post-Start body checkpoint, only after reading
  the caller's token as true. `net/src/stream.rs` does the same for body streaming
  and 205 empty-body verification. Local-file body loading uses that streamer.
- `net/src/backend.rs` does not classify transport errors as Cancelled. Connection,
  timeout, TLS, read and framing failures retain their own kinds; Interrupted
  reads are retried. Fake acquisition fixtures also reject Cancelled as an
  arbitrary configured network failure; tests trigger the real token path.
- `runtime_net/src/lib.rs` creates a false token per `(TabId, RequestId)` and its
  only true write handles `CoreCommand::CancelRequest`. The platform router only
  forwards that command; it does not create cancellation errors on shutdown or
  failed command delivery.
- Browser's `cancel_pending_navigation()` latches the existing typed Cancelled
  terminal before issuing CancelRequest for same-document navigation/traversal,
  supersession or tab retirement. `fail_navigation_attempt()` similarly latches
  its actual first failure before cancelling unfinished transfer work. Late AI
  cancellation cannot replace that cause. Supersession installs a distinct
  monotonically reserved request, so the old event becomes stale.
- Active-document replacement, fatal active failure and tab closure can cancel
  the active generation. Its ID cannot alias the distinct pending generation;
  tab closure also latches/cancels the pending attempt before removing the tab.

Thus a production cancellation event for pending work is either already terminal
or stale when Browser accepts events again. Expected cancellation remains typed
and diagnostic; cleanup after a real preparation/network failure cannot conceal
that original failure. A deterministic fake-Fetch barrier regression checks real
AI cancellation events after fragment navigation, traversal, supersession,
retirement and response rejection. Existing AI tests qualify acquisition,
redirect, streaming and null-body token checkpoints. Any future producer of
independent cancellation must refine this contract and presentation before it
can use the same suppression rule.

## Supported response policy

Request purpose, exact original issuance and selected URL provenance are checked
before applicability. An absent/invalid final URL gains no requested-URL fallback.
AI retains actual redirect transitions. Failed target acquisition may retain the
last acquired source response, never an invented final response at that target.

Disposition is considered before ordinary status/MIME selection:

| AI disposition classification | Browser decision |
| --- | --- |
| Missing / Inline / Unusable | Continue through status and MIME policy. Unusable remains observable metadata. |
| Attachment / Other valid type | DownloadUnsupported document preparation failure; preserve the active document and cancel the pending request. No download success is claimed. |

This follows [HTML's separate attachment path](https://html.spec.whatwg.org/multipage/browsing-the-web.html#populating-a-session-history-entry)
and [RFC 6266's invalid-field default](https://www.rfc-editor.org/rfc/rfc6266.html#section-3).
AI's type-only extraction accepts one ASCII disposition token with HTTP OWS and
optional opaque parameter suffix. It does not parse/decode filenames or validate
complete parameter grammar. Empty/malformed/ambiguous/repeated type declarations are
Unusable rather than transport failures. A valid type token with an opaque
parameter suffix is classified by that type; parameter grammar is not validated. No first/last occurrence or speculative
recovery is selected. Raw fields remain immutable and ordered.

| Response | Preparation |
| --- | --- |
| 204 / 205 | Await matching empty-body completion; preserve active document. |
| 304 | NotModifiedWithoutRepresentation; no cache exists to supply a representation. |
| 206 | PartialRepresentationUnsupported. |
| 226 | InstanceManipulationUnsupported; no reconstruction/negotiation exists. |
| Other selected 2xx / 3xx / 4xx / 5xx | Apply the same MIME gate, including 404/500 and unknown valid codes. AI owns redirect following and selected-response rules. |
| Informational/invalid final HTTP status | AI admission failure. |
| Local file | No HTTP status/fields fabricated; apply the MIME gate to local-loader facts. |

The MIME gate admits only Parsed `text/html`. Missing, Empty, Unusable and other
parsed essences have distinct typed rejection outcomes. Parameters and charset
facts remain retained metadata; they do not select a character encoding. This
strict declared-HTML subset deliberately does not claim MIME sniffing or a
non-HTML viewer. An eligible empty HTML transfer sends the existing parser EOF;
AE constructs its result without fabricated markup.

## Completion, failures and observation

Response eligibility, delivered bytes, accepted body completion, parser completion,
committed DOM, stylesheet/runtime settlement and accepted rendering remain
separate. AI's 20 MiB encoded / 10 MiB delivered document guards remain the only
production size enforcement. Browser checks existing immutable completion facts
and progress; it adds no HTTP framing, redirect or byte-limit implementation.

AJ2 terminal and first-failure latches remain. Pending failures live only in the
attempt terminal; active failures live only in DocumentParseStatus::Failed.
Failing one owner cannot cancel or erase the other. Only an activated request can
publish DOM or accept its subresources. Active work remains authoritative while
replacement is pending and becomes stale only after actual replacement.

Tab.loading and supported-input settlement describe active inputs only. UI busy
presentation additionally derives pending-navigation progress without another
stored flag. A pending navigation alone does not invalidate an accepted settled
active-document frame. Such a frame proves rendering of that active document,
not success of the pending navigation. Existing AC identity/artifact/frame checks
remain mandatory. Ordinary preview rendering remains available under AJ2 rules.

Diagnostics separately project active request/input/failure facts and pending,
failed or non-replacing navigation outcomes. They retain fixed field order,
redacted URL/header/charset/filename contents, bounded redirect statuses, 64 sheet
rows and a 16 KiB envelope. Complete typed provenance remains internally retained.
Observations never schedule, install, regenerate origins or repair state.

A successful non-replacing terminal reports `preserved-active-document` only
when an installed active request exists; an empty tab instead reports
`completed-without-document`, consistent with the UI's "no document installed".
Both projections derive from the installed request without retaining another
outcome or changing loading, navigation or retained rendering state.

## Regression and qualification boundary

Production-path Browser fixtures exercise AI fake networking and existing parser
and CSS runtimes. Required scenarios include HTML/error documents, redirects,
empty input, unsupported schemes/MIME, network/body/limit failures, 204/205,
attachment/Unusable disposition, monotonic IDs, parser enqueue failure, active
work/frame preservation and deterministic diagnostics. Existing AI tests retain
compression and exact encoded/decoded boundary qualification; Browser adds only
distinct propagation/EOF invariants and no compression dependency.

Required validation follows targeted, subsystem/feature, runtime smoke and full CI
levels. Candidate results and limitations must be reported for the reviewed
source; historical passes or baseline failures are not candidate passes.

## Remaining platform gaps

MIME sniffing, full charset determination/decoding, download execution/filenames,
complete disposition recovery, range/delta/cache reconstruction, full session
history/BFCache, JS/events, cookie/security policies and full browser navigation
remain unsupported. AJ3 does not close stylesheet assembly or Milestone AJ.

## Candidate validation (2026-11-23)

These results record the initial uncommitted AJ3 candidate on
`feat/1134-fetch-and-prepare-main-document-response`, based on
`efdcc3ca07825fff667225dcb3243ef3d9e5528c`. They predate the review corrections
below and do not qualify those subsequent changes or turn inherited failures
into passing candidate evidence.

| Lane | Actual result |
| --- | --- |
| Targeted Browser `aj3_` tests with conformance observation | 17 passed, including actual Fetch/parser paths, allocator exhaustion, input/focus/frozen-base preservation and parser-command failures. |
| Browser default features | 414 unit tests and 5 integration tests passed. |
| Browser conformance-observation features | 425 unit tests and 5 integration tests passed. |
| Fetch and runtime_net | 279 Fetch unit/integration tests and 5 runtime tests passed. Existing compressed-body and exact limit boundaries remain AI-owned tests. |
| Required AG CI / addressing / networking suites | 7 / 22 / 16 passed; 0 / 2 / 2 explicitly unsupported. No harness errors. |
| Formatting, default / HTML5 / hardening lint, dependency qualification and generated entities | Passed. |
| Default and HTML5 complete workspace runs (`--no-fail-fast`) | Failed only the conformance binary target: 218 passed, 9 failed, 13 ignored in that target. Every other executed target passed. |
| Full `make ci` | Failed at workspace tests with the same nine native macOS Chromium ownership failures. Aggregate CI is not green. |
| Remaining CI parser/fuzz lanes | Parser conformance, external fixtures, fatal failures, DOM/patch goldens, real pages, rawtext regressions, six tokenizer/tree/pipeline fuzz smoke lanes, pipeline regressions and WPT tree builder passed. |
| Builds and bench compilation | Default/HTML5 debug and release builds and HTML/CSS bench compilation passed. Release builds retain warnings in unchanged HTML/CSS/runtime_parse source; warning-free release qualification is not claimed. |
| Release networking process qualification | Passed intentional abort control, 30 failure, 2 persistence, 2 closed-sink and 2 image-consumer processes. |
| Runtime smoke | `make run` reached the running Browser without a fatal error before intentional interruption. The deterministic local file/stylesheet/image/frame smoke passed. Manual visual navigation was not qualified. |

The native failures were reproduced in a clean detached checkout of this exact
base: 1 passed, 9 failed, 1 ignored under `chromium::tests::native_` (229 filtered).
The temporary checkout was verified clean and removed. The nine failing wrappers
were capture-failure diagnostics, cleanup diagnostics, lifecycle regressions,
macOS BSD ownership states, process/pipe contract, resource completion/shutdown,
shutdown deadline, startup exit classification and temporary artifact containment.
Their failures include the existing ownership scan report
`live process executable unavailable during ownership scan`. Reproduction
establishes an inherited host/platform qualification gap, not a candidate pass.
No ownership assertions were weakened or ignored to qualify AJ3.

Issue-specific behavior and ownership are ready for owner review. Complete green
aggregate CI and manual visual navigation remain unqualified. This record does
not close #1134 or Milestone AJ.

## Review correction qualification (2026-11-23)

This records the preceding correction review; the subsequent presentation edge
case qualification below covers that review stage. Three ownership/presentation
corrections refined the initial candidate:

- Cancelling after acquired 204/205 headers retains the immutable response
  context and `body_progress: None`. Acquisition cannot supply AI-owned progress
  or completion evidence. A late zero-body completion cannot replace cancellation.
- Pre-dispatch rejection returns the original NavigationError and preserves the
  active document's progress/failure and the actual issued attempt's status.
  Idle rejection notices are not attributed to a pending or active document.
- Typed Cancelled attempt terminals remain diagnostic evidence and are omitted
  from navigation-failure overlays. Actual non-cancellation failures remain
  visible and retain their first cause. No cancellation discriminator/cache was
  added; active identity, allocator and event admission are unchanged.

The 204/205, A/B/rejected-C and fragment cancellation regressions demonstrated
these findings before the correction and passed afterward. The fragment fixture
also checks derived fallback URL behavior under AH, frozen base/origin/history
association, actual DOM/handle/version, retained AC artifacts, accepted-frame
observation and inert late network/parser publications.

| Correction validation | Actual result |
| --- | --- |
| Targeted `aj3_` / `aj2_` Browser tests, conformance-observation feature | 20 / 19 passed. |
| Browser default / observation features | 417 / 428 unit tests and 5 integration tests each passed. |
| Fetch / runtime_net | 279 / 5 tests and the net doctest passed. No Fetch source or body-limit changes. |
| Required AG CI / addressing / networking | 7 / 22 / 16 passed; 0 / 2 / 2 explicitly unsupported, no harness errors. |
| Formatting, default / HTML5 / hardening Clippy and workspace all-target build | Passed. |
| Conformance test target | 218 passed, 9 failed, 13 ignored; the same native macOS ownership wrappers as the initial base-reproduced qualification gap. |
| Runtime smoke | `make run` reached the running corrected Browser without fatal output, then was intentionally interrupted. Deterministic local-resource/frame smoke passed in Browser tests; manual visual navigation was not qualified. |

Standalone conformance initially encountered obsolete Fetch build artifacts
missing the source's disposition API. Clearing only generated `net` artifacts
and rebuilding resolved that build failure without source or guard changes.
Both initial failed attempts and the fresh results are retained in correction
validation logs. Aggregate `make ci` and the full workspace lanes were not rerun
for these focused corrections; their initial failed qualification remains
historical, not a pass for this source. No assertions, existing fixtures or
native test failures were weakened or ignored. The inherited CI gap remains
unresolved, and the corrected issue remains ready for owner review.

## Navigation presentation qualification (2026-11-23)

These results predate the P2 diagnostic and regression corrections recorded below.

This review extends rejected-submission feedback to active documents, pending
navigations and empty tabs retaining terminal outcomes, while preserving the
previous ownership/progress/cancellation corrections. One presentation-only
optional string is the only new stored value; it never becomes a navigation or
document failure registry. No NavigationError variant or submission API changed.

The current status display independently identifies the rejected submission,
actual attempt outcome and active document. A 204/205 completion without an
installed active request reports "no document installed". The source audit above
and real fake-Fetch cancellation events verify that production Cancelled outcomes
follow Browser intent/first terminal or request retirement; no new cancellation
abstraction or suppression heuristic was added.

| Latest validation | Actual result |
| --- | --- |
| Targeted AJ3 / AJ2 tests with conformance observation | 24 / 19 passed. |
| Production pointer activation fixture | Passed both admitted and rejected hyperlink cases through actual page input handling. |
| Browser default / observation features | 421 / 432 unit tests, plus 5 integration tests each, passed. |
| Fetch / runtime_net | 279 / 5 tests, plus the net doctest, passed. |
| Required AG CI / addressing / networking | 7 / 22 / 16 passed; 0 / 2 / 2 explicitly unsupported, no harness errors. |
| Formatting, default / HTML5 / hardening Clippy, workspace all-target build | Passed. |
| Native conformance test target | 218 passed, 9 failed, 13 ignored; the same native macOS ownership wrappers remain unqualified. |
| Runtime smoke | `make run` reached the running updated Browser without fatal output and was deliberately interrupted. Manual visual navigation was not qualified. |

Regression coverage includes settled A and pending B with rejected C, rejection
without pending replacement, idle tabs retaining success/failure outcomes,
redacted/bounded feedback and its dismissal on successful action, and shell-drawn
text even when the caller does not retain its Result. An accepted active-document
frame remains valid with a notice present. A callback barrier checks actual AI
cancellation after fragment navigation, same-document traversal, supersession,
tab retirement and response rejection; first causes and late-event guards remain
intact. The existing fixtures/assertions were extended for the new feedback
requirement without weakening load/event/identity admission.

Full `make ci` and complete workspace lanes were not rerun for this focused
presentation change. Their earlier failures and exact-base reproduction remain
historical qualification, not passes for this source. The fresh native target
still exhibits the same nine ownership failures. The index and prior staged
implementation were preserved; these updates remain unstaged for owner review.

## P2 diagnostic and regression qualification (2026-11-23)

The non-replacing terminal diagnostic now distinguishes an installed active
request from successful completion on an empty tab. Both 204/205 regressions
assert exact diagnostic wording, consistent UI status, redaction, bounded and
repeatable output, and unchanged load/navigation/rendering state and artifacts.
No new production state or lifecycle authority was introduced.

The real-runtime partial-body regression now requires actual HTML admission,
the issued active request ID, a single parser start and exactly the delivered
`abc` input. Its active failure must retain MalformedResponse, the actual selected
response and three delivered HTML bytes. A pending failure cannot satisfy it;
EOF remains absent and late completion cannot replace the first failure.
Existing CSS and image branch guarantees remain unchanged.

| Fresh corrected-source validation | Actual result |
| --- | --- |
| Focused active/empty 204/205 and real-runtime partial-body regressions | All three passed with conformance observation. |
| Targeted AJ3 / AJ2 tests with conformance observation | 24 / 19 passed. |
| Browser default / observation features | 421 / 432 unit tests, plus 5 integration tests each, passed. |
| Fetch / runtime_net | 279 / 5 tests, plus the net doctest, passed. |
| Required AG CI / addressing / networking | 7 / 22 / 16 passed; 0 / 2 / 2 explicitly unsupported, no harness errors. |
| Formatting, default / HTML5 / hardening workspace Clippy, observation-feature Browser Clippy, workspace all-target build | Passed. |
| Full conformance test target | 218 passed, 9 failed, 13 ignored. The failing wrapper set exactly matches the recorded base-native ownership failures; the ownership-scan diagnostic remains present. |

The first focused run exposed an incomplete new UI expectation; it was corrected
to include the existing active-document summary without changing production
status behavior. Final focused runs passed before broader validation.

Aggregate `make ci`, complete workspace tests and a GUI smoke were not rerun for
this observation-only production change. Real loopback Fetch/runtime and Browser
parser/local-resource/frame tests ran freshly. Earlier aggregate and GUI results
remain historical; green full CI and manual visual navigation are not claimed.
The original staged index remains unchanged and these four-file corrections
remain unstaged for owner review. No assertions or admission guards were weakened.
