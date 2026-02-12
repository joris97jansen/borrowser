# HTML5 Tokenizer Spec Mapping Matrix (Milestone D1)

Last updated: 2026-02-12
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

## Current Repository Baseline (Before D1 Execution)

- `crates/html/src/html5/tokenizer/states.rs` defines only `TokenizerState::Data`.
- `crates/html/src/html5/tokenizer/mod.rs` has scaffold behavior (`push_input()` unimplemented, `finish()` can emit `EOF`).
- Existing tokenizer fixtures (renamed to acceptance-ID convention):
  - `crates/html/tests/fixtures/html5/tokenizer/tok-empty-eof` (`active`)
  - `crates/html/tests/fixtures/html5/tokenizer/tok-basic-text` (`xfail`)
  - `crates/html/tests/fixtures/html5/tokenizer/tok-simple-tags` (`xfail`)
  - `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-comment-smoke` (`xfail`)
- WPT manifest currently has tokenizer smoke case `tokenizer-basic` as `xfail`.

## State Matrix (Spec -> Files/Tests -> Status)

| ID | Tier | Spec state name | Anchor | Implementation mapping | Test mapping (current + planned) | Streaming notes | Key edge cases | Rationale |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `TOK-STATE-DATA` | MVP | `Data state` | `#data-state` | `crates/html/src/html5/tokenizer/states.rs`, `crates/html/src/html5/tokenizer/mod.rs`, `crates/html/src/html5/tokenizer/input.rs` | Current: `tok-empty-eof`, `tok-basic-text`. Planned WPT: `tokenizer-data-text`. | `STREAM-INV-01`, `STREAM-INV-02` are hard gates. | EOF-in-data, `<` and `&` dispatch, U+0000 handling. | Universal tokenizer baseline. |
| `TOK-STATE-TAG-OPEN` | MVP | `Tag open state` | `#tag-open-state` | `states.rs`, `mod.rs` | Current: `tok-simple-tags`. Planned fixture: `tok-tag-open-recovery`; planned WPT: `tokenizer-tag-open`. | Must suspend cleanly when declaration prefix is partial at chunk end. | `<!`, `</`, `<?`, invalid next-char fallback. | Required for structural token emission. |
| `TOK-STATE-END-TAG-OPEN` | MVP | `End tag open state` | `#end-tag-open-state` | `states.rs`, `mod.rs` | Current: `tok-simple-tags`. Planned WPT: `tokenizer-end-tag-open`. | Must preserve reconsume semantics across split `</`. | EOF after `</`, non-name after `</`, parse-error fallback path. | Required for balanced close-tag handling. |
| `TOK-STATE-TAG-NAME` | MVP | `Tag name state` | `#tag-name-state` | `states.rs`, `emit.rs`, `mod.rs` | Current: `tok-simple-tags`, WPT `tokenizer-basic`. Planned fixture: `tok-tag-name-case-folding`. | Must not lose partial name token on chunk boundary before whitespace or `>`. | ASCII case folding, NUL replacement, `/` and `>` transitions. | Required for deterministic atomization. |
| `TOK-STATE-BEFORE-ATTR-NAME` | MVP | `Before attribute name state` | `#before-attribute-name-state` | `states.rs`, `mod.rs` | Planned fixture: `tok-attrs-core`; planned WPT: `tokenizer-attrs-before-after-name`. | Whitespace runs may span chunks with no semantic drift. | whitespace skip, `/` self-closing handoff, `>` early close. | Core attribute parser entry. |
| `TOK-STATE-ATTR-NAME` | MVP | `Attribute name state` | `#attribute-name-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-attrs-core`; planned WPT: `tokenizer-attr-name`. | Partial attr names must persist until delimiter appears. | ASCII case fold, NUL replacement, parse-error chars (`"`, `'`, `<`, `=`). | Needed for stable attribute token shape. |
| `TOK-STATE-AFTER-ATTR-NAME` | MVP | `After attribute name state` | `#after-attribute-name-state` | `states.rs`, `mod.rs` | Planned fixture: `tok-attrs-core`; planned WPT: `tokenizer-attrs-before-after-name`. | Must not consume `=`/`/`/`>` twice when chunk split lands on delimiter. | repeated separators, implicit empty attr values, close/self-close transitions. | Ensures deterministic attr finalization. |
| `TOK-STATE-BEFORE-ATTR-VALUE` | MVP | `Before attribute value state` | `#before-attribute-value-state` | `states.rs`, `mod.rs` | Planned fixture: `tok-before-attr-value-transitions`; planned WPT: `tokenizer-before-attr-value`. | Transition dispatch must survive split quote characters. | quoted vs unquoted dispatch, `>` parse-error close. | Dispatch point for value semantics. |
| `TOK-STATE-ATTR-VALUE-DQ` | MVP | `Attribute value (double-quoted) state` | `#attribute-value-double-quoted-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-attr-value-quoted`; planned WPT: `tokenizer-attr-value-quoted`. | Unterminated quote at chunk end must yield `NeedMoreInput` with intact buffer. | `&` charref dispatch, quote close, embedded control handling. | Required for common attribute syntax. |
| `TOK-STATE-ATTR-VALUE-SQ` | MVP | `Attribute value (single-quoted) state` | `#attribute-value-single-quoted-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-attr-value-quoted`; planned WPT: `tokenizer-attr-value-quoted`. | Same as DQ; boundary behavior must be symmetric. | `&` charref dispatch, quote close, parse-error recovery. | Required for spec parity on quoted attrs. |
| `TOK-STATE-ATTR-VALUE-UQ` | MVP | `Attribute value (unquoted) state` | `#attribute-value-unquoted-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-attr-value-unquoted`; planned WPT: `tokenizer-attr-value-unquoted`. | Must preserve pending token when encountering split delimiter bytes. | parse-error chars (`"`, `'`, `<`, `=`, `` ` ``), whitespace/value termination. | Required for realistic HTML input tolerance. |
| `TOK-STATE-MARKUP-DECL-OPEN` | MVP | `Markup declaration open state` | `#markup-declaration-open-state` | `states.rs`, `mod.rs` | Current: `tok-doctype-comment-smoke`. Planned fixtures: `tok-comment-core`, `tok-doctype-core`. | Declaration prefix matching cannot overconsume partial `<!-`/`<!D`. | comment vs doctype dispatch, unknown declaration fallback. | Root dispatch for comment/doctype flows. |
| `TOK-STATE-COMMENT-CORE` | MVP | `Comment core states` | `#comment-start-state`, `#comment-start-dash-state`, `#comment-state`, `#comment-end-state` | `states.rs`, `emit.rs`, `mod.rs` | Current: `tok-doctype-comment-smoke`; planned fixtures: `tok-comment-core`, `tok-comment-weird-endings`; planned WPT: `tokenizer-comments`. | Comment temporary data must survive chunk splits around `--` and `>`. | EOF in comment, `--!>` handling, nested `<` comment sequences. | Required for web-compat error recovery. |
| `TOK-STATE-BOGUS-COMMENT` | MVP | `Bogus comment state` | `#bogus-comment-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-bogus-comment`; planned WPT: `tokenizer-bogus-comment`. | Must emit single comment token even when terminator arrives in later chunk. | `<?` and malformed declarations routed here. | Explicit parse-error recovery path. |
| `TOK-STATE-DOCTYPE` | MVP | `DOCTYPE state` | `#doctype-state` | `states.rs`, `emit.rs`, `mod.rs` | Current: `tok-doctype-comment-smoke`; planned fixture: `tok-doctype-core`; planned WPT: `tokenizer-doctype-quirks`. | Partial `DOCTYPE` keyword must not mis-route to bogus comment. | case-insensitive keyword handling. | Entry point for standards/quirks tokenization. |
| `TOK-STATE-BEFORE-DOCTYPE-NAME` | MVP | `Before DOCTYPE name state` | `#before-doctype-name-state` | `states.rs`, `mod.rs` | Planned fixture: `tok-doctype-core`. | Whitespace and EOF boundary behavior must be deterministic. | missing name triggers quirks path. | Needed for correct token fields. |
| `TOK-STATE-DOCTYPE-NAME` | MVP | `DOCTYPE name state` | `#doctype-name-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-doctype-core`; WPT reuse: `tokenizer-basic`. | Name accumulation must survive boundary before close quote/`>`. | case fold, NUL replacement, EOF mid-name. | Core doctype payload correctness. |
| `TOK-STATE-AFTER-DOCTYPE-NAME` | MVP | `After DOCTYPE name state` | `#after-doctype-name-state` | `states.rs`, `mod.rs` | Planned fixture: `tok-doctype-core`, `tok-doctype-public-system`. | Keyword dispatch (`PUBLIC`/`SYSTEM`) must not overconsume on short chunk. | public/system keyword recognition and recovery. | Required for public/system field parsing. |
| `TOK-STATE-BOGUS-DOCTYPE` | MVP | `Bogus DOCTYPE state` | `#bogus-doctype-state` | `states.rs`, `emit.rs`, `mod.rs` | Planned fixture: `tok-doctype-quirks-missing-name`; planned WPT: `tokenizer-doctype-quirks`. | Must preserve `force_quirks=true` through chunked malformed inputs. | malformed public/system IDs, EOF before `>`. | Needed for standards vs quirks token correctness. |
| `TOK-STATE-CHARREF-ENTRY` | MVP_PARTIAL | `Character reference state` | `#character-reference-state` | `states.rs`, `mod.rs`; delegated table/validation in `crates/html/src/entities.rs` | Planned fixtures: `tok-charrefs-text`, `tok-charrefs-attr`; planned WPT: `tokenizer-charrefs-text`, `tokenizer-charrefs-attr`. | Must checkpoint return state and resume correctly across chunk boundary. | context return rules (data vs attr value). | Charref dispatcher for all supported contexts. |
| `TOK-STATE-CHARREF-NAMED` | MVP_PARTIAL | `Named character reference state` | `#named-character-reference-state` | `states.rs`, `mod.rs`, `crates/html/src/entities.rs` | Existing hardening tests in `entities.rs` (`html5-entities` feature); planned fixtures above. | Longest-match scan must suspend without data loss on partial name tail. | semicolon rules, legacy forms, attribute restrictions. | Core named reference behavior. |
| `TOK-STATE-CHARREF-AMBIGUOUS-AMP` | MVP_PARTIAL | `Ambiguous ampersand state` | `#ambiguous-ampersand-state` | `states.rs`, `mod.rs`, `entities.rs` | Planned fixture: `tok-charrefs-attr`. | Return-to-state behavior must be stable under split alnum runs. | text vs attr handling differences. | Needed for spec-correct fallback behavior. |
| `TOK-STATE-CHARREF-NUMERIC` | MVP_PARTIAL | `Numeric character reference state family` | `#numeric-character-reference-state`, `#hexadecimal-character-reference-state`, `#decimal-character-reference-state`, `#numeric-character-reference-end-state` | `states.rs`, `mod.rs`, `entities.rs` | Existing numeric hardening in `entities.rs`; planned fixture: `tok-charrefs-text`. | Numeric parse state must hold partial digit runs across chunks. | overflow, surrogate, invalid scalar replacement behavior. | Core numeric reference correctness. |
| `TOK-STATE-RAWTEXT` | DEFERRED | `RAWTEXT state` | `#rawtext-state` | Planned: `states.rs`, `mod.rs` | Planned fixture: `tok-rawtext-style-end-tag`; planned WPT: `tokenizer-rawtext-style`. | Appropriate end-tag buffer must persist across chunked `</sty` + `le>`. | mismatched end-tag fallback to text. | Post-Core-v0 scope. |
| `TOK-STATE-RAWTEXT-END-TAG` | DEFERRED | `RAWTEXT end tag detection` | `#rawtext-end-tag-open-state`, `#rawtext-end-tag-name-state` | Planned: `states.rs`, `mod.rs` | Planned fixture: `tok-rawtext-style-end-tag`. | Temporary end-tag buffer must be chunk-safe and reset-safe. | false positive/negative close-tag detection. | Needed for robust RAWTEXT handling. |
| `TOK-STATE-RCDATA` | DEFERRED | `RCDATA state` | `#rcdata-state` | Planned: `states.rs`, `mod.rs` | Planned fixture: `tok-rcdata-title-charrefs`; planned WPT: `tokenizer-rcdata-title`. | Must combine charrefs + appropriate-end-tag logic under chunking. | charrefs in rcdata text, fallback behavior. | Deferred complexity after MVP stabilization. |
| `TOK-STATE-RCDATA-END-TAG` | DEFERRED | `RCDATA end tag detection` | `#rcdata-end-tag-open-state`, `#rcdata-end-tag-name-state` | Planned: `states.rs`, `mod.rs` | Planned fixture: `tok-rcdata-title-charrefs`. | Same buffer semantics as rawtext end-tag path. | mismatched close-tag fallback. | Deferred with RCDATA family. |
| `TOK-STATE-SCRIPT-DATA` | OUT_OF_SCOPE | `Script data state` | `#script-data-state` | Not scheduled in Core v0 | Policy fixture: `tok-script-data-out-of-scope` (`skip` only). | N/A in Core v0. | script text tokenization and close-tag detection. | Explicitly excluded from Core v0. |
| `TOK-STATE-SCRIPT-DATA-ESCAPED` | OUT_OF_SCOPE | `Script data escaped families` | `#script-data-escaped-state`, `#script-data-double-escaped-state` | Not scheduled in Core v0 | Policy fixture: `tok-script-data-out-of-scope` (`skip` only). | N/A in Core v0. | escaped/double-escaped transitions and temp buffer handling. | Highest complexity, intentionally deferred to dedicated milestone. |

## Core v0 Must-Support Subset

Core v0 passes only if all `MVP` rows and all `MVP_PARTIAL` rows pass their Core-v0 gate fixtures in both whole-input and UTF-8 chunked runs.

Required in Core v0:

1. Data, tag-open, end-tag-open, tag-name flows.
2. Attribute name/value flows (DQ/SQ/UQ).
3. Comment flows including bogus comment recovery.
4. DOCTYPE flows including quirks-token signaling (`force_quirks`).
5. Character reference flows in explicitly scoped contexts.

### Character References Scope For Core v0 (`MVP_PARTIAL`)

- Included contexts:
  - Data text.
  - Attribute value states (`DQ`, `SQ`, `UQ`).
- Included forms:
  - Named references (longest match).
  - Decimal and hexadecimal numeric references.
- Delegation contract:
  - Tokenizer delegates named-reference table and scalar validation behavior to `crates/html/src/entities.rs` and must match that behavior.
- Deferred inside charrefs:
  - Full legacy named-reference edge matrix beyond the hardened `entities.rs` behavior in current scope.

## Acceptance Inventory (Deterministic ID <-> Fixture Mapping)

Status source of truth:

- Tokenizer golden fixtures use `tokens.txt` headers (`# status`, `# reason`).
- WPT cases use `tests/wpt/manifest.txt` (`status`, `reason`).

| Acceptance ID | Core v0 gate | Canonical fixture dir | Status now | WPT reference |
| --- | --- | --- | --- | --- |
| `tok-empty-eof` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-empty-eof` | Active | `tokenizer-basic` (EOF) |
| `tok-basic-text` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-basic-text` | XFail | planned `tokenizer-data-text` |
| `tok-simple-tags` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-simple-tags` | XFail | `tokenizer-basic` |
| `tok-doctype-comment-smoke` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-comment-smoke` | XFail | `tokenizer-basic`, `comments-and-text` proxy |
| `tok-attrs-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attrs-core` | XFail | `tokenizer-attrs-before-after-name`, `tokenizer-attr-name` |
| `tok-before-attr-value-transitions` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-before-attr-value-transitions` | XFail | `tokenizer-before-attr-value` |
| `tok-attr-value-quoted` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attr-value-quoted` | XFail | `tokenizer-attr-value-quoted` |
| `tok-attr-value-unquoted` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-attr-value-unquoted` | XFail | `tokenizer-attr-value-unquoted` |
| `tok-comment-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-comment-core` | XFail | `tokenizer-comments` |
| `tok-bogus-comment` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-bogus-comment` | XFail | `tokenizer-bogus-comment` |
| `tok-doctype-core` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-core` | XFail | `tokenizer-doctype-quirks` |
| `tok-doctype-public-system` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-public-system` | XFail | `tokenizer-doctype-quirks` |
| `tok-doctype-quirks-missing-name` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-doctype-quirks-missing-name` | XFail | `tokenizer-doctype-quirks` |
| `tok-charrefs-text` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-charrefs-text` | XFail | `tokenizer-charrefs-text` |
| `tok-charrefs-attr` | Yes | `crates/html/tests/fixtures/html5/tokenizer/tok-charrefs-attr` | XFail | `tokenizer-charrefs-attr` |
| `tok-rawtext-style-end-tag` | No (`DEFERRED`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rawtext-style-end-tag` | Planned | `tokenizer-rawtext-style` |
| `tok-rcdata-title-charrefs` | No (`DEFERRED`) | `crates/html/tests/fixtures/html5/tokenizer/tok-rcdata-title-charrefs` | Planned | `tokenizer-rcdata-title` |
| `tok-script-data-out-of-scope` | No (`OUT_OF_SCOPE`) | `crates/html/tests/fixtures/html5/tokenizer/tok-script-data-out-of-scope` | Planned (`skip`) | none in Core v0 |

## Out-Of-Scope Policy (Enforceable)

Out-of-scope behavior is not represented as `xfail`.
It is represented as `skip` with policy reason.

### Policy Patterns (Core v0)

The following WPT case IDs or paths must be marked `skip`:

- pattern ID `TOK-PATTERN-SCRIPT-01`: contains `script-data`
- pattern ID `TOK-PATTERN-SCRIPT-02`: contains `script-escaped`
- pattern ID `TOK-PATTERN-SCRIPT-03`: contains `script-double-escaped`

Harness policy contract:

1. Manifest loader must reject matching cases unless `status: skip`.
2. CI reporting must keep `skip` separate from `xfail`.
3. `xfail` is only for in-scope known-broken behavior.

## Explicit Unsupported / Deferred Contract

For HTML5 Core v0:

- Script-data escaped and double-escaped families are out of scope and `skip` only.
- RAWTEXT and RCDATA families are deferred and do not block Core v0 exit criteria.
- Deferred/out-of-scope WPT cases must not contribute to Core v0 pass/fail gate counts.
- Any tier changes require an explicit update to this matrix and acceptance table.
