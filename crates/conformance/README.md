# Focused static conformance harness (AG1–AG7)

AG4 adds the optional `--compare-aws` and artifact-independent `--aws-cleanup`
paths. Its two-resource Python CDK capture stack, shared eleven-resource
networking/service-role/EICE stack, ordinary SSH through the endpoint, strict capture
ingestion, operation recovery and qualification gates are documented in the
[AG4 contract](../../docs/conformance/ag4-disposable-aws-chromium-runner.md).
The run keeps its own SG and instance; shared EICE/SGs are never run cleanup
targets. Live configuration binds the exact shared StackId and canonical template
digest, derives physical IDs, and rejects obsolete `controller_cidr` fields.
Historical cleanup-only remains independent of EICE/current operator configuration.
The reviewed AMI/package snapshot are pinned. AG4 #1387 is completed and
PR #1433 is merged. Earlier direct-SSH lifecycle evidence is historical. The DNS
contract requires the authenticated `ec2-instance-connect-endpoint.eu-west-1.amazonaws.com`
hostname. Command cleanup uses absolute phases: a two-second natural-shutdown
opportunity, at most one second for TERM, the latest planned forced transition
at T0+3, and forced cleanup/finalization through the unchanged T0+5 authority. All
PGID authority ends before root reap; only kernel-owned native child obligations
remain afterward. This composition qualified on native macOS arm64 and Ubuntu
24.04 x86-64 for exact tree `ffb82ab09cfa1eb8ea7e9ec5ed4f0ba9c1bbcd23`.
Both platforms met the T0+4 qualification target with complete 32+32 timing
records; minimum cross-platform headroom was 0.997243 s to that target and
1.997243 s to the hard deadline. Linux run 39017456664/1 succeeded. See the
[retained timing qualification](../../docs/conformance/ag4-disposable-aws-chromium-runner.md#native-cleanup-timing-qualification-2026-10-17)
for measurements and durable evidence under `~/.local/share/borrowser/reviews/ag4/`.

This documentation-only update reports that exact qualified tree; its resulting
Git tree is different and is not a new qualification identity. Later executable,
test, CI-semantic or timing-source changes require review of whether fresh
qualification is necessary. Final live artifacts receive fresh source/artifact
identities.

The old Linux run 38957426608/1 failed; its durable evidence is retained. Future
Linux qualification publications/runs require separate authorization. Earlier lost
macOS logs, including two `BrowserExited` observations and their reported
pre-EICE reproduction, remain historical summaries only. The Chromium observations
are unresolved and are not established EICE regressions.

AG6 preserves the completed AG4 contracts and validates its affected fixture and
artifact integration. It does not reopen cloud qualification or perform live AWS
work. Historical qualification records below retain their original scope.

AG6 discovers and embeds fixture metadata at build time. See the
[AG6 authoring and discovery contract](../../docs/conformance/ag6-fixture-discovery-and-metadata.md)
for the schema, validation boundaries and mode behavior.

AG7 adds a small pinned external corpus and scoped direct match/mismatch
relationships. See the [AG7 contract](../../docs/conformance/ag7-focused-external-tests-and-references.md)
for provenance, raw-byte preservation, link policy, authoring and reporting.
Original upstream visual assertions remain unsupported. The whole catalogue now
returns **3 locally** (unsupported full visual coverage) and **1 in comparison**
(four confirmed RGB differences); neither result is suppressed by KnownFailure.
The original six-fixture baseline remains unchanged.

Ordinary local commands below remain independent of AWS configuration,
credentials and artifact preparation.

Run the embedded discovered fixture set:

```sh
cargo run -p borrowser-conformance --locked
make conformance
cargo run -p borrowser-conformance --locked -- canvas/cascade
```

The initial six-fixture corpus compares committed DOM structure, four selected computed
properties, fixed block border boxes, and the existing guarded opaque sRGB canvas
sample at pixel `(32,32)`. See the [AG5 observation and comparison contract](../../docs/conformance/ag5-phase-comparison-projections.md)
for authority, exact coverage, unsupported cases, wire formats and qualification.
This adds tooling coverage, not engine features or broad browser conformance.

## Production execution

Each case constructs a fresh `Tab`, calls `navigate_to_new`, and fulfils the
emitted document fetch with fixed `NetworkStart/Chunk/Done` events. Browser's
resulting parser commands run in the normal `runtime_parse` worker. Its genuine
mode-bearing publications go through `Tab::on_core_event`: Browser validates and
commits DOM, mode, and mutation facts, reconciles authored stylesheets, and queues
render work. The harness never constructs or accesses private document/style or
retained-rendering state.

After the parser terminal event and successful Browser completion validation,
`Tab::render_observed_frame` (or the canvas-only `ui_content` path) performs the normal CSS preparation, Layout, GFX Paint, and
Browser presentation path in a fresh egui pass. Publication errors, parser
failures, missing documents, style errors, follow-up work, discarded output,
changed navigation, and unexpected subresources are execution errors. All
outgoing commands are drained after publications and after rendering. There is
no network runtime or external I/O; any additional resource request is an error.
The five-second parser deadline bounds waiting, never establishes success.

Parser completion is distinct from response completion and Browser commit.
The terminal handle/version/mode must match committed Browser state. An earlier
publication failure cannot be replaced by a later success. Cancellation and
stale request filtering retain production ownership.

## Observation and environment

Inputs are fixed: a 640 × 480 viewport, device scale 1, egui zoom 1, time 0,
default pinned egui fonts/configuration, no input events or scrolling, fresh
Tab/context, URL `https://borrowser.invalid/ag1/fixture.html`, HTTP 200 metadata
with `text/html; charset=utf-8`, UTF-8 bytes in 128-byte chunks, and standards
HTML doctype. The production HTML parser selects the document mode.

The canvas background is derived from production CSS/style output and presented
by Browser-owned `egui::CentralPanel` rendering. The ordinary Layout and GFX Paint
paths execute, but this observation does **not independently prove their general
correctness**. It does not test GPU rasterization, arbitrary DOM geometry,
general element painting, text rendering, or comprehensive rendering conformance.

The canvas collector inspects only the current `egui::FullOutput`. It requires exactly one
opaque, untextured, square-cornered, unstroked, unblurred rectangle covering the
viewport, whose clip also covers the viewport. No other effective paint may
intersect the guard rectangle `(30, 30)`–`(35, 35)`. The collector uses existing
epaint visual bounds and clips, after egui layer transforms, to exclude unrelated
shapes. It does not rasterize, blend, infer DOM identity, or receive an expected
color. Missing or ambiguous output, intersecting unsupported shapes, custom
callbacks, nonfinite geometry, and a different scale are capture errors.
A wrong opaque canvas color remains an observation and produces a mismatch.

The two original canvas fixtures have zero-height empty content, transparent bodies, zero margins,
padding and borders, and no text, images, external resources, scripts, animation,
or transforms. UI decorations are acceptable only outside the sample guard.
Fresh egui passes drain their own shapes; neither a second frame nor channel
silence is used as a readiness heuristic.

The opt-in AG2 Chromium path independently samples that pixel from an sRGB
screenshot with the same viewport and device scale. Neither path makes a
`getBoundingClientRect()` equivalence claim.

## Fixtures and independent expectations

`build.rs` discovers `fixtures/**/*.fixture.json` and generates embedded fixtures
in lexicographic ID order. `src/fixtures.rs` validates the entire catalogue before
selection, execution or artifact identity publication. No runtime checkout is needed.

- `canvas/root`: `html { background-color: #123456 }` yields RGB `(18, 52, 86)`.
- `canvas/cascade`: the `#canvas` rule sets `#345678`; a later `html` selector has
  lower specificity, so the expected RGB is `(52, 86, 120)`.

These expectations come from authored CSS and the
[CSS root/canvas background rule](https://www.w3.org/TR/css-backgrounds-3/#root-background),
not generated Borrowser output. Current production background selection prefers
a nontransparent body color; AG1 deliberately tests transparent bodies and does
not claim the full root/body propagation behavior is conformant.

To add a case, author static HTML, its strict `.fixture.json` declaration and any
independent `.expected.json` projection, then rebuild and run the harness. No Rust
registration or test-list edit is required. Explicit `requires_javascript` metadata
records eligibility; runnable fixtures must set it to false. There is no automatic
unsupported-feature detector or expectation blessing command. Follow the
[AG6 contract](../../docs/conformance/ag6-fixture-discovery-and-metadata.md).

## Results

Execution, exact comparison, and expectation metadata remain separate:

| Execution/comparison | Expectation | Status |
| --- | --- | --- |
| Observation matches | Match | PASS |
| Observation differs | Match | FAIL |
| Observation differs | KnownFailure | XFAIL |
| Observation matches | KnownFailure | XPASS |
| Explicit unsupported classification | No expectation needed | UNSUPPORTED |
| Explicit intentional skip | No expectation needed | SKIP |
| Harness/parser/publication/render/capture error | Any | ERROR |

Known failures retain mismatch details. Unexpected success exposes stale metadata.
Errors never satisfy known-failure metadata. The original six fixtures remain
runnable and expected to match. AG7 separately reports scoped relationships and
unsupported original upstream assertions; reference dependencies never count as
passed tests. See the AG7 status table for aggregation across these dimensions.

Reports include relative fixture paths, fixed sample location, expected/actual
hex colors, differing channel values, and fixed-order summary counts. They omit
time, internal engine identities, absolute paths and orchestration traces. Exit
precedence is 2 for errors; 1 for independent FAIL/XPASS or relationship failure;
3 for incomplete requested observations or unsupported runnable upstream
assertions; 0 otherwise. Unsupported/skipped cases
are counted separately and do not cause failure. CLI selection is all fixtures
or one exact identity; unknown identities are errors.

## Validation and exclusions

```sh
cargo test -p borrowser-conformance --locked
```

Tests execute the real fixtures, change authored CSS, deliberately change an
expectation, verify all classifications, reject ambiguous capture, and compare
stdout/stderr/exit across independent processes. Production runtime tests cover
terminal outcomes, publication ordering, cancellation and failure ownership.

## Local Chromium capture (AG2)

Provision the exact Chrome for Testing archive in
[`chromium-reference.json`](chromium-reference.json), verifying its SHA-256 before
extraction. The harness never downloads a browser, searches PATH, launches your
existing browser profile, or accepts an arbitrary browser version.

```sh
export BORROWSER_CHROMIUM_EXECUTABLE='/absolute/path/to/pinned/browser'
cargo run -p borrowser-conformance --locked -- --chromium
cargo run -p borrowser-conformance --locked -- --chromium --chromium-executable "$BORROWSER_CHROMIUM_EXECUTABLE" canvas/root
# Each invocation is a fresh standalone process, before any parser workers.
cargo test -p borrowser-conformance --locked real_chromium_ag5_read_only_inspection -- --ignored --nocapture --test-threads=1
cargo test -p borrowser-conformance --locked real_chromium_ag5_profile -- --ignored --nocapture --test-threads=1
```

On macOS, the executable is `Google Chrome for Testing.app/Contents/MacOS/Google
Chrome for Testing`; on Linux it is `chrome-linux64/chrome`. A missing or
incompatible browser is an error, including when an opt-in test is explicitly
requested. Ordinary AG1 execution and tests require no installed browser.

The `ag7-static-references-v1` profile uses one ordinary headless page. Fixed
read-only inspection permits only reviewed inert `author`, `help`, `match` and
`mismatch` links whose complete parsed facts equal the embedded source facts.
Mixed/resource-bearing or unknown relations remain rejected. A browser-default favicon candidate is aborted
before network delivery and accepted only after document eligibility, paired
Network/Fetch identities, an abort acknowledgment and its failed-load terminal
are verified. An aborted authored resource still fails capture. No HTML bytes,
fixture expectations or Chromium sandbox settings are changed.

`--chromium` emits `borrowser.chromium-phases.v2` capture-only JSON: fixture identity, requested native DOM/style/layout facts or RGB8 color,
environment, and exact browser identity. There are no PASS/FAIL classifications,
expectations or Borrowser comparisons in that report. Success requires verified
process cleanup; failure emits diagnostics to stderr, no observation to stdout,
and exits 2. Default invocation retains the AG1 report and exit behavior above.

The native lifecycle tests need normal process-inspection/signaling permissions;
restrictive execution sandboxes may reject them. The Chromium sandbox stays on.
Real-browser tests are explicitly ignored until requested; native helper tests
use isolated subprocesses and no installed browser.

See [the AG2 contract](../../docs/conformance/ag2-local-chromium-capture.md) for
FD ownership, bounded CDP execution, fixture delivery, process guarantees and the
platform qualification record. AG2 is integrated; both macOS arm64 and native
Linux x86-64 have recorded production-v2 qualification. Compilation on Linux
ARM64 does not establish runtime qualification.

No broad WPT import/execution, generalized adapters/providers, historical storage, dashboards,
flaky-test policy, or new engine semantics are included.

## Integrated cross-engine comparison (AG3 / #1395)

After the existing pinned-browser installation, one command performs the entire
comparison without collecting or copying reports:

```sh
cargo run -p borrowser-conformance --locked -- --compare --chromium-executable "$BORROWSER_CHROMIUM_EXECUTABLE" canvas/cascade
cargo run -p borrowser-conformance --locked -- --compare
```

The first command is the acceptance case; the second selects the discovered
catalogue in lexicographic fixture-ID order. The executable
option overrides the environment variable. Exactly one test ID or no ID is
accepted. Unknown/duplicate selections, extra options and missing required
configuration are errors. This does not provision a browser.

The original AG3 slice selects `canvas_color_rgb8`: opaque sRGB RGB8 at physical
pixel `(32, 32)`, CSS center `(32.5, 32.5)`. All existing viewport, scale, URL,
response, inline-only fixture and observation restrictions above apply unchanged.
The cascade fixture exercises an ID selector beating a later less-specific
selector, whose background reaches the observed canvas through each engine's
production path. This is a useful, deliberately bounded cross-engine fact. It
is not a direct computed-style or DOM observation, general layout test,
Borrowser GPU-rasterization test, or full screenshot comparison.

### Capture and projection boundary

Selection supplies exactly the same embedded HTML bytes to both engines.
`chromium::CaptureInput` contains fixture identity, source, bytes and an explicit
observation request: no expected values or expectation metadata. The shared crate-local `capture_batch`
entry is used by both `--chromium` and `--compare`. It returns each identity,
source, requested native inspection/sample and existing Chromium `BrowserIdentity`
only after verified cleanup. Borrowser's `execute_fixture` projects its selected
facts while the accepted frame borrow is alive, after normal parser, Browser
publication and frame/capture validation. Cross-engine common representations are
separate from native capture data.

Engine-specific capture records remain under `captures`. Chromium records its
observed product/version/revision/protocol/platform/profile; Borrowser records
its `guarded-egui-canvas` observation method and observed color. Egui shapes,
PNG pixels and CDP responses are validated transiently, then discarded; there
are no raw screenshot archives or retained protocol logs. These capture records
are distinct from the common `fields` projection. Provenance is inspectable but
is not compared as a semantic field.

The projection uses the existing `CanvasColor([u8; 3])`, with red, green and blue
in that order. Both observed samples are compared exactly, without tolerance,
rounding or CSS re-interpretation. Required capture count, identity, source and
order are checked before pairing or starting Borrowser. Missing, extra,
duplicate or mismatched captures are errors, never omitted comparisons.

### Whole-invocation lifecycle

The standalone capture entry requires no existing parser/runtime workers,
concurrent descriptor openers, unrelated children or competing reapers. It is
not a general concurrent library launcher.

Every selected runnable fixture is captured in Chromium, sequentially, with
verified process/artifact cleanup before the next launch. After the final
capture and cancellation check, the standalone signal guard is released. Only
then does the harness start any Borrowser parser worker. There is no
Chromium/Borrowser per-fixture interleaving, even for multiple fixtures. The
Borrowser phase performs no child-process launches or native launcher/reaper
operations. All AG2 deadlines, cancellation, sandbox and ownership rules remain.

A parser terminal publication or return from `execute_fixture` does not establish
worker termination. Dropping its command sender eventually closes the worker's
input, but AG3 does not wait heuristically or rely on that shutdown to make a
later launch safe: there is no later launch. Linux's existing process-global
subreaper setting persists until invocation exit; the Borrowser phase creates
no child processes. Restored signal dispositions apply after the capture phase.

### Report and failure semantics

`--compare` emits pretty JSON, schema `borrowser.cross-engine-phases.v2`, followed
by one newline. Member order is fixed by typed serialization; case order is
fixture-ID order; engine order is Borrowser then Chromium. Each canvas case has
one `canvas_color_rgb8` field with both observed arrays, its `equal`/`different`
status, and red/green/blue channel statuses in that order. For `canvas/cascade`,
both arrays should be `[52, 86, 120]` and every channel should be `equal`.

DOM, style and layout cases expose their common values under `projections`,
with native Chromium data and Browser frame provenance under `captures`.
Case status is `equal_for_requested_scope`, `different` or `incomplete`; family
coverage explicitly shows unrequested and unsupported observations. The AG5
contract defines the selected properties, geometry, field paths and bounds.
The passing six-fixture baseline compares exactly 34 fields: two canvas, two
whole-DOM, twelve style and eighteen layout fields. Requested record identities,
order, counts and states are validated before comparison; a truncated projection
is a required-data error even with KnownFailure metadata. An empty layout target
list is invalid configuration. Complete missing-node and zero-box records remain
data, and different tree/rectangle lengths remain structural differences.
When native DOM is included, style/layout presence must agree with active-document
HTML-id lookup, excluding inert template contents, in local and AG4 ingestion.
Native `inner_width`/`inner_height` record the configured 640×480 viewport,
including scrollbars. `layout_width` records the root client width, excluding
a classic vertical scrollbar; it is not assumed equal to 640. The AG5 contract
defines the strict family-specific viewport checks and bounded failure metrics.
Canvas sampling retains the full client-size restriction. Producers and local/AG4
consumers must be rebuilt together; missing required viewport facts are errors.

`borrowser_expectation` separately reports the independently authored expected
RGB8, expectation metadata, optional known-failure reason and existing AG1
classification. It reuses the same successful Borrowser observation, without a
second execution. Equal engine observations can still produce an expectation
`FAIL`; Chromium never supplies or blesses expected values. `XFAIL` does not
suppress a cross-engine difference.

Explicitly unsupported fixture metadata makes the selected field unavailable
with a reason; unsupported fields have no channel comparisons. Explicit skips
remain skips. Neither classification executes an engine. Fields outside this
projection have no placeholder rows. An adapter error, missing sample,
unsupported runtime platform or unavailable executable is an error, not an
unsupported field. Runnable cases request supported AG5 fields; excluded cases retain explicit
classification without entering either engine. Zero comparable observations never mean successful equality.

The summary counts equal/different/unsupported fields, skipped cases, and
independent expectation failures/unexpected passes. Exit precedence is:

| Exit | Meaning |
| --- | --- |
| 2 | Configuration, execution, capture, required-data, cleanup, serialization or output error |
| 1 | Cross-engine difference, independent FAIL/XPASS, or relationship failure |
| 3 | Otherwise incomplete: requested/upstream coverage, excluded selection or no comparable observations |
| 0 | All selected cases compared equally, without independent expectation failure |

Exit 3 also marks incomplete requested observations in local AG5 execution.
Local text diagnostics and status use the same validated typed expectations;
integer and floating JSON forms of the same CSS-pixel value do not create a
false difference. Text output preserves unsupported identities/reasons,
incomplete coverage explanations and known-failure reasons alongside differences.
Existing canvas and fixture-classification behavior is preserved. Capture/execution failures suppress the entire comparison report,
including earlier successful cases. Chromium primary and cleanup failures remain
separate stderr diagnostics. Execution, validation, report construction and JSON
serialization finish before stdout publication. Writes and flush are checked;
publication failure exits 2, but bytes already written cannot be retracted.
Diagnostic helpers propagate I/O failures to the terminal CLI boundary, which
preserves exit 2 without panicking if stderr also fails. Diagnostics may then
be unavailable or partial; they are never redirected into comparison stdout.
Native cleanup retains its directory before attempting the retention diagnostic.
Retention and emergency-cleanup diagnostics use non-panicking writes: failed
stderr cannot interrupt the finished bookkeeping, trigger another cleanup
attempt, change cleanup errors or add a panic during an existing unwind. An
unrelated original panic remains a panic, rather than being mapped to exit 2.
Successful reports exclude process IDs, temporary paths, timing and diagnostic
ordering. Platform provenance is retained, so byte stability is claimed for the
same inputs and pinned environment, not across different platforms.

### AG3 validation

Pure tests cover exact equality/differences, unsupported availability, ordering,
missing captures, independent expectations, exit precedence and write/flush
failures (including partial output and failed diagnostics). CLI regressions close
stderr's pipe reader before spawning the binary and require exit 2 with empty
stdout for configuration and invalid-UTF-8 errors. These checks and existing
Borrowser execution run without Chromium. Native lifecycle helpers remain
isolated subprocesses.

`native_cleanup_diagnostic_failures` uses the existing isolated native wrapper
to exercise explicit finish, emergency Drop and Drop during an existing unwind,
each with writable and closed-pipe stderr. A test-only deadline fault fires
after actual native reaping, while directory ownership is still retained.
Instrumentation checks the diagnostic results and exactly one completed cleanup
attempt. Tests verify returned errors or the original unwind payload, retained
evidence and absence of children before removing only their own directory.

```sh
cargo test -p borrowser-conformance --locked comparison::tests::unit_
cargo test -p borrowser-conformance --locked --test comparison_cli
cargo test -p borrowser-conformance --locked
# Each invocation is a fresh standalone process, before any parser workers.
cargo test -p borrowser-conformance --locked real_chromium_ag5_read_only_inspection -- --ignored --nocapture --test-threads=1
cargo test -p borrowser-conformance --locked real_chromium_ag5_profile -- --ignored --nocapture --test-threads=1
```

Explicit real-browser tests require `BORROWSER_CHROMIUM_EXECUTABLE` and fail if it
is missing. They include repeated independent combined CLI executions, multiple
fixtures, real authored-CSS sensitivity through both paths, and isolated
composition failures. Test-only instrumentation records capture start, verified
cleanup, standalone release and the actual parser-worker startup call. The
outer composition wrapper invokes every isolated scenario; private entries are
not standalone test selections:

```sh
cargo test -p borrowser-conformance --locked -- --include-ignored \
  --skip chromium::tests::native_case --skip chromium::tests::browser_helper \
  --skip comparison::tests::isolated_case --test-threads=1 --nocapture
```

The existing Linux pinned-browser lane includes combined execution and retains
its strict empty-runtime-directory gate. Local `make ci` does not replace real
Chromium/native platform qualification. AG3 adds no DOM/style/geometry coverage,
WPT integration, remote execution or engine behavior.

### Historical AG3 qualification: 2026-10-12

On 2026-10-12, macOS 27.0 arm64 (26A428) qualification with the unchanged pinned
Chrome for Testing 155.0.8059.39/profile v2 passed all **52** applicable tests in
the complete configured-browser invocation above. The three private entry points
were excluded from direct selection; their outer wrappers executed **50** AG2
native scenarios and all **six** AG3 composition scenarios. Repeated combined CLI
runs passed for both exact `canvas/cascade` selection and the full registry.
The acceptance command returned exit 0 with Borrowser and Chromium both observing
`[52,86,120]`, one equal field, and independent AG1 `PASS`. All-fixture comparison
returned two equal fields. Separate default AG1 and capture-only AG2 commands
retained their existing reports. The dedicated runtime directory was empty after
qualification. A sandbox-restricted native attempt could not obtain macOS process
ownership authority; qualification used normal native permissions, and its
verified test-only leftovers were removed.

Targeted tests, the ordinary conformance suite (47 passed, eight explicit
ignores), warnings-denied conformance Clippy, build/format checks and full local
`make ci` also passed on 2026-10-12. The full CI exit was 0; existing release/bench
warnings in unchanged HTML/CSS code were not expanded into this issue.

At that checkpoint, native AG3 Linux x86-64 qualification was pending. The
available local Docker host was Linux ARM64 and could not establish that result;
historical AG2 Linux evidence did not qualify AG3.

The 2026-10-12 results above predate the terminal and native-cleanup diagnostic-I/O
corrections and remain historical evidence. They do not qualify the corrected
source. The following record supersedes that checkpoint's pending qualification.

### Corrected AG3 implementation qualification: 2026-10-13

Implementation commit `c0f0a3fe5a386a90cc777ec339971e4c5670920a`, tree
`cda41beb94d11cceff9ebf302bc09e1caaca5861`, passed qualification on both intended
native platforms. The [AG3 PR qualification record](https://github.com/joris97jansen/borrowser/pull/1432)
records the local commands and exits; [CI run 38349078438, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/38349078438)
and its [native qualification artifact](https://github.com/joris97jansen/borrowser/actions/runs/38349078438/artifacts/11790930071)
provide the Linux logs, environment, captures, comparison and cleanup evidence.

- macOS 27.0 arm64, build 26A428: the configured-browser command above passed
  **57 top-level tests**. Targeted comparison/publication, terminal-diagnostic and
  native cleanup-diagnostic tests, warnings-denied conformance Clippy,
  `make fmt-check`, `git diff HEAD --check` and full local `make ci` exited 0.
- Native Ubuntu 24.04.5 x86-64 VM, image `20261004.327.1`, kernel
  `6.17.0-1022-azure`, non-root runner: the configured suite passed **56 top-level
  tests**. The existing environment checks and browser sandbox policy were
  unchanged. All 11 CI jobs passed, as did GitGuardian;
  `gh run watch --exit-status` returned 0.
- Both used checksum-verified Chrome for Testing **155.0.8059.39**, revision
  `@3ff7ac5a9224be9156d7f8703a06e22890aafd34`, capture profile
  `ag2-canvas-srgb-v2`, with the existing platform-specific archive pins.

On each platform, outer wrappers exercised **56 distinct native helper
scenarios**, including the six cleanup-diagnostic scenarios, and **six AG3
real-Chromium composition scenarios**. These nested scenario counts are not
additional top-level tests; the platform-specific top-level totals need not
match. Excluding the three private helper entries from direct selection did not
exclude their wrappers. Terminal closed-stderr, partial-write/flush and failed
diagnostic regressions passed, together with real authored-CSS sensitivity and
three independent combined CLI runs for each of `canvas/cascade` and the full
registry.

The explicit macOS acceptance command and Linux CI acceptance step both returned
exit 0: Borrowser and Chromium observed `[52,86,120]` for `canvas/cascade`, with
one equal field and independent AG1 `PASS`. Full-registry comparison returned two
equal fields and two independent `PASS` results: `canvas/root` `[18,52,86]` and
`canvas/cascade` `[52,86,120]`. This was checked by the explicit macOS command and
the repeated CLI regression on both platforms. Default AG1 and capture-only AG2
retained their reports; all four explicit macOS CLI commands exited 0, and the
Linux configured CLI regressions and capture/compare step passed.

Build temporary storage and conformance runtime storage were separate in the
successful runs. The final macOS configured suite and each explicit CLI command
left the dedicated runtime directory empty. Linux's strict cleanup step passed;
`remaining-artifacts.log` was empty and `result.txt` recorded cleanup verification.
These results include the retained-directory and emergency-Drop diagnostic fixes,
including Drop during an existing unwind.

An initial local `make ci` exited 0, but its subsequent directory gate **failed**:
the conformance target-runner override had mistakenly been applied to the entire
workspace, so the unchanged HTML API-boundary test put nested Cargo builds in
runtime storage. The failed gate's logs and directory were retained. Full CI was
rerun with build-only `TMPDIR`, then the complete configured suite and all four
CLI commands were rerun in a fresh runtime directory; their exits and cleanup
checks passed. No source, cleanup assertion or artifact exclusion changed.

The inspected native and main Rust checkout logs establish this exact tested
integration for run 38349078438, attempt 1:

- Source head: `c0f0a3fe5a386a90cc777ec339971e4c5670920a`.
- Base (`master`): `f6574c421f165119227f16af72c497a3fc9e0965`.
- Actual checkout/test commit: `d7479758bf5281707f774abb76698de232de5d2f`, whose
  parents are that base and source head.
- Tested tree: `cda41beb94d11cceff9ebf302bc09e1caaca5861`, identical to the
  qualified implementation tree.

Implementation qualification passed. At this documentation checkpoint, the
documentation update and its resulting integration candidate still require final
CI and read-only integration review; their subsequent status is recorded in the PR.
The original macOS run qualifies the
unchanged implementation, not a later documentation commit. This record does not
claim that the PR is merged or AG3 is closed, or extend coverage beyond the
approved exact canvas sample.

## AG7 contributor validation

Keep raw sources under `fixtures/external/raw/` unchanged. Pin repository revision,
paths, SHA-256 and licence/attribution in separate sidecars. Declare a single
source-authorized sibling reference and one meaningful present background-color
target. Independently justified expectations are optional for relationships;
unsupported JS/platform cases have no execution observations. See the
[complete authoring workflow](../../docs/conformance/ag7-focused-external-tests-and-references.md#adding-another-supported-case).
Distribute the pinned WPT licence with embedded binaries as well as sources.

```sh
cargo test -p borrowser-conformance --locked --test fixture_build
cargo test -p borrowser-conformance --locked --bin borrowser-conformance relationship::tests::
cargo test -p borrowser-conformance --locked --bin borrowser-conformance tests::ag7_malformed_expected_colors_are_errors_even_under_known_failure -- --exact
cargo test -p borrowser-conformance --locked --test comparison_cli acceptance_
cargo test -p borrowser-conformance --locked
cargo run -p borrowser-conformance --locked -- wpt-style/background-color-mismatch
# These require the pinned browser and native process ownership permissions.
cargo test -p borrowser-conformance --locked --bin borrowser-conformance chromium::tests::real_chromium_ag7_inert_links -- --exact --ignored --nocapture --test-threads=1
cargo test -p borrowser-conformance --locked --test comparison_cli real_chromium_ag7_scoped_relationships -- --exact --ignored --nocapture
cargo test -p borrowser-conformance --locked --test comparison_cli real_chromium_combined_cli_is_stable_and_selects_exactly -- --exact --ignored --nocapture --test-threads=1
cargo test -p borrowser-conformance --locked --test fixture_build isolated_cargo_rebuild_and_standalone_execution -- --exact --ignored --nocapture
```

The initial-corpus test-owned oracle checks exactly four RGB differences, both
roles and values, expectations, per-engine relationships, coverage and exits.
Fixed counts apply only to the explicitly named initial subset. Whole-catalogue
checks discover declarations and authored expectations, verify ordered capture
descriptors and coverage, and account for additional passing cases without a Rust
registration/count edit. The isolated test keeps a new passing reference case
present through ordinary CLI qualification and source-free Chromium execution.
Both roles retain raw native capture evidence separately from projections.
An additional unrelated canvas/cascade difference fails the acceptance checker.
Temporary additions have explicit test-owned contributions; no production
registration list or automatic blessing exists. Native macOS results are not
native Linux or live-AWS qualification. Historical AG4 evidence is unchanged.
Required native Linux x86-64 qualification and applicable CI passed for the
recorded AG7 implementation; see the
[AG7 qualification record](../../docs/conformance/ag7-focused-external-tests-and-references.md#qualification-record-2026-10-21).
The historical combined-CLI shutdown and composition incidents remain unresolved,
with separate owner dispositions pending. Successful qualification does not
establish their causes or add live AWS qualification.
