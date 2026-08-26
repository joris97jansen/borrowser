# CSS Fixture And Conformance-Evidence Index

This directory contains deterministic regression fixtures for Borrowser's CSS
subsystem. The fixtures exercise production parsers, typed CSS models, and
CSS-owned debug serializers. They are not parser inputs to the engine, public
web-platform APIs, or a claim of full CSS conformance.

Milestone AF10 uses this index to consolidate the existing selector, cascade,
inheritance, computed-style, and representative-document evidence. AF10 does
not introduce a parallel conformance implementation or an aggregate snapshot
format.

## Fixture bands

| Directory | Authoritative harness | Evidence |
| --- | --- | --- |
| `tokenizer/` | `crates/css/tests/syntax_golden.rs` | Tokenization and tokenizer recovery snapshots. |
| `parser/` | `crates/css/tests/syntax_golden.rs` | Structured stylesheet parsing and malformed recovery. |
| `model/` | `crates/css/tests/model_golden.rs` | Engine-facing stylesheet/rule/value model output, including selector diagnostics. |
| `selectors/` | `crates/css/tests/selector_golden.rs` | Typed selector parse results, selector AST, per-selector and per-compound specificity, invalid selectors, unsupported selectors, and the supported tree-structural pseudo-class subset. |
| `declarations/` | `crates/css/tests/syntax_golden.rs` and `crates/css/src/cascade/integration/debug_snapshot.rs` | Declaration parsing, classification, cascade candidacy, unsupported/custom properties, invalid values, and deterministic declaration-pipeline diagnostics. |
| `properties/` | `crates/css/tests/property_registry_golden.rs` and `crates/css/src/properties/tests.rs` | Registry metadata, supported-property coverage, specified/computed boundaries, shorthand expansion, and invalidation-impact classification. |
| `computed/` | `crates/css/tests/computed_golden.rs` | Representative computed values and deterministic `ComputedDocumentStyle` output. |
| `representative_pages/` | `crates/css/tests/representative_pages.rs` | Curated static documents parsed by AE's real `html::parse_document` path and styled through production selector matching, cascade, inheritance, inline-style, and computed-style APIs. |

## Deterministic AF evidence outside this directory

Some authoritative AF evidence is intentionally colocated with the owning
implementation because it needs crate-private invariant seams or bounded
diagnostic state. These tests are not replaced by external goldens:

- selector matching semantics and versioned snapshots:
  `crates/css/src/selectors/matching/tests/`;
- parser-backed selector matching and document-mode conformance:
  `crates/css/tests/af4_parser_conformance.rs` and
  `crates/css/tests/host_language_matching.rs`;
- parser-document versus Browser materialization parity:
  `crates/browser/tests/af4_selector_materialization_parity.rs`;
- integrated matched-selector diagnostics:
  `crates/css/src/document_selector_matching/tests.rs`;
- rule collection, cascade candidates and winners, inheritance, defaulting,
  and resolved-style snapshots: `crates/css/src/cascade/tests/` and
  `crates/css/src/cascade/contract/tests/`;
- computed-style semantic, fallibility, reuse, and style-tree tests:
  `crates/css/src/computed/tests/`;
- selector/cascade invalidation dependency snapshots and bounded failure
  behavior: `crates/css/src/style_invalidation/dependencies.rs`;
- Browser retention and opaque CSS invalidation consumption:
  `crates/browser/src/tab/tests/style_cache.rs` and
  `crates/browser/src/tab/tests/dom_patches.rs`;
- computed-style authority at the Layout and Paint handoffs:
  `crates/browser/src/rendering/tests/phase_boundaries.rs`.

The direct semantic assertions in these tests remain authoritative. Snapshots
supplement those assertions by pinning deterministic diagnostic and artifact
projections.

## Supported and unsupported evidence

Supported fixtures cover only the selector, property, cascade, inheritance,
and computed-value subsets recorded in the owning contracts. Unsupported or
deferred syntax is exercised through the real parser, rule-collection, and
cascade paths and remains non-matchable, inactive, or non-candidate as
appropriate.

In particular, fixture presence does not imply support for dynamic or
functional pseudo-classes, pseudo-elements, media queries, custom properties,
cascade layers, CSS scoping, animations, transitions, CSSOM, JavaScript-facing
style APIs, broad property/value coverage, or broad CSS WPT conformance.

## Snapshot discipline

- Stable serializers, not Rust's derived `Debug`, define snapshot text.
- Existing bounded diagnostics retain their configured record, storage, and
  serialized-byte limits.
- Snapshot regeneration is never the correctness oracle. A changed golden must
  be justified against semantic assertions and the owning contract.
- Internal snapshots are regression contracts, not rendering inputs or public
  CSSOM-like APIs.
