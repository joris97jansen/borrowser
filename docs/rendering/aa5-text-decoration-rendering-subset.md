# AA5: Text Decoration Rendering Subset

Last updated: 2026-06-11
Status: implemented supported underline subset for Milestone AA issue 5

This document defines Borrowser's first text decoration rendering subset. It
extends the AA1 paint ordering contract and AA2 paint primitive model without
introducing broad shorthand expansion, font-table underline metrics, bidi/ruby
decoration behavior, retained paint state, or compositor behavior.

Related code:
- `crates/css/src/properties/types.rs`
- `crates/css/src/properties/data.rs`
- `crates/css/src/specified/text_decoration.rs`
- `crates/css/src/computed/style.rs`
- `crates/layout/src/inline/tokens.rs`
- `crates/layout/src/inline/types.rs`
- `crates/layout/src/inline/engine/text.rs`
- `crates/gfx/src/paint/primitives.rs`
- `crates/gfx/src/paint/inline.rs`
- `crates/gfx/src/paint/contracts.rs`

Related documents:
- `docs/rendering/aa1-paint-model-architecture-ordering-contracts.md`
- `docs/rendering/aa2-paint-primitives-input-model.md`
- `docs/rendering/w7-inline-formatting-context-foundations.md`
- `docs/css/s9-property-system-computed-style-runtime-contract.md`

## Supported CSS Surface

AA5 supports the canonical property:

- `text-decoration-line`

Supported values are:

- `none`
- `underline`

The initial value is `none`. `text-decoration-line` is not globally inherited.
Borrowser does not implement the `text-decoration` shorthand in AA5 because the
current supported CSS property pipeline is one canonical property name to one
single-component specified value. Shorthand expansion must be introduced as an
explicit CSS feature in a later issue.

## Ownership

CSS owns parsing, cascade participation, computed values, defaults, and the
supported property vocabulary for `text-decoration-line`.

Layout owns inline fragment geometry, baselines, ascent/descent metrics, and
the explicit inline decoration context used to propagate an active underline
from an inline-formatting-context host or inline container to descendant text
fragments. This context is layout-owned metadata; it is not CSS inheritance.

Paint owns semantic `TextDecoration` primitive construction and ordering from
layout inline fragments. Paint consumes the resolved inline decoration metadata
and does not infer decorations from raw CSS declarations.

GFX/backend owns only low-level drawing of the resolved primitive in the
immediate frame path.

Browser/runtime orchestration does not infer, emulate, schedule specially, or
repair text decoration behavior.

## Inline Decoration Model

AA5 decorates text fragments only. When layout builds inline text fragments, it
attaches optional `InlineTextDecoration` metadata with:

- decoration line;
- current text color for the fragment;
- fragment font size;
- deterministic underline thickness;
- deterministic underline offset.

The supported propagation subset is:

- `text-decoration-line: underline` on the inline-formatting-context host
  activates underline for descendant text fragments in that inline context;
- `text-decoration-line: underline` on an inline container activates underline
  for descendant text fragments in that container;
- descendant `none` does not cancel an active underline in AA5;
- atomic inline and replaced fragments are not decorated by AA5.

## Geometry

Underline placement is derived from inline fragment geometry and metrics, not
from generic block, content, or box bottoms.

For one text fragment:

```text
fragment baseline = fragment paint rect y + ascent + baseline shift
underline y       = fragment baseline + underline offset
underline x       = fragment paint rect x
underline width   = fragment paint rect width
```

AA5 uses deterministic fallback metrics:

```text
thickness        = max(1px, font-size * 0.0625)
underline offset = max(1px, descent * 0.35)
```

These are stable CSS-px values. They are not real font-table underline metrics.

## Paint Order

Text decoration is part of the existing inline formatting content phase. For
each decorated text fragment, paint emits:

1. `Text`
2. `TextDecoration`

The containing box's supported paint order remains:

1. background
2. border
3. list marker
4. overflow clip for contents and descendants
5. inline formatting primitives
6. child paint nodes in layout child order
7. outline

## Determinism

For a fixed DOM, computed style tree, layout output, viewport, text measurer,
resource state, and input state:

- `text-decoration-line` resolves through the CSS property registry;
- inline propagation is deterministic and layout-owned;
- decoration thickness and offset are deterministic functions of fragment
  font size and descent;
- paint input snapshots expose `TextDecoration` as a semantic primitive;
- backend drawing consumes the same metadata used by the semantic paint model.

## Deliberate Exclusions

AA5 deliberately does not implement:

- `text-decoration` shorthand expansion;
- `text-decoration-color`;
- `text-decoration-style`;
- `text-decoration-thickness`;
- `overline`;
- `line-through`;
- `blink`;
- skip-ink;
- real font-table underline metrics;
- descendant cancellation semantics;
- bidi-specific decoration behavior;
- ruby;
- vertical writing modes;
- atomic inline or replaced decoration behavior;
- full CSS Text Decoration propagation semantics;
- stacking, compositing, retained display lists, GPU-specific behavior, or
  pixel snapshot testing.
