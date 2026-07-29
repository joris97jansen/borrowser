# HTML5 Core v0 Supported Subset Contract (Milestone D3)

Last updated: 2026-07-05
Scope: `crates/html/src/html5` (feature `html5`)
Normative matrix sources:
- `docs/html5/spec-matrix-tokenizer.md`
- `docs/html5/spec-matrix-treebuilder.md`
- `docs/html5/dompatch-contract.md`
- `docs/html5/node-identity-contract.md`
- `docs/html5/ae1-html-parser-dom-ownership-contract.md`
- `docs/html5/ae2-parser-created-dom-node-model.md`
- `docs/html5/ae7-body-mode-recovery-contract.md`
- `docs/html5/ae8-specialized-table-tree-construction-contract.md`
- `docs/html5/ae9-form-tree-construction-contract.md`
- `docs/html5/ae9b-current-select-tree-construction-contract.md`
- `docs/html5/ae10-template-tree-construction-contract.md`

Related text-mode hardening note:
- `docs/html5/rawtext-script-stability.md`

## Purpose

This document defines the normative supported subset for `HTML5 Core v0`.
It is the contract for:

- what behavior is guaranteed,
- what behavior is explicitly deferred or out-of-scope,
- how unspecified input is handled safely, and
- what parity criteria are required to promote to `v1`.

Any behavior not listed as supported here is non-contractual and must not be relied on by runtime or tests.

## Normative Language

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are normative.

<a id="tier-mapping-and-id-authority"></a>
## Tier Mapping And ID Authority

- Tier authority remains in the matrix documents.
- Core v0 includes:
  - all items tagged `MVP` in the tokenizer/tree-builder matrices,
  - only the explicitly listed `MVP_PARTIAL` items in this contract.
- `DEFERRED` and `OUT_OF_SCOPE` items are excluded from Core v0 guarantees unless this contract explicitly defines a fallback behavior (for example, table robustness).
- Stable IDs referenced here (`TOK-*`, `TB-*`) are defined by the `ID` columns in:
  - `docs/html5/spec-matrix-tokenizer.md`
  - `docs/html5/spec-matrix-treebuilder.md`
- Consistency rule: every `TOK-*` or `TB-*` ID referenced in this contract MUST exist verbatim in those matrix `ID` columns; missing or mismatched IDs are contract drift and MUST be fixed in the same change set.

## Contract Boundaries

- Tokenizer (`html5/tokenizer`) owns tokenization state machines, token emission, tokenizer-level normalization, and parse-error recovery.
- Tree builder (`html5/tree_builder`) owns insertion modes, SOE/AFE structures, tree-construction algorithms in scope, and `DomPatch` emission.
- Session (`html5/session`) owns streaming orchestration, pumping, and policy-level test classification integration.
- AE1 defines the broader parser-created DOM and downstream-consumer boundary:
  browser/runtime, CSS, Layout, and Paint consume documented parser output and
  must not depend on tokenizer states, insertion modes, parse-error recovery
  internals, or parser-created identity as retained render identity.
- AE2 defines the parser-created DOM node model, including `DocumentType` as a
  real internal node and first-wins deterministic attribute storage.

## Guaranteed Support In Core v0

Core v0 guarantees behavior for the following scope.

<a id="input-and-streaming-model"></a>
### Input And Streaming Model

- Input model is UTF-8/scalar text streaming through session/tokenizer
  (`push_input` + `finish` flow).
- Current byte input assumes UTF-8. Invalid UTF-8 and incomplete final UTF-8
  prefixes are replaced with `U+FFFD`; full byte-stream encoding sniffing,
  charset detection, BOM switching, and legacy encodings are deferred.
- One authoritative incremental decoder owns ordinary chunks, validated
  truncated carry, carry resolution, EOF finalization, event-aware decoding,
  and compatibility string output. A byte that invalidates a carried prefix is
  reprocessed; only a valid truncated scalar prefix produces
  `IncompleteSequenceAtEof`.
- Current string/scalar input preprocessing normalizes CRLF and lone CR to LF.
  Split CRLF across chunks MUST be chunk-equivalent.
- Whole-input and chunked-input execution MUST be semantically equivalent for all Core v0 gate tests.
- Parser state machines MUST be resumable at chunk boundaries without token duplication or token loss.
- Supported tokenizer states that encounter `U+0000` MUST record a tokenizer
  `UnexpectedNullCharacter` parse error and emit deterministic replacement
  text for the affected token payload.
- Decoder-generated U+FFFD increments `decode_errors`; literal U+FFFD does not.
  Feature-gated observation records a typed invalid/incomplete reason,
  affected-byte payload, and normalized position without affecting decoding.

<a id="tokenizer-state-families"></a>
### Tokenizer State Families

Core v0 tokenizer support includes:

- Data and tag primitives:
  - `TOK-STATE-DATA`
  - `TOK-STATE-TAG-OPEN`
  - `TOK-STATE-END-TAG-OPEN`
  - `TOK-STATE-TAG-NAME`
- Attribute parsing:
  - `TOK-STATE-BEFORE-ATTR-NAME`
  - `TOK-STATE-ATTR-NAME`
  - `TOK-STATE-AFTER-ATTR-NAME`
  - `TOK-STATE-BEFORE-ATTR-VALUE`
  - `TOK-STATE-ATTR-VALUE-DQ`
  - `TOK-STATE-ATTR-VALUE-SQ`
  - `TOK-STATE-ATTR-VALUE-UQ`
  - `TOK-STATE-AFTER-ATTR-VALUE-QUOTED`
  - `TOK-STATE-SELF-CLOSING-START-TAG`
- Comments and declarations:
  - `TOK-STATE-MARKUP-DECL-OPEN`
  - `TOK-STATE-COMMENT-CORE`
  - `TOK-STATE-BOGUS-COMMENT`
- DOCTYPE:
  - `TOK-STATE-DOCTYPE`
  - `TOK-STATE-BEFORE-DOCTYPE-NAME`
  - `TOK-STATE-DOCTYPE-NAME`
  - `TOK-STATE-AFTER-DOCTYPE-NAME`
  - `TOK-STATE-BOGUS-DOCTYPE`
- Character references (`MVP_PARTIAL` scope):
  - `TOK-STATE-CHARREF-ENTRY`
  - `TOK-STATE-CHARREF-NAMED`
  - `TOK-STATE-CHARREF-AMBIGUOUS-AMP`
  - `TOK-STATE-CHARREF-NUMERIC`
- Text-mode tokenizer subset (`MVP_PARTIAL` scope):
  - `TOK-STATE-RAWTEXT`
  - `TOK-STATE-RAWTEXT-END-TAG`
  - `TOK-STATE-RCDATA`
  - `TOK-STATE-RCDATA-END-TAG`
  - `TOK-STATE-SCRIPT-DATA`

Character references are guaranteed only in:

- Data text context.
- Attribute value contexts (`DQ`, `SQ`, `UQ`).
- RCDATA text for supported `title` and `textarea` text-mode containers.
- This character-reference scope is `MVP_PARTIAL`; AE5 makes the active
  behavior deterministic without claiming full WHATWG character-reference
  parity.
- For those contexts, Core v0 guarantees named and numeric decoding behavior
  exactly as implemented by the explicit tokenizer-context API in
  `crates/html/src/entities.rs`; divergence from that module behavior is an
  in-scope Core v0 bug.
- The active named subset is semicolon-terminated `amp`, `lt`, `gt`, `quot`,
  `apos`, and `nbsp`.
- Unknown semicolon-terminated named references, supported names missing their
  semicolon, malformed numeric references, overlong numeric references, and
  invalid numeric scalar values remain literal and record deterministic
  tokenizer-owned `InvalidCharacterReference` diagnostics.
- RAWTEXT and script-data contexts preserve entity-looking text literally and
  must not call the character-reference decoder.
- Legacy semicolon-less behavior is not part of the active default tokenizer
  policy except for deterministic literal recovery diagnostics for supported
  names missing a semicolon.
- Deferred/explicitly out of Core v0 charref parity (unless already covered by `entities.rs` behavior):
  - full WHATWG context-sensitive semicolonless named-reference rules in every edge context,
  - attribute-specific ambiguous-ampersand branches beyond the implemented subset,
  - guaranteed full named-entity table parity beyond the currently active
    minimal decoder behavior.

<a id="tree-builder-modes-and-algorithms"></a>
### Tree Builder Modes And Algorithms

Core v0 tree-builder support includes:

- Insertion modes:
  - `TB-MODE-INITIAL`
  - `TB-MODE-BEFORE-HTML`
  - `TB-MODE-BEFORE-HEAD`
  - `TB-MODE-IN-HEAD` (`MVP_PARTIAL`)
  - `TB-MODE-AFTER-HEAD`
  - `TB-MODE-IN-BODY`
  - `TB-MODE-AFTER-BODY`
  - `TB-MODE-AFTER-AFTER-BODY`
  - `TB-MODE-IN-TABLE` (`MVP_PARTIAL`)
  - `TB-MODE-IN-TABLE-TEXT` (`MVP_PARTIAL`)
  - `TB-MODE-IN-CAPTION` (`MVP_PARTIAL`)
  - `TB-MODE-IN-COLUMN-GROUP` (`MVP_PARTIAL`)
  - `TB-MODE-IN-TABLE-BODY` (`MVP_PARTIAL`)
  - `TB-MODE-IN-ROW` (`MVP_PARTIAL`)
  - `TB-MODE-IN-CELL` (`MVP_PARTIAL`)
  - `TB-MODE-IN-TEMPLATE` (`MVP_PARTIAL`)
- Algorithms/invariants:
  - `TB-ALGO-REPROCESS`
  - `TB-ALGO-SOE`
  - `TB-ALGO-AFE` (`MVP_PARTIAL`)
  - `TB-ALGO-AAA` (`MVP_PARTIAL`, supported formatting-element subset only)
  - `TB-ALGO-QUIRKS-DOCTYPE`
  - `TB-ALGO-PATCH-SINK`
  - `TB-ALGO-FOSTER` (`MVP_PARTIAL`)
  - `TB-ALGO-TABLE-CONSTRUCTION` (`MVP_PARTIAL`)
  - `TB-ALGO-TEMPLATE-MODES` (`MVP_PARTIAL`)

Core v0 tree-builder partial-scope guards:

- `TB-ALGO-AFE` (`MVP_PARTIAL`) guarantees basic AFE marker handling and
  reconstruction for the supported formatting-element subset.
- `TB-ALGO-AAA` (`MVP_PARTIAL`) is limited to Borrowser's supported
  formatting-element set and representative deterministic recovery fixtures.
  This does not claim full WHATWG adoption-agency conformance.
- `TB-ALGO-REPROCESS` guarantees that reprocessing reuses the same token instance and does not emit duplicate patches for a single logical token unless explicitly required by the spec algorithm.

### Text Coalescing Policy (Core v0)

- Tree-builder text coalescing is controlled by `TreeBuilderConfig::coalesce_text`.
- When enabled, coalescing is deterministic and parent-local:
  - first adjacent text insertion under a parent emits `CreateText` then `AppendChild`,
  - subsequent adjacent text insertions under the same parent emit `AppendText` on that same text-node key.
- Coalescing MUST stop on any structural boundary, including:
  - document materialization (`CreateDocument`),
  - element insertion,
  - successful SOE pop/end-tag closure,
  - comment insertion,
  - recovery literalization boundaries.
- Batch/chunk boundaries MUST NOT change semantic coalescing behavior:
  - whole-input and chunked-input runs must converge to the same final DOM,
  - patch logs must remain deterministic under different drain boundaries.

<a id="supported-tags-and-contexts-baseline"></a>
### Supported Tags And Contexts Baseline

Core v0 guarantees the following tag/context baseline:

- Document bootstrap context:
  - implicit or explicit `html`, `head`, and `body` routing in early insertion modes.
  - EOF in `Initial`, `Before html`, `Before head`, `In head`, and `After head`
    creates the supported implicit document shell deterministically.
  - missing `html`, `head`, and `body` shell construction records
    deterministic tree-builder diagnostics in debug fixture output.
- Head context (`TB-MODE-IN-HEAD`, partial):
  - guaranteed routing for `meta`, `link`, `base`.
  - guaranteed minimal `title` routing behavior.
  - comments and whitespace handling in head flow.
- Body context (`TB-MODE-IN-BODY`):
  - character insertion, comment insertion, and generic element insertion path for non-deferred constructs.
  - AE7 supported body-mode malformed-content recovery for the narrow subset
    defined in `docs/html5/ae7-body-mode-recovery-contract.md`:
    supported implied-end-tag generation for `p` and `li`, paragraph
    auto-close before supported block starts, nested/open `p` recovery,
    unmatched `</p>` synthesis through normal parser-created element insertion,
    and sibling `li` recovery.
  - supported `body`/`html` end-tag handling transitions through explicit
    `After body` and `After after body` modes for normal static documents.
  - comments after `</body>` are inserted under the `html` element; comments
    after `</html>` are inserted under the document node.
  - AE8 supported table-family insertion-mode behavior for `table`,
    `caption`, `colgroup`, `col`, `tbody`, `thead`, `tfoot`, `tr`, `td`,
    and `th`, including supported implied wrappers, table-text buffering,
    malformed table recovery, and foster parenting as defined by
    `docs/html5/ae8-specialized-table-tree-construction-contract.md`.
  - AE9b current full-document `select`, `option`, and `optgroup` behavior runs
    through shared `InBody` rules and existing table delegation. It includes
    select scope boundaries, the supported select-family implied-end subset,
    select-aware `input`/`hr` recovery, and the generic special-barrier end-tag
    scan defined in `docs/html5/ae9b-current-select-tree-construction-contract.md`.
  - unknown or unsupported elements outside table-family MUST follow the generic element insertion path defined by `TB-MODE-IN-BODY`.
  - AE10 `template` is not an unknown/generic element. It owns a typed
    parser-created contents fragment and uses the explicit template insertion-
    mode stack defined by
    `docs/html5/ae10-template-tree-construction-contract.md`.
  - the unknown/unsupported-element rule applies only where a later declared-
    scope contract does not define specialized behavior.

This is a context-level baseline, not a full tag-by-tag HTML5 completion claim.

<a id="attribute-rules-baseline"></a>
### Attribute Rules Baseline

Core v0 attribute behavior guarantees:

- tag and attribute names are tokenizer-normalized per HTML tokenizer rules (ASCII case folding in relevant states).
- parser-created attribute storage preserves first-wins encounter order after duplicate removal.
- duplicate attributes are removed before downstream consumers and snapshots with first-wins semantics on tokenizer-normalized attribute names (for example `a` and `A` are treated as duplicates).
- the tokenizer reports `duplicate-attribute` with
  `DropDuplicateAttribute`: only the later attribute is discarded; the pending
  tag is still emitted and the first attribute remains in its token, DOM node,
  and creation patch.
- valueless and explicitly empty syntax both materialize with DOM string value
  `""`; any source-spelling distinction is tokenizer diagnostic information,
  not parser-created DOM, patch, CSS, or snapshot state.
- tokenizer, DOM, and patch snapshots serialize the normalized string value;
  the current tokenizer retains no missing-versus-empty source-syntax bit.
- value forms supported: double-quoted, single-quoted, unquoted.
- character references in attribute values follow Core v0 charref scope and delegate named reference table/validation to `crates/html/src/entities.rs`.

AE11 adds typed HTML, SVG, and MathML parser namespaces; per-token foreign
dispatch while retaining the active HTML insertion mode; complete pinned SVG
tag/attribute and XML/XMLNS/XLink adjustment tables; integration points;
breakout/reprocessing; foreign self-closing and end-tag recovery; and the
tree-builder-controlled CDATA tokenizer boundary. See
`ae11-foreign-content-tree-construction-contract.md`.

AE12 adds the current five HTML processing-instruction tokenizer states and a
typed target/data token. Recognition is Data/TagOpen-only and preserves the
existing resumable prefix-first TagOpen cursor representation. Valid targets
retain exact case; invalid and ASCII-case-insensitive `xml`/
`xml-stylesheet` targets use exact `?`-prefixed bogus-comment recovery; leading
separator whitespace is discarded; questionable-state non-`>` input appends
`?` and reconsumes in PI data; unfinished EOF emits no PI.

Tree construction inserts typed PI leaves through the shared adjusted
insertion location across the supported early/body/table/template/after-body
and foreign-content modes. InTable PIs are direct and never foster-parented;
template redirection remains owned by the adjusted-location abstraction. The
node survives structural patches, strict validation, Browser materialization,
and deterministic snapshots while selector indexing, Layout box generation,
retained render identity, and Paint exclude it. See
`ae12-processing-instruction-contract.md`.

<a id="doctype-and-quirks-stance"></a>
### DOCTYPE And Quirks Stance

Core v0 guarantees:

- tokenizer emits DOCTYPE token fields including `force_quirks`.
- tree builder determines document mode from DOCTYPE during early bootstrap.
- an accepted initial DOCTYPE creates a parser-created `DocumentType` node when
  the document is materialized.
- the `DocumentType` node is a document child and appears before the document
  element in deterministic document-child order.
- after accepting a DOCTYPE in `Initial`, tree builder control moves to
  `BeforeHtml` for subsequent tokens.
- document mode MUST NOT change after the first non-DOCTYPE token that causes insertion of the root `html` element (implicit or explicit).
- duplicate/late DOCTYPE tokens after the `Initial` handoff or after that
  boundary MUST NOT change document mode.
- document mode is internal parser state in Core v0 (no dedicated `DomPatch` mode event) and is not encoded in `DocumentType` node identity or payload.
- the feature-gated AE13 observation boundary reads the existing production
  `DocumentMode` after successful parser finish and returns it only after the
  ordinary patch-validation/materialization path succeeds. It never
  reclassifies the doctype or infers mode from the final tree.

### Parse-Error Diagnostics

Core v0 parser diagnostics are deterministic internal regression/debug data:

- one `DocumentParseContext` diagnostic fanout owns counters, the independent
  parse-error and implementation-diagnostic occurrence sequences, bounded
  canonical retention, dropped counts, and optional legacy projection for
  preprocessing, tokenizer, and tree-construction production rules;
- tokenizer-origin malformed input records exact-position legacy `ParseError`
  entries where representable. Tree-construction canonical events currently
  use `Unavailable(ParserDidNotProvidePosition)` and are not projected with a
  fabricated offset;
- `Counters::parse_errors` counts every genuine recoverable tokenizer and tree
  error when counter tracking is enabled, so it may exceed the exact-position
  legacy vector length. `errors_dropped` remains legacy-deque capacity loss
  only;
- parse errors do not automatically abort tokenization or tree construction;
- supported EOF recovery paths record exact canonical identities while still
  emitting the recoverable token stream defined by the current tokenizer state
  support; the legacy facade may project them to `UnexpectedEof`;
- tokenizer-origin malformed tag, attribute, comment, doctype, and declaration
  recovery records exact typed canonical identities. Conditions without an
  existing broad facade category may project to legacy `Other`; detail strings
  are non-authoritative metadata and are never parsed back into identity;
- dedicated HTML Standard identities and exact Borrowser tokenizer-extension
  identities are separate. Numeric-reference digit hardening is a resource
  limit, not a Standard parse error; numeric values above U+10FFFF report the
  Standard `character-reference-outside-unicode-range` condition;
- unsupported surrogate and out-of-range numeric references remain literal
  Core-v0 character data. Their exact Standard conditions use typed
  `PreserveCharacterReferenceLiteral` recovery metadata and do not claim a
  U+FFFD replacement;
- condition identity and recovery are recorded separately. Core-v0 drops
  `=`, `"`, `'`, and `<` in `BeforeAttributeName` under their applicable
  Standard identities and typed `DropInputCharacter` metadata; grave accent
  and question mark use exact extension identities. Current unquoted-value
  termination uses typed reconsume metadata, while slash remains ordinary
  unquoted value data;
- regular end-tag attribute and trailing-solidus diagnostics are derived from
  the live production attribute vector and self-closing flag at emission.
  Unexpected solidus is owned by the self-closing-start-tag transition; raw
  source tails are never rescanned to reconstruct tokenizer state;
- comment diagnostics follow their exact Standard state transitions:
  CommentStartDash anything-else and CommentEnd anything-else do not invent
  errors, `incorrectly-closed-comment` is owned by the `>` in CommentEndBang,
  and `nested-comment` is owned only by the less-than/bang/dash/dash nested
  path. Typed recoveries state whether production emits at `>`, emits at EOF,
  starts a bogus comment, or retains a nested delimiter and reconsumes. Bogus
  comment EOF does not invent `eof-in-comment`; pending comment-state
  delimiters are excluded from EOF token data through checked state-owned
  bounds and exact constant-size suffix validation (`-`, `--`, or `--!` as
  selected by the active state). This production invariant check does not scan
  backwards or reconstruct comment state for observation. Every active
  comment state requires a pending start at or before the cursor, with the
  cursor inside normalized input. Missing metadata and invalid ranges stop the
  production state before errors, limits, or tokens are emitted;
- routing end tags through production attribute states and treating `/` as
  ordinary unquoted-value data are intentional production-tokenizer behavior
  corrections required for truthful AE13 observation. They apply equally when
  observation is disabled: `a=b/` is a non-self-closing token with value `b/`,
  while `a=b />` is self-closing with value `b`;
- an accepted self-closing flag always retains the exact slash offset that
  entered `SelfClosingStartTag`. The offset is replaced by a later applicable
  slash and cleared at emission, abandonment, reset, and stall recovery.
  Missing, stale, or non-slash metadata is an engine invariant, never an
  approximate diagnostic position. The retained slash must be at or after the
  current `tag_name_start`, before the cursor, and actually reference `/`; a
  slash before the current pending tag is a distinct typed invariant;
- legacy resource-limit and stall-guardrail `aux` fields retain their clamped
  `u32` compatibility values. Canonical payloads retain full-width typed values
  and never consume the legacy projection;
- unfinished tags, attributes, markup declarations, comments, and doctypes
  record deterministic EOF diagnostics and do not emit partial start/end tag
  tokens unless the current state has already reached a complete token
  boundary;
- invalid tag-open diagnostics identify the invalid scalar following `<`.
  Canonical EOF diagnostics identify the terminal normalized insertion point
  after CRLF/lone-CR preprocessing, never a pre-recovery cursor;
- canonical position resolution keeps exact, genuinely unavailable, and
  invariant-failure outcomes distinct. An invalid normalized offset retains no
  false unavailable-position event and fails feature-gated execution.
  `NormalizedPositionIndexMissing` is reserved for recorder corruption while a
  position-bearing event can still retain; bounded index retirement is normal;
- canonical parse-error and implementation-diagnostic occurrences are
  independent and there is no global cross-surface timeline or post-capture
  sorting. Reserving `u64::MAX` latches exhaustion immediately, including
  for a full or zero-capacity surface, so no successful result can contain a
  silently exhausted sequence. The first invariant detected in production
  operation order remains authoritative and later failures cannot replace it;
- tree construction distinguishes genuine HTML parse errors, deterministic
  Borrowser implementation deviations, configured resource limits, fatal
  execution/invariant failures, and normal operations that emit no event.
  Implied `html`, `head`, and `body` insertion and ordinary in-head fallback
  are normal operations, not parse errors;
- tree events retain detection-time token kind, insertion mode, and adjusted
  current-node namespace where available. Current production rules do not
  provide exact input offsets, so AE13b2 records the explicit unavailable
  reason rather than scanning source or borrowing the tokenizer cursor;
- each successfully completed start-tag tree step emits at most one
  `UnacknowledgedSelfClosingFlag`. Genuinely ignored flags report
  `IgnoreSelfClosingFlag`; the retained deprecated non-void HTML
  `LegacySkipPush` behavior reports no false recovery action and emits a
  separate implementation-deviation diagnostic at its production decision.
  Supported HTML void-element rules acknowledge the flag only when that
  production rule reaches its acknowledgement step; there is no global
  `is_void_tag` acknowledgement, and ignored tokens remain unacknowledged.
  The legacy altered-stack decision is committed only after configured
  insertion limits accept the insertion, so a limit-suppressed token reports
  the configured resource condition plus ignored-flag recovery without the
  altered-stack diagnostic. Contradictory effect transitions are fatal
  tree-builder invariants;
- the tree Text insertion-mode rule solely owns integrated `EofInTextMode`;
  standalone tokenizer text-mode EOF recovery flushes literal data and EOF
  without synthesizing a tree error;
- the doctype-name state, doctype-tail scanning, and doctype resource-limit
  observation require retained name-start metadata. Missing metadata latches
  an operation-specific tokenizer invariant, emits no cursor-positioned
  diagnostic, and stops normal progress. The shared accessor also requires the
  retained start not to exceed the current cursor; violation is the exact
  `DoctypeNameStartAfterCursor` invariant rather than a saturated empty range.
  Present, ordered metadata must also form an input-bounded UTF-8 range;
  required materialization rejects empty or unsliceable spans as
  `DoctypeNameRangeInvalid`, while transient zero-length name-state progress is
  permitted explicitly;
- shared ASCII-prefix scans distinguish invalid internal ranges from ordinary
  mismatch and partial input. Impossible quoted-doctype-tail relationships
  use `DoctypeTailRangeInvalid`, not a name-start identity;
- CDATA-end state requires parser-owned pending-text metadata before it
  excludes `]]` through a checked, fixed-size comparison selected by the live
  state. Missing ownership, range/boundary corruption, and suffix mismatch are
  distinct invariants; an owned zero-length range remains valid empty CDATA,
  and corrupt ownership cannot emit empty or truncated text;
- authoritative tokenizer paths modified for AE13b1 do not rely on panicking
  metadata access. A single non-mutating processing-instruction classifier is
  shared by production preflight, emission, EOF cleanup, and debug hardening;
  state/range corruption is latched before access or mutation. Remaining
  cursor-helper `expect`s are limited to preconditions mechanically established
  by the immediately preceding input slice operation;
- downstream browser/runtime, CSS, Layout, and Paint code must not interpret
  parse-error kinds as rendering semantics.
- DOM golden fixtures may opt into tree-builder parse-error output with
  `# include_parse_errors: true`; this is a deterministic regression/debug
  surface and not a public runtime diagnostics API. Its
  `parser-conformance`-gated test-harness adapter uses finite capacity, rejects
  partial capture, and projects only authoritative typed tree parse errors
  one way to legacy lines. It does not merge implementation/resource
  diagnostics or alter patch goldens.

### Script/RAWTEXT/RCDATA Stance

Core v0 stance:

- RAWTEXT is supported for HTML RAWTEXT containers using the Core-v0 shared text-mode subset.
- RCDATA is supported for HTML `title`/`textarea` using the Core-v0 shared text-mode subset, including current tokenizer-side character-reference decoding behavior.
- Script supports a dedicated Core-v0 script tokenizer state family, including escaped and double-escaped comment-like branches, while still using the shared script close-tag matcher.
- Core-v0 shared text-mode close-tag recognition for RAWTEXT/RCDATA/script:
  - matches the expected end-tag name ASCII-case-insensitively,
  - treats `>`, HTML-space-led attribute continuations, and `/` self-closing continuations after the matched name as real end-tag tails,
  - consumes those tails incrementally and chunk-safely until the closing `>`,
  - keeps the whole candidate sequence in text and resumes scanning only when the matched name is followed by any other continuation byte.
- In Core v0, sequences such as `</style class=x>`, `</script type=text/plain>`, and `</textarea/>` now close the active RAWTEXT/RCDATA/script element instead of staying literal text.
- Attribute-bearing and self-closing end-tag tails record tokenizer parse errors because end tags ignore attributes and self-closing syntax.
- The shared text-mode matcher carries the exact closing `>` used for
  `end-tag-with-attributes` and the exact accepted slash used for
  `end-tag-with-trailing-solidus`; these positions survive pause/resume and use
  the ordinary end-tag contract without rescanning source. Live and completed
  evidence is checked against the current candidate bounds and punctuation.
  Retained candidates must own the complete fixed `</` opener; a lone partial
  `<` suspends without becoming retained matcher state. Candidate range and
  delimiter validity is checked unconditionally, including when no diagnostic
  evidence exists; candidate, attribute, and solidus corruption use separate
  exact invariants before diagnostics or an end-tag token can be emitted.
- Tree-builder `Text` insertion mode is supported only to the extent required by the supported tokenizer text-mode subset above.
- Parser-scripting interaction (parser pause/suspension and script execution integration) is not implemented in Core v0.

<a id="tables-stance"></a>
### Tables Stance

Core v0 stance:

- AE8 promotes the supported static table tree-construction subset into
  Core-v0 `MVP_PARTIAL` scope.
- supported insertion modes are `InTable`, `InTableText`, `InCaption`,
  `InColumnGroup`, `InTableBody`, `InRow`, and `InCell`.
- supported table elements are `table`, `caption`, `colgroup`, `col`, `tbody`,
  `thead`, `tfoot`, `tr`, `td`, and `th`.
- supported omitted wrappers, malformed row/cell/body recovery, pending
  table-character-token handling, and foster-parent insertion locations are
  parser-owned behavior.
- unsupported table interactions still require robust fallback:
  - parser MUST remain deterministic,
  - parser MUST preserve core invariants (SOE/patch ordering),
  - parser MUST NOT panic on table-family tags.
- AE8 does not claim full WHATWG table parsing conformance.
- AE9b composes select-family tokens with these table modes through the same
  bounded delegation and adjusted insertion-location machinery. No select-
  specific insertion mode is entered.

- AE9a adds declared-scope parser-owned form tree construction: stable form
  pointers, exact stack-only form removal, specialized input/textarea/button/
  fieldset/keygen handling, and parser-owned textarea initial-LF suppression.
  It does not add interactive form platform behavior.

- AE10 adds ordinary full-document template tree construction: typed parser-
  created contents roots, owner-aware template modes, shared dispatch,
  template-aware adjusted/foster insertion, exact last-marker closure, semantic
  reprocessing cycle/progress separation, and deterministic depth-decreasing
  EOF unwind. Template starts reserve final parent-child storage before commit;
  typed reservation errors distinguish resource denial from engine corruption.
  Ordinary tokens use O(1) template-validation fast paths, and start/replace/
  close validation is transition-local, while heavy full-model audits remain
  test/fuzz/invariant work. Same-token fingerprints are only lookup keys for
  exact collision-resolved state equality. Checked validation counters cannot
  wrap, and counter-backed depth-16/depth-256 fixtures prove linear aggregate
  EOF close/owner/reset work with O(1) auxiliary recovery memory. Contents are
  preserved in the centralized full-model traversal and inert to active
  consumers.

<a id="explicitly-unsupported-or-deferred-in-core-v0"></a>
## Explicitly Unsupported Or Deferred In Core v0

The following are intentionally not part of the Core v0 guarantee:

- `DEFERRED`:
  - full WHATWG tokenizer behavior outside the tokenizer state families and
    fixtures explicitly listed in this contract and the tokenizer matrix
  - full WHATWG table insertion-mode conformance beyond the AE8 supported
    subset
  - select fragment-context behavior; the historical `InSelectInTable` mode is
    not a current Living Standard mode and is neither implemented nor deferred
  - table/template behavior beyond the composed AE8/AE10 supported subset
  - public template/fragment DOM APIs, fragment parsing, cloning, adoption and
    owner-document semantics, scripting, declarative shadow DOM, custom
    elements, live mutation, and rendering/resource activation of contents
  - full DOM `ProcessingInstruction` APIs, pseudo-attributes, constructors,
    CharacterData mutation, cloning, public mutation, and broader DOM PI
    validity
  - `PLAINTEXT`, frameset insertion modes, and fragment parsing for AE12

Policy classification requirements:

- Out-of-scope tests MUST be `skip` (not `xfail`).
- In-scope but not yet passing tests MAY be `xfail` with actionable reason text.

<a id="unspecified-behavior-handling-fail-safe-contract"></a>
## Unspecified Behavior Handling (Fail-Safe Contract)

For inputs or state combinations not fully covered by Core v0:

- parser MUST fail safe and deterministic; it MUST NOT panic due to unsupported syntax alone.
- panics are permitted only for internal invariant violations (for example debug assertions or unreachable bug states) and are treated as engine bugs; user-controlled input MUST NOT trigger panics.
- parser MUST preserve internal invariants (state continuity, SOE consistency, deterministic patch sequencing).
- parser MUST continue producing a recoverable stream/result where possible, ending in deterministic `finish()` behavior.
- parser MUST terminate deterministically on finite input; infinite reprocess/dispatch loops are parser bugs.
- unsupported constructs MUST follow documented fallback paths instead of ad-hoc behavior.
- newly observed unsupported behaviors MUST be documented in matrix/docs before being considered contractual.

This contract prevents accidental reliance on unspecified behavior.

## Observability And Error Accounting

- Parse errors MUST be recordable/countable by session-level accounting.
- Core v0 does not guarantee specific numeric parse-error counts as a public contract.
- Core v0 does guarantee deterministic parser outputs for gate tests under fixed inputs and chunk plans.

## Memory Safety And Unsafe Policy

- Core v0 guarantees memory safety and absence of undefined behavior (UB) in tokenizer/tree-builder code paths.
- Any `unsafe` introduced in tokenizer/tree-builder modules MUST include an in-code `// SAFETY:` comment documenting required invariants and why safe Rust was insufficient.
- Tokenizer/tree-builder behavior MUST NOT rely on `unsafe` for parsing semantics; if `unsafe` is introduced for implementation reasons, it must preserve identical parsing semantics and maintain memory safety guarantees.

## Non-Goals (Core v0)

- parser pause/suspension and script execution integration for `<script>`.
- Public template/fragment APIs and behavior beyond the declared AE10 static
  tree-construction subset.
- Full WHATWG table insertion-mode conformance beyond AE8.
- select fragment-context behavior and table/template interactions beyond the
  AE8/AE10 composition;
  historical `InSelect`/`InSelectInTable` modes are intentionally not used.
- Full tree-builder text mode parity (`TB-MODE-TEXT`), including deferred RAWTEXT/RCDATA coupling.
- XML parsing, stylesheet PI behavior, PI execution, and resource loading.

<a id="core-v0-gate-and-evidence-model"></a>
## Core v0 Gate And Evidence Model

Core v0 exit depends on gate cases defined by acceptance inventories in:

- `docs/html5/spec-matrix-tokenizer.md`
- `docs/html5/spec-matrix-treebuilder.md`

A gate case is considered compliant only when:

- it is in-scope for Core v0,
- it passes active expectations in whole-input and chunked-input runs,
- and it has deterministic outcomes across seeded CI runs.

## Promotion To HTML5 v1: Parity Definition

For promotion from `Core v0` to `v1`, parity means:

- **Declared-scope parity**, not implicit “full HTML5 parity”.
- Every behavior promoted into `v1` scope is:
  - explicitly listed in spec matrices with stable IDs and tiers,
  - covered by acceptance fixtures/WPT mapping,
  - `active` and passing (no `xfail`) for promoted scope,
  - validated under whole-input and chunked-input equivalence.
- Out-of-scope behavior remains `skip` until explicitly promoted.

Minimum promotion gates:

1. No in-scope `xfail` remains for declared `v1` scope.
2. Core invariants remain intact under streaming/fuzz chunk plans.
3. Policy boundaries (`active` vs `xfail` vs `skip`) remain unambiguous in harness and CI.
4. All scope promotions are documented by updating this contract and both spec-matrix documents in the same change set.

## Change Control

Any change to supported/unsupported status MUST update all of:

1. `docs/html5/html5-core-v0.md` (this contract),
2. tokenizer/tree-builder spec matrices,
3. acceptance fixtures and/or WPT manifest policy as applicable.

Without those updates, behavior changes are non-contractual and must not be treated as stabilized API/engine behavior.
