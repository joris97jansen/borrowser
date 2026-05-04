# W3: Display-To-Box Generation Behavior

Last updated: 2026-05-04  
Status: implemented display-to-box generation contract for Milestone W issue 3

This document defines how Borrowser maps computed `display` values into
generated layout boxes for the current supported subset.

Related code:
- `crates/layout/src/box_tree.rs`
- `crates/layout/src/lib.rs`
- `crates/css/src/values.rs`
- `crates/css/src/computed/style.rs`

Related documents:
- `docs/rendering/w1-box-tree-layout-model-contract.md`
- `docs/rendering/w2-structured-box-tree-data-structures.md`
- `docs/rendering/w4-anonymous-box-generation-supported-subset.md`
- `docs/rendering/v1-rendering-architecture-ownership-phase-contracts.md`
- `docs/rendering/v2-rendering-pipeline-phase-output-models.md`
- `docs/rendering/v6-deterministic-debug-surfaces-and-phase-regression-coverage.md`
- `docs/rendering/v7-rendering-pipeline-invariants-and-extension-hooks.md`

## Purpose

W2 introduced `BoxTree` as the generated layout-owned structure. W3 makes the
first step of box generation explicit:

```text
StyledNode + computed display + source role
  -> DisplayBoxGeneration decision
  -> principal BoxNode or subtree suppression
```

This prevents supported display behavior from being inferred implicitly by
later geometry code.

## Supported Display Behavior

The current supported computed `display` values are:

- `block`
- `inline`
- `inline-block`
- `list-item`
- `none`

The current principal-box mapping is:

| source / computed display | generation behavior | generated `BoxKind` |
| --- | --- | --- |
| document root | principal document-root box | `Block` |
| document element | principal document-element box | `Block` |
| text run | principal text-run box | `Block` in the current geometry bridge |
| ordinary element, `display: block` | principal block box | `Block` |
| ordinary element, `display: inline` | principal inline box | `Inline` |
| ordinary element, `display: inline-block` | principal inline-block box | `InlineBlock` |
| ordinary element, `display: list-item` | principal list-item block box | `Block` plus list marker metadata when applicable |
| replaced ordinary element, inline-level display | principal replaced inline box | `ReplacedInline` |
| ordinary element, `display: none` | no box, suppress subtree | none |
| comment node | no box | none |

`DisplayBoxBehavior` records the semantic display-driven behavior separately
from `BoxKind`, because multiple behaviors can currently project into the same
geometry kind. For example, `display: block`, `display: list-item`, document
root, document element, and text runs all project to `BoxKind::Block` today,
but they are not the same box-generation behavior.

## Principal Box Contract

Every generated DOM-backed `BoxNode` has one principal box decision represented
by `PrincipalBox` metadata:

- `BoxGenerationRole`
- `BoxKind`
- `DisplayBoxBehavior`

Principal-box generation is deterministic for a fixed styled tree and layout
environment. `display: none` and comments produce `SuppressSubtree` decisions
instead of principal boxes.

## Unsupported And Deferred Display Values

Unsupported CSS display keywords are not represented as deferred layout modes
inside `BoxTree`. They are rejected or ignored before computed style exposes a
`Display` value to layout.

The layout contract is intentionally exhaustive over `css::Display`. Adding a
new `Display` variant must update `DisplayBoxGeneration`,
`DisplayBoxBehavior`, tests, and this document. Layout must not silently route
new display modes through the current block fallback.

Deferred future display modes include:

- flex
- grid
- flow-root
- table-internal display values
- ruby
- contents
- multi-keyword outer/inner display decomposition

Those must be introduced as explicit box-generation behavior, not as ad hoc
fallbacks in geometry code.

## Regression Surface

The layout crate tests cover:

- supported `block`, `inline`, `inline-block`, `list-item`, and `none`
  mappings
- replaced inline-level box generation
- principal-box behavior metadata
- comment suppression
- context-based document-element role assignment
- unsupported display keyword fallback before layout
- deterministic `BoxTree::to_debug_snapshot()` output including
  display-behavior labels
