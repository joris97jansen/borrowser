# AJ7 — Deterministic stylesheet acquisition, retained evidence and recoverable failures

Issue #1138, Milestone AJ. Local implementation contract; publication and hosted
qualification remain independent gates. Candidate-specific receipts live in the
external review packet, not in this historical contract.

## Ownership and publication

Browser owns dependency order, attachments, acquisition applicability, recovery,
retention and settlement. AH owns URLs; AI owns request/response construction,
redirects, acquisition, coding and transport limits; CSS owns parsing/cascade.
AJ6 is an independent passive authored-input observation, never loading authority.

Production retains its HTML namespace, ordinary-child traversal (no template
contents), rel token matching, `str::trim` href preprocessing and selection subset.
Missing, empty, whitespace-only, invalid and unsupported hrefs now occupy ordered
failed attachment positions. They cannot install CSS or fabricate a request.
Duplicate URLs have independent owner/slot identities. Completion order does not
change cascade order. Unchanged owner/source retains the frozen selected address,
AH diagnostics/failure, terminal result and reservations across base changes;
media changes do not refetch. Changed source/replacement gets a new monotonic ID.

`DocumentStyleSet::prepare_reconcile` borrows old records and iteratively examines
the candidate DOM. It checks count and borrowed source lengths before copying,
uses fallible bounded storage, prepares retained/new slots, destination map/order,
CSS projection, retirements and dispatch IDs only. Retained unchanged media uses
Keep; changed media owns a prepared replacement. No selected address is cloned
into a dispatch descriptor. After commit, preflight borrows the authoritative
frozen address; owned request input is constructed only after metadata admission.
Commit moves retained slots,
drops retired owners and installs prepared accounting. Tab publishes DOM,
addressing, handle and version through the existing publication seam, with DOM
invalidation before stylesheet invalidation and dispatch after publication.
Preparation failure never publishes speculative state or dispatches requests.
CSS parse/projection clones and inherited DOM/AH allocations are not certified
fallible or allocation-free by the destination reservation.

Inventory failure latches `DocumentLoadFailure::StylesheetInventory` before
cleanup and returns public `DocumentPublicationFailure::ResourceUnavailable`.
The previous coherent preview remains. This path builds neither a new cleanup
collection nor eager status text. Presentation formats the typed cause later.
Unrelated publication failures retain their established detailed status behavior.
The first fatal failure wins. Existing generation cancellation remains fatal
cleanup; it is never recovery from one stylesheet failure.

## Acquisition and event admission

Attachments own terminal `StylesheetFetchResult`; the existing live map alone
admits events. Fetching owns actual request, exact issued retrieval key, immutable
selected response, policy and raw Vec. Successful queue send establishes issuance,
not worker execution or transmission. After validated Done, original Vec ownership
moves to the attachment. Ready and installation success are independent.
The existing settlement query checks source payload and independent pool totals,
no stable provisional credit, issued request/frozen source/selected response
relationships, classified policy, raw-body completeness and exact CSS progress.
Live Fetching cannot carry installed base or premature bytes. AwaitingRuntime
checks its exact cleanup key and block/parse relationship. Installation transitions
require coherent Ready evidence. All current rows are validated before diagnostic
formatting limits apply; fatal document failure retains its existing precedence.


Start authenticates tab/generation/slot/CSS route, stylesheet purpose, complete
original address (including fragment) and exact fragmentless issued key. Chunk
has only route/key/phase identity; it cannot supply response identity. Done must
match the whole selected immutable response and producer CSS body progress exactly.
Wrong route/tab/generation/retired or already terminal events are ignored. A
contradiction inside admitted Fetching becomes a local protocol failure; duplicate
Start cannot replace facts. Runtime block/Done authenticate live AwaitingRuntime,
Ready issued evidence and exact issued key. They cannot create acquisition facts.

A pre-Start Error may legitimately carry redirect/admission/coding context and
absent or zero progress. Its context is not selected final provenance. Present nonzero/wrong-route
pre-Start progress is a local protocol failure and is omitted. Selected response facts cannot be replaced by Error
context. Conflicting context/progress has explicit omission; the producer error
kind survives separately. Failure precedence is wrong key, conflicting response,
progress route/count contradiction in either phase, then producer failure.
Optional response and progress facts are validated independently: a higher-priority
contradiction never admits invalid lower-priority evidence. Wrong issued keys
discard all event-supplied optional facts; selected facts remain immutable. Transport failure takes
precedence over a provisional response-policy rejection because completion failed.
Policy rejection takes precedence over installation, which is never attempted.

Only complete Ready input is handed to the existing CSS runtime. One fallible
body copy plus prebuilt chunk/Done/abort URL copies precede sends. Empty Ready sends
an empty CssChunk and CssDone. Copy/send/runtime failure preserves Ready bytes and
records independent installation failure. CssAbort only cleans an existing runtime
buffer; it is best effort, never per-resource HTTP cancellation. Before Done no
runtime buffer exists. Loaded after decoded block remains lifecycle-pending until
authenticated CssSheetDone. Completed acquisition/runtime work may finish on a
failed preview; uncompleted network work is terminally aborted. Retirement and
navigation remove admission and release owned records.

## Response subset

| Facts | Acquisition applicability after validated completion |
| --- | --- |
| HTTP 200, acceptable MIME, complete usable final provenance | Ready, including zero bytes |
| Other 2xx complete representation | Ready except exclusions below |
| 204/205 | Recoverable no-body rejection; Null evidence |
| 206 | Unsupported unsolicited partial representation |
| 226 | Unsupported instance/delta representation |
| 304 | Rejected unmerged cache representation; Null evidence |
| Other 3xx/4xx/5xx | Typed rejection; their CSS bodies cannot install |
| Missing/invalid final URL | Typed provenance rejection; never a fallback base |
| HTTP request with non-HTTP final URL or absent HTTP status | Provenance rejection |
| Local file request with usable file final URL and no HTTP status | Apply same MIME subset; complete zero bytes is Ready |
| MIME Missing / Empty / parsed text/css | Accepted compatibility subset |
| MIME Unusable / parsed other essence | Rejected |

All available policy dimensions survive; primary rejection is provenance, status,
then MIME. AI null-body behavior precedes Browser handling; malformed null framing
can produce transport failure instead of policy rejection. Content-Length is not
used to fabricate delivered bytes or require equality for null statuses. Redirects
retain original request/key/history; installed external base is admitted parsed
final response provenance, separately cloned by DocumentStyleSet. Requested URL is
not the resource base and these observations are not a new CSSOM API.

Normative context: [HTML stylesheet links](https://html.spec.whatwg.org/multipage/links.html#link-type-stylesheet),
[Fetch](https://fetch.spec.whatwg.org/#concept-main-fetch),
[CSS Syntax byte input](https://drafts.csswg.org/css-syntax-3/#input-byte-stream),
[HTTP semantics](https://www.rfc-editor.org/rfc/rfc9110.html), and
[HTTP caching](https://www.rfc-editor.org/rfc/rfc9111.html).
These policies are a Borrowser supported subset, not full standard conformance.
Missing/Empty MIME compatibility is preserved. AJ7 adds no sniffing, nosniff,
quirks-dependent cross-origin MIME/security enforcement, charset decoding, cache,
range merging or delta reconstruction. Charset and other metadata are preserved
without introducing AJ8 decoding policy.

## Evidence, limits and accounting

Body evidence distinguishes absent (unconstructed/unissued), not-started, null,
complete (including empty), legitimate partial prefix and deliberately unavailable.
Complete HTTP/MIME-rejected bodies remain retained. Network/resource/protocol
failure retains the legitimate prefix, observed count and unretained-event count
where available, with collection-stop/omission facts. Premature, wrong-key or
nonempty null-body chunks are rejected before byte admission; their separate
discarded-event count/reason does not masquerade as observed legitimate input. It never fabricates missing
request, response, progress, final URL or bytes. AI continues to enforce 4 MiB
encoded and 2 MiB delivered stylesheet limits.

| Document logical guardrail | Admission/lifetime |
| --- | --- |
| 4,096 attachments | Inline/external, including source failures; check before next owned candidate |
| 65,537 ordinary nodes including root | Iterative borrowed traversal; bounded fallible frame growth |
| 8 MiB authored/AH payload | href/inline/media UTF-8 lengths, selected URL, AH failure base and diagnostics |
| 64 MiB body pool, 2 MiB per issued attachment | 32 issued owners, full credit until actual retirement/replacement |
| Independent 8 MiB fetch-metadata pool, 256 KiB per owner | 32 variable fetch-evidence owners, including constructed but not enqueued terminals |

Canonical dispatch checks body capacity, metadata pool, preconstruction charge,
request admission, provisional body reservation, live-map growth and queue send.
A failed pre-issuance operation rolls back body credit. No actual retained request
means release metadata; constructed-not-enqueued evidence retains metadata without
body credit. Issued credit is never refunded on success/empty/rejection/transport
or runtime failure. No completion-driven retry, eviction or re-admission occurs.
Identical accepted publication/retirement history gives deterministic admission;
that history itself may depend on external events. A 33rd tiny stylesheet can fail
despite low delivered usage. Metadata exhaustion independently rejects later
attachments after 32 retained constructed-but-not-enqueued outcomes.

Logical source charge S is input href/inline/media lengths plus selected address
URL, successful AH diagnostics (`len * size_of(UrlDiagnostic)`), or failure base
URL and its diagnostics. Checked sums precede authored copies. AH resolution is
bounded by admitted input/base lengths; delegated AH result construction may
allocate before its actual retained output is charged. Authoritative overflow is
fatal instead of publishing a truncated inventory.

For fetch metadata, A = complete original URL length; I = initiator URL length;
O = 0 for opaque origin, otherwise conservative I+32 tuple-origin allowance.
Q = request original/current URL lengths + request header name/value lengths + I+O.
R = response original/requested/reported/parsed URL or failure-base lengths and
AH diagnostics + header name/value lengths + parsed MIME/charset lengths + all
three URL lengths per redirect hop + conservative 8 KiB private local MIME allowance.
Charge is Q + 2A + R + B, where B separately charges installed response-base AH
outcome/failure. The two A allowances cover exact key and live abort URL. Response
facts are immutable Arc-shared and counted once. Fixed enums/counters/cardinality
are bounded by attachment/map limits. Preliminary request charge is 4A+I+O+9
for HTTP default Accept (no 9 for local files). All sums use checked arithmetic;
request/response charge must fit a full independent metadata credit before retention.
A defensive post-construction over-budget request is dropped with
ConstructedOmitted(MetadataLimit), fixed RequestMetadataLimit failure and no
request/key/body ownership. Metadata credit is released; recoverable denial must
not manufacture a ledger contradiction.

These are logical retained payload bounds, not total heap or process bounds.
Allocator capacities/overhead, map buckets, AH/backend internals, candidate DOM,
old/new publication overlap, transient request/event clones, command
channels, runtime buffers, the one handoff body copy, decoded strings, CSS parse
and flattened projection clones, renderer artifacts and inherited infallible
allocations remain outside the claim. Fallible growth/refusal at qualified engine
boundaries produces typed recoverable per-sheet failures or fatal inventory failure;
process-wide OOM recovery is not promised.

Preparation refusal tests distinguish simulated refusal from actual allocator null
returns immediately at qualified fallible growth. They cover traversal, owner
lookup, entries, source, order, destination, projection, retirement and dispatch,
including later media/source and final projection growth. They preserve old
owners, body allocation and ledgers before the intended fatal transition. The
isolated fatal-cleanup test counts zero caller-thread engine allocations with no
channel; this does not qualify channel/worker or process-wide allocation behavior.


64 MiB permits 32 issued owners; 128 MiB would permit 64, 256 MiB 128. The independent
metadata pool still permits32 variable metadata owners, so raising body capacity
alone would not increase the typical effective issued count. Larger pools commit
more logical capacity and can admit more small sheets only with a separately
reviewed metadata policy. No representative real-site distribution evidence was
collected. Wide 64/2,048-owner regression fixtures establish storage/lookup behavior,
not real-site compatibility. These bounds are intentional supported-subset
policies, never standard browser limits.

## Observation and validation

`ParsedDocumentObservation::stylesheet_fetches` uses the existing committed-state
coherence gate. Feature-gated source-bearing views borrow ordered attachments and
results, exact authored/AH source outcomes and applicable live request/selected
response/policy/prefix/count facts; they never load, mutate, copy bodies or repair
state. Live prefixes do not imply terminal completion. Ordinary snapshot output
is bounded to 16 KiB, 64 rows, 32 redirect statuses and 1,024-byte row scratch,
with 512 bytes reserved for its footer. Row acceptance commits counters atomically.
Redirects omitted with discarded rows and statuses omitted by the redirect cap
are counted separately; exact limiter flags describe the actual accepted output.
No committed document, complete empty inventory, inconsistent unavailable evidence
and formatting truncation remain distinct. Unavailable inventory
and successful empty bodies remain distinct. Debug output omits private URLs,
headers, bodies, arbitrary error strings and implementation identities.

AJ7 tests distinguish real production-issued FakeNetwork requests from direct
fault-injected CoreEvents. The local runtime fixture uses real file acquisition,
parser/CSS workers, recoverable missing CSS, empty CSS and checked-frame acceptance.
A loopback HTTP runtime fixture additionally proves 404 CSS rejection with a
successful sibling, empty CSS, exact retained rejected body, effective blue style
and checked-frame acceptance. Targeted default/observation debug/release inventories and runs, affected crate
suites, networking/addressing/extended lanes, L3 local/redirected fixtures and local
make ci are required candidate evidence. Original failures remain visible. Hosted
exact-candidate/integration qualification is later and separately authorized.

AWS is not required: AG execution rejects external subresource acquisition and its
projections cannot qualify internal AJ7 evidence contracts. Resource-backed Chromium
assembly conformance remains AJ10. No cloud operations are part of AJ7 qualification.
AJ8 decoding/parsing/static-set, AJ9 assembly/rendering, AJ10 conformance, broader
selection/media/import/security semantics and HTTP cancellation remain deferred.

## AJ8 downstream qualification

[AJ8](aj8-ordered-static-stylesheet-set.md) refines downstream behavior: complete
Ready bytes select a canonical CSS encoding; UTF-8 runtime handoff is one complete
Chunk plus Done/Abort. Checked outcomes and actual terminal receipts remain distinct
from acquisition/installation. Unsupported effective encoding rejects recoverably
without installation. Excluded hosts begin NotRequested without reservations;
prepared first eligibility dispatches at most once. Existing status/MIME/provenance,
body admission, stale-event protection, accounting and first-fatal precedence remain.
The corrected API is `prepare_reconcile_with_environment`; commit still only moves
prepared owners. Historical AJ7 results are not rewritten.
