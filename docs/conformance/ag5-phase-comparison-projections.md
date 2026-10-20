# AG5 static phase comparison contract

AG5 extends the AG3 vertical slice within the existing nine-issue milestone.
It adds observations and comparisons, not rendering features. The registry has
six fixtures. The Chromium pin remains Chrome for Testing 155.0.8059.39, revision
`@3ff7ac5a9224be9156d7f8703a06e22890aafd34`, protocol 1.3, with unchanged archive
checksums. The capture profile is `ag5-static-phases-v1`.

## Authority and ownership

The DOM authority is the successfully committed Browser document:

```text
production parser → CoreEvent → Tab::on_core_event
                  → Browser publication/commit → completion validation
                  → accepted production frame → read-only observation
```

`html::conformance` observations are not an input. A correct parser result cannot
hide a publication or completion failure. CSS owns cascade and computation;
Layout consumes computed styles; GFX consumes layout; Browser owns runtime
invalidation, retained artifacts, and observation validity. Tooling owns field
selection, common projections, comparison and reporting.

The `browser/conformance-observation` feature exposes:

```rust
Tab::render_observed_frame<'tab>(
    &'tab mut self, ctx: &egui::Context, input: egui::RawInput,
) -> Result<CompletedPageFrame<'tab>, FrameObservationError>
```

It owns one begin/end pass and calls the same internal body as `ui_content`.
There is no alternate renderer or observation cache. The ordinary UI passes no
observation sink and performs no extra observation copies. A call-local draft
copies the `ComputedStyle` values of elements in the actual `StylePhaseOutput`
and clones the retained layout result supplying that frame, before the result
is recorded by PageState. The read-only `FrameElementStyle` carries an internal
DOM ID only for matching within this borrowed document. It is never a
cross-engine identity.

The boundary requires successful parser completion, a committed handle/version
and mode, no outstanding loading work, unchanged document/navigation identity,
`Rendered` status, no follow-up/action or pending render work, and no egui
discard. After recording results, Browser checks clean style/layout/paint state,
current style key/cache, layout key/generations/domain, and the paint key tied
to that layout. Cached presence alone establishes nothing.

`CompletedPageFrame` borrows the committed DOM and owns the frame's copied style
facts, layout artifact and `FullOutput`. Its immutable accessors cannot mutate
Tab or PageState. The borrow prevents navigation/publication during observation.
Tooling constructs its owned selected values before the local Tab is dropped.
There are no leaked objects, unsafe lifetime extensions or self-references.

`RetainedBoxView` exposes immutable rectangle/children/direct-DOM identity. It
never maps anonymous or marker anchors to DOM-backed box identity. Inspection
does not materialize another layout tree. The retained artifact records the
original unquantized production width as provenance. The current frame's actual
available width is also recorded; neither is inferred from the rounded cache
key. This adds metadata without changing layout computation or reuse policy.

## Selected common surface

| Family | Facts and Borrowser authority | Chromium observation | Equality and limits | Fixtures |
| --- | --- | --- | --- | --- |
| DOM | Committed document, doctype name/public/system IDs, elements, namespace URI/local name, namespaced/prefixed attribute names and values, exact text/comments, ordered children, distinct template fragment edge | Native document/childNodes/attributes/namespaceURI/localName and HTMLTemplateElement.content in isolated world | Exact tree and strings; attributes sorted only in comparison by namespace/name/prefix/value; absent doctype IDs become DOM empty strings | `dom/structure`, `dom/template-namespaces` |
| Computed style | Actual accepted-frame `ComputedStyle`: color, background-color, font-size, display, selected by unique HTML id | Native `getComputedStyle` color/backgroundColor/fontSize/display | Four property-specific domains below; no authored-declaration comparison | `style/cascade-inheritance` |
| Layout | Actual retained geometry supplying accepted frame; only direct DOM-node boxes | Native `getClientRects`, with zero scroll and scale 1 | Ordered border-box rectangles; exact left/top/right/bottom in document CSS px; no internal box-tree equivalence | `layout/block-boxes` |
| Paint/reftest | Existing guarded opaque Browser-presented canvas sample from accepted egui output | Existing sRGB PNG RGB8 pixel (32,32) | Exact RGB8 under unchanged AG3 guard/environment/resource restrictions | `canvas/root`, `canvas/cascade` |

DOM child ordering remains meaningful and exact. Text whitespace, case-sensitive
foreign local names, namespace distinctions, comments and inert template contents
are retained. Production attribute ordering and existing parser snapshot
contracts do not change. XML, shadow trees, frames, processing instructions,
scripting-dependent documents and additional node kinds are outside this profile.
An unexpected unsupported native node causes a capture error, never omission.

Style domains and semantic stages:

- `color`: Borrowser computed RGBA8; Chromium CSSOM resolved/used color equals
  that value for the selected absolute opaque sRGB, unvisited, non-forced domain.
- `background-color`: the same absolute sRGB domain plus transparent black
  `[0,0,0,0]`. This is element computed color, not canvas propagation.
- `font-size`: computed absolute CSS px and inheritance of absolute values;
  Chromium CSSOM reports the same stage as an absolute px serialization.
- `display`: computed `block`, `inline`, or `none` for static ordinary elements,
  without flex/positioning blockification contexts.

Native CSSOM serialization is decoded into these types, not computed again.
Percentages are not resolved by tooling. Advanced color, partial alpha,
viewport-dependent font sizing, additional display values and unselected
properties require explicit unsupported selection. An invalid serialization or
unexpected value outside the declared domain is a capture error, not an equality
or a retroactive unsupported classification. Supported-value disagreement remains
a difference. The regression suite includes an explicit unsupported `line-height`
request alongside a supported color request.

Layout targets are unique explicit HTML ids in the active document. Duplicate ids
are a correspondence error. The fixture uses supported physical longhands, fixed
integer widths/heights, explicit zero margins, padding and borders, and no text,
wrapping or viewport-dependent sizes. The container's authored content size
160×64, padding 4 and border 1 yield a 170×74 border box; its 32×16 child starts at
(5,5). An 80×12 sibling starts at y=74. Hidden, zero-sized and missing targets are
separate cases. These constraints fit both engines' recorded widths.

Layout coordinates start at document (0,0), in CSS px. GFX's presentation origin
is not added to layout values. Native client rects coincide with document coords
under the checked zero-scroll environment. There are no guessed offsets. Width
provenance is reported outside common equality; matching selected fixed geometry
does not assert equal general layout constraints. A rectangle's edges are formed
from the production x/y/width/height widened to f64. Equality is exact, except
canonical positive zero. Nonfinite numbers are errors. No Rectangle epsilon,
rounding, tolerances, identity mapping, or whole box-tree claim is introduced.
Tests retain fractional geometry and detect sub-epsilon edge changes.

Paint proves only the guarded opaque sRGB canvas-background sample at CSS pixel
center (32.5,32.5). It does not prove general painting, clipping, stacking, text,
paint-command equivalence, or Borrowser GPU rasterization. Internal paint
snapshots remain supplemental internal regressions. No screenshot subsystem,
general pixel diff, compositor inspection or Chromium paint extraction is added.

## Requests, results and expectations

Fixture execution and observation selection are separate. `ObservationRequest`
selects DOM, individual style fields, layout targets and canvas independently.
`Request::{NotRequested, Compare, Unsupported}` is a small closed tooling model.
An empty layout `Compare(&[])` is invalid configuration; `NONE` continues to
mean that no observation was requested. Before local expectation evaluation or
cross-engine comparison, both common projections and the independent expected
projection must match the request's exact family states, style target/property
records, layout target records, and order. Missing, extra, duplicate or
misidentified records and unexpected unsupported/unrequested states are
`observation.required_data` errors, including under KnownFailure metadata.
This checks observation records, not observed content: different DOM child counts
or rectangle counts remain meaningful differences.

A DOM/style/layout fixture never invokes the canvas collector unless it also
requests canvas. The canvas guard and original fixture bytes are unchanged.

- Not requested: explicitly absent from equality/coverage counts.
- Unsupported: explicit metadata and reason; never an equality result.
- Missing node: a complete target lookup found no element; structural data.
- Present with zero boxes: element exists but generates no box.
- Present with a zero-sized rectangle: an actual box, distinct from both above.
- Missing/malformed/incomplete/stale required capture data: error; no report.

Native Chromium facts remain in `captures`; common values are separately in
`projections`. Borrowser capture metadata records the accepted production source
and width provenance; common values are copied while the accepted frame borrow
is alive. Expected values are absent from `CaptureInput` and all collectors.
The four new expected JSON files are independently authored from the fixtures;
there is no expectation generation or blessing command.

Report order is registry order, then DOM/style/layout/canvas, request target and
property order, and left/top/right/bottom edge order. DOM differences follow
canonical tree paths; canvas channels remain red/green/blue. Structural absence
has a path and both values, distinct from a missing required record. A deliberate
geometry mismatch identifies, for example,
`layout:#container/border-box[0]/right`.

Case status is `different` when any compared field differs, otherwise
`incomplete` when requested coverage is unsupported/skipped or no comparison was
made, otherwise `equal_for_requested_scope`. Family coverage states/counts and
unsupported fields remain visible even on a differing case. Independent
Borrowser expectation status remains separate; agreement can coexist with FAIL.
XFAIL never hides a cross-engine difference. Local partial expectations use
INCOMPLETE rather than PASS. Existing canvas selection text, classification and
error precedence are preserved; the default registry now runs six fixtures.
Local status and diagnostics consume the same validated typed expectation;
JSON `20` and `20.0` both deserialize to the same CSS-pixel value. This is typed
decoding, not numeric tolerance. Text reports show unsupported field/family
identities and reasons, incomplete coverage even without differences, and
KnownFailure reasons independently of supported-field differences.

The passing baseline has exactly 34 equal comparison fields: two canvas samples,
two whole-DOM fields, twelve style properties, and eighteen layout fields
(sixteen rectangle edges plus hidden/absent target records). Real-profile and
independent CLI tests assert exact field identities, order and counts.

Comparison exits: 2 for errors, 1 for differences or independent FAIL/XPASS,
3 for otherwise incomplete coverage, 0 for complete agreement over the requested
scope. Errors suppress the entire comparison report, including earlier cases.

## Chromium and capture compatibility

The fixed checked-in `inspect.js` reads native DOM, CSSOM and client rectangles.
It runs through `Page.createIsolatedWorld` for the verified main frame and
`Runtime.evaluate` with an explicit context, return-by-value and the remaining
capture deadline. Page-script execution remains disabled before navigation and
through capture. The collector does not mutate fixture nodes, execute fixture
source, dispatch events, schedule work, fetch resources, or implement semantics.
Only serialized request data is supplied to the fixed function.

Existing document URL/mode, resource policy, viewport/scroll/scale checks,
pre/post frame/loader barriers, cancellation and cleanup guarantees remain.
All Chromium captures and verified cleanup complete before any Borrowser parser
worker starts. Run real-browser qualifications in separate processes; a test that
has executed Borrowser must not launch Chromium afterward.

The configured CSS viewport is 640×480. Native inspection records `inner_width`
and `inner_height` from `window.innerWidth/innerHeight`, which include classic
scrollbars, and requires those exact configured dimensions. `layout_width`
comes from the standards-mode root's `clientWidth`, excludes a classic vertical
scrollbar, and must agree exactly with `Page.getLayoutMetrics.cssLayoutViewport`
at live capture. It is positive, integral and no greater than `inner_width`.
Local and AG4 ingestion require all three fields; none has a default.
These engine-specific facts stay outside common geometry equality.

The pinned [Page protocol definitions](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/third_party/blink/public/devtools_protocol/domains/Page.pdl)
define both CSS client viewports as excluding scrollbars. For DOM/style/layout
inspection their dimensions must be positive integral values within the
configured dimensions, with layout and visual client dimensions equal. No
scrollbar width is assumed. If canvas is also requested, both client viewports
must instead retain the full 640×480 size, preserving the canvas restriction.
Every request retains zero document scroll and zero visual offsets, unit visual
scale, and unit zoom when the optional protocol zoom field is present. Missing,
malformed, nonfinite, inconsistent or out-of-range required values are errors.
Scrollable content extent is diagnostic evidence, not a viewport-size equality
requirement. This distinction adds no numeric tolerance or layout computation.

Capture errors carry bounded fixture id/repository path/family attribution.
Viewport failures retain expected dimensions, both CSS viewport objects, content
extent when available, and exact rejected fields. Missing and malformed values
are explicit; arbitrary remote strings/objects and fixture contents are not
dumped. Primary failures and cleanup errors remain separate stderr diagnostics;
no partial successful report is emitted.

Producer, local consumer and AG4 ingestion use `borrowser.chromium-phases.v1`;
comparison uses `borrowser.cross-engine-phases.v1`. Old schema/profile output is
rejected explicitly. Producer/controller/guest harness binaries must be rebuilt
from the same AG5 source contract. There is no version negotiation or migration
framework. AG4 transport, credentials, infrastructure and cleanup are unchanged.
Its guest wrapper still allows 64 KiB reports and 256 KiB diagnostics.

Limits remain 4 MiB per CDP message and 64 KiB for the whole capture report.
An inspection permits at most 128 nodes, depth 32, 32 attributes per node,
8 KiB of strings, eight unique targets and the four fixed computed properties.
Client rectangles are bounded as well. Overflow is rejected, never truncated.
Typed decoding rejects unknown/duplicate fields, malformed JSON, missing nullable
records, wrong order/identity, inconsistent missing-node facts and nonfinite data.
Exactly the selected registry cases must be present. Explicit null target data
means complete absence; a missing record/field is an error. When DOM is included,
native style/layout presence claims must agree with unique explicit HTML-id
lookup in that active tree. Template fragments are excluded from the lookup;
a duplicate active target or a contradictory presence claim is rejected at both
local and AG4 ingestion boundaries. This consistency check does not derive
computed values, box generation or geometry from DOM or CSS.

## Validation and qualification

Use the existing four-level workflow and `borrowser-test-selector`:

1. Targeted Browser observation, retained layout, projection, comparison and
   Chromium inspection/ingestion tests. They cover completion/publication failure,
   discarded/mismatched/stale artifacts, reuse without extra computation,
   canonical attributes, exact text/template distinctions, property domains,
   missing/absent/zero geometry, selection, bounds and localized differences.
2. Browser feature lane, Layout/GFX/conformance suites, guest-wrapper tests,
   formatting, lint and build. The ordinary conformance suite preserves native
   lifecycle and CLI regressions. Independent CLI processes compare complete
   stdout/stderr/exit results.
3. Pinned-browser acceptance and deterministic fixture runtime smoke. Run each
   ignored real-browser test in a separate cargo invocation, including
   `real_chromium_ag5_read_only_inspection`, `real_chromium_ag5_profile`, existing
   canvas/script/lifecycle/composition tests, both real CLI tests and the exact
   guest-wrapper qualification. The inspection test verifies page-script
   suppression and matches the exact collector error for oversized strings,
   oversized trees and duplicate targets, with clean shutdown. Inert template
   test inputs isolate the bounds checks from unrelated viewport overflow.
   An earlier viewport/navigation/capture error cannot satisfy these negatives.
4. `make ci` and the existing native Linux Chromium qualification lane. The lane
   now includes the new profile and independent-process tests. Historical AG3/AG4
   evidence does not qualify this profile. No live AWS operation is needed for
   AG5 implementation validation.

Native macOS qualification was rerun on the review-corrected code on 2026-10-19:
34 equal fields, no differences, and six independent expectation passes. The
new inspector, existing native lifecycle/script/composition tests, repeated
capture/comparison CLI runs with exact field assertions, and all three exact
guest-wrapper tests passed. Targeted authority/projection/reporting/ingestion
regressions, the conformance suite and full local `make ci` passed at that
macOS checkpoint, before the later Linux corrections described below. The
prior blank-window `make run` smoke is historical. A later ordinary-Browser
inspection on 2026-10-19 loaded the unchanged fixed-block fixture after Refresh
and observed its outlined container. The initial URL+Enter submission remained
blank; that pre-existing navigation limitation is outside AG5 and is unchanged.
The accepted smoke proves rendering after Refresh, not exact geometry or a
successful complete URL-entry workflow.

Native Linux [run 39311219271, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39311219271)
on source `bc663f83631716787d37c2cf895479da2c9a4922` failed capture before
comparison. Its separate Rust job passed. The single diagnostics-only follow-up
`e63c8aebd06efeca28181806369f5a075cd64ea4`, tested by
[run 39366788841, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39366788841),
identified `dom/template-namespaces`: both CSS client viewports were 625×480,
content extent was 625×647, all offsets were zero and scale/zoom were one.
Only the client widths differed from configured 640. The old guard incorrectly
applied the canvas full-client-size condition to this DOM-only observation.
Both failed attempts remain evidence; neither qualifies the functional viewport
and negative-test corrections. This was a capture defect, not an observed
Borrowser/Chromium semantic difference.

The diagnostic attempt also exposed the bootstrap test helper's incidental
newline at the oversized-input boundary; its producer failure did not establish
validator rejection. The later viewport/bootstrap correction was tested in
[run 39372426117, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39372426117),
which failed separately in real-Chromium cleanup (Failure A) and Linux ownership
discovery before the cleanup-reap fault was armed (Failure B). Failure A's
original scenario, cleanup vector and cause remain unknown. Failure B's
disappearance-handling source defect is corrected, but its exact historical
failing syscall remains unknown. No shared cause is established. The passing
[diagnostics-only run 39375279230, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39375279230)
qualifies only its own published tree, not the later functional correction.

The final functional candidate `821ef72424851da41e87192ca46d503fada592bd`,
tree `72704e4148368fc078fcd206aea85d38f48e8b8b`, passed native Linux x86-64
qualification in [run 39390972794, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39390972794)
on 2026-10-20. Its synthetic checkout
`a00289491806550ac277c9acaf5ff50dd600ac10`, based on
`871da2141c66b66a394dd12c8191fd7447eac468`, has the identical tree. The lane
used Ubuntu 24.04 x86-64, Rust 1.92.0 and pinned Chrome for Testing
155.0.8059.39 with `ag5-static-phases-v1`.

Executed checks passed: all eight production-path observation regressions,
both bounded fixture controls and the expected-parent spawn-error regression;
native ownership and cleanup-reap fault activation/consumption; ordinary
conformance; capture/script, lifecycle/topology and composition acceptance;
the read-only inspector's positive control and exact string/tree/duplicate-target
rejections; the full six-fixture profile; both independent-process CLI
qualifications; all three exact guest-wrapper tests without a real-browser
skip; and the final private-profile cleanup gate with empty remaining-artifact
and profile inventories. Separate Chromium capture/cleanup-before-parser-worker
invocations were preserved.

The full-profile acceptance and CLI assertions established exact fixture order
and field identities: 2 canvas + 2 whole-DOM + 12 style + 18 layout = 34 equal
fields, with six independent expectation PASS results. The persisted
`comparison.json` is canvas-only (`canvas/cascade`); it is not the full-profile
evidence. Paint remains the guarded canvas sample, with no general painting,
GPU-rasterization, whole-box-tree or broad browser/WPT claim.

The same run passed ordinary Rust default and explicit HTML5 lanes, build/lint,
parser/regression checks, credential-free bootstrap/synthesis (including both
oversized inputs and producer exit-status preservation), and downstream tooling.
Intentional opt-in skips remained explicit: disposable Noble/OpenSSH, real
Docker cancellation and the credential-free lane's real-browser wrapper case
(which executed in the native lane). The unrelated AG4 timing job was skipped
by its existing branch/event condition. All twelve applicable CI jobs passed.
These results qualify the functional tree above; later documentation-only
commits have different complete trees and require their own current PR checks.
AG5 stays one issue within the existing nine-issue milestone.
