# HTML5 Tree Builder Spec Mapping Matrix (Milestone D2)

Last updated: 2026-02-12
Scope: `crates/html/src/html5/tree_builder` (feature `html5`)
Spec source: WHATWG HTML, section `Tree construction` (`parsing.html#tree-construction`)

## Purpose

This document maps HTML5 tree-builder insertion modes and core tree-construction algorithms to Borrowser implementation files and acceptance-test planning.
It defines HTML5 Core v0 tree-builder scope and explicitly records deferred and out-of-scope areas.

## Stable IDs And Tier Labels

- Mapping IDs are stable and use either:
  - insertion modes: `TB-MODE-*`
  - algorithms/invariants: `TB-ALGO-*`
- Tier labels are machine-stable:
  - `MVP`
  - `MVP_PARTIAL`
  - `DEFERRED`
  - `OUT_OF_SCOPE`

## Component Contracts (Tokenizer vs Tree Builder vs Session)

- Tokenizer contract:
  - Produces ordered token stream with tokenizer-level normalization and parse-error recovery.
  - Emits DOCTYPE token fields including `force_quirks`.
  - Does not decide insertion mode behavior, foster parenting, AFE reconstruction, or document mode.
- Tree builder contract:
  - Owns insertion modes, stack of open elements (SOE), active formatting elements (AFE), and DOM construction semantics.
  - Applies tree-construction algorithms (AAA, foster parenting, template mode stack) when in scope.
  - Consumes tokenizer tokens deterministically and emits `DomPatch` through `PatchSink`.
- Session contract:
  - Orchestrates tokenizer/tree-builder pumping, counters, and error accounting.
  - Owns policy-level classification (`active`/`xfail`/`skip`) for WPT integration.

## Document Mode Propagation Contract (Quirks)

- Source of truth:
  - Tokenizer emits DOCTYPE token fields (`name`, public/system IDs, `force_quirks`).
  - Tree builder computes document mode from those fields in `TB-MODE-INITIAL`.
- Ownership:
  - Document mode is owned by shared parse context (`DocumentParseContext`) at document scope.
  - Core v0 implementation may introduce a dedicated context field (for example, `document_mode`) to make this explicit.
- Immutability boundary:
  - Document mode becomes immutable when leaving early bootstrap (`Initial`/`Before html`) and must not be changed later by body tokens.
- Patch visibility:
  - Core v0: document mode is internal parser state (no dedicated `DomPatch` event).
  - If externalized later, it must be a single deterministic document-level signal.

## Current Repository Baseline (Before D2 Execution)

- Tree-builder state is scaffolding only:
  - `crates/html/src/html5/tree_builder/mod.rs` has TODO implementation in `push_token_impl`.
  - `crates/html/src/html5/tree_builder/modes.rs`, `stack.rs`, `formatting.rs` are placeholders.
- Existing golden tree-builder fixtures:
  - `crates/html/tests/fixtures/html5/tree_builder/empty` (`xfail`)
  - `crates/html/tests/fixtures/html5/tree_builder/text` (`xfail`)
  - `crates/html/tests/fixtures/html5/tree_builder/simple-element` (`xfail`)
- Existing WPT DOM cases are currently `xfail`:
  - `basic-structure`, `comments-and-text`, `void-elements`

## Insertion Mode Matrix (Spec -> Files/Tests -> Status)

| ID | Tier | Insertion mode | Spec anchor | Implementation mapping | Test mapping (current + planned) | Key edge cases | Acceptance placeholder | Rationale |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `TB-MODE-INITIAL` | MVP | `Initial` | `#the-initial-insertion-mode` | `tree_builder/mod.rs`, `tree_builder/modes.rs` | Current: `tree_builder/empty`; WPT: `basic-structure`. | DOCTYPE handling at document start, parse-error recovery before `<html>`. | `tb-initial-doctype` | Required for document bootstrap and doctype handoff. |
| `TB-MODE-BEFORE-HTML` | MVP | `Before html` | `#the-before-html-insertion-mode` | `mod.rs`, `modes.rs` | Current: `tree_builder/empty`, `tree_builder/simple-element`. Planned fixture: `tb-before-html-implicit-root`. | Implicit `<html>` insertion, stray comments/whitespace before root. | `tb-before-html-implicit-root` | Core parser behavior for malformed/short documents. |
| `TB-MODE-BEFORE-HEAD` | MVP | `Before head` | `#the-before-head-insertion-mode` | `mod.rs`, `modes.rs` | Current proxy: WPT `basic-structure`. Planned fixture: `tb-before-head-implicit-head`. | Implicit `<head>` insertion, head-element start/end transitions. | `tb-before-head-implicit-head` | Needed to route early metadata/body transitions correctly. |
| `TB-MODE-IN-HEAD` | MVP_PARTIAL | `In head` | `#parsing-main-inhead` | `mod.rs`, `modes.rs` | Planned fixtures: `tb-in-head-meta-title`, `tb-in-head-misnested-head-content`. WPT proxy: `basic-structure`. | Proper handling of head-only tags vs body fallback; limited title/script handling in Core v0. | `tb-in-head-core` | Core head parsing needed; full script/template behavior deferred. |
| `TB-MODE-AFTER-HEAD` | MVP | `After head` | `#parsing-main-afterhead` | `mod.rs`, `modes.rs` | Current proxy: `tree_builder/simple-element`, WPT `basic-structure`. Planned fixture: `tb-after-head-body-bootstrap`. | Implicit `<body>` insertion; mispositioned head tokens after head close. | `tb-after-head-body-bootstrap` | Required handoff into main body mode. |
| `TB-MODE-IN-BODY` | MVP | `In body` | `#parsing-main-inbody` | `mod.rs`, `modes.rs`, `stack.rs`, `formatting.rs` | Current: `tree_builder/text`, `tree_builder/simple-element`; WPT: `comments-and-text`, `void-elements`. Planned fixture pack: `tb-in-body-core-inline-block`. | Character token insertion, end-tag matching via SOE, formatting elements hooks, comment insertion; unsupported table-family tags must follow deterministic fallback path without invariant breakage. | `tb-in-body-core` | Main workload mode; required for Core v0 semantics. |
| `TB-MODE-TEXT` | DEFERRED | `Text` | `#the-text-insertion-mode` | `mod.rs`, `modes.rs` | Planned fixtures: `tb-text-mode-rcdata`, `tb-text-mode-rawtext`. | Return-to-original-mode mechanics and EOF in text mode. | `tb-text-mode-core` | Mostly coupled to tokenizer rawtext/rcdata/script completeness. |

## `TB-MODE-IN-HEAD` Core v0 Partial Scope

Included in `MVP_PARTIAL`:

- `meta`, `link`, `base` as core head-element routing behavior.
- `title` routing contract (mode switch + character accumulation behavior) at minimal correctness level.
- comments, whitespace character tokens, and parse-error-tolerant fallbacks to later modes.

Explicitly deferred from Core v0 `In head`:

- `script` handling.
- `template` handling and template mode stack interaction.
- `noscript` script-enabled parsing nuances.
- full `style`/RAWTEXT-dependent semantics (coupled to tokenizer deferred states).

## Algorithm Matrix (SOE/AFE/AAA/Foster/Template/Quirks)

| ID | Tier | Algorithm / structure | Spec anchor(s) | Implementation mapping | Test mapping (current + planned) | Key edge cases | Acceptance placeholder | Rationale |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `TB-ALGO-REPROCESS` | MVP | Token reprocessing semantics | `#tree-construction`, `#parsing-main-inbody` | `tree_builder/mod.rs`, `tree_builder/modes.rs` | Planned fixtures: `tb-reprocess-mode-switch`, `tb-reprocess-no-duplicate-patch`. | Reprocess current token after insertion-mode switch without losing token identity and without duplicate patch emission. | `tb-reprocess-core` | Required for spec-faithful mode transitions and deterministic emission. |
| `TB-ALGO-SOE` | MVP | Stack of open elements (SOE) | `#stack-of-open-elements` | `tree_builder/stack.rs`, `tree_builder/mod.rs` | Current proxy: `tree_builder/simple-element`. Planned fixtures: `tb-soe-nested-pop`, `tb-soe-implied-end-tags`. | Correct push/pop ordering, implied end tags, scope checks for end-tag processing. | `tb-soe-core` | Foundational invariant for all insertion modes. |
| `TB-ALGO-AFE` | MVP_PARTIAL | Active formatting elements (AFE) | `#the-list-of-active-formatting-elements` | `tree_builder/formatting.rs`, `mod.rs` | Planned fixtures: `tb-afe-reconstruction-basic`, `tb-afe-marker-boundary`. | Reconstruction triggers, marker boundaries, duplicate formatting entries. | `tb-afe-core` | Needed for in-body formatting fidelity; full AAA interaction staged. |
| `TB-ALGO-AAA` | DEFERRED | Adoption agency algorithm (AAA) | `#adoption-agency-algorithm` | `tree_builder/formatting.rs`, `mod.rs` | Planned fixtures: `tb-aaa-misnested-b-i`, `tb-aaa-misnested-nesting-depth`. | Misnested formatting tags and reparenting complexity. | `tb-aaa-core` | High complexity; defer until SOE/AFE core is stable. |
| `TB-ALGO-FOSTER` | DEFERRED | Foster parenting | `#foster-parenting` | `tree_builder/mod.rs`, planned table-mode helpers | Planned fixtures: `tb-foster-text-in-table`, `tb-foster-misnested-phrasing`. | Text/phrasing content in table contexts requiring foster insertion location. | `tb-foster-core` | Coupled to table insertion modes; defer with tables. |
| `TB-ALGO-TABLE-ROBUSTNESS` | MVP_PARTIAL | Unsupported table-tag robustness in Core v0 | `#parsing-main-intable`, `#parsing-main-inbody` | `tree_builder/mod.rs`, `tree_builder/modes.rs`, `tree_builder/stack.rs` | Planned fixture: `tb-table-tags-dont-explode`. | Encountering `<table>`-family tags must not panic, corrupt SOE invariants, or emit nondeterministic patches when table modes are deferred. | `tb-table-tags-dont-explode` | Ensures production robustness under unsupported constructs. |
| `TB-ALGO-TEMPLATE-MODES` | OUT_OF_SCOPE | Template insertion modes stack | `#parsing-main-intemplate`, `#stack-of-template-insertion-modes` | planned `tree_builder/modes.rs` | Planned policy fixture: `tb-template-out-of-scope` (`skip`). | Template mode push/pop and reprocessing semantics. | `tb-template-out-of-scope` | Excluded from Core v0 for complexity/runway reasons. |
| `TB-ALGO-QUIRKS-DOCTYPE` | MVP | Quirks mode + doctype effects | `#the-initial-insertion-mode`, `#the-before-html-insertion-mode` | `tree_builder/mod.rs`, `html5/shared/context.rs` (mode/counters), tokenizer doctype token handoff | Current tokenizer proxy: `tok-doctype-quirks-missing-name`. Planned tree fixture: `tb-quirks-from-doctype`. | Document mode decision from DOCTYPE token (`force_quirks` true/false), deterministic propagation and immutability boundary. | `tb-quirks-from-doctype` | Required for standards/quirks compatibility boundary. |
| `TB-ALGO-PATCH-SINK` | MVP | Deterministic patch emission contract | engine contract (`docs/adr/001-html5-parsing-architecture.md`, section `DomPatch contract (html5 tree builder)`) | `tree_builder/mod.rs`, `tree_builder/emit.rs` | Current harness path: `html5_golden_tree_builder.rs` + patch materialization. Planned fixture: `tb-patch-order-stability`. | Stable ordering, deterministic document bootstrap, no invalid patch sequencing. | `tb-patch-order-stability` | Runtime consumes patches incrementally; determinism is non-negotiable. |

## Reprocess Control-Flow Contract (`TB-ALGO-REPROCESS`)

- Tree-builder token handling must use an explicit mode-dispatch loop shape:
  - `loop { dispatch(current_mode, current_token) }`
  - dispatch returns an explicit step/result that can request `Reprocess` in a new mode.
- Reprocessing is intra-token only:
  - The token is not re-fetched from tokenizer input.
  - Token identity means the same `Token` value (and associated parser-side buffers) remains stable across reprocess steps; implementation must not re-lex or reconstruct a new token from input bytes.
  - The same logical token instance is processed until terminal `Continue`/`Done` behavior for that token.
- Reprocessing must be emission-safe:
  - No duplicate patch emission for the same semantic action during mode switches.
  - Reprocessing must be implemented with the iterative dispatch loop above; recursion is not permitted in Core v0.
  - Implementation should include a bounded reprocess-iteration guard in debug builds (for example, max 64 loop iterations per token) to catch infinite mode-flip bugs early.

## SOE Ownership And Ordering Invariants (`TB-ALGO-SOE`)

- SOE is a parsing structure, not the authoritative DOM container.
- SOE entries must reference element handles that map 1:1 to already-created DOM nodes.
- Ordering contract:
  - element creation patch/state update occurs before pushing that element handle onto SOE.
  - SOE pop operations do not emit structural patches by default.
  - Exception: implied end-tag processing may pop SOE entries and may emit corresponding close semantics when the patch model requires it.
    - Current `DomPatch` model has no explicit close operation; closure is represented by SOE updates and subsequent insertion/parenting behavior unless another explicit patch type (for example, reparent/remove) is required by the algorithm.
- SOE and emitted DOM state must never diverge for live open elements.

## Patch Emission Granularity (`TB-ALGO-PATCH-SINK`)

- Patch emission is token-bounded by default:
  - tree builder should not buffer patches across token boundaries unless a specific spec algorithm requires delayed emission.
- Delayed emission is allowed only for spec-mandated algorithms.
  - Examples (non-exhaustive): adoption agency algorithm (`TB-ALGO-AAA`), foster parenting (`TB-ALGO-FOSTER`), template insertion mode stack reprocessing (`TB-ALGO-TEMPLATE-MODES`).
- Reprocessing loops remain within the current token boundary and may emit during that loop, but must preserve deterministic order and single-action semantics.
- Streaming objective:
  - avoid unbounded cross-token buffering and keep memory growth proportional to current-token work.

## Early Table Stance (Core v0 Decision)

Tables are `DEFERRED` for Core v0.

Scope consequence:

- `In table`, `In table text`, `In caption`, `In column group`, `In table body`, `In row`, `In cell`, `In select in table` insertion modes are deferred.
- Foster parenting is deferred with table-mode implementation.
- WPT/table-heavy cases must not be Core v0 gates until this status is promoted.
- Robustness requirement still applies (`TB-ALGO-TABLE-ROBUSTNESS`): unsupported table tags must not break parser invariants.

Rationale:

- Table insertion + foster parenting materially increases algorithm complexity and misnested-content surface area.
- Core v0 value is higher by stabilizing Initial/Head/Body + SOE/AFE core first.

## Core v0 Tree Builder Subset

Core v0 tree-builder completion requires:

1. `TB-MODE-INITIAL`, `TB-MODE-BEFORE-HTML`, `TB-MODE-BEFORE-HEAD`, `TB-MODE-AFTER-HEAD`, `TB-MODE-IN-BODY`.
2. `TB-MODE-IN-HEAD` at `MVP_PARTIAL` scope sufficient for basic document/head routing.
3. `TB-ALGO-SOE` + `TB-ALGO-QUIRKS-DOCTYPE` + deterministic patch emission contract.
4. `TB-ALGO-AFE` at `MVP_PARTIAL` scope (basic reconstruction/markers, without full AAA).
5. `TB-ALGO-REPROCESS` behavior across mode transitions.
6. `TB-ALGO-TABLE-ROBUSTNESS` for deterministic unsupported-table handling.

Deferred/out-of-scope algorithms do not gate Core v0 exit criteria.

### No Partial AAA Guard (Core v0)

- Core v0 must not implement a partial subset of the adoption agency algorithm.
- Until `TB-ALGO-AAA` is promoted from `DEFERRED`, misnested formatting end-tag handling must follow a single documented fallback path covered by fixtures.
- Any AAA introduction must be explicit and gated with dedicated acceptance fixtures.

## Acceptance Inventory Placeholder (Tree Builder)

Status source of truth:

- Golden tree-builder fixtures use `dom.txt` headers (`# status`, `# reason`).
- WPT uses `tests/wpt/manifest.txt` (`status`, `reason`).

| Acceptance ID | Core v0 gate | Canonical fixture dir (planned/current) | Status now | Notes |
| --- | --- | --- | --- | --- |
| `tb-empty-smoke` | Yes | current `crates/html/tests/fixtures/html5/tree_builder/empty` | XFail | Document bootstrap smoke. |
| `tb-text-smoke` | Yes | current `crates/html/tests/fixtures/html5/tree_builder/text` | XFail | Character insertion smoke in body. |
| `tb-simple-element-smoke` | Yes | current `crates/html/tests/fixtures/html5/tree_builder/simple-element` | XFail | Basic start/end/comment structure. |
| `tb-initial-doctype` | Yes | planned `crates/html/tests/fixtures/html5/tree_builder/tb-initial-doctype` | Planned | Initial mode + doctype routing. |
| `tb-before-html-implicit-root` | Yes | planned `.../tb-before-html-implicit-root` | Planned | Implicit html element creation. |
| `tb-before-head-implicit-head` | Yes | planned `.../tb-before-head-implicit-head` | Planned | Implicit head insertion. |
| `tb-in-head-core` | Yes (Partial) | planned `.../tb-in-head-core` | Planned | Head mode partial coverage for Core v0. |
| `tb-after-head-body-bootstrap` | Yes | planned `.../tb-after-head-body-bootstrap` | Planned | Transition into body mode. |
| `tb-in-body-core` | Yes | planned `.../tb-in-body-core` | Planned | Core body algorithm surface. |
| `tb-soe-core` | Yes | planned `.../tb-soe-core` | Planned | SOE push/pop/scope behavior. |
| `tb-afe-core` | Yes (Partial) | planned `.../tb-afe-core` | Planned | AFE reconstruction/markers basic behavior. |
| `tb-reprocess-core` | Yes | planned `.../tb-reprocess-core` | Planned | Mode-switch token reprocessing invariants. |
| `tb-quirks-from-doctype` | Yes | planned `.../tb-quirks-from-doctype` | Planned | force_quirks propagation to document mode. |
| `tb-table-tags-dont-explode` | Yes (Partial) | planned `.../tb-table-tags-dont-explode` | Planned | Unsupported table tags keep parser deterministic and invariant-safe. |
| `tb-aaa-core` | No | planned `.../tb-aaa-core` | Planned | Deferred: adoption agency algorithm coverage. |
| `tb-foster-core` | No | planned `.../tb-foster-core` | Planned | Deferred: foster parenting in table contexts. |
| `tb-text-mode-core` | No | planned `.../tb-text-mode-core` | Planned | Deferred: text insertion mode return semantics. |
| `tb-template-out-of-scope` | No | planned `.../tb-template-out-of-scope` | Planned (`skip`) | Out-of-scope: template insertion modes stack excluded. |

## Spec Modes Not Yet In Core v0 Matrix

These spec insertion modes exist but are intentionally not Core v0 targets yet:

- `After body`
- `After after body`
- `In frameset`
- `After frameset`
- `After after frameset`
- table-family insertion modes listed in `Early Table Stance`

## Out-Of-Scope / Deferred Contract

For HTML5 Core v0 tree builder:

- Table insertion modes and foster parenting are `DEFERRED`.
- Template insertion modes are `OUT_OF_SCOPE` and should be `skip` in policy-driven manifests.
- Adoption agency algorithm is `DEFERRED` and not a Core v0 gate.
- Any status promotion requires explicit matrix update and acceptance plan update.
