# AJ8 — Ordered static author stylesheet input

Issue [#1139](https://github.com/joris97jansen/borrowser/issues/1139), Milestone AJ.
Local implementation candidate; independent implementation review and subsequent
committed/published qualification are separate gates. This contract records
behavior, not a historical or current CI pass claim.

## Authority and data flow

`DocumentStyleSet` remains the sole attachment/order owner. DOM owner, monotonic
slot ID, sparse stylesheet order, CSS source ID and request generation are distinct.
Duplicate URLs remain independent attachments. Pending, unrequested, failed,
rejected and aborted positions are never compacted.

Browser owns host applicability, ordering, lifetime, provenance and coordination.
AH owns URLs. AI owns acquisition and immutable aggregate response metadata.
CSS owns parsing, recovery, completeness, media, semantic at-rule names and AF
classification. Layout/Paint gain no stylesheet acquisition or decoding role.

```text
prospective DOM + actual document encoding
  -> prepare_reconcile_with_environment (fallible, no publication/dispatch)
  -> atomic DOM/addressing/style publication -> retire old requests -> dispatch
  -> AJ7 complete coherent Ready response/body
  -> select_stylesheet_decoding -> rejected OR CssChunk(complete bytes, decision)
  -> CssDone -> tools::utf8 -> checked CssDecodedStylesheet -> Browser validation
  -> CSS parse -> slot-owned parse/admitted response base -> authenticated Done
  -> ordered borrowed AF inputs / existing compatibility projection
```

AJ8 can be inspected and qualified before computed style or rendering. Existing
style execution only supplies a small invalidation regression; AJ9 assembly and
AJ10 cross-engine resource-backed conformance remain separate.

## Encoding subset and exact byte grammar

Inline DOM Unicode bypasses byte decoding. External selection requires coherent
complete AJ7 Ready evidence; status, MIME and final-response provenance remain AJ7
policies. HTTP decompression precedes this boundary; CSS preprocessing follows it.

Only UTF-8 decoding is implemented. `encoding_rs` 0.8.35 provides canonical
Encoding Standard label identities through direct bus/runtime dependencies;
`tools::utf8` remains the actual authoritative decoder. No second label registry
or legacy decoder is introduced, and AJ4 HTML selection is unchanged.

Selection priority is BOM, recognized transport charset, qualifying byte
declaration, applicable environment, UTF-8 default. Unknown labels continue
fallback. Recognized unsupported effective encodings reject recoverably.
UTF-8 BOM removes exactly its first three bytes. UTF-16BE/LE BOMs explicitly
select unsupported encodings, even with a conflicting UTF-8 transport declaration.

A declaration qualifies only at offset zero, beginning with the exact ten bytes
`@charset "`. Label bytes are 00–21 or 23–7F inclusive. The first subsequent double
quote must be immediately followed by semicolon. The semicolon must be at offset
1023 or earlier, so the entire declaration fits the first 1,024 bytes. No trimming,
comment skipping, escape decoding or case normalization is performed on this
grammar. Label resolution itself follows Encoding Standard normalization.
Malformed declarations differ from exact declarations with unknown labels;
higher-priority selection records the declaration as not consulted. Only a
qualifying declaration selecting UTF-16 converts its effective encoding to UTF-8.

Recognized legacy link `charset` supplies environment encoding; otherwise an
actual AJ4 `DocumentEncodingDecision` supplies UTF-8 document evidence. Missing
document evidence stays absent. The environment's source and canonical value are
part of attachment identity. A canonical context change replaces/retire the old
attachment before new dispatch. Transport/BOM still outrank that context.

## Checked evidence and accounting

Bus DTO fields are private. Checked constructors enforce decisions and reports;
canonical identities cannot be fabricated from arbitrary names. Browser compares
the received decision with the previously selected decision and retained Ready
bytes before storing evidence, parsing or admitting a response base. Settlement
also recomputes selection from authoritative AI metadata/environment.

For finalized success, N is delivered body bytes, B consumed initial BOM bytes
(0 or 3), P decoder payload bytes, O emitted UTF-8 bytes. I/AI are invalid-sequence
events/affected payload bytes, F/AF incomplete-EOF events/affected bytes:

```text
N = complete Ready body length; P = N - B; O = returned String.len()
I <= AI <= 3*I; F in {0,1}; AF = 0 if F=0, otherwise 1..=3
A = AI + AF <= P; E = I + F
O = P - A + 3*E; O <= 3*P
actual EOF returned; pending prefix = 0
```

Declaration `end_exclusive` counts original delivered bytes from offset zero.
Decoder affected-byte counts refer to raw payload bytes after BOM removal;
emitted counts refer to UTF-8 String bytes. CSS spans/diagnostic offsets belong to
the retained decoded `CssInput` (a bounded UTF-8 prefix when limited), rather than
transport-byte coordinates. No per-byte reverse map or unbounded issue log is kept.

Every operation uses checked arithmetic. Literal U+FFFD is not a replacement
event. Accounting consistency does not authenticate arbitrary text; Browser does
not redundantly decode the entire body to authenticate an internal worker String.

Interrupted output is a separate report. A sink callback cannot interrupt the
tools decoder: processed input covers each whole call that returned, even after
appending stopped. Callback counts continue, output stops at the first failure,
pending-prefix evidence reflects actual decoder state, and EOF is true only after
the actual finish call returned. Output/pending bounds use processed payload;
successful output equality is never applied to interrupted output. Reservation
failure is `NotStarted`; output-bound failure requires `Interrupted`. Partial text
is never returned as successful or installed.

## Runtime and terminal protocol

The supported production sequence is **one complete-body CssChunk followed by
matching CssDone or CssAbort**. Delivery partition independence belongs to the
authoritative decoder tests, not a multi-command streaming session contract.

The existing worker map owns `PendingSheet { issued_url, bytes, decision }`.
Initial map reservation/input admission failures attempt dedicated terminal
`CssRuntimeFailed`. No allocation-dependent tombstone or permanent key history
is needed. Matching Done removes the session, decodes, then attempts outcome and
Done sends in order. An admitted output-allocation failure produces a failed
outcome followed by Done. The two URL owners come from Chunk and Done; no new
full-body or URL clone is needed in the worker.

Duplicate Chunk removes the old session and reports duplicate-command failure.
Wrong-context Done removes that session and reports command provenance failure.
Unknown Done is silent. Authenticated Abort removes without EOF/output; wrong
Abort is inert. Closed command input drops remaining sessions; closed event output
stops and drops all state. Synthetic repeated/reused keys have no global at-most-once
guarantee. Browser's current-generation/live-slot admission prevents stale results
from reopening authoritative attachments. No join/cancellation acknowledgement or
guaranteed terminal delivery is claimed.

| Browser progress | Parse | Decoding | Terminal fact |
| --- | --- | --- | --- |
| AwaitingOutcome | absent | selected | none |
| ParsedAwaitingDone | available | successful finalized | none |
| DecodeFailedAwaitingDone | absent; slot remains Pending | failed | none |
| settled success | available | successful finalized | actual Done + installation Ok |
| settled decode failure | absent | failed | actual Done + matching Decode error |
| initial runtime failure | absent | selected | actual RuntimeFailed + matching error |
| unsupported selection | absent | rejected | no runtime/installation receipt |
| handoff/protocol failure | absent | available prior facts | installation error; no fabricated receipt |

Wrong issued keys precede duplicate suppression. Correct-key later outcomes are
ignored after the first accepted outcome, including contradictory success/failure.
Done/terminal failure closes live admission. Retirement/replacement drops ownership
and reservations; late events are inert. Partial handoff (Chunk sent, Done send
failed) retains Ready, records installation failure and attempts best-effort Abort.

External parser integrity failure retains validated decoding, excludes the local
offending parse/base, latches detailed `StylesheetParserInvariant` first, then
removes live admission, records installation failure, marks aborted, attempts
cleanup, and updates loading only from coherent state. Earlier fatal document
failure wins. Completed sibling/previous coherent preview evidence survives;
existing incomplete network cleanup remains unchanged. The fatal payload identifies
`AdmittedExternal { slot }`. Inline preparation carries
`ProspectiveInline { owner, slot }` through publication failure; that reserved or
retained prospective identity is not evidence of an accepted publication. The
existing borrowed Tab/document context supplies generation/handle/version rather
than duplicating them in retained failure data. Fatal status text remains fixed
and source-free.

Settlement/observation validation rejects a selected or failed decode with an
installed response base. A succeeded outcome retracted by a subsequent wrong-key
runtime event legitimately retains exactly its admitted final-response base and
no parse/receipt. That history must not be confused with pre-parse failure. Any
current external parse requires finalized successful decoding, matching
`stats.input_bytes == input.len_bytes() <= emitted_utf8_bytes`. Complete parsing
requires equality with emitted bytes; resource-limited input may be a shorter
CSS-owned UTF-8-boundary prefix. Browser does not repeat CSS input admission or
redecode the body. Fetching has neither decode facts nor runtime receipts; a
terminal ParserIntegrity installation requires an already-latched fatal document
cause (including a legitimate earlier cause). Contradictory retained state is an
error, never repaired by observation.

## Participation and prepared transitions

Ordinary untitled eligible sheets participate when complete. Disabled, alternate
and named external sheets remain represented but initially request nothing and
reserve no fetch metadata/body. Named inline sheets may parse but stay inactive.
Inline `type` accepts absent, empty or ASCII-case-insensitive exact `text/css`;
no trimming or MIME parameters. Unsupported inline types produce no new parse.
External type hints/security attributes gain no invented enforcement. Existing
production rel token membership is preserved, including its existing Unicode
splitting behavior; AJ6's passive discovery contract remains distinct.

CSS AF5 classifies absent/empty/five-ASCII-whitespace media as active; every other
value, including `all` and `screen`, remains deferred. Host-eligible media-deferred
external sheets acquire and parse normally; deferral is not acquisition failure.

Retained prepared actions are Keep, ParseInline, or BeginExternal with Dispatch
or an unissued source failure. Host/media changes, first activation and parse
availability changes prepare prospective compatibility inputs before publication.
Commit only moves already prepared values using reserved storage. Initially
excluded sources dispatch at most once upon first eligibility; issued terminal
work never retries on eligibility changes. Already admitted work may finish while
inactive. Returning eligibility can reuse a retained complete parse. External
selected address/admitted base stays frozen; inline references use current
committed document/base context. A failed prepare preserves prior authoritative
DOM, slots, projection and invalidation generations.

AF inputs exclude host-inactive/incomplete/unavailable parses and retain raw media
and sparse source coordinates. Compatibility `&[StylesheetParse]` also omits media
deferral because it cannot carry conditions. Same-slot effective changes trigger
existing stylesheet-set invalidation. Canonically equivalent host facts do not
invalidate the stylesheet set; DOM publication still advances its own version.

## CSS completeness and authored resources

Ordinary recoverable syntax diagnostics do not suppress usable CSS. Empty input is
a complete successful parse. `StylesheetParse::completion` prioritizes independent
`had_invariant_violation`, then `hit_limit`, then Complete. Tokenizer/parser input,
token/rule/container and selector-local limits indicate ResourceLimited. Diagnostic
collection/cap alone does not. Integrity statistics survive suppressed diagnostics,
merges and debug/fuzz projections. Fatal whole-stylesheet integrity policy does not
broaden to unrelated CSS consumers or style-attribute execution.

Limited parses remain inspectable but noncontributing. AH8 retains legitimate
inactive/partial authored-source observations; this evidence does not imply complete
resource discovery. CSS converts at-rule names semantically once while preserving
raw input/spans. Shared classification reports import/media/supports/layer/scope
deferral and unknown/unresolved names before AF execution. AH8 no longer unescapes
already semantic names. No recursive imports or conditional block flattening occurs.

## Ownership, limits and passive observation

AJ7 logical limits remain 4,096 slots, 65,537 ordinary nodes, 8 MiB source payload,
8 MiB fetch metadata and 64 MiB body credits, with 2 MiB raw bytes per sheet.
Runtime output reserves at most 3*payload (6 MiB worst replacement expansion).
Existing CSS input/token/model limits remain unchanged; decoded output may exceed
the parser's 4 MiB input limit and then remains a limited noncontributing parse.

Ready owns raw bytes. Worker temporarily owns the existing handoff copy. The
decoded String lives through parsing; CSS owns its bounded parser input/structures.
Slot owns one parse; compatibility retains only its existing eligible model clones.
Prepared publication overlaps old/new projections. The current CssInput owns a
String and line-index Vec: compatibility model cloning also deep-copies those
buffers. No input-sharing optimization is introduced. Logical retained credits
are not total heap/RSS guarantees.
No independent full-parse observation copy is added.

`Tab::static_stylesheet_observation` borrows authoritative state and validates all
applicable slots before display limits, distinguishing no committed document from a committed
empty set and rejecting inconsistent presence tuples. It never schedules, repairs, reparses,
matches, cascades or renders. Snapshot limits: 16 KiB total, 64 rows, 1,024-byte
stack row scratch, 512-byte footer allowance, 128 diagnostic and 128 at-rule display
budgets. Separate row/retained-diagnostic/at-rule omission evidence is explicit;
parser-retention omission is a separate CSS-owned boolean; no exact dropped-diagnostic
count is invented. Stable source-free labels redact
raw URLs, labels, headers, CSS and diagnostic messages. The v2 grammar uses explicit
labels and snapshot-local sheet/owner/document identities, not derived Debug or
runtime identity values. Persisted `decode=selected` is independent of live runtime
progress: `awaiting-outcome`, `parsed-awaiting-done`,
`decode-failed-awaiting-done`, or `not-live`. Terminal failures never claim pending
runtime work. Installation and actual receipt remain separate fields.
A row record is atomic: if it exceeds scratch space, its entire bytes are rolled
back and it is omitted from displayed-record counts. No diagnostic/at-rule fragment
is reported as a complete record; footer omission and truncated-row facts remain
explicit.

Exact source-bearing access is confined to conformance-observation (and private
unit tests). `StaticStylesheetObservation::document` returns the existing
`ParsedDocumentObservation` on the same immutable Tab borrow. Its handle/version/
request generation and document URL/active base correlate with every entry; inline
contexts always use these current committed addresses. Entry methods expose actual
DOM owner and slot, checked sparse CSS order/source ID, existing host/media,
parse availability/completion, decoding outcome/report, live runtime progress,
installation and actual receipt. `entry.fetch()` returns the AJ7 entry for that
exact slot/order: authored href/selected address, issued request/key/generation,
immutable acquisition response and body facts remain distinct from
`entry.response_base()`, which is the installed base. No new retained inventory,
source copy, parse copy or body copy is introduced. All source coordinates are
checked before formatter limits, even omitted rows.

Borrow/view/validation allocates zero; bounded snapshot reserves one 16 KiB output
allocation with no reallocations. Guards cover a mixed inline/external set with
Ready, failed decoding awaiting Done, inactive, limited and pending inputs, plus
no-document, empty, terminal-failed and inconsistent states. Selection and received
report validation are measured separately from decoder allocation on a worker
thread. These are cumulative requested allocation counts in defined regions, not
live-memory/RSS measurements. Existing structural clone/commit guards remain.

## Qualification and exclusions

`tools/ci/aj8_tests.py` records nonempty per-package lists, every authored required acceptance member and required anchors,
listed/executed equality and parameter-row evidence in default/exact and optimized
configurations. `aj8_stylesheet_probe` exercises real loopback acquisition, AI
response charset metadata, tools decoding, CSS parsing and observations before
style execution. Its release wrapper reuses unchanged AJ5 supervision, requires
exact expected Result-failure/SIGABRT markers, independent cleanup/listener/binary
checks, 70-second supervision, 10-second group cleanup and 65,536-byte stream limits.
Timeout/overflow/ownership uncertainty is never waived. Candidate receipts retain
actual commands, identities, failures and limitations externally.

Existing hosted correctness/release jobs preserve mandatory checks/source_complete
and job timeouts; AJ8 adds bounded evidence steps/artifacts. Hosted integration
source identity differs from the exact uncommitted local fingerprint and existing
streaming exact-head qualification. No hosted pass is claimed here.

AWS is not needed: the AG runner profile does not expose AJ8 internal evidence;
local actual workers and direct semantic contracts answer AJ8 acceptance. No AWS
operation/authentication occurs. AJ9/AJ10, full legacy decoding, complete preferred
or alternate set selection, real media evaluation, CSSOM, imports, security policy,
cache, image/font loading and JavaScript remain excluded.

CSS syntax/model snapshot grammar advances to version 2 for the independent
integrity-stat field. Other CSS snapshot grammars keep their existing versions.

The six promoted CSS fuzz inputs retain immutable approved-base expectations in a
private library-unit audit. Only the exact new false integrity-stat line and the
changed syntax/model v2 headers (including nested indentation) are normalized in
that test-only RAII mode. Complete legacy summaries must still match before current
summary digests migrate. Production hashing always uses complete v2 strings; this
fixed-fixture audit does not claim parser-wide equivalence.

Correction qualification also uses private scoped one-shot internal faults for
actual tokenizer payload-span recovery, canonical structured-parser EOF validation,
model selector conversion and semantic at-rule name resolution. Ordinary,
disabled and zero-cap diagnostic modes qualify producer-to-model/completeness/debug
propagation; tokenizer/parser faults reach the actual syntax-fuzz rejection path.
Model-only faults do not claim syntax-fuzz coverage. Browser's injected final-stat
hooks separately qualify inline prepared-publication and external fatal consumers;
no authored CSS syntax error is treated as an engine invariant.

The aggregate `make ci` remains blocked at committed-input preflight for an
unstaged/uncommitted candidate. Independently executed constituents are recorded
as constituent evidence, never aggregate success. Ordinary stable libFuzzer smoke
scripts (including hosted equivalents) lack coverage instrumentation; coverage-guided
ASan qualification requires an explicitly selected available toolchain/tool setup.
The original extended RGB fixture INCOMPLETE outcomes and mixed benchmark timings
remain disclosed in the external candidate packet; none is rewritten as PASS.
