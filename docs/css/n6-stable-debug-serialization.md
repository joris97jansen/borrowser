# N6: Add Stable Debug And Serialization Output For Syntax-Layer Testing

Last updated: 2026-04-05  
Status: implemented

## Implemented Result

N6 formalized the stable snapshot and serialization contract for the CSS syntax
layer and added file-backed golden fixtures for representative tokenizer and
parser cases.

The syntax layer now exposes explicit stable serializer functions for:

- tokenizer output
- structured stylesheet parse output
- declaration-list parse output
- token streams

Those serializers are deterministic, human-readable, and independent of Rust
derived `Debug` formatting.

Their implementation now lives in `crates/css/src/syntax/serialize.rs`, which
keeps stable snapshot formatting isolated from tokenizer and parser mechanics.

## Why This Exists

Long-lived parser work needs a regression surface that is:

- stable across runs
- focused on syntax-layer state
- readable enough to diagnose failures quickly
- independent of internal Rust formatting details

Earlier milestones introduced stable snapshot-shaped methods and serializers,
but N6 finishes that work by making the contract explicit and backing it with
golden fixtures stored in the repository.

## Delivered Changes

- added explicit stable serializer functions for tokenizer and parser result
  types
- versioned the stable snapshot format with an explicit `version: 1` header
- kept tokenizer, stylesheet, and declaration-list snapshot output
  deterministic
- added file-backed golden fixtures under `crates/css/tests/fixtures/`
- added regression tests covering representative valid and malformed tokenizer
  and parser cases
- replaced Rust `Debug`-style string escaping with syntax-owned stable escaping
- kept snapshot output focused on syntax-layer state rather than cascade or
  computed-style semantics

## Stable Serializer Surface

The stable syntax snapshot surface now includes:

- `serialize_tokens_for_snapshot`
- `serialize_tokenization_for_snapshot`
- `serialize_stylesheet_for_snapshot`
- `serialize_stylesheet_parse_for_snapshot`
- `serialize_declarations_for_snapshot`
- `serialize_declaration_list_parse_for_snapshot`
- `serialize_compat_stylesheet_for_snapshot`

The compatibility snapshot remains available for migration/testing support, but
the primary golden contract is centered on syntax-layer tokenizer and parser
state.

Ordering and formatting guarantees for this surface are explicit:

- snapshots begin with `version: 1`
- tokens serialize in emission order
- rules serialize in source order
- declarations serialize in source order
- diagnostics serialize in encounter order
- string and character escaping is syntax-owned and stable rather than borrowed
  from Rust `Debug`

Snapshot versioning guidance:

- bump `SNAPSHOT_VERSION` only when the snapshot format itself changes in a
  way that invalidates existing goldens
- do not bump `SNAPSHOT_VERSION` for ordinary parser/tokenizer behavior changes
  that still fit the same serialization grammar; update the fixtures instead

## Golden Fixture Coverage

Representative fixtures now exist for:

- valid tokenizer output
- malformed tokenizer recovery output
- valid structured stylesheet parsing
- malformed structured stylesheet recovery
- malformed declaration-list recovery

These fixtures live under:

- `crates/css/tests/fixtures/tokenizer/`
- `crates/css/tests/fixtures/parser/`
- `crates/css/tests/fixtures/declarations/`

## Exit Criteria

- tokenizer output has stable test formatting
- parser output has stable test formatting
- golden/regression fixtures exist for representative valid and malformed inputs
- debug/serialization output is usable for future CSS milestone tests
