# AG7 focused external tests and direct references

AG7 remains one issue within the nine-issue AG milestone. It adds a curated,
embedded WPT subset and one local WPT-style mismatch case to the AG6 catalogue.
It reuses AG5 production observations and exact comparisons. No parser, CSS,
layout, paint or browser behavior is changed to make these cases pass.

## Corpus and provenance

All upstream files are unchanged bytes from
[`web-platform-tests/wpt` at `2c705104a295c48053eeddf7fe0170d790a4e853`](https://github.com/web-platform-tests/wpt/tree/2c705104a295c48053eeddf7fe0170d790a4e853).
Paths below are relative to that repository. Local copies mirror those paths
beneath `crates/conformance/fixtures/external/raw/`. Strict sidecars beside `raw/`
keep classification, observations, independent expectations and reference
selection separate. Every upstream input, reference and licence has a SHA-256
in its sidecar. Build validation verifies those hashes, repository, full revision,
paths and attribution. The source tree binds the sidecars as well as raw bytes.

| Case | Upstream input → reference | Execution and scoped assertion | Original assertion |
| --- | --- | --- | --- |
| `wpt/rgb-comments-p6` | `css/css-color/background-color-rgb-001.html` → `background-color-rgb-001-ref.html` | Static; match the present `#p6` computed background-color. Comments inside the RGB function must preserve the opaque `(10,150,50)` color represented by the reference's RGBA form. | Full visual equality unsupported |
| `wpt/rgb-alias-p6` | `css/css-color/background-color-rgb-003.html` → `background-color-rgb-003-ref.html` | Static; match the present `#p6` computed background-color. Three-component `rgba()` aliases `rgb()`, yielding opaque `(10,150,50)`. | Full visual equality unsupported |
| `wpt/unsupported/first-child-js` | `css/selectors/first-child.html` | Excluded before either engine: testharness.js, JS assertions and DOM selector APIs. No observations requested. | Unsupported testharness assertion |
| `wpt/unsupported/testdriver` | `infrastructure/reftest/testdriver.html` → `green.html` | Excluded before either engine: page scripts, testdriver/platform automation and dynamic readiness. Reference provenance is retained without executing it. | Unsupported visual reftest |
| `wpt-style/background-color-mismatch` | Locally authored `wpt-style/background-color-mismatch.html` → `red.html` | Mismatch the present `#probe` computed background-color: green `(0,128,0)` versus red `(255,0,0)`. No independent expectation. | No upstream claim; this is not WPT coverage |

The imported CSS color files retain Jerry Shih/Mozilla author links and upstream
help/match metadata. `raw/LICENSE.md` is the exact pinned WPT licence, SHA-256
`5fac07febb0e2a97fb0d7b0def149ec08b642e1ba4b9c345283ab1cbd2af6570`.
These authored tests use its three-clause BSD terms; preserve that notice,
disclaimer and attribution when redistributing sources or embedded binaries.
There are no local adaptations. A changed upstream file must never retain a
claim of unchanged provenance. AG7 has no adaptation schema or generic importer;
an adaptation would need explicit recorded provenance and review first.

Only the selected computed property is asserted. The other paragraphs, text,
geometry and painting in each upstream document still execute unchanged but are
not asserted. This is useful external static coverage, not full WPT reftest
success or WPT compliance.

## Model and semantics

`DocumentInput` holds exact embedded HTML, its byte digest and provenance/link
facts. `IndependentExpectation` is separate from a reference. Runnable checks
are either independent, or one direct reference with an optional independent
expectation on the test document. References have no invented expected result,
case classification, independent execution result or separate test PASS count.
`RelationshipKind` is closed to `Match` and `Mismatch`.

The single request is used identically for both documents: exactly one compared
`background-color` property selected by a unique HTML ID. DOM, layout and canvas
are unrequested. Both requested records must be complete and compatible under
AG5 validation. Relationship comparison extracts the present typed RGBA8 values
within AG5's opaque-sRGB or transparent-black domain. Validation rejects other
`StyleValue` variants, partial alpha and nonblack transparent RGBA payloads on
either side, including equal invalid values. The same validation applies to
serialized independent expectations before KnownFailure classification; malformed
expected colors are configuration/required-data errors, never XFAIL.
Relationship comparison uses AG5's exact semantic comparison helper. Source paths, hashes, document
roles and unrelated metadata cannot establish a difference. No DOM facts are
removed from existing projections.

| Valid present values | Match | Mismatch |
| --- | --- | --- |
| Equal | satisfied | violated |
| Different | violated | satisfied |
| Complete lookup establishes either target missing | presence_failed | presence_failed |
| Missing/malformed capture, unsupported observation or execution error | error; no verdict | error; no verdict |

Two absent targets therefore cannot satisfy either relationship. Intentional
absence assertions are not part of this relationship form. AG5's distinction
between missing nodes, present elements without boxes, zero-sized boxes and
malformed records remains unchanged in all other projections. There are no
numeric tolerances, geometry adjustments, screenshot equality or fuzzy matching.

## Relationship authority and metadata links

At build time the synchronous production HTML parser inspects imported inputs
and local relationship inputs. It walks active ordinary children, excluding
comments, script text and inert template content. This is metadata validation,
not test execution, and starts no runtime/parser worker. The bounded walk allows
64 KiB input, 128 nodes, depth 32 and 32 attributes per element. Runnable sources
reject active scripts/base, readiness/print flags and fuzzy/flags metadata.

For declared relationships the active parsed source must contain exactly one
match/mismatch link. Its kind and parsed raw href must agree with the sidecar.
The reference must contain no further relationship. An imported reference must
share the test's repository/revision and its upstream sibling path. A correct
hash with contradictory relationship metadata is a configuration error.

The supported href grammar is `[a-z0-9][a-z0-9_-]*.html`, a filename in the test's
own directory. No query, fragment, percent decoding, slash, parent component,
absolute URL or general resolver is supported. Sidecar `relationship.input` is
relative to the sidecar directory; it must identify exactly that sibling file.
Existing containment, regular-file and no-symlink checks apply to every path
component. This also applies to the local mismatch case.

Valid upstream forms outside this boundary must be declared `unsupported` or
`skipped` with a reason. Excluded forms can omit unsupported reference selection;
they do not acquire a partly emulated assertion. Invalid hashes, missing files,
contradictory declarations and malformed local metadata remain errors.

Build-time and Chromium inspection share `LinkFact` / `AttributeFact`: element
namespace URI/local name and sorted attributes with namespace URI, prefix,
local name and parsed value. Links retain document order; attributes sort by
namespace/prefix/name/value. Only HTML links and unqualified `rel`, `href`, and
optional `title` are accepted. Missing and empty attributes remain distinct.
There must be exactly one ASCII-whitespace-separated relation token, normalized
with ASCII lowercase for eligibility; duplicate or mixed tokens are rejected.
Reviewed inert tokens are `author`, `help`, `match`, `mismatch`. Author is needed
by the pinned corpus. Unknown/resource-bearing forms are rejected. At most 16
links and 8 KiB of fact strings are accepted.

Chromium's fixed, bounded, read-only isolated-world collector reads attributes,
never resolved `element.href` properties, and must produce exactly the embedded
parsed facts. Raw parsed values, including relation spelling, are preserved for
correspondence. Page scripts remain disabled. HTML is never stripped or rewritten.
The existing default-deny request policy remains: even an aborted authored
resource request fails capture. The existing tightly verified browser-default
favicon exception remains distinct from authored links. Reference execution is
controlled by the harness, not browser navigation through the relationship href.

## Execution, correspondence and bounds

Selection is deterministic lexicographic case order, then test before reference.
Validate the complete embedded catalogue and selected requests first. Capture
and verify cleanup for **every** selected Chromium test/reference document before
starting any Borrowser runtime/parser worker. Validate exact capture correspondence
and native projections. Then execute each selected Borrowser document in a fresh
Tab through production parsing, style, layout and paint. Excluded cases execute
neither engine. Every document uses the unchanged AG environment.

```rust
struct CaptureDescriptor {
    case_id: String,
    role: DocumentRole, // Test | Reference
    source_sha256: String,
    request: RequestDescriptor, // exact owned AG5 selections, targets and order
}
```

The local comparison boundary and AG4 ingestion compare the entire ordered typed
descriptor vector, including count and uniqueness. Case/role rejects substituted
or duplicate dependencies; the byte digest rejects stale HTML; the exact request
rejects changed targets/properties/selections. Order rejects reordered captures.
No request, case or selection digests are needed. Filenames are diagnostic only.
The AG4 artifact identity uses the same expanded vector; reference-byte changes
therefore change artifact identity. Existing source-tree identity binds all source
and metadata, executable SHA-256 binds the built binary, and pin/profile/platform
checks remain. Nothing here is a signing or replay-protection protocol.

Capture and comparison schemas are `borrowser.chromium-phases.v2` and
`borrowser.cross-engine-phases.v2`, capture profile `ag7-static-references-v1`,
artifact manifest schema 2. Producers and consumers must be rebuilt together.
The existing 64-KiB capture and other size/time/resource limits remain. Historical
cleanup-only does not validate today's catalogue. No infrastructure, credentials,
transport or cleanup mechanism changes; no live AWS work is implied.

## Reporting, aggregation and exits

Each case reports independently authored expectation outcome (when present),
Borrowser relationship, Chromium relationship in comparison mode, same-document
cross-engine fields for each role, and original upstream assertion coverage.
Relationships say `satisfied`/`violated` for the named scope, never full visual PASS.
A satisfied mismatch is not a cross-engine difference. Two satisfied relationships
do not suppress a difference between engines.

| Condition | Default mode | Comparison mode |
| --- | --- | --- |
| Configuration/execution/capture/required-data/cleanup error | ERROR, exit 2 | error, exit 2 |
| Independent FAIL/XPASS or relationship violation/presence failure | FAIL/XPASS, exit 1 | failure, exit 1 |
| Same-document cross-engine difference | Not applicable | different, exit 1 |
| Existing incomplete requested observations | INCOMPLETE, exit 3 | incomplete, exit 3 |
| Satisfied upstream scoped relationship; unsupported full visual assertion | INCOMPLETE; independent outcome retained, exit 3 | upstream incomplete, exit 3 if no higher-precedence result |
| Known independent mismatch without other failure/incompleteness | XFAIL, exit 0 | independent XFAIL, exit 0 if complete/equal |
| Satisfied local relationship, complete observations | PASS, exit 0 | equal_for_requested_scope, exit 0 |
| Excluded-only selection | UNSUPPORTED/SKIP, exit 0 | incomplete, exit 3 |

This table describes harness invocations. Build-time declaration/provenance errors
stop Cargo before execution; a failed Cargo command has its own nonzero exit.

Precedence is 2, then 1, then 3, then 0. KnownFailure applies only to the
independent expectation. It cannot hide relationship failures, errors or
cross-engine differences. Upstream unsupported coverage is separate from
unsupported requested observations and excluded execution classification.
Runnable upstream unsupported coverage makes the aggregate incomplete;
excluded upstream coverage adds no default-mode exit requirement.

The default report counts cases once. A runnable upstream case may therefore
have aggregate INCOMPLETE and independent XFAIL. Comparison summaries separately
count executed documents, same-document fields, independent expected failures,
relationship outcomes, all upstream unsupported cases and runnable upstream
incomplete cases. A reference never contributes another passed test.

Both document roles retain `captures` separately from common `projections`.
The shared capture formatter includes Borrowser's accepted-frame provenance,
canvas observation when requested, Chromium's native inspection, pinned browser
identity and verified parsed metadata-link facts. A reference gains evidence,
not an independent expectation or a separate case verdict.

Native macOS arm64 execution against pinned Chromium 155.0.8059.39 confirmed:

| Case | Independent B expectation | B test → reference | C test → reference | Same-document B ↔ C | Exit default / compare |
| --- | --- | --- | --- | --- | --- |
| Each RGB case | XFAIL; correct expected `[10,150,50,255]` | transparent `[0,0,0,0]` = transparent, match satisfied | `[10,150,50,255]` = same, match satisfied | Different for test and reference | 3 / 1 |
| Local mismatch | None | green ≠ red, satisfied | green ≠ red, satisfied | Equal for both roles | 0 / 0 |
| Each excluded case | None | Not executed | Not executed | No fields | 0 / 3 |

The original six cases remain PASS with 34 equal fields and their meaningful
value assertions. The initial whole catalogue has 11 cases, 12 executed documents,
36 equal and four different cross-engine fields, two independent XFAILs, three
satisfied relationships per engine, two excluded cases, and four unsupported
upstream assertions (two runnable). Default counts are 7 PASS, 2 INCOMPLETE,
2 UNSUPPORTED, exit 3. Comparison exits 1. This is macOS evidence, not native Linux
or live AWS qualification.

## Qualification record: 2026-10-21

Required native Linux x86-64 qualification and applicable CI passed for the
reviewed implementation in [PR #1436](https://github.com/joris97jansen/borrowser/pull/1436):

- Base (`master`): `23a2ddf8af24cb06d74f781cea44d8616316fb6e`.
- Implementation commit: `8daf925fdc2c74802426103856dc9894a27f880b`.
- Implementation tree: `1cb3f2695155746af352141a63e6fcdba277bb8b`.
- Actual CI test-merge checkout: `c592401b4d3cf04bf1d6a588158a811d497900be`.
- Actual checkout tree: `1cb3f2695155746af352141a63e6fcdba277bb8b`, identical to
  the implementation tree, including file modes.

[CI run 39601789076, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39601789076)
completed successfully: twelve jobs passed; the separate AG4 native proxy cleanup
timing job was skipped by its existing branch condition. These results qualify
the implementation commit/tree above. Later documentation-only trees were not
tested by this run.

The [native Chromium job](https://github.com/joris97jansen/borrowser/actions/runs/39601789076/job/119227835636)
ran on Ubuntu 24.04.5 x86-64, image `ubuntu24 20261012.334.1`, in a native Microsoft
VM with the container probe returning `none`. It used Chrome for Testing
`155.0.8059.39`, revision `@3ff7ac5a9224be9156d7f8703a06e22890aafd34`, CDP `1.3`,
profile `ag7-static-references-v1`. The Linux archive SHA-256 was verified as
`55672d1f392fd3e7b7a08621b6e804e6bcb39d40cf155504abb74b3a021ea8ea`.
Each of the nine explicitly invoked pinned-Chromium qualifications executed one
test and passed with exit 0. Native capture and profile-cleanup checks passed.

Linux qualification retained the original six fixtures' 34 equal-field assertions
and exactly four RGB differences: `wpt/rgb-comments-p6` and `wpt/rgb-alias-p6`,
each in test and reference roles, at `style:#p6/background-color`. Borrowser
observed `[0,0,0,0]`; Chromium and the independent semantic expectations remained
`[10,150,50,255]`. Both engines satisfied their scoped matches; independent
expectations remained XFAIL and full upstream visual coverage remained unsupported.
The qualification tests passed by verifying those exact results; the actual
whole-catalogue comparison still exited 1. An additional unrelated canvas/cascade
difference was rejected by the acceptance checker.

The Linux lane did not execute the isolated incremental/source-free rebuild or
native EICE/OpenSSH qualification. Separate final macOS 27.0.1 ARM64 validation,
including the expanded reference/numeric corpus and incremental/source-free test,
passed against reviewed working-source manifest
`757b22749b5d51b799ac0f3b2dad5b5a4454d41899df0127f32d774258e75d1e`.
Its source contents and modes were verified against the implementation tree.
The earlier macOS checkpoint above retains its original platform scope. Native
Linux guest-wrapper checks do not add live AWS qualification; none is claimed.

Historical macOS incidents remain separate from completed qualification:

- **Combined-CLI shutdown incident: unresolved; owner disposition pending.**
  `real_chromium_combined_cli_is_stable_and_selects_exactly` exited 101; the
  harness exited 2 instead of 1 with `Shutdown deadline expired`. The retained
  record has no cleanup diagnostic line. The binding deadline, cause and verified
  historical cleanup outcome remain unknown.
- **Composition incident: unresolved; owner disposition pending.**
  `comparison::tests::real_chromium_comparison_composition` and its isolated child
  exited 101 in `borrowser-failure`, after the second capture started and before
  expected cleanup/worker events. Historical harness exit and primary/cleanup
  details were omitted; the operation label could be stale. The cause and final
  second-capture cleanup outcome remain unknown.

Missing historical evidence establishes neither successful cleanup nor a proven
leak. Later macOS/Linux passes and completed CI do not resolve either incident.
Neither owner residual-risk recommendation has been accepted. Recording completed
qualification does not authorize readiness, merge or issue/milestone closure.

## Adding another supported case

1. Review the exact upstream revision, HTML and reference together. Check static
   eligibility, resource use and the assertion. Exclude stronger/dynamic forms
   rather than weakening their meaning or altering their source.
2. Copy exact bytes under `external/raw/<upstream-path>` and retain the pinned
   licence/attribution. Verify each SHA-256 against a separate fetch from the
   pinned repository revision. Record repository/revision/path/hash/licence for
   test and reference in the sidecar. No build-time network fetch is performed.
3. Declare an ID and execution classification. For a supported direct relationship,
   declare the matching kind and sibling file already expressed by the source.
   Choose one meaningful, present ID/background-color on both sides; no inserted
   IDs or rewritten source. If unavailable, classify the form unsupported.
4. Record unsupported original visual assertion coverage explicitly. Add a correct
   independent expectation only when independently justified, with KnownFailure
   if a known engine deficiency warrants it. Never invent a reference expectation.
5. Rebuild; run build validation, exact-ID default execution and pinned Chromium
   comparison. Inspect actual fields, relationships, independent results and
   coverage separately. The corpus expands through data, without Rust registration.
6. Add focused semantic assertions for the new test contribution. Fixed initial
   counts apply only to the named initial subset. Whole-catalogue qualification
   discovers declarations, checks authored expectations and direct relationships,
   verifies exact ordered descriptors, coverage and summary/exit consistency, and
   accepts only the four confirmed RGB differences. Additional passing cases need
   no Rust registration or global count change. The test-owned acceptance helper
   is not a generic expected-cross-engine-failure mechanism.
   CSS-pixel and layout-coordinate expectations use AG5's exact typed numeric
   equality: JSON `20` and `20.0` are both valid spellings of the same `f64`
   value. Qualification decodes only those schema fields; RGBA8 bytes, value
   kinds, record identities/order and observation states remain strict. It adds
   no rounding or tolerance. The temporary-corpus qualification retains an
   integer-authored font-size case through catalogue, CLI and source-free runs.
   Never derive correct expectations from captured output or automatically bless it.

Use temporary copies for deliberate mutation tests. Validation includes provenance,
contradictory links, relation truth/presence/error cases, strict capture identity,
exclusions, native resource denial, incremental discovery and source-free execution.
Run the commands in the conformance README. Ignored qualifications must be invoked
explicitly and must execute their named tests. Local guest-wrapper execution does
not establish native Linux or live AWS qualification.
See the [qualification record](#qualification-record-2026-10-21) for completed
native Linux qualification and the separate pending owner incident dispositions.

Multiple references, chains/cycles, fuzzy/print/readiness support, external
resources/fonts, page JS, testharness.js execution, WebDriver, screenshots and
broader selectors remain excluded. There is no generic importer, document graph,
provider interface, cache or metadata-version framework.
