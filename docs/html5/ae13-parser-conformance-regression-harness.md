# AE13 Parser Conformance and Regression Harness

## Status

This contract begins in AE13a. AE13a defines the canonical fixture and result
models, deterministic discovery and validation, exact input handling, shared
document-mode ownership, disposition policy, and one standalone-tokenizer
fixture. AE13b1 adds the parser-owned observation foundation for tokens,
tokenizer parse errors, and implementation diagnostics. It does not claim
completion of overarching AE13.

## Ownership

The `html` crate owns parser semantics and typed canonical observation values.
`html::DocumentMode` is the one document-mode type used by tree construction
and future parser observations. The exact token, position, parse-error, and
implementation-diagnostic semantic identities live in always-compiled
HTML/parser ownership. `html::conformance` re-exports those same types and owns
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
`ValidatedFixtureSpec`; public consumers receive read-only accessors. A future
external adapter must enter through the same declaration and validation path.
It must not construct validated values directly.

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
the explicit text-mode EOF control recovery, Core-v0 malformed numeric
character-reference recovery, and the exact Core-v0 attribute-recovery paths
where Core-v0 drops or terminates at a question mark or grave accent even
though the Standard would retain it. It has no catch-all variant.

AE13b1 Borrowser-owned tokenizer-extension identities are exactly:

- `EofInTextMode`;
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

Rule-defined tree-construction conditions without dedicated standard codes use
stable Borrowser-owned `ParseErrorCode::TreeConstruction` variants. Serialized
code names will be stable, documented rule identities with no `other` fallback;
renaming or changing their meaning requires a format-version change or an
explicit compatibility mapping. AE13b1 migrates tokenizer-owned call sites to
the exact production taxonomy; tree-builder diagnostics remain later work.
Recovery action remains separate metadata.

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
`PatchKey` is replaced by caller-supplied snapshot-local labels. AE13a does not
assign stream labels or implement the patch-v3 serializer.

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
order. Production execution of these checks remains AE13c work.

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

## Snapshot format status

- `html5-token-v1`: executable AE13a compatibility format.
- `html5-dom-v2`: existing compatibility tree format.
- `html5-dompatch-v2`: existing compatibility patch format.
- `html5-dompatch-v3`: planned native AE13 patch format using labels assigned by
  first semantic appearance and no normative transport batch boundaries.
- Native parse-error, implementation-diagnostic, document-mode, transition,
  unsupported-feature, and final-invariant formats are reserved/planned for
  AE13b through AE13e. AE13a does not implement or claim stability for them.

The `html5-token-v1` reader lives beside the existing token formatter and emits
dedicated typed snapshot-format errors. Malformed snapshots are not reported as
fixture-TOML errors.

## Later slices

- AE13b1: parser-owned token and tokenizer-diagnostic observation foundation.
- AE13b2 through AE13b5: remaining parser observations, shared escaping, and
  stable serializers.
- AE13c: semantic whole/chunked parity and production final-invariant execution.
- AE13d: existing corpus consolidation and migration.
- AE13e: external html5lib/WPT adapter, intentional snapshot updates, final
  documentation, and CI coverage expansion.

Fragment execution, scripting-dependent parsing, original source-byte
provenance, Layout, Paint, JavaScript execution, navigation, and resource
loading are not implemented by AE13a.
