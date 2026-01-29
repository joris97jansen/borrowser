# ADR 001: HTML5 Parsing Architecture (Tokenizer → Tree Builder → DomPatch)

Date: 2026-01-29
Status: Accepted (architecture locked; implementation staged under feature gate)

## Context
Borrowser needs an HTML5-compliant parsing architecture that supports streaming input and incremental rendering without building a full DOM and then diffing it. The current runtime pipeline consumes `Tokenizer` tokens and feeds a `TreeBuilder` that emits `DomPatch` updates; this ADR formalizes the future HTML5 parser architecture (tokenizer + tree builder), its module boundaries, ownership rules, error strategy, and integration plan with `runtime_parse`.

Goals:
- Spec-aligned tokenizer state machine and tree builder insertion modes.
- Streaming, resumable parsing on arbitrary chunk boundaries.
- `DomPatch` as the first-class output model (no “build DOM then diff”).
- Clear invariants across atoms, handles, patch keys, and versioning.
- Performance model with zero-copy spans where possible and bounded allocations.

Non-goals:
- Full implementation in this milestone.
- Complete HTML5 edge-case coverage immediately; correctness can be staged.

## Decision
We will implement an HTML5 parsing pipeline with a tokenizer state machine feeding a tree builder with insertion modes. The tree builder is the sole owner of DOM construction and `DomPatch` emission. Tokenization and tree building are streaming and resumable, with explicit state structs and no hidden global state.

### Architecture shape
- **Tokenizer**
  - Implements the HTML5 tokenizer state machine (spec-faithful states and transitions).
  - Exposes a streaming API: chunks are appended, tokens are drained.
  - Maintains `reconsume` and pending buffers for partial tokens across chunk boundaries.
  - Emits tokens that reference `AtomId` and zero-copy spans into the decoded `Input` buffer (owned by the session/pre-tokenizer stage) where possible.

- **Tree builder**
  - Implements HTML5 insertion modes and the stack of open elements + active formatting list.
  - Consumes tokens and emits `DomPatch` operations as the primary output.
  - Maintains its own arena/ID allocator and ensures patch protocol invariants.

- **Output**
  - `DomPatch` is the primary output from the tree builder.
  - The “DOM-first then diff” path remains as a debug/test path only.

### Streaming model
- Tokenizer input is a Unicode scalar stream (decoded text input). A separate pre-tokenizer stage performs byte decoding and charset sniffing/locking.
- The architecture does not assume UTF-8 at the tokenizer boundary; decoding must handle BOM, HTTP headers, and `<meta charset>` sniffing per HTML5 rules.
- Tokenizer (or the decoded `Input` it consumes) owns an append-only source buffer (or segmented buffer) to support `TextSpan` and `AttributeValue::Span` without copying.
- Tokens are drained in batches; the tokenizer preserves enough buffer history to keep spans valid until the tree builder has consumed them.
- Tree builder is fully resumable; it can pause between tokens and resume with preserved internal state.
- EOF is explicit (`finish()`/`push_eof()`), ensuring end-of-file processing runs exactly once.

### Module boundaries & ownership
Proposed module layout in `crates/html`:
- `html::html5::tokenizer` (public, new): HTML5 tokenizer state machine and streaming API.
- `html::html5::tree_builder` (public, new): HTML5 insertion modes, DOM construction, and `DomPatch` emission.
- `html::html5::shared` (internal, `pub(crate)`): shared token/span/input/error types.
- `html::html5::session` (public, feature gated): runtime entrypoint and lifecycle owner.
- `html::html5::bridge` (internal, transitional): legacy adapters for the current pipeline.
- `html::dom_patch` (public): patch protocol and invariants (already exists).
- `html::types` (internal): stable IDs, atom table, shared token types (already exists; may add HTML5-specific token variants if needed).

Ownership rules:
- Tokenizer owns:
  - Source buffer and any zero-copy spans.
  - Tokenizer state machine and parse error accumulator.
- DocumentParseContext (new) owns:
  - Atom table (`AtomTable`) for canonical ASCII-folded names (HTML namespace matching).
  - Encoding policy/state (e.g., charset lock state) and document-level sinks/metrics.
  - Shared, document-lifetime resources used by tokenizer and tree builder.
- Tree builder owns:
  - Node/key allocator and DOM construction state.
  - Patch emission buffer and invariant validation.
  - Open element stack and active formatting list.
- Runtime pipeline owns:
  - Batching/flush policy and patch delivery.
  - Document handle + versioning used by the UI `DomStore`.

### Public/internal APIs (sketch)
Public API (feature gated, `html5-parse`):
- `pub struct Html5Tokenizer { ... }`
  - `new(config, ctx: &mut DocumentParseContext) -> Self`
  - `push_input(&mut self, input: &mut Input) -> TokenizeResult` (consumes decoded Unicode scalar input; processes until it needs more input or reaches EOF)
  - `next_batch(&mut self) -> TokenBatch` (tokens/spans valid for the batch epoch; includes `TextResolver` view bound to the batch)
  - `finish(&mut self) -> TokenizeResult` (emit EOF)
- `pub struct DocumentParseContext { ... }`
  - `atoms(&self) -> &AtomTable`
  - `decoder(&mut self) -> &mut ByteStreamDecoder`
- `pub struct Html5ParseSession { ... }` (runtime entrypoint; feature gated)
  - Owns `ByteStreamDecoder`, decoded `Input`, `Html5Tokenizer`, and `Html5TreeBuilder`
  - `push_bytes(&mut self, bytes: &[u8]) -> Result<(), EngineInvariantError>`
  - `pump(&mut self) -> Result<(), EngineInvariantError>`
  - `take_patches(&mut self) -> Vec<DomPatch>`
- `pub struct Html5TreeBuilder { ... }`
  - `new(config, ctx: &mut DocumentParseContext) -> Self`
  - `push_token(&mut self, token: &Token, atoms: &AtomTable, text: &dyn TextResolver) -> Result<(), TreeBuilderError>`
  - `take_patches(&mut self) -> Vec<DomPatch>`
  - Note: `Html5Tokenizer` and `Html5TreeBuilder` are public for testing/tooling, but runtime integration must go through `Html5ParseSession`.

Internal API:
- Tokenizer state enums and insertion-mode enums are internal and not re-exported.
- `TokenTextResolver` remains internal: it provides text extraction from spans and owned buffers.

Error strategy:
- HTML5 tokenization parse errors are **non-fatal**; they are recorded and processing continues.
- HTML tree building is not expected to error on malformed HTML; it recovers per spec.
- Tree builder errors are **engine invariant violations** (e.g., invalid patch keys, impossible internal states). These are fatal for the current stream and result in a controlled reset (see failure modes).
- `TreeBuilderError` may be kept for recoverable/spec-level cases if ever introduced; `EngineInvariantError` is reserved for bug/corruption failures (aliasing allowed).
- Allocation failures or internal panics are treated as fatal and reported to `runtime_parse`.

### Invariants (explicit)
Tokenizer invariants:
- Source buffer is append-only while spans are live.
- For HTML namespace elements/attributes, matching is ASCII case-insensitive; atoms store a canonical ASCII-lowercased form for ASCII letters.
- `TextSpan` and `AttributeValue::Span` ranges are valid UTF-8 boundaries.

Tree builder invariants:
- `PatchKey` values are monotonic, non-zero, and never reused within a document lifetime.
- Patch streams are self-contained and deterministic.
- `DomPatch::Clear` (reset) may only appear as the first patch in a batch.
- Child ordering is explicit; no cycles; each node has at most one parent.

Runtime invariants:
- DOM handle + versioning is monotonic (`from -> to` increment only).
- Patch batches preserve order and are not interleaved across documents.

Token lifetime invariants:
- Token batches that borrow spans must be consumed within the same batch epoch.
- Tree builder must not store borrowed slices beyond a batch; it must intern/copy if data is retained.
- Source buffer trimming/compaction is only allowed when no outstanding batches exist.
- The batch epoch is enforced by a typed guard (`TokenBatch<'t>` + `TextResolver<'t>`) or by explicit copy/intern APIs on retention.

Encoding invariants:
- The decoder may use provisional encoding and supports a restart window until charset is locked.
- Charset locking policy is explicit and consistent across runs.

### Performance model
- Zero-copy spans (`TextSpan`/`AttributeValue::Span`) are used whenever token data is a direct slice of the source buffer.
- Interned atoms (`AtomTable`) avoid repeated allocations for tag/attribute names.
- Tokenizer uses bounded scanning for entities and tag names; no O(n²) growth from repeated concatenations.
- Patch buffers are size-capped and reused to limit allocator churn.

## Alternatives considered
1. **Spec-faithful tokenizer + tree builder** (chosen)
   - Pros: best long-term correctness, easier to validate against spec tests.
   - Cons: more complex state machine, longer implementation runway.

2. **Hybrid simplified tokenizer**
   - Pros: faster to implement, fewer states.
   - Cons: diverges from spec behavior in edge cases; harder to reason about correctness and streaming semantics.

3. **DOM-first then diff**
   - Pros: simpler tree builder; uses existing diff engine.
   - Cons: defeats streaming goals, higher memory usage, slower first paint; introduces O(n) diff cost per update.

## Consequences
- The `html` crate will expose a feature-gated HTML5 parser path while keeping the existing tokenizer/tree builder stable for now.
- Integration with `runtime_parse` will be opt-in to avoid behavior changes.
- Additional tests (spec corpus + streaming parity) are required before the new parser becomes default.

## Module boundaries & crate layout (html5 path)

This section defines the html5 parsing module boundaries, public surfaces, and dependency rules. It is a design contract; implementation can be staged behind feature gates.

### Folder skeleton plan

```
crates/html/
├── src/
│   ├── lib.rs
│   ├── html5/
│   │   ├── mod.rs
│   │   ├── tokenizer/
│   │   │   ├── mod.rs
│   │   │   ├── states.rs
│   │   │   ├── input.rs
│   │   │   ├── emit.rs
│   │   │   └── tests.rs
│   │   ├── tree_builder/
│   │   │   ├── mod.rs
│   │   │   ├── modes.rs
│   │   │   ├── stack.rs
│   │   │   ├── formatting.rs
│   │   │   ├── emit.rs
│   │   │   └── tests.rs
│   │   ├── shared/
│   │   │   ├── mod.rs
│   │   │   ├── input.rs
│   │   │   ├── span.rs
│   │   │   ├── atom.rs
│   │   │   ├── token.rs
│   │   │   ├── error.rs
│   │   │   └── counters.rs
│   │   ├── session.rs
│   │   └── bridge/
│   │       ├── mod.rs
│   │       └── adapters.rs
│   └── dom_patch.rs
```

Notes:
- `html5/shared` contains the minimal shared types that are stable across tokenizer and tree builder and safe to expose in `html5` public APIs.
- `html5/bridge` is a temporary adapter layer to integrate with existing `TreeBuilder`/patch pipeline; it is explicitly transitional and can be removed once the html5 path is the default.

### Module list and public surface

**`html::html5::tokenizer` (public)**
- Public items:
  - `Html5Tokenizer`, `TokenizerConfig`
  - `TokenBatch`
  - `TokenizeResult`
- Internal-only:
  - State machine enums (`TokenizerState`)
  - Low-level scanner helpers (`states.rs`, `emit.rs`)
  - Input buffer ownership and carry handling details

**`html::html5::tree_builder` (public)**
- Public items:
  - `Html5TreeBuilder`, `TreeBuilderConfig`
  - `TreeBuilderStepResult`, `SuspendReason`
  - `TreeBuilderError`
- Internal-only:
  - Insertion modes (`InsertionMode`)
  - Stack of open elements and active formatting list types
  - Adoption agency and reconstruction helpers

**`html::html5::shared` (internal, `pub(crate)`)**
- Shared types (internal; re-exported selectively through `html::html5` root; downstream code must not import `shared::*` directly):
  - `Input` (decoded text stream abstraction)
  - `Span` / `TextSpan`
  - `Atom` / `AtomTable` (document-level)
  - `Token` (HTML5 token variants)
  - `ParseError`
  - `Counters` (tokenization/tree-builder stats)
- Internal-only:
  - Internal text resolver traits and token batch epoch guards
  - Public consumers must import shared types via `html::html5::{Token, Span, ParseError, ...}` only

**`html::html5::bridge` (internal, transitional)**
- Public items (within crate only):
  - `Html5BridgeAdapter` (tokenizer + tree builder glue to existing pipeline)
  - `PatchEmitterAdapter` (converts html5 tree builder output to `DomPatch` stream)
- Internal-only:
  - Legacy compatibility shims and feature-flag gating

**`html::html5::session` (public, feature gated)**
- Public items:
  - `Html5ParseSession` (runtime entrypoint and lifecycle owner)
- Internal-only:
  - Bridge wiring and parity comparison helpers

### Dependency direction rules (import policy)

Allowed dependencies (by module):
- `html5/shared` is foundational and must not depend on `tokenizer` or `tree_builder`.
- `html5/tokenizer` may depend only on `html5/shared` (no `dom_patch` dependency).
- `html5/tree_builder` may depend on `html5/shared` and `dom_patch` (for patch emission).
- `html5/session` may depend on `html5/tokenizer`, `html5/tree_builder`, `html5/shared`, and `html5/bridge`.
- `html5/bridge` may depend on `html5/tokenizer`, `html5/tree_builder`, `dom_patch`, and the legacy `TreeBuilder`/`Tokenizer` modules.

Disallowed dependencies:
- `html5/shared` must not depend on `html5/tokenizer` or `html5/tree_builder`.
- `html5/tokenizer` must not depend on `html5/tree_builder`.
- `html5/tree_builder` must not depend on `html5/tokenizer` internals (only `Token` + `TextResolver`).
- `html5/bridge` is the only module allowed to reference legacy parsing modules.

### Bridge layer contract (temporary)

The bridge layer exists to let `runtime_parse` select the html5 path without breaking current behavior:
- Converts legacy byte chunks into `ByteStreamDecoder` + `Input`, then feeds `Html5Tokenizer`.
- Binds `TokenBatch` and `TextResolver` to `Html5TreeBuilder` consumption.
- Emits `DomPatch` batches identical to the legacy path’s expectations (including `CreateDocument`, `Clear`, and key allocation invariants).
- Provides a feature-gated “parity mode” that can emit both patch stream and owned DOM for test comparison.

## Integration plan (runtime_parse)
- Add a feature flag `html5-parse` (and `runtime-parse-html5` if needed) to select the new parser.
- Default path remains unchanged; feature-gated path wires:
  - `Html5Tokenizer` → `Html5TreeBuilder` → `DomPatch` emission.
- No behavior change without explicit feature enablement.

## Failure modes & handling
- **Tree builder invariant violation**: log error, mark parser failed, stop emitting patches for that document; runtime emits a reset on next full rebuild.
- **Tokenizer buffer growth**: if zero-copy spans prevent trimming, fall back to owned text for subsequent tokens or flush chunks earlier.
- **Patch protocol violation** (e.g., `Clear` mid-batch): treated as a bug; `DomStore` rejects updates and logs.

Patch transaction semantics:
- A patch batch is an atomic transaction: apply all patches or none.
- A `Clear` starts a new baseline for the document handle; it invalidates all prior keys for that handle and does not reset the key allocator.
- Patch keys are monotonic and never reused for a given document handle, including across `Clear` baselines.
- On fatal parse failure, the runtime must either (a) emit a new handle and a full create stream, or (b) emit `Clear` + full create on the existing handle. The choice is explicit and consistent.
- A “full create stream” is `CreateDocument` + a complete create/append stream for every node in document order, with a fresh key allocator for the new handle when a new handle is used.
- `CreateDocument` is part of the patch protocol in this design; if not already present, it is introduced as a first-class patch operation.

Parser suspension points (future-proofing):
- Tree builder can signal suspension (e.g., script execution) and resume later with preserved tokenizer and tree builder state.
- Suspension API: `TreeBuilderStepResult::{Continue, Suspend(SuspendReason)}` with a well-defined resume contract.
- On `Suspend`, the runtime stops advancing the parse pump; it may continue buffering input and resumes from the same token/batch boundary.
- Injection points (e.g., document.write-like input) are modeled as additional byte stream chunks with ordering guarantees.

## Follow-up plan
- Define HTML5 tokenizer state enum + insertion mode enum and scaffolding modules.
- Add spec test harness integration (WHATWG tokenizer + tree builder tests).
- Add streaming tests with chunk plans to validate resumability and patch parity.
- Introduce an internal `ParseErrorSink` for logging/metrics without panicking.
- Incrementally replace current tokenizer/tree builder in `runtime_parse` under feature gate.
