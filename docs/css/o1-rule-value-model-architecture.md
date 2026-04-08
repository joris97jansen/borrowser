# O1: Define CSS Rule/Value Model Architecture And Ownership Strategy

Last updated: 2026-04-08  
Status: architecture contract implemented

This document is the source-of-truth contract for Milestone O's first issue:
the architecture boundary, ownership rules, source-span policy, and
deterministic serialization goals for Borrowser's engine-facing CSS rule/value
representation.

Milestone N already established the syntax layer in `css::syntax`. That syntax
layer is now the only acceptable source of parsed CSS structure, but it is not
the final stylesheet/value contract for selector, cascade, and computed-style
work. The current `Compat*` types and raw `Declaration { name: String, value:
String }` path remain migration bridges only.

This issue does not finish the CSSOM/rule-value implementation. It defines the
model contract that follow-up Milestone O issues must implement without
re-interpreting the basic architecture.

Related code:
- `crates/css/src/syntax/mod.rs`
- `crates/css/src/syntax/parser/model.rs`
- `crates/css/src/syntax/results.rs`
- `crates/css/src/syntax/compat.rs`
- `crates/css/src/cascade.rs`
- `crates/css/src/computed.rs`

Related documents:
- `docs/css/syntax-parser-contract.md`
- `docs/css/n8-parser-contract-cutover.md`
- `docs/css/n9-selector-structure-expansion.md`
- `docs/css/o8-cssom-contract-and-legacy-retirement.md`

## Implemented Result

Milestone O now has an explicit in-repository contract for:

- the engine-facing stylesheet/rule/declaration/value layer that sits
  downstream of `css::syntax`
- the authoritative transformation path from syntax-layer output into that
  engine-facing model
- the ownership rules for names, values, spans, and source-attached data
- the retention policy for syntactically parsed declarations before later
  semantic validation exists
- the boundary between purely syntactic structure and later semantic
  interpretation
- the authored-versus-canonical preservation policy for engine-owned CSS data
- the deterministic serialization/debug requirements for the model
- the non-goals that remain deferred to later selector, cascade, and computed
  style milestones

Follow-up implementation work must conform to this contract. It must not grow a
new rule/value representation opportunistically inside `CompatStylesheet`,
`Declaration`, or ad hoc raw strings.

## Why This Exists

The repository currently has three distinct CSS representations:

1. the syntax-layer AST in `css::syntax::parser::model`
2. the compatibility projection in `CompatSelector`, `CompatRule`, and
   `CompatStylesheet`
3. the raw `String`-based declaration/value inputs still consumed by
   `cascade` and `computed`

That split was acceptable during Milestone N because the goal there was to
replace ad hoc parsing with a real tokenizer/parser foundation. It is not an
acceptable long-term architecture for an engine-quality CSS stack.

Later milestones need a stable internal rule/value contract that:

- is selector-aware without owning selector matching semantics
- is declaration/value-aware without pushing raw strings further into the
  engine
- preserves enough source information for diagnostics, snapshots, and future
  tooling
- can be consumed by cascade and computed-style work without reparsing CSS text

## Layer Boundary

Milestone O introduces a distinct layer between syntax parsing and later CSS
semantics.

The architectural boundary is:

1. `css::syntax`
   - owns tokenization
   - owns syntax parsing and malformed-input recovery
   - owns source-preserving component-value structure
   - does not own selector matching, cascade semantics, or computed values
2. CSS rule/value model layer
   - consumes syntax-layer output
   - owns the engine-facing stylesheet/rule/declaration/value representation
   - canonicalizes names only where CSS defines case-insensitive matching
   - preserves source/debug metadata needed by later stages
   - does not own selector matching, cascade winner resolution, or computed
     values
3. Later CSS semantics
   - selector AST refinement and selector matching
   - at-rule interpretation such as `@media` / `@supports`
   - cascade order and winner selection
   - inheritance, specified values, computed values, and layout-facing
     interpretation

The rule/value model is the stable handoff layer from parsing into the rest of
the engine. Later milestones may add semantics on top of it, but they must not
reinterpret its basic ownership or source policies.

## Authoritative Transformation Path

The engine-facing rule/value model must be constructed only from structured
`css::syntax` output.

Required transformation rules:

- the permanent stylesheet/rule/declaration/value model is built from
  `StylesheetParse`, `DeclarationListParse`, `CssStylesheet`, and related
  syntax-layer structures
- later stages must not reconstruct the model by reparsing raw stylesheet text
  or raw declaration strings
- `CompatSelector`, `CompatRule`, `CompatStylesheet`, and
  `Declaration { name: String, value: String }` are not a legal foundation for
  the permanent engine model
- compatibility projection may remain as a migration bridge for old consumers,
  but it must not become the syntax-to-engine transformation path

This rule exists to prevent architectural regression back toward ad hoc
string-based CSS handling after Milestone N established the syntax layer.

## Scope For Milestone O

Milestone O owns:

- stylesheet structure suitable for long-lived engine storage
- explicit rule kinds
- explicit declaration representation
- typed or semi-typed value storage that replaces raw `String`-only values
- source/debug span preservation for parsed nodes
- deterministic, explicit serialization/debug output for the model
- ownership and interning rules for reusable CSS names and identifiers

Milestone O does not require full property-specific value semantics in its
first issue. It does require the representation to be structured enough that
later property parsing does not need to start from raw strings again.

## Contracted Model Shape

The concrete Rust type names may land incrementally, but the model contract is
already fixed at the level below.

### Stylesheet

The engine-facing stylesheet object must:

- own rules in source order
- preserve stylesheet origin metadata needed for later cascade work
- be independent from tokenizer internals
- be suitable for long-lived storage beyond a single parser call

The stylesheet is the unit later selector and cascade work consume. It must not
require downstream callers to inspect syntax-layer tokens directly.

### Rule Kinds

The rule model must distinguish at least:

- style rules
- at-rules

Style rules must carry:

- selector-side source or syntax payload
- ordered declarations
- rule-level source/debug spans

At-rules must carry:

- canonical at-rule name
- prelude/value payload
- optional block payload
- rule-level source/debug spans

At-rules are structural at this milestone. The model must preserve enough data
for later interpretation, but it does not evaluate media queries, supports
conditions, keyframes, or nesting semantics yet.

### Declaration

The declaration model must be explicit and stable. Each declaration is expected
to carry at least:

- property name
- value representation
- `!important` state
- declaration span
- property-name span
- value span
- `!important` span when present

Declaration storage must remain ordered. The model must not collapse
declarations into hash maps or any other form that loses source order, because
source order is part of later cascade behavior.

### Declaration Retention Policy

Milestone O stores syntactically parsed declarations that survived
syntax-layer recovery. It does not decide full property semantics yet.

Required retention rules:

- declarations successfully produced by `css::syntax` remain present in the
  engine-facing model in source order
- unknown properties are still retained as parsed declarations at this stage
- values that are structurally parsed but later prove semantically invalid for
  a property or context are not dropped by the O-layer contract itself
- syntax-rejected or recovery-dropped declarations are not recreated in the
  engine model; they remain represented only through syntax diagnostics and
  recovery behavior
- later cascade/computed-value stages may mark declarations inapplicable or
  invalid, but they must do so on top of the O-layer model rather than by
  reparsing text

This keeps the engine contract broad enough for later semantic validation while
still respecting syntax-layer recovery as the only place malformed declaration
fragments are discarded.

### Value Representation

Milestone O's value model is property-independent but not raw-text-based.

The required direction is a semi-typed specified-value syntax tree or list that
can represent:

- identifiers and keywords
- strings
- numbers
- percentages
- dimensions with unit identity
- hashes where still syntactically meaningful
- functions
- simple blocks
- significant delimiters and separators where list structure depends on them

The value model must preserve ordering and explicit structure so later code can
interpret property values without reparsing CSS text.

The value model must not use a single raw `String` as the normative storage for
declaration values.

Preserved source text may still exist for debug/recovery purposes, but only as
an explicit auxiliary field or source span, not as the primary semantic
representation.

## Authored Versus Canonical Data Policy

Milestone O is an engine-normalized model with explicit, limited authored-data
preservation. It is not yet a lossless CSSOM-text reconstruction layer.

Required policy:

- canonical identifiers are the primary engine-facing contract for
  case-insensitive CSS names
- authored spelling is preserved through source spans and optional
  source/debug attachments, not by requiring every node to carry authored text
  as a first-class semantic field
- the model is not required to preserve every authored lexical distinction as a
  semantic enum/field when that distinction has no engine meaning at this
  stage
- when both canonical and authored views exist, engine behavior must use the
  canonical form while diagnostics/debug output may surface authored source

This resolves the intended tradeoff now: the model is canonical for engine use,
with enough source fidelity for diagnostics and deterministic debugging, but it
is not yet a full-fidelity authored-text round-tripping layer.

## Syntax Versus Semantics

This section defines the boundary between Milestone O and later work.

Purely syntactic in Milestone O:

- rule ordering
- rule kind classification
- declaration ordering
- property-name capture
- structural value representation
- source span preservation
- deterministic serialization

Semantic interpretation deferred to later milestones:

- selector AST semantics and DOM matching
- specificity calculation as a selector-system contract
- shorthand expansion
- longhand alias handling beyond basic name canonicalization
- custom property substitution
- `var()` / `env()` / `calc()` evaluation
- property-specific validation and invalid-at-computed-value-time behavior
- cascade origin/layer/importance ordering logic
- inheritance and computed-style generation
- at-rule-specific runtime evaluation

Milestone O may expose enough structure for those later systems, but it must
not absorb their logic prematurely.

## Ownership And Interning Strategy

Ownership rules are fixed by this issue.

### General Ownership

- The engine-facing stylesheet model must own its rule/declaration/value data.
- It must not borrow from transient parser-local buffers or token vectors.
- Parser output may be converted into the model immediately, but the resulting
  model must remain valid after parser scratch structures are dropped.

### Source Ownership

- Source text ownership remains with a parse artifact that also owns the
  `CssInput` used to create spans.
- Model nodes may store spans and other source metadata, but they must not
  require borrowed `&str` slices into parser-local memory.
- Later runtime code must be able to keep the stylesheet model alive even if it
  chooses not to keep all source text for production execution.

### Interning Policy

Reusable identifiers should be interned behind stable handle types rather than
copied repeatedly into every node.

The interner contract is:

- no process-global mutable CSS interner is introduced by default in Milestone
  O
- interned text is owned by an explicit CSS model context such as a stylesheet,
  stylesheet set, or paired model arena created for that stylesheet/model
  family
- interned handles are stable only within that owning model context unless a
  future contract explicitly widens their scope
- two stylesheets must not assume handle identity is comparable across
  different owning model contexts
- caches or cross-stylesheet data structures must not persist handle identity
  outside the owning context unless they also retain or resolve the owning text
  table explicitly
- handles used in debug/snapshot output must serialize deterministically by
  resolved canonical text or another stable textual encoding, never by pointer
  value or incidental numeric allocation order

This is the intended middle ground for Borrowser's current maturity: explicit
CSS-owned interning without accidental global-state semantics.

Interning is expected for:

- property names
- at-rule names
- function names
- identifier/keyword values that recur frequently
- unit identifiers
- custom property names

Default non-interned storage is acceptable for:

- large string literals
- URL payloads
- rare preserved raw fragments

Those should use owned string storage rather than repeated temporary
allocations.

### Canonicalization Rules

Canonicalization happens before interning when CSS defines case-insensitive
matching for the relevant grammar position.

Required rules:

- standard property names are ASCII-lowercased before interning
- at-rule names are ASCII-lowercased before interning
- function names are ASCII-lowercased before interning when the grammar
  position is ASCII case-insensitive
- unit identifiers are ASCII-lowercased before interning
- custom property names are preserved exactly and interned case-sensitively
- string payloads preserve authored text
- value normalization must not silently fold property semantics such as color
  canonicalization, shorthand expansion, or zero-unit folding at this stage

The model may carry both canonical identifiers and authored-span metadata, but
canonical names are the engine contract for case-insensitive CSS names.

## Source Span And Debug Span Policy

Source spans are part of the Milestone O contract, but they are debug and
diagnostic metadata rather than the primary ownership mechanism.

Parsed nodes must carry spans where they materially help later work:

- rules
- rule selectors/preludes
- blocks
- declarations
- declaration names
- declaration values
- important annotations
- structured value nodes when their origin needs to remain inspectable

Policy rules:

- parsed nodes must populate spans deterministically
- authored nodes carry source-backed span metadata when available
- synthesized nodes created later by CSSOM mutation or expansion are
  explicitly synthesized and may carry no source span by design
- later expanded/transformed nodes may carry synthesized identity plus optional
  provenance back to one or more authored spans
- spans must remain bound to the owning `CssInputId` while source-backed debug
  output is available
- span absence must be explicit rather than represented by invalid offsets

Later milestones may introduce richer source-map types, but they must remain
compatible with the current `CssSpan` ownership contract from Milestone N.

## Deterministic Serialization And Debug Goals

Milestone O must not use incidental Rust `Debug` formatting as its regression
surface.

The rule/value model must expose explicit, deterministic serializers for:

- full stylesheet snapshots
- rule snapshots
- declaration/value snapshots

Those serializers must:

- be versioned when the snapshot grammar changes
- preserve source order
- use explicit field labels and variant names
- avoid hash-order or address-based output
- render canonical names deterministically
- include spans when available
- distinguish authored-source text from canonicalized names when both matter

Stable span encoding rules:

- spans in the stable snapshot surface are encoded as byte offsets only
- the canonical present-span shape is `@start..end`
- line/column rendering is not part of the stable regression surface
- raw `CssInputId` values are not part of the default stable snapshot grammar,
  because they are owner-local identities rather than cross-run semantic data
- absent spans must use an explicit sentinel such as `@<none>`, not placeholder
  numeric offsets

If a future serializer needs to show provenance across multiple source owners,
it must add an explicit, deterministic origin label rather than exposing opaque
runtime handle values.

Human-oriented `Debug` implementations may exist for local development, but the
stable contract for regression tests is an explicit serializer.

## What Remains Out Of Scope

The following are intentionally deferred beyond this issue:

- JS-exposed CSSOM mutation APIs and live stylesheet objects
- selector matching semantics
- specificity and cascade winner resolution
- computed value generation
- property-specific semantic parsing completeness
- at-rule evaluation semantics
- stylesheet mutation invalidation strategy
- style sharing, cache keys, and performance tuning beyond the ownership rules
  above

These are follow-up milestones. They must build on the rule/value contract
defined here rather than redefining it.

## Contributor Rules After O1

Until the full Milestone O implementation lands:

- do not extend `CompatStylesheet` into the permanent engine stylesheet model
- do not treat `Declaration { name: String, value: String }` as the long-term
  declaration contract
- do not add new downstream reparsing steps that recover structure from raw CSS
  strings
- keep selector syntax, rule/value representation, and cascade semantics as
  separate layers
- add deterministic serializers alongside any new rule/value model types
- preserve source-order semantics in all rule/declaration collections

## Exit Criteria

- a written model contract exists in the repository
- ownership and source-span policy are documented explicitly
- the rule/value model scope for Milestone O is unambiguous
- later milestones can implement selector, cascade, and computed-style work on
  top of this contract without redefining basic rule/value architecture
