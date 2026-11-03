# AH7: Authored HTML Resource-Reference Inventory

Date: 2026-11-03
Scope: synchronous, on-demand Browser observation of supported parser-created
HTML reference occurrences. No new resource loading or executable behavior.

## Ownership and observation

Browser owns discovery through `PageState::html_resource_references()`. It reads
the committed ordinary DOM and existing `DocumentAddressState` together under
one immutable borrow. AH4 supplies the document URL and authoritative active
base; AH7 does not inspect head metadata to select a base or reconstruct frozen
base history. The staged publication path remains unchanged.

The getter returns `None` without both a managed document address and DOM,
including after navigation reset and before an initial committed publication.
`Some([])` means an observable document has no supported occurrences. Committed
previews are observable without parser completion or rendering. Rejected/stale
publications cannot replace the observed DOM/base. Replacement and `Clear`
naturally change the next observation; there is no retained inventory to retire.

HTML owns names, namespaces, attributes, decoding and tree construction.
`addressing::BrowserUrl::resolve()` owns URL parsing and normalization. CSS,
Layout and Paint acquire no discovery or URL responsibilities.

## Output and identity

Each `HtmlResourceReference` carries materialized document-local `Id`, a fixed
source attribute name, `addressing::ResourceKind`, and `HtmlReferenceOutcome`:

- `Resolved { address, diagnostics }`: complete `ResourceAddress` and ordered
  recoverable `UrlDiagnostic` events, including repetitions.
- `EmptyReference`: the HTML inventory's exactly-empty outcome for the rows below.
  It is neither a fabricated URL-parser failure nor an effective load result.
- `Failed(UrlFailure)`: existing typed invalid-input/implementation failure,
  preceding diagnostics and explicit base provenance, without reclassification.

One classification supplies both the record kind and successful address kind.
Failures and empty occurrences retain their kind. An invalid reference never
terminates traversal or suppresses later occurrences. There is no inventory-wide
validity verdict or invented URL/source position.

IDs refer to the originating committed document only. They are not HTML `id`
attributes, parser `PatchKey`s, render IDs or cross-document identities. Callers
must not reuse them to identify nodes after replacement. The sole current
consumer is synchronous debugging, with no asynchronous request context.

Ordinary-tree preorder determines record order, not original source-byte order
after parser recovery. Each matching attribute produces one occurrence even if
its URL or retrieval key matches another. URL query/fragment information stays
complete. AH6 retrieval-key projection and operation capabilities are unchanged;
neither is used to deduplicate or gate the inventory.

## Supported-reference matrix

Every supported row requires an HTML-namespace element and an unnamespaced
attribute. `Resolve` means passing the stored DOM value unchanged to the active
base's existing resolver, retaining success or failure and every diagnostic.
It is not a claim of conforming HTML author input or implemented loading.

| Element / attribute | Conditions | Kind | Missing | Exactly empty | Whitespace-only | Status |
| --- | --- | --- | --- | --- | --- | --- |
| `link[href]` | `rel` has a stylesheet token | Stylesheet | No record | EmptyReference | Resolve | Supported |
| `img[src]` | Including picture/srcset contexts | Image | No record | EmptyReference | Resolve | Supported |
| `source[src]` | Immediate parent is HTML audio/video | Media | No record | EmptyReference | Resolve | Supported |
| `a[href]` | No further condition | Hyperlink | No record | Resolve | Resolve | Supported |
| `form[action]` | Authored action only | FormAction | No record | Complete document URL | Resolve | Supported |
| `iframe[src]` | Including when srcdoc is present | Document | No record | EmptyReference | Resolve | Supported authored reference |
| `script[src]` | Classic JavaScript or module classification below | Script | No record | EmptyReference | Resolve | Supported, always inert |

Other nonempty values use Resolve for each supported row. Empty and valueless
attribute syntax both have the same stored empty DOM value; AH7 does not recover
source spelling. There is no universal trimming operation. Nonempty whitespace
reaches the URL parser and can resolve against a hierarchical active base with
recoverable diagnostics; opaque bases can instead produce existing failures.
Non-ASCII whitespace is not silently removed.

Stylesheet rel tokens are separated only by HTML ASCII whitespace: U+0009,
U+000A, U+000C, U+000D and U+0020. Token matching is ASCII case-insensitive, not
substring matching. Duplicate stylesheet tokens still yield one href occurrence.
`media`, `disabled`, advisory `type` and alternate-sheet selection do not filter
this authored inventory.

The image src remains an authored occurrence alongside `srcset` or within
picture. No candidate is selected, and no srcset text is approximately parsed.
Media source occurrences are retained even when their parent has its own src or
their type/media hints would affect selection. Source src under picture or any
other immediate parent does not qualify.

Exactly empty form action clones the already-parsed complete document URL,
independently of base href; it does not replay old URL diagnostics. A missing
action has no authored occurrence, and whitespace-only action uses ordinary
resolution. No submitter overrides, method/body processing or submission occurs.

Iframe src records describe the authored attribute even when srcdoc would take
precedence during actual document selection. Srcdoc contents are not parsed or
traversed. Missing/empty src does not manufacture an about:blank dependency, and
parse failures are not rewritten into fallback navigations.

### Script classification

AH7 adds only `addressing::ResourceKind::Script`. It describes classic/module
JavaScript references and adds no transport route or execution capability.

- An explicitly empty type classifies as classic JavaScript.
- Without type, absent/empty language defaults to classic. Otherwise use the
  effective `text/` plus exact language value for JavaScript MIME essence matching.
- An explicit type matching a JavaScript MIME essence after the HTML-prescribed
  leading/trailing ASCII-whitespace stripping classifies as classic.
- ASCII-case-insensitive `module` classifies as module without general keyword
  trimming. Type takes precedence over language.
- The closed MIME Sniffing JavaScript essence list includes its legacy aliases;
  MIME parameters are not removed. AH7 adds no MIME parser or sniffer.
- Import maps, speculation rules, data blocks and unrecognized types produce no
  ordinary external-JavaScript record, even if src is present. Inline script
  contents and imports are not scanned. Async/defer/nomodule do not select or
  execute inventory records.

All Script records remain inert. An HTTP Script address still has AH6's Supported
retrieval-scheme classification; that does not imply a script loader or runtime.
Likewise parsed data/blob references remain Deferred, and javascript/unknown
schemes remain Unsupported operations rather than synthetic parse errors.

## Boundaries and compatibility

Traversal follows ordinary `children()` edges only, excluding AE10 template
contents. It visits foreign elements' ordinary children so HTML descendants at
SVG/MathML integration points remain discoverable. Foreign lookalikes and
namespaced attributes do not match HTML rows. Comments, text, doctypes and
processing instructions contribute no references.

The inventory resolves against the current committed active base. Existing
AH4 image/stylesheet associations retain their selected addresses across base-only
changes. These can therefore differ, intentionally: inventory is not a request,
selected-source or attachment observation. AH7 does not call reconciliation,
`Tab::discover_resources()`, ResourceManager, navigation or script preparation.

Legacy image/stylesheet helpers retain their existing trimming/empty handling,
and the existing stylesheet loader retains its existing rel-token helper.
Their optional results discard resolution diagnostics and cannot serve as AH7
outcomes. AH7's local ASCII rel predicate does not silently migrate those paths.
Existing loading commands, attachment identities and ordering remain unchanged.

## Debug integration and privacy

Call `Tab::document_address_debug_snapshot()` on the runtime Tab. It delegates to
the existing PageState document-address diagnostic, which now appends
`HtmlResourceReferences` using `html_resource_address_debug_snapshot()`. This is
an always-compiled on-demand diagnostic, not a test-only API, UI panel or logger.

The formatter computes one inventory, assigns snapshot-local element labels in
first-occurrence order, and reports fixed attribute/kind labels, outcomes,
diagnostics and both operation-capability classifications. Its temporary label
map is not retained or iterated for ordering. Source allocation IDs and HTML id
attribute values are not printed by the resource formatter.

Nested URL/address/failure Debug uses AH3/AH6 content redaction, including opaque
payloads, credentials and private URL components. Authored values, arbitrary
script types, exact serialized keys, hashes and sensitive lengths are absent.
Exact URL assertions are separate controlled semantic tests. The text remains
an internal regression contract, not a stable public schema. Invoking either
diagnostic emits no commands and changes no document, resource or render state.

## Explicit deferrals and non-goals

No picture/srcset parsing or responsive selection; no media selection/playback;
no nested-document loading or srcdoc parsing; no form submission; no script
fetching, module graphs or execution. Direct audio/video src, video poster,
track src, area href, submitter formaction, object/embed references, non-stylesheet
link types and foreign resource-reference discovery remain outside AH7.
Missing/empty/whitespace values of these deferred combinations produce no records.
Base href remains AH4 input, not a resource record.

No all-href/src scan, CSS extraction, loader migration, retained inventory,
revision service, extractor registry, dependency graph, scheduler, mutation
framework, AG schema/capture change or CI infrastructure is introduced. Network,
navigation, cache, redirects, cookies/storage, CORS/CSP/mixed-content enforcement
and broader AH completion remain outside this issue.

## Authority and regression coverage

- [AH1 ownership](ah1-url-origin-resource-addressing-contract.md),
  [AH2/AH3 resolution/privacy](ah2-typed-url-parsing-and-diagnostics.md),
  [AH4 document/base](ah4-document-url-and-base-state.md),
  [AH6 addressing](ah6-resource-addresses-and-identity-keys.md).
- [AE2 DOM model](../html5/ae2-parser-created-dom-node-model.md),
  [AE10 ordinary traversal](../html5/ae10-template-tree-construction-contract.md),
  [node identities](../html5/node-identity-contract.md).
- WHATWG HTML checked 2026-11-03: [link](https://html.spec.whatwg.org/multipage/semantics.html#the-link-element),
  [source](https://html.spec.whatwg.org/multipage/embedded-content.html#the-source-element),
  [image source sets](https://html.spec.whatwg.org/multipage/images.html#creating-a-source-set-from-attributes),
  [script classification](https://html.spec.whatwg.org/multipage/scripting.html#prepare-the-script-element),
  [form action](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#form-submission-algorithm),
  [iframe](https://html.spec.whatwg.org/multipage/iframe-embed-object.html#the-iframe-element),
  and [JavaScript MIME essence matching](https://mimesniff.spec.whatwg.org/#javascript-mime-type).

Parser-created DOM tests cover the matrix, all requested reference examples,
namespace/template exclusions, duplicates, complete fragments/queries,
successful diagnostics, failures and scheme capabilities. Tab tests observe
production publications, base-dependent inventories, preserved associations,
stale/rejected/replaced state, literal/redacted snapshots and independent labels.
`html_resource_inventory_is_observational` first accounts for normal production
stylesheet/image commands, then proves repeated inventory/debug calls emit no
additional commands and preserve document, resource, navigation and render state.

## Validation on 2026-11-03

| Level | Executed evidence | Result |
| --- | --- | --- |
| Targeted | `cargo test -p browser html_resource --locked` | 17 pass, including the production-publication observation smoke. |
| Targeted | `cargo test -p addressing --test resource_address --test resource_address_debug --locked` | Seven pass. |
| Existing contracts | Browser `document_addressing` and `resource_associations` filters | 17 and 14 pass respectively. |
| Subsystem | `cargo test -p browser --features conformance-observation --locked` | 350 unit and five integration tests pass. |
| Repository checks | Formatting and default/HTML5/hardening workspace lint | Pass, with no AH7 warning suppression. |
| CI-independent gates | Dependency enforcement (including 32 Python tests), seven AG CI cases, parser conformance/external/fatal/golden/real-page/script/WPT lanes, six deterministic fuzz smokes, debug/release default/HTML5 builds, both bench compilations, entity checks | Pass. |
| Full CI qualification | Default and HTML5 workspace test lanes | Not green: nine existing native macOS Chromium/process-ownership tests fail during ownership discovery, before AH7 observation. |

The initial `make ci` stopped on a needless-borrow lint in a new test helper;
that was corrected and all three lint configurations then passed. Gates resumed
from that point instead of repeating passed dependency/AG checks. The workspace
test failure persisted with host permissions: `live process executable unavailable
during ownership scan`, followed by incomplete cleanup/timeouts. Conformance
ownership code is unchanged by AH7. Remaining independent gates were run with
`make -k`, preserving the nonzero result; no tests were disabled or weakened.
The full Browser suite was also run directly because workspace execution stopped
in conformance before reaching it. Existing release-only debug-guard and fuzz
instrumentation warnings remain outside AH7. No windowed smoke was needed for
this on-demand diagnostic feature. Full CI remains an explicit qualification gap.
