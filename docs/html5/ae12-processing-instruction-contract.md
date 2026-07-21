# AE12: HTML Processing-Instruction Contract

Last updated: 2026-07-21
Status: Milestone AE implementation contract
Scope: static full-document HTML tokenization, tree construction, parser-created
DOM transport, materialization, and non-rendering consumption

This contract defines Borrowser's supported HTML processing-instruction (PI)
profile. It is pinned as `AE12-WHATWG-processing-instruction-profile-v1` to:

- WHATWG HTML `source` commit
  `24c5e48bf66ea61bc199ec6338c81258275ba9c6`;
- WHATWG DOM commit `8a5f57c61ca1de8dc21b7e114501b1b57882e935`;
- Web Platform Tests commit `4809b72f863e05ab1df710d3390547dd86694239`,
  source `html/syntax/parsing/resources/processing-instructions.dat`.

The immutable hashes, selected cases, and adaptation notes are recorded in
`tests/wpt/provenance/ae12-supported-profile.provenance.txt`. This is HTML PI
parsing. It is not XML PI parsing and does not add general XML names,
namespaces, declarations, or execution semantics.

## Ownership And Data Flow

AE12 is one parser-owned vertical feature:

```text
HTML input
  -> typed ProcessingInstruction token
  -> tree-builder semantic insertion
  -> DomPatch::CreateProcessingInstruction plus attachment patch
  -> LiveTree applies the structural patches
  -> external patch stream
  -> strict staged validation
  -> Browser DomStore
  -> materialized Node
```

The tree builder does not mutate a PI side structure before patch emission.
Structural patches are the single mutation path: the internal `LiveTree`
applies the same creation and attachment patches that are emitted to external
consumers.

HTML owns recognition, recovery, target/data semantics, insertion, and the
parser-producible payload validator. Browser applies validated patches and
materializes the node without reimplementing HTML validity. CSS may carry the
typed non-element leaf where its styled-tree shape requires it; selector
indexing remains element-only. Layout suppresses the node centrally before box
generation. It therefore receives DOM identity but no retained render, layout,
paint, or stacking identity.

## Tokenizer Entry And Cursor Representation

Normatively, ordinary PI recognition is entered only through the HTML Data and
Tag open states: Data observes `<`, and the Tag open semantic `?` branch clears
the temporary target buffer and switches to processing instruction open. No
RCDATA, RAWTEXT, script-data, escaped or double-escaped script-data, or
foreign-content CDATA state recognizes PI syntax.

Borrowser preserves its established internal **prefix-first** cursor
representation. Data enters `TokenizerState::TagOpen` with the cursor still on
`<`. `step_tag_open` uses the same resumable prefix matcher used by `</` and
`<!` to recognize `<?`. A match consumes exactly `<?`, clears/initializes the
PI pending state, and enters `ProcessingInstructionOpen`; `NeedMoreInput`
retains TagOpen and the cursor; `NoMatch` continues through the pre-existing
tag-open paths. This internal cursor ownership is equivalent to the normative
state transitions and is not a global scanner or a state-independent
two-character recognizer. A chunk ending after `<` must resume with `?` in the
next chunk and equal whole-input parsing.

## Five Tokenizer States

AE12 implements exactly these five dedicated states:

1. **Processing instruction open state.** ASCII alpha or `_` begins the target;
   another character converts the temporary buffer to a bogus comment.
2. **Processing instruction target state.** Continuation characters are ASCII
   alphanumeric, `-`, and `_`. HTML whitespace, `?`, or `>` ends the target.
   Other characters convert the temporary buffer to a bogus comment.
3. **After processing instruction target state.** Separator HTML whitespace is
   consumed and discarded. The first non-space is reconsumed in data.
4. **Processing instruction data state.** `>` emits; `?` switches to the
   questionable state; anything else is appended to data.
5. **processing instruction questionable state.** `>` emits. For anything
   else, append the already-consumed `?` to data and reconsume the current
   character in the processing instruction data state. PI tokenization does
   not exit on this branch.

The typed token contains owned `String` target and `TextValue` data fields.
The target preserves exact ASCII case and never passes through the lowercased
HTML element-name atom path. Leading separator whitespace after the target is
not data; later whitespace is data. Both `>` and `?>` terminate, so the
question mark immediately before `>` is not data. Other question marks are
preserved exactly, including `<?pi a?b>`, `<?pi a??b>`, and
`<?pi a???b?>`.

Targets are non-empty, begin with ASCII alpha or `_`, and continue with ASCII
alphanumeric, `-`, or `_`. `xml` and `xml-stylesheet` are rejected using ASCII
case-insensitive comparison while preserving their original spelling in
fallback data. Invalid or disallowed input converts to bogus-comment data
equal to `"?" + temporary target buffer`; the leading `?` is conversion
semantics, not a target character. Valid PIs never use the bogus-comment path.

EOF in processing instruction open, target, after-target, data, or questionable
records the established tokenizer EOF diagnostic, discards pending PI state,
exits the PI state family, and emits no partial PI. Every PI state requires
valid `PendingProcessingInstruction` metadata whose target/data ranges resolve
against the bound input. Missing metadata or invalid ranges are engine
invariant failures, never document recovery, empty-data substitution, or
silent token loss. Production stall recovery clears the state and metadata
together. All state, malformed recovery, error, token, and EOF results must be
identical for whole and arbitrarily chunked input.

PLAINTEXT is a pre-existing unsupported tokenizer-state gap. AE12 does not add
that state. The negative guarantee applies to every currently supported
text-specialized state and does not claim PLAINTEXT support.

## Tree Construction

The shared **insert a processing instruction** path resolves the shared
adjusted insertion location before allocating and emitting its structural
patch. It supports an explicit override target and therefore preserves
template-content redirection and existing insertion-location behavior. PIs are
never foster-parented.

Initial comments and PIs share a node-category-neutral document-bootstrap
operation. It creates the `Document` without completing Initial-mode bootstrap,
so a following accepted doctype retains its source-order position and document
mode ownership.

The supported full-document insertion-mode profile is:

| Mode | Handling | Effective placement |
| --- | --- | --- |
| Initial | direct, document override | document, in source order before an accepted doctype/html when present |
| Before html | direct, document override | document, between preceding document children and html |
| Before head | direct | adjusted current html element |
| In head | direct | adjusted current head/template target |
| After head | direct | adjusted current html element |
| In body | direct | adjusted current node |
| In table | direct before `anything else` | adjusted current table target; no foster parenting |
| In table text | flush pending characters, restore the recorded mode, then reprocess | whatever the restored table-family PI rule selects |
| In caption | delegated to In body | adjusted current caption descendant |
| In column group | direct | adjusted current colgroup target |
| In table body | delegated to In table | adjusted current table-body target |
| In row | delegated to In table | adjusted current row target |
| In cell | delegated to In body | adjusted current cell target |
| In template | delegated to In body for this token category | template contents through adjusted-location redirection |
| After body | direct, html-element override | html element after body in source order |
| After after body | direct, document override | document after the document element in source order |
| Foreign content | direct | adjusted current SVG/MathML node, unless normal integration-point dispatch has already returned the token to HTML handling |

A synthetically supplied `ProcessingInstruction` token while the insertion
mode is `Text` is impossible for normal tokenizer output. The tree builder is
the single compatibility owner: its central preflight returns the existing
internal invariant error before parser state, LiveTree, key allocation, patch
output, or HTML parse errors change. The arbitrary-token fuzz harness delivers
the combination through the normal entry point and classifies that generic
internal error; it has no PI-specific gate.
The lower-level Text-mode arm is an unreachable assertion: invoking it directly
documents that the required central preflight was bypassed rather than creating
a second semantic owner.

## Parser-Created Node And Identity

`ProcessingInstructionNode` stores only an `Id`, owned target, and owned data.
It is a leaf with no namespace, local name, element attributes, or children in
the AE12 representation. It may be an ordered child of a `Document`, `Element`,
or parser-created template contents where tree construction places it. Document
PIs may appear before a doctype, between doctype and document element, or after
the document element; existing doctype uniqueness/order constraints remain
unchanged.

Materialized PI construction uses a checked internal factory backed by the
shared parser-created payload validator. Invalid external patches therefore
remain ordinary atomic protocol errors, while invalid data reaching a
post-validation internal materializer remains an explicit internal invariant
failure in every build configuration; validity does not depend on
`debug_assert!`.

Each PI receives `PatchKey`, LiveTree, Browser `DomStore`, and materialized
`Id` identity. Those identities do not imply a `RetainedRenderId`, layout box
identity, paint identity, or stacking identity.

The target/data-only model does **not** claim that DOM ProcessingInstruction
objects conceptually lack attributes. DOM Standard pseudo-attribute maps,
attribute access or mutation, constructors, CharacterData mutation, cloning,
public live DOM mutation, and broader DOM PI validity remain deferred.

## Patch Protocol And Strict Validation

`DomPatch::CreateProcessingInstruction { key, target, data }` is deliberately
parser-owned and accepts the narrower HTML parser-producible domain, not every
value a future DOM constructor might accept. The shared HTML validator requires:

- a non-empty target with the AE12 first/continuation grammar;
- ASCII case-insensitive rejection of `xml` and `xml-stylesheet`;
- exact target case preservation;
- data without `>`, because tokenizer-emitted PI data cannot contain it.

The ordinary strict patch machinery additionally requires a fresh non-zero
key, valid parent kind, no duplicate creation, no cycle or illegal reparenting,
valid document structure, and leaf behavior. Appending or inserting a child
under a PI is invalid. Validation is staged and atomic: any failure commits no
node, parent edge, key allocation, or partial mutation. Browser consumes the
same validator through `html::internal`; it does not duplicate the algorithm.

Token, patch, LiveTree/DOM, parser-state, and materialized snapshots render
target and data separately with deterministic escaping. A PI is never encoded
only as snapshot text.

## Resource-Hardening Policy

PI resource limits follow the existing category-specific `TokenizerLimits`
architecture:

| Field | Default | Recovery |
| --- | ---: | --- |
| `max_processing_instruction_target_bytes` | `1024` | continue scanning the complete target and malformed/disallowed checks, but suppress an otherwise valid oversized PI |
| `max_processing_instruction_data_bytes` | `64 * 1024` | continue scanning to the real terminator and emit the bounded UTF-8 prefix |

Each first overflow records a stable `ResourceLimit` diagnostic with the
target- or data-specific detail. Target truncation is never used as a semantic
target: it cannot rename a PI, turn a long name into `xml`, or hide malformed or
disallowed-target detection. Data bounding does not change terminator scanning.
Diagnostics and recovery are whole/chunk equivalent, and patch-byte accounting
includes both target and data.

Oversized-target suppression and bounded-data emission are additive Borrowser
**resource-hardening** recovery, not WHATWG PI behavior and not
standards-conformant PI output. WPT-derived conformance cases remain below the
configured limits; hardening fixtures are separate and cannot be counted as
conformance coverage.

## Conformance Provenance And Goldens

`tests/wpt/provenance/ae12-processing-instructions.cases` records each executed
WPT-derived case with its one-based upstream identity, exact `#data`, per-case
SHA-256, scripting profile, adaptation note, and exact upstream `#document`.
The provenance test recomputes every case hash and compares a representation-
only translation of that expected output with the DOM produced from those same
bytes. The HTML, DOM, and WPT source paths and source hashes are recorded in
`ae12-supported-profile.provenance.txt`.

Versioned token, DOM, and patch goldens cover document placement, InTableText
flush and non-foster attachment order, template redirection, SVG/MathML
placement, malformed/disallowed fallback, and escaped target/data formatting.
The golden harnesses require whole/chunked byte-identical output. Resource-limit
fixtures remain separate hardening evidence.

## Deliberate Exclusions

AE12 does not add XML parsing, arbitrary XML names or namespace processing,
stylesheet PI behavior, runtime PI execution, scripting, parser pausing,
events, resource loading, public constructors or mutation APIs, PI
pseudo-attributes, fragment parsing, frameset modes, PLAINTEXT, or complete
HTML/DOM conformance.
