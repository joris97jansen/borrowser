# AE13 Parser Conformance and Regression Harness

## Status

This contract begins in AE13a. AE13a defines the canonical fixture and result
models, deterministic discovery and validation, exact input handling, shared
document-mode ownership, disposition policy, and one standalone-tokenizer
fixture. AE13b1 adds the parser-owned observation foundation for tokens,
tokenizer parse errors, and implementation diagnostics. AE13b2 extends that
same production recorder to tree construction and captures the scalar
`DocumentMode` selected by the production tree builder. AE13b4 adds
parser-owned central tree-dispatch transitions and exact observations for six
encountered, deliberately unimplemented tree-construction behaviors. It does
not claim completion of overarching AE13 or implementation of those six
algorithms. AE13c adds bounded semantic whole/chunk parity and mandatory
production-owned terminal audits for every supported fixture-v2 execution.

AE13d curates the native fixture corpus against the completed AE contracts. It
adds no parser behavior, harness semantics, snapshot format, registration
architecture, external adapter, or blessing workflow. Coverage, observation
selection, and independent expected-output evidence are tracked in
`docs/html5/ae13d-native-corpus-coverage.md`. AE13d does not claim complete
HTML conformance; exact documented AE13b4 unsupported identities remain
explicit until their owning parser work is completed.

## Ownership

The `html` crate owns parser semantics and typed canonical observation values.
`html::DocumentMode` is the one document-mode type used by tree construction
and future parser observations. The exact token, position, parse-error,
implementation-diagnostic, transition, and unsupported-feature semantic
identities live in always-compiled HTML/parser ownership.
`html::conformance` re-exports those same types and owns
the passive request/result execution boundary behind the non-default
`parser-conformance` feature. These are versioned engine-test contracts rather
than stable DOM or general-purpose parser APIs. `HtmlParseOptions`,
`ParseOutput`, `HtmlParser::new`, and `parse_document` do not expose conformance
configuration or results.

The `html-test-support` crate owns serialized fixture-v1 loading, safe path and
input validation, deterministic bundle discovery, expectation selection,
disposition evaluation, and the canonical test runner behind the non-default
`parser-fixtures` feature. That feature activates `html/parser-conformance` and
the optional SHA-256 dependency. It may invoke existing HTML parser APIs, but
it must not reimplement parsing algorithms.

The serialized declaration is the only construction input. Validation seals
fixture IDs, delivery names, snapshot paths, exact input, execution, target,
expectation, source, and disposition state behind an opaque
`ValidatedFixtureSpec`; public consumers receive read-only accessors. The AE13e
external adapter enters through the same declaration and validation path. It
must not construct validated values directly; future external adapters follow
the same rule.

AE13c parity compares borrowed typed canonical observations before any snapshot
serialization. A serializer is invoked only for the selected parity surface
after typed inequality, or for an expectation surface that actually applies to
that delivery. The runner retains one owned baseline, borrows it while
comparing candidates, and moves it into the completed report only after the
candidate schedule succeeds; it does not deep-clone canonical baselines.

Final-audit allocation injection is attached immediately before each real
fallible reservation. A reservation site occurrence is counted only when that
collection, projection, or traversal stack actually attempts to reserve. The
injected and natural failures use the same observation-resource identity and
never produce a partial report. Boundary diagnostics use the streaming,
platform-independent `borrowser-html-delivery-boundaries-v1` SHA-256 encoding;
the digest is lazy diagnostic metadata and never participates in strategy
equality or scheduling.

There is one canonical parser-fixture runner. Existing golden, WPT-style, and
internal corpus infrastructure will be adapted to it in later slices rather
than being joined by another independent harness.

## AE13a execution boundary

AE13a executes one configuration:

- native, active fixture;
- exact UTF-8 `input.html`;
- standalone tokenizer;
- whole Unicode-scalar delivery;
- `tokens.txt` in `html5-token-v1`.

The runner executes every discovered fixture in deterministic
repository-relative order and aggregates failures with fixture ID and bundle
path. Adding an ordinary fixture requires no Rust registration or test edit.

The runner uses the existing tokenizer driver and `TokenFmt` for the
`html5-token-v1` compatibility comparison. It obtains owned canonical tokens
from the feature-gated production observation boundary at the shared token
queue drain. It does not introduce another tokenizer implementation, parser
algorithm, or token formatter.

Every other fixture-v1 expectation remains declarable so the schema will not
need replacement as later slices land. An active fixture that requests an
unimplemented surface fails with typed `UnsupportedExpectation`. Valid but
unimplemented execution semantics fail with typed
`UnsupportedFixtureSemantics`. Neither outcome is treated as a passing active
fixture.

## AE13b1 production observation boundary

`html::conformance::execute_parser_observation` is the single feature-gated
engine-test boundary. Its standalone-tokenizer and document-parser targets run
the production decoder, input preprocessing, tokenizer, token queue, and, for a
document target, ordinary tree construction and final DOM materialization.
`html-test-support` obtains canonical tokens from this boundary; it does not
canonicalize raw production tokens itself.

The canonical result retains exactly nine top-level surfaces: tokens, parse
errors, implementation diagnostics, document mode, tree, patches, transitions,
unsupported features, and final invariants. AE13b1 can populate only the first
three. Every other surface remains `NotRequested`; fixture-v1 expectation
activation is unchanged.

Observation is request-driven, independently bounded per surface, and absent
from ordinary parser construction. Occurrences are assigned at the production
recording point before capacity filtering. Parse errors have one sequence and
implementation diagnostics have another; both begin at one. There is no
cross-surface timeline and no event sorting. A full surface retains its original
prefix and reports later drops as `Incomplete(StorageLimitExceeded)`. Semantic
occurrences, dropped counts, and normalized coordinates never saturate: an
unrepresentable next value terminates conformance execution with a typed
observation-invariant error instead of emitting duplicate or false data.
Reserving `u64::MAX` for either occurrence sequence latches
`OccurrenceSequenceOverflow` immediately because the next value cannot be
represented; the current event may retain `u64::MAX`, but successful
conformance completion is no longer possible and no second event can duplicate
that occurrence. This rule applies even when the requested surface has zero or
already-exhausted capacity.
Invariant retention is first-wins in production detection order. Once an
invariant is latched, later invariant failures cannot replace it. For example,
one reservation can detect occurrence exhaustion before a dropped-count
overflow; the occurrence failure remains authoritative because that is the
reservation's production operation order. AE13b1 neither ranks invariants by
severity nor collects and reorders multiple failures.

Tokenizer tokens are converted to owned canonical values only at the
authoritative production queue-drain primitive and only while the requested
token prefix can still be retained. Ordinary and observed draining consume that
same queue. Diagnostic capture does not change legacy `ErrorPolicy` retention,
parser counters, tokenization, recovery, patches, or final output.
If any surface was requested, both execution targets require the production
recorder to survive through result extraction; a missing recorder is
`ObservationRecorderMissing`, never an apparently valid all-`NotRequested`
result. An absent recorder is normal only when every surface was unrequested.

## AE13b2 tree-construction observation boundary

`DocumentParseContext` owns one neutral `ParserEventSink` fanout for
diagnostics and non-diagnostic semantic observations across preprocessing,
tokenization, and tree construction. A short-lived
`TreeBuilderProcessContext` borrows only the atom table and that fanout for one
logical token, including every reprocessing iteration. The authoritative
central dispatch boundary reserves and retains transitions through this
context; exact production fallback branches reserve unsupported events.
Individual insertion-mode modules cannot own counters, a second recorder, or
legacy projection. Unobserved parsing uses the same production path and
installs no tree-owned recorder, queue, or string store.

Genuine HTML tree-construction parse errors increment
`Counters::parse_errors` whenever counter tracking is enabled, regardless of
canonical capture and capacity. Configured resource limits and Borrowser
implementation deviations use the implementation-diagnostic sequence and do
not increment that counter. Fatal parser-owned reservation/resource exhaustion
and engine-invariant failures remain execution failures. AE13b2.2a gives these
failures bounded, allocation-free identities and latches the first live-session
fatal failure; it does not claim that all allocation, patch, or
materialization boundaries are fallible.

The production taxonomy is:

- typed recoverable tree-construction parse errors for invalid authored-input
  conditions defined by the supported HTML tree rules;
- typed implementation diagnostics for deterministic Borrowser deviations;
- typed configured resource-limit diagnostics for bounded open-elements,
  node, child, and template-mode capacity;
- fatal execution failure for covered parser-owned reservation/resource
  exhaustion and engine invariants; and
- no observation for normal implied elements, mode transitions, delegation,
  reprocessing, scope scans, or stack mutation.

One production rule may emit both a genuine parse error and a distinct
implementation diagnostic. Descriptions are fixed, non-authoritative text,
not identities or retained parser state.

Canonical document results remain success-gated. A requested observation drain
returns the latched parser fatal identity rather than partial observations, and
document mode, observations, and the materialized document are assembled into
the canonical result only after successful finish and final materialization.
Fatal failures are not recorded as authored-input parse errors or observation
events. The conformance execution error preserves the precise parser fatal
identity; tokenizer invariant specialization remains available for tokenizer
engine invariants.

The deterministic failure-injection facility is internal, non-default,
parser-scoped, and keyed by bounded semantic reservation-site identities. It
can fail a selected occurrence once, then disarms, without mutable
process-global allocator state. It exercises the production reservation path
and exists only for engine tests; it is not part of stable parser options.
`make test-html5-parser-fatal-failures` executes the HTML and runtime
failure-injection suites and is part of both local `make ci` and GitHub CI.

The stable `HtmlParser::parse_errors()` facade retains only events its legacy
model can truthfully represent with an exact normalized-input offset. Current
tree events use `Unavailable(ParserDidNotProvidePosition)` and are omitted.
`Counters::parse_errors` can therefore exceed
`HtmlParser::parse_errors().len()`. `errors_dropped` still counts only bounded
legacy-deque eviction; canonical capacity drops and unavailable tree positions
do not affect it. `HtmlParseEventOrigin::TreeBuilder` remains a reserved
compatibility identity, but AE13b2 never fabricates a position to reach it.

Tree events capture token kind, active production insertion mode, and adjusted
current-node namespace, when present, at the detecting rule before recovery
mutates that state. AE13b2 neither scans source nor uses a later tokenizer
cursor as an approximation.

The self-closing finalizer runs once only after reprocessing, statistics,
incremental template validation, and optional EOF audit succeed. Explicit
acknowledgements emit no error. Acknowledgement belongs to the applicable
production rule: the supported HTML void-element rules (`area`, `base`,
`basefont`, `bgsound`, `br`, `col`, `embed`, `hr`, `img`, `input`, `keygen`,
`link`, `meta`, `param`, `source`, `track`, and `wbr`) acknowledge only when
their implemented rule reaches that step; ignored tokens are not acknowledged
by a global void-name check. Foreign-content acknowledgement remains owned by
foreign dispatch. A genuinely ignored flag emits
`UnacknowledgedSelfClosingFlag` with `IgnoreSelfClosingFlag`; the deprecated
non-void HTML `LegacySkipPush` path emits the same parse-error code without a
claimed recovery action plus
`NonVoidHtmlSelfClosingFlagAlteredStackDisposition` at the production decision.
That decision is committed only after configured open-elements, node, and
child limits accept insertion. Suppressed insertion therefore retains its
resource diagnostic and truthful ignored-flag recovery without claiming that
the stack disposition changed. Contradictory self-closing state transitions
are fatal tree-builder invariants and cannot append the common parse error.
This records the current deviation without changing DOM or patch behavior.

The tree builder's Text insertion-mode EOF rule is the sole integrated owner
of `TreeConstructionParseErrorCode::EofInTextMode`. The tokenizer still
flushes literal tails and emits EOF, but a standalone tokenizer does not
synthesize a tree error.

Document-mode capture is a dedicated scalar request. After
`HtmlParser::finish()` succeeds, conformance execution reads the existing
`DocumentMode` from `Html5ParseSession`/`Html5TreeBuilder`, extracts collection
observations, and runs the existing `into_output()` validation/materialization
path. The scalar is returned only if the complete execution succeeds. A
standalone tokenizer returns `NotApplicable(StandaloneTokenizerRun)`. No test
support reclassifies a doctype or infers mode from a `DocumentType`, patches,
or the materialized DOM.

Legacy DOM-golden parse-error lines are produced only by the
`parser-conformance`-gated adapter in
`crates/html/src/test_harness/tree_diagnostics.rs`. It explicitly installs the
production recorder with a finite capacity of 4096 tree parse errors, rejects
incomplete capture or drops, and projects authoritative typed tree parse
events one way to non-canonical description lines. It neither captures nor
merges implementation/resource diagnostics, and patch goldens do not use this
adapter.

## AE13b4 dispatch and unsupported-feature observations

One `TreeTransitionEvent` is one invocation by the central iterative
tree-builder driver of exactly one selected top-level algorithm family for the
current logical token. The typed paths are `HtmlInsertionMode(mode)`,
`SharedTemplateRules`, `ForeignContent`, and `TextMode`.

The event captures its surface-local occurrence, an immutable canonical token
summary, the committed insertion mode immediately before selection, the
selected path, the committed mode after central outcome validation and
application, and whether this is a later central attempt for the same logical
token. `reprocessed` means attempt index greater than zero; it does not predict
the current attempt's outcome.

Internal delegation is not another attempt. Table-family calls to InBody,
InTemplate calls to supported InHead behavior, temporary-head delegation, and
other direct calls between rule sets remain inside the selected attempt.
Changing insertion mode for the next tokenizer token likewise does not make
that next token reprocessed. Shared-template selection precedes Text-mode
selection, which precedes ordinary active insertion-mode handling.

Foreign processing never calls an HTML insertion-mode handler directly. A
foreign breakout or foreign end-tag fallback returns a central directive that
redispatches the same token once under `HtmlRulesOnly` selection. That scope
skips foreign selection exactly once and still selects shared-template,
Text-mode, or ordinary HTML rules centrally. It participates in exact cycle
identity, but the route-only move is not generic semantic progress. A forced
HTML attempt cannot request the same forced route; ordinary reprocessing after
it returns to normal selection. EOF and integration-point conditions selected
as HTML rules by the normal foreign decision produce one HTML-family attempt.
Template EOF depth unwind remains recovery before the next traced dispatch
attempt.

Handler outcomes are applied centrally before the event is retained. For
`Reprocess(next_mode)`, handler-owned mode may remain at the committed before
mode or already equal `next_mode`; any third mode is an engine invariant. The
driver commits `next_mode` before capturing `after`. Foreign `Done` and
`HtmlRulesOnly` directives cannot change insertion mode. Self-closing
finalization still occurs exactly once after the logical token reaches its
terminal outcome.

Transition and unsupported-feature surfaces are independent. Each has its own
request state, capacity, occurrence sequence beginning at one, retained
prefix, dropped count, and exact occurrence/dropped-overflow invariant. There
is no fabricated cross-surface sequence. Occurrence is reserved at the
production boundary before capacity filtering, including at zero capacity.
One logical token lazily creates at most one
`Arc<TransitionTokenSummary>` when an attempt can retain it; later retained
attempts share it. Unrequested, zero-capacity, and already-exhausted attempts
do not resolve or clone token strings.

Transition capacity bounds event count only. It does not bound bytes retained
by token names, character data, or processing-instruction targets. AE13b4
therefore does not describe this surface as byte-memory-bounded.

Unsupported events are structurally phase-correct:
`UnsupportedFeatureEvent::TreeConstruction` always carries a
`ParserContextSummary`. The exact closed tree-construction identities are:

- `MergeAttributesIntoExistingHtmlElement`;
- `MergeAttributesIntoExistingBodyElement`;
- `MarkFramesetNotOkForRepeatedBodyStartTag`;
- `RequireSameNamedTableCellInScopeForEndTag`;
- `GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingTableCell`;
- `GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingCaption`.

Repeated-html/body merge events occur only when the applicable production rule
is reached, no HTML template is on the stack, the authoritative root/second
stack entry has the required HTML identity, and at least one first-wins
unqualified token attribute is absent by expanded name from the corresponding
`LiveTree` element. Values, prefixes, encounter order, and already-present
different values do not create a missing attribute. The check reads the stack
entry's `PatchKey` and `LiveTree::element_semantics`, resolves no values,
mutates no element, and is skipped entirely when unsupported observation is
unrequested.

For explicit `</td>`/`</th>`, no cell in table scope preserves the existing
parse-error-and-ignore behavior and emits no unsupported event. An opposite
scoped cell emits only `RequireSameNamedTableCellInScopeForEndTag`; it does not
also claim downstream close preparation, because the Standard algorithm would
stop at the failed same-name guard. A matching current cell emits no event. A
matching non-current cell emits the compound preparation identity.
Table-structure-driven closure has no failed same-name guard and emits the
compound identity when the exact scoped cell `PatchKey` is not current.
Caption closure uses the same exact-key rule for its compound preparation
identity.

General causality rule: an unsupported event is not emitted for a downstream
algorithm step that would be unreachable if an earlier missing guard for the
same token were implemented correctly.

These events replace the four former attribute-merge, caption-close, and
mismatched-cell implementation-diagnostic identities. Independent
authored-input parse errors remain. Ordinary implementation diagnostics,
configured limits, parser guardrails, operating-resource exhaustion, engine
invariants, fixture limitations, unsupported execution targets, and deliberate
fully implemented Borrowser deviations remain separate categories.

All recorder failures share one first-observed
`ParserObservationFailure::{Capture, Invariant}` slot. Observer-only token
canonicalization, attribute comparison, or live-element lookup failure is
latched without changing parser control flow. A separately occurring parser
fatal remains authoritative and suppresses canonical observation output.
Standalone-tokenizer transition requests are
`NotApplicable(StandaloneTokenizerRun)`; unsupported-feature requests remain
applicable because future exact preprocessing/tokenizer variants may use that
surface. Tree-construction unsupported events can occur only when document
tree construction actually runs. Unrequested surfaces remain `NotRequested`.

AE13b4 adds no serializer, fixture sidecar, adapter, corpus migration, final
invariant execution, or implementation of the observed missing algorithms.

### Approved follow-up backlog

Milestone: **AE follow-up — Remaining observed tree-construction conformance**

Description: implement the exact tree-construction semantics exposed by
AE13b4 while preserving parser ownership, production dispatch, patch/stack/AFE
invariants, and whole-versus-chunked parity. The milestone does not add
scripting, frameset insertion modes, fragment parsing, public DOM APIs, or
runtime behavior.

- **Implement attribute merging for repeated html start tags.** Implement the
  applicable first-wins expanded-name merge into the authoritative existing
  parser-created root element, preserving template suppression, patch/live-tree
  consistency, source attribute order, and deterministic chunk parity.
- **Implement attribute merging for repeated body start tags.** Implement the
  applicable first-wins expanded-name merge into the authoritative second-stack
  HTML body element, preserving template and stack-state exceptions,
  patch/live-tree consistency, source order, and chunk parity.
- **Mark frameset not ok for repeated body start tags.** Implement the
  applicable repeated-body `frameset_ok = false` transition independently of
  attribute merging, respecting template and second-stack-entry body
  exceptions. Do not add frameset insertion modes or frameset element parsing.
- **Implement complete table-cell end-tag and close preparation.** Implement
  same-named `td`/`th` table-scope validation; parse-error-and-ignore behavior
  for mismatched explicit cell end tags; implied-end-tag generation before
  valid cell closure; the post-generation current-node check; and all explicit
  and table-structure-driven `close_cell` paths. Preserve stack, AFE,
  insertion-mode, patch, transition, and whole/chunk invariants.
- **Implement complete caption close preparation.** Generate the required
  implied end tags before caption closure, perform the post-generation
  current-node check and parse error, and preserve caption scope, stack, AFE,
  patch, transition, and whole/chunk invariants for explicit and
  table-structure-driven closure.

Milestone: **AE follow-up — Parser observation resource accounting**

Description: complete parser observation memory accounting without changing
semantic event identity or production parsing.

- **Add byte-bounded parser observation payload policies.** Add explicit,
  independently configured retained-string byte capacities for parser
  observation surfaces while preserving event-count capacity, production-order
  occurrences, drop accounting, passive failures, lazy logical-token
  canonicalization, shared immutable summaries, and deterministic conformance
  projection. Do not add serializers or infer events after parsing.

## Exact input and path boundary

All inputs are loaded with `fs::read`; the loader never trims or normalizes
fixture data. Every fixture declares a mandatory lowercase SHA-256 digest of
the exact stored bytes.

`input.html` is valid UTF-8 checked out with LF endings, and every carriage
return byte is rejected. `input.bin` is required for CRLF, lone CR, trailing CR,
invalid UTF-8, byte delivery, byte-position
coverage, or any other original-byte-sensitive fixture. Root `.gitattributes`
rules make those representations consistent across checkouts.

Declared paths are bundle-relative portable paths. Absolute paths, `..`, `.`
components, backslashes, missing files, non-files, and symlinks are rejected.
Recognized but undeclared snapshot sidecars are rejected as orphans. Diagnostic
paths use repository-relative `/` separators.

## Discovery and identity

Discovery recursively finds `fixture.toml` bundles and sorts them by normalized
repository-relative path. It does not depend on directory enumeration order,
timestamps, map iteration, test scheduling, or platform separators.

A directory containing `fixture.toml` is a fixture leaf. Another
`fixture.toml` below it is rejected rather than silently ignored.

Fixture IDs and delivery names use lowercase ASCII kebab case. Duplicate IDs,
case-unsafe IDs, case-insensitive ID collisions, and case-insensitive bundle or
declared-path collisions fail deterministically.

## Result and expectation semantics

The canonical result has distinct surfaces for tokens, parse errors,
implementation diagnostics, document mode, tree, patches, transitions,
unsupported parser features, and final invariants. Each observation is one of:

- not requested;
- not applicable;
- captured, including a captured empty collection;
- incomplete with a typed reason.

Incomplete capture is non-authoritative. Parse errors and recoverable
implementation diagnostics are separate. Engine invariants, impossible parser
states, finalization failures, patch-validation failures, and materialization
failures are execution or invariant failures and can never be blessed as
expected implementation diagnostics.

Dedicated HTML Standard codes use `ParseErrorCode::Standard`. The enum contains
only named parse-error identities from the current HTML Standard, including
`invalid-processing-instruction-target`, `incorrectly-opened-comment`,
`end-tag-with-attributes`, and `end-tag-with-trailing-solidus`. Exact supported
tokenizer recovery conditions without a dedicated Standard identity use
`ParseErrorCode::TokenizerExtension`; AE13b1 currently uses this category for
Core-v0 malformed numeric character-reference recovery and the exact Core-v0
attribute-recovery paths
where Core-v0 drops or terminates at a question mark or grave accent even
though the Standard would retain it. It has no catch-all variant.

The current Borrowser-owned tokenizer-extension identities are exactly:

- `MalformedNumericCharacterReference`;
- `DroppedGraveAccentBeforeAttributeName`;
- `GraveAccentInAttributeName`;
- `DroppedQuestionMarkBeforeAttributeName`;
- `TerminatedUnquotedAttributeValueBeforeQuestionMark`.

Condition identity and recovery metadata are independent. In the Core-v0
`BeforeAttributeName` path, `=`, `"`, `'`, and `<` retain their applicable
Standard identity while `ParserRecoveryAction::DropInputCharacter` records
that Core-v0 drops the scalar instead of constructing the attribute shape
required by the Standard. Grave accent and question mark use the exact
Borrowser-owned extension identities above plus the same typed drop action.
In an unquoted attribute value, the Standard
`unexpected-character-in-unquoted-attribute-value` condition is retained while
`ReconsumeInputCharacter` records Core-v0's current terminate-and-reconsume
recovery. A question mark uses the exact Borrowser-owned terminate extension
and typed reconsume action. A slash is ordinary unquoted attribute-value data;
it is not reconstructed later as self-closing syntax.

AE13b1 currently emits the following tokenizer recovery actions:

| Recovery action | Exact production operation |
| --- | --- |
| `DropInputCharacter { code_point }` | Consume and discard that one input scalar in the current tokenizer state. |
| `ReconsumeInputCharacter { code_point }` | Leave that scalar unconsumed while transitioning state so the named next state processes the same scalar; any separately emitted literal prefix remains visible in the token stream. |
| `ReplaceInvalidInput` | Replace a consumed U+0000 in the affected token payload with U+FFFD. |
| `PreserveCharacterReferenceLiteral` | Preserve the original unsupported numeric-reference syntax as literal emitted character data; no U+FFFD is inserted. |
| `DropDuplicateAttribute` | Discard only the later duplicate attribute after normalized-name comparison; the pending tag and first attribute remain. |
| `EmitCurrentCommentAndSwitchToData` | Emit the current comment at the triggering `>` and enter Data; used for abrupt empty-comment close and `--!>` close. |
| `EmitCurrentCommentAtEof` | Emit the current comment at the terminal insertion point, then complete EOF recovery. |
| `StartBogusComment` | Start the production bogus-comment recovery from the malformed markup-declaration-open decision. |
| `RetainNestedCommentDelimiterAndReconsumeInCommentEnd { code_point }` | Keep the consumed nested `<!--` delimiter in comment data and reconsume the triggering scalar in CommentEnd. |
| `DropEndTagAttributes` | Emit the end tag while dropping only its parsed attribute tail. |
| `IgnoreEndTagTrailingSolidus` | Emit the end tag while ignoring its accepted self-closing flag. |

`recovery: None` means that AE13b1 does not represent a recovery action for
that event; it never means that no recovery occurred. Where AE13b1 does emit a
recovery action, the variant describes the production mutation rather than an
inferred or desired Standard behavior.

Surrogate numeric references report the Standard
`surrogate-character-reference` condition, and values above U+10FFFF report
`character-reference-outside-unicode-range`; both use
`PreserveCharacterReferenceLiteral` because that is the current Core-v0
production recovery. Duplicate attributes report the Standard
`duplicate-attribute` condition with `DropDuplicateAttribute`. Neither behavior
is encoded in descriptions or the lossy legacy projection.

End-tag diagnostics are produced from the pending production token state at
emission. A non-empty attribute vector produces `end-tag-with-attributes`, and
the actual self-closing flag produces `end-tag-with-trailing-solidus`.
`unexpected-solidus-in-tag` is recorded only when the live
self-closing-start-tag state consumes its non-`>` condition. No raw input tail
is rescanned to infer any of these conditions.

These are intentional production-tokenizer semantics, not observer-only
mechanics. Both start and end tags use the production attribute state machine.
A slash remains ordinary data while an unquoted attribute value is active; it
enters `SelfClosingStartTag` only after that value has ended. Consequently
`a=b/` retains the value `b/` and does not set the self-closing flag, while
`a=b />` retains `b` and does set the flag. This browser-shaped production
behavior is a prerequisite for canonical observation to report semantic token
state without reconstructing it from source text.

The tokenizer retains the exact normalized offset of the slash that enters
`SelfClosingStartTag`. A later slash replaces an earlier failed transition and
is therefore the position attached to a subsequently accepted self-closing
flag. The position is cleared when the pending tag is emitted, abandoned,
reset, or discarded by stall recovery and cannot survive into the next tag.
`current_tag_self_closing == true` requires a retained slash position; missing,
stale, or non-slash positions are typed tokenizer invariants propagated through
the parser/session invariant path. Canonical observation never substitutes the
cursor, fabricates an unavailable position, or omits the diagnostic.
The retained offset must also satisfy
`tag_name_start <= solidus_position < cursor` and reference `/` on an input
boundary. A real slash before `tag_name_start` is specifically
`SolidusPositionOutsideCurrentPendingTag`, not a valid position inherited from
an earlier tag.

Comment diagnostics are attached only to their defining production states.
CommentStartDash's ordinary anything-else transition emits no error.
`incorrectly-opened-comment` belongs only to the malformed
MarkupDeclarationOpen decision; `incorrectly-closed-comment` belongs to the
`>` consumed by CommentEndBang; and `nested-comment` belongs only to the
CommentLessThanSign → Bang → Dash → DashDash path. Arbitrary input after `--`
does not acquire a convenient Standard identity. EOF after a malformed markup
declaration or another bogus-comment entry emits that bogus comment without
inventing `eof-in-comment`; that identity remains limited to actual comment
states. EOF recovery materializes comment data according to the active comment
state, so pending `-`, `--`, or `--!` state delimiters are not accidentally
retained as data. An impossible delimiter/range relationship is a tokenizer
invariant rather than a saturating subtraction. Before trimming, the tokenizer
compares the fixed state-owned suffix exactly: CommentStartDash,
CommentEndDash, and CommentLessThanSignBangDash require `-`; CommentEnd and
CommentLessThanSignBangDashDash require `--`; CommentEndBang requires `--!`;
the remaining supported comment states require no pending delimiter. A
constant-size suffix comparison selected by live tokenizer state validates
production metadata; it is not observer reconstruction or a backwards scan.
Every active comment-family state owns a pending comment start. Missing
metadata is `CommentStateMissingPendingStart`. Present metadata is first
validated as a UTF-8-boundary-safe base range; a start after the cursor, a
cursor beyond normalized input, or an unsliceable boundary is
`CommentPendingRangeInvalid`. Only after that succeeds can a delimiter-owning
state report `CommentPendingDelimiterOutsideCurrentRange` or
`CommentPendingDelimiterDoesNotMatchState`. Delimiter-free states never enter
delimiter validation. These failures stop the production step before comment
errors, resource diagnostics, or tokens are created and never masquerade as
`NeedMoreInput` inside the state machine.

The incremental RCDATA, RAWTEXT, and script appropriate-end-tag matcher owns
the semantic evidence needed by the ordinary end-tag contract. It carries the
closing `>` position when attributes were parsed and the exact slash position
when self-closing syntax was accepted. The same `DropEndTagAttributes` and
`IgnoreEndTagTrailingSolidus` recoveries and positions are used as ordinary
end-tag emission. No observer rescans or searches the completed source tail.
Live accepted-solidus evidence and completed diagnostic evidence are validated
inside the current candidate lifetime. Attribute evidence must identify its
closing `>`, solidus evidence must identify the accepted `/`, and a retained
solidus must precede the closing position. Contradictory evidence stops
tokenization through exact typed invariants before any canonical event is
retained.
Candidate validity is independent of optional diagnostic evidence. Every
retained live or completed candidate must remain inside normalized input,
start on its active fixed `</` prefix, and complete on its consumed `>`. A
partial lone `<` can suspend tokenization but is not retained as matcher state.
Direct fixed-offset `</` validation is production-state validation, not a
reverse search or observer reconstruction. Candidate corruption is
`TextModeEndTagCandidateRangeInvalid`; attribute and solidus identities are
reserved for their corresponding optional evidence.

CDATA-end state uses the same production-state validation posture. Before
excluding its pending delimiter, the tokenizer requires parser-owned
`pending_text_start` and checks the fixed two-byte `]]` suffix inside the
pending text range. Missing ownership is
`CdataStateMissingPendingTextStart`; a valid empty CDATA section has
`pending_text_start == delimiter_start` and is not conflated with absence.
Range underflow/escape/boundary corruption and suffix mismatch have separate
exact invariants. The shared pending-text emitter reports
`PendingTextRangeInvalid` instead of treating an invalid retained span as
empty. These constant-size checks validate live tokenizer state and do not
reconstruct CDATA behavior for observation.

Pending doctype operations retain the doctype-name start used by the name
state, tail scanning, and resource-limit observation. Missing metadata
is separated into exact tokenizer invariants for those three operations. It
stops normal tokenizer progress, retains no fabricated resource diagnostic,
projects to the stable facade's existing generic invariant error, and exposes
the exact identity only through the feature-gated conformance boundary. The
same checked accessor requires `pending_doctype_name_start <= cursor`; a start
after the cursor is `DoctypeNameStartAfterCursor`, never a zero-length
saturated range. Present and ordered metadata must still form the operation's
input-aware UTF-8 range. Required name materialization rejects an empty,
out-of-input, non-boundary, or otherwise unsliceable span as
`DoctypeNameRangeInvalid`; zero length is permitted only for the explicitly
transient name-state progress operation. Invalid finalization never reports
success, emits a doctype token, or creates a resource diagnostic.
Shared ASCII-prefix scanning distinguishes mismatch and partial input from an
invalid candidate range. Quoted doctype-tail relationships use
`DoctypeTailRangeInvalid`; they are not relabeled as a doctype-name ordering
failure unless the retained name start is actually after the relevant cursor.

AE13b1 production paths touched by observation use checked invariant
propagation rather than authoritative panic fallbacks. An `expect` is retained
only where its precondition is established immediately by the same input slice
or cursor operation. One non-mutating processing-instruction classifier owns
state/metadata presence, target and data ordering, UTF-8 ranges,
state-specific shape, and emission completeness. Production preflight,
emission, EOF cleanup, and debug hardening use that classifier and therefore
cannot disagree or panic on missing or corrupt PI metadata. Classification
precedes metadata removal, token emission, bogus-comment conversion, and
diagnostic production.

Rule-defined tree-construction conditions use
stable Borrowser-owned `ParseErrorCode::TreeConstruction` variants. Serialized
code names will be stable, documented rule identities with no `other` fallback;
renaming or changing their meaning requires a format-version change or an
explicit compatibility mapping. AE13b2 derives identities from invalid parser
conditions rather than legacy debug wording, consolidates semantically
identical call sites, and removes normal implied-element/mode-transition debug
strings without replacing them with false parse errors. Recovery action
remains separate metadata.

Implementation diagnostics use payload-safe event variants. Stable codes carry
only semantic kinds: invalid UTF-8 replacement reason, exact parser resource
limit, or exact parser guardrail. Runtime values such as affected byte count,
configured limit, and consecutive stall steps live in their corresponding
typed payload. Human-readable descriptions are non-authoritative.
The configured numeric-character-reference digit bound is
`ParserResourceLimit::NumericCharacterReferenceDigits`, not a fictional WHATWG
parse error. Numeric values beyond U+10FFFF use the Standard
`character-reference-outside-unicode-range` identity.

The stable `HtmlParseEvent` facade remains deliberately lossy. Exact EOF and
character-reference conditions project to its existing broad categories;
resource limits and guardrails project to their existing categories; exact
conditions with no legacy category may project to legacy `Other`. Canonical
identity is never reconstructed from that facade, `detail`, or `aux`, and
parser decisions never depend on the projection. Modernizing the stable facade
is a separate follow-up issue. For compatibility, resource-limit `aux` remains
the configured limit clamped to `u32::MAX`, and tokenizer-stall `aux` remains
the consecutive step count clamped the same way. Canonical payloads retain the
full typed width and never read those lossy values.

Follow-up (outside AE13b1): modernize the public parser-event facade only under
a separately reviewed compatibility issue; conformance code must continue to
consume parser-owned typed events rather than the legacy buffer.

Tokenizer attributes are lexical name/value pairs in encounter order. Tree and
patch observations use a separate DOM attribute model containing namespace,
prefix, local name, and value. Trees structurally retain doctype public/system
identifiers and template contents beneath their host. Patch observations can
faithfully represent every current `DomPatch` variant and payload; only
`PatchKey` is replaced by deterministic snapshot-local labels. AE13b3 assigns
those labels in first semantic operand appearance order. Multi-key operand
order is fixed as host then contents for `CreateTemplateContents`, parent then
child for `AppendChild`, and parent then child then before for `InsertBefore`.
`Clear` neither resets label numbering nor permits parser-session key reuse.
No raw numeric `PatchKey`, runtime batch boundary, or batch version enters the
canonical patch contract. AE13b3 does not implement a patch serializer.

### Canonical parser-created tree and complete patch capture

AE13b3 separates three observation owners:

- `DocumentParseContext` records only tokenizer tokens, parse errors,
  implementation diagnostics, diagnostic occurrence sequences, and normalized
  positions.
- `PatchEmitterAdapter` optionally retains raw semantic `DomPatch` history at
  the single production boundary that receives each owned patch before
  caller-controlled drains.
- conformance execution retains the tree request until successful parser
  finish, patch validation, and `ParseOutput::document` materialization.

Tree-only and patch-only requests therefore do not install the diagnostic
recorder, build the normalized-position index, use observed token drains, or
change decoder/tokenizer execution. The final canonical tree is projected only
from the successfully materialized `ParseOutput::document`. `LiveTree`,
`PatchValidationArena` internals, legacy DOM snapshot text, and
`ParseOutput::patches` are not canonical tree or complete-history sources.

The tree projector preserves the document root, real `DocumentType` children
and all three doctype strings, text, comments, processing instructions with
separate target/data, element namespaces and exact local names, ordered
qualified attributes, ordinary children, and the typed template-contents
boundary. Template traversal order is host and attributes, ordinary children
in source order, the contents boundary, then contents children in source
order. Preflight and projection share one iterative event walker; children are
scheduled in reverse on its LIFO work stack. This removes native recursion only
from the AE13 canonical projector, not from the existing parser-owned
materializer.

Tree capacity counts canonical structural units: document, document type,
element or HTML template host, text, comment, processing instruction, and the
typed template-contents boundary each consume one unit. Ordered attributes and
the outer `ObservedTree` wrapper consume no structural units, although their
owned storage remains checked and fallible. A tree is atomic. Insufficient
capacity returns an empty, clearly `Incomplete` tree with the exact required
unit count dropped; it never returns a recursively truncated tree. The complete
iterative preflight validates materialized tree invariants before capacity can
produce `Incomplete`. Patch capacity counts semantic operations and retains
the exact original prefix with an exact dropped count. Neither capacity is a
byte-memory budget. Individual strings and attribute payloads may be large;
every newly owned nested string, vector, attribute payload, label, map, history
set, and traversal stack uses checked fallible allocation. A byte-budget
request model is outside AE13b3.

Preflight requires exactly one document root, rejects the legacy document-level
doctype compatibility field, and requires every HTML-namespace `template` to
own a `TemplateContents` fragment of the correct kind. Foreign SVG/MathML
elements whose local name is `template` remain ordinary elements. These
contradictions have typed observation-invariant identities and take precedence
over every capacity outcome.

Live patch-history allocation failure terminalizes the parser through
`ParserFatalError::ResourceExhaustion(PatchHistoryObservationStorage)`. A
patch-history dropped-count contradiction terminalizes stable parsing as the
existing `EngineInvariant`; feature-gated conformance state retains and
specializes the exact `PatchDroppedCountOverflow` identity. The adapter is
checked immediately after every tree-builder token call, even when that call
also returns a fatal error, and the synchronously latched capture failure has
precedence. The original patch still enters ordinary transport unchanged, but
the failed session exposes no drain, document mode, materialized output, or
observation.

Canonical tree/patch allocation occurs after successful production parsing and
materialization, so it reports typed observation resource exhaustion at
`CanonicalTreeProjection`, `CanonicalPatchProjection`, or
`SnapshotLabelStorage`, never a false parser failure. Arithmetic and semantic
contradictions remain typed observation invariants. Any failure suppresses the
entire `CanonicalParserResult`.

Retained patch prefixes are checked in release builds. Create operations
introduce fresh non-zero keys; `CreateTemplateContents` requires its host and
introduces contents; every structural/content operation requires retained
creation history. Duplicate creation, `PatchKey::INVALID`, or a reference
without retained creation history is an execution invariant. `Clear` resets
live structure only and preserves historical creation identity.

Transition token summaries, insertion modes, dispatch paths, and parser-context
token kinds are typed semantic values. Unsupported-feature observations are
limited to encountered preprocessing, tokenizer, and tree-construction behavior.

Document and fragment declarations default omitted `scripting` to the concrete
validated state `disabled`. Fragment namespaces validate exhaustively to
`html::ElementNamespace::{Html, Svg, MathMl}`; arbitrary strings cannot enter a
validated fragment context.

Final-invariant fields carry only `Satisfied`, typed `NotApplicable`, or
`Failed`. A failed field cannot select its own error code. Exhaustive
field-by-field collection assigns the stable `InvariantFailureCode`, so adding a
mandatory field requires updating collection and preserves deterministic field
order. AE13c supplies the production-owned execution and aggregation described
below.

### Normalized parser positions

Canonical event positions use the production parser's normalized input space,
not a fixture-only coordinate system. `utf8_byte_offset` is zero-based and
counts bytes in the decoded UTF-8 string after CR/LF preprocessing. `line` and
`column` are one-based; column counts Unicode scalar values rather than UTF-8
bytes, UTF-16 code units, or grapheme clusters. A non-EOF position identifies
the point immediately before the normalized scalar that triggered the event.
EOF identifies the terminal point after the last normalized scalar.
`invalid-first-character-of-tag-name` identifies the invalid following scalar,
not the preceding `<`. Every canonical EOF identity uses
`input.as_str().len()` after preprocessing as that terminal insertion point,
including tag, doctype, comment, CDATA, processing-instruction, and supported
text-mode EOF paths.

CRLF and lone CR each become one LF before coordinates are assigned. That LF
occupies the current line and scalar column; the following scalar starts the
next line at column 1. Invalid UTF-8 subsequences decoded from a raw-byte
fixture contribute their resulting U+FFFD scalar to normalized coordinates:
three normalized UTF-8 bytes and one scalar column. UTF-8 carry and pending CR
state must therefore produce identical positions for whole and chunked delivery.

These coordinates cannot identify original source bytes after decoding and
newline normalization. `SourceBytePosition` must remain
`Unavailable(NoInputProvenanceMap)` unless a separate exact provenance map is
introduced. AE13b1 adds no such map and performs no approximate offset
reconstruction. Its observation-only normalized-position index is incremental,
checkpointed, and allocated only while a requested position-bearing diagnostic
surface can retain another event. It is disabled once all such surfaces are
full. Arithmetic exhaustion or index discontinuity is a typed observation
invariant failure; it is not converted to a saturated coordinate. A production
path that genuinely cannot provide a position records an explicitly
unavailable position. A supplied offset beyond the normalized input or inside
a UTF-8 scalar is instead
`ParserObservationInvariant::InvalidNormalizedPositionOffset` and fails the
canonical execution; it is never mislabeled as an unavailable position. The
recorder models known, genuinely unavailable, and invariant-failure outcomes
separately. If resolution fails after an occurrence was reserved, no
parse-error or implementation-diagnostic item is retained for that occurrence;
the fatal invariant makes the partial capture unusable.
`ParserDidNotProvidePosition` is reserved for an event source that genuinely
has no supported position.
`NormalizedPositionIndexMissing` means recorder corruption only: a retaining
reservation required position conversion while the index should still have
existed. Intentional index retirement after both position-bearing surfaces
lose retaining capacity cannot trigger it, because no later event is resolved
after an unsuccessful reservation.

The UTF-8 decoder has one incremental state machine for chunks, carry
continuation, EOF, event-aware decoding, and string-only compatibility wrappers.
Production carry is a fixed-size `Utf8DecoderState`, owned by
`ByteStreamDecoder`; only the decoder can construct its zero-to-three-byte
validated truncated prefix. It allocates no carry `Vec`. Bytes that disprove
that prefix are reprocessed. `IncompleteSequenceAtEof` therefore identifies
only a genuinely truncated valid prefix; other malformed prefixes produce
ordered `InvalidSequence` replacements with typed affected-byte counts. The
legacy `Vec<u8>` helpers are adapters: they revalidate arbitrary incoming carry
through that same state machine and export only validated state. Literal U+FFFD
is ordinary input and never creates a decoder diagnostic.

Decoder replacement accounting is mandatory parser work:
`decode_errors` increments once for each decoder-generated U+FFFD after the
scalar is appended, independent of whether implementation diagnostics are
unrequested, zero-capacity, or exhausted. Canonical reservation and retention
are optional work after that accounting and cannot affect the counter.

## Dispositions

Fixture-v1 supports active, expected-unsupported, expected-failure, and skipped
dispositions. Non-active dispositions require a non-empty reason, an exact
typed capability or failure classification, and a tracking or provenance
reference. One evaluator owns policy for every disposition. Unsupported fixture
semantics, unsupported expectations, execution failures, expectation mismatches,
and invariant failures use exact typed matching; unexpected success is an XPASS
failure. Unsupported-capability skips retain the exact capability.

Before a skipped disposition can enter the sealed validated model, the
declaration boundary proves that its exact capability is relevant to semantics
the fixture actually declares. Relevance is derived from validated input,
target and scripting state, every declared delivery plan, enabled expectation
surfaces, and the exact set of unknown required extension IDs. An unrelated
capability substitution is a malformed disposition, even when that capability
is generally permitted for external fixtures. Capability relevance and the
completed-capability registry are separate requirements: a capability must be
both relevant and permitted to use a non-active disposition.

All declared delivery plans count as fixture semantics for this check. The
reference delivery selects the ordinary comparison baseline, and a transition
expectation may select another declared plan; neither role makes other declared
plans irrelevant. `byte-delivery` requires a byte-unit delivery,
`unicode-scalar-chunking` requires a Unicode-scalar boundaries plan, and an
expectation capability requires that exact surface to be declared.

`unsupported-capability` is the only fixture-v1 skip classification. Broad
external-source and environment skips are deliberately absent: upstream records
that are duplicate, malformed, unlicensed, or outside an imported profile must
be reported by the future adapter rather than represented as passing skipped
parser fixtures. The completed-capability registry rejects use of the retained
skip to hide completed token or document behavior.

The runner may bypass a skipped fixture only because it accepts a sealed
`ValidatedFixtureSpec` whose skip relevance has already been established. It
does not duplicate capability-relevance policy. External-source import
exclusions remain future adapter-report concerns rather than parser-fixture
skips.

Native fixtures in `crates/html/tests/fixtures/html5/conformance/` must be
active. Non-active dispositions are reserved for later external/adapted inputs
or an explicitly identified quarantine source. Completed Milestone AE behavior
must not be hidden behind a non-active disposition.

An `unsupported_features` observation means a parser limitation was encountered
during an otherwise supported parse. It is distinct from an unsupported fixture
target, delivery mechanism, or required extension.

## Extensions

Extensions use versioned namespaced IDs and strict declarations containing
`required` plus a TOML value. Unknown required extensions produce
`UnsupportedFixtureSemantics`. Unknown optional extensions are retained only as
non-semantic metadata and cannot alter core fixture behavior.

AE13a intentionally has no speculative generic adapter registry. A real known
semantic extension must later receive an exact-version typed adapter at the
single validation boundary.

## AE13b5 snapshot and runner status

Fixture v2 activates `html5-token-v2`, `html5-parse-errors-v1`,
`html5-implementation-diagnostics-v1`, `html5-document-mode-v1`,
`html5-dom-v3`, `html5-dompatch-v3`, `html5-tree-transitions-v1`, and
`html5-unsupported-features-v1`. Writers consume only typed
`CanonicalParserResult` surfaces. The native standalone token path no longer
runs `TokenFmt` and canonical observation as two semantic executions.

Fixture v1 and `html5-token-v1` remain isolated compatibility contracts. Their
accepted syntax and scalar failure identities are unchanged.

Every declared delivery is capability-checked. Only reference and
transition-selected deliveries are planned; requests are unioned per delivery
and each planned delivery executes once in declaration order. No comparison
begins until every execution succeeds and every requested observation is
authoritative. Completed reports are all-or-nothing.

Private fixture guardrails are expectation-independent. Canonical tree capacity
counts document, document type, element/template host, text, comment,
processing instruction, and template-contents boundary. Attributes and the
outer `ObservedTree` wrapper do not consume units. Incomplete prefixes never
reach serialization or comparison. The private policy is injected through
request construction so tests exercise exact and capacity-plus-one boundaries;
fixture declarations and sidecars cannot configure it. Incomplete diagnostics
retain exact delivery, surface, reason, retained count, and dropped count.

Canonical tree snapshot writing and framing validation are iterative. The
writer preserves canonical preorder and the template ordinary/content split
without recursive descent. The reader enforces only physical traversal
framing, including immediate owner attributes and one contents boundary per
template host, while remaining deliberately agnostic about parser and namespace
correctness. Lexical paths accept repeated complete `/contents` segments for
nested template hosts; the independent framing stack proves which host owns
each boundary and rejects consecutive boundaries without an intervening host.
Canonical patch labels are quoted `node-<positive-canonical-decimal>` values,
and all patch ordinals/attribute indices use canonical decimal spellings.

Fixture-v2 declaration validation inspects expected-sidecar metadata without
opening or reading the complete content. Non-skipped v2 execution reads content
only in the ordered expected-sidecar phase. A skipped v2 disposition therefore
performs zero sidecar-content reads as well as zero parser executions and
exposes no canonical result. Fixture-v1 preserves its frozen legacy boundary:
validation fully reads declared sidecars, including for skipped fixtures, and
the legacy token runner rereads its sidecar during execution.

Surface-specific parsed/canonical snapshot types seal each format at compile
time. V2 completed reports store each delivery result once and borrow the
reference result through the compatibility accessor. One authoritative
failure-spelling codec owns structured declaration parsing and stable identity
formatting.

See `parser-fixture-format-v2.md` and
`ae13b5-parser-snapshot-formats.md` for normative grammar, precedence, and
diagnostics.

## Slice status

- AE13b1: parser-owned token and tokenizer-diagnostic observation foundation.
- AE13b2: tree-construction diagnostics and production document-mode capture.
- AE13b3 and AE13b4: canonical tree/patch/transition/unsupported observations
  are complete. AE13b5 strict serializers and fixture diagnostics are
  implemented and its architecture review is complete.
- AE13c: semantic whole/chunked parity and production final-invariant execution
  are implemented in the current slice.
- AE13d: existing corpus consolidation and migration.
- AE13e: external html5lib/WPT adapter, intentional snapshot updates, final
  documentation, and CI coverage expansion. The cumulative AE13 parent
  architecture review is complete; `make ci` and
  `make test-html5-external-fixtures-extended` both pass.

Fragment execution, scripting-dependent parsing, original source-byte
provenance, Layout, Paint, JavaScript execution, navigation, and resource
loading are not implemented by AE13b5.

## AE13c whole/chunk parity and final audits

AE13c keeps one parser observation boundary. `html` owns compact delivery,
decoder/tokenizer/tree execution, terminal audits, patch draining and trusted
application, materialization, and structural/semantic DOM comparison.
`html-test-support` owns fixture-v2 validation policy, strategy planning,
scheduling, canonical serialization, comparison, disposition, and diagnostics.
Test support never reads tokenizer/tree-builder internals and never replays
patches or reconstructs HTML scope, namespace, template, or recovery rules.

Fixture-v1 and fixture-v2 select a closed validation policy once. V1 retains
its legacy whole-tokenizer path: it generates no representative strategies,
does not request final invariants, and performs no AE13c parity. V2 requires an
authoritative domain baseline and requests final invariants for every supported
execution, whether or not a `final_invariants` sidecar is declared.

### Strategy semantics and order

UTF-8 text uses whole Unicode input as its baseline. Unicode deliveries use
Unicode-scalar ordinals; byte deliveries use exact UTF-8 byte offsets and may
split a code point. Whole bytes are an additional decoder-entry candidate.
Raw-byte input uses untouched whole bytes as its baseline and never passes
through a test-support string decoder or sanitizer.

`Whole`, `Fixed`, and `Explicit` are compact storage/execution shapes, not
semantic identities. Equality compares transport, coordinate space, exact
input extent, and the exact yielded interior semantic boundaries. Fixed plans
compare arithmetically and never expand to an offset vector. Whole,
fixed-with-no-boundary, and explicit-empty plans alias. Scalar ordinals resolve
to execution-only UTF-8 byte offsets by one checked forward traversal; derived
offsets do not participate in identity or diagnostics.

Execution order is fixed:

1. the authoritative baseline, ordinal 1;
2. unique declared non-baseline strategies by first semantic appearance in
   declaration order; and
3. representative-only strategies in generator order: whole bytes where
   applicable, then fixed-one, fixed-seven, and edge-triplet scalar/byte forms.

Later aliases add ordered origins without moving a strategy. Baseline,
declared names, and representative names are roles, not mutually exclusive
identities. Empty and one-unit inputs produce no zero-length push; collapsed
representatives become aliases. Completed reports retain one result per
required semantic strategy and expose ordered declared aliases and the checked
contiguous fixture-local ordinal.

Fixture-v2 privately limits declared deliveries to 32, boundaries per declared
delivery to 4,096, and unique planned strategies to 24. These are validation
policy, not parser or input-size limits. Excess is rejected before execution;
no required strategy is truncated. Runtime is
`O(input length * bounded unique strategy count)`. Fixed delivery has `O(1)`
strategy memory independent of chunk count. Explicit and edge strategies keep
only bounded offsets. Peak retention is one typed baseline, one current
candidate, bounded failure summaries, applicable expectation snapshots, and
only the selected baseline/candidate snapshots for a parity diagnostic;
successful parity-only candidates are dropped.

### Parity surfaces and precedence

Every applicable surface is compared in this order: tokens, parse errors,
implementation diagnostics, document mode, tree, patches, transitions,
unsupported features, and final invariants. Canonical values and codecs own
comparison. Raw parser IDs, allocation identity, patch transport batches,
source chunk shapes, hash iteration, and Rust `Debug` are transport or
implementation metadata and are not semantic parity surfaces.

The baseline executes first. A parser/fatal/resource/delivery failure,
incomplete observation, or failed final invariant stops immediately, before
sidecar content parsing or candidate execution. After a valid baseline, the
first strategy-ordered parser execution failure returns immediately. The
runner retains at most the first incomplete observation, first final-audit
failure, first parity mismatch, and first expectation mismatch, then selects
in that order. Incomplete or final-audit-failing candidates are never compared.
No failure exposes a partial completed report.

A parity diagnostic contains fixture ID, repository-relative path, strategy
ordinal, transport, semantic coordinates, exact extent and boundaries, ordered
origins, canonical surface, and the first meaningful record difference.
Equality never depends on a digest. A production `InvalidDelivery` after v2
validation maps by typed identity to
`validated-boundary-rejected-by-executor`; messages are not classified.

### Finalization lifecycle

`FinalInvariantRequest` is independent of bounded collection requests.
Standalone tokenizer runs report five input/tokenizer fields and mark eleven
document/tree/DOM/patch fields
`NotApplicable(StandaloneTokenizerRun)`. Document runs aggregate a
production-owned pre-materialization session/tree audit with a post-drain,
post-materialization parser-session audit. Parser fatal errors, audit allocation
failure, checked overflow, patch failure, and materialization failure are
execution failures with no partial report.

The patch witness exists only for final-audit capture. It records builder
pending patches after finish, checked counts for each exact drained batch,
successful trusted application of that same complete batch, terminal `None`,
builder/emitter pending counts after terminal drain, and materialization after
that drain. Trusted application is deliberately in-place and
non-transactional: on failure the private partially updated arena is discarded
immediately and is never inspected, materialized, audited, or exposed.

`live_tree_matches_materialized_dom` is a compound semantic end-state check. A
fallible `LiveTree` structural projection must equal the fully applied patch
arena projection, and a complete semantic patch-arena traversal must equal the
materialized DOM. Semantic comparison covers document legacy doctype data,
doctype fields, expanded element names, ordered qualified attributes, text,
comments, processing instructions, ordinary children, and template contents.
It is iterative with one frame per active ancestor, so temporary storage is
`O(tree depth)`, not `O(sibling count)`.

### AE13e external adapter and closeout boundary

The external proof follows the same canonical validation, normalized
execution, and production observation path as native fixtures. The adapter
understands only the pinned WPT .dat record format and emits fixture-v3
declarations; it does not execute HTML, compare trees, or construct a
ValidatedFixtureSpec directly. External provenance and licence metadata are
validated by the fixture-v3 boundary.

WPT error text remains provenance. WPT #errors and #new-errors are represented
as an exact parse-error count where the source defines only a count. Native
fixtures retain exact typed parse-error snapshots. Missing scripting markers,
fragment records, and valid external expectations that the current canonical
representation cannot express without inventing precision are classified
explicitly and are not counted as passes. The latter uses
unsupported-expectation-representation; it does not claim that the production
parser lacks the corresponding behavior. Unsupported-parser-feature is reserved
for a genuine production parser capability gap. Browser-wrapper and platform
requirements such as `document.write`, DOM bindings, events, navigation,
resources/networking, and rendering remain future exclusions; the raw adapter
does not infer them from literal HTML input.

AE13e closes the scoped static tokenizer/tree-construction/parser-created-DOM
conformance harness foundation. It does not claim complete html5lib, complete
WPT, complete HTML, DOM API, scripting, or event-loop conformance.
