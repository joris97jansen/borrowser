# AE10: Current Select Tree-Construction Contract

Last updated: 2026-07-14
Scope: `crates/html/src/html5/tree_builder` and curated parser conformance evidence

## Normative acceptance sources

AE10 is accepted against immutable WHATWG HTML repository revision
`88ae68cb961651f0f92c5d2046049f53ecdfc6cf`, snapshot date **2026-07-11**.
The normative source is that revision's `source`, specifically the stack of
open elements, scope, implied-end-tag, appropriate-place-for-inserting-a-node,
and `in body`/table-family token-processing algorithms.

Curated upstream DOM-tree evidence is adapted from Web Platform Tests revision
`2c705104a295c48053eeddf7fe0170d790a4e853`. Each adapted `.dat` case records
its exact source path, input bytes, errors, expected tree, parsing context, and
representational adaptation. Upstream error strings are provenance only;
Borrowser owns a separate deterministic parse-error taxonomy.

The live specification and later WPT revisions are drift-detection aids only.
They do not change AE10 acceptance.

## Ownership and insertion-mode stance

HTML tree construction owns parser-created `select`, `option`, and `optgroup`
DOM shape, stack recovery, scope checks, implied-end-tag generation, adjusted
insertion locations, parser errors, and patch ordering. Browser/runtime does not
repair these trees and owns any future selectedness, values, interaction, form,
accessibility, layout, or paint behavior.

Current full-document select handling runs through `InBody` and the existing
table-family delegation architecture. AE10 introduces no `InSelect`,
`InSelectInTable`, remembered return mode, select flag, recursive redispatch,
or post-parse repair state.

## Shared HTML special category

AAA furthest-block discovery and the InBody "any other end tag" algorithm use
one shared allocation-free HTML-namespace classifier derived from the pinned
83-name special-element set. The pre-AE10 private AAA table was missing
`keygen`, `noscript`, and `object`; AE10 corrects all three. `option`,
`optgroup`, ordinary inline names, and custom names are not special.

This classification is HTML-only. MathML and SVG special-category membership
requires namespace-aware stack entries and remains unsupported.

The production source is `tree_builder/html_semantics.rs`. It resolves an
`AtomId` through the borrowed atom-table path and performs allocation-free
binary search over one strictly sorted table. An independent test-only 83-name
oracle checks exact equality, sorting, uniqueness, length, all positives, the
three corrected entries, and representative negatives. The pinned audit found
exactly these former omissions: `keygen`, `noscript`, and `object`; it found no
extra entries.

## Scope versus special barriers

`select` is a boundary in the shared general in-scope algorithm. Button and
list-item scope inherit that boundary; table scope remains unchanged.

Scope and the special category are distinct semantics. Dedicated select
start/end algorithms use general scope. Generic option/optgroup end tags use a
single reverse stack scan: the first matching target succeeds, while the first
special HTML element blocks the token. There is no scope precheck or second
target scan.

The supported full-document parser keeps the HTML root on the stack while this
generic algorithm is callable. Because HTML `html` is special, an absent target
normally returns a special barrier. Exhausting the stack is an
`EngineInvariantError`, not malformed-input recovery.

The scan result is explicit:

```rust
enum InBodyEndTagScan {
    Matched(OpenElementMatch),
    BlockedBySpecial { index: usize, element: OpenElement },
}
```

`scan_in_body_any_other_end_tag` is probe-only. A match carries stable index
and identity into exact suffix removal after implied-end generation, avoiding a
scope precheck and second target scan. Only the successful generic algorithm
invalidates text coalescing; it does not perform generic AFE cleanup. Dedicated
`soe_end_tag_scan_calls`/`soe_end_tag_scan_steps` metrics remain distinct from
scope metrics.

## Supported implied end tags

The supported subset is `p`, `li`, `option`, and `optgroup`. The shared helper
accepts an exception target. AE10 does not claim support for other implied-end
members.

## Select-family and related InBody behavior

- A nested select start records an error, pops through the existing in-scope
  select, ignores the conflicting insertion, and inserts no replacement.
- An ordinary select start reconstructs active formatting, performs semantic
  normal insertion, and makes frameset-not-ok without changing mode.
- A select end requires general scope, generates supported implied ends,
  diagnoses a non-current target, and pops through the select.
- Option/optgroup starts use the shared implied-end helper and ordinary InBody
  reconstruction/insertion. Their end tags use the generic reverse scan.
- Input first closes an in-scope select, then follows shared input processing.
- HR retains paragraph recovery, adds select-aware implied closure, and uses
  semantic void insertion.
- Self-closing-flag finalization remains dispatch-owned and occurs exactly once.

The dispatch disposition is `Acknowledge` for semantic void `input`/`hr` and
`LeaveUnacknowledged` for normal `select`/`option`/`optgroup`, including ignored
nested-select and insertion-rejected outcomes. Direct-InTable hidden input
keeps its dedicated route but the same single dispatch finalization boundary.

## Table and foster-parenting relationship

Table-family modes delegate through existing bounded same-token processing.
Foster placement applies only when enabled and the current target is `table`,
`tbody`, `tfoot`, `thead`, or `tr`. The final location is selected before node
creation and patch emission; append-then-move is forbidden.

Thus `<table><select><option>3</select></table>` inserts select before table via
`InsertBefore`, while option is inserted under the current select.

Direct-InTable non-hidden input delegates with foster parenting and then uses
the select-aware InBody prelude. Direct-InTable hidden input remains distinct.
`InCell` delegates ordinary tokens to InBody. All reprocessing preserves the
same logical token and the bounded iterative dispatch guard.

The corrected generic special barrier also requires nested-table close to
restore the supported insertion mode from SOE rather than unconditionally
entering InBody. This bounded reset recognizes the existing supported table,
head, and body contexts and introduces no remembered return state or select
mode.

## Resource-limit recovery

Spec-required stack recovery remains committed if later semantic insertion is
rejected by a parser resource limit. Stack name counts, foster caches, counters,
text coalescing, high-water metrics, parser-error ordering, and panic freedom
must remain consistent. Stack-only recovery never emits `RemoveNode`.

## Required acceptance evidence

Acceptance requires focused scope, implied-end, special-category, generic-end,
select-family, input, HR, AAA, table, limit, state, and performance tests;
deterministic DOM/patch/error fixtures; whole/chunked parity; exact WPT case
adaptations; materialization checks; and bounded fuzz corpus/smoke evidence.

The fixed `wpt_html5_select` suite validates these exact full-document `.dat`
adaptations from WPT commit `2c705104a295c48053eeddf7fe0170d790a4e853`:

| ID | Source | SHA-256 of exact `#data` bytes |
| --- | --- | --- |
| `select-nested-formatting` | `html/syntax/parsing/resources/tests1.dat` | `3bdc2731c4f57fb934769c10bae07732df31ac89b8f90aadb9161ca9cc2b16d7` |
| `select-input-recovery` | `html/syntax/parsing/resources/tests7.dat` | `44c9d0ea49afb6e5b42d61af6bc8dcd77e168c46b5dff45aab2f701894c42226` |
| `select-nested-simple` | `html/syntax/parsing/resources/tests7.dat` | `81fa58555ea62a7dc493cfaab2931ddc191fd8192a8eae46e93bef04d9ceafd3` |
| `select-table-foster-option` | `html/syntax/parsing/resources/tables01.dat` | `2540a8c59f9046301e1483f5458a7a44b96a72a1bd16947d9451142c4713994e` |
| `select-table-token-open` | `html/syntax/parsing/resources/tables01.dat` | `74bbf12f95de0fb0a8fb12da0c07949a5f5c6e223f7743dba7b34ba68cf5c428` |
| `select-table-row-recovery` | `html/syntax/parsing/resources/tables01.dat` | `9a056bd114975bc358f15fca4e2c54457ef1742b3d507216cad6f5f57817fd77` |

Each input has no added terminal newline. Its provenance sidecar retains the
complete upstream `#errors` and `#document` sections. The runner recomputes the
SHA-256 and proves the committed `html5-dom-v1` expectation is only the
approved representation translation of the upstream tree. Upstream error
strings remain provenance; local fixtures own Borrowser error ordering.

Local fixtures carry the explicit label “Local WHATWG-derived fixture; not an
upstream WPT or html5lib-tests import.” They cover diagnostics, special
barriers, ordinary descendants, patches, limits, state parity, counters, and
fuzz replay.

Synthetic tree-builder fuzz inputs are an explicitly versioned deterministic
byte format. Unmarked inputs use decoder V1, whose exact pre-AE10 30-name tag
catalog and modulo mapping are frozen so legacy corpus bytes retain their
meaning. The exact `TB-FUZZ-V2\n` metadata prefix selects decoder V2 and is
consumed before token decoding; truncated or unknown marker-like prefixes are
ordinary V1 token bytes. V2 preserves V1's catalog prefix and appends
`select`, `option`, `optgroup`, `input`, and `hr`. The AE10
regression at the exact path
`fuzz/regressions/html5_tree_builder_tokens/select-special-barrier` explicitly
uses V2; it is the only committed V2 input, and every other committed
tree-builder token corpus/regression input remains V1.

Version framing is raw input before it is decoder metadata. The V2 prefix
counts toward `max_input_bytes`, is included in
`TreeBuilderFuzzSummary::input_bytes`, and participates in raw-input fuzz-seed
derivation. This keeps the raw byte bound strict and domain-separates V1 and V2
input identities. Once recognized, the prefix is removed before synthetic
token decoding and contributes no generated token, generated attribute,
generated string byte, decoded-token processing step, or emitted synthetic
token.

Decoder versioning is distinct from the global `FuzzDigest` schema. AE10 does
not change `fuzz/digest.rs` or its compatibility schema. Comprehensive digest
expansion remains the separate AE11 follow-up.

The local DOM/error golden inventory is `ae10-basic-options`,
`ae10-optgroup-transitions`, `ae10-generic-end-special-barrier`,
`ae10-corrected-special-barriers`, `ae10-unmatched-select-family-ends`,
`ae10-customizable-descendants`, `ae10-input-recovery`, and
`ae10-option-outside-select`. The local patch inventory is
`ae10-fostered-select`, `ae10-nested-select-parent`, `ae10-input-parent`,
`ae10-implied-option-parent`, and `ae10-table-stack-clearing`. The golden
loaders require the local provenance label for every `ae10-*` fixture.

## Implementation boundaries and follow-ups

AE10 uses semantic normal/void insertion only for handlers it directly owns or
changes. The deprecated `insert_element` helper and unrelated call sites remain
for AE9b; AE10 neither completes nor blocks on that repository-wide migration.

AE11 — Complete deterministic tree-builder fuzz digest coverage — will encode
all future-affecting parser state with stable explicit representations,
version or deliberately migrate expectations, and add mutation-sensitivity
tests without introducing feature-specific remembered state.

## Deliberate exclusions

AE10 excludes fragment parsing, template modes, foreign content, historical
select modes/filtering, selectedness, values, disabled propagation, option
collections, popup/listbox behavior, form submission/reset, validation, focus,
input interaction, events, accessibility semantics, layout, paint, JavaScript,
full DOM APIs, repository-wide AE9b migration, and the AE11 global fuzz-digest
expansion.
