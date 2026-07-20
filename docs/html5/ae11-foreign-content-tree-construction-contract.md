# AE11 Foreign-Content Tree-Construction Contract

Status: implementation contract for Milestone AE issue AE11.

Supported profile: `AE11-WHATWG-supported-token-profile-v1`.

## Immutable upstream provenance

AE11 uses the following immutable upstream revisions:

- WHATWG HTML: `85b40db7c40436be8d459e8f4ca2120e823c34f0`
  (`whatwg/html`, file `source`, SHA-256
  `0f50cff6018d38cb8878a5610d9de1e49ae64e0047ab26e6e569562e1229059b`).
- Web Platform Tests: `e4ea1706fa708c3ac4523c534a65160d1ab20db8`
  (`web-platform-tests/wpt`).

The WHATWG revision is the normative algorithm and table source. The WPT
revision supplies exact conformance cases adapted into Borrowser's deterministic
tree, patch, error, and whole-input/chunked-input fixtures. WPT expected trees
are translated only into Borrowser's versioned snapshot representation; local
tests add state, patch, parity, and rendering assertions without changing the
upstream input bytes.

### Normative WHATWG source locations

All locations below refer to the pinned `source` file at the exact commit above.
The line spans are review aids for that immutable file; the named algorithms and
anchors are authoritative.

| AE11 behavior | WHATWG algorithm/state/table | Pinned source location |
| --- | --- | --- |
| tokenizer duplicate removal and error | attribute-name state; `duplicate-attribute` | `#attribute-name-state`, around lines 142638-142668 |
| adjusted current node | adjusted current node algorithm | around lines 141264-141267 |
| special elements | stack-of-open-elements element categories | around lines 141279-141309 |
| scope barriers | element-in-scope algorithms | around lines 141329-141405 |
| Noah's Ark comparison | push onto the list of active formatting elements | `#noah`, around lines 141445-141466 |
| frameset state | frameset-ok flag | around lines 141588-141590 |
| CDATA dispatch | markup declaration open state | `#markup-declaration-open-state`, around lines 142925-142954 |
| CDATA tokenization | CDATA section, bracket, and end states | `#cdata-section-state`, around lines 143781-143838 |
| per-token foreign dispatch | tree-construction dispatcher | around lines 144303-144330 |
| MathML text integration points | MathML text integration-point definition | around lines 144340-144351 |
| HTML integration points | HTML integration-point definition | around lines 144353-144367 |
| MathML attribute adjustment | adjust MathML attributes | around lines 144688-144692 |
| SVG attribute adjustment | adjust SVG attributes | around lines 144694-144763 |
| qualified foreign attributes | adjust foreign attributes | around lines 144765-144793 |
| foreign characters and frameset-ok | parsing tokens in foreign content | `#parsing-main-inforeign`, around lines 148361-148397 |
| breakout and reprocessing | foreign-content breakout clauses | `#parsing-main-inforeign`, around lines 148403-148428 |
| SVG element-name adjustment | foreign ordinary-start-tag table | `#parsing-main-inforeign`, around lines 148431-148483 |
| foreign insertion and self-closing | foreign ordinary-start-tag steps | `#parsing-main-inforeign`, around lines 148484-148514 |
| static SVG script end handling | SVG script foreign end-tag branch | `#scriptForeignEndTag`, around lines 148516-148542 |
| foreign end tags | any-other-foreign-end-tag algorithm | `#parsing-main-inforeign`, around lines 148544-148566 |

### WPT fixture provenance

AE11's exact upstream fixture sources are:

- `html/syntax/parsing/resources/tests10.dat`, SHA-256
  `2d2624a819c323661e396d864ac23440053127b5ea7adb44a5904f5ceee5fa64`;
- `html/syntax/parsing/resources/svg.dat`, SHA-256
  `4c819b8dbdfbd98cfbce9a535a304b0a16597eb29ccc04077131a62810f309cd`;
- `html/syntax/parsing/resources/math.dat`, SHA-256
  `3c2ecc07272c175676ecfafa0cf6e18e74c3293075703702a4b7929fcb0d07bd`;
- `html/syntax/parsing/resources/namespace-sensitivity.dat`, SHA-256
  `318fbc9926eddf5863524f1503c711ddaea93b55bcbd1eaab71b7e354ddfeb09`.

The exact-byte AE11 imports activated in the WPT runner are `tests10.dat`
cases 1, 23, 35, and 52: basic SVG, XLink adjustment, `desc` integration,
and the direct `annotation-xml` plus `svg` exception. Their individual
provenance records contain exact input hashes, errors, documents, and
representation-only adaptations.

Cases 24-26, 27-34, 36-37, 40, 42-51, and 53-54 were reviewed at the same
immutable revision as normative regression-design sources. AE11 covers their
relevant algorithms with local unit, golden, malformed-recovery, parity, fuzz,
and rendering tests; it does not mislabel those local cases as verbatim WPT
imports.

The fragment-only cases in `svg.dat`, `math.dat`, and
`namespace-sensitivity.dat` are recorded for algorithm review and later fragment
coverage. AE11 does not activate them as parser fixtures because fragment parsing
is explicitly deferred. Equivalent full-document local cases may exercise the
same namespace/scope invariants but must be labelled local WHATWG-derived cases,
not verbatim WPT imports.

Each imported case must record its exact `#data` ordinal, original input bytes,
input SHA-256, upstream `#errors`, upstream `#document`, scripting flag, local
snapshot translation, and any additional local assertions.

## Supported-profile relationship and deviations

`AE11-WHATWG-supported-token-profile-v1` implements the pinned WHATWG foreign
tree-construction, namespace-boundary, adjustment, integration, CDATA, scope,
special-element, self-closing, breakout, and recovery algorithms over
Borrowser's supported token and parser-created DOM models.

The following deviations are explicit:

- Processing-instruction tokenizer states, tokens, parser-created nodes,
  patches, and materialization are deferred to AE12. Foreign dispatch remains
  exhaustive over Borrowser's current token enum but does not claim complete
  processing-instruction coverage.
- Parser-created SVG `script` elements retain namespace-correct static tree
  shape and deterministic self-closing/end-tag stack behavior. Parser pausing,
  insertion-point mutation, script nesting behavior, preparation, execution,
  fetching, and runtime side effects are excluded.
- Fragment parsing is deferred. The adjusted-current-node abstraction retains a
  typed future fragment-context source so the dispatcher and tokenizer CDATA
  boundary do not require redesign.
- SVG rendering, MathML layout/paint, XML parsing, public namespace-aware DOM
  APIs, scripting, events, resource loading, animation, and CSS namespace
  selector syntax are outside AE11.

CDATA section tokenization is included and is not a profile deviation.

## Ownership boundary

The tokenizer continues to produce namespace-less HTML tokens. Tree
construction owns namespace selection, adjusted-current-node semantics,
integration points, adjustment tables, foreign dispatch, foreign stack
recovery, and self-closing acknowledgement. The tokenizer receives only a
tree-builder-derived CDATA eligibility context before each token-granular pump.

The parser-created DOM model owns typed expanded element and attribute names.
CSS consumes namespace-aware names for matching and cascade input. Layout owns
both the decision to suppress unsupported foreign box subtrees and resolution
of supported HTML replaced-element presentation metadata from DOM semantics.
Layout extracts exact stored no-namespace DOM strings; Browser resource
integration performs image-URL preprocessing and resolution while Layout builds
the typed metadata. Paint/GFX consumes the resulting resolved image and
text-control presentation data; it does not inspect or normalize DOM
namespaces, names, or attributes and does not recreate namespace policy.

## Canonical names, identity, and extensibility

`crates/html/src/names.rs` is the single owner of `ElementNamespace`,
`ExpandedElementName`, `InternedLocalName`, and `NameInterner`. HTML, Browser,
CSS, Layout, and GFX consume these types; no downstream crate defines a parallel
namespace enum. Every general element constructor requires an explicit
expanded name. There is no absent-namespace state and no implicit HTML default.

`NodeId`, materialized `Id`, and `PatchKey` remain stable numeric identities.
They never encode namespace or spelling. Element semantic identity is
`(ElementNamespace, exact local name)`. The per-document name interner folds
HTML-tokenized input on entry, interns adjusted SVG names such as
`foreignObject`, `linearGradient`, and `feGaussianBlur` exactly, and interns
unknown foreign names exactly once per document. Equality and hashing use the
canonical exact string. Stack caches use the opaque
`ExpandedNameKey(ElementNamespace, NameAtomId)` projection of that same
identity. `NameAtomId` encodes its parser-session interner domain, the stack is
bound to that domain before production parsing begins, and cross-domain atoms
are rejected before stack mutation; raw atom indices are never compared across
sessions. Snapshots recover the exact string.

The AE11 namespace enum intentionally denotes the well-known parser-created
HTML/SVG/MathML namespaces. A later XML/public-DOM milestone may introduce an
extensible interned namespace-URI identifier which wraps or maps these
well-known constants. That future storage extension does not require arbitrary
namespace strings at HTML tree-builder call sites or a duplicate type in every
consumer crate.

## Parser-created attribute contract

`crates/html/src/attributes.rs` owns `AttributeNamespace`,
`QualifiedAttributeName`, and `ParserCreatedAttribute`. Private qualified-name
variants and smart constructors make the supported states valid by
construction:

- unqualified: namespace `None`, no prefix;
- XML: namespace `Xml`, prefix `xml`;
- XLink: namespace `XLink`, prefix `xlink`;
- default XMLNS: namespace `Xmlns`, no prefix, local name `xmlns`;
- prefixed XMLNS: namespace `Xmlns`, prefix `xmlns`.

Unknown colon-containing names not present in the pinned foreign-attribute
table remain deterministic unqualified local names. Namespace declarations are
stored but never change HTML parser namespace selection. Invalid
namespace/prefix combinations cannot be constructed through parser, patch,
`DomStore`, or materialization APIs.

Every parser-created attribute value is a `String`. Valueless syntax and
explicitly empty syntax both become `""`; no missing-versus-empty distinction
survives into DOM, patches, CSS, materialization, or snapshots. Tokenizer
duplicate detection retains the first normalized name and emits its standard
`duplicate-attribute` parse error. A synthetic post-adjustment expanded-name
collision is first-wins hardening recorded only by an internal counter; it does
not add a non-standard HTML parse error.

Storage, patches, diffing, materialization, and snapshots preserve the first-
surviving encounter order. Exact DOM/patch comparison is ordered and prefix-
preserving. The HTML parser's Noah's Ark “same attributes” comparison is a
separate order-insensitive one-to-one comparison of namespace, local name, and
value; prefix does not participate.

## Snapshot and transport contract

`html5-dom-v2` begins its payload with `#dom-snapshot-v2` and exposes the
namespace and exact local name of every element. Attributes are emitted in
stored order as structured namespace, prefix, local-name, and value fields.
`html5-dompatch-v2` applies the same representation to `CreateElement` and
`SetAttributes`. Patch validation, Browser `DomStore`, diffing, and
materialization reject namespace-blind construction instead of defaulting it to
HTML.

## Adjusted current node and tokenizer CDATA boundary

Foreign dispatch and tokenizer CDATA eligibility use a semantic
`AdjustedCurrentNode` view containing an optional stack key, expanded name,
ordered attributes, and a typed source (`StackCurrent` today, future fragment
context later). Callers do not substitute a raw stack entry. Fragment parsing
is excluded, but a future context element need not have a `PatchKey` or belong
to the open-elements stack.

Before every incremental tokenizer pump, the tree builder's
`prepare_tokenizer_pump` supplies only the adjusted-current-node namespace.
Tree construction still owns the algorithm; the tokenizer owns the markup-
declaration-open decision and CDATA section, bracket, and end states. `<![CDATA[`
in HTML emits the existing HTML parse error/bogus-comment behavior. In SVG or
MathML it emits ordinary character tokens. Template contents change insertion
location, not the adjusted-current-node namespace. Chunk boundaries cannot
change the decision. CDATA U+0000 output enters the ordinary foreign-character
replacement/error path.

## Per-token dispatch and namespace selection

The active HTML insertion mode never becomes a foreign or XML mode. Before
ordinary insertion-mode dispatch, each supported token is classified:

- empty stack, HTML adjusted current node, or EOF: ordinary HTML dispatch;
- MathML `mi`, `mo`, `mn`, `ms`, or `mtext`: character tokens and start tags
  use HTML dispatch except `mglyph` and `malignmark` starts;
- MathML `annotation-xml` plus a start tag named `svg`: ordinary HTML dispatch,
  whose HTML `svg` entry creates an SVG element;
- SVG `foreignObject`, `desc`, or `title`: character and start-tag tokens use
  HTML dispatch;
- MathML `annotation-xml` with unqualified `encoding` equal, ASCII-
  insensitively, to `text/html` or `application/xhtml+xml`: character and
  start-tag tokens use HTML dispatch;
- every other supported token: foreign processing.

Integration points are per-token conditions, not persistent nested HTML modes.
Namespace selection comes from the HTML entry algorithm or the adjusted current
node. It never comes from the actual insertion parent, template contents,
foster parent, adjusted insertion location, patch parent, or materialization
parent. Therefore `<svg><math>` creates SVG `math`, `<math><svg>` creates
MathML `svg`, and only the specified `annotation-xml` exception changes the
latter boundary.

## Complete pinned tables and classifications

`tree_builder/foreign/tables.rs` contains the complete pinned WHATWG tables:
37 SVG tag-name adjustments, 58 SVG attribute adjustments, MathML
`definitionurl` to `definitionURL`, the XML/XMLNS/XLink foreign-attribute
mappings, 44 breakout start tags, conditional `font` breakout for `color`,
`face`, or `size`, and breakout end tags `br` and `p`. Tables are immutable,
centralized, binary-searched where applicable, sorted/unique tested, and never
distributed as ad-hoc dispatch branches. Unknown names retain their inherited
foreign namespace and deterministic tokenized spelling.

General scope barriers and special-element classification use expanded names.
The MathML set includes `mi`, `mo`, `mn`, `ms`, `mtext`, and
`annotation-xml`; SVG includes `foreignObject`, `desc`, and `title`. HTML
special, implied-end-tag, template, form, select, table, generic-end-tag,
adoption-agency furthest-block, and stack-clearing behavior explicitly requires
the HTML namespace where specified.

## Foreign processing and recovery

SVG/MathML HTML entry, ordinary descendants, nested boundaries, and unknown
foreign elements use explicit selected namespaces while adjusted insertion
locations continue to own only placement. Foreign self-closing starts insert,
pop, and acknowledge the flag without changing HTML void-element rules.
Comments insert normally; doctypes emit the foreign-content doctype error and
are ignored.

Foreign character handling owns the exact `frameset-ok` transition:

- U+0000 emits the foreign null error, inserts U+FFFD, and adds no frameset
  transition;
- ASCII whitespace inserts and leaves `frameset-ok` unchanged;
- every other character inserts and sets `frameset-ok` to not ok.

For a foreign end tag, the current foreign local name is compared ASCII-
insensitively with the token and one mismatch error is emitted only when they
differ. The stack is walked backward deterministically. A matching foreign
local name pops through the match. Reaching the root guard returns safely.
Reaching an HTML element reprocesses the same token through the active HTML
mode without a synthetic boundary error; that handler may independently emit
its own specified error.

Breakout emits `unexpected-html-token-in-foreign-content`, pops to HTML or a
supported integration point, then reprocesses the identical token through the
still-active HTML insertion mode. Exact-state fingerprints and bounded
reprocessing guards require every retry to change state or terminate, including
template and table delegation.

## CSS, rendering, and runtime consumers

The selector/style index retains the complete mixed tree. Author selectors keep
their existing no-default-namespace semantics; HTML element names match ASCII-
insensitively while foreign canonical names match case-sensitively. Unprefixed
attribute selectors query only `AttributeNamespace::None`. Internal UA rule
groups carry `SelectorNamespaceConstraint::Exact(Html)` through selector lists,
combinators, every compound (including universal and typeless compounds), and
all currently supported recursion points. Future SVG/MathML UA groups can use
another exact constraint without redesigning cascade; CSS `@namespace` and
namespace-selector syntax remain deferred.

Browser traversal, resource/style discovery, forms, page background,
replacement, inline classification, templates, and tables gate HTML behavior
on `ElementNamespace::Html`. They preserve rather than flatten foreign nodes.
Browser-owned retained render identities continue to cover the complete active
DOM and do not imply box participation. CSS-owned resolved and computed document-style entries
retain both namespace and canonical local name; incremental reuse and
style-tree handoff reject a same-spelled element in a different namespace.

Layout owns the single temporary rendering policy through
`BoxGenerationDecision::SuppressSubtree(BoxSuppressionReason::UnsupportedForeignNamespace(_))`.
The complete DOM and
style subtree remains present, including integration-point HTML descendants,
but no boxes are generated beneath an unsupported SVG/MathML root and Paint
receives no such boxes. Browser and Paint do not repeat this decision. A future
supported foreign subset can return `Generate` without changing the API's
meaning.

For supported HTML replaced elements, Layout produces the typed
`ReplacedElementPresentation` handoff. `ImagePresentation` contains a
Browser-resource-resolved source and alternative text;
`TextControlPresentation` contains generated placeholder text. Layout retains
`alt` and `placeholder` exactly as stored: absent is `None`, present-empty is
`Some("")`, and surrounding whitespace, case, and Unicode are unchanged.
Layout also passes the exact stored `src` string to the Browser resource
provider. Browser alone strips the five HTML ASCII-whitespace code points
(TAB, LF, FORM FEED, CR, and SPACE) surrounding that URL input, rejects an
empty result, parses absolute URLs, and joins relative URLs against document
context. Broader Unicode whitespace is not stripped. The resulting values are
retained unchanged as Layout-owned artifact data. Paint/GFX only performs
resource-state lookup and draws the already classified presentation; it does
not normalize the typed strings. Foreign `img`, `input`, and `textarea`
lookalikes never receive this HTML presentation metadata. Complete HTML
placeholder rendering and accessibility semantics remain deferred.

The stack cache retains one semantic expanded-name model. Its `name_counts`
storage is a `Vec<(ExpandedNameKey, usize)>`; lookup and update are linear in
the number of distinct expanded names currently tracked. `ExpandedNameKey` is
an opaque, interner-domain-bound projection of the canonical expanded name, not
a second element-name model. This representation remains selected for AE11
because the representative bounded mixed-content workload measured 5.490707
scan steps per lookup/update operation, below the approved limit of 32, while
avoiding a second map/interner identity and preserving exact cache/live-stack
invariants.

The deliberately adversarial deep-distinct workload recorded approximately
384.751460 scan steps per operation at distinct-name high-water 770 and stack
depth 770. That result is linear, is not represented as constant-time behavior,
and did not regress against the immutable pre-AE11 baseline: its candidate
median was 1,321,083 ns versus 2,237,667 ns at baseline (40.96% faster). The
current parser resource ceiling (`max_open_elements_depth = 1024` by default)
bounds the supported stack depth. An indexed cache should be reconsidered only
if a representative production corpus exceeds 32 scan steps per operation, an
ordinary or mixed workload exceeds the approved regression threshold, or
production profiling shows material name-cache cost. Any future indexed cache
must use the same canonical expanded-name/interner domain and must not create a
second semantic element-name identity.

The immutable comparison baseline is
`3db23a15ab640be8c038dba36a030c6c95ac9e59`; baseline and candidate use the
same Rust toolchain, release profile, feature set, inputs, warm-up count, and
measured-iteration count and execute sequentially on the same hardware. The
mandatory thresholds are at most 10% median regression for ordinary workloads,
at most 15% for mixed workloads, and at most 32 name-cache scan steps per
operation for the representative bounded mixed workload. Exceeding any
threshold is a review stop requiring profiling and an AE11 fix or explicit
architectural decision; a follow-up issue alone cannot waive the gate. The
in-process mixed-versus-ordinary wall-clock ratio is only an additional
resource/scheduler sanity guard and is not regression evidence.

## AE12 follow-up

Milestone: **AE — HTML Tokenizer, Tree Construction, and DOM Construction
Conformance**

Issue: **AE12 — Add HTML processing-instruction tokenization and parser-created
nodes**

Implement the current HTML processing-instruction tokenizer states, typed token
representation, parser-created `ProcessingInstruction` node, tree-construction
insertion behavior, patch transport, materialization, deterministic snapshots,
parse errors, and whole-input/chunked-input parity. Cover applicable ordinary
HTML insertion modes and foreign-content processing without introducing an XML
parser, XML namespace resolution, scripting, resource processing, or public
DOM mutation APIs.
