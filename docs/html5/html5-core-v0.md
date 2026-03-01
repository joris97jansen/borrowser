# HTML5 Core v0 Supported Subset Contract (Milestone D3)

Last updated: 2026-02-13
Scope: `crates/html/src/html5` (feature `html5`)
Normative matrix sources:
- `docs/html5/spec-matrix-tokenizer.md`
- `docs/html5/spec-matrix-treebuilder.md`

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

## Guaranteed Support In Core v0

Core v0 guarantees behavior for the following scope.

<a id="input-and-streaming-model"></a>
### Input And Streaming Model

- Input model is UTF-8 text streaming through session/tokenizer (`push_input` + `finish` flow).
- Whole-input and chunked-input execution MUST be semantically equivalent for all Core v0 gate tests.
- Parser state machines MUST be resumable at chunk boundaries without token duplication or token loss.

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

Character references are guaranteed only in:

- Data text context.
- Attribute value contexts (`DQ`, `SQ`, `UQ`).
- For those contexts, Core v0 guarantees named and numeric decoding behavior exactly as implemented by `crates/html/src/entities.rs`; divergence from that module behavior is an in-scope Core v0 bug.
- Legacy semicolon-less behavior is contractual only to the extent currently implemented in `entities.rs` for the supported contexts above.
- Deferred/explicitly out of Core v0 charref parity (unless already covered by `entities.rs` behavior):
  - full WHATWG context-sensitive semicolonless named-reference rules in every edge context,
  - attribute-specific ambiguous-ampersand branches beyond the implemented subset,
  - guaranteed full named-entity table parity beyond the currently shipped decoder behavior.

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
- Algorithms/invariants:
  - `TB-ALGO-REPROCESS`
  - `TB-ALGO-SOE`
  - `TB-ALGO-AFE` (`MVP_PARTIAL`)
  - `TB-ALGO-QUIRKS-DOCTYPE`
  - `TB-ALGO-PATCH-SINK`
  - `TB-ALGO-TABLE-ROBUSTNESS` (`MVP_PARTIAL`)

Core v0 tree-builder partial-scope guards:

- `TB-ALGO-AFE` (`MVP_PARTIAL`) guarantees basic AFE marker handling and reconstruction for simple inline formatting cases; full adoption-agency behavior remains deferred to `TB-ALGO-AAA`.
- `TB-ALGO-REPROCESS` guarantees that reprocessing reuses the same token instance and does not emit duplicate patches for a single logical token unless explicitly required by the spec algorithm.

### Text Coalescing Policy (Core v0)

- Tree-builder text coalescing is controlled by `TreeBuilderConfig::coalesce_text`.
- When enabled, coalescing is deterministic and parent-local:
  - first adjacent text insertion under a parent emits `CreateText` then `AppendChild`,
  - subsequent adjacent text insertions under the same parent emit `SetText` on that same text-node key with cumulative content.
- Coalescing MUST stop on any structural boundary, including:
  - document materialization (`CreateDocument`),
  - element insertion,
  - successful SOE pop/end-tag closure,
  - comment insertion,
  - recovery literalization boundaries.
- Batch/chunk boundaries MUST NOT change semantic coalescing behavior:
  - whole-input and chunked-input runs must converge to the same final DOM,
  - patch logs must remain deterministic under different drain boundaries.
- Core v0 performance tradeoff (intentional and tracked):
  - current coalescing semantics use cumulative `SetText` payloads for adjacent runs,
  - this keeps patch semantics simple and deterministic but can increase payload-copy cost for very long tokenized text runs,
  - planned evolution paths are: add append-style text patches (for example `AppendText`) or emit a single `SetText` at run-flush boundaries with explicitly documented observer semantics.

<a id="supported-tags-and-contexts-baseline"></a>
### Supported Tags And Contexts Baseline

Core v0 guarantees the following tag/context baseline:

- Document bootstrap context:
  - implicit or explicit `html`, `head`, and `body` routing in early insertion modes.
- Head context (`TB-MODE-IN-HEAD`, partial):
  - guaranteed routing for `meta`, `link`, `base`.
  - guaranteed minimal `title` routing behavior.
  - comments and whitespace handling in head flow.
- Body context (`TB-MODE-IN-BODY`):
  - character insertion, comment insertion, and generic element insertion path for non-deferred constructs.
  - deterministic fallback behavior for unsupported table-family tags (robustness contract).
  - unknown or unsupported elements outside table-family MUST follow the generic element insertion path defined by `TB-MODE-IN-BODY`.
  - this unknown/unsupported-element rule applies only when the behavior is not explicitly marked `OUT_OF_SCOPE` with `skip` policy semantics (for example, template insertion mode stack behavior).

This is a context-level baseline, not a full tag-by-tag HTML5 completion claim.

<a id="attribute-rules-baseline"></a>
### Attribute Rules Baseline

Core v0 attribute behavior guarantees:

- tag and attribute names are tokenizer-normalized per HTML tokenizer rules (ASCII case folding in relevant states).
- attribute encounter order is preserved.
- duplicate attributes are deduped at tokenization stage with first-wins semantics on tokenizer-normalized attribute names (for example `a` and `A` are treated as duplicates).
- value forms supported: double-quoted, single-quoted, unquoted.
- character references in attribute values follow Core v0 charref scope and delegate named reference table/validation to `crates/html/src/entities.rs`.

<a id="doctype-and-quirks-stance"></a>
### DOCTYPE And Quirks Stance

Core v0 guarantees:

- tokenizer emits DOCTYPE token fields including `force_quirks`.
- tree builder determines document mode from DOCTYPE during early bootstrap.
- document mode MUST NOT change after the first non-DOCTYPE token that causes insertion of the root `html` element (implicit or explicit).
- duplicate/late DOCTYPE tokens after that boundary MUST NOT change document mode.
- document mode is internal parser state in Core v0 (no dedicated `DomPatch` mode event).

### Script/RAWTEXT/RCDATA Stance

Core v0 stance:

- Script-data tokenizer families are `OUT_OF_SCOPE`.
- RAWTEXT and RCDATA tokenizer families are `DEFERRED`.
- Tree-builder `Text` insertion mode is `DEFERRED` (coupled to deferred tokenizer text families).
- Parser-scripting interaction (parser pause/suspension and script execution integration) is not implemented in Core v0.

<a id="tables-stance"></a>
### Tables Stance

Core v0 stance:

- table insertion modes are `DEFERRED`.
- foster parenting is `DEFERRED`.
- robust unsupported-table behavior is required:
  - parser MUST remain deterministic,
  - parser MUST preserve core invariants (SOE/patch ordering),
  - parser MUST NOT panic on table-family tags.
- deterministic fallback semantics for Core v0:
  - table-family start/end tags are processed via the generic element path in `TB-MODE-IN-BODY`,
  - generic element path here means normal `In body` insertion flow (create element, push to SOE, emit deterministic patch sequence) without table-specific adjustments,
  - parser does not switch to table insertion modes,
  - foster parenting is not performed.

<a id="explicitly-unsupported-or-deferred-in-core-v0"></a>
## Explicitly Unsupported Or Deferred In Core v0

The following are intentionally not part of the Core v0 guarantee:

- `OUT_OF_SCOPE`:
  - tokenizer script-data escaped/double-escaped families (`TOK-STATE-SCRIPT-DATA`, `TOK-STATE-SCRIPT-DATA-ESCAPED`)
  - template insertion mode stack (`TB-ALGO-TEMPLATE-MODES`)
- `DEFERRED`:
  - tokenizer RAWTEXT and RCDATA families (`TOK-STATE-RAWTEXT`, `TOK-STATE-RAWTEXT-END-TAG`, `TOK-STATE-RCDATA`, `TOK-STATE-RCDATA-END-TAG`)
  - tree-builder `TB-MODE-TEXT`
  - adoption agency algorithm (`TB-ALGO-AAA`)
  - foster parenting (`TB-ALGO-FOSTER`)
  - full table insertion-mode set (see tree-builder matrix `Early Table Stance`)

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

- Script-data escaped/double-escaped tokenizer families.
- Parser-scripting interaction (parser suspension/pause and script execution coupling).
- Template insertion mode stack.
- Table insertion modes and foster parenting semantics.
- Adoption agency algorithm (`TB-ALGO-AAA`).
- Full tree-builder text mode parity (`TB-MODE-TEXT`), including deferred RAWTEXT/RCDATA coupling.

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
