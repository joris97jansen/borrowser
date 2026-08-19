# AF4d: Tree-Structural Pseudo-Class Matching

Status: implemented current contract for Milestone AF issue AF4d

AF4d adds the first typed pseudo-class family to Borrowser's CSS-owned
selector pipeline. The supported Selectors Level 4 subset is `:root`,
`:empty`, `:first-child`, `:last-child`, and `:only-child`.

## Ownership and typed IR

`TreeStructuralPseudoClass` is semantic selector state. A
`TreeStructuralPseudoClassSelector` stores that enum plus the source span of
the complete `:keyword`, and participates in the existing `SubclassSelector`
IR. Matcher code dispatches on the enum; it never compares authored pseudo
strings. HTML exposes only AF4b's neutral document-element identity, element
axes, ordinary direct element children, and exact ordinary direct text.

Pseudo keywords are recognized ASCII case-insensitively. Debug serialization
uses the enum's canonical lowercase keyword. Standards-conformant semantic CSS
escape decoding remains a selector-wide gap: AF4d supports the currently
representable unescaped and mixed-ASCII-case subset and does not add local
escape decoding.

The parser distinguishes supported tree-structural pseudos, unsupported
identifier pseudo-classes, unsupported functional pseudo-classes, unsupported
pseudo-elements, and malformed pseudo syntax. `:before`, `:after`,
`:first-line`, and `:first-letter`, including ASCII-case variants, follow the
unsupported pseudo-element path in their legacy single-colon spelling.
Double-colon pseudo-elements remain unsupported. Functional pseudos remain
unsupported; `:is()` and `:where()` retain forgiving-list diagnostics with
ASCII-case-insensitive name recognition. Invalid or unsupported selector lists
remain wholly non-matchable and are never partially salvaged.

Pseudo selector spans are composed from the first colon through the identifier
or function component. Cross-input or non-monotonic bounds are internal
`InvariantViolation` outcomes; the parser never collapses such a span to the
colon and never reclassifies the selector as supported or ordinarily
unsupported.

## Specificity

`Specificity` uses truthful bounded A/B/C component terminology:

- A counts ID selectors;
- B counts class selectors, attribute selectors, and supported pseudo-classes;
- C counts type selectors and, when eventually supported, pseudo-elements.

Ordering is lexicographic A then B then C, addition is fieldwise saturating,
and combinators are neutral. Each AF4d pseudo contributes exactly one B.
`:only-child` is one semantic node and contributes one B even though matching
may share first/last sibling predicates. Cascade transports selector-produced
specificity without recalculation.

## Matching

- `:root` compares the candidate with `document_element()` identity. It is not
  inferred from parentlessness, local name, or projection position. An explicit
  element-subtree root is not `:root`.
- `:first-child` means no preceding element sibling.
- `:last-child` means no following element sibling.
- `:only-child` means neither a preceding nor following element sibling.

The child-position pseudos use Selectors Level 4 inclusive-sibling semantics
and do not require an element parent. The document element and an isolated
subtree root therefore satisfy the child-position predicates when their
element sibling axes are empty. Text, comments, processing instructions, and
doctypes do not participate in those axes.

For the selected Level 4 target, `:empty` matches when the element has no
ordinary direct element child and every ordinary direct text child contains
only TAB, LF, FF, CR, or SPACE. Empty text and mixtures of those five document
whitespace characters are ignorable. NBSP and every other character are
meaningful content. Comments and processing instructions do not affect
emptiness. Associated template contents are not ordinary template-host
children; real ordinary template-host children, where representable, do
participate. CSS does not special-case the `template` name and HTML exposes no
pseudo-specific query.

## Text mutation invalidation and retained runtime handoff

Exact text can change `:empty` matching. Until CSS owns a reverse selector
dependency index, an aggregate `StyleChangeFacts::DomPublication` containing a
text dimension classifies to a full-document `StyleInvalidationPlan`. CSS
continues to own plan construction,
canonicalization, merging, full-over-suffix dominance, and execution scope.

A Browser-observed mutation fact does not by itself authorize a retained
style-input generation change. CSS's invalidation classification result
determines whether that fact invalidates retained style results:

```text
neutral StyleChangeFacts -> CSS classification
None       -> no style-input generation change and no new Style dirtiness
Some(plan) -> advance style-input generation, CSS-merge the opaque plan,
              apply retained-artifact policy, and schedule Style work
```

Browser applies that classification result exactly once at publication scope.
Successful application produces one `AppliedCssStyleInvalidation` capability;
consuming it creates one separate `DomPublicationStyleInvalidated` request.
Intrinsic text, attribute, structural, and unknown requests remain independent
and never receive copied Style authorization. All requests are then projected
into retained dirty state and pending frame work. Static intrinsic entry-point
contracts cannot fabricate a CSS classification result.

`RenderInvalidationRequest` and its `RenderInvalidationWorkPlan` are sealed,
read-only runtime values. Consumers may inspect their entry point, owner, and
phase work, but cannot construct arbitrary phase combinations. The intrinsic
request factory and typed CSS Style composition path are the production
construction authorities.

CSS-authorized Style construction accepts only a capability created after the
typed DOM-publication or stylesheet input has been classified and applied.
Viewport, resource, input-state, and intrinsic DOM entry points cannot enter
this path or manufacture direct Style triggers.

For AF4e, text changes return `Some(full-document)`, discard a retained style
artifact when present, and perform a full selector/cascade/computed-style
recomputation. Independently, `DomTextChanged` remains direct Layout input
with `TextContentChanged`; its Layout reason must survive even when Style is
also dirty and later style-impact cleanup removes a cascaded reason.

## Unsupported scope

AF4d does not implement indexed structural pseudos such as `:nth-child()`,
dynamic state (`:hover`, `:active`, `:focus`, `:visited`, `:target`),
`:is()`, `:where()`, `:not()`, `:has()`, pseudo-element matching, generated
content, CSSOM serialization, dynamic JavaScript pseudo state, selector escape
decoding, selector dependency indexing, or fine-grained text invalidation.

AF4e completes the parser-backed and retained-runtime proof for these AF4d
semantics; see
`docs/css/af4e-selector-invalidation-parser-conformance-closeout.md`.
