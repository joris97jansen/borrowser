# AJ6: Passive stylesheet dependency discovery

Issue: [#1137](https://github.com/joris97jansen/borrowser/issues/1137).
Approved implementation base: `3ea2f4b07ec521afb88858954ca3bc9d7e08b3e0`, tree
`e9ea16a31f435b79c10163a86af641f76354edb3`.

Status: uncommitted implementation candidate. Exact byte/mode manifests and actual
local qualification results belong to the external review packet accompanying the
candidate. This document does not certify independent implementation approval,
hosted qualification, publication, merge, issue closure, or Milestone AJ completion.

## Independent production acquisition

[AJ7](aj7-deterministic-stylesheet-acquisition.md) changes production retention,
acquisition and failure handling without invoking this observer or adopting its
limits. Production keeps Unicode rel splitting and `str::trim` href preprocessing;
missing/empty/invalid/unsupported sources retain ordered failure positions.
Frozen selected addresses and source history survive retained attachments.
AJ7 borrowed fetch views use the same committed-document coherence gate, but
neither observer schedules loading or becomes an attachment authority.

## Ownership and entry points

Browser owns on-demand discovery and candidate order. HTML supplies the committed
parser-created DOM; AH supplies the authoritative active base, URL outcomes and
retrieval capabilities. CSS supplies effective style-attribute eligibility and its
current media classification. Layout and Paint acquire no discovery responsibility.

The feature-gated `Tab::observe_parsed_document()` view exposes
`ParsedDocumentObservation::stylesheet_discovery()`. The ordinary-build
`Tab::stylesheet_discovery_debug_snapshot()` uses that same view and semantic core.
Neither operation reconciles attachments, parses CSS, fetches, schedules commands,
mutates state, invalidates rendering, or retains an inventory/cache.

The existing production loader remains independent and unchanged: its Unicode rel
splitting, href trimming, request eligibility, inline parsing, attachment identity,
address retention, dispatch, retirement, settlement and preview rendering are not
migrated to AJ6 semantics. See [AJ1](aj1-static-document-loading-and-page-assembly-contract.md)
and [AH7](ah7-html-resource-reference-discovery.md) for the distinct existing paths.

## Committed-state and lifecycle authority

Only the existing parsed-document observation admits committed inputs. PageState
has DOM/mode/addresses but does not own the Tab's handle/version. There is no
PageState forwarding method or competing lifecycle machine.

For presence tuple `(DOM, handle, mode, addresses)`:

- `0000`: no committed document; return `Ok(None)`.
- `0001`: prepared addresses only; return `Ok(None)`.
- `1111`: admit the borrowed DOM and active base to the pure semantic core.
- All other thirteen tuples: `CommittedStateInconsistent`.

The gate is independent of existing `phase()`, `tree_scope()` and `address_scope()`.
A Failed phase cannot mask an inconsistent tuple. Conversely, coherent committed
state can coexist with `InvalidEvidence` from body/terminal evidence; discovery can
describe that tree while preserving the invalid lifecycle metadata. It does not
certify parsing, input settlement or assembly.

Committed previews, final documents, retained failed previews and the old active
document during pending replacement remain observable with their existing scopes.
Response activation leaves prepared-only state until publication. Atomic accepted
publication remains the handle/version/DOM/mode/base authority; observation does
not rematerialize the DOM or introduce another version check/authority.

`Ok(Some(inventory))` with zero records is a complete empty discovery, distinct
from no committed document. Every successful inventory is complete. A semantic
failure returns an error without a successful prefix.

## Membership, order and identity

Walk ordinary children in preorder. Associated template-content fragments are
excluded; the template host remains an ordinary element. Foreign ordinary children
are traversed so genuine HTML descendants at integration points can participate.

| Source | Occurrence | Authored candidate dependency position |
| --- | --- | --- |
| Every HTML link | Relation and contextual attributes | Only with an ASCII-tokenized stylesheet relationship |
| Every HTML style | Inline block, including empty/unsupported type | Yes |
| AF-eligible style attribute | Separate element-attached observation | No |
| Foreign link/style lookalike | No HTML link/block occurrence; its supported style attribute can qualify | No |

At one element, link/block precedes its style-attribute occurrence. All records get
a checked zero-based `OccurrenceIndex`; only stylesheet-bearing links and HTML
style blocks get a separate checked `StylesheetDependencyOrder`. Missing, invalid,
disabled, alternate and otherwise unsupported candidates retain their positions.
Duplicates remain separate even when complete URLs, retrieval keys or text match.

These are authored candidate positions, not CSSOM indices, Browser attachment slots,
CSS source IDs or effective cascade positions. Materialized node IDs remain bound
to the originating document; the existing view exposes handle/version separately.
The inventory borrows the Tab's document, preventing mutation while correlated
identities/source values are in use. No request generation is inferred.

[DOM tree order](https://dom.spec.whatwg.org/#concept-tree-order) controls the
result, including parser recovery, rather than source-byte or completion order.

## Independent facts and applicability

Each dimension survives independently; no aggregate active/eligible/fetchable
verdict or arbitrary winning unsupported reason is created.

| Dimension | Contract |
| --- | --- |
| rel | HT, LF, FF, CR and SPACE separate tokens; keywords match ASCII-insensitively. Additional/duplicate tokens do not cancel or duplicate stylesheet membership. NBSP and VT do not split. |
| Other link relationships | Retain raw facts, do not resolve href or allocate dependency position; stylesheet-specific applicability remains unassessed. |
| href missing / exactly empty | Explicit Missing / Empty, without an AH call or fabricated parser error. |
| Every other stylesheet href | Pass stored DOM value unchanged to AH, including whitespace-only values. No Browser Unicode trim or control removal. |
| AH success/failure | Preserve complete addresses and every ordered diagnostic, or exact typed failure with its base provenance. Unsupported retrieval is not URL failure. |
| Link type | Raw advisory hint, unevaluated; no new MIME parser or response applicability verdict. |
| Style type | Missing, empty or exact ASCII-insensitive text/css has CSS-type input meaning; other values remain explicit. No trimming or MIME parameter removal. |
| Title / alternate | Missing, empty or named title is retained; whitespace-only nonempty title is named. Named/preferred and alternate set selection remain unsupported. |
| Link disabled | Boolean presence, including disabled=false, observed without enforcing it. |
| Media | Exact optional raw input plus direct CSS subset classification: absent, empty and five-ASCII-whitespace-only active; all other inputs, including all/screen/VT/NBSP, deferred/inactive. |
| crossorigin / integrity / referrerpolicy | Independent absent/present-empty/present facts; explicitly unenforced. Other link relationships are unassessed. |
| Link-only attributes on style | Tracked disabled/policy values retained as inapplicable content attributes, not processed as link attributes. |
| Style attributes | CSS get_inline_style supplies first effective unqualified input, including ordinary foreign elements. Empty/malformed/large values remain observed without parsing or predicting contribution. |

HTML preprocessing has already occurred: observations retain DOM values, not raw
network spelling. For example CRLF has become LF, and default tokenizer attribute
limits may have truncated values. Controlled resource tests use existing parser
configuration where necessary; AJ6 does not alter those policies.

The tiny AH7 ASCII token iterator is shared concretely. The loader's global token
helper is untouched. CSS owns media meaning and element-attached precedence; AH
owns URL preprocessing and outcomes. See [AF5](../css/af5-stylesheet-rule-collection-source-order-contract.md),
[HTML link processing](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element),
[style-block processing](https://html.spec.whatwg.org/multipage/semantics.html#update-a-style-block)
and [CSS style attributes](https://www.w3.org/TR/css-style-attr/#interpret).

Current authored hrefs resolve against the authoritative current active base.
An unchanged loader attachment may retain an address selected before a base change;
that intentional difference is not repaired by observation. Frozen-base history
and attachment lifetime remain AH/Browser authorities.

## Rust ownership and failure boundary

The inventory owns a bounded vector of read-only occurrences and owned AH outcomes.
It borrows authored attributes, inline text child slices and style-attribute values.
Inline content is exposed as direct text chunks without concatenation or separators.
Source-bearing values implement neither raw Debug nor automatic inventory Clone.
Exact APIs/types are reexported only with conformance-observation; ordinary builds
expose the bounded redacted snapshot and its source-free allocation error.

Private constructors maintain relation/href/order consistency. Index newtypes have
no unchecked public constructors or conversions to CSS/attachment/request domains.
Semantic errors separately identify committed-state/identity inconsistency,
configured limits, arithmetic/index overflow and fallible vector reservations.
AH invalid-input, capacity and backend failures stay per-occurrence outcomes.

## Resource policy and delegated AH costs

| Semantic admission | Default |
| --- | --- |
| Ordinary nodes including document root | 65,537 |
| Ordinary edge depth, root zero | 1,024 |
| Occurrences | 8,192 |
| Admitted DOM attributes | 262,144 |
| Aggregate classification input | 10 MiB |
| One href plus serialized-base pair | 64 KiB |
| Aggregate href plus serialized-base pairs | 10 MiB |

These are observer policies. The node scale resembles the default parser's
65,536 non-document nodes plus root; parser open-element depth is not a proof of
final DOM depth. CSS sheet limits, network body limits and per-publication mutation
limits do not bound the cumulative DOM or observation heap.

Admission precedes work: node/depth, attribute count, relevant classification input,
record/index/storage, then URL pair/aggregate charge before AH. Charge the base for
every attempted resolution. Use iterative ancestor frames, fetching one child at a
time, and checked amortized fallible growth with requests capped by remaining
capacity. An exact boundary succeeds if no required work remains; exceeding it
fails the whole semantic observation. A valid larger document remains valid and
continues its unchanged production loading.

Observer work is proportional to admitted node/attribute/classification work plus
delegated AH operations. Frame/record requests are bounded separately. Borrowed CSS
and uninterpreted source values are not scanned/copied for their contents.

Input charges are not heap-byte or time bounds. AH can expand canonical output,
retain many repeated diagnostics, use temporary host/IDNA allocations, and clone the
whole explicit base on failure, including a failing absolute reference. Aggregate
failure-base logical string lengths are bounded by charged base bytes, excluding
capacities/overhead. Full canonical/address/diagnostic allocation and delegated time
are not equated with the 64-KiB/10-MiB inputs. AH's existing allocation/panic
qualification remains [explicit](ah2-typed-url-parsing-and-diagnostics.md#outcomes-failures-and-diagnostics).
AJ6 does not modify AH, truncate diagnostics or add a global memory framework.

## Redacted output and completeness

Formatting uses the same complete semantic result with independent limits:
8,192 output bytes including a 512-byte reserved footer, 16 displayed occurrences,
16 URL diagnostic events total and fixed 1,024-byte atomic record scratch storage.
Output reservation is fallible; default diagnostics contain fixed labels, indices,
snapshot-local owner labels, capability categories and bounded diagnostic identities.
They never print authored values, private URL components/arbitrary schemes, CSS,
runtime IDs, pointers, hashes or private input lengths.

The header reports existing lifecycle/tree/address scope and pending navigation.
Availability distinguishes no committed document, successful inventory and semantic
failure. Semantic completeness and formatting completeness are separate.

Record-cap omission shows a prefix. Diagnostic-cap omission keeps subsequent
structural records where space permits. Byte/scratch failure omits the current
atomic record and stops records. The reserved footer reports shown/total counts,
actual limiter flags and diagnostics omitted, distinguishing omission caused by a
different limiter. Semantic failure exposes no partial inventory or false zero result.
A complete error summary may have complete formatting and incomplete semantics.
Totals and diagnostic omission are `not-applicable` without a committed document
(including prepared-only addressing), and `unavailable` after semantic failure,
even if internal work preceded the error. Only a successful inventory reports
exact totals; a complete empty inventory reports zero. Shown counts describe
formatted records and never expose a discarded semantic prefix.

## Validation and exclusions

Regression cohorts exercise actual parser/materializer/publication/runtime seams,
coherence tuples, preview/final/failure/pending state, current base versus retained
attachment history, duplicates and ordering, exact classification/URL outcomes,
namespace/template/recovery, observation purity, redaction and resource boundaries.
Canonical expansion, diagnostic-heavy inputs, repeated bases, bounded growth and
borrowed content are measured independently. Controlled exact expectations are
separate from redacted snapshots. Candidate-bound receipts inventory selected and
executed tests in default/observation and debug/release configurations.

L3 is not applicable. Applicable local L1/L2 results are in the external review
packet; exact-source hosted L4 remains a later publication gate. AWS is not needed;
zero live operations are part of this issue. Existing AG projections do not expose
these records and do not justify a new capture/cloud campaign.

AJ7 fetching/response handling, AJ8 CSS decoding/parsing/static sets, AJ9 rendering
assembly and AJ10 resource-backed conformance/closeout remain separate. No real
media evaluator, alternate/named selection, imports, policy enforcement, loader
trimming/tokenization correction, navigation/events, image/font loading, JS or
other future capability is implemented here.

## AJ8 participation qualification

[AJ8](aj8-ordered-static-stylesheet-set.md) adds host eligibility and conservative
participation to production attachments. AJ6 remains passive authored discovery;
its candidate/dependency positions do not authorize acquisition. Initially disabled,
alternate and named external attachments request nothing. Media classification is
still CSS-owned, and inactive/partial authored evidence remains observable. Earlier
AJ6 qualification results above remain historical.
