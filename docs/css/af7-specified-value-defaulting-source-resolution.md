# AF7: Specified-Value Defaulting Source Resolution

Status: implemented living contract for Milestone AF issue 7

Last updated: 2026-08-24

AF7 defines the CSS-owned boundary between AF6's sparse cascade winners and
AD5 computed-style materialization. It applies inheritance/defaulting and the
supported CSS-wide keywords to every AD4-registered longhand.

## Related code

- `crates/css/src/cascade/contract/resolved_style.rs`
- `crates/css/src/cascade/integration.rs`
- `crates/css/src/cascade/integration/debug_snapshot.rs`
- `crates/css/src/computed/document/materialize.rs`

## Related contracts

- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af6-cascade-ordering-winner-selection-contract.md`
- `docs/css/ad3-css-wide-keyword-handling.md`
- `docs/css/ad4-css-property-registry-longhand-metadata.md`
- `docs/css/ad5-specified-computed-value-boundaries.md`
- `docs/css/ad6-shorthand-expansion-foundation.md`
- `docs/css/r5-inheritance-behavior-supported-properties.md`
- `docs/css/r6-initial-default-value-handling.md`
- `docs/css/r7-structured-resolved-style-output.md`
- `docs/css/r8-cascade-style-resolution-debug-output.md`
- `docs/css/r9-cascade-invariants-supported-property-behavior-computed-style-handoff.md`

## Artifact boundary

`ResolvedStyle` is Borrowser's total specified-value/defaulting
source-resolution artifact. `ResolvedDocumentStyle` stores one such artifact
per styled selector-DOM element in deterministic document order.

They are not:

- a second fully materialized specified-value tree;
- computed styles;
- CSSOM resolved values;
- layout or paint inputs.

For every property in the AD4 registry, `ResolvedStyle` records exactly one of:

- a local AF6 winner;
- ordinary inherited defaulting;
- ordinary initial fallback;
- a supported CSS-wide keyword plus its resolved initial/inherited behavior
  and winning declaration provenance.

`ResolvedValueSource::Inherited` is intentionally symbolic. It does not carry
or copy a parent winner, parsed declaration, specified value, or resolved
source.

## Authoritative classifier inputs

The AF7 per-property classifier depends only on:

1. AD4 property metadata;
2. zero or one local AF6 winner;
3. `InheritanceParentPresence`, a typed `Absent`/`Present` immediate-parent
   topology fact.

The classifier cannot receive a parent `ResolvedStyle`. The public historical
`resolve_cascade_style(..., Option<&ResolvedStyle>)` entry point is a
compatibility adapter that converts `None`/`Some` to typed parent presence
before entering the authoritative path; parent contents cannot affect output.

Borrowed test/compatibility winners and owned production winners enter one
semantic classifier. The production path moves retained winners into the
result. Only the borrowed adapter clones a winner when an owned resolved result
must retain it.

## Defaulting and CSS-wide behavior

After AF6 winner selection, AF7 resolves each registered property as follows:

| local result | metadata and parent state | AF7 source |
| --- | --- | --- |
| ordinary winner | any | `Winner` |
| missing | inherited, parent present | ordinary `Inherited` |
| missing | inherited, no parent | ordinary `Initial` |
| missing | non-inherited | ordinary `Initial` |
| `initial` winner | any | CSS-wide initial |
| `inherit` winner | parent present | CSS-wide inherited |
| `inherit` winner | no parent | CSS-wide initial, retaining `inherit` provenance |
| `unset` winner | inherited, parent present | CSS-wide inherited, retaining `unset` provenance |
| `unset` winner | non-inherited | CSS-wide initial, retaining `unset` provenance |
| `unset` winner | inherited, no parent | CSS-wide initial, retaining `unset` provenance |

Inheritance classification and initial values come exclusively from AD4
metadata. No AF7 call site may duplicate property-specific defaulting tables.

`revert` and `revert-layer` remain recognized-but-unsupported under AD3. The
specified/declaration boundary rejects them, including atomic shorthand
rejection, so they are not AF6 candidates and cannot normally reach AF7. AF7
does not approximate them. Its internal unreachable guard protects this
upstream invariant rather than defining behavior.

AD6 expands supported shorthands before AF6. A supported CSS-wide shorthand
such as `outline: inherit` reaches AF7 only as ordinary longhand winners; AF7
has no shorthand-specific semantics.

## Computed inheritance

CSS inheritance transfers the immediate parent's computed value. During
top-down computed-style materialization:

- `Winner` normalizes its property-aware specified value;
- `Initial` materializes the AD4 initial value;
- ordinary or CSS-wide inherited sources read the same property's value from
  the immediate parent `ComputedStyle`.

The computed layer receives the symbolic AF7 source and parent computed style.
It does not receive a copied parent declaration through AF7. Parent-before-child
document ordering remains required at this materialization boundary.

## Document and subtree semantics

Full document, incremental suffix, and integrated diagnostic resolution derive
parent presence from `SelectorMatchingContext::parent_element(...)`. They do
not retain resolved styles merely to prove that a DOM parent exists.

Incremental prefix reuse continues to validate selector element identity,
namespace, and local name. Required retained prefix entries are cloned into the
new `ResolvedDocumentStyle`; this is artifact construction, not inheritance
lookup.

The element-subtree compatibility path is a closed projection:

- its projection root has `InheritanceParentPresence::Absent`;
- descendants with parents inside the projection use `Present`.

It is not attached-subtree recomputation and has no external parent style
context. Adding such recomputation requires a separate contract.

## Diagnostics

Resolved-style diagnostics preserve distinct labels for:

- a winning declaration;
- ordinary inheritance;
- ordinary initial fallback;
- CSS-wide initial resolution with keyword and winner provenance;
- CSS-wide inherited resolution with keyword and winner provenance.

The integrated document diagnostic additionally derives the immediate parent
selector identity from selector-DOM context. It does not retain parent IDs per
property and does not compute an effective inherited value. The integrated
grammar is version 5 because the `inheritance-parent` line is new; lower
resolved-style grammars are unchanged.

## Invariants

- Every AD4-registered property is resolved exactly once in registry order.
- AF6 winners remain sparse; AF7 output is total.
- Parent resolved/specified contents cannot influence AF7 classification.
- Ordinary inherited defaulting applies only to inherited properties.
- Explicit `inherit` can request inheritance for a non-inherited property.
- Root/no-parent inheritance falls back to the metadata initial value.
- Parent computed values are consumed only during computed materialization.
- Full and eligible suffix-incremental resolution produce equivalent retained
  artifacts for the same source identities and DOM/style inputs.
- Layout, Paint, and Browser/runtime do not own inheritance or property
  defaulting semantics.

## Deliberate exclusions

AF7 does not add:

- `revert` or `revert-layer` support;
- a materialized specified-style tree;
- attached-subtree recomputation;
- new selector matching or invalidation eligibility;
- computed-style redesign;
- Layout, Paint, or Browser/runtime orchestration changes;
- custom properties, animations, transitions, CSSOM, or broad property
  coverage.
