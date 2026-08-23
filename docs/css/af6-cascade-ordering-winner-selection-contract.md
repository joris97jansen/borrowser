# AF6: Cascade Ordering And Winner Selection Contract

Status: implemented living contract for Milestone AF issue 6

Last updated: 2026-08-22

AF6 reconciles the Milestone R cascade implementation with AF3 specificity,
AF5 rule collection and semantic source order, AD4's longhand registry, AD6
shorthand expansion, and Borrowser's supported subset of CSS Cascading and
Inheritance Level 5. CSS Cascade Level 5 is normative for the modeled cascade.
Level 6 scope proximity is only an extension-boundary constraint here.

The supported production pipeline is:

```text
matched AF5 rule inputs and inline declarations
  -> supported longhand candidate admission
  -> explicit supported cascade priority
  -> invariant-safe property-indexed winner selection
  -> sparse cascaded winner set
```

Inheritance, initial/default fill, post-win CSS-wide keyword interpretation,
and computed-value materialization remain downstream of AF6.

## Ownership and related contracts

CSS owns declaration admission, priority construction, winner selection,
cascade invariants, cascade diagnostics, and cascade failures. Browser owns
stylesheet discovery, orchestration, and retained artifact lifetime. Computed
style consumes resolved CSS output. Layout and Paint consume computed output;
they do not receive declarations, specificity, or cascade priority.

AF6 builds on:

- AF1 for selector/cascade/computed-style ownership;
- AF3 for typed selector specificity;
- AF5 for collected rule uniqueness, matched-rule traversal, and
  `StylesheetRuleOrder`;
- AD3 for supported CSS-wide keywords and post-win interpretation;
- AD4 for the longhand registry, canonical property order, and winner-slot
  capacity;
- AD6 for atomic shorthand expansion and authored-source preservation.

The relevant implementation is rooted in `crates/css/src/cascade.rs` and
`crates/css/src/cascade/contract.rs`; these remain flat module roots.

## Supported priority

`CascadePriority` is a complete priority for the current emitted subset. Its
comparison is lexicographic:

1. origin and importance band;
2. element-attached versus style-rule declaration precedence;
3. selector specificity for style rules;
4. order of appearance: AF5 stylesheet/style-rule order followed by authored
   declaration order, or authored inline declaration order.

The emitted origin/importance order, from lower to higher precedence, is:

```text
UA normal
user normal
author normal
author important
user important
UA important
```

`CascadeOriginBand` reserves animation and transition positions for inspection
and future integration, but no current constructor can emit either band.

`CascadeDeclarationPrecedence` represents the two currently meaningful
declaration shapes:

```text
StyleRule {
    specificity,
    source_order: StylesheetRuleOrder,
    declaration_order,
}

ElementAttached {
    declaration_order,
}
```

Element-attached declarations are author declarations. Their checked priority
constructor admits only author-normal or author-important priority. Inline
style does not win through synthetic specificity, a maximum numeric sentinel,
source identity, or a source-order variant. `CascadeSpecificity::InlineStyle`
and `CascadeSourceOrder::InlineStyle` are removed. Specificity and stylesheet
source order are not applicable to element-attached declarations.

No placeholder layer, scope, or encapsulation fields are present. Those steps
must be added as real typed semantics when supported, at their specification
positions, without reinterpreting attachment or source order.

## Source identity, order, and collision proof

Candidate identity is:

```text
(CascadeDeclarationSource, PropertyId)
```

One shorthand source may emit different registered longhands. The same source
may not emit the same property twice. Exact duplicate emission is an engine
error even when priority and value agree. Reuse with a changed priority or
changed semantic candidate data is a distinct typed error; neither state is
deduplicated.

AF5 and AF6 make complete-priority equality between distinct valid production
candidates impossible:

- `StylesheetOrder` is unique across collected stylesheet inputs;
- `StyleRulePosition` is unique within one stylesheet;
- `DeclarationOrder` is unique within a rule or inline declaration list;
- one collected rule is visited at most once for an element;
- at most one effective inline declaration list is appended;
- candidate source/property identity is unique;
- style-rule and element-attached priority have distinct typed shapes.

Therefore production winner evaluation does not hash every candidate. The
opaque matched-input builder establishes rule traversal and inline-list
invariants in pass one. The production evaluator uses only property-indexed
winner slots. The checked crate-private/test compatibility boundary performs
the additional whole-input identity and complete-priority validation needed to
diagnose malformed historical fixtures.

The typed invariant failures are:

- `DuplicateCandidateIdentity`;
- `InconsistentCandidateIdentity`, with priority/value/expansion-metadata
  mismatch detail;
- `EqualPriorityDistinctCandidates`;
- `RuleInputSequenceInvariant` for malformed rule-input structure.

Candidate failures retain the property, both sources, and both complete
priorities. Source metadata is part of candidate identity and therefore is not
an independent mismatch state. Failures do not clone full declaration values
merely to report an error. Stable diagnostic labels are explicit and are not
derived from Rust variant names.

Incoming vector order, iterator order, stable sorting, parser storage order not
present in the key, and test-construction order are not hidden precedence
dimensions.

## Exactly two declaration traversals

Ordinary per-element evaluation traverses already-classified declaration
inputs exactly twice:

1. `ValidatedCascadeRuleInputBuilder` constructs the opaque input view,
   enforces rule/list and candidate-identity invariants, counts admitted
   supported candidates exactly, and enforces the checked candidate ceiling.
2. `resolve_cascade_winners_from_validated_inputs` constructs borrowed
   candidate views, compares complete priorities, updates property slots,
   notifies an optional observer, and materializes sparse winners.

These passes do not repeat selector matching, property lookup, value parsing,
shorthand expansion, or declaration classification. Filtered declarations are
observed in pass one but do not consume candidate workspace or candidate
diagnostic records.

## Candidate admission

AD/AF5 classification remains authoritative:

| declaration state | AF6 result |
| --- | --- |
| valid supported longhand | candidate |
| valid supported expanded longhand | candidate |
| valid supported declaration outranked later | losing candidate |
| invalid supported value | diagnostic non-candidate |
| atomically invalid shorthand | no longhands and no candidates |
| unsupported property | diagnostic non-candidate |
| custom property while deferred | diagnostic non-candidate |
| invalid property name | diagnostic non-candidate |

Invalid and unsupported declarations are not assigned artificial low
priority. AD6 expansion preserves the authored source, importance, and
declaration order. `expansion_order` remains deterministic presentation
metadata and never participates in comparison.

## Budget, workspace, and algorithm

The crate-private `CascadeResolutionBudget` is constructed once by
`StyleResolutionExecution::try_new`. Its candidate ceiling uses checked
arithmetic:

```text
max_declaration_inputs_per_element
  + max_inline_declarations_per_element
```

The budget also validates internal locator representability and derives winner
capacity from `property_registry().entries().len()`. Diagnostic record IDs and
diagnostic byte limits are deliberately not production budget concerns.

One `CascadeResolutionWorkspace` is created before a full-document element
loop, one before an incremental suffix loop, and one for a bounded diagnostic
traversal. It is local to that style execution or future worker. Clearing it
sets every winner slot to empty without releasing capacity. It contains
lifetime-independent typed locators and never retains references into a prior
element's inline declarations. There is no global or cross-document mutable
workspace.

The selected algorithm is a property-indexed accumulator:

- time: `O(D + P)`, where `D` is classified declarations traversed and `P` is
  registered properties scanned in canonical order;
- transient winner workspace: `O(P)`;
- candidate values: borrowed;
- retained value cloning: once, only for each final sparse winner;
- output: sparse and canonical in AD4 registry order.

The previous sorted-vector resolver required a candidate vector, another
sorted reference vector, `O(C log C)` comparison, and stable input order for
exact-key ties. It is historically superseded and should not return as the
production winner engine. Sorting remains valid for deterministic diagnostic
presentation only. A candidate-wide identity/priority `HashMap` is also
rejected for the production hot path: opaque AF5/AF6 construction makes the
malformed states impossible. Such validation storage is appropriate only in
the narrow checked compatibility/test builder.

Top-level AF6 vectors use fallible reservation for matched rule inputs,
winner workspace, winner output, diagnostic records, and serialized diagnostic
output. Bounded diagnostic strings are measured before allocation and use
fallible exact reservation. AF6 does not claim recovery from every CSS model
allocation or every process-level allocator failure outside these explicit
bounded containers.

## Sparse winner projection

`CascadeWinnerSet` is the current sparse cascaded-value projection. It holds
one winner per property because, in today's supported subset:

- layers do not participate;
- `revert` and `revert-layer` are rejected before candidacy;
- animations and transitions emit no declarations;
- scope proximity is absent;
- there is one implicit encapsulation context.

This is not a claim that a future full cascade can permanently discard all
lower-precedence candidates. Rollback, layering, animations, transitions,
scope, and encapsulation may require richer stacks or phase-specific retained
state. Candidate provenance and ordering remain inspectable without selector
rematching, declaration reparsing, or declaration reclassification.

Inheritance, initial/default fill, CSS-wide keyword interpretation after a
winner is known, and computed style are not winner-selection semantics. The
production resolved-style path consumes the sparse winner set so winner values
are not cloned a second time.

## Errors and propagation

`CascadeResolutionError` owns checked-budget, reservation, sequence, identity,
and priority-collision failures. Authoritative propagation is:

```text
CascadeResolutionError
  -> StyleResolutionError::CascadeResolution
  -> ComputedStyleResolutionError::StyleResolution
  -> Browser/Page caller or CSS-owned diagnostic failure
  -> deterministic fuzz invariant classification
```

Incremental cascade failure is not selector no-match, an empty/default style,
`IncrementalUnavailable`, or permission for silent full fallback. A bounded
diagnostic cannot report `Complete` after cascade failure.

Declaration-source ordering failures are property-independent. A malformed
non-candidate declaration reports `DeclarationSourceOrderInvariant` with its
rule source, previous/current declaration sources, and previous/current
orders; AF6 never fabricates a registered property for an unrelated sequence
error. Stable error labels remain concise, while maintained `Display` text
writes limits, coordinates, sources, priorities, properties, and violation
labels explicitly without using Rust-derived `Debug` grammar.

## Bounded candidate/winner diagnostic

`cascade_evaluation_diagnostic` is the production-triage AF6 surface. Schema
version 1 includes:

- checked typed `CascadeDiagnosticCandidateId` values;
- typed element, property, and declaration-source identities;
- every active priority dimension;
- deterministic candidate presentation order;
- final winner-to-candidate references assigned after sorting;
- candidate and winner record limits;
- retained-storage and serialized-byte limits;
- bounded source, property, and value text;
- typed style-execution, configuration, limit, reservation, and ID failures.

Diagnostic limits are checked by `CascadeEvaluationDiagnosticLimits`, not by
the crate-private production cascade budget. `max_retained_bytes` bounds every
diagnostic-owned live heap allocation throughout construction and retention:
candidate and winner `Vec` capacities, bounded-text `String` capacities,
provisional-ID remapping, winner marking, and the retained serialized `String`
capacity. All totals use checked arithmetic. Every growth site preflights the
requested capacity against that total and maps reservation failure into the
diagnostic-local error domain. Candidate and winner record vectors use an
explicit amortized policy owned by CSS: the first non-zero target is eight
records, subsequent targets double with checked arithmetic, and each target is
clamped by its record-count limit and the capacity permitted by
`max_retained_bytes`. A geometric-overflow condition is a typed diagnostic
failure rather than permission to fall back to repeated exact growth.

Candidate reservation uses the exact admitted-candidate count known at the
start of each element. Winner reservation uses the safe per-element upper hint
`min(admitted candidates, registered properties)`, while requiring only the
first winner that is guaranteed when candidates exist; record and byte limits
may clamp the hint without misreporting unobserved winners. Actual winner
callbacks use the same amortized helper if they later cross retained capacity.
CSS chooses every target before calling `Vec::try_reserve`; it does not rely on
the allocator's growth strategy for amortization. After reservation, the
actual allocator-provided capacity is re-accounted and the diagnostic fails
honestly if that capacity exceeds the live-heap byte budget. Candidate bounded
text, candidate-ID remapping, winner marking, and retained serialization keep
fallible exact reservation because their complete sizes are known.

The observer retains one candidate-record vector. Candidates receive checked
provisional observation identities from the shared evaluator, the vector is
sorted in place by a total presentation key, and a fallibly reserved
old-identity-to-final-ID vector remaps winners directly. A bounded byte marking
vector marks final winners. These two scratch vectors are live-byte-accounted
and dropped before serialized output is allocated. Finalization is
`O(C log C)` for presentation sorting plus `O(C + W)` indexed remapping and
marking; it never scans all candidates for every winner or all winners for
every candidate.

The complete snapshot retains the already measured, bounded serialized
artifact. `serialized()` borrows it and `to_debug_snapshot()` only clones that
artifact; complete diagnostics are never reserialized through an unbounded
writer. `serialized_bytes` is exactly the retained artifact length. Diagnostic
value text uses an explicit quoted grammar: double quote, backslash, LF, CR,
and TAB become `\"`, `\\`, `\n`, `\r`, and `\t`; other C0 controls and DEL use lowercase
`\u{hex}`; other Unicode scalars remain literal. Rust `Debug` grammar is not a
diagnostic format.

The shared evaluator has separate cascade and
observer error channels. Production exposes only cascade errors; diagnostics
map observer/storage failures only to diagnostic-local failures. The final
winner callback runs after all candidates for that property have been
compared. There is no second diagnostic winner engine and one cascade failure
has one stable failed-diagnostic representation.

R8 exact-string snapshots remain crate-private/test-only regression fixtures.
The arbitrary-rule-input helper is compiled only for unit tests and returns a
typed cascade result. The integrated document trace consumes its existing
`ValidatedCascadeRuleInputs`, execution budget, and reusable workspace without
cloning or revalidating the rule-input vector and without a third declaration
traversal. These fixtures delegate admission
and winner selection to the shared evaluator, sort references only for
presentation, and no longer use stable sort as cascade semantics. AF6 advances
the R8 cascade/winner/resolved schemas to version 3, the declaration pipeline
to version 3, and the integrated document trace to version 4.

## Unsupported cascade features

The current deterministic policy is:

| feature | current policy |
| --- | --- |
| cascade layers / `@layer` | explicit `LayerDeferred`; nested content skipped, never flattened |
| CSS `@scope` | explicit `ScopeDeferred`; nested content skipped, never flattened |
| historical HTML `<style scoped>` | no scoped-style semantics; distinct from CSS `@scope` |
| `revert`, `revert-layer` | recognized by AD3, rejected before candidacy |
| animations, transitions | reserved ordering positions; emit no declarations |
| encapsulation contexts / Shadow DOM | one implicit context; no Shadow DOM cascade |
| author presentational hints | not emitted into the AF6 structured cascade |
| runtime user stylesheets | user origin modeled and testable; no Browser user-sheet manager |

AF6 does not parse or order layers, implement scoped matching or proximity,
approximate rollback, or add any of the other deferred systems.

Browser regression coverage proves that `<style scoped>` is collected as an
ordinary global author stylesheet: the attribute is ignored, it creates no
CSS `@scope` record or proximity step, and its rules match elements inside and
outside the containing element normally.

## Future-work records

These are future milestones, not AF6 sub-issues.

### Cascade Layers and Rollback Semantics

- Existing or new: new milestone.
- Milestone title: **Cascade Layers and Rollback Semantics**.
- Milestone description: add CSS Cascade Level 5 layer order, layer-aware
  origins, and the retained lower-precedence information required for correct
  `revert` and `revert-layer`, without moving cascade semantics outside CSS.
- Issue title: **Implement layer-aware cascade stacks and rollback values**.
- Issue description: parse and collect supported `@layer` forms, insert layer
  order at the normative priority position, retain the candidate history
  needed by rollback, resolve `revert`/`revert-layer`, and add bounded
  diagnostics and conformance tests.

### Scoped Cascade and Level 6 Proximity

- Existing or new: new milestone.
- Milestone title: **Scoped Cascade and Level 6 Proximity**.
- Milestone description: implement CSS Scoping and the CSS Cascade Level 6
  scope-proximity ordering step over parser-created DOM, separately from the
  historical HTML `scoped` attribute.
- Issue title: **Implement `@scope` matching and scope-proximity ordering**.
- Issue description: parse supported scope roots/limits, match scoped rules,
  insert proximity at its normative cascade position, retain scope provenance,
  and add invalidation and bounded diagnostic coverage.

### Shadow DOM and Encapsulation Cascade

- Existing or new: new milestone.
- Milestone title: **Shadow DOM and Encapsulation Cascade**.
- Milestone description: introduce Shadow DOM tree scopes and encapsulation
  context ordering while preserving CSS/DOM/Browser ownership boundaries.
- Issue title: **Model encapsulation contexts in cascade priority**.
- Issue description: define tree-scope identities and ordering, integrate
  shadow-origin declarations and selector boundaries, and test normal and
  important context reversal without exposing cascade semantics to Browser.

### CSS Diagnostic Contract Consolidation

- Existing or new: new milestone.
- Milestone title: **CSS Diagnostic Contract Consolidation**.
- Milestone description: consolidate selector, rule-collection, cascade,
  resolved-style, and computed-style diagnostics into a coherent bounded,
  versioned family while keeping subsystem-specific payloads and limits.
- Issue title: **Unify bounded CSS diagnostic envelopes and serialization**.
- Issue description: define shared schema/version and terminal-failure
  conventions, reusable bounded text/storage primitives, cross-phase identity
  references, and parity tests without merging semantic error domains or
  creating duplicate evaluators.

The rejected production candidate-wide hash table and magic inline
specificity/source-order sentinels should never be implemented. The historical
stable-sort exact-tie fallback should never be restored.

## Deliberate exclusions

AF6 does not implement inheritance, default fill, computed-value construction,
layers, rollback keywords, CSS scope, scope proximity, Shadow DOM,
encapsulation ordering, presentational hints, runtime user stylesheet
management, custom properties, animations, transitions, CSSOM, JavaScript
style APIs, broad properties, selectors, or media queries.
