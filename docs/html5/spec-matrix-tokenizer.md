# HTML5 Tokenizer Spec Mapping Matrix (HTML5 Core v0)

Last updated: 2026-07-05
Scope: `crates/html/src/html5/tokenizer` (feature `html5`)
Spec source: WHATWG HTML, section `Tokenization` (`parsing.html#tokenization`)

## Purpose

This document maps HTML tokenizer states from the HTML Standard to Borrowser implementation files, acceptance fixtures, and WPT coverage.
It defines HTML5 Core v0 tokenizer scope, including explicit deferred and out-of-scope behavior.

## Stable IDs And Naming

- State mapping IDs use `TOK-STATE-*` and are stable across refactors.
- Tier labels are machine-stable:
  - `MVP`
  - `MVP_PARTIAL`
  - `DEFERRED`
  - `OUT_OF_SCOPE`
- Acceptance test IDs use lowercase kebab-case and map 1:1 to fixture directory names under:
  - `crates/html/tests/fixtures/html5/tokenizer/<acceptance-id>/`
- Fixture layout contract:
  - `input.html`
  - `tokens.txt`
  - optional `notes.md`
- If a state family is split later, old IDs are kept as deprecated aliases in this document until removal.

## Component Contracts (Hard Boundaries)

- Tokenizer contract:
  - Consumes preprocessed parser input. Current supported parser input is
    decoded UTF-8/scalar text, with CRLF and lone CR normalized to LF by the
    shared `Input` boundary.
  - Emits spec-shaped token stream and parse-error-tolerant recovery transitions.
  - Performs tokenizer-level normalization only (for example, tag/attribute ASCII case folding and U+0000 handling).
  - Preserves attribute encounter order and does not apply tree-builder duplicate-attribute semantics.
  - Sets DOCTYPE token fields including `force_quirks`; does not choose document mode.
- Tree builder contract:
  - Applies insertion modes, duplicate-attribute semantics, foster parenting, and document mode decisions.
  - Consumes tokenizer output without requiring tokenizer-owned lookbehind state.
- Session contract:
  - Owns streaming orchestration (`push_input`, `NeedMoreInput`, `finish`), counters, and error accounting.
  - Enforces policy status handling (`active`/`xfail`/`skip`) for WPT integration.

## Streaming Invariants (Cross-Cutting)

These invariants apply to every `TOK-STATE-*` row:

1. `STREAM-INV-01`: no state may require unbounded lookahead; if continuation bytes/chars are missing, return `NeedMoreInput` without corrupting state.
2. `STREAM-INV-02`: partial UTF-8 scalar boundaries are preserved through `Input`; tokenizer state must not split or emit invalid scalar fragments.
3. `STREAM-INV-03`: temporary buffers and reconsume semantics survive chunk boundaries exactly once (no duplicate consume, no skipped consume).
4. `STREAM-INV-04`: EOF processing is deterministic and idempotent (`finish()` cannot double-emit semantic tokens).

## AE3 Input And Parse-Error Foundation

AE3 formalizes the tokenizer foundation without claiming full WHATWG tokenizer
completion.

Current input behavior:

- `Input::push_str` accepts already-decoded Unicode scalar text and applies
  HTML input preprocessing for newline handling in the supported scope.
- `ByteStreamDecoder` assumes UTF-8 only. Invalid UTF-8 and incomplete final
  UTF-8 prefixes are decoded with `U+FFFD`; full byte-stream encoding sniffing,
  charset detection, BOM switching, and legacy encodings remain deferred.
- CRLF and lone CR are normalized to LF before tokenization.
- Split CRLF across chunks is chunk-equivalent: `"\r"` followed by `"\n"`
  produces the same preprocessed input as `"\r\n"` in one chunk.

Current parse-error behavior:

- `ParseError` is deterministic parser diagnostic data owned by
  HTML/parser. It does not automatically abort tokenization.
- Supported tokenizer states that encounter `U+0000` record
  `ParseErrorCode::UnexpectedNullCharacter` and emit the replacement character
  in the affected token payload.
- Supported malformed EOF recovery paths record
  `ParseErrorCode::UnexpectedEof` and still emit deterministic recoverable
  tokens where the current Core-v0 tokenizer supports recovery.
- Parser diagnostics are test/debug surfaces and must not become rendering,
  CSS, layout, or browser/runtime semantics.

## Current Repository Baseline

AE4 formalizes the Core-v0 static-HTML tokenizer subset. The tokenizer now has
explicit state handlers for data, tag open, end tag open, tag name, core
attribute states, after-quoted-attribute-value, self-closing start tags, markup
declarations, comments, bogus comments, and doctypes.

Implementation entry points:

- `crates/html/src/html5/tokenizer/api.rs`: streaming API, EOF recovery, token queueing.
- `crates/html/src/html5/tokenizer/machine.rs`: state dispatch and data/markup-declaration dispatch.
- `crates/html/src/html5/tokenizer/tag/open.rs`: tag-open and end-tag-open recovery.
- `crates/html/src/html5/tokenizer/tag/name.rs`: tag-name transitions and end-tag trailing-content diagnostics.
- `crates/html/src/html5/tokenizer/tag/attributes.rs`: attribute states and self-closing start tag handling.
- `crates/html/src/html5/tokenizer/tag/emit.rs`: start/end tag finalization.
- `crates/html/src/html5/tokenizer/comment.rs`: supported comment and bogus-comment states.
- `crates/html/src/html5/tokenizer/doctype.rs`: supported doctype states.
- `crates/html/src/html5/tokenizer/normalization.rs`: tokenizer-owned normalization and stable diagnostic detail strings.

Every typed-token attribute has a string value (`Span` or owned storage is an
internal lifetime choice). Valueless and explicitly empty syntax both resolve
to `""`; tokenizer snapshots render both as `name=""`. Source spelling is not
retained as token, DOM, patch, CSS, materialization, or snapshot semantics.

Malformed tag, attribute, comment, doctype, declaration, and EOF recovery paths
record deterministic tokenizer-owned parse errors where currently supported.
This does not claim full WHATWG tokenizer parity; unsupported behavior must be
kept documented as deferred, skipped, or outside the declared Core-v0 subset.

## State Matrix (Spec -> Files/Tests -> Status)

| ID | Tier | Spec state name | Anchor | Implementation mapping | Test mapping (current + planned) | Streaming notes | Key edge cases | Rationale |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `TOK-STATE-DATA` | MVP | `Data state` | `#data-state` | `states.rs`, `machine.rs`, `api.rs`, `input.rs` | Current: `tok-empty-eof`, `tok-basic-text`, `tok-ae3-basic-stream`. Planned WPT: `tokenizer-data-text`. | `STREAM-INV-01`, `STREAM-INV-02` are hard gates. | EOF-in-data, `<` and `&` dispatch, U+0000 handling. | Universal tokenizer baseline. |
| `TOK-STATE-TAG-OPEN` | MVP | `Tag open state` | `#tag-open-state` | `states.rs`, `machine.rs`, `tag/open.rs`, `normalization.rs` | Current: `tok-simple-tags`, `tok-bogus-comment`, `tok-ae4-malformed-static`, AE12 TagOpen regression/chunk tests. Planned WPT: `tokenizer-tag-open`. | Prefix-first `</`, `<!`, and AE12 `<?` matching suspends without consuming `<` when a prefix is partial at chunk end. | `<!`, `</`, `<?`, invalid next-char fallback; AE12 does not change existing branch ownership. | Required for structural token emission. |
| `TOK-STATE-PI-OPEN` | MVP | `Processing instruction open state` | `#processing-instruction-open-state` | `states.rs`, `machine.rs`, `processing_instruction.rs`, `tag/open.rs` | AE12 tokenizer state/chunk tests; pinned supported-profile cases. | `<?` entry uses resumable TagOpen prefix matching and does not require both code points in one chunk. | ASCII alpha/`_` first target character; invalid-first conversion to exact `?`-prefixed bogus comment. | Typed HTML PI entry, not XML parsing. |
| `TOK-STATE-PI-TARGET` | MVP | `Processing instruction target state` | `#processing-instruction-target-state` | `states.rs`, `machine.rs`, `processing_instruction.rs` | AE12 grammar, exact-case, forbidden-target, malformed, limit, and chunk tests. | Target span/pending flags persist exactly across every split. | ASCII alphanumeric/`-`/`_`; case-insensitive `xml`/`xml-stylesheet` rejection; oversized target suppression is hardening only. | Preserves a target distinct from element names and data. |
| `TOK-STATE-AFTER-PI-TARGET` | MVP | `After processing instruction target state` | `#after-processing-instruction-target-state` | `states.rs`, `machine.rs`, `processing_instruction.rs` | AE12 separator-whitespace, empty-data, EOF, and chunk tests. | Separator runs may span chunks without entering data early. | Discard leading separator HTML whitespace; reconsume first non-space in data. | Defines exact target/data boundary. |
| `TOK-STATE-PI-DATA` | MVP | `Processing instruction data state` | `#processing-instruction-data-state` | `states.rs`, `machine.rs`, `processing_instruction.rs` | AE12 data, terminator, question-mark, EOF, hardening, and chunk tests. | Data spans shared append-only input; bounded end and pending start persist across chunks. | `>` termination, `?` dispatch, exact data; bounded-data output is hardening only. | Typed PI payload collection. |
| `TOK-STATE-PI-QUESTIONABLE` | MVP | `Processing instruction questionable state` | `#processing-instruction-questionable-state` | `states.rs`, `machine.rs`, `processing_instruction.rs` | AE12 `a?b`, `a??b`, `a???b?`, EOF, and every relevant split tests. | A chunk may end after `?`; pending state resumes without emitting or losing it. | `>` emits without the final `?`; otherwise append `?` and reconsume the current character in PI data. | Preserves current WHATWG questionable-state semantics. |
| `TOK-STATE-END-TAG-OPEN` | MVP | `End tag open state` | `#end-tag-open-state` | `states.rs`, `machine.rs`, `tag/open.rs`, `api.rs` | Current: `tok-simple-tags`, `tok-ae4-malformed-static`. Planned WPT: `tokenizer-end-tag-open`. | Must preserve reconsume semantics across split `</`. | EOF after `</`, non-name after `</`, parse-error fallback path. | Required for balanced close-tag handling. |
| `TOK-STATE-TAG-NAME` | MVP | `Tag name state` | `#tag-name-state` | `states.rs`, `machine.rs`, `tag/name.rs`, `tag/emit.rs`, `api.rs` | Current: `tok-simple-tags`, `tok-ae4-malformed-static`, WPT `tokenizer-basic`. Planned fixture: `tok-tag-name-case-folding`. | Must not lose partial name token on chunk boundary before whitespace or `>`. | ASCII case folding, NUL replacement, `/` and `>` transitions. | Required for deterministic atomization. |
| `TOK-STATE-BEFORE-ATTR-NAME` | MVP | `Before attribute name state` | `#before-attribute-name-state` | `states.rs`, `machine.rs`, `tag/attributes.rs` | Current: `tok-attrs-core`, `tok-before-attr-value-transitions`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-attrs-before-after-name`. | Whitespace runs may span chunks with no semantic drift. | whitespace skip, `/` self-closing handoff, `>` early close. | Core attribute parser entry. |
| `TOK-STATE-ATTR-NAME` | MVP | `Attribute name state` | `#attribute-name-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `tag/emit.rs`, `normalization.rs` | Current: `tok-attrs-core`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-attr-name`. | Partial attr names must persist until delimiter appears. | ASCII case fold, NUL replacement, parse-error chars (`"`, `'`, `<`, `=`). | Needed for stable attribute token shape. |
| `TOK-STATE-AFTER-ATTR-NAME` | MVP | `After attribute name state` | `#after-attribute-name-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `api.rs` | Current: `tok-attrs-core`, `tok-before-attr-value-transitions`; planned WPT: `tokenizer-attrs-before-after-name`. | Must not consume `=`/`/`/`>` twice when chunk split lands on delimiter. | repeated separators, implicit missing attr values, close/self-close transitions. | Ensures deterministic attr finalization. |
| `TOK-STATE-BEFORE-ATTR-VALUE` | MVP | `Before attribute value state` | `#before-attribute-value-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `api.rs` | Current: `tok-before-attr-value-transitions`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-before-attr-value`. | Transition dispatch must survive split quote characters. | quoted vs unquoted dispatch, `>` parse-error close. | Dispatch point for value semantics. |
| `TOK-STATE-ATTR-VALUE-DQ` | MVP | `Attribute value (double-quoted) state` | `#attribute-value-double-quoted-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `api.rs` | Current: `tok-attr-value-quoted`; planned WPT: `tokenizer-attr-value-quoted`. | Unterminated quote at chunk end must yield `NeedMoreInput` with intact buffer. | `&` charref dispatch, quote close, embedded control handling. | Required for common attribute syntax. |
| `TOK-STATE-ATTR-VALUE-SQ` | MVP | `Attribute value (single-quoted) state` | `#attribute-value-single-quoted-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `api.rs` | Current: `tok-attr-value-quoted`; planned WPT: `tokenizer-attr-value-quoted`. | Same as DQ; boundary behavior must be symmetric. | `&` charref dispatch, quote close, parse-error recovery. | Required for spec parity on quoted attrs. |
| `TOK-STATE-ATTR-VALUE-UQ` | MVP | `Attribute value (unquoted) state` | `#attribute-value-unquoted-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `normalization.rs` | Current: `tok-attr-value-unquoted`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-attr-value-unquoted`. | Must preserve pending token when encountering split delimiter bytes. | parse-error chars (`"`, `'`, `<`, `=`, `` ` ``), whitespace/value termination. | Required for realistic HTML input tolerance. |
| `TOK-STATE-AFTER-ATTR-VALUE-QUOTED` | MVP | `After attribute value (quoted) state` | `#after-attribute-value-quoted-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `api.rs` | Current: `tok-attr-value-quoted`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-attr-value-quoted`. | Must preserve just-closed attribute state when chunk split lands before whitespace, `/`, or `>`. | missing whitespace between attributes, `/` self-closing handoff, `>` close, EOF. | Required for deterministic quoted-attribute recovery. |
| `TOK-STATE-SELF-CLOSING-START-TAG` | MVP | `Self-closing start tag state` | `#self-closing-start-tag-state` | `states.rs`, `machine.rs`, `tag/attributes.rs`, `tag/emit.rs`, `api.rs` | Current: `tok-attrs-core`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-self-closing-start-tag`. | Must preserve pending start tag and self-closing flag across split `/>`. | `/>` emission, invalid non-`>` recovery, EOF after `/`. | Required for self-closing flag emission. |
| `TOK-STATE-MARKUP-DECL-OPEN` | MVP | `Markup declaration open state` | `#markup-declaration-open-state` | `states.rs`, `machine.rs`, `api.rs`, `normalization.rs` | Current: `tok-doctype-comment-smoke`, `tok-comment-core`, `tok-doctype-core`, `tok-ae4-markup-eof-recovery`. | Declaration prefix matching cannot overconsume partial `<!-`/`<!D`. | comment vs doctype dispatch, unknown declaration fallback. | Root dispatch for comment/doctype flows. |
| `TOK-STATE-COMMENT-CORE` | MVP | `Comment core states` | `#comment-start-state`, `#comment-start-dash-state`, `#comment-state`, `#comment-end-state`, `#comment-end-bang-state` | `states.rs`, `machine.rs`, `comment.rs`, `emit.rs`, `normalization.rs` | Current: `tok-doctype-comment-smoke`, `tok-comment-core`, `tok-ae4-malformed-comment`; planned WPT: `tokenizer-comments`. | Comment temporary data must survive chunk splits around `--`, `!`, and `>`. | EOF in comment, `--!>` malformed close recovery, nested `<` comment sequences. | Required for web-compat error recovery. |
| `TOK-STATE-BOGUS-COMMENT` | MVP | `Bogus comment state` | `#bogus-comment-state` | `states.rs`, `machine.rs`, `comment.rs`, `emit.rs`, `processing_instruction.rs` | Current: `tok-bogus-comment`, `tok-ae4-malformed-static`, `tok-ae4-markup-eof-recovery`, AE12 malformed-target cases; planned WPT: `tokenizer-bogus-comment`. | Must emit one comment even when its terminator arrives later. | Malformed declarations and invalid/disallowed AE12 PI targets; PI conversion preserves exact leading `?`. | Explicit parse-error recovery path, not valid PI storage. |
| `TOK-STATE-DOCTYPE` | MVP | `DOCTYPE state` | `#doctype-state` | `states.rs`, `machine.rs`, `doctype.rs`, `normalization.rs` | Current: `tok-doctype-comment-smoke`, `tok-doctype-core`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-doctype-quirks`. | Partial `DOCTYPE` keyword must not mis-route to bogus comment. | case-insensitive keyword handling. | Entry point for standards/quirks tokenization. |
| `TOK-STATE-BEFORE-DOCTYPE-NAME` | MVP | `Before DOCTYPE name state` | `#before-doctype-name-state` | `states.rs`, `machine.rs`, `doctype.rs`, `api.rs` | Current: `tok-doctype-core`, `tok-doctype-quirks-missing-name`, `tok-ae4-malformed-static`. | Whitespace and EOF boundary behavior must be deterministic. | missing name triggers quirks path. | Needed for correct token fields. |
| `TOK-STATE-DOCTYPE-NAME` | MVP | `DOCTYPE name state` | `#doctype-name-state` | `states.rs`, `machine.rs`, `doctype.rs`, `emit.rs`, `api.rs` | Current: `tok-doctype-core`, WPT reuse: `tokenizer-basic`. | Name accumulation must survive boundary before close quote/`>`. | case fold, NUL replacement, EOF mid-name. | Core doctype payload correctness. |
| `TOK-STATE-AFTER-DOCTYPE-NAME` | MVP | `After DOCTYPE name state` | `#after-doctype-name-state` | `states.rs`, `machine.rs`, `doctype.rs` | Current: `tok-doctype-core`, `tok-doctype-public-system`. | Keyword dispatch (`PUBLIC`/`SYSTEM`) must not overconsume on short chunk. | public/system keyword recognition and recovery. | Required for public/system field parsing. |
| `TOK-STATE-BOGUS-DOCTYPE` | MVP | `Bogus DOCTYPE state` | `#bogus-doctype-state` | `states.rs`, `machine.rs`, `doctype.rs`, `emit.rs` | Current: `tok-doctype-quirks-missing-name`, `tok-ae4-malformed-static`; planned WPT: `tokenizer-doctype-quirks`. | Must preserve `force_quirks=true` through chunked malformed inputs. | malformed public/system IDs, EOF before `>`. | Needed for standards vs quirks token correctness. |
| `TOK-STATE-CHARREF-ENTRY` | MVP_PARTIAL | `Character reference state` | `#character-reference-state` | `states.rs`, `mod.rs`; delegated table/validation in `crates/html/src/entities.rs` | Current: `tok-charrefs-text`, `tok-charrefs-attr`, entity unit tests; planned WPT: `tokenizer-charrefs-text`, `tokenizer-charrefs-attr`. | Supported references must remain chunk-equivalent because text/attribute/RCDATA emission uses tokenizer-owned pending spans. | explicit tokenizer contexts: data text, attribute value, and RCDATA; RAWTEXT/script bypass decoding. | Charref dispatcher for all supported contexts. |
| `TOK-STATE-CHARREF-NAMED` | MVP_PARTIAL | `Named character reference state` | `#named-character-reference-state` | `states.rs`, `mod.rs`, `crates/html/src/entities.rs` | Current: `tok-charrefs-text`, `tok-charrefs-attr`, entity unit tests; existing generated full-table tests remain behind `html5-entities`. | Minimal active named subset is semicolon-terminated and deterministic; unsupported semicolon-terminated names stay literal and report tokenizer-owned diagnostics. | supported names: `amp`, `lt`, `gt`, `quot`, `apos`, `nbsp`; full legacy named-reference table is not active by default. | Core named reference behavior without full WHATWG named-entity parity claims. |
| `TOK-STATE-CHARREF-AMBIGUOUS-AMP` | MVP_PARTIAL | `Ambiguous ampersand state` | `#ambiguous-ampersand-state` | `states.rs`, `mod.rs`, `entities.rs` | Current: `tok-charrefs-attr`, tokenizer attribute diagnostics. | Return-to-state behavior must be stable under split alnum runs. | attribute context is explicit; AE5 deliberately keeps the minimal semicolon-required policy rather than legacy semicolonless attribute rules. | Needed for deterministic fallback behavior in supported contexts. |
| `TOK-STATE-CHARREF-NUMERIC` | MVP_PARTIAL | `Numeric character reference state family` | `#numeric-character-reference-state`, `#hexadecimal-character-reference-state`, `#decimal-character-reference-state`, `#numeric-character-reference-end-state` | `states.rs`, `mod.rs`, `entities.rs` | Current: `tok-charrefs-text`, `tok-charrefs-attr`, entity/tokenizer invalid-reference tests. | Numeric parse state must remain chunk-equivalent through pending text/attribute spans. | valid decimal/hex scalar values decode; missing digits, missing semicolons, overlong digit runs, malformed digits, surrogates, and out-of-range scalars stay literal and report deterministic diagnostics. | Core numeric reference correctness. |
| `TOK-STATE-RAWTEXT` | MVP_PARTIAL | `RAWTEXT state` | `#rawtext-state` | `states.rs`, `text_mode.rs`, `api.rs` | Current fixtures: `tok-rawtext-style`; no WPT slice currently vendored. | Appropriate end-tag buffer must persist across chunked `</sty` + `le>`. | mismatched end-tag fallback to text. | Core-v0 text-mode subset for HTML RAWTEXT containers. |
| `TOK-STATE-RAWTEXT-END-TAG` | MVP_PARTIAL | `RAWTEXT end tag detection` | `#rawtext-end-tag-open-state`, `#rawtext-end-tag-name-state` | `text_mode.rs`, `scan.rs` | Current fixtures: `tok-rawtext-style`, `tok-rawtext-style-end-tag-attrs-close`, `tok-rawtext-style-end-tag-slash-close`. | Temporary end-tag buffer must be chunk-safe and reset-safe. | ASCII-case-insensitive match; `>`, HTML-space-led attribute tails, and `/` self-closing tails all close in Core v0 once the expected name matches; any other continuation literalizes the candidate. | Required for production-safe RAWTEXT close recognition. |
| `TOK-STATE-RCDATA` | MVP_PARTIAL | `RCDATA state` | `#rcdata-state` | `states.rs`, `text_mode.rs`, `api.rs` | Current fixtures: `tok-rcdata-title`, `tok-rcdata-textarea`; no WPT slice currently vendored. | Must combine charrefs + appropriate-end-tag logic under chunking. | charrefs in rcdata text, fallback behavior. | Core-v0 text-mode subset for `title`/`textarea`. |
| `TOK-STATE-RCDATA-END-TAG` | MVP_PARTIAL | `RCDATA end tag detection` | `#rcdata-end-tag-open-state`, `#rcdata-end-tag-name-state` | `text_mode.rs`, `scan.rs` | Current fixtures: `tok-rcdata-title`, `tok-rcdata-textarea`, `tok-rcdata-title-close-tag-whitespace`, `tok-rcdata-title-end-tag-attrs-close`, `tok-rcdata-textarea-end-tag-slash-close`. | Same buffer semantics as rawtext end-tag path. | ASCII-case-insensitive match; `>`, HTML-space-led attribute tails, and `/` self-closing tails all close in Core v0 once the expected name matches; any other continuation literalizes the candidate. | Required for chunk-safe RCDATA end-tag recognition. |
| `TOK-STATE-SCRIPT-DATA` | MVP_PARTIAL | `Script data state` | `#script-data-state` | `states.rs`, `text_mode.rs`, `api.rs` | Current fixtures: `tok-script-data-basic`, `tok-script-data-close-tag-whitespace`, `tok-script-data-string-close`, `tok-script-data-end-tag-attrs-close`, `tok-script-data-end-tag-slash-close`; WPT: `tokenizer-script-data`. | Dedicated script-family state machine must preserve chunk-safe script close detection and linear scanning. | literal `</script>` closes even inside JS strings; HTML-space-led attribute tails and `/` self-closing tails after the matched name also close and record tokenizer parse errors. | Core-v0 script tokenizer now uses a dedicated script-data family rather than the shared text-mode subset. |
| `TOK-STATE-SCRIPT-DATA-ESCAPED` | MVP_PARTIAL | `Script data escaped families` | `#script-data-escaped-state`, `#script-data-double-escaped-state` | `states.rs`, `text_mode.rs`, `api.rs` | Current fixtures: `tok-script-data-escaped-comment-family`; WPT: `tokenizer-script-escaped`. | Escaped/double-escaped transitions must remain chunk-safe across `<!--`, `<script`, `</script`, and `-->` boundaries. | escaped entry, double-escape start/end transitions, comment-like script tails. | Dedicated script-family support landed in G5. |

## Core v0 Must-Support Subset

Core v0 passes only if all `MVP` rows and all `MVP_PARTIAL` rows pass their Core-v0 gate fixtures in both whole-input and UTF-8 chunked runs.

Required in Core v0:

1. Data, tag-open, end-tag-open, tag-name flows.
2. Attribute name/value flows (DQ/SQ/UQ).
3. Comment flows including bogus comment recovery.
4. DOCTYPE flows including quirks-token signaling (`force_quirks`).
5. Character reference flows in explicitly scoped contexts.

### Character References Scope For Core v0 (`MVP_PARTIAL`)

AE5 makes the active Core-v0 character-reference behavior explicit without
claiming full WHATWG character-reference parity.

- Included contexts:
  - Data text.
  - Attribute value states (`DQ`, `SQ`, `UQ`).
  - RCDATA text for supported `title` and `textarea` text-mode containers.
- Included forms:
  - Semicolon-terminated named references in the active minimal subset:
    `amp`, `lt`, `gt`, `quot`, `apos`, and `nbsp`.
  - Decimal and hexadecimal numeric references for valid Unicode scalar values
    within the active digit bounds.
- Recovery and diagnostics:
  - Unknown semicolon-terminated named references remain literal and record
    `unknown-named-character-reference`.
  - Supported names without semicolons remain literal and record
    `missing-semicolon-after-named-character-reference`.
  - Malformed numeric references remain literal and record deterministic
    tokenizer-owned `InvalidCharacterReference` details for missing digits,
    missing semicolons, malformed digits, overlong digit runs, or invalid
    scalar values.
- Delegation contract:
  - Tokenizer delegates named-reference table, numeric validation, and
    diagnostic classification to `crates/html/src/entities.rs` through explicit
    tokenizer contexts. Data text, attribute values, and RCDATA call into that
    API; RAWTEXT and script-data do not.
- Deferred inside charrefs:
  - Full legacy named-reference edge matrix beyond the active minimal subset.
  - Activating the generated full WHATWG named-entity table for the default
    tokenizer policy.

## Acceptance Inventory (Deterministic ID <-> Fixture Mapping)

Status source of truth:

- Tokenizer golden fixtures use `tokens.txt` headers (`# status`, `# reason`).
- WPT cases use `tests/wpt/manifest.txt` (`status`, `reason`).

| Acceptance ID | Core v0 gate | Canonical fixture dir | Status now | WPT reference |
| --- | --- | --- | --- | --- |
| `tok-empty-eof` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-empty-eof` | Active | `tokenizer-basic` (EOF) |
| `tok-basic-text` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-basic-text` | Active | planned `tokenizer-data-text` |
| `tok-simple-tags` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-simple-tags` | Active | `tokenizer-basic` |
| `tok-doctype-comment-smoke` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-comment-smoke` | Active | `tokenizer-basic`, `comments-and-text` proxy |
| `tok-attrs-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attrs-core` | Active | `tokenizer-attrs-before-after-name`, `tokenizer-attr-name` |
| `tok-before-attr-value-transitions` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-before-attr-value-transitions` | Active | `tokenizer-before-attr-value` |
| `tok-attr-value-quoted` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attr-value-quoted` | Active | `tokenizer-attr-value-quoted` |
| `tok-attr-value-unquoted` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attr-value-unquoted` | Active | `tokenizer-attr-value-unquoted` |
| `tok-comment-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-comment-core` | Active | `tokenizer-comments` |
| `tok-bogus-comment` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-bogus-comment` | Active | `tokenizer-bogus-comment` |
| `tok-doctype-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-core` | Active | `tokenizer-doctype-quirks` |
| `tok-doctype-public-system` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-public-system` | Active | `tokenizer-doctype-quirks` |
| `tok-doctype-quirks-missing-name` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-quirks-missing-name` | Active | `tokenizer-doctype-quirks` |
| `tok-charrefs-text` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-charrefs-text` | Active | `tokenizer-charrefs-text` |
| `tok-charrefs-attr` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-charrefs-attr` | Active | `tokenizer-charrefs-attr` |
| `tok-ae4-malformed-static` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-ae4-malformed-static` | Active | none currently vendored |
| `tok-ae4-markup-eof-recovery` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-ae4-markup-eof-recovery` | Active | none currently vendored |
| `tok-ae4-malformed-comment` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-ae4-malformed-comment` | Active | none currently vendored |
| `tok-rawtext-style` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rawtext-style` | Active | none currently vendored |
| `tok-rawtext-style-end-tag-attrs-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rawtext-style-end-tag-attrs-close` | Active | none currently vendored |
| `tok-rawtext-style-end-tag-slash-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rawtext-style-end-tag-slash-close` | Active | none currently vendored |
| `tok-rcdata-title` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-title` | Active | none currently vendored |
| `tok-rcdata-textarea` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-textarea` | Active | none currently vendored |
| `tok-rcdata-title-close-tag-whitespace` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-title-close-tag-whitespace` | Active | none currently vendored |
| `tok-rcdata-title-end-tag-attrs-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-title-end-tag-attrs-close` | Active | none currently vendored |
| `tok-rcdata-textarea-end-tag-slash-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-textarea-end-tag-slash-close` | Active | none currently vendored |
| `tok-script-data-basic` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-basic` | Active | `tokenizer-script-data` (basic close-tag behavior) |
| `tok-script-data-close-tag-whitespace` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-close-tag-whitespace` | Active | none currently vendored |
| `tok-script-data-end-tag-attrs-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-end-tag-attrs-close` | Active | none currently vendored |
| `tok-script-data-end-tag-slash-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-end-tag-slash-close` | Active | none currently vendored |
| `tok-script-data-string-close` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-string-close` | Active | none currently vendored |
| `tok-script-data-escaped-comment-family` | Yes (`MVP_PARTIAL`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-escaped-comment-family` | Active | `tokenizer-script-escaped` |

## Out-Of-Scope Policy (Enforceable)

Out-of-scope behavior is not represented as `xfail`.
It is represented as `skip` with policy reason.

### Policy Patterns (Core v0)

There are currently no vendored tokenizer WPT script-family patterns that are
out of scope after G5 landed.

Harness policy contract:

1. If future out-of-scope tokenizer script-family cases are vendored, manifest
   loader must reject matching cases unless `status: skip`.
2. CI reporting must keep `skip` separate from `xfail`.
3. `xfail` is only for in-scope known-broken behavior.

## Explicit Unsupported / Deferred Contract

AE12 recognizes PIs only from the Data/TagOpen route. Focused negative tests
prove that PI-like bytes stay text in currently supported RCDATA, RAWTEXT,
ScriptData (including escaped/double-escaped families), and foreign-content
CDATA. `PLAINTEXT` remains a pre-existing unsupported tokenizer state and is
not added by AE12.

AE12 standard-profile cases are pinned separately in
`tests/wpt/provenance/ae12-supported-profile.provenance.txt`. PI target/data
limit tests are additive Borrowser hardening fixtures and do not count as
WHATWG/WPT conformance.

For HTML5 Core v0:

- Core-v0 supports RAWTEXT, RCDATA, and the dedicated script tokenizer family (`TOK-STATE-SCRIPT-DATA`, `TOK-STATE-SCRIPT-DATA-ESCAPED`) as `MVP_PARTIAL`.
- Core-v0 shared text-mode close-tag recognition now includes HTML-space-led attribute tails and `/` self-closing tails for the bounded RAWTEXT/RCDATA/script subset.
- Script-data escaped and double-escaped families are in scope for Core v0; parser execution/pause semantics remain out of scope.
- AE4's static-HTML tokenizer work does not promote unsupported tokenizer
  behavior to full WHATWG parity. Behavior outside the rows and fixtures above
  remains deferred until explicitly classified and covered.
- Deferred/out-of-scope WPT cases must not contribute to Core v0 pass/fail gate counts.
- Any tier changes require an explicit update to this matrix and acceptance table.
