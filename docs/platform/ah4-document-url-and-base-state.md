# AH4: Document URL And Frozen Document Base

Date: 2026-10-30
Status: implemented; review pending
Scope: Browser-managed UTF-8 documents, parser-created DOM publication, existing
resource consumers, and internal diagnostics. No new fetching capability.

## Ownership and state

Browser `PageState` owns optional `DocumentAddressState`. Absence means there is
no managed document address; it does not mean an implicit `about:blank` or a
guessed URL. Standalone HTML parsing remains independent of URL context.

`DocumentAddressState` holds an immutable `BrowserUrl` document URL and an
optional selected parser key/frozen-base pair. For the supported creation
context the fallback is derived from the document URL. The active base is the
selected frozen URL, or the fallback when no candidate applies. There is no
independently synchronized active/fallback URL cache.

Requested/response provenance stays in `DocumentLoadState.response` and
`NetworkResponseInfo`; toolbar/history state stays in `Tab`. None of these is
replaced by a base element. Origins are not implemented or inferred by AH4.

`addressing` continues to own parsing, explicit-base resolution, normalized URL
values, and typed diagnostics. Browser calls `BrowserUrl::resolve()` and
`scheme()` and owns HTML selection, freezing, prohibited schemes, and outcomes.
There is no new addressing API, backend, dependency, public DOM API, or policy
framework.

## Lifetime

| Boundary | Effect |
| --- | --- |
| New tab / new navigation | Retire old stylesheet attachments, then clear document address, DOM, and owner associations. Pending toolbar input is not a document address. |
| Accepted initial HTML response start | Parse the effective response address before starting the parser. Use nonempty `final_url`, otherwise the existing requested-URL compatibility fallback. Invalid nonempty final addresses do not fall back. |
| Accepted publication | Publish DOM, document/base state, handle and version coherently, then discover resources. |
| New handle / same-handle `Clear` | Reset selection and all resource attachments, including when local node keys recur. Keep the current load's document URL. |
| Rejected / stale publication | Preserve committed DOM, document/base state, and attachment ownership. A rejected publication can fail the load and cancel unfinished stylesheet work; stale publications do not. |
| Parser failure after preview | Retain the last committed preview, its document identity, and its addressing state. |
| Same-document navigation / applicable back-forward | Update document URL and derived fallback, retaining an unchanged selected frozen base. |
| Completion / duplicate response start | Do not reinitialize the committed address state. |

## HTML base rules

Select the first HTML `base` element with an unnamespaced `href` in ordinary
document tree order, including descendants outside the head. A target-only base
does not win href selection. Foreign lookalikes do not qualify; HTML descendants
at foreign integration points can qualify. Ordinary traversal does not cross
AE10 template-content associations. Raw `HeadMetadata::base_href` remains
head metadata and does not select the active base.

Resolve the selected href against the fallback, never a previous active base.
Absolute, relative, empty, and valueless hrefs use the existing URL resolver.
Recoverable diagnostics accompany success. Invalid-input failure and `data:` or
`javascript:` results freeze the fallback and record a distinct Browser outcome.
An invalid first candidate does not authorize selecting a later candidate.
Capacity/backend failures remain implementation failures and reject publication;
they are not invalid-authored-input verdicts.

Becoming the first candidate or changing its href freezes a result. Removing
the selected candidate or its href, insertion, or reordering can select another.
An atomic move is not a fabricated detach/reinsert pair. Unrelated attributes
and text do not refreeze a candidate. Changing the document URL does not
refreeze an existing selected element, including one that froze an old fallback.

Normative references checked on 2026-10-30:
[HTML document bases](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-urls),
[HTML base element](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element),
[URL validation errors](https://url.spec.whatwg.org/#validation-error).

## Ordered application and atomic publication

Browser's existing strict arena applier emits private ordered
`BaseElementChange` facts. They describe canonical names, href values,
connectivity, and tree order; they contain no URL semantics. The existing public
`DomStore::apply()` remains available and shares the same structural applier.

The observed path receives the committed selected key. It observes operations
on the staged arena, records `Clear` boundaries and selection/href transitions,
and folds those facts into staged address state. Existing version, mode,
materialization, identity-resolution, and mutation-detail validation completes
before the Browser commit. No resource command is emitted before acceptance.

Final-tree scanning is not the temporal model. Transient candidate transitions
survive within a batch. The fixed fallback and UTF-8 context within one
publication permit processing recorded facts after structural validation without
changing frozen results. Equivalent ordered streams yield equivalent transitions
under valid publication partitioning. Static HTML with the same creation context
has the same final base across input chunking.

Observation inspects the affected subtree before insertion/movement and carries
only its first candidate key across the arena mutation. A new/disconnected
subtree cannot contain the selected connected base. In a connected subtree
containing the globally first base, that base must also be the subtree's first
candidate. This invariant detects selected-subtree moves without climbing the
selected candidate's ancestry. Atomic movement preserves the subtree's internal
order. Actual candidate connectedness is checked after the successful operation.
Base-free insertion and movement need no ancestry or sibling-order comparison.
Removal uses the same first-candidate invariant.

The original observation did perform a selected-ancestry walk for every ordinary
insertion, adding `O(P*D)` work for P unrelated inserts and selected depth D. Full
scan/subtree counters alone did not measure that work. The corrected observer
has test-only ancestry-visit and sibling-comparison counters as well. Repeated
base-free insertions and moves at selected depths 16 and 256 perform zero of both.
Tests for 1,000/10,000-element wide documents, actual candidate comparisons,
disconnected candidates, selected-subtree movement and partitioning remain active.

Additional observation work is `O(P + A + S + C + K*N)`: patches, inspected
attributes, affected subtrees, actual-candidate connectedness/tree-order work,
and K full reselections of N live nodes. For base-free inserts/moves C and K are
zero; a real candidate can still need ancestry paths and a sibling search.
Repeated actual displacement can still cause repeated scans. Existing arena
cycle checks, cloning, materialization, vector operations and debug invariant
checks have their own costs; this is not a whole-applier linearity claim. No
persistent base-observation index, second DOM, or generic observer is introduced.

## Resource associations and retirement

Discovery retains the existing accepted-publication boundary. A new or changed
source resolves against that committed active base. An unchanged existing
association retains its selected URL, even if the active base changes.

Images retain exact authored source and optional typed selected URL by live
document-local owner ID. Failed resolution is retained while the same source
remains applicable. The next association map contains only applicable current
owners: removal, source removal and subtree deletion retire entries. A new owner
with the same text resolves anew. Atomic same-document moves retain unchanged
associations. `Clear` and replacement reset the entire map.

Layout's existing image provider receives owner ID plus exact source. Missing,
failed, or source-mismatched associations return no URL; rendering never repairs
them against the current base. Network errors keep their selected resource key.
`ImagePresentation` carries the selected URL into intrinsic-size lookup and paint.
Frame preparation borrows the association map instead of copying it per frame.

Stylesheet slots retain owner identity, authored href, selected URL, media,
slot identity and parse state. Reorder updates cascade order; media-only changes
retain the slot and loaded parse. Source changes retire the old attachment;
removed/nonqualifying elements disappear. Failed resolution produces no fetch
and is not retried merely because the base changes. Duplicate URLs remain
distinct attachments. Loaded contributions and response metadata retire with
their attachments.

The existing checked stylesheet counter remains monotonic for the PageState
lifetime, including `Clear` and handle replacement. Retired slots are never
reused by new owners. Browser sends the existing `CssAbort` command for retired
external attachments before discarding their ownership state. This includes
element/source retirement, accepted document replacement, navigation or refresh,
history traversal that starts a new fetch, and explicit tab removal. Navigation
and tab removal use the old request generation and slot ID. The CSS runtime owns
the corresponding partial buffers and removes them on `CssAbort`; `CancelRequest`
does not by itself clear the CSS runtime. Repeating an abort for a retired slot
is harmless, and a different generation or slot cannot be affected. Late CSS
events cannot recreate response metadata or install into retired attachments.
Existing request-generation and slot-state checks remain in force.
Requested/response URL provenance is preserved.

Parser or document failure cancels unfinished external transfers in the retained
preview and aborts their partial buffers. Their owning elements and selected URLs
remain associated, while completed styles remain available. A transfer whose
network completion was already accepted may still deliver its queued CSS runtime
result to the preview. Later network events for the failed load cannot restart
buffering, and a cancelled transfer cannot install a result.

Stylesheet records have one owning `HashMap<StylesheetSlotId, StylesheetSlot>`.
A separate ordered ID sequence represents current document/cascade order; IDs
are map keys and are not also stored in each record. This replaces the owning
slot vector and does not add a mirrored live-slot registry. Reconciliation builds
both views together, and retirement/Clear remove records and ordering entries.
All ordered output, including loaded stylesheet projection and fetch retirement,
uses the ID sequence rather than hash iteration. Monotonic IDs remain independent
of document order.

The original `has_external_slot` and mutable slot lookup scanned S records per
event. C streamed chunks could therefore incur `O(C*S)` membership work. Both
point lookups now use the owning map (expected `O(1)` each, expected `O(C)` for
chunk membership); hash-table worst-case bounds are not claimed constant. The
existing response metadata map cannot supply this attachment-lifetime authority
because it is absent before response start and removed at completion. Test-only
counters measure actual point-map lookup operations, not hash probe comparisons.
Tests with 64/2,048 attachments and 4,096 live plus 4,096 retired chunks require
one point lookup per chunk and reject retired traffic. CSS decode/aggregation,
stylesheet ordering and pending counts retain their existing separate costs;
this does not claim constant-time stylesheet assembly or reconciliation.

Image cache identity remains ResourceManager's selected-URL identity. Retiring
an owner neither retargets an outstanding request nor evicts assets shared by
other owners. Existing cache retention/eviction policy is unchanged. An image
still loading when its old generation is cancelled can remain marked `Loading`;
a replacement document selecting the same URL can reuse that entry without a new
request while old-generation events are rejected. AH4 does not fix this inherited
cancelled-image retry limitation.

## Hyperlinks

Browser passes no base through the existing GFX viewport API. Its existing
passthrough returns authored `PageAction::Navigate` intent. Browser resolves that
reference against the active base, then navigates without address-bar guessing.
Relative and fragment references can therefore target another document when a
base supplies another address. Unsupported absolute schemes are rejected
explicitly instead of receiving a guessed HTTPS prefix. Missing address state
does not supply a synthetic base. Existing supported navigation machinery and
request-generation lifecycle remain in use. Same-document comparisons use the
retained typed document URL, independently of editable toolbar text.

For hyperlink fragment navigation, the resolved destination must have a
non-null fragment and match the document URL with fragments excluded. An empty
fragment (`#`) is non-null; an absent fragment is not. This comparison includes
scheme, username, password, host, port, path and query, using the existing
normalized `BrowserUrl` components. It is URL equivalence, not a same-origin
test. An empty href is first resolved against the active base; if the resolved
destination lacks a fragment, it does not take the fragment-navigation shortcut.
The regular cross-document lifecycle clears the prior document and owner
associations before an accepted response establishes the next document URL and
fallback. A genuine same-document fragment navigation retains the DOM, resource
associations and unchanged selected frozen base while updating document URL and
history.

Browser history stores each entry's URL and an optional opaque document-state
identity together. The identity is independent of the URL, DOM handle and
request generation; matching URLs do not establish document identity. New
same-document fragment entries share the current document's identity. Back and
Forward reuse a document only when the target has that identity and the current
entry has a published DOM and document address. This includes traversal to an
entry without a fragment. Otherwise Browser starts the existing document fetch;
there is no retained-document restoration or back-forward cache.

A pending network request has no active document merely because it has a target
history entry. After a response is accepted, a successfully committed DOM preview
and its document address establish the active document, even while parsing is
still pending. If parsing subsequently fails, Browser retains that committed
preview and its established identity. New fragment navigation and traversal to
another entry with that same identity may therefore reuse it in either state.
An uncommitted response, a failed load without a committed preview, and a
navigation that has reset the page cannot reuse a prior document. Rejected or
stale publications do not create or change the committed preview identity.

A fresh identity is assigned only after a current-generation response passes
the existing lifecycle guards, its requested URL matches the pending entry, and
its effective document URL parses successfully. Pending, cancelled and failed
requests do not make an unavailable document active; stale responses are ignored.
For successful refresh or repopulation, entries sharing the former identity are
rebound to the new identity when the accepted URL matches the target entry's URL
excluding fragments. A substantive redirect instead associates only the target
entry with the new identity and updates that entry to the accepted URL. Existing
response metadata retains requested/final provenance. Rebinding scans the history
vector once and adds no persistent index or document cache.

If response metadata omits a fragment on an otherwise matching URL, the history
entry retains its requested fragment. The active document URL still follows the
existing effective-response URL contract; this does not implement fragment
inheritance through redirects or fragment scrolling. An accepted response without
a published DOM is not eligible for document reuse. A response with no tracked
history entry receives no history association and likewise cannot be reused by
history traversal.

The initial parser publication, same-load `Clear`, and DOM handle changes do not
create navigation identities. They follow the existing DOM and resource
replacement rules within the accepted document load. The identity is a Browser
history association, not a DOM handle, request ID, origin, or proof of complete
WHATWG session-history conformance. AH4 does not add bfcache, History/Navigation
APIs, scroll restoration, persisted state, browsing-context hierarchy, or
navigation policy machinery.

The formerly public mutable `Tab.history` and `Tab.history_index` fields are
now Browser-owned so callers cannot change an entry URL or index independently
of its association. Public read-only `history()` and `history_index()` accessors
replace those fields; `HistoryEntry::url()` exposes each entry's URL. This is a
source-compatibility change for callers of the previous fields.

See [HTML navigation](https://html.spec.whatwg.org/multipage/browsing-the-web.html#navigate)
and [URL equivalence](https://url.spec.whatwg.org/#concept-url-equals).

## Debugging and guarantees

The private document-address snapshot exposes document, fallback and active URLs,
selected candidate/outcome, and ordered diagnostics. URLs use AH3 structural
redaction; exact hrefs, hosts, credentials, paths, queries, fragments, custom
scheme text and sensitive lengths are not formatted. Exact URL assertions occur
only in controlled tests. These snapshots remain internal regression contracts.

The guarantees are separate:

1. Deterministic final document/base state across static input chunking.
2. Ordered supported candidate selection and frozen-base processing.
3. Atomic publication and rollback, including owner associations.
4. Explicit publication-boundary resource discovery and stable constructed
   request associations.
5. No full browser-standard resource loading/scheduling conformance claim.

## Explicit limitations

UTF-8 only, matching the existing decoder. Inherited about-document bases,
document base overrides, CSP policy hooks, `base target` processing, public
dynamic DOM APIs and same-value setter restart semantics are unsupported.
Intermediate resource source changes coalesced within one publication remain
subject to existing discovery, rather than individual setter scheduling.

Image mutation/microtask algorithms, full stylesheet loading triggers,
crossorigin/srcset/picture processing, CSS URL values and stylesheet-location
integration are not implemented by AH4. Same-document moves retain associations
under this bounded model; this is not a full HTML move-triggered loading claim.
See [HTML image processing](https://html.spec.whatwg.org/multipage/images.html#updating-the-image-data)
and [stylesheet processing](https://html.spec.whatwg.org/multipage/links.html#link-type-stylesheet).

No new transport, redirects, navigation history feature, origins, same-origin
policy, resource identity-key model, cache policy, cookies, storage, CORS, CSP,
mixed-content enforcement, service workers, or JavaScript URL APIs are added.
Legacy URL helpers outside the Browser consumer paths remain migration points.
The pre-existing file loader treats the suffix after `file://` as a filesystem
path without reparsing, so file URL queries or fragments can become path text
and fail or identify a different file. The existing network error logger and
some user-visible status paths can format raw URLs, including credentials; AH3
structural debug redaction does not cover those legacy outputs. Neither behavior
is corrected or claimed safe by AH4.

## Regression evidence

Browser tests cover base values and invalid-first selection, canonical
namespace/template boundaries, UTF-8/entity chunk splits, frozen fallback
lifetime, history, unsupported/missing hyperlink contexts, publication rejection,
replacement, association retirement, late responses, intrinsic lookup and debug
redaction. DOM-store tests compare ordered facts across partitions and count
selection work on 1,000/10,000-element documents with thousands of irrelevant
mutations. Existing HTML/CSS/layout/paint/conformance lanes remain regression
coverage; AH4 adds no AG comparison projection or upstream WPT claim.

## Validation on 2026-10-30

| Level | Command / evidence | Result |
| --- | --- | --- |
| Targeted | `cargo test -p browser document_addressing --locked` | 13 matching tests passed; final additional lifetime cases are also covered by full workspace lanes. |
| Targeted | `cargo test -p browser resource_associations --locked` | 9 matching tests passed. |
| Targeted | `cargo test -p browser dom_store::tests::base --locked` | Four initial work/ordering guards passed; the added partitioning guard also passes in full workspace lanes. |
| Subsystem | Browser default/observation suites; formatting; lint; diff check | Passed. Full CI runs all 281 Browser tests in both workspace lanes. |
| Runtime | `make run` | Compiled and reached the native executable without startup errors; intentionally interrupted. No manual visual page inspection. Deterministic production pointer/frame tests cover the changed path. |
| Full CI | `RUST_TEST_THREADS=1 make ci` with host permissions | PASS, exit 0: dependency enforcement, seven AG cases, all lint variants, workspace/HTML5 suites, parser/fatal/golden/real-page/WPT lanes, six fuzz smoke lanes, debug/release builds, bench compilation and entity checks. |

The initial sandboxed CI attempt failed nine native macOS process-ownership
checks (`task_name_for_pid` / Mach access). A host concurrent attempt passed the
workspace lane but observed an extra emergency diagnostic in one native cleanup
scenario in the HTML5 lane. The identical test passed in isolation, then the
complete serial host pipeline passed with unchanged assertions and no new skips.
Release/bench builds retain warnings in untouched HTML/CSS debug-guard code.
Neither infrastructure behavior nor loading-timing support is claimed fixed by AH4.

### Scalability review qualification

The review correction adds the ancestry/sibling counters and owning stylesheet
map described above. The targeted base lane passes all eight tests; the resource
association lane passes all ten tests. The Browser observation-feature lane
passes 285 unit tests and five integration tests. Workspace strict lint passes.

The earlier extra emergency diagnostic remains **unclassified**. The native
cleanup test, launcher, dependency manifests and lockfile are unchanged from
pre-AH4 HEAD `db805553f31fd5cd1e2d234c18a215d7187fab6a`.
An exported copy of that HEAD passed its complete conformance crate suite under
normal host concurrency (186 unit tests passed, 13 existing ignores), three
additional native-test rounds (10 passed, one existing ignore per round), and
eight cleanup-test invocations in two rounds of four concurrent processes. None
reproduced the extra event. Each cleanup invocation retains all six scenarios
and their original assertions.

The unchanged Drop path records one emergency event for each actual cleanup
error. An extra event therefore indicates an additional cleanup error, but the
historical trace does not identify that error. These native subprocess scenarios
do not exercise document addressing or resource associations. This is evidence
against a direct AH4 code-path cause, not proof of a pre-existing environment
defect. No assertion, skip, native infrastructure or test-thread setting was
changed to obtain these results.

The corrected tree passed the complete host gate with normal concurrency:
`env -u RUST_TEST_THREADS CARGO_TARGET_DIR=/private/tmp/ah4-review-current-target make ci`
exited 0. Both workspace lanes passed all 285 Browser unit tests and the native
cleanup diagnostic test. Dependency enforcement, seven AG cases, all strict lint
variants, parser/fatal/golden/real-page/WPT lanes, all six fuzz smoke lanes,
debug/release builds, bench compilation and entity checks passed. Existing
release-only HTML/CSS debug-guard warnings remain outside this change.

An earlier review gate stopped at compilation after the exported baseline build
shared the working tree's Cargo target directory and left an old Layout trait
artifact. The complete successful rerun used a fresh isolated target; no source
workaround or test-thread override was applied. The successful concurrent gate
does not establish a root cause for the historical native diagnostic failure.

### Fragment-navigation correction validation

With the isolated AH4 Cargo target, `cargo test -p browser navigation --locked`
passed six matching tests and `cargo test -p browser document_addressing --locked`
passed 15. The default Browser suite passed 281 unit tests and five integration
tests; the `conformance-observation` suite passed 287 unit tests and five
integration tests. The new cases exercise credential differences, empty versus
absent fragments, retained same-document history, accepted response replacement,
and owner-association lifetime.

`env -u RUST_TEST_THREADS CARGO_TARGET_DIR=/private/tmp/ah4-review-current-target make ci`
passed with exit 0 under normal host concurrency. Both workspace lanes passed
287 Browser unit tests and the native cleanup diagnostic test; strict lint,
dependency enforcement, conformance, parser/golden/WPT lanes, six fuzz smoke
lanes, builds, benches and entity checks passed. Existing release warnings in
untouched HTML, CSS and runtime-parse code remain. A host `make run` startup
smoke reached the native executable and remained running until interrupted; no
manual visual navigation inspection was performed.

### History document-state correction validation

With the isolated AH4 Cargo target, `cargo test -p browser navigation --locked`
passed 16 matching tests and `cargo test -p browser document_addressing --locked`
passed 15. The default Browser suite passed 291 unit tests and five integration
tests; the `conformance-observation` suite passed 297 unit tests and five
integration tests. New regressions cover cross-document replacement and Back,
identical URLs in separate lifetimes, fragment groups, refresh and repopulation,
redirects, forward truncation, pending/failure/stale responses, and resource
association replacement.

`env -u RUST_TEST_THREADS CARGO_TARGET_DIR=/private/tmp/ah4-review-current-target make ci`
passed with exit 0 under normal host concurrency. Both workspace lanes passed
297 Browser unit tests and the native cleanup diagnostic test; dependency and
conformance checks, strict lint, parser/golden/WPT lanes, six fuzz smoke lanes,
builds, benches and entity checks passed. Existing release warnings in untouched
HTML, CSS and runtime-parse code remain. A host `make run` startup smoke reached
the executable and remained running until interrupted; it did not include manual
visual navigation inspection. The historical native diagnostic remains
unclassified.

### Stylesheet retirement and preview qualification

The missing-abort behavior was reproduced against an isolated copy of the
previously staged AH4 implementation. Focused real-runtime tests failed for
navigation, parser failure and tab close because the original stylesheet slot
received no `CssAbort`. The corrected tests pass and use an ordered `CssDone`
event as a barrier: a discarded old key produces no decoded CSS before its
completion event. They cover navigation, refresh, Back, Forward, same-handle
`Clear`, new-handle replacement, both tab-close paths, late events, repeated
abort isolation, parser failure and a new document's successful stylesheet.
They also verify that a network completion accepted before parser failure can
still deliver its queued CSS result, including a rejected-content-type case.

With the isolated AH4 Cargo target, the focused Browser resource-association,
document-lifecycle, shell-close, navigation, document-addressing and rendering
lanes passed 14, 10, 1, 17, 15 and 119 matching tests respectively. The CSS runtime
passed three tests and HTML parser conformance passed its focused test. Browser
default and `conformance-observation` suites passed 299 and 305 unit tests,
respectively, plus five integration tests each. URL dependency enforcement passed.

`env -u RUST_TEST_THREADS CARGO_TARGET_DIR=/private/tmp/ah4-review-current-target make ci`
passed with exit 0 under normal host concurrency. Both workspace lanes passed
305 Browser unit tests and the native cleanup diagnostic test. Strict lint,
dependency, conformance, parser/golden/WPT lanes, six fuzz smoke lanes, builds,
benches and entity checks passed. The first gate attempt stopped on Clippy's
argument-count limit in a new test helper; the helper was corrected without
weakening assertions or suppressing lint before the successful rerun. Existing
release warnings in untouched HTML, CSS and runtime-parse code remain. The host
startup smoke reached the native executable and was intentionally interrupted;
no manual visual inspection was performed. The historical native diagnostic
remains unclassified.
