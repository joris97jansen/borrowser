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
  - Emits tokens that reference `AtomId` and zero-copy spans into the tokenizer’s source buffer where possible.

- **Tree builder**
  - Implements HTML5 insertion modes and the stack of open elements + active formatting list.
  - Consumes tokens and emits `DomPatch` operations as the primary output.
  - Maintains its own arena/ID allocator and ensures patch protocol invariants.

- **Output**
  - `DomPatch` is the primary output from the tree builder.
  - The “DOM-first then diff” path remains as a debug/test path only.

### Streaming model
- Tokenizer input is a byte stream. An explicit decoding layer determines encoding and produces text for tokenization.
- The architecture does not assume UTF-8 at the tokenizer boundary; decoding must handle BOM, HTTP headers, and `<meta charset>` sniffing per HTML5 rules.
- Tokenizer owns an append-only source buffer (or segmented buffer) to support `TextSpan` and `AttributeValue::Span` without copying.
- Tokens are drained in batches; the tokenizer preserves enough buffer history to keep spans valid until the tree builder has consumed them.
- Tree builder is fully resumable; it can pause between tokens and resume with preserved internal state.
- EOF is explicit (`finish()`/`push_eof()`), ensuring end-of-file processing runs exactly once.

### Module boundaries & ownership
Proposed module layout in `crates/html`:
- `html::html5::tokenizer` (public, new): HTML5 tokenizer state machine and streaming API.
- `html::html5::tree_builder` (public, new): HTML5 insertion modes, DOM construction, and `DomPatch` emission.
- `html::html5::types` (internal): tokenizer states, insertion modes, tag names, and error enums.
- `html::dom_patch` (public): patch protocol and invariants (already exists).
- `html::types` (internal): stable IDs, atom table, shared token types (already exists; may add HTML5-specific token variants if needed).

Ownership rules:
- Tokenizer owns:
  - Source buffer and any zero-copy spans.
  - Tokenizer state machine and parse error accumulator.
- DocumentParseContext (new) owns:
  - Atom table (`AtomTable`) for canonical ASCII-folded names (HTML namespace matching).
  - Encoding state and decoder configuration.
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
  - `push_bytes(&mut self, bytes: &[u8]) -> TokenizeResult`
  - `next_batch(&mut self) -> TokenBatch` (tokens/spans valid for the batch epoch; includes `TextResolver` view bound to the batch)
  - `finish(&mut self) -> TokenizeResult` (emit EOF)
- `pub struct DocumentParseContext { ... }`
  - `atoms(&self) -> &AtomTable`
  - `decoder(&mut self) -> &mut ByteStreamDecoder`
- `pub struct Html5TreeBuilder { ... }`
  - `new(config, ctx: &mut DocumentParseContext) -> Self`
  - `push_token(&mut self, token: &Token, atoms: &AtomTable, text: &dyn TextResolver) -> Result<(), TreeBuilderError>`
  - `take_patches(&mut self) -> Vec<DomPatch>`

Internal API:
- Tokenizer state enums and insertion-mode enums are internal and not re-exported.
- `TokenTextResolver` remains internal: it provides text extraction from spans and owned buffers.

Error strategy:
- HTML5 tokenization parse errors are **non-fatal**; they are recorded and processing continues.
- Tree builder errors are **invariant violations** (e.g., invalid patch keys, illegal transitions). These are fatal for the current stream and result in a controlled reset (see failure modes).
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
- A `Clear` starts a new baseline for the document handle; it invalidates all prior keys for that handle.
- On fatal parse failure, the runtime must either (a) emit a new handle and a full create stream, or (b) emit `Clear` + full create on the existing handle. The choice is explicit and consistent.
- A “full create stream” is `CreateDocument` + a complete create/append stream for every node in document order, with fresh patch keys allocated from scratch for that baseline.
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
