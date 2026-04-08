# O6: Define Canonicalization And Deterministic Serialization For The CSS Model

Last updated: 2026-04-08  
Status: implemented

This document defines the stable snapshot and canonicalization contract for
Borrowser's engine-facing CSS stylesheet/rule/declaration/value model in
`css::model`.

Milestones O2 through O5 established the model's structural types, declaration
representation, semi-typed value representation, and span policy. O6 turns that
surface into a first-class regression and review contract.

Related code:
- `crates/css/src/model/mod.rs`
- `crates/css/src/model/serialize.rs`
- `crates/css/src/model/tests.rs`
- `crates/css/tests/model_golden.rs`

Related documents:
- `docs/css/o1-rule-value-model-architecture.md`
- `docs/css/n6-stable-debug-serialization.md`
- `docs/css/syntax-parser-contract.md`

## Implemented Result

The engine-facing CSS model now has explicit stable serializer functions for:

- stylesheets
- parsed stylesheet results
- individual rules
- individual declarations
- individual declaration values

These serializers are deterministic, versioned, human-readable, and aligned
with the model layer rather than authored CSS pretty-printing.

Repository-backed golden fixtures now cover representative valid and malformed
model snapshots under `crates/css/tests/fixtures/model/`.

## Why This Exists

The engine-facing CSS model is now the handoff contract for later selector,
cascade, and computed-style work. That model needs a regression surface that:

- is independent of Rust derived `Debug`
- reflects model-layer structure rather than authored CSS formatting
- preserves source order and span policy explicitly
- makes canonicalized versus authored-preserved fields obvious in reviews
- remains stable across runs and allocator/layout differences

Without this surface, later Milestone O work would have to review model changes
through ad hoc debugging output or syntax-layer snapshots that do not accurately
describe the engine-owned representation.

## Stable Serializer Surface

The stable model snapshot surface now includes:

- `css::model::serialize_stylesheet_for_snapshot`
- `css::model::serialize_stylesheet_parse_for_snapshot`
- `css::model::serialize_rule_for_snapshot`
- `css::model::serialize_declaration_for_snapshot`
- `css::model::serialize_value_for_snapshot`
- `css::model::StylesheetParse::to_debug_snapshot`

Snapshot guarantees:

- snapshots begin with `version: 1`
- stylesheet snapshots begin with the explicit kind marker
  `model-stylesheet`
- field order is fixed and explicit
- rules serialize in stored source order
- declarations serialize in stored source order
- declaration value components serialize in stored order
- preserved selector/prelude/block component lists serialize in stored order
- diagnostics serialize in encounter order
- stats serialize in a fixed field order
- spans serialize only as byte-offset ranges (`@start..end`) or `@<none>`
- unresolved model text uses explicit sentinels such as `<invalid-name>` and
  `<invalid-text>`
- escaping is model-owned and stable rather than borrowed from Rust `Debug`

Snapshot versioning guidance:

- bump `SNAPSHOT_VERSION` only when the snapshot grammar itself changes in a
  fixture-breaking way
- do not bump the version for ordinary parser/model behavior changes that still
  fit the same grammar; update the fixtures instead

## Model-Layer Canonicalization Rules

O6 does not introduce new semantic normalization. It formalizes the
canonicalization already permitted by the Milestone O model contract.

The model snapshot reflects these rules:

- standard property names serialize from canonical lowercase model text
- at-rule names serialize from canonical lowercase model text when resolution
  succeeded
- custom property names serialize from preserved authored model text
- value text serializes from preserved authored source text when resolution
  succeeded
- unsupported or unresolved text-bearing payloads serialize explicit invalid
  sentinels instead of manufacturing plausible fallback strings
- no rule, declaration, or value reordering is performed for serialization
- no computed-style normalization, shorthand expansion, unit folding, or color
  canonicalization is introduced here

This keeps the snapshot aligned with the internal model rather than implying
later semantic interpretation has already happened.

## Golden Fixture Coverage

Model snapshot fixtures now cover:

- representative valid stylesheet/rule/declaration/value structure
- malformed stylesheet recovery with deterministic surviving rule order

These fixtures live under:

- `crates/css/tests/fixtures/model/`

## Exit Criteria

- engine-facing model debug output is deterministic
- stylesheet/rule/declaration/value formatting is explicit and stable
- model-layer canonicalization rules are defined in-repository
- representative file-backed model fixtures exist and pass stably
