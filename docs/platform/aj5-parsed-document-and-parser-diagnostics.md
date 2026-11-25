# AJ5: parsed document and recoverable parser evidence

Local implementation candidate for [#1136](https://github.com/joris97jansen/borrowser/issues/1136).
Approved source: `d91d134acfb1ab057f926e2eaf345a51b1bc5c77`, tree
`05c8936ad14c18def67b07a83e6fe4d3a5c448ae`. This contract describes the
candidate; implementation review, validation review, publication, hosted CI,
human merge authority and issue closure remain separate gates.

## Ownership and transport

Fetch owns issued response provenance. HTML owns UTF-8 replacement decoding,
CR/CRLF preprocessing, tokenization, recovery, mode and parser node identities.
Browser materializes and atomically accepts existing DOM/identity/mode/address
publications. AH owns origin and frozen base resolution. No second parser,
document model, position index, address model or observation cache is added.

`runtime_parse_options()` selects disabled legacy diagnostic retention
(`track=false`, `max_stored=0`) and mandatory counting. The runtime-specific
canonical parser constructor installs recoverable capture in the existing event
fanout and ensures mandatory counters are enabled; it does not override arbitrary
callers' separate legacy-retention policy. Ordinary standalone parsing and the existing
conformance capture retain their own policies. Runtime capture requests no
implementation diagnostics, tokens, transitions, unsupported-feature events,
patch history or normalized position checkpoints. Narrow decoder and token
predicates retain the counted decoder and ordinary token drain in production.
Decoder replacements, implementation diagnostics, recoverable HTML errors,
parser fatal errors and Browser publication failures remain separate evidence.

The non-Clone `HtmlParserDiagnosticReport` moves once from terminal parser
extraction through `HtmlParseFinished.diagnostics` to the admitted
`DocumentLoadState`. No Arc, cache or report copies are needed. Private report
fields expose immutable borrowed getters for completion, capture status,
normalized input size, total/retained/dropped counts, exactness and canonical
ordered events. Extraction is one-shot and does not drive the parser or its
patch mirror. Calling runtime extraction on a conformance recorder preserves it.

## Retention, failures and bounded work

Capture starts available with no event storage. Retain the earliest 128 events.
Exactly 128 with no later events is complete; a later omission is truncated.
On the first retained event, reserve exactly 128 pending slots fallibly, once.
At extraction reserve only the retained output length, fallibly, once. A storage
refusal is sticky and never retried. An invalid retained offset or other capture
failure freezes further retention; later events cannot fill unused slots.
Earlier valid rows survive as a contiguous occurrence prefix. Output reservation
refusal exports no rows and performs no scan. Coordinate conversion failure
exports the largest resolvable contiguous occurrence prefix, never later islands.
The first failure and any additional terminal failure remain explicit.

A known normalized offset is validated against the actual current Input length
and UTF-8 boundary at emission. Later appends cannot rehabilitate it. Tree errors
without positions preserve `ParserDidNotProvidePosition`. No original network
byte offsets are fabricated: known positions use normalized UTF-8 byte offsets,
one-based Unicode scalar columns and LF lines, with source-byte evidence
`NoInputProvenanceMap`. At terminal extraction a fixed 128-entry scratch array
is sorted without allocation by offset and occurrence index; at most one scan
of existing normalized Input resolves coordinates. A second allocation-free
sort restores occurrence order. No input copy, per-event scan or production
NormalizedPositionIndex is used. Clean and unavailable-only captures scan zero
scalars. Chunk count does not create diagnostic reservations or copies.

When exact, `total = retained + dropped`, and total is the authoritative parser
counter including unavailable tree events. Counter/occurrence exhaustion marks
counts inexact (saturated lower bounds) and capture incomplete; no wrap occurs.
Capture degradation is observational and cannot turn successful parsing fatal.
Existing parser budgets and fatal-failure policies are unchanged.

## Terminal evidence and lifecycle

| Path | Diagnostic descriptor | Completion/admission |
| --- | --- | --- |
| Initialization/handle allocation fails | Absent | Existing initialization failure |
| Execution/parser finalization fails | PrefixOnly when session exists | Existing failure latches |
| Successful parser finish, later deliverable internal failure | Finalized | Failure still latches; never successful assembly |
| Successful finish/publication/terminal | Present, Finalized | Existing body, decoder EOF, handle/version/mode/DOM checks precede retention |
| Input closes | PrefixOnly | InputClosed; no implicit EOF |
| Cancellation/restart | No terminal extraction | Removed session evidence discarded |
| Known SinkClosed during publication | No extraction or scan | Existing SinkClosed behavior |
| Receiver disappears after last successful publication | Bounded failed terminal send possible | No acknowledgement, retry or queue |

`ParserInputCompletion` records actual successful parser finish, separately from
decoder EOF, capture completeness, publication and static-input settlement.
Missing or prefix-only evidence cannot establish successful completion. A
Finalized report with truncated/incomplete capture can establish parser
completion, while explicitly disclosing diagnostic degradation. Settlement and
checked-frame relationships require the finalized descriptor before examining
stylesheet relationships. Failure admission remains Pending-only; rejected
success, duplicate/stale/replaced terminals and events after first failure
cannot install or enrich diagnostics. Retained previews remain usable under
existing policies, but their presence never certifies final parsing/assembly.

## Observation contracts

`Tab::parsed_document_debug_snapshot()` is separate from AJ2/AJ4 Tab Debug
snapshots. It passively reports input counts/encoding decision, diagnostic
availability/capture/accounting, structural tree, URL/origin/base categories and
mode. Default output never contains private URLs, header values, markup, text,
comments, element names, attribute names/values, diagnostic descriptions or
runtime IDs. Diagnostic identity enums and occurrence order are retained; tree
labels are snapshot-local, never runtime identities.

Budgets: 8,192 serialized bytes, 16 diagnostic rows, 128 visited entries
(including template fragments), depth 32, 33 fixed traversal frames, 256 bytes
record scratch and 512 bytes reserved footer space. The iterative walker fetches
one child at a time, template association before ordinary children. It never
collects a full tree or counts omitted descendants. Omission and byte/record,
node/depth truncation are explicit. Output reservation is fallible; no
uncontrolled intermediate formatted string is needed.

The feature-gated `conformance-observation` API
`Tab::observe_parsed_document()` returns a borrowed read-only view. Its getters
cover phase, navigation pending, tree/address/diagnostic scopes, request,
handle/version, body/normalized input size, encoding/decoder report, document,
mode, canonical report and document URL/origin/fallback/active base. Owning
Browser state stays private. Idle, awaiting response, parsing, parsed, failed
and invalid-evidence states remain distinct. Addressing may be Prepared before
accepted publication, then Committed; a failed tree is RetainedPreview while
diagnostics describe the parser session. Pending navigation leaves the current
active document and its evidence paired until replacement admission.

Controlled exact fixture serialization first bounds source to 4 KiB, tree to
128 entries/depth 32, attributes to 128 and string fields to 8 KiB. A conservative
escaping bound must fit 128 KiB before existing DomSnapshot serialization.
Arbitrary documents never use unrestricted serializers through this API.

## Addressing and supported standards boundary

Existing [HTML document/base rules](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#document-base-url),
[base element freezing](https://html.spec.whatwg.org/multipage/semantics.html#the-base-element),
[tree-construction modes](https://html.spec.whatwg.org/multipage/parsing.html#the-initial-insertion-mode)
and [URL origins](https://url.spec.whatwg.org/#origin) remain owned by AE/AH.
Final admitted response URL is document URL and fallback base. First applicable
HTML base[href] wins and freezes temporally. Invalid/prohibited first bases
retain fallback without advancing to later bases; empty href participates;
target-only, foreign and template-contained bases do not win. Cross-origin bases
do not change document origin. Tests independently assert actual activate_link
issued target, final document initiator and requesting origin, without adding
AJ6 discovery or scheduling.

AJ4 remains UTF-8-only with declared/default charset admission. No BOM removal,
meta encoding selection or HTML sniffing is added; its BOM-to-Quirks consequence
remains documented. Diagnostic position/provenance/retention/privacy contracts
are Borrowser contracts, not WHATWG-mandated debug APIs.

## Qualification and exclusions

Targeted default/feature and debug/optimized tests cover parser, runtime and
actual Fetch/runtime/Browser admission. Existing parser, DOM/patch/WPT, fuzz,
addressing, networking and CI lanes remain required. The allocation guard checks
zero event-storage allocation for clean input and type-derived bounded storage
for retained errors: at most two reservations, pending capacity 128 plus the output reservation for the pending retained
prefix, using actual event layouts. Coordinate failure can export fewer rows
without shrinking that bounded output capacity. Private work instrumentation records
requested bytes, live Vec capacities, simultaneous peak storage, fixed scratch,
scan/scalar work and disabled unrelated/index capture. Refusal tests distinguish
attempted reservations from acquired capacity and prove no retries. These bounds
describe retained Vec capacity, not allocator metadata or a whole-process RSS cap. The identical stable-API runtime probe measures 18
cases (64 KiB/1 MiB × clean/late/heavy × 1/64/1024-byte chunks), five paired fresh
processes with alternating order and explicit one-warmup/one-sample arguments.
Both executables read the same six external files and SHA-256 manifest. The
probe hashes actual consumed bytes outside measured parsing and emits flushed
case-start/end records. Source, probe, corpus, manifest, lock, receipt and
executable identities are checked before and after each process. Grouped heavy
markup stays within parser budgets. No input is independently generated in Rust. Limits are 600 seconds per case including
warmup, 1,800 per process, 10,800 per campaign, 10 seconds cleanup and 64 KiB per
pipe. No timeout, missing row, incomplete campaign or baseline failure passes.
Greater than 10% median 1 MiB slowdown beyond paired variability requires review.
The supervisor observes leader exit without reaping, preserving owned PID/PGID
identity through its single group signal on success and failure. After reaping
it performs only bounded absence queries; unresolved cleanup fails and stops the
campaign. Darwin may report EPERM for an unreaped zombie or a transient killed
orphan; that result is recorded, never interpreted as absence. Only eventual
verified ESRCH establishes group cleanup. macOS requires Python 3.13+ for waitid.

The loopback example owns its server, redirects, explicit UTF-8 malformed HTML
and clean replacement. Expected failures use Result and retain primary and
cleanup results separately. All paths cancel actual issued work, close runtime
inputs, stop/join the owned server and require event-channel disconnection under
one ten-second shutdown deadline. The AJ5-only parent supervisor also qualifies
controlled optimized panic=abort behavior by reaping the process/group and
checking the loopback listener is closed; this is distinct from an in-process
server/runtime acknowledgement. No production runtime join API is added.

Local uncommitted CI input changes cannot satisfy committed-source qualification.
Hosted exact-source Linux/native and optimized qualification remains pending;
local failures must retain matching exact-base evidence. Historical Chromium
failures do not qualify this candidate. See docs/conformance for AG2/AG4 support.
AWS is NOT NEEDED: no authentication, operations, provisioning, cost or teardown.

No AJ6 discovery/classification, scheduling expansion, stylesheet/CSS semantics,
JavaScript, full navigation/history, inherited contexts, security-policy
enforcement, BOM/meta work, full assembly or new production dependency is added.

## Corrected-candidate evidence and qualification provenance

Browser tests now read the accepted anchor's actual href, release its borrow,
and assert complete target/fragment separately from fragmentless retrieval,
initiator and requesting origin. Actual streaming publications establish stable
parser-key/materialized-ID mappings and temporal frozen-base lifetime. A real
fallible output reservation is refused in the existing Browser test allocator
to demonstrate finalized incomplete capture through terminal admission,
settlement and checked-frame rendering; no production fault injection is added.
Missing/prefix stored descriptors fail before pending stylesheet relationships.

`tools/ci/check_dependency_subtrees.py` includes the task supervisor among
committed qualification inputs. This is qualification-input provenance, not a
weakened gate: new untracked qualification tests still reject uncommitted local
candidates. Full diffs, manifests, command receipts and prior failures accompany
each local review packet. Hosted qualification and owner merge authority remain
pending separate gates.

Prior local V5 `make test` failed the native Chromium process-contract `drop`
fixture at `task_name_for_pid(87856): Mach 5`, with incomplete cleanup and
retained artifacts. Later exact-base and candidate executions passed, which does
not establish a cause or waive the failed run. The separately reproduced AJ5
supervisor defect is corrected independently and does not explain that native
failure. Earlier performance campaigns remain historical measurements with the
then-current cleanup/protocol evidence limits; they do not qualify this corrected
candidate. The corrected campaign is one authorized fresh execution, without
retries after an incomplete or failed result.
