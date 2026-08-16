# AF4c: HTML Host-Language Selector Comparison

Last updated: 2026-08-16
Status: implemented

AF4c makes the existing supported selector subset apply HTML host-language
comparison rules when matching parser-created HTML documents. It does not add
selector syntax. CSS remains the sole owner of selector comparison policy and
operator execution; HTML supplies exact parser-created names, namespaces,
attribute values, and `DocumentMode` as neutral facts.

This contract refines the earlier Milestone Q matching descriptions. Where a Q
document describes all supported selector values as exact or describes HTML
document names as generally ASCII-insensitive, this AF4c contract is the
normative current behavior.

Related implementation:

- `crates/css/src/selectors/matching/comparison.rs`
- `crates/css/src/selectors/matching/host_language.rs`
- `crates/css/src/selectors/matching/context/attributes.rs`
- `crates/css/src/selectors/matching/context/compound.rs`
- `crates/css/src/selectors/matching/context/queries.rs`
- `crates/css/src/dom_attributes.rs`

Related contracts:

- `docs/css/af1-selector-cascade-computed-style-architecture-contract.md`
- `docs/css/af2-selector-ast-and-parser.md`
- `docs/css/af4a-document-matching-environment.md`
- `docs/css/af4b-selector-dom-query-contract.md`
- `docs/css/q1-selector-matching-architecture.md`
- `docs/css/q2-selector-matching-context.md`
- `docs/css/q8-selector-matching-invariants-extension-hooks.md`
- `docs/html5/ae2-parser-created-dom-node-model.md`
- `docs/html5/ae11-foreign-content-tree-construction-contract.md`

## Scope And Ownership

AF4c applies to CSS matching against parser-created HTML documents. The
document may contain HTML, SVG, and MathML elements, and policy is selected
from the candidate element and effective attribute facts rather than from an
ancestor or from raw source spelling.

CSS owns:

- host-language name-matching policy;
- document-mode policy for ID and class selector values;
- HTML default attribute-value comparison policy;
- every supported attribute operator and its empty-value behavior;
- ASCII-only comparison primitives and CSS-whitespace tokenization.

HTML owns:

- parser-created `DocumentMode` selection;
- canonical parser-created element names and namespaces;
- ordered attributes with exact expanded names and values;
- foreign-content name adjustment and integration-point construction.

Browser/runtime transports the selected matching environment and consumes
CSS-owned artifacts. Layout and Paint consume later CSS/layout artifacts. None
of those layers interprets selector comparison semantics.

XML-document matching is outside AF4c. `SelectorMatchingEnvironment` currently
contains HTML `DocumentMode`, not a general document-language discriminator.
The rules below must not be generalized to XML by assumption.

## Comparison Architecture

AF4c separates policy selection from comparison execution.

### Symmetric value comparison

`TextCaseSensitivity` is a small matching-private value policy:

```rust
enum TextCaseSensitivity {
    Sensitive,
    AsciiInsensitive,
}
```

It applies only where the semantics genuinely compare values either exactly or
symmetrically ASCII-insensitively:

- ID selector values;
- class selector values;
- attribute selector values;
- equality, token, dash, prefix, suffix, and substring execution after policy
  has been selected.

It does not select policy and does not know document mode, element namespace,
attribute identity, or selector syntax.

### Asymmetric host-language name matching

HTML element and attribute names use a separate, narrowly typed concept
equivalent to:

```rust
enum HostLanguageNameMatch {
    Exact,
    AsciiLowercaseSelector,
}
```

For `AsciiLowercaseSelector`, matching compares each exact actual-name byte
with the corresponding selector/request byte after ASCII-lowercasing only that
selector/request byte. It does not fold the actual DOM name.

Consequently:

```text
actual "type", selector "TYPE"  -> match
actual "TYPE", selector "TYPE"  -> no match
```

The second result is intentionally different from symmetric
ASCII-insensitive equality. Parser-created HTML names normally satisfy the
canonical lowercase invariant, but CSS must not bake in behavior that would
reinterpret a future noncanonical, script-created, or mutated actual DOM name.

The name matcher is allocation-free and ASCII-only. Foreign element and
attribute names use exact comparison.

### Policy selectors

Policy selection is centralized in CSS and divided by semantic category:

- type-selector name policy takes the candidate element namespace;
- unqualified attribute-name policy takes the candidate element namespace;
- ID/class value policy takes `DocumentMode`;
- attribute-value policy takes the candidate element namespace and the
  effective matched attribute's namespace and local name.

These are intentionally separate functions. In particular, attribute-value
policy has no document-mode input, and ID/class value policy has no element-
namespace input. Operator execution receives only a previously selected
`TextCaseSensitivity` and borrowed values.

## Element And Attribute Names

For a named type selector in the current HTML-document scope:

- on an HTML element, ASCII-lowercase the selector-side name and compare it
  identically to the actual canonical element local name;
- on an SVG or MathML element, compare the selector name and actual local name
  exactly.

For the name of an unqualified attribute selector:

- first require `AttributeNamespace::None`;
- on an HTML element, ASCII-lowercase the selector/request-side name and
  compare it identically to the actual attribute local name;
- on an SVG or MathML element, compare names exactly;
- retain the first matching attribute in provider order.

The shared effective unqualified-attribute helper implements only this name
resolution and ordering. Selector matching consumes the returned complete
borrowed attribute. Inline-style discovery uses the same helper with the
canonical request `style`. The helper does not know selector values, attribute
operators, ID/class rules, or the insensitive-value inventory.

AF4b's canonical parser-created HTML element-name validation remains in force.
AF4c neither weakens that validation nor creates noncanonical production DOM
fixtures to exercise the asymmetric primitive.

## ID And Class Values

ID and class selector value policy is document-wide:

| Document mode | `#id` | `.class` |
| --- | --- | --- |
| `NoQuirks` | sensitive | sensitive |
| `LimitedQuirks` | sensitive | sensitive |
| `Quirks` | ASCII-insensitive | ASCII-insensitive |

The Quirks rule is not namespace-gated. It applies to candidate elements in
the parser-created document regardless of whether the element is HTML, SVG, or
MathML.

Attribute selectors named `id` or `class` do not use this policy. Their values
are governed only by attribute-selector value policy, so `[id=...]`,
`[class~=...]`, and every other supported operator remain case-sensitive for
those names in Quirks mode.

Class tokenization uses exactly CSS whitespace: TAB, LF, FF, CR, and SPACE.
NBSP and other Unicode whitespace are not separators.

## Effective Attribute Identity And Value Policy

Attribute matching retains the complete effective borrowed attribute until
after policy selection:

```text
candidate element namespace
effective attribute namespace
effective attribute local name
exact borrowed attribute value
```

The authored selector name is used to resolve the effective unqualified
attribute and is not reused to classify value semantics. Therefore
`[TYPE=button]` on an HTML element resolves the semantic `type` attribute and
receives the same value policy as `[type=button]`. Raw selector spelling is not
a policy key.

HTML default ASCII-insensitive attribute-value comparison applies only when:

- the candidate element is in the HTML namespace;
- the effective attribute is unqualified;
- the effective actual attribute local name exactly appears in the canonical
  inventory below.

The inventory lookup is exact and allocation-free. It does not symmetrically
fold a noncanonical actual attribute name into a canonical inventory member.
That is correct for AF4c's parser-created HTML scope and avoids defining future
DOM-mutation semantics accidentally.

The canonical 46-name inventory is:

```text
accept
accept-charset
align
alink
axis
bgcolor
charset
checked
clear
codetype
color
compact
declare
defer
dir
direction
disabled
enctype
face
frame
hreflang
http-equiv
lang
language
link
media
method
multiple
nohref
noresize
noshade
nowrap
readonly
rel
rev
rules
scope
scrolling
selected
shape
target
text
type
valign
valuetype
vlink
```

The inventory is a sorted fixed-size static array. Exact binary search gives a
small bounded lookup without heap storage, hashing, lazy initialization, or
normalization. Tests independently protect its exact length, order,
uniqueness, missing entries, and additional entries.

Ordinary attribute values remain case-sensitive. HTML-only value policy does
not apply to SVG elements, MathML elements, or qualified attributes.

## Attribute Operators

Presence matching is value-independent. For a matched value selector, the
selected value policy is applied consistently to all six operators:

| Operator | Behavior, including an empty expected value |
| --- | --- |
| `=` | Full equality; `=""` may match an empty actual value. |
| `~=` | CSS-whitespace token equality; an empty expected value or one containing CSS whitespace never matches. |
| `|=` | Equality or prefix immediately followed by `-`; `|=""` may match `""` or a value beginning with `-`. |
| `^=` | Prefix comparison; an empty expected value never matches. |
| `$=` | Suffix comparison; an empty expected value never matches. |
| `*=` | Substring comparison; an empty expected value never matches. |

There is no generic empty-needle rejection. These rules are operator-owned and
do not change selector parse validity.

## ASCII And Allocation Invariants

Ordinary matching:

- operates on borrowed strings and byte slices;
- does not allocate lowercase `String` or `Cow` values;
- does not call Unicode lowercase or case-folding APIs;
- does not normalize or mutate selector IR or DOM storage;
- selects between small enum variants through direct static dispatch;
- does not use trait objects, boxed closures, function-pointer strategies, or
  heap-backed/lazy sets for comparison policy.

ASCII-insensitive value comparison folds only ASCII letters. Identical
non-ASCII code points remain identical, but distinct non-ASCII uppercase and
lowercase code points do not become equal. Thus a comparison equivalent to
`FOO-é-BAR` versus `foo-é-bar` may match in ASCII-insensitive mode, while
`FOO-É-BAR` versus `foo-é-bar` does not.

A focused allocation regression guard constructs and parses all inputs before
measurement, repeatedly executes `matches_compound_selector`, observably
consumes the match count, and requires zero allocations, allocation bytes, and
reallocations inside the measured comparison region. Criterion remains timing
coverage, not deterministic allocation proof.

## Validation Contract

Focused tests cover:

- asymmetric selector-side name normalization and foreign exact matching;
- all three document modes for ID and class values;
- non-ASCII behavior for name and value primitives;
- exact operator-specific empty behavior and CSS whitespace;
- the independent exact 46-name inventory;
- the complete 46 by 6 attribute-value policy/operator matrix;
- negative inventory controls, including `id` and `class`;
- every supported `[id...]` and `[class...]` value operator in Quirks mode;
- exact-zero comparison-path allocation behavior.

Representative parser-backed tests cover NoQuirks, LimitedQuirks, Quirks,
HTML, SVG, MathML, qualified attributes, and an HTML descendant reached
through a foreign-content integration point. At least one document-level
cascade/computed-style path proves that the comparison policy affects real
style results, not only hand-built matcher fixtures.

Selector serialization, specificity, match-result shape, and debug snapshot
schema remain unchanged. Environment-dependent match outcomes may change, so
regression snapshots cover the new outcomes without a version bump.

## Deliberate Exclusions

AF4c does not add or implement:

- `[attr=value i]` or `[attr=value s]` modifiers;
- CSS namespace selector syntax;
- new selector grammar or selector IR nodes;
- CSS escape decoding;
- XML-document matching semantics;
- broader DOCTYPE classification;
- DOM mutation or scripting behavior;
- new selector invalidation behavior;
- selector caches or unrelated indexing optimizations;
- Layout-, Paint-, Browser/runtime-, HTML-, or DOM-adapter-owned selector
  meaning.

CSS tokenizer/parser paths do not yet consistently expose fully escape-decoded
semantic selector names and values. AF4c consumes the existing selector IR and
does not key policy from raw escape spelling as a workaround. Standards-
conformant selector escape decoding remains separate CSS Syntax/parser work.
