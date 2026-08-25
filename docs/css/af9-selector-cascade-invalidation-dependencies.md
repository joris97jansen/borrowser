# AF9: Selector/Cascade Invalidation Dependencies

Status: implemented contract for Milestone AF issue 9

Last updated: 2026-08-25

AF9 adds an owned CSS dependency artifact and uses it to classify neutral DOM
publication transitions before retained Style execution. It supersedes AF4e's
temporary rule that every attribute mutation receives a suffix plan and every
text mutation receives a full-document plan. Structural membership or order
changes remain full-document Style work under the current retained executor.

AF9 is a dependency-classification issue, not a targeted-restyle issue. It
does not add stable source-DOM identity to retained style entries, arbitrary
affected-element sets, transactional topology deltas, or subtree/sibling
execution. Those capabilities remain Milestone AG work.

## Related code and contracts

- `crates/css/src/style_invalidation.rs`
- `crates/css/src/style_invalidation/dependencies.rs`
- `crates/css/src/selectors/matching/context/attributes.rs`
- `crates/css/src/cascade/integration/limits.rs`
- `crates/browser/src/dom_store/mutation.rs`
- `crates/browser/src/page/dom_mutation.rs`
- `crates/browser/src/page/retained_render_state.rs`
- `crates/browser/src/page/style_cache.rs`
- `crates/browser/src/page/style_phase.rs`
- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af4c-html-host-language-selector-comparison.md`
- `docs/css/af4d-tree-structural-pseudo-class-matching.md`
- `docs/css/af4e-selector-invalidation-parser-conformance-closeout.md`
- `docs/css/af5-stylesheet-rule-collection-source-order-contract.md`
- `docs/css/ad7-css-owned-invalidation-impact-classification.md`
- `docs/css/u8-runtime-integration-contracts-extension-points.md`
- `docs/rendering/ac5-retained-style-artifact-reuse.md`
- `docs/html5/dompatch-contract.md`
- `docs/html5/node-identity-contract.md`

## Ownership boundary

CSS remains the sole semantic owner. Browser/runtime may observe and retain
neutral mutation facts, retain an opaque CSS artifact under a generic lifecycle
key, pass both back to CSS, store and merge opaque plans through CSS APIs, and
schedule or record actual execution. Browser/runtime must not inspect selector
ASTs or branch on class, ID, attribute, combinator, structural pseudo-class, or
subject-path categories.

The production order is:

```text
neutral committed DOM transition
  + compatible CSS-owned dependency artifact
  + CSS matching environment
  -> CSS-owned invalidation decision and opaque plan
  -> retained Style execution
  -> computed-style comparison
  -> AD7 property impact
  -> Browser Layout/Paint scheduling
```

AD7 is deliberately downstream. Property metadata cannot prove that selector
matching or cascade winner resolution may be skipped before the new computed
style exists.

## Owned dependency artifact

`StyleDependencyArtifact` is an owned, immutable CSS value. Its semantic index
is private. Browser has only a matching-environment compatibility query and a
CSS-owned debug serialization; it cannot enumerate dependency records.

Construction first canonicalizes selector dependency occurrences, then freezes
them into separate sorted immutable keyed groups for type names, ID values,
class tokens, attribute names, structural pseudo kinds, and relationship
kinds. Attribute-name groups contain sorted predicate groups. Classification
uses binary search to enter only groups named by the mutation; it does not
linearly scan unrelated retained selector dependencies. Every effect within a
keyed group remains deterministically sorted and deduplicated. Each occurrence
preserves:

- a type-name, ID-value, class-token, or current supported unqualified
  attribute-selector key;
- for attributes, existence or the exact supported operator/value predicate;
- `:root`, `:empty` direct-content, and first/last/only-child order kinds;
- descendant, direct-parent/child, adjacent-sibling, and following-sibling
  relationship kinds;
- the applicable selector namespace constraint; and
- the composed path from the dependency-bearing compound to the rightmost
  selector subject.

For example, `.a + .b .c` retains `.a` with the path
`next-sibling -> descendants`; it is not flattened to global sibling/tree
booleans. AF9 execution may still map that richer proof to a document suffix.
The path is the extension point for AG2, not a per-node dependency graph.

Current selector syntax supports unqualified attribute selectors only. The
artifact therefore keys the corresponding unqualified selector identity.
Neutral DOM facts retain exact qualified parser-created attributes so a future
namespace-qualified selector extension does not require Browser to acquire CSS
name semantics.

### Authoritative cascade participation

Extraction consumes AF5's pass-scoped `RuleCollection`; it does not reparse
declarations. Only an active style rule with at least one declaration whose
`CascadeDeclarationApplicability` is `Supported` contributes active dependency
records. A parsed selector attached only to invalid, unsupported,
custom-property-only, or otherwise non-candidate declarations cannot change a
supported cascade winner and is not indexed.

Invalid, malformed, unsupported, conditionally deferred, and skipped rules
remain represented by AF2/AF5 diagnostics but are inactive for AF9. Their
presence does not poison unrelated invalidation. Full fallback applies to an
active semantic dependency that unexpectedly cannot be represented, or to an
artifact construction/lifecycle failure—not to intentionally inactive future
syntax.

### Construction failures and limits

The artifact state is either a complete index or typed
`ConservativeUnavailable` metadata. Construction uses checked counters,
fallible vector/string reservation, and these `StyleResolutionLimits`:

- selector dependency records per document;
- owned selector dependency bytes per document; and
- composed subject-path steps per document.

Classification bounds actual candidate work: neutral before/after ID values,
borrowed class tokens and attribute local names, binary-search probes,
applicable namespace effects, and predicates evaluated beneath a found
attribute group. ID state is compared before its at-most-two probes. An
unchanged effective class value is not tokenized. Changed class values are
tokenized lazily as borrowed slices, every visited token consumes the bounded
work budget, consecutive duplicates skip repeated keyed probes, and Quirks
ASCII folding is performed by the CSS-owned comparator without constructing a
folded `String`. Attribute groups are likewise probed from
borrowed DOM local names. No mutation-side vector owns one string per raw
candidate. Unrelated dependency groups do not consume the evaluation budget.
Exceeding the configured publication budget during candidate processing or
keyed lookup selects full-document invalidation.

Limit, counter-exhaustion, and reservation failures discard the partial index.
They do not fail otherwise valid style resolution. CSS then selects a safe
full-document invalidation for later selector-relevant mutations. A failed new
generation never silently reuses an older complete artifact.

Decision diagnostics distinguish unavailable exact DOM transition details,
missing or incompatible retained dependency metadata, dependency-evaluation
limit exhaustion, and CSS classification/plan-storage resource failure. The
last case is reported as `dependency-classification-resource-unavailable`; it
does not imply that the neutral DOM snapshot was incomplete.

## Neutral DOM mutation contract

AF9 layers exact transition details on top of AF4e's coarse publication truth.
The coarse facts still preserve every occurring dimension, surviving IDs,
historical-target counts, document replacement, topology/order, ordinary
allocation, template association, and unclassified patch count.

Exact details are independently either complete or typed
`ConservativeUnavailable`. Attribute details retain, per surviving element:

- materialized source ID and element namespace;
- the committed pre-publication ordered qualified attribute vector, when the
  identity existed; and
- the final staged ordered qualified attribute vector.

Text details retain the surviving text identity, committed old and final text,
and the current direct parent element identity/namespace when present. Browser
does not tokenize class values, interpret ID values, apply selector name/value
case policy, evaluate attribute operators, or assign `:empty` meaning.

Exact capture is owned by `DomStore` and compares records resolved directly by
canonical `PatchKey` in the committed arena and fully applied staged arena.
The DOM-owned materialization bridge supplies surviving `Id` values, and the
arena parent back-reference supplies a text node's neutral direct parent.
Known mutation targets never require walking the materialized node tree.
Staged materialization remains part of publication validation. Multiple
`SetAttributes`, `SetText`, or `AppendText` operations in one atomic
publication therefore expose the net old-to-final transition, never
intermediate CSS behavior.

Historical final targets remain represented by the coarse historical count
and do not require an impossible live snapshot. Document replacement does not
compare old and new arenas across identity domains merely because patch-key or
materialized-ID numbers coincide; full Style invalidation is already required.

Capture is bounded before owned copying. Target, attribute-entry,
attribute-byte, and text-byte counts use checked arithmetic; vector and string
growth uses fallible reservation where Rust exposes it. Precision failure may
widen CSS invalidation but cannot erase coarse mutation truth or permit a fatal
DOM/protocol/identity invariant failure to publish. Publication state is
committed only after staged application, materialization, identity resolution,
and exact-fact invariant validation succeed.

## Shared matching semantics

Dependency transition classification and ordinary selector matching call the
same CSS-owned helpers for:

- quirks versus limited/no-quirks ID and class comparison;
- HTML versus SVG/MathML effective attribute-name behavior;
- exact qualified DOM attribute transport and the currently supported
  unqualified selector lookup;
- AF4c attribute value case policy and every supported attribute operator; and
- AF4d's document-whitespace definition for `:empty`.

There is no second approximate matching policy in invalidation code.

Inline `style` is classified separately from attribute-selector dependencies.
An effective unqualified `style` value transition is a direct cascade-input
change even when no active selector contains `[style]`. CSS owns inline
declaration parsing during recomputation; Browser only transports attributes.

## Classification and current execution vocabulary

`StyleInvalidationInput` combines the coarse publication fact, optional exact
views, a compatible artifact, and the matching environment. CSS returns an
opaque `StyleInvalidationDecision`, whose plan remains `None`, document suffix,
or full document.

| committed transition | current CSS result |
| --- | --- |
| document replacement or stylesheet-set change | full document |
| insertion, removal, reparent, or reorder | full document |
| unclassified patch | full document |
| relevant live ID/class/attribute predicate transition | suffix from affected identity |
| effective inline `style` transition | suffix from affected identity |
| irrelevant or net-no-op attribute replacement, complete compatible artifact | no Style plan |
| direct text whitespace-contribution transition with active `:empty` dependency | suffix from direct parent, or full when the current proof is unavailable |
| text transition with no active `:empty` dependency | no CSS Style plan; intrinsic Layout work remains independent |
| stale, missing, incomplete, or incompatible metadata for attribute/text classification | full document |
| historical attribute/text target that cannot be classified exactly | full document |

Structural selector metadata is still recorded, but structural membership or
order changes always require total style construction for inserted or moved
content and cannot safely reuse position-bound retained entries. Absence of a
structural selector dependency never means absence of baseline computed-style
construction. AF9 therefore does not use dependency misses to skip structural
Style work.

The suffix proof remains the AF1/U8 proof: supported selectors can affect the
changed subject or later document-order subjects, and top-down recomputation
also covers inherited changes in that suffix. Any missing cache, identity/key
incompatibility, or CSS execution validation failure falls back to a full
recompute.

## Retained lifetime and publication

`RuleCollection<'source>` stays pass-scoped and borrowed. The dependency
artifact owns every retained selector key/path and is never borrowed from the
collection.

Browser retains it under:

```text
PageStyleDependencyKey {
  identity_domain,
  stylesheet_generation,
}
```

The stylesheet component is the existing authoritative
`PageStyleGenerations.stylesheets`; AF9 introduces no second generation
counter. CSS separately validates `SelectorMatchingEnvironment`. Ordinary DOM
style-input generation changes do not invalidate the artifact because its
selector/cascade structure is DOM-independent. A structural full Style pass
therefore reuses a compatible artifact. Document identity replacement,
stylesheet generation change, or environment mismatch makes it ineligible.

When a compatible artifact is absent, CSS builds a replacement from the same
AF5 collection used by the style execution. The replacement is published only
after the corresponding style recomputation succeeds. A failed or aborted
style pass cannot publish mismatched dependency state. A typed unavailable
artifact for a genuinely new generation is published as unavailable; an older
complete artifact is not substituted.

## Debug and regression surface

`StyleDependencyArtifact::to_debug_snapshot()` is versioned and emits the
matching environment, active/inactive rule summary, complete/unavailable
state, canonical records, predicates, namespace constraints, and composed
subject paths. Its projection has an independent 4,096-record and 512-KiB
bound, deterministically reports total and visible records, and explicitly
marks truncation without weakening the semantic artifact.
The artifact's ordinary Rust `Debug` implementation is deliberately compact:
it exposes only matching environment, complete/unavailable state, aggregate
counts, the classification limit, and a compact failure kind. It never formats
dependency keys, predicates, effects, subject paths, or the private index.
The versioned bounded snapshot is the sole detailed dependency diagnostic.
`StyleInvalidationDecision::to_debug_snapshot()` is separately
versioned and emits CSS's reason, dependency-hit summary, inline cascade-input
state, and selected opaque execution scope. Browser exposes these CSS-owned
strings without interpreting them.

The neutral DOM mutation snapshot advances to version 2 and reports both
coarse dimensions and exact-detail complete/unavailable state. These snapshots
are deterministic internal regression contracts, not CSSOM or public web APIs.

## Invariants

- CSS exclusively owns selector/cascade dependency meaning and plan merging.
- Browser branches only on neutral DOM/lifecycle facts and opaque plan effects.
- Exact precision augments; it never replaces or weakens coarse mutation truth.
- Artifact compatibility uses retained document identity, the one authoritative
  stylesheet generation, and CSS matching-environment validation.
- Inactive unsupported syntax does not create active dependency records.
- Metadata failure widens work and can never preserve stale computed style.
- A committed publication must carry its parser-selected document mode;
  Browser never manufactures a fallback matching environment.
- Structural mutations remain full Style recomputation in AF9.
- Inline cascade input is independent of `[style]` selector presence.
- AD7 impact is evaluated only after new computed style exists.
- Canonical record ordering/deduplication and debug output are deterministic.

## Deliberate remaining limitations

AF9 does not add Blink/Gecko-scale invalidation sets, per-node dependency
graphs, candidate bloom filters, selector caches, new selector syntax, dynamic
pseudos, `:has()`, `:nth-child()`, pseudo-elements, complete media-query
invalidation, CSSOM, JavaScript style APIs, targeted Layout/Paint, or stable
source-identity partial structural restyle.

Milestone AG — Stable-DOM-Identity Style Invalidation and Targeted Restyle
Execution remains responsible for identity-bound retained style entries and
transactional structural deltas, followed by dependency-directed self,
descendant, child, sibling, root, empty, and child-order affected-element
execution with inherited-style propagation and full fallback.
