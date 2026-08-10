# AE13d Native Parser Corpus Coverage

This document is the coverage and provenance inventory for AE13d. It is part
of the review contract for the native v2 corpus; it is not a second fixture
format and it does not add fixture registration or snapshot-blessing
workflow. The complete canonical root currently contains 36 active native v2
bundles, including unchanged AE13 coverage audited for this issue.

## Authoring and provenance rules

The parser under test is never the conformance oracle. A generated v2 snapshot
may be used as candidate serialization while authoring, but each
parser-significant expectation is reviewed against the completed AE contract
and the applicable independent evidence listed below. In particular, current
output is not accepted blindly for duplicate attributes, self-closing syntax,
EOF recovery, document mode, formatting adoption, tables, forms/select,
templates, namespaces, integration-point breakout, or HTML processing
instructions.

Native provenance cannot be stored in the v2 `[source]` table: the schema
requires native fixtures to omit `source.provenance`. The fixture description,
this inventory, the referenced AE contract, and the existing unit/golden/WPT
evidence are therefore the provenance record. No sidecar provenance field is
being introduced.

`input.bin` is used only for the completed parser byte/preprocessing boundary:
the existing UTF-8 decoder, CR/LF normalization, and byte-delivery behavior.
This corpus does not add encoding sniffing, legacy encodings, transport or
network decoding, resource loading, or another byte-to-Unicode layer.

Observation surfaces are semantic contracts, not a dump of available state.
Transitions are present only when parser state is the behavior under test.
Patches are required where final-tree equality can hide insertion ordering,
moves, delayed insertion, foster parenting, or template-content attachment.
Whole-input execution is the parity baseline only; it is not semantic evidence.

## Coverage matrix

Status values are **promoted** (focused semantic v2 expectations), **gap**
(trusted evidence exists outside v2, but canonical coverage is missing or
only invariant/parity coverage exists), and **follow-up** (the completed AE
contract or production behavior exposes a defect/limitation that AE13d must
not repair or hide).

| AE behavior | Completed contract / trusted evidence | Canonical fixture(s) | Status and exact scope | Proving observations | Independent evidence / provenance |
| --- | --- | --- | --- | --- | --- | --- |
| Input preprocessing and tokenizer recovery | AE4 input preprocessing; AE5 tokenizer states; tokenizer unit/golden tests | `utf8-decoding-preprocessing`, `preprocessing-cr-normalization`, `tokenizer-eof-recovery` | promoted for UTF-8 byte delivery, lone-CR normalization, and EOF recovery; invalid/legacy decoding remains outside AE13d | tokens, errors, invariants, parity on all three; the CR fixture splits immediately before the held lone CR is finalized | `docs/html5/ae4-tokenizer-input-preprocessing.md`; `crates/html/src/html5/tokenizer/tests/input_preprocessing.rs`; `eof_recovery.rs`; `fixtures/html5/tokenizer/tok-ae4-*` |
| Comments and EOF | AE5 comment recovery; AE6 comment construction | `comment-and-shell`, `tokenizer-eof-recovery` | promoted for comment construction and malformed-comment EOF; the minimal comment fixture is intentionally whole-only | tokens and tree on `comment-and-shell`; tokens/errors and parity on `tokenizer-eof-recovery` | `crates/html/src/html5/tokenizer/tests/comments.rs`; `eof_recovery.rs`; `tree_builder/ae6-comments-document-structure` |
| Doctypes and document mode | AE5 doctype token contract; AE2/AE13b2 mode; `quirks.rs` | `tokenizer-doctype-null-identity`, `document-mode-quirks`, `document-mode-limited`, `document-mode-no-quirks`, `document-structured-observations` | promoted; mode fixtures are doctype-minimal and whole-only because mode is the sole contract | tokens or document mode only; no parity claim | `crates/html/src/html5/tokenizer/tests/doctype.rs`; `crates/html/src/html5/tree_builder/tests/quirks.rs`; `fixtures/html5/tokenizer/tok-doctype-*` |
| Attributes, duplicates, self-closing syntax | AE2 first-wins attributes; AE5 tag/attribute states; AE9 self-closing diagnostics | `document-recovery-diagnostics`, `tag-attribute-boundaries` | promoted for duplicate retention, non-void self-closing recovery, and ordinary tag/attribute delivery; recovery fixture is whole-only, ordinary tag fixture supplies its declared parity | tokens, errors, diagnostics, tree; parity only through `tag-attribute-boundaries` | `docs/html5/ae2-parser-created-dom-node-model.md`; `crates/html/src/html5/tokenizer/tests/tags_attrs.rs`; `fixtures/html5/tokenizer/tok-active-attrs-dedupe`; `tree_builder/ae9-self-closing-finalization` |
| Document shell and body recovery | AE6 shell/body construction | `comment-and-shell`, `body-text-recovery`, `document-structured-observations` | promoted for comment-before-shell, implicit body recovery, and a combined shell; these fixtures are whole-only | tree; no parity claim | `crates/html/src/html5/tree_builder/tests/insertion_modes.rs`; `tree_builder/ae6-comments-document-structure`; `tree_builder/ae6-text-before-body` |
| Formatting/adoption recovery | AE7 AFE/Noah's Ark/adoption contracts | `active-formatting-recovery` | promoted for adoption reconstruction with parser error and ordered patches; declared scalar parity executes | tree, patches, errors, invariants, parity | `crates/html/src/html5/tree_builder/tests/formatting.rs`; `aaa.rs`; `fixtures/html5/tree_builder_patches/ae7-formatting-reconstruct-after-p-close` |
| Tables, wrappers, pending text, foster parenting, non-append insertion | AE8/AE13b4 table contracts | `table-buffering-foster-parenting`, `foster-parented-element`, `select-table-interaction` | promoted for pending text, fostered text/element `InsertBefore`, and select-table interaction; broader table-mode variants remain lower-level evidence | tree, patches, parity on all three | `crates/html/src/html5/tree_builder/tests/table_modes.rs`; `table_state.rs`; `fixtures/html5/tables/patches/ae8-foster-text-and-element`; `tree_builder_patches/i7-foster-parent-text-then-element`; `tree_builder_patches/ae9b-fostered-select` |
| Form parser behavior | AE9 form pointer and form-element contracts | `form-pointer-state`, `form-self-closing`, `form-controls-textarea` | promoted for nested form-pointer recovery, non-void form self-closing, input, textarea, and button construction; other controls remain lower-level evidence | tree, errors/diagnostics where applicable, invariants, parity on all three | `crates/html/src/html5/tree_builder/tests/form_controls.rs`; `attributes.rs`; `fixtures/html5/tree_builder/ae9-form-recovery`; `ae9-self-closing-finalization`; `ae9-basic-form-controls`; `ae9-textarea-leading-lfs` |
| Select parser behavior | AE9b select modes and option recovery | `select-recovery`, `select-table-interaction` | promoted for option replacement and select-in-table foster placement; nested/customizable select cases remain lower-level evidence | tree, patches for table interaction, parity on both | `crates/html/src/html5/tree_builder/tests/select.rs`; `fixtures/html5/tree_builder_patches/ae9b-fostered-select`; `tests/wpt/provenance/select-table-foster-option.provenance.txt` |
| Templates and template-content boundaries | AE10 template modes, contents ownership, EOF | `template-state-eof`, `document-structured-observations` | promoted for nested template contents, table modes, EOF attachment, and a combined template/SVG case; parity is exercised only by `template-state-eof` | tree, patches, invariants, parity on `template-state-eof` | `crates/html/src/html5/tree_builder/tests/template.rs`; `fixtures/html5/tree_builder/ae10-nested-templates`; `ae10-unclosed-eof`; `tests/wpt/provenance/template-nested-table-modes.provenance.txt` |
| SVG and MathML namespaces | AE11 expanded-name and foreign-tree contract | `foreign-content-integration`, `foreign-qualified-attributes`, `document-structured-observations` | promoted for SVG/MathML elements, HTML integration, adjusted SVG names, and XML/XLink/XMLNS attributes; breakout parity is not claimed for the whole-only breakout fixture | tree; parity on `foreign-content-integration` and `foreign-qualified-attributes` | `crates/html/src/html5/tree_builder/foreign/tests.rs`; `fixtures/html5/tree_builder/ae11-svg-qualified-attrs-self-close`; `tests/wpt/provenance/ae11-qualified-xlink.provenance.txt`; `ae11-annotation-xml-svg.provenance.txt` |
| Integration points and foreign-content breakout | AE11 integration-point and breakout rules | `foreign-content-integration`, `foreign-breakout-integration` | promoted for `foreignObject`, MathML `annotation-xml` HTML integration, and HTML paragraph breakout; breakout fixture is intentionally whole-only | tree; parity only on the integration fixture | `fixtures/html5/tree_builder/ae11-foreignobject-html`; `ae11-mathml-integration`; `ae11-breakout-recovery`; `tests/wpt/provenance/ae11-svg-desc-html-integration.provenance.txt` |
| HTML processing instructions | AE12 HTML PI contract only | `processing-instruction-boundaries`, `processing-instruction-malformed` | promoted for valid HTML PI placement/data and malformed/disallowed target recovery; both declare scalar parity | tokens, errors, tree, patches, parity | `docs/html5/ae12-processing-instruction-contract.md`; `crates/html/src/html5/tokenizer/tests/processing_instructions.rs`; `tree_builder/tests/processing_instructions.rs`; `tests/wpt/provenance/ae12-processing-instructions.cases` |
| Malformed recoverable structures | AE5–AE12 recovery contracts | `document-recovery-diagnostics`, `tokenizer-eof-recovery`, `processing-instruction-malformed`, `body-text-recovery` | promoted for focused malformed cases plus the malformed combined diagnostics document; parity is claimed only for the declared split fixtures | errors, tree, rule-specific patches, parity where declared | `crates/html/src/html5/tokenizer/tests/parse_errors.rs`; `tree_builder/tests/recovery.rs`; `fixtures/html5/tree_builder/ae12-malformed-escaped`; `tree_builder_patches/ae12-in-table-text-order` |
| Representative and combined static documents | AE6 shell through AE12 parser-created DOM | `representative-static-document`, `document-structured-observations`, `document-recovery-diagnostics` | promoted: ordinary static composition, ordinary template/SVG composition, and malformed duplicate/self-closing composition are additive integration checks; none claims parity | mode/tree/diagnostics as selected; no parity claim | `fixtures/html5/tree_builder/ae6-complete-document`; `ae10-nested-templates`; `ae11-mathml-integration`; canonical structured/recovery fixtures |
| Exact documented unsupported parser features | AE13b4 exact unsupported identities | `document-unsupported-features`, `unsupported-html-attribute-merge`, `unsupported-body-attribute-merge`, `unsupported-table-cell-end-guard`, `unsupported-table-cell-preparation`, `unsupported-caption-preparation` | promoted as explicit current limitations; all are whole-only active runs, not positive conformance substitutions | exact unsupported-feature identities only; no parity claim | `docs/html5/ae13-parser-conformance-regression-harness.md`; `crates/html/src/conformance/execution.rs` exact-trigger tests; `parser_snapshot/unsupported_features.rs` |

The audit also retained unchanged canonical v2 coverage rather than replacing
it: `character-reference-boundaries`, `text-mode-boundaries`,
`tokenizer-character-data`, `tokenizer-doctype-null-identity`,
`document-unsupported-features`, and `document-structured-observations` remain
active evidence for their existing AE contracts. Their whole-only or declared
parity surfaces are not silently generalized to unrelated rows above.

## Review gates

Every new or promoted row must have a minimum input shape, an explicit reason
that a simpler fixture is insufficient, and an evidence reference. Expectations
must be exact. A parser, harness, invariant, snapshot-format, or contract
mismatch is recorded as a follow-up; its expectation is not weakened and it is
not reclassified as unsupported.

The final audit must show every requirement row promoted with a focused
fixture, or explicitly marked follow-up with its owning issue. Combined
documents are additive and cannot substitute for focused rows.

## AE13d corpus quality gates

- Every requirement row maps to at least one focused v2 fixture and an
  observation surface that proves the rule; combined documents are additive.
- Every parser-significant expectation names the completed AE contract and
  independently reviewed unit, golden, html5lib/WPT, or equivalent evidence.
- No expectation was blessed from Borrowser output alone, and no skip, xfail,
  wildcard, permissive snapshot, or broad unsupported declaration hides an AE
  behavior.
- Patches are present for foster parenting, non-append insertion, formatting
  reconstruction, and template-content attachment; incidental bookkeeping is
  not serialized.
- Unsupported-feature records are exact AE13b4 identities and remain distinct
  from positive conformance fixtures.
- The canonical v2 test passes, the relevant tokenizer/tree-builder/golden
  lanes pass, the coverage matrix matches the discovered fixture tree, and
- AE13e adds the separate canonical external adapter/provenance/update path;
  this native coverage document remains limited to the native corpus and does
  not claim complete html5lib or WPT support.
