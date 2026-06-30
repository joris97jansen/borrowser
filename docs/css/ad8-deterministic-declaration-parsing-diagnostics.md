# AD8: Deterministic Declaration Parsing And Diagnostics

Last updated: 2026-06-30
Status: implemented contract for Milestone AD issue 8

This document defines Borrowser's current declaration-list parsing,
declaration-to-cascade classification, unsupported-property handling, and
declaration pipeline debug output.

AD8 does not add broad CSS property coverage, a complete known-property
database, custom property semantics, full cascade conformance, media queries,
animations, CSSOM, selector invalidation, or additional shorthand families.

Related code:

- `crates/css/src/syntax/parser`
- `crates/css/src/model`
- `crates/css/src/cascade/integration/declarations.rs`
- `crates/css/src/cascade/integration/debug_snapshot.rs`
- `crates/css/src/cascade/contract/snapshot.rs`
- `crates/css/src/properties`
- `crates/css/src/specified`

Related documents:

- `docs/css/ad1-css-value-property-ownership-architecture.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad6-shorthand-expansion-foundation.md`
- `docs/css/n5-deterministic-parse-recovery.md`
- `docs/css/s5-invalid-value-handling-fallback.md`
- `docs/css/r8-cascade-style-resolution-debug-output.md`
- `docs/engine-feature-gap-tracker.md`

## Purpose

Declaration interpretation is a CSS subsystem boundary. Authored declaration
lists must parse deterministically, recover from malformed declarations where
the current parser supports recovery, and classify each resulting declaration
before rendering consumers can observe CSS values.

AD8 adds a real model-layer declaration-list parse entrypoint so inline style
attributes no longer need to be parsed by wrapping text in a synthetic
stylesheet rule. The model declaration-list path reuses the existing tokenizer
and structured syntax parser, then converts structured syntax declarations into
the same engine-facing `model::Declaration` type used by stylesheet rules.

## Declaration Pipeline

The current declaration pipeline is:

```text
declaration-list text
  -> syntax tokenizer/parser declaration-list recovery
  -> model::Declaration list
  -> cascade declaration classification
  -> supported candidates
  -> current-scope winner resolution
```

The cascade classification step is owned by CSS:

- supported longhands resolve through `property_registry().lookup_id(...)`;
- supported shorthands resolve through `shorthand_registry().lookup_id(...)`
  and `expand_shorthand_declaration(...)`;
- unknown standard names become generic unsupported-property declarations;
- custom-shaped property names remain explicit custom-property declarations;
- invalid property names remain explicit invalid-property-name declarations;
- invalid supported longhand values become invalid-value declarations;
- invalid supported shorthand values become invalid-shorthand declarations.

Only supported declarations become cascade candidates. Unsupported, custom,
invalid, and malformed declarations are deterministic non-candidates.

## Known Versus Unsupported Properties

Borrowser does not currently maintain a complete known CSS property universe.
The supported longhand registry is the supported-property universe for
computed style. The supported shorthand registry contains only shorthands with
implemented expansion behavior.

Representative unsupported standard CSS properties such as `border`,
unsupported flex properties, or unknown names such as `zoom` are therefore
classified as generic unsupported properties unless a future issue adds a real
supported parser, metadata, computed-value contract, tests, and docs.

Unsupported properties must not be added as fake placeholder longhands.

## Invalid Values

Known supported longhands route through the registry-backed specified-value
parser. If parsing fails, the current invalid-value policy is
`RejectDeclaration`.

Rejected declarations:

- preserve deterministic debug text and a stable invalid reason;
- do not become cascade candidates;
- cannot overwrite earlier valid declarations;
- allow later valid declarations to win through the existing cascade subset.

Layout, Paint, GFX, and Browser/runtime must not recover invalid supported
values after CSS rejects them.

## Shorthand Behavior

Supported shorthands are parsed through the shorthand registry and expansion
mechanism. The current supported shorthand subset remains `outline`.

Shorthand expansion is atomic. A failing shorthand emits no longhand
candidates. If a shorthand expands successfully, the emitted longhands still
flow through the normal longhand specified-value parser before entering
candidate generation.

Unsupported shorthands such as `border`, `margin`, and `padding` remain
generic unsupported properties until implemented deliberately.

## Parser Recovery Scope

AD8 documents the currently supported declaration-list recovery behavior; it
does not claim full CSS Syntax recovery.

Current declaration-list recovery:

- malformed declarations report deterministic syntax diagnostics;
- recovery advances to a top-level semicolon, declaration block end, EOF, or a
  deterministic next declaration start when one is visible;
- later valid declarations can still be emitted after representative malformed
  input;
- recovery is bounded by existing tokenizer/parser limits;
- diagnostics are ordered by encounter and are not rendering behavior.

Malformed declarations that are not emitted as model declarations cannot become
cascade candidates.

## Debug Surface

AD8 adds `declaration_list_pipeline_debug_snapshot(...)`.

The snapshot is a maintenance and regression surface. It records:

- model declaration-list parse output;
- syntax diagnostics and parse stats;
- cascade declaration classification;
- invalid-value and invalid-shorthand reason labels;
- candidate materialization;
- winner resolution for the current supported cascade subset.

The snapshot is not CSSOM, not presentation-facing serialization, and not a
runtime behavior path.

## Invariants

- CSS owns declaration interpretation.
- Browser/runtime, Layout, Paint, and GFX do not gain CSS property-name
  knowledge.
- Supported longhands route through the supported longhand registry.
- Supported shorthands route through the supported shorthand registry and
  expansion mechanism.
- Unsupported properties remain deterministic non-candidates.
- Custom-shaped declarations do not implement custom property semantics.
- Invalid supported values do not overwrite previous valid declarations.
- Invalid shorthand expansion is atomic.
- Diagnostics and debug output are stable regression contracts, not rendering
  inputs.

## Deliberate Exclusions

AD8 deliberately excludes:

- complete known-unsupported property modeling;
- fake placeholder longhands;
- broad CSS property coverage;
- additional shorthand families;
- custom properties, `var(...)`, inheritance, or computed custom property
  storage;
- full CSS Syntax recovery;
- full cascade conformance;
- selector invalidation;
- media queries and container queries;
- animations and transitions;
- Browser/runtime, Layout, Paint, or GFX property-name handling.
