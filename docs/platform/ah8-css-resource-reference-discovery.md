# AH8: Authored CSS Resource References And Addresses

Status at the 2026-11-04 local qualification checkpoint: reviewed Borrowser
candidate with the published immutable parser revision integrated after required
source and dependency-graph qualification. The local
Borrowser CI attempt failed on native host tests; AH8 is open and not merge-ready.
The parser source and qualification controls were published; Borrowser was
uncommitted and unpublished at that checkpoint. Historical local-override
evidence below is distinct from the new
immutable-source evidence.

## Ownership and observable outcome

CSS produces authored resource occurrences from existing parsed declarations
before cascade filtering. Browser supplies attachment/document provenance;
`addressing` parses and resolves URLs and classifies resource capabilities.
Layout, Paint, the stylesheet assembly runtime and transport gain no URL logic.

`Tab::document_address_debug_snapshot()` appends `CssResourceReferences` to the
existing document/base and HTML diagnostic. Observation is on demand: no retained
inventory, generation service, style invalidation, additional command, or I/O.
It is an authored inventory, not effective dependencies, selected candidates,
computed CSS values, or issued requests. Discovery does not change acceptance.
The existing 35 longhands and `outline` shorthand still accept no URL grammar.

CSS APIs live in `css::resource_references`:

- `discover_stylesheet_resource_references(&StylesheetParse, &ParseOptions)`
  borrows the retained parse and its input;
- `discover_style_attribute_resource_references(&str, &ParseOptions)` invokes
  the existing model declaration parser and owns its temporary input.

Reports retain source-bound spans, ordered references, original syntax
diagnostics/statistics and bounded typed feature notices. `Cow<CssInput>` keeps
attribute spans valid without copying retained stylesheet inputs. Values and
reports deliberately have no derived Debug that could expose authored contents.
The caller supplies the applicable parse options; Browser uses the same default
stylesheet/style-attribute options as its source parsing paths.

## Supported discovery matrix

| Context | Authored observation | Addressing |
| --- | --- | --- |
| Ordinary parsed declaration | Each direct URL token or quoted URL function, including duplicates | Eligible literal resolves independently of declaration acceptance. |
| `background`, `background-image`, `border-image`, `border-image-source`, `list-style`, `list-style-image`, `content` | Image role for URL forms | Role is purpose, not complete grammar validation. |
| `cursor`; `mask` / `mask-image` | Cursor; Mask role | Existing platform Image kind, with precise CSS role retained. |
| Other ordinary properties | Unknown role | No guessed property support. |
| External sheet | Available parse belonging to its real attachment | Retained admitted response base. |
| HTML `style` element | Existing parse from stylesheet reconciliation | Current committed active document base at observation. |
| Effective unqualified style attribute | Fresh declaration-list parse during observation | Current committed active document base at that parse. Existing CSS attribute-name/namespace policy is reused. |
| `@import` | Leading bare string and explicit URL forms, StylesheetImport role | Unsupported context; no resolution or following imports. Conditions/placement are not validated. |
| `@font-face` | Explicit URL forms in preserved components, contextual Font role | Unsupported; no descriptor validation, `local()` interpretation, source selection or loading. |
| `image-set()` / `-webkit-image-set()` | Feature notice and explicit URL forms | Unsupported; candidate strings, densities, types and selection are uninterpreted. |
| Custom properties | Explicit URL forms plus CustomProperty context | Never resolved at the definition site. |
| Declarations containing `var()`, `env()`, `attr()` | Substitution context across the declaration; explicit fallback URL forms remain visible | Conservatively unresolved. Substitution is not implemented. |
| Gradients without URL forms | No resource occurrence/address | No gradient evaluation or URL failure. A separate sibling direct URL can resolve. |
| Unknown functions / preserved grouping or unknown at-rules | Explicit URL forms with unsupported contexts | No semantic flattening, string extraction or dependencies. |
| Nested/malformed constructs discarded by existing recovery | Existing syntax diagnostics | No reconstruction from raw source. |
| Comments / ordinary strings / unrelated BadString | No resource occurrence | Exceptions: recognized quoted URL arguments and leading import strings. |

Image, Cursor and Mask roles map to existing `ResourceKind::Image`; Font,
StylesheetImport and Unknown map to existing Font, Stylesheet and Unknown. No new
platform kinds or transport routes are introduced.

Existing registries, specified parsers and shorthand expansion supply declaration
acceptance. Existing selector matchability and stylesheet condition classification
are reported independently. No selector matching or winner selection occurs.
Effective participation is explicitly Unevaluated.

## Decoding, shapes, contexts and precedence

The existing tokenizer/model is authoritative. Shared CSS escaped-code-point
consumption handles hexadecimal escapes, terminator whitespace, escaped
punctuation and replacement of invalid scalar escapes. Discovery decodes semantic
names/payloads from the source-preserving model once; it never reparses serialized
CSS or substitutes a URL serialization for the original decoded reference.

String continuations are valid only in strings. Backslash-newline remains invalid
in identifiers/unquoted URLs. BadString outside a URL-bearing context remains an
ordinary syntax diagnostic. EOF recovery retains usable URL/string/function
payloads with diagnostics, rather than throwing away their values. Actual bad
URL/string forms retain the existing recovery boundaries. These changes add no
property acceptance and do not claim full CSS Syntax conformance.

Every reference retains its shape (`Literal`, `Empty`, `LocalFragment`,
`InvalidCss`) separately from ordered enclosing blocking contexts. Source
incompleteness remains separate in report statistics. Resolution precedence is:

1. Incomplete source: IncompleteSource.
2. Any blocking context: BlockedContext.
3. InvalidCss, Empty or LocalFragment: corresponding non-address outcome.
4. Eligible Literal: invoke addressing with the original decoded value.

All underlying facts and syntax diagnostics remain visible. Thus custom
`url('#mask')` retains LocalFragment and CustomProperty; an empty import retains
Empty and Import; malformed URLs in unsupported rules retain InvalidCss and
UnsupportedRule. No single outcome erases those overlaps.

Empty means exactly empty after CSS decoding, not arbitrary quoted whitespace.
It cannot resolve into the stylesheet/document address. A leading `#` retains
local-reference meaning without address construction or target binding. External
references containing fragments retain their complete resolved URLs. Retrieval
identity remains the existing explicit fragmentless projection.

Traversal is iterative over retained components; it does not scan comments or
ordinary strings. Existing parser input/token/container/nesting limits bound the
retained input. Feature notices use the configured diagnostic limit; exhausting
that optional reporting budget sets `feature_notices_truncated`, exposed separately
in the report and debug output. It does not change parser/source completeness,
reference traversal or address outcomes. Discovery-added syntax diagnostics also
respect their reporting cap without marking the source incomplete. `stats.hit_limit`
retains the parser/source limit fact; genuinely partial sources never claim
definitive addressing coverage. Discovery visits all retained components without
a separate reference-traversal cap. Source order and duplicate occurrences survive.

## External response provenance

The admitted `NetworkResponseInfo` remains immutable through Receiving,
validated NetworkDone and AwaitingRuntime. A decoded block must pass existing
request, generation, slot, MIME, completion and block-progress checks before the
same admitted response is passed through PageState into stylesheet installation.
The runtime event URL continues to authenticate the issued request.

Each external attachment retains its parsed nonempty admitted `final_url`, with
base-construction diagnostics, or its typed parse failure. Empty final URL means
missing provenance. There is no `display_url()`/request/document-base fallback.
Missing/invalid base provenance does not fail an otherwise accepted CSS load.
The original authored href, complete selected address, issued key, request
identity and response metadata keep their existing responsibilities.

This is the effective base for AH8 resource resolution. It is distinct from
original-request provenance and CSSOM's first-request `location` vocabulary;
AH8 does not implement CSSOM location or redirect processing. It resolves AH1
and AF5's handoff question for this authored addressing consumer.

Base-only document changes, moves and media updates retain an external sheet's
base. Source changes, retirement, Clear, replacement and navigation retire it
with the attachment. Accepted previews retain loaded provenance; validated
network completions waiting for CSS may still install under existing failure
rules. Stale events and rejected publications cannot publish speculative bases.

## Inline observation timing

A style element reuses its existing parsed text but obtains the current committed
AH4 active document base when observed. A style attribute is freshly parsed from
the current committed DOM value at observation and uses that same current base.
Subsequent observations can therefore change inline addresses after AH4 base
updates while external sheet addresses stay fixed. This does not retroactively
rebase computed values, restart requests, simulate setters or implement CSSOM
mutation. AH4 still owns fallback/frozen selection and publication semantics.

Sources follow ordinary DOM preorder, excluding template contents: attribute
first, then the owner's actual stylesheet attachment. CSS references follow
source/component order. A temporary owner lookup is never iterated for output;
equal addresses do not merge references, slots or CSS source identities.

## Unavailable base and parser independence

A missing/invalid base is not a base known to be absent. Eligible literals with a
usable base call existing `resolve`. Otherwise Browser calls
`BrowserUrl::parse_if_base_independent(&str) -> Result<Option<UrlParseOutcome>, UrlFailure>`:

- Some: parser-established independent result, including diagnostics;
- None: independence unproven; UnresolvedBase without a guessed interpretation;
- Err: independently established typed failure and ordered diagnostics.

The maintained parser hook shares actual preprocessing, scheme parsing and
dispatch. It certifies non-special branches and explicit forward-solidus authority
branches for special/file schemes. Other spellings conservatively return None,
even if an omitted-base parse could succeed. No callbacks escape an unproven
operation. Existing ordinary parse/resolve behavior is unchanged.

`https:icons/logo.svg` and `https:` require the unavailable context; they do not
publish standalone guesses/failures. Ordinary fully qualified HTTPS and explicit
file-host forms can be certified. There are no scheme-prefix checks in consumers,
dummy bases, parallel classifiers, origins, permissions or loader machinery.

Source base-construction diagnostics/failure appear once at the source. Resolved
occurrences retain their own ordered diagnostics; independent failures retain
UrlFailure. None emits no hypothetical null-base diagnostics. Unsupported schemes
remain resolved addresses with separate operation capability, never fake syntax
failures. Backend/capacity failures remain implementation failures.

## Debug privacy and qualification

The existing Tab surface reports snapshot-local owner/sheet labels, finite roles,
shape/context/acceptance, provenance, completeness, diagnostics and capabilities.
It omits authored text, arbitrary names, URL contents, private components, raw
keys, allocation IDs, offsets/lengths, hashes and timestamps. Nested address/base
Debug uses AH3/AH6 redaction. Exact addresses belong to controlled semantic tests.
Snapshots remain internal regression contracts.

The maintained parser is published at the immutable revision recorded below.
Both production Cargo roots, lockfiles and independently frozen approval now
select that source. Temporary path overrides were confined to the earlier local
qualification copy; they are not used for immutable-source verification. Final
Borrowser validation remains a separate requirement.

Detailed executed validation and the remaining integration checkpoint are
reported with the candidate review packet. No fetching, redirect handling,
image/font loading, new computed/rendering support, binding, CSSOM mutation,
scheduler, cache/security policy or AG schema extension is introduced.

## Executed candidate evidence and immutable checkpoint

Local candidate qualification on 2026-11-03:

| Evidence | Result |
| --- | --- |
| Maintained parser unit/WPT | 84 unit tests and the existing WPT runner pass; existing expected failures unchanged. Focused independence callback/base witnesses and no-default-features build pass. |
| Borrowser addressing | Full suite passes in both workspace lanes up to the unrelated conformance failure, including pinned 560 absolute / 336 explicit-base WPT qualification and AH8 regressions. |
| CSS / Browser | Before the notice-budget correction, CSS 478 unit tests plus integration/goldens and Browser 360 unit tests plus five integrations passed. The correction is separately covered by eight CSS resource tests and eleven Browser resource tests, including genuine source limits, notice/diagnostic caps, exact addresses and observational debug. |
| Static/build gates | Default, HTML5 and hardening workspace lint; formatting; debug/release default/HTML5 builds; both bench compilations; entity checks pass for the local candidate. |
| Conformance/parser gates | AG CI suite and independent parser fixture/fatal/golden/real-page/script/WPT lanes pass. |
| Fuzz | CSS tokenizer/parser corpus replays, promoted regression summaries and both 128-run smokes pass. Existing HTML fuzz gates pass after offline local-lock preparation. Existing sanitizer/coverage-instrumentation warnings remain; these are bounded deterministic smokes, not exhaustive fuzz qualification. |
| Full workspace tests | Both lanes fail in nine existing native macOS Chromium process-ownership tests (`task_name_for_pid: Mach 5`, cleanup timeouts). No conformance code or test policy was changed. CSS/Browser were run directly because workspace execution stopped before reaching them. |
| Dependency policy | Unmodified original production-pin checks, including 32 negative tests, pass. Both local graphs retain the approved 35-package coordinates and registry checksums, with four explicitly local candidate paths. This does not verify a new immutable candidate identity. |

The EOF diagnostic intentionally changes only the digest in the existing
malformed-function CSS fuzz summary; its 35 properties, 105 value cases and
specified/computed outcome counts are unchanged. Initial local fuzz compilation
attempts encountered unavailable registry DNS before the temporary fuzz lock was
prepared; offline lock preparation and reruns passed. These results do not hide
the original unsuccessful executions.

Historical integration checkpoint recorded before the separately authorized
2026-11-04 integration (current results follow below):

1. Commit the reviewed maintained-parser candidate and make its real immutable
   revision available through the intended existing fork source. Neither action
   was performed for this candidate.
2. Update `Cargo.toml`'s `url` and `percent-encoding` patches and
   `fuzz/Cargo.toml`'s `url` patch to that same verified revision; replace no entry
   with a permanent local path.
3. Regenerate `Cargo.lock` and `fuzz/Cargo.lock` against that source, preserving
   the qualified dependency closure or explicitly requalifying an actual change.
4. After immutable-source qualification, update `APPROVED_REVISION` and associated
   reviewed provenance in `tools/ci/check_url_dependencies.py` and the exact
   revision assertion in `tools/ci/tests/test_url_dependencies.py`. Preserve all
   negative source/closure/checksum guards.
5. Run immutable-source dependency checks and final required validation, including
   full CI on a host capable of the native process-ownership tests. Record that
   evidence before declaring AH8 closeable or merge-ready.

For the 2026-11-03 local candidate, no source revision was invented, committed,
pushed or published, and production pins, locks and policy were unchanged.

## Immutable dependency integration (2026-11-04)

The reviewed three-file parser change was committed and published as
`baf19df8ec249522e4a22e79249ac96824a91052`. The existing full native qualification
then found a test-only no-default-features build failure: the new regression
imported `std::cell::RefCell` in a no_std test crate. After separate approval,
follow-up `92f7fb395f9ffb2f706e7c664283781fd3b294a2` removed that redundant import,
reusing the existing core import. Parser implementation and Borrowser runtime
behavior are unchanged by the follow-up; history was not rewritten.

Both commits are on `feat/AH8-base-independent-parsing` in
`https://github.com/joris97jansen/rust-url.git`. The published parent, Git tree and
all changed file contents were checked against the local commits. Production
pins select **`92f7fb395f9ffb2f706e7c664283781fd3b294a2`**, not the initial revision
or the CI-control commit.

The separately approved CI-control update is
`823ba428f3a9883e8873e91d6cf46b7ac822c0a7` on `qualification/borrowser-ci`.
It changes five literal source pins only. The established qualification workflow,
fixtures, frozen graphs, checksums and rejection guards are preserved.
[Hosted run 41777938203](https://github.com/joris97jansen/rust-url/actions/runs/41777938203)
passes all eight required adoption jobs: Linux/Windows/macOS native feature lanes,
Linux/macOS AddressSanitizer, consumer-graph fuzz replay, lint and consumer audit.
The completed workflow has 27 successful jobs and one failed finding lane, not
an overall green result.
Native artifacts verify all 896 URL outcomes and retain the same nine setter
findings; matched Unicode 16 passes 12,804 cases on each adoption platform.
Every inspected artifact records the actual parser and CI-control revisions.
The legacy modern-graph IDNA baseline still fails on its exact ten known cases;
that finding is not a pass or a newly accepted conformance result.

Locked Cargo metadata for the main and fuzz roots selects the published Git
source for url, idna, form_urlencoded and percent-encoding, without local path
overrides. Both URL closures retain the same 35 package coordinates and registry
checksums. Each lockfile differs only in those four Git source identities;
unrelated packages are unchanged. No patch entries were added: the main root
keeps url and percent-encoding patches; fuzz keeps only its url patch.

The old on-disk approval rejected the nominated revision as expected. Both graphs
and all required hosted adoption artifacts were qualified before advancing
`APPROVED_REVISION`. Existing verifier functions, repository guards, frozen
versions and checksum tables are unchanged. Negative tests retain the historical
CI-control rejection and additionally reject the current control revision.
The retained audit records source/graph qualification at 05:19:07 UTC and policy
advancement at 05:19:40 UTC on 2026-11-04.

Fresh immutable-source local evidence also includes the full parser native
feature lane (including no-default and all-features), the addressing suite with
560/560 null-base and 336/336 explicit-base outcomes, CSS tokenizer/parser corpus
replays, promoted regression summaries and both 128-run CSS smokes. Historical
local-path qualification is not used as proof of this immutable identity.

### Final pinned Borrowser checks

`CARGO_NET_OFFLINE=true make ci` was run once against the real configuration.
The dependency-policy gate passes both roots and all 32 negative tests; the
frozen verifier functions and graph/checksum constants remain unchanged.
AG CI, formatting and default/HTML5/hardening lint pass. The aggregate then fails
in the native macOS conformance suite: 177 passed, nine failed, 13 ignored, with
Mach process-inspection failures and cleanup timeouts. The separately executed
unreached `test-html5-runtime` gate reproduces the same nine failures. A focused
native ownership diagnostic outside the sandbox also fails with a live-process
executable/ownership-inspection error. No conformance code or policy was changed.

Only remaining unexecuted recipe targets were then run independently. Parser
conformance, external fixtures, fatal failures, DOM/patch goldens, real-page and
rawtext/script regressions, all six HTML fuzz-smoke gates, pipeline regressions,
WPT tree building, default/HTML5 debug and release builds, both bench compilations
and entity checks pass. This continuation does not turn the aggregate green.

Because the workspace stopped before the changed consumers, their tests were
run directly: Browser 361 unit tests plus five integrations and CSS 480 unit
tests plus integration/goldens pass. The initial combined invocation reached
net's sandbox-denied loopback bindings; a permitted rerun passes all 14 net tests.
An initial HTML5 consumer command omitted the owning html package and therefore
ran no tests; the corrected command explicitly selects html and passes the
Browser/CSS/HTML/net suites. The unsuccessful attempts remain in the raw record.

Reproduction commands for the supplemental consumer and CSS checks:

```sh
cargo test -p css -p browser -p net --features browser/conformance-observation --locked --offline
cargo test -p net --locked --offline
cargo test -p css -p browser -p net -p html --features browser/conformance-observation,html/html5 --locked --offline
CARGO_NET_OFFLINE=true make test-css-tokenizer-fuzz-corpus test-css-parser-fuzz-corpus test-css-fuzz-regressions test-css-tokenizer-fuzz-smoke test-css-parser-fuzz-smoke
```

Networking tests need permitted local loopback binding. CSS and HTML local fuzz
smokes retain sanitizer/coverage warnings and the missing outer timeout-wrapper
limitation; they do not replace the hosted parser sanitizer/fuzz qualification.
No historical local-path result is presented as a final immutable-source run.

The remaining requirement is successful required Borrowser validation on a
capable supported runner, including the native Linux Chromium/ownership job
`ag2_linux_chromium` in `.github/workflows/ci.yml` and clean required CI results.
That job checks published source identity. At the 2026-11-04 qualification
checkpoint, obtaining hosted evidence still required separate Borrowser
commit/publication authorization.
The recorded host failures do not waive a gate. AH8 and Milestone AH remain open;
Borrowser was uncommitted and unpublished at that checkpoint. Required CI must
pass before merge readiness is established.

## #1457 source provenance update (2026-11-09)

The same approved `92f7fb395f9ffb2f706e7c664283781fd3b294a2` implementation is
now selected through the [local source-governance record](../../vendor/rust-url/BORROWSER-PATCH.md).
The original source qualification above remains historical evidence. This
migration preserves parser-owned independence, CSS discovery, Browser response
provenance, fixtures and expectations. See the [current AH2 amendment](ah2-typed-url-parsing-and-diagnostics.md#1457-source-governance-migration-2026-11-09)
for exact source transformations, graph equivalence and qualification limits.
