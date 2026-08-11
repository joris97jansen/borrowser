# AE14 HTML Parser Foundation Closeout

Status: `Closeable with documented out-of-scope gaps`

This document is the evidence-based closeout for Milestone AE. It records the
implemented static HTML tokenizer, tree-construction, parser-created DOM,
recovery, diagnostic, and conformance-testing foundation. It does not claim a
complete HTML platform, public DOM, forms platform, SVG or MathML renderer,
JavaScript runtime, resource loader, navigation stack, event loop,
html5lib implementation, or WPT-conformant browser platform.

## Purpose and scope

Milestone AE makes static HTML parsing an explicit production pipeline:
preprocessing, typed tokenization, tree construction, parser-created node and
patch production, deterministic observation, and downstream materialization.
The milestone closes only the documented subsets in the AE contracts. A type,
enum, fixture, serializer, or unsupported-feature observation is not evidence
of support by itself; each classification below is based on the owning issue,
contract, production path, and executable evidence.

The audit was performed against the canonical GitHub issue history, not only
the top-level AE labels. GitHub issue identity is authoritative when a
repository document uses a historical or slice label.

## Final architecture

The supported end-to-end flow is:

1. Supported UTF-8 strings or byte/chunk input enters the HTML parser boundary.
2. Parser-owned preprocessing normalizes the supported CR/LF and character
   sequences while retaining the documented coordinate model.
3. The tokenizer consumes the preprocessed stream through explicit states and
   emits typed tokens, tokenizer parse errors, and implementation diagnostics.
4. The tree builder dispatches each token using insertion mode, the stack of
   open elements, active formatting elements, table pending-text state, form
   pointer, template mode state, and foreign-content context.
5. Parser-created nodes and `DomPatch` operations are produced in deterministic
   order. The parser retains parser semantics; it does not read fixture or
   snapshot files.
6. The production tree builder selects the supported `DocumentMode` and the
   parser finalizer flushes pending state, drains patches, and runs production
   final audits.
7. AE13 observations expose tokens, errors, diagnostics, document mode,
   transitions, unsupported identities, trees, patches, and final invariants
   through deterministic snapshots and fixtures.
8. Browser/runtime transports and materializes parser output. CSS, Layout,
   Paint, and accessibility consume the resulting tree; they do not repeat
   tokenizer or tree-construction recovery.

Preprocessing and tokenization are incremental at the parser input boundary;
the session accepts UTF-8 or byte chunks and preserves the covered whole/chunk
semantics. Tree construction is stateful across deliveries. A complete
document finalization is a terminal operation: preprocessing, tokenizer EOF,
pending token data, table text, template state, patches, and final audits are
not re-run after successful completion. The conformance harness also supports
standalone tokenizer and deterministic fixed-boundary deliveries, but the
production parser does not depend on fixture delivery formats.

## Ownership map

HTML/parser owns preprocessing used by HTML parsing, tokenizer states and token
semantics, tokenizer/tree parse errors, implementation diagnostics, insertion
modes, the stack of open elements, active formatting elements, pending table
text, the form-element pointer, template insertion-mode state, foreign-content
dispatch, namespace and document-mode selection, parser-created DOM semantics,
`DomPatch` emission, canonical observations, and unsupported-feature identity.

The parser-created DOM/shared node model owns node kinds, stable parser-created
identity, parent-child structure, attributes and ordering, namespaces,
document-fragment/template-content representation, and non-renderable node
invariants. Some concrete node types physically live in `crates/html`; they
remain conceptually part of this parser-created DOM boundary and are not
runtime-owned objects.

`html-test-support` owns fixture declaration/loading/validation, deterministic
discovery, expectations, snapshot codecs/comparison, update workflow, and
runner mechanics. Production parser code has no dependency on fixture TOML,
snapshot blessing, or runner semantics.

Browser/runtime owns parser orchestration, patch transport/materialization,
document lifetime, supported runtime form-control initialization, and future
navigation/resource coordination. It must not repair malformed HTML or make
insertion decisions. CSS, Layout, Paint, and accessibility consume the
parser-created tree and must not independently repair it.

## Support summary

### Tokenizer

| Feature | Classification | Contract and production path | Evidence | Remaining limitation |
| --- | --- | --- | --- | --- |
| Preprocessing | supported for the declared input boundary | AE3; `crates/html/src/html5/tokenizer/input.rs`, parser/session input | tokenizer tests, AE13d preprocessing fixtures, AE13c parity | No general encoding sniffing, BOM switching, or legacy decoding. |
| Data, tag, end-tag, name, attribute, comment, doctype, self-closing states | supported subset | AE4; `crates/html/src/html5/tokenizer/` | tokenizer state, token, EOF, and parse-error test modules; token snapshots and native corpus | Full WHATWG tokenizer state/edge parity is not claimed. |
| Character references | partially supported | AE5; tokenizer character-reference states | tokenizer tests; references fixtures; chunk parity | Active named set and malformed-reference behavior are intentionally narrower than the full table. |
| RCDATA, RAWTEXT, script-data | supported for static text subset | AE5; tokenizer text modes and tree text dispatch | text-mode tests, rawtext/script regression target, corpus | Script is inert text; PLAINTEXT and scripting-enabled branches remain out of scope. |
| Processing-instruction tokenizer states | supported for AE12 subset | AE12; tokenizer PI state and typed token | AE12 unit/fixture/patch/parity tests | This is HTML PI support, not XML parsing or public PI APIs. |
| Duplicate attributes | supported | AE2/AE4; first-wins parser-created representation | tokenizer/tree/DOM tests and snapshots | Full every-edge tokenizer conformance is not claimed. |
| EOF recovery and finalization | supported for covered states | AE3-AE5; tokenizer/session finalization | EOF tests, final audits, parity | Unsupported states/branches retain documented limitations. |
| Chunked input | supported for covered behavior | AE3/AE5/AE13c; session delivery APIs | `streaming_parity`, AE13c parity corpus, runtime chunk tests | Coverage is bounded to declared strategies and input boundary. |

The tokenizer therefore has a supported static subset, not “HTML5 tokenizer
complete” status. Unsupported/deferred branches include complete character
reference conformance, full byte decoding, PLAINTEXT, scripting-enabled
parser behavior, and remaining WHATWG state branches.

### Tree construction and recovery

| Area | Classification | Evidence and boundary | Remaining limitation |
| --- | --- | --- | --- |
| Initial/document shell, before-html, before-head, in-head, after-head, in-body, after-body, after-after-body | supported subset | AE6; `crates/html/src/html5/tree_builder/`; insertion-mode, document-shell, recovery, and snapshot test modules | Frameset and other advanced modes are not implemented. |
| Repeated html/body starts and frameset flag | partially supported | AE6/Core-v0; `crates/html/src/html5/tree_builder/unsupported.rs` | unsupported-feature branch tests, tree/diagnostic snapshots, AE13b4 observations | Attribute merging and repeated-body `frameset_ok` are deferred. |
| Explicit insertion modes and token reprocessing | supported for implemented modes | AE6-AE11; `crates/html/src/html5/tree_builder/process_context.rs`, transitions | Absent standard modes and branches remain partial/deferred. |
| Stack of open elements and scope | supported for implemented paths | AE2/AE6-AE11; `crates/html/src/html5/tree_builder/stack/`, invariant audit | Not a claim of all WHATWG scope predicates. |
| Paragraph/list recovery and implied ends | supported subset | AE7 contract and body recovery tests | Full implied-end-tag and adoption-agency algorithms are not claimed. |
| Active formatting/reconstruction | partially supported | AE7; AFE state and formatting tests | Adoption-agency behavior is only the documented subset. |
| Tables and foster parenting | supported subset | AE8; table modules, table/patch snapshots, parity | Three explicit AE8 table-close/cell/caption omissions remain; no table layout or CSS table formatting. |
| Forms | supported parser tree subset | AE9a; form pointer and control tests | Submission, validation, focus, events, accessibility, and complete form-owner algorithms remain deferred. |
| Select | supported current full-document subset | AE9b; select/option/optgroup tests and snapshots | No historical standalone select mode or control selectedness/UI semantics. |
| Templates | supported static parsing subset | AE10; typed contents, mode stack, nested/EOF tests | Public APIs, cloning, scripting, custom elements, shadow DOM, and live mutation are deferred. |
| SVG/MathML foreign content | supported namespace-aware subset | AE11; foreign dispatch/adjustment/integration tests | No SVG rendering, MathML layout, XML parsing, or complete namespace APIs. |
| Processing instructions | supported AE12 subset | tokenizer → tree → node → patch → validation → `DomStore` → materialization | No XML PI semantics, runtime execution, selector, layout, or paint meaning. |
| Finalization | supported for covered parser state | session finalizer, patch/materialization audits, invariant tests | Resource-failure coverage is bounded to the AE13b2.2 contract. |

#### Body recovery

AE7 supports paragraph auto-closing before its documented block-start set,
unmatched paragraph end-tag recovery, sibling list-item recovery, supported
implied-end generation, and reconstruction of the supported active-formatting
set. Malformed body markup outside those paths is deterministic where the
implemented tree builder has a rule, but is not represented as complete
adoption-agency or full body-mode conformance.

#### Tables and foster parenting

AE8 owns `InTable`, `InTableText`, `InCaption`, `InColumnGroup`, `InTableBody`,
`InRow`, and `InCell` for the documented subset. It constructs implied table
wrappers, buffers and flushes table character tokens, selects foster-parent
locations including insertion-before, clears the supported stack regions, and
reprocesses tokens through the supported modes. Table layout, sizing, border
collapsing, CSS table formatting, painting, and accessibility table semantics
are outside AE.

The AE8 contract explicitly excludes the same-named cell end-tag guard,
implied-end/current-node preparation before cell closure, and caption-close
preparation. That declared partial scope is preserved.

#### Forms and select

AE9a keeps parser-owned form pointer identity and stack-only removal semantics,
including covered `form`, `input`, `textarea`, `button`, `fieldset`, and `keygen`
paths and the parser-owned initial textarea LF rule. Runtime control
initialization does not repeat parser source normalization. AE9a does not own
submission, validation, focus, selection, events, or the accessibility tree.

AE9b uses current in-body and table delegation for full-document `select`,
`option`, and `optgroup` parsing. Supported option/optgroup closure, select
scope, and table/select recovery are parser semantics. Runtime selectedness,
control values, interaction, and UI behavior remain outside the parser issue.

#### Templates and foreign content

AE10 constructs one typed contents root per supported template host, tracks
template insertion modes with the open template context, applies marker and
reprocessing behavior, supports nested templates and EOF recovery, and keeps
contents inert to current rendering/runtime paths. Public `HTMLTemplateElement`
APIs, cloning, scripting, custom elements, declarative shadow DOM, and live DOM
mutation are deferred.

AE11 preserves HTML, SVG, and MathML namespaces through live state, adjusted
names/attributes, patches, validation, `DomStore`, materialization, and
snapshots. It covers foreign dispatch, SVG/MathML integration points, breakout,
self-closing foreign elements, and unknown-foreign fallback. Namespace-correct
parsing does not imply SVG rendering, MathML layout, animation, filters,
geometry, resource handling, XML parsing, or complete namespace-aware DOM APIs.

### Parser-created DOM and document mode

The parser-created model supports document, doctype, element, text, comment,
processing-instruction, and template-content/document-fragment representations
for the declared subset. Node identity is stable within the parser-created
identity domain; `PatchKey`, materialized HTML IDs, and retained render IDs stay
distinct. Parent-child relationships are checked for acyclicity and
consistency. Attributes are typed during parsing, first-wins for duplicate
semantic names, preserve encounter order, and expose string values; namespaces
are retained on elements and attributes. PIs are leaf nodes with exact target
and data. Template contents are represented as a non-ordinary fragment boundary
and are not flattened into the host's ordinary children.

`DomPatch` creation, ordering, identity, template contents, PI leaves, and
non-renderable semantics are defined by `docs/html5/dompatch-contract.md` and
validated before browser materialization. Public node traversal/mutation APIs,
ranges/selections, mutation observers, event dispatch, custom elements, and
full `DocumentType`/template/PI APIs are not implemented.

Doctype recognition selects `no-quirks`, `limited-quirks` where the supported
classification recognizes it, or `quirks` for the supported malformed/legacy
cases. The selected mode is parser metadata, appears in canonical observations
and snapshots, and is not a claim of complete standard doctype classification.

## Diagnostics and unsupported features

Diagnostics distinguish preprocessing errors where applicable, tokenizer parse
errors, tree-construction parse errors, typed implementation diagnostics,
unsupported-feature observations, transition traces, and final invariant
failures. Parse errors are observable and deterministic but do not necessarily
abort parsing. Positions use the documented normalized coordinate space; some
production tree diagnostics intentionally report unavailable positions. There
is no fabricated global timeline between the independently owned parse-error
and implementation-diagnostic sequences.

The six current `TreeConstructionUnsupportedFeature` identities were audited
against their production detection paths and owning contracts:

| Identity | Detection path | Owning scope | Tree/patch consequence | Final classification |
| --- | --- | --- | --- | --- |
| `MergeAttributesIntoExistingHtmlElement` | `crates/html/src/html5/tree_builder/unsupported.rs` | AE6/Core-v0 repeated html start handling | Existing element is retained without the unimplemented merge; observation is deterministic. | `partially supported`; legitimate core supported-subset limitation; future parser conformance. |
| `MergeAttributesIntoExistingBodyElement` | `crates/html/src/html5/tree_builder/unsupported.rs` | AE6/Core-v0 repeated body start handling | Existing body is retained without the unimplemented merge. | `partially supported`; legitimate core supported-subset limitation; future parser conformance. |
| `MarkFramesetNotOkForRepeatedBodyStartTag` | `crates/html/src/html5/tree_builder/unsupported.rs` | AE6/Core-v0 frameset flag branch | The unsupported frameset state transition is not applied. | `intentionally deferred`; core frameset/parser conformance behavior. |
| `RequireSameNamedTableCellInScopeForEndTag` | `crates/html/src/html5/tree_builder/table/in_cell.rs` | AE8 explicitly excludes it | AE8 uses its documented deterministic substitute and records the omission. | `partially supported`; legitimate AE8 table scope; not a closeout blocker. |
| `GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingTableCell` | `crates/html/src/html5/tree_builder/table/close.rs` | AE8 explicitly excludes it | Cell-close preparation is not silently synthesized; output and observation remain deterministic. | `partially supported`; legitimate AE8 table scope; future parser conformance. |
| `GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingCaption` | `crates/html/src/html5/tree_builder/table/close.rs` | AE8 explicitly excludes it | Caption-close preparation is not silently synthesized. | `partially supported`; legitimate AE8 table scope; future parser conformance. |

No later AE issue promoted these six algorithms into supported scope. AE13b4
observes them; it does not implement them. Their omissions are therefore
documented limitations rather than hidden unfinished AE requirements. The
unsupported-feature tests in `crates/html/src/conformance/execution.rs` and
table tests
also prove that the identities are emitted only on their production branches.

## AE12 processing-instruction audit

The AE12 path is complete for its declared HTML subset:

`Data`/`TagOpen` PI states → typed PI token → ordinary/table/template/foreign
tree dispatch → parser-created PI leaf → `DomPatch::CreateProcessingInstruction`
→ strict patch validation → Browser `DomStore` → materialization → token/DOM/
patch snapshots → whole/chunk parity and final invariants.

The target and data remain exact, identity is preserved, and the node cannot
have children. PI nodes are deliberately excluded from selector indexing,
layout boxes, paint artifacts, stylesheet processing, script execution, and
runtime PI behavior. This is not XML parsing, XML namespace processing, public
DOM PI API support, or stylesheet PI support.

Relevant evidence includes the AE12 tokenizer/tree/patch/materialization tests,
`crates/html/src/conformance/projection.rs` PI projections and invariants,
`crates/runtime_parse` PI patch tests, Browser `DomStore` PI tests, deterministic
fixture snapshots, and the whole/chunk parity suites.

## AE13 regression and conformance surfaces

AE13 owns fixture declarations, canonical observation capture, snapshot codecs,
native corpus, parity/final audits, external adapters, and CI workflow. The
production `html` crate remains the source of parser observations; the
`html-test-support` crate only loads, validates, serializes, compares, and runs
fixtures.

Normative regression surfaces are typed token snapshots, parse-error and
implementation-diagnostic snapshots, document-mode snapshots, canonical tree
snapshots, canonical patch snapshots, transition traces, namespace and
template-content output, PI output, unsupported-feature output, final-invariant
reports, and covered whole/chunk parity. The fixture v2/v3 formats are
versioned test contracts, not production parser inputs.

The normal CI lane runs the native corpus and the pinned external subset. The
existing correctness-owned extended lane is
`make test-html5-external-fixtures-extended`, backed by
[`.github/workflows/html5-conformance.yml`](../../.github/workflows/html5-conformance.yml)
and scheduled/manual separately from ordinary CI. It was run for this
closeout; it passed with no fixture drift, one external fixture passing, one
native parser conformance fixture passing, and one diff/parity test passing.
The two pinned external unsupported scripting records remain expected
out-of-scope records, not failures.

## Conformance evidence matrix

The following matrix records support claims without treating raw test counts as
the proof. Source paths and contract links identify the production owner; tests
and fixtures identify the observed evidence.

| Area | Classification | Contract/issue | Production source | Unit/invariant evidence | Fixture/snapshot/parity/downstream evidence | Exact limitation |
| --- | --- | --- | --- | --- | --- | --- |
| Input preprocessing | supported subset | AE3 | tokenizer input/session | tokenizer and session finalization tests | AE13d preprocessing; AE13c parity | UTF-8/string boundary; no full encoding sniffing. |
| Tokenizer states | supported subset | AE4 | `crates/html/src/html5/tokenizer/` | tokenizer module tests | token v2 snapshots; native corpus; parity | Full WHATWG state coverage absent. |
| Character references | partially supported | AE5 | tokenizer reference states | reference tests | reference fixtures/parity | Minimal active named set and edge gaps. |
| Text modes | supported subset | AE5 | tokenizer text-mode state | rawtext/script/text tests | rawtext regression and corpus | Inert script; PLAINTEXT/scripting omitted. |
| Doctypes/document mode | supported subset | AE2/AE6/AE13b2 | `crates/html/src/document_mode.rs`, `crates/html/src/html5/tree_builder/document.rs` | document-mode tests | mode snapshots and fixtures | Not full doctype classification. |
| Core document construction | supported subset | AE6 | tree-builder dispatch | tree-builder tests | tree snapshots/parity | Advanced modes omitted. |
| Stack of open elements | supported subset | AE2/AE6 | tree-builder stack | final audits/invariant tests | tree/patch snapshots | Only implemented scopes/modes. |
| Token reprocessing | supported subset | AE6-AE11 | process context | transition/finalization audits | transition snapshots/parity | Bounded to implemented modes. |
| Body recovery | supported subset | AE7 | in-body handlers | body recovery tests | corpus/tree/error snapshots | No full body/AAA conformance. |
| Active formatting | partial | AE7 | AFE/reconstruction handlers | formatting/AFE invariants | formatting snapshots/parity | Conservative AAA subset. |
| Adoption-agency subset | partial | AE7 | formatting close paths | targeted formatting tests | transition/tree evidence | Full algorithm deferred. |
| Tables | supported subset | AE8 | `crates/html/src/html5/tree_builder/table/` | table mode/close tests | table fixtures/tree/patch/parity | Three explicit AE8 table-close/cell/caption branches omitted; no table layout/paint. |
| Foster parenting | supported subset | AE8 | insertion-location helpers | table/foster tests | patch/tree snapshots/parity | No table layout/paint. |
| Forms | supported parser subset | AE9a | form pointer/in-body/table handlers | form pointer/control tests | form snapshots/parity/runtime smoke | No forms platform. |
| Select | supported current subset | AE9b | select dispatch | select/option/optgroup tests | select snapshots/parity | No control interaction semantics. |
| Templates | supported static subset | AE10 | template modes/contents | nested/EOF/invariant tests | template tree/patch/parity/runtime | No public/live template APIs. |
| SVG | supported parse subset | AE11 | foreign dispatch/adjustment | foreign tests | mixed namespace snapshots/parity | No SVG renderer. |
| MathML | supported parse subset | AE11 | MathML dispatch/integration | foreign tests | mixed namespace snapshots/parity | No MathML layout. |
| Namespaces | supported through materialization | AE11 | node/patch/materialization | namespace invariants | namespace snapshots; Browser tests | No XML namespace APIs. |
| Processing instructions | supported AE12 subset | AE12 | tokenizer/tree/patch path | PI tokenizer/tree/patch/validation tests | PI snapshots/parity; DomStore tests | No XML/runtime/public PI semantics. |
| Parser-created DOM | supported declared node model | AE2 | `crates/html/src/types.rs`, `crates/html/src/html5/tree_builder/live_tree.rs`, `crates/html/src/conformance/projection.rs` | DOM/identity/invariant tests | canonical tree snapshots/parity | No public DOM APIs/mutation. |
| Template contents | supported representation | AE10 | typed fragment/patch path | contents-root invariants | template snapshots/materialization | No cloning/adoption/live mutation. |
| `DomPatch` | supported parser emission | AE1/AE2/AE13b3 | `crates/html/src/dom_patch.rs`, parser session patch history | patch validation/golden tests | patch v3 snapshots/parity; runtime tests | No general DOM mutation protocol. |
| Finalization | supported covered state | AE3-AE13c | session finalizer/conformance execution | final audit/invariant tests | final-invariant snapshots/parity | Resource-failure coverage bounded. |
| Parse errors | supported deterministic subset | AE3/AE13b1/2 | observation model/tree/tokenizer | diagnostic tests | error snapshots/parity | Some positions unavailable; not all spec errors. |
| Implementation diagnostics | supported observation surface | AE13b1/2 | observation fanout | diagnostic/error tests | diagnostic snapshots/parity | Diagnostic-only, not normative rendering behavior. |
| Unsupported observations | supported exact surface | AE13b4 | unsupported branches | identity/branch tests | unsupported snapshots/parity | Records omissions; does not implement them. |
| Fixture/snapshot harness | supported test-support boundary | AE13a/b/c/d/e | `crates/html_test_support/src/parser_fixture/`, `crates/html_test_support/src/parser_snapshot/` | loader/codec/runner tests | native and pinned external corpus | Not full html5lib/WPT. |
| Whole/chunk parity | supported covered schedules | AE13c | `crates/html/src/conformance/`, `crates/html/src/html5/session/` | `crates/html/src/streaming_parity.rs`, invariant tests | parity fixtures; runtime chunk tests | Bounded declared strategies. |
| Final invariants | supported executable audit | AE13c | tree/session/conformance audits | invariant/failure-injection tests | invariant snapshots/parity | No universal OOM audit. |
| Runtime materialization | supported consumption boundary | AE1/AE2/AE11/AE12 | `crates/runtime_parse/src/patching.rs`, `crates/browser/src/dom_store/` | runtime/browser tests | chunk/template/PI/namespace smoke | Runtime does not own recovery. |
| Render-consumer boundary | supported ownership boundary | AE1/AE2/AE10/11/12 | browser/style/layout inputs | non-renderable/template/foreign tests | downstream smoke/snapshots | Rendering support remains partial. |

## Final invariant audit

The production and test surfaces establish the following invariants:

| Invariant | Existing proof surface |
| --- | --- |
| Preprocessing is finalized before tokenizer completion | session finalization audit and AE13c final-invariant report. |
| Tokenizer completion occurs exactly once | tokenizer/session EOF lifecycle invariant and EOF tests. |
| Pending token data is flushed | tokenizer finalization and text-mode/EOF tests. |
| Token reprocessing is bounded | transition observations and final-audit state checks. |
| SOE entries reference valid parser-created nodes | `crates/html/src/html5/tree_builder/stack/` checks and `crates/html/src/html5/tree_builder/invariants/`. |
| AFE markers/state remain valid on supported paths | formatting/table/template invariant tests. |
| Pending table text is exhausted | AE8 table finalization audit and table tests. |
| Form pointer cannot dangle | AE9a form-pointer invariant and form tests. |
| Template mode stack matches open template context | AE10 mode-stack audits, nested-template and EOF tests. |
| Every supported template has one valid contents root | template projection/materialization preflight and invariant tests. |
| Namespaces survive state, patch, validation, materialization | AE11 namespace snapshots, `crates/html/src/patch_validation/`, and `crates/browser/src/dom_store/tests/`. |
| PI target/data and leaf-node invariants hold | AE12 tokenizer/tree/projection/patch tests and `crates/browser/src/dom_store/tests/processing_instruction.rs`. |
| Parent/child relationships are acyclic and consistent | DOM tree, patch validation, projection capacity/failure tests. |
| Snapshot IDs are deterministic | AE13b5 canonical label codecs and snapshot tests. |
| Whole/chunk execution is semantically equivalent | AE13c parity execution and runtime chunk tests. |
| Browser applies parser output without reinterpretation | `crates/runtime_parse/src/tests/` and `crates/browser/src/dom_store/tests/`. |

These are production-owned audits or validations of production output; AE14
does not introduce a duplicate parser model in test code.

## Runtime and rendering boundary

Browser/runtime receives parser-created patches and materializes them while
preserving parser-created order, identity, namespaces, template-content
boundaries, and non-renderable node kinds. It does not re-tokenize input,
re-run insertion modes, repair malformed structure, or move form parsing into
runtime. Template contents remain represented and inert on current rendering
and runtime paths. Unsupported SVG/MathML rendering suppresses unsupported
foreign boxes without flattening parser namespaces. PI nodes remain available
to parser/DOM observations but do not create selector entries, layout boxes,
or paint artifacts. Diagnostics remain observable through parser/conformance
surfaces and are not rendering dependencies.

The relevant smoke evidence is in runtime patch/materialization tests, Browser
`DomStore` lifecycle/PI/namespace/template tests, and downstream non-renderable
node/layout coverage. Rendering output is a boundary check only; it is not the
normative parser conformance surface.

## Canonical issue and slice closeability audit

The following inventory was derived from the actual Milestone AE GitHub issue
history. Child/slice issues are audited before their parent. The status below
means the issue's own acceptance criteria are satisfied within its declared
scope; it does not mean the whole HTML platform is complete.

| Issue | Actual identity and relationship | Contract/implementation/evidence | Remaining work | Closeable? |
| --- | --- | --- | --- | --- |
| [#1068 — AE1 — Define HTML parser and parser-created DOM ownership contracts](https://github.com/joris97jansen/borrowser/issues/1068) | Top-level Milestone AE issue; no child slice | AE1 contract; parser/session, patch, runtime boundaries; ownership tests/docs | Future DOM/runtime APIs | Yes |
| [#1069 — AE2 — Define parser-created DOM node model and document invariants](https://github.com/joris97jansen/borrowser/issues/1069) | Top-level Milestone AE issue; no child slice | AE2 contract; node/attribute/identity/tree/patch tests and snapshots | Public DOM APIs/mutation | Yes |
| [#1070 — AE3 — Add tokenizer input preprocessing, token model, and parse-error reporting](https://github.com/joris97jansen/borrowser/issues/1070) | Top-level Milestone AE issue; no child slice | AE3 contract; tokenizer/session implementation, EOF/errors, parity | Encoding boundary beyond UTF-8 scope | Yes |
| [#1071 — AE4 — Implement core tokenizer states for tags, attributes, comments, doctype, and text](https://github.com/joris97jansen/borrowser/issues/1071) | Top-level Milestone AE issue; no child slice | AE4 contract; tokenizer tests/snapshots | Full tokenizer conformance | Yes |
| [#1072 — AE5 — Add character references and special text parsing modes](https://github.com/joris97jansen/borrowser/issues/1072) | Top-level Milestone AE issue; no child slice | AE5 contract; reference/rawtext/RCDATA/script tests | Full references, PLAINTEXT, scripting | Yes |
| [#1073 — AE6 — Implement core tree construction for document, html, head, body, and normal content](https://github.com/joris97jansen/borrowser/issues/1073) | Top-level Milestone AE issue; no child slice | AE6 contract; insertion modes, SOE, shell/tree tests | Advanced modes/branches | Yes |
| [#1074 — AE7 — Add body-mode recovery, implied end tags, and formatting-element handling foundation](https://github.com/joris97jansen/borrowser/issues/1074) | Top-level Milestone AE issue; no child slice | AE7 contract; recovery, AFE, diagnostic and snapshot evidence | Full AAA/body conformance | Yes |
| [#1075 — AE8 — Complete specialized table tree construction and foster-parenting recovery](https://github.com/joris97jansen/borrowser/issues/1075) | Top-level Milestone AE issue; no child slice | AE8 contract; table modes, foster, pending-text, patch/parity tests | Three explicit table-close/cell/caption omissions remain partial scope | Yes |
| [#1076 — AE9a — Add parser-owned form state and form-associated tree-construction rules](https://github.com/joris97jansen/borrowser/issues/1076) | Top-level Milestone AE issue; no child slice | AE9 form contract (internal title legitimately “AE9a”); form pointer/control tests | Forms platform/runtime semantics | Yes |
| [#1311 — AE9b — Add current select, option, and optgroup tree-construction rules](https://github.com/joris97jansen/borrowser/issues/1311) | Top-level Milestone AE issue; no child slice | AE9b contract; select/table/option tests and snapshots | UI selectedness/interaction | Yes |
| [#1314 — AE9c — Normalize select roadmap identity and reserve AE10 for templates](https://github.com/joris97jansen/borrowser/issues/1314) | Independently scoped normalization issue in the AE9 select lineage | Actual AE9c acceptance; byte-identical fixture/terminology evidence | No parser behavior; preserve AE9b owner | Yes |
| [#1077 — AE10 — Introduce template tree-construction state and parser-created template contents](https://github.com/joris97jansen/borrowser/issues/1077) | Top-level Milestone AE issue; no child slice | AE10 contract; typed contents/mode/nested/EOF/parity tests | Public template APIs, cloning, scripting/shadow DOM | Yes |
| [#1307 — AE11 — Add namespace-aware SVG and MathML tree-construction boundaries](https://github.com/joris97jansen/borrowser/issues/1307) | Top-level Milestone AE issue; no child slice | AE11 contract; namespace/adjustment/patch/materialization tests | Rendering/XML/namespace APIs | Yes |
| [#1318 — AE12 — Add HTML processing-instruction tokenization and parser-created nodes](https://github.com/joris97jansen/borrowser/issues/1318) | Top-level Milestone AE issue; no child slice | AE12 contract; PI token/tree/patch/validation/DomStore/snapshot/parity tests | XML/public/runtime PI semantics | Yes |
| [#1320 — AE13a — Establish the canonical parser fixture and observation foundation](https://github.com/joris97jansen/borrowser/issues/1320) | Sibling independently scoped issue under AE13 | AE13a contract; production observation and fixture loader tests | Broader corpus/conformance | Yes |
| [#1321 — AE13b — Integrate parser observations and deterministic snapshot serializers](https://github.com/joris97jansen/borrowser/issues/1321) | Sibling independently scoped parent under AE13; parent of AE13b1, AE13b2, AE13b2.2, AE13b3, AE13b4, and AE13b5 | AE13b acceptance criteria are mapped to the cumulative production observation and serializer evidence in the appendix below | Broader observation/corpus scope remains outside AE13 | Yes |
| [#1326 — AE13b1 — Establish parser-owned observation plumbing and typed preprocessing/tokenizer diagnostics](https://github.com/joris97jansen/borrowser/issues/1326) | Child of AE13b (#1321) | AE13 harness contract; observation model, diagnostic tests, snapshots/parity | Broader diagnostic coverage | Yes |
| [#1327 — AE13b2 — Migrate tree-construction diagnostics and capture actual document mode](https://github.com/joris97jansen/borrowser/issues/1327) | Child of AE13b (#1321) | AE13 harness contract; tree observations, mode/error snapshots | Full doctype/error conformance | Yes |
| [#1333 — AE13b2.2 — Introduce typed parser resource-exhaustion failures](https://github.com/joris97jansen/borrowser/issues/1333) | Child/slice of AE13b2 under AE13b (#1321) | AE13b2.2 contract; typed failure propagation and injection tests | Complete allocation/materialization/OOM policy | Yes |
| [#1328 — AE13b3 — Capture canonical parser-created trees and complete DomPatch streams](https://github.com/joris97jansen/borrowser/issues/1328) | Child of AE13b (#1321) | AE13 harness/dompatch contracts; projection, patch golden, materialization tests | Public mutation protocol | Yes |
| [#1329 — AE13b4 — Trace actual tree-construction dispatch and unsupported parser branches](https://github.com/joris97jansen/borrowser/issues/1329) | Child of AE13b (#1321) | AE13 harness contract; production unsupported branches, transitions, identity tests | Implementing omitted algorithms belongs elsewhere | Yes |
| [#1330 — AE13b5 — Add deterministic parser snapshot serializers and fixture diagnostics](https://github.com/joris97jansen/borrowser/issues/1330) | Child of AE13b (#1321) | AE13b5 contract; strict codecs, snapshots, fixture runner tests | Larger corpus/blessing policy | Yes |
| [#1322 — AE13c — Add whole/chunked parity and final parser invariant execution](https://github.com/joris97jansen/borrowser/issues/1322) | Sibling independently scoped issue under AE13 | AE13c contract; parity and final-audit execution/tests | Wider input/conformance schedules | Yes |
| [#1323 — AE13d — Build the curated native parser conformance corpus](https://github.com/joris97jansen/borrowser/issues/1323) | Sibling independently scoped issue under AE13 | AE13d coverage contract; native fixtures/snapshots/parity | Larger corpus | Yes |
| [#1324 — AE13e — Add external adapters, snapshot workflow, CI integration, and closeout](https://github.com/joris97jansen/borrowser/issues/1324) | Sibling independently scoped issue under AE13 | AE13e contract; pinned external adapter, workflow, normal/extended lanes | Full html5lib/WPT and larger imports | Yes |
| [#1308 — AE13 — Add parser conformance fixtures, deterministic debug snapshots, and regression harnesses](https://github.com/joris97jansen/borrowser/issues/1308) | Milestone AE parent; assessed after AE13a, AE13b, AE13c, AE13d, and AE13e | Parent is assessed only after all listed AE13 slices; cumulative harness and docs pass | Parent does not expand child scope | Yes |
| [#1309 — AE14 — Close out HTML tokenizer, tree construction, and parser-created DOM foundation](https://github.com/joris97jansen/borrowser/issues/1309) | Current Milestone AE closeout issue | This document, tracker update, audits, and final validation | Future work listed below | Verdict below |

### Per-issue evidence appendix

The concise table above is backed by the following traceable issue/slice
records. Each record names the GitHub identity/title, parent relationship,
contract, production path, executable evidence, fixture/snapshot or downstream
evidence where applicable, remaining work, and closeability decision. Paths are
repository-relative. Parent issues are assessed only after their independently
scoped children below have been audited.

#### [#1068 — AE1 — Define HTML parser and parser-created DOM ownership contracts](https://github.com/joris97jansen/borrowser/issues/1068)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae1-html-parser-dom-ownership-contract.md`;
  `crates/html/src/parser/`, `crates/html/src/html5/session/`,
  `crates/html/src/dom_patch.rs`, `crates/runtime_parse/src/patching.rs`, and
  `crates/browser/src/dom_store/`.
- Evidence: `crates/html/src/html5/session/tests/`,
  `crates/html/src/patch_validation/tests.rs`,
  `crates/runtime_parse/src/tests/runtime.rs`, and Browser
  `crates/browser/src/tab/tests/dom_patches.rs` exercise the parser-to-runtime
  boundary; ownership is also recorded in `docs/html5/dompatch-contract.md`.
- Remaining work: public DOM/runtime APIs and lifecycle behavior. Decision:
  closeable within AE scope.

#### [#1069 — AE2 — Define parser-created DOM node model and document invariants](https://github.com/joris97jansen/borrowser/issues/1069)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae2-parser-created-dom-node-model.md`,
  `docs/html5/node-identity-contract.md`, `crates/html/src/types.rs`,
  `crates/html/src/attributes.rs`, `crates/html/src/dom_patch.rs`, and
  `crates/html/src/patch_validation/`.
- Evidence: `crates/html/src/html5/tree_builder/tests/dom_model.rs`,
  `crates/html/src/html5/tree_builder/tests/attributes.rs`,
  `crates/html/src/html5/tree_builder/tests/insertion_semantics.rs`,
  `crates/html/src/dom_snapshot/tests.rs`, and
  `crates/html/src/patch_validation/tests.rs` cover node kinds, identity,
  duplicate attributes, structure, and patch invariants.
- Remaining work: public traversal and mutation APIs. Decision: closeable
  within AE scope.

#### [#1070 — AE3 — Add tokenizer input preprocessing, token model, and parse-error reporting](https://github.com/joris97jansen/borrowser/issues/1070)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/spec-matrix-tokenizer.md`,
  `docs/html5/html5-core-v0.md`, `crates/html/src/html5/tokenizer/`, and
  `crates/html/src/html5/session/`.
- Evidence: tokenizer `crates/html/src/html5/tokenizer/tests/input_preprocessing.rs`,
  `crates/html/src/html5/tokenizer/tests/parse_errors.rs`,
  `crates/html/src/html5/tokenizer/tests/chunking.rs`,
  `crates/html/src/html5/tokenizer/tests/eof_recovery.rs`,
  `crates/html/src/html5/tokenizer/invariants.rs`, and conformance fixtures
  `crates/html/tests/fixtures/html5/conformance/preprocessing-cr-normalization/`
  and `crates/html/tests/fixtures/html5/conformance/tokenizer-eof-recovery/`.
- Remaining work: byte encoding sniffing and legacy decoding. Decision:
  closeable within the declared input boundary.

#### [#1071 — AE4 — Implement core tokenizer states for tags, attributes, comments, doctype, and text](https://github.com/joris97jansen/borrowser/issues/1071)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/spec-matrix-tokenizer.md` and
  `crates/html/src/html5/tokenizer/` state modules.
- Evidence: `crates/html/src/html5/tokenizer/tests/tags_attrs.rs`,
  `crates/html/src/html5/tokenizer/tests/comments.rs`,
  `crates/html/src/html5/tokenizer/tests/doctype.rs`,
  `crates/html/src/html5/tokenizer/tests/api.rs`,
  `crates/html/src/html5/tokenizer/tests/eof_recovery.rs`, and the token
  snapshots in
  `crates/html_test_support/src/parser_snapshot/token_v2.rs`.
- Remaining work: unsupported WHATWG tokenizer branches. Decision: closeable
  as the documented core subset.

#### [#1072 — AE5 — Add character references and special text parsing modes](https://github.com/joris97jansen/borrowser/issues/1072)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/html5-core-v0.md` and tokenizer entity,
  RCDATA, RAWTEXT, and script-data state modules.
- Evidence: `crates/html/src/html5/tokenizer/tests/entities.rs`,
  `crates/html/src/html5/tokenizer/tests/rcdata.rs`,
  `crates/html/src/html5/tokenizer/tests/rawtext.rs`,
  `crates/html/src/html5/tokenizer/tests/script_data.rs`,
  `crates/html/src/html5/tokenizer/tests/script_tag_boundary.rs`, the
  `crates/html/tests/fixtures/html5/conformance/text-mode-boundaries/` fixture,
  and the rawtext/script
  regression targets in `Makefile`.
- Remaining work: full named-reference parity, PLAINTEXT, and scripting.
  Decision: closeable as the documented static-text subset.

#### [#1073 — AE6 — Implement core tree construction for document, html, head, body, and normal content](https://github.com/joris97jansen/borrowser/issues/1073)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/html5-core-v0.md`,
  `crates/html/src/html5/tree_builder/dispatch/`,
  `crates/html/src/html5/tree_builder/process_context.rs`, and
  `crates/html/src/html5/tree_builder/stack/`.
- Evidence: `crates/html/src/html5/tree_builder/tests/insertion_modes.rs`,
  `crates/html/src/html5/tree_builder/tests/recovery.rs`,
  `crates/html/src/html5/tree_builder/tests/state_snapshot.rs`,
  `crates/html/src/html5/tree_builder/tests/dom_model.rs`,
  `crates/html/src/html5/tree_builder/tests/invariants.rs`, and fixtures
  `crates/html/tests/fixtures/html5/conformance/comment-and-shell/`,
  `crates/html/tests/fixtures/html5/tree_builder/missing-doctype-implicit-html/`,
  and `crates/html/tests/fixtures/html5/conformance/document-recovery-diagnostics/`.
- Remaining work: advanced modes, repeated html/body attribute merging, and
  frameset-specific state. Decision: closeable as the declared core subset.

#### [#1074 — AE7 — Add body-mode recovery, implied end tags, and formatting-element handling foundation](https://github.com/joris97jansen/borrowser/issues/1074)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae7-body-mode-recovery-contract.md`,
  `crates/html/src/html5/tree_builder/dispatch/in_body.rs`,
  `crates/html/src/html5/tree_builder/formatting.rs`, and active-formatting state.
- Evidence: `crates/html/src/html5/tree_builder/tests/recovery.rs`,
  `crates/html/src/html5/tree_builder/tests/formatting.rs`,
  `crates/html/src/html5/tree_builder/tests/aaa.rs`,
  `crates/html/src/html5/tree_builder/tests/aaa_integration.rs`,
  `crates/html/src/html5/session/tests/aaa.rs`, the
  `crates/html/tests/fixtures/html5/conformance/body-text-recovery/` fixture,
  and transition/tree
  snapshots.
- Remaining work: complete adoption-agency and full body-mode conformance.
  Decision: closeable as the documented recovery subset.

#### [#1075 — AE8 — Complete specialized table tree construction and foster-parenting recovery](https://github.com/joris97jansen/borrowser/issues/1075)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae8-specialized-table-tree-construction-contract.md`,
  `crates/html/src/html5/tree_builder/table/`,
  `crates/html/src/html5/tree_builder/insert/tests/foster.rs`, and
  `crates/html/src/html5/tree_builder/stack/tests/table.rs`,
  `crates/html/src/html5/tree_builder/stack/tests/foster.rs`.
- Evidence: `crates/html/src/html5/tree_builder/tests/table_modes.rs`,
  `crates/html/src/html5/tree_builder/tests/table_state.rs`,
  `crates/html/src/html5/tree_builder/tests/table_cell.rs`,
  `crates/html/src/html5/tree_builder/tests/table_caption_colgroup.rs`,
  `crates/html/src/html5/tree_builder/tests/table_body_row.rs`,
  `crates/html/src/html5/tree_builder/table/in_table_text.rs`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-table-cell-end-guard/`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-table-cell-preparation/`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-caption-preparation/`,
  and `crates/html/tests/fixtures/html5/conformance/foster-parented-element/`.
- Remaining work: the three explicitly excluded AE8 cell/caption-close
  algorithms; no table layout or paint. Decision: closeable as the declared
  table subset.

#### [#1076 — AE9a — Add parser-owned form state and form-associated tree-construction rules](https://github.com/joris97jansen/borrowser/issues/1076)

- Parent: top-level Milestone AE issue; no child slice. The contract's internal
  title correctly retains AE9a.
- Contract and production: `docs/html5/ae9-form-tree-construction-contract.md`,
  `crates/html/src/html5/tree_builder/dispatch/form_controls.rs`, form pointer
  state, and session form handling.
- Evidence: `crates/html/src/html5/tree_builder/tests/form_controls.rs`,
  `crates/html/src/html5/session/tests/text_mode.rs`,
  `crates/html/tests/fixtures/html5/conformance/form-pointer-state/`, runtime
  textarea tests, and form-related
  patch/tree snapshots.
- Remaining work: submission, validation, focus, events, accessibility, and
  complete form-owner reassociation. Decision: closeable within AE scope.

#### [#1311 — AE9b — Add current select, option, and optgroup tree-construction rules](https://github.com/joris97jansen/borrowser/issues/1311)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae9b-current-select-tree-construction-contract.md`,
  `crates/html/src/html5/tree_builder/dispatch/select.rs`, and select/table
  delegation.
- Evidence: `crates/html/src/html5/tree_builder/tests/select.rs`,
  `crates/html/src/html5/tree_builder/table/delegation.rs`,
  `crates/html/tests/fixtures/html5/conformance/select-recovery/`,
  `crates/html/tests/fixtures/html5/tree_builder/ae9b-basic-options/`,
  `crates/html/tests/fixtures/html5/tree_builder/ae9b-optgroup-transitions/`,
  `crates/html/tests/fixtures/html5/tree_builder_patches/ae9b-implied-option-parent/`,
  and select parity cases.
- Remaining work: selectedness, control values, UI interaction, and event
  semantics. Decision: closeable as the current full-document parser subset.

#### [#1314 — AE9c — Normalize select roadmap identity and reserve AE10 for templates](https://github.com/joris97jansen/borrowser/issues/1314)

- Relationship: independently scoped Milestone AE normalization issue in the
  AE9 select lineage; AE9b remains the parser-behavior owner. It does not
  supersede AE9b behavior.
- Contract and production: the AE9b contract and the AE13 corpus/fixture
  declarations are the applicable repository contracts; no parser production
  path was changed by AE9c.
- Evidence: `crates/html/tests/fixtures/html5/conformance/README.md`, the
  select fixture declarations under `crates/html/tests/fixtures/html5/`, and
  `docs/html5/ae13d-native-corpus-coverage.md` preserve the renamed identity
  without changing expected input/output.
- Remaining work: none within the behavior-neutral normalization. Decision:
  closeable; AE9b remains the behavior owner.

#### [#1077 — AE10 — Introduce template tree-construction state and parser-created template contents](https://github.com/joris97jansen/borrowser/issues/1077)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae10-template-tree-construction-contract.md`,
  `crates/html/src/html5/tree_builder/dispatch/template.rs`,
  `crates/html/src/html5/tree_builder/template_state.rs`, and typed fragment
  insertion.
- Evidence: `crates/html/src/html5/tree_builder/tests/template.rs`,
  `crates/html/src/html5/session/tests/template.rs`, the
  `crates/html/tests/fixtures/html5/conformance/template-state-eof/` fixture,
  `crates/html/tests/fixtures/html5/tree_builder/ae10-nested-templates/` and
  `crates/html/tests/fixtures/html5/tree_builder_patches/ae10-nested-templates/`
  fixtures, patch snapshots, and
  `crates/runtime_parse/src/tests/runtime.rs` template materialization tests.
- Remaining work: public template APIs, cloning, scripting, custom elements,
  declarative shadow DOM, and live mutation. Decision: closeable as the static
  parser-created contents subset.

#### [#1307 — AE11 — Add namespace-aware SVG and MathML tree-construction boundaries](https://github.com/joris97jansen/borrowser/issues/1307)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae11-foreign-content-tree-construction-contract.md`,
  `crates/html/src/html5/tree_builder/foreign/`, adjusted-name/attribute
  dispatch, and patch
  materialization.
- Evidence: `crates/html/src/html5/tree_builder/foreign/tests.rs`,
  `crates/html/src/html5/tree_builder/foreign/attributes.rs`,
  `crates/html/src/html5/tree_builder/foreign/tables.rs`, the
  `crates/html/tests/fixtures/html5/conformance/foreign-content-integration/`
  and `crates/html/tests/fixtures/html5/conformance/foreign-qualified-attributes/`
  fixtures, the
  `crates/html/tests/fixtures/html5/tree_builder/ae11-mathml-integration/`
  fixture, namespace snapshots, and
  `crates/browser/src/dom_store/tests/` materialization tests.
- Remaining work: SVG rendering, MathML layout, XML parsing, and public
  namespace APIs. Decision: closeable as the namespace-aware parser subset.

#### [#1318 — AE12 — Add HTML processing-instruction tokenization and parser-created nodes](https://github.com/joris97jansen/borrowser/issues/1318)

- Parent: top-level Milestone AE issue; no child slice.
- Contract and production: `docs/html5/ae12-processing-instruction-contract.md`,
  `crates/html/src/html5/tokenizer/processing_instruction.rs`, tree-builder PI dispatch,
  `crates/html/src/dom_patch.rs`, and patch materialization.
- Evidence: tokenizer and
  `crates/html/src/html5/tree_builder/tests/processing_instructions.rs`,
  the `crates/html/tests/fixtures/html5/conformance/processing-instruction-boundaries/`
  and `crates/html/tests/fixtures/html5/conformance/processing-instruction-malformed/`
  fixtures,
  `crates/html/src/conformance/projection.rs`,
  `crates/html/src/patch_validation/`, runtime PI tests, and
  `crates/browser/src/dom_store/tests/processing_instruction.rs`.
- Remaining work: XML/public PI APIs, stylesheet or runtime PI behavior,
  scripting, selector, layout, and paint semantics. Decision: closeable as the
  declared HTML PI subset.

#### [#1320 — AE13a — Establish the canonical parser fixture and observation foundation](https://github.com/joris97jansen/borrowser/issues/1320)

- Parent: AE13 parent; independent child/slice.
- Contract and production: `docs/html5/ae13-parser-conformance-regression-harness.md`,
  `crates/html/src/conformance/`, and
  `crates/html_test_support/src/parser_fixture/`.
- Evidence: fixture schema/load/validate/runner tests, canonical fixture
  corpus under `crates/html/tests/fixtures/html5/conformance/`, and
  `make test-html5-parser-conformance`.
- Remaining work: larger external corpus and full html5lib/WPT. Decision:
  closeable within AE scope.

#### [#1326 — AE13b1 — Establish parser-owned observation plumbing and typed preprocessing/tokenizer diagnostics](https://github.com/joris97jansen/borrowser/issues/1326)

- Parent: AE13b parent (#1321); independent child/slice.
- Contract and production: AE13 harness observation sections,
  `crates/html/src/html5/shared/observation_model.rs`, tokenizer/session
  observation fanout, and `crates/html/src/conformance/execution.rs`.
- Evidence: tokenizer parse-error and diagnostic tests, observation model tests,
  `crates/html_test_support/src/parser_snapshot/parse_errors.rs`,
  `crates/html_test_support/src/parser_snapshot/implementation_diagnostics.rs`,
  token v2 snapshots, preprocessing fixtures, and AE13c parity.
- Remaining work: broader diagnostic coverage and the separate #1331 allocation
  guard. Decision: closeable; #1331 is not part of this acceptance scope.

#### [#1327 — AE13b2 — Migrate tree-construction diagnostics and capture actual document mode](https://github.com/joris97jansen/borrowser/issues/1327)

- Parent: AE13b parent (#1321); independent child/slice.
- Contract and production: `crates/html/src/conformance/execution.rs`,
  tree-builder observation dispatch, and `crates/html/src/document_mode.rs`.
- Evidence: `crates/html_test_support/src/parser_snapshot/document_mode.rs`,
  `crates/html_test_support/src/parser_snapshot/parse_errors.rs`, tree
  diagnostic tests,
  `crates/html/tests/fixtures/html5/conformance/document-mode-quirks/`,
  `crates/html/tests/fixtures/html5/conformance/tokenizer-doctype-null-identity/`,
  and canonical conformance execution.
- Remaining work: complete doctype classification and all standard diagnostic
  positions. Decision: closeable within the documented subset.

#### [#1333 — AE13b2.2 — Introduce typed parser resource-exhaustion failures](https://github.com/joris97jansen/borrowser/issues/1333)

- Parent: AE13b parent (#1321), scoped under AE13b2; independent GitHub issue.
- Contract and production: typed fatal identities in
  `crates/html/src/html5/shared/error.rs`, reservation/failure propagation in
  `crates/html/src/conformance/execution.rs`, session
  failure latching, and runtime fatal handling.
- Evidence: `crates/html/src/html5/session/tests/fatal_failures.rs`, conformance
  failure-injection tests,
  `crates/html_test_support/src/parser_snapshot/final_invariants.rs`, and
  runtime buffering/fatal
  tests. The repository's internal `AE13b2.2a` wording maps to GitHub #1333.
- Remaining work: complete allocation/materialization/OOM policy outside the
  declared reservation sites. Decision: closeable within its explicit scope.

#### [#1328 — AE13b3 — Capture canonical parser-created trees and complete DomPatch streams](https://github.com/joris97jansen/borrowser/issues/1328)

- Parent: AE13b parent (#1321); independent child/slice.
- Contract and production: `docs/html5/dompatch-contract.md`,
  `crates/html/src/conformance/projection.rs`,
  `crates/html/src/dom_snapshot/`, `crates/html/src/dom_patch.rs`, and patch
  history in the parser session.
- Evidence: `crates/html/src/dom_snapshot/tests.rs`,
  `crates/html/src/patch_validation/tests.rs`,
  `crates/html_test_support/src/parser_snapshot/tree.rs`,
  `crates/html_test_support/src/parser_snapshot/patches.rs`, patch golden
  fixtures, template/PI projection tests, and runtime materialization tests.
- Remaining work: public DOM mutation protocol. Decision: closeable within AE
  scope.

#### [#1329 — AE13b4 — Trace actual tree-construction dispatch and unsupported parser branches](https://github.com/joris97jansen/borrowser/issues/1329)

- Parent: AE13b parent (#1321); independent child/slice.
- Contract and production: `crates/html/src/conformance/execution.rs`,
  `crates/html/src/html5/shared/observation_model.rs`,
  `crates/html/src/html5/tree_builder/process_context.rs`,
  `crates/html/src/html5/tree_builder/unsupported.rs`, and table close paths.
- Evidence: transition and unsupported-feature tests in
  `crates/html/src/conformance/execution.rs`,
  `crates/html/src/html5/tree_builder/tests/table_cell.rs`, the six
  unsupported-feature fixtures:
  `crates/html/tests/fixtures/html5/conformance/unsupported-html-attribute-merge/`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-body-attribute-merge/`,
  `crates/html/tests/fixtures/html5/conformance/document-unsupported-features/`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-table-cell-end-guard/`,
  `crates/html/tests/fixtures/html5/conformance/unsupported-table-cell-preparation/`,
  and `crates/html/tests/fixtures/html5/conformance/unsupported-caption-preparation/`;
  transition/unsupported snapshot codecs, and AE13c parity.
- Remaining work: the explicitly documented parser algorithms; observations do
  not implement them. Decision: closeable as an observation slice.

#### [#1330 — AE13b5 — Add deterministic parser snapshot serializers and fixture diagnostics](https://github.com/joris97jansen/borrowser/issues/1330)

- Parent: AE13b parent (#1321); independent child/slice.
- Contract and production/test-support boundary:
  `docs/html5/ae13b5-parser-snapshot-formats.md`,
  `crates/html_test_support/src/parser_snapshot/`, and fixture runner code.
- Evidence: token, parse-error, diagnostic, document-mode, tree, patch,
  transition, unsupported-feature, and final-invariant serializer tests;
  canonical fixture v2/v3 snapshots; deterministic label/lexical validation.
- Remaining work: larger corpus migration and blessing policy. Decision:
  closeable within AE scope.

#### [#1321 — AE13b — Integrate parser observations and deterministic snapshot serializers](https://github.com/joris97jansen/borrowser/issues/1321)

- Parent: AE13 parent (#1308); independently scoped sibling of AE13a, assessed
  from the cumulative AE13b1, AE13b2, AE13b2.2, AE13b3, AE13b4, and AE13b5
  evidence relevant to its acceptance criteria.
- Acceptance-criteria trace:
  - Production origin and no test-only parser:
    `crates/html/src/html5/shared/observation_model.rs`,
    `crates/html/src/conformance/execution.rs`,
    `crates/html/src/conformance/projection.rs`, and the AE13b1/b2/b3/b4
    production observation paths.
  - Typed identity/order:
    `crates/html_test_support/src/parser_snapshot/parse_errors.rs`,
    `crates/html_test_support/src/parser_snapshot/implementation_diagnostics.rs`,
    `crates/html_test_support/src/parser_snapshot/transitions.rs`,
    `crates/html_test_support/src/parser_snapshot/unsupported_features.rs`,
    with sequence tests in `crates/html/src/conformance/execution.rs`.
  - Namespace/template tree boundaries:
    `crates/html/src/conformance/projection.rs`,
    `crates/html_test_support/src/parser_snapshot/tree.rs`,
    `crates/html/src/dom_snapshot/`, and the foreign/template
    conformance fixtures.
  - Parser-significant patch ordering: `docs/html5/dompatch-contract.md`,
    `crates/html_test_support/src/parser_snapshot/patches.rs`,
    `crates/html/src/patch_validation/`, and patch golden fixtures.
  - Actual dispatch transitions:
    `crates/html/src/html5/tree_builder/process_context.rs`,
    `crates/html/src/conformance/execution.rs`, and transition snapshots.
  - No unstable IDs/addresses/derived `Debug`: strict lexical and snapshot
    codecs in `crates/html_test_support/src/parser_snapshot/`, including
    canonical `node-<positive-decimal>` labels.
  - Unsupported reports describe parser limitations only:
    `TreeConstructionUnsupportedFeature` in
    `crates/html/src/html5/shared/observation_model.rs`, its six production
    detection paths, unsupported-feature fixtures, and
    `docs/html5/ae13-parser-conformance-regression-harness.md`.
- Downstream evidence: `crates/html/src/patch_validation/`,
  `crates/runtime_parse/src/tests/runtime.rs`,
  `crates/browser/src/dom_store/tests/`, and AE13c whole/chunk canonical
  comparison prove the observations consume real parser output and survive
  materialization.
- Remaining work: broader corpus and platform conformance, not AE13b's
  observation/serializer acceptance criteria. Decision: closeable only after
  the child records above; the cumulative evidence satisfies the parent.

#### [#1322 — AE13c — Add whole/chunked parity and final parser invariant execution](https://github.com/joris97jansen/borrowser/issues/1322)

- Parent: AE13 parent (#1308); independent child/slice.
- Contract and production: `crates/html/src/streaming_parity.rs`,
  `crates/html/src/conformance/execution.rs` finalization and audit paths, and
  `docs/html5/invariants.md`.
- Evidence: `crates/html/src/streaming_parity.rs` tests,
  `crates/html/src/conformance/` final-invariant tests,
  `crates/html_test_support/src/parser_snapshot/final_invariants.rs`,
  failure-injection tests, native parity
  fixtures, and runtime chunk-parity tests.
- Remaining work: broader input schedules and full allocation policy. Decision:
  closeable within AE scope.

#### [#1323 — AE13d — Build the curated native parser conformance corpus](https://github.com/joris97jansen/borrowser/issues/1323)

- Parent: AE13 parent (#1308); independent child/slice.
- Contract and production: `docs/html5/ae13d-native-corpus-coverage.md` and
  canonical fixture discovery in `crates/html_test_support/src/parser_fixture/`.
- Evidence: `crates/html/tests/fixtures/html5/conformance/`, coverage/provenance
  rows for preprocessing, tokenizer, recovery, tables, forms/select, templates,
  foreign content, namespaces, PIs, diagnostics, and representative documents;
  `make test-html5-parser-conformance`.
- Remaining work: larger corpus and external conformance imports. Decision:
  closeable within the curated scope.

#### [#1324 — AE13e — Add external adapters, snapshot workflow, CI integration, and closeout](https://github.com/joris97jansen/borrowser/issues/1324)

- Parent: AE13 parent (#1308); independent child/slice.
- Contract and production/test-support boundary:
  `docs/html5/ae13e-external-fixture-and-snapshot-workflow.md`,
  `crates/html_test_support/src/external_wpt.rs`, update/check binary, and
  `.github/workflows/html5-conformance.yml`.
- Evidence: `make test-html5-external-fixtures`,
  `make test-html5-external-fixtures-extended`, native/external fixture
  validation, `html5_external_wpt` and `diff_html5` targets, and the scheduled/
  manual workflow definition.
- Remaining work: full html5lib/WPT and larger imports. Decision: closeable as
  the pinned external and workflow subset.

#### [#1308 — AE13 — Add parser conformance fixtures, deterministic debug snapshots, and regression harnesses](https://github.com/joris97jansen/borrowser/issues/1308)

- Parent: Milestone AE parent issue for AE13a, AE13b, AE13c, AE13d, and AE13e;
  assessed only after those independent parent/slice records above.
- Contract and production/test-support evidence:
  `docs/html5/ae13-parser-conformance-regression-harness.md`,
  `docs/html5/ae13b5-parser-snapshot-formats.md`,
  `docs/html5/ae13d-native-corpus-coverage.md`,
  `docs/html5/ae13e-external-fixture-and-snapshot-workflow.md`,
  `crates/html/src/conformance/`, and `crates/html_test_support/src/`.
- Validation: the native corpus, pinned external adapter, whole/chunk parity,
  final-invariant, snapshot, runtime materialization, and extended workflow
  targets recorded in this document all pass. Decision: closeable after the
  child evidence is established; the parent adds no unsupported child scope.

Issue #1331, “HTML: repair pathological small-chunk allocation guard
baseline,” is an open related follow-up that references AE13b1 but is not an
independently named AE issue in the canonical Milestone AE history. It is not
absorbed into AE14. It is a real parser performance/allocation follow-up whose
guard also fails on clean `master`; its threshold must not be weakened or
rebaselined for AE closeout. Its performance-guard scope was not an AE13b1
acceptance requirement and was already baseline-failing independently of
AE13b1, so it is non-blocking for Milestone AE. AE14 does not claim that
allocation behavior is solved: semantic parser/conformance tests do not
substitute for resolving #1331. Its allocation-guard scope remains with that
follow-up issue.

No child acceptance criterion was found to be silently superseded or missing.
The six unsupported tree-construction identities are explicit contract
limitations, not prior-AE blockers.

## Exact remaining gaps and roadmap mapping

Remaining gaps are preserved precisely rather than collapsed into “HTML
supported”:

- tokenizer branches and complete character-reference parity;
- full byte decoding/encoding sniffing beyond the supported input boundary;
- fragment parsing, parser pause/resume, `document.write`, speculative parsing,
  scripting-enabled branches, and full html5lib/WPT coverage;
- public DOM traversal/mutation, ranges/selections, mutation observers, events,
  custom elements, shadow DOM, and complete template/PI APIs;
- complete form-owner algorithms, form submission, validation, focus,
  selection, interaction, and accessibility semantics;
- SVG rendering, MathML layout, XML parsing, animation, filters, geometry, and
  foreign-resource behavior;
- resource discovery/loading, navigation, browsing contexts, history, event
  loop/task queues, and JavaScript execution.

The existing roadmap is preferred for each follow-up: DOM/public mutation work
belongs to the DOM/runtime roadmap; parser-discovered resources and lifecycle
handoff belong to browser/resource-loading work; control state/submission
belongs to the forms platform work; semantic trees belong to accessibility;
bindings, scheduling, `document.write`, custom elements, and live DOM belong
to JavaScript/DOM runtime; and SVG/MathML output belongs to rendering/layout.

The future broader HTML parser-conformance work has two explicit workstreams
under the same coherent parser owner where appropriate. Core document/body
tree-construction work covers repeated `<html>` attribute merging, repeated
`<body>` attribute merging, and repeated-body `frameset_ok`. AE8 table
tree-construction work covers the same-named table-cell scope guard,
implied-end/current-node preparation for table-cell closure, and caption-close
preparation. The same issue may also group fragments, broader corpus, fuzzing,
and differential testing, but it must preserve these ownership distinctions.

No new milestone or GitHub issue is created by AE14 because the repository
roadmap already provides coherent owners. Any future parser-conformance issue
must depend on the AE contracts and explicitly exclude DOM APIs, scripting,
rendering, resources, and navigation unless those owners opt in.

## Documentation consistency audit

The audit covered AE1/AE2, tokenizer contracts and spec matrices, AE7, AE8,
AE9a, AE9b, AE10, AE11, AE12, AE13, `docs/html5/html5-core-v0.md`,
`docs/html5/invariants.md`, `docs/html5/dompatch-contract.md`,
`docs/html5/node-identity-contract.md`, fixture READMEs, and the
feature-gap tracker. Legitimate AE9a/AE9b terminology and the existing AE13
harness filename are preserved. Only demonstrably stale closeout numbering is
normalized to AE14; historical sub-issue names are not globally rewritten.

## Validation record

The following commands were run for this closeout:

- `cargo test -p html --features html5 --lib html5::tokenizer --locked` — pass,
  259 tests.
- `cargo test -p html --features html5 --lib html5::tree_builder --locked` —
  pass, 343 tests.
- `cargo test -p html --features html5 --lib parser --locked` — pass, 35 tests.
- `cargo test -p html --features html5 --lib patch_validation --locked` — pass,
  18 tests.
- `cargo test -p html --features html5 --lib streaming_parity --locked` — pass,
  2 tests.
- `make test-html5-parser-conformance` — pass, canonical fixture corpus.
- `make test-html5-external-fixtures-extended` — pass; no fixture drift, one
  external fixture, one parser-conformance fixture, and one diff/parity test
  passed; two scripting records remained expected unsupported records.
- `make ci` — pass when rerun with permission for the repository's local
  HTTP/HTTPS test servers to bind sockets. The workspace, HTML5
  runtime/conformance, fixture, DOM/patch, smoke, fuzz-smoke, build, release,
  and benchmark stages completed without a failure. The initial sandboxed
  attempt stopped in unrelated `net` tests because local server binding was
  denied with `Operation not permitted`; this was an environment restriction,
  not an AE14 failure. Existing unrelated compiler warnings did not fail the
  successful command.

The extended lane is not duplicated by AE14; it is the repository's existing
correctness-owned lane and is also represented by the scheduled/manual
`.github/workflows/html5-conformance.yml` workflow.

## Final milestone decision

**Closeable with documented out-of-scope gaps**

Every canonical AE issue and independently scoped implementation slice has
been audited against its own acceptance criteria, contract, production path,
tests, fixture/snapshot/parity evidence, and downstream evidence where
applicable. The remaining gaps are either explicitly excluded by the owning AE
contract or belong to later DOM, forms, rendering, accessibility, JavaScript,
resource-loading, navigation, and broader parser-conformance ownership. No
prior-AE blocker was discovered, and AE14 added no substantial parser
algorithm.

Milestone AE establishes a static HTML tokenizer, tree-construction,
parser-created DOM, recovery, diagnostic, and conformance-testing foundation.
It is not complete HTML-platform support, a complete public DOM, a complete
forms platform, an SVG renderer, a MathML renderer, a JavaScript runtime, a
resource loader, a navigation stack, an event loop, an html5lib implementation,
or a WPT-conformant browser platform.
