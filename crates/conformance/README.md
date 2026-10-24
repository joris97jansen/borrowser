# Focused static conformance harness (AG1–AG9)

Borrowser has focused deterministic conformance infrastructure for its current
static HTML/CSS scope and can compare selected behavior against Chromium.
This is not broad WPT compliance, CSS compatibility, visual equivalence or general
browser compatibility. AG9 consolidates the existing AG1–AG8 implementation.

## Start here

From the repository root, with the pinned Rust toolchain and native build
dependencies installed:

```sh
make conformance-ci
cargo run -q -p borrowser-conformance --locked -- canvas/cascade
cargo run -q -p borrowser-conformance --locked -- --suite extended
```

The CI suite has seven cases/eight executable documents and currently exits 0.
Extended has eleven cases/twelve executable documents and intentionally returns
binary exit 3: seven PASS, two INCOMPLETE, two UNSUPPORTED, plus two independent
XFAIL facts. Read the results even when the command is nonzero.
`make conformance-extended` runs the same selection but Make reports its own
nonzero recipe status. Neither local suite requires Chromium, network resources or AWS.

On **native Linux x86-64**, first follow the
[Linux Chromium setup](../../docs/conformance/ag2-local-chromium-capture.md#supported-linux-setup).
Then run:

```sh
cargo run -q -p borrowser-conformance --locked -- --compare --suite ci
cargo run -q -p borrowser-conformance --locked -- --compare --suite extended
```

The pinned executable is supplied through `BORROWSER_CHROMIUM_EXECUTABLE` or
`--chromium-executable PATH`. CI comparison currently exits 0; extended comparison
exits 1 with the four known RGB differences described below. `--chromium` captures
only; `--compare` executes both engines and compares selected facts. macOS Chromium
execution is withdrawn and returns an infrastructure error, with no fallback.
Borrowser-only execution on macOS remains supported. AWS is an optional remote
Linux capture path with its own existing prerequisites, not a local fallback.

Continue with [adding a fixture](#add-and-classify-a-fixture),
[reading results](#results), or [investigating a difference](#investigate-a-difference).

## Harness map and supported observations

| Location | Responsibility |
| --- | --- |
| `build.rs`, `build_support/`, `src/fixtures.rs` | Discover and embed declarations; validate the catalogue; select exact IDs or fixed named suites. |
| `fixtures/` | Native HTML inputs, `.fixture.json` declarations and independent `.expected.json` projections. |
| `fixtures/external/raw/`, `fixtures/external/expected/` | Unchanged pinned upstream inputs/references/licence and separate authored expectations; sidecars live under `fixtures/external/`. |
| `fixtures/wpt-style/` | Locally authored static reference cases, explicitly distinct from upstream WPT coverage. |
| `src/execute.rs`, `src/environment.rs` | Supply fixed document bytes at the production resource boundary and observe an accepted production frame. |
| `src/chromium/`, `chromium-reference.json` | Pinned native capture, trusted read-only inspection and process cleanup. |
| `src/projection/`, `src/relationship.rs`, `src/comparison*`, `src/report.rs` | Select common values, evaluate exact comparisons and relationships, report independent results and coverage. |
| `src/aws/`, `aws/`, `../../tools/conformance/` | Optional existing disposable-host capture, artifact preparation and recovery. |

| Observation | Supported comparison | Limit |
| --- | --- | --- |
| Parser/DOM | Browser-committed document structure, doctype, namespaces, attributes, exact text/comments, child order and template contents. | Not parser-internal snapshots, DOM scripting or broad parser compliance. |
| Computed style | Selected unique-ID targets: `color`, `background-color`, `font-size`, `display` in AG5's explicit value domains. | Not arbitrary CSSOM values, additional properties or CSS compatibility. |
| Layout | Selected direct DOM-backed border boxes in document CSS pixels, including distinct missing/hidden/zero-sized results. | Fixed supported block geometry; no whole-box-tree or general layout equivalence. |
| Paint | Guarded opaque sRGB canvas background at pixel `(32,32)`. | No general element/text painting, GPU rasterization or screenshot equivalence. |
| Reference relationship | One direct match/mismatch over a present target's computed background color. | Original upstream full visual assertions remain unsupported. |

[AG5](../../docs/conformance/ag5-phase-comparison-projections.md) owns exact
observation domains and projection rules; [AG7](../../docs/conformance/ag7-focused-external-tests-and-references.md)
owns scoped reference semantics. Native capture/provenance remains separate from
common comparison projections. Tooling does not recompute CSS, layout or paint.

## Catalogue and suite contracts

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

AG8 adds fixed `ci` (seven cases/eight documents) and `extended` (eleven
cases/twelve executable documents) suites. See the [AG8 reporting and suite
contract](../../docs/conformance/ag8-deterministic-reporting-and-suites.md) for
membership rationale, derived case counts, run errors, reproduction and strict
exit policy. `make conformance-ci` is the bounded Borrowser-only CI entry;
`make conformance-extended` preserves incomplete coverage (currently exit 3 from
the binary, propagated as a nonzero Make recipe). Explicit Chromium comparison
uses `--compare --suite ci|extended` on supported native Linux x86-64. AWS is
optional and has no suite policy. New fixtures do not expand either named suite.

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
registration or test-list edit is required for discovery; named AG8 suites require
an explicit reviewed membership change. Explicit `requires_javascript` metadata
records eligibility; runnable fixtures must set it to false. There is no automatic
unsupported-feature detector or expectation blessing command. Follow the
[AG6 contract](../../docs/conformance/ag6-fixture-discovery-and-metadata.md).

## Add and classify a fixture

1. Choose an existing observation from the table above and read its AG5 limits.
   Start from the matching native fixture, keeping HTML static and self-contained.
   Fixture HTML and metadata paths are relative to `crates/conformance/fixtures/`;
   references must stay within the sidecar directory or its descendants.
2. Add UTF-8 HTML and a unique-ID `.fixture.json`. For a minimal canvas example,
   copy `root-canvas-color.html` to `example-canvas.html`, change the authored
   root color to `#234567`, and add `example-canvas.fixture.json`:

   ```json
   {
     "id": "canvas/example",
     "input": "example-canvas.html",
     "requires_javascript": false,
     "classification": { "kind": "runnable" },
     "observations": { "canvas": { "kind": "compare" } },
     "expectation": { "kind": "match" },
     "expected": { "kind": "canvas", "rgb8": [35, 69, 103] }
   }
   ```

   The RGB bytes come from the authored hexadecimal color, not a captured result.
   Keep the original empty body and geometry so the canvas guard remains valid.
   For DOM/style/layout, use a separate independently authored expected projection
   following the corresponding existing `.expected.json` and AG5 request rules.
3. Classify eligibility honestly. `classification.kind` is `runnable`,
   `unsupported` or `skipped`; exclusions require a nonblank reason and omit
   expectation data. A known engine defect is still runnable: use
   `expectation: { "kind": "known_failure", "reason": "..." }` with the correct
   independent expected value. This does not suppress a cross-engine difference.
   JavaScript requirements are explicit metadata, never permission to execute it.
4. Rebuild and run the exact ID:

   ```sh
   cargo run -q -p borrowser-conformance --locked -- canvas/example
   ```

   On supported Linux with the pinned browser, optionally compare it:

   ```sh
   cargo run -q -p borrowser-conformance --locked -- --compare canvas/example
   ```

   This example should produce independent PASS and equal canvas observations.
   Discovery requires no Rust registration. The whole catalogue is validated
   before selection, and orphan inputs or malformed declarations fail explicitly.
   Adding a fixture does not add it to `ci` or `extended`; suite changes require
   separate explicit membership review within the relevant fixture change.
5. Inspect expected/actual values, coverage and reproduction output. Add regression
   assertions appropriate to new coverage. Never bless captured output or change
   an expectation merely to remove a difference.

The [AG6 authoring contract](../../docs/conformance/ag6-fixture-discovery-and-metadata.md)
defines the strict schema and source-free embedding. For upstream tests, follow
[AG7's import path](../../docs/conformance/ag7-focused-external-tests-and-references.md#adding-another-supported-case):
preserve exact raw bytes, pinned revision/path/SHA-256 and licence/attribution;
keep metadata and independent expectations separate. A supported relationship
must match the source's direct sibling reference and use the existing present-ID
background-color scope. Do not insert IDs, rewrite upstream HTML, emulate page
scripts or claim the original visual assertion passed. Unsupported reference
forms and JS/platform tests are classified with reasons. There is no generic
WPT importer or full WPT runner.

## Results

Execution, exact comparison, and expectation metadata remain separate:

| Independent observation or execution | Expectation | Independent status / exclusion |
| --- | --- | --- |
| Observation matches | Match | PASS |
| Observation differs | Match | FAIL |
| Observation differs | KnownFailure | XFAIL |
| Observation matches | KnownFailure | XPASS |
| Explicit unsupported classification | No expectation needed | UNSUPPORTED |
| Explicit intentional skip | No expectation needed | SKIP |
| Harness/parser/publication/render/capture error | Any | ERROR |

These independent statuses are not the whole case verdict. A satisfied scoped
relationship can produce PASS without an independent expectation. A runnable
upstream visual assertion outside the observed scope makes its aggregate verdict
INCOMPLETE even when its independent expectation is XFAIL and its relationship
is satisfied. A matching partial projection is also INCOMPLETE. FAIL/XPASS and
ERROR retain their precedence; a known mismatch can still be XFAIL with partial
coverage. See [AG8's aggregate status rules](../../docs/conformance/ag8-deterministic-reporting-and-suites.md#case-totals-and-independent-facts).

`SUMMARY` counts each case once; independent expectations, reference documents,
cross-engine fields and relationships have separate counts. Current extended
results have aggregate XFAIL=0 but `INDEPENDENT_EXPECTATIONS XFAIL=2`.
The two RGB cases have four cross-engine differences: test and reference for
each of `wpt/rgb-alias-p6` and `wpt/rgb-comments-p6`, at
`style:#p6/background-color`. Borrowser yields transparent `[0,0,0,0]` while
Chromium yields opaque `[10,150,50,255]`. Both engines still satisfy their own
scoped match relationships. That does not establish cross-engine equality.

Fatal invocation errors have a separate stderr `RUN_ERROR` and reproduction
record. They never create results for unexecuted cases. A real entered-case
ERROR is distinct. Reject any comparison output from exit 2, including partial
bytes after an output-write failure. Exit precedence is **2, then 1, then 3,
then 0**, not numerical severity order.

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
are counted separately and do not cause failure. CLI selection is all fixtures, one exact identity, or an explicit named suite
optionally narrowed to a member; unknown/out-of-suite identities are errors.

## Investigate a difference

1. Check the exit and stderr first. Exit 2 is configuration, execution, capture,
   required-data, cleanup or publication failure, not an engine mismatch or skip.
   Preserve the primary and cleanup diagnostics; do not interpret partial JSON.
2. Use the report's exact-ID reproduction command from the same source checkout.
   Check fixture metadata, source hashes, requested fields, fixed environment,
   capture profile and pinned Chromium identity. Rebuild after fixture edits.
   For example, run the known RGB case independently and then compare on Linux:

   ```sh
   cargo run -q -p borrowser-conformance --locked -- wpt/rgb-comments-p6
   cargo run -q -p borrowser-conformance --locked -- --compare --chromium-executable "$BORROWSER_CHROMIUM_EXECUTABLE" wpt/rgb-comments-p6
   ```

   The current local result is INCOMPLETE/exit 3 with an independent XFAIL;
   comparison returns exit 1 with two differing fields, one per document role.
3. Read `borrowser_expectation`, aggregate `borrowser_status`, cross-engine
   `status`, `fields[].differences`, `coverage`, `upstream_assertion` and
   `relationships` separately. Inspect `reference_document` as well as the test.
   A relationship compares test and reference within one engine. Cross-engine
   fields compare the same document between engines.
4. Inspect `captures` for native evidence/provenance and `projections` for the
   selected common facts. DOM order/text, style values and layout edges have
   exact AG5 semantics; no offsets, tolerances or CSS recomputation are allowed.
   Use `--chromium` with the same exact ID if capture-only output is useful;
   it contains no expectations or comparison verdicts. There are no retained
   screenshot archives or general pixel-diff tools in this harness.
5. Decide whether the finding is invalid/missing infrastructure data, a projection
   defect, a pre-existing engine gap, or a regression in a previously supported
   observation. Minimize only a local copy; preserve upstream provenance and the
   original independent expectation. Direct a semantic defect to HTML/parser,
   CSS, Layout, Paint or Browser according to its production owner. Do not fix
   engine behavior inside a collector or suppress differences with KnownFailure.

## Optional AWS Chromium workflow

The existing AG4 path runs the same capture mechanism on a disposable Linux host,
retrieves and validates its observations, confirms cleanup, then executes the
local Borrowser comparison. It accepts the catalogue or one exact ID, **not**
`--suite`. Shared networking/EICE and operator permissions are prerequisites;
the run-owned instance and security group are cleanup targets, not shared resources.
Ordinary conformance commands never require cloud credentials or provisioning.

Follow the [AG4 artifact and operator contract](../../docs/conformance/ag4-disposable-aws-chromium-runner.md#toolchain-and-immutable-artifacts)
before using a prepared, source-matched bundle:

```sh
/absolute/private/ag4-bundle/borrowser-conformance-controller \
  --compare-aws --config /absolute/private/ag4-operator.json \
  --artifact-dir /absolute/private/ag4-bundle \
  --state-dir /absolute/private/ag4-operations canvas/cascade
```

Keep the operation state and diagnostics until termination is confirmed. Recovery
uses the existing artifact-independent cleanup entry:

```sh
/absolute/private/ag4-bundle/borrowser-conformance-controller --aws-cleanup \
  --operation /absolute/private/ag4-operations/RUN_ID \
  --aws-cli /absolute/path/to/pinned/aws
```

See [AG4 recovery](../../docs/conformance/ag4-disposable-aws-chromium-runner.md#durable-operation-lifecycle-and-recovery)
for identity checks and terminal cleanup evidence. A cleanup failure is an error,
never successful comparison. Existing completed AG4 qualification retains its
recorded scope; AG9 does not require new resources or claim fresh live AWS evidence.

## Validation and exclusions

```sh
cargo test -p borrowser-conformance --locked
```

Tests execute the real fixtures, change authored CSS, deliberately change an
expectation, verify all classifications, reject ambiguous capture, and compare
stdout/stderr/exit across independent processes. Production runtime tests cover
terminal outcomes, publication ordering, cancellation and failure ownership.

## AG9 closeout evidence

AG9 changes documentation only. The implementation baseline is merge commit
`d883e006ae84bbd4f6c47ba993491340ff12f7b0`, tree
`06e42ff58e2049eafc0db1bc9c6996605834484d`. Rust, fixtures, expectations, browser
pins, CI semantics and AWS machinery are unchanged. Documentation changes create
a different working tree; they are not a newly qualified native execution tree.

### Fresh AG9 local validation (2026-10-24)

Host: macOS 27.0.1 arm64, Rust/Cargo 1.92.0. Commands ran from the repository
root against the baseline above plus this documentation-only diff. No real
Chromium was launched and no AWS calls were made. These results are separate
from the retained native Linux and live AWS qualification below.

| ID | Actual command | Actual result |
| --- | --- | --- |
| V1 | `cargo test -p borrowser-conformance --locked --test fixture_build --test cli --test comparison_cli --test chromium_cli --test aws_cli -- --test-threads=1` | Exit 0; 29 passed, 5 ignored. The ignored tests do not count as executed qualification. |
| V2 | `cargo test -p browser --lib --features conformance-observation --locked tab::tests::observation::` | Exit 0; all 6 observation tests passed, 245 filtered out. |
| V3 | `env -u BORROWSER_CHROMIUM_EXECUTABLE cargo test -p borrowser-conformance --locked --test fixture_build isolated_cargo_rebuild_and_standalone_execution -- --exact --ignored --nocapture` | Exit 0; the one named test passed, including 4 nested ordinary CLI tests. Add/edit/remove and source-free execution passed; optional Chromium check explicitly not executed. |
| V4 | `cargo test -p borrowser-conformance --locked -- --test-threads=1` | Exit 101; binary tests: 177 passed, 9 failed, 13 ignored. Native helper ownership inspection failed; later integration targets were not reached. |
| F1 | `cargo test -p borrowser-conformance --locked --bin borrowser-conformance chromium::tests::native_macos_bsd_ownership_states -- --exact --nocapture --test-threads=1` | Exit 101 outside the execution sandbox; one named test failed, 198 filtered out. This did not establish a successful native qualification. |
| V5 | `make conformance-ci` | Exit 0; exactly 7 PASS, no FAIL/XPASS/ERROR/INCOMPLETE. |
| V6 | `cargo run -q -p borrowser-conformance --locked -- --suite extended` | Exit 3; exactly 7 PASS, 2 INCOMPLETE, 2 UNSUPPORTED, with 2 independent XFAIL facts and 3 satisfied scoped relationships. |
| V13 | `make ci` | Exit 2 outside the execution sandbox. CI fixtures, formatting and all three Clippy lanes passed; workspace tests stopped at the same 9 native helper failures (177 passed, 13 ignored in that binary). Subsequent pipeline stages did not run. |

V6's exact case identities, correct independent expected RGBA values, observed
transparent values, relationship results and exclusions were inspected; its
nonzero result was not accepted solely by exit code. No expectations or reports
were altered. No filtered zero-test command was counted as validation.

The nine V4/V13 failures are `chromium::tests::native_capture_failure_diagnostics`,
`native_cleanup_diagnostic_failures`, `native_lifecycle_regressions`,
`native_macos_bsd_ownership_states`, `native_process_and_pipe_contract`,
`native_resource_completion_and_shutdown`, `native_shutdown_uses_capture_deadline`,
`native_startup_exit_event_classification` and `native_temporary_artifact_containment`.
The restricted run reported `task_name_for_pid: Mach 5`. The focused retry and
full CI outside that sandbox instead reported a live process executable
unavailable during ownership scanning, with incomplete-cleanup diagnostics and
retained test artifacts. Removing the execution sandbox did not resolve host
inspection availability. The affected ambient process/underlying cause was not
established, and successful cleanup is not inferred for these failed helpers.

These remain **failed local checks**, not skips, expected fixture failures or
freshly passing CI. They exercise the retained macOS native supervisor whose
production Chromium backend is already withdrawn by AG7. This is a category B
platform limitation in unchanged code, not evidence of a new parser/style/layout/
paint defect or a reason to restore macOS inside AG9. The supported native Linux
baseline passed its applicable CI and native qualifications. With no executable,
fixture or CI changes in AG9, that exact-source evidence and the fresh local
workflow checks satisfy the focused milestone's supported-platform requirements;
AG9 does not claim green full CI on this macOS host.

### Retained native Linux evidence

[AG8 PR #1437](https://github.com/joris97jansen/borrowser/pull/1437) qualified
commit `66cc60339b0950f791f4cafed81f17fc33305396` in
[run 40067542665, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/40067542665).
Its PR test merge `c2d6dc4001a43be0f0ecdc4d98aefc9e49a7a88a` and final merge
have the exact baseline tree above. The [AG8 completion record](https://github.com/joris97jansen/borrowser/issues/1428#issuecomment-6381065501)
records closure of AG8, not AG9.

The retained environment is native Ubuntu 24.04 x86-64, Chrome for Testing
155.0.8059.39, revision `@3ff7ac5a9224be9156d7f8703a06e22890aafd34`, CDP 1.3,
profile `ag7-static-references-v1`. The complete Linux archive SHA-256 is
`55672d1f392fd3e7b7a08621b6e804e6bcb39d40cf155504abb74b3a021ea8ea`.
AG9 rechecked the retained eighteen artifact digests, full CI log archive digest,
exact Git tree, unchanged browser pin and explicit successful test executions.
The retained CI archive digest is
`7990b2289996ab1f65a9ff66d2497169423a5ac60bb6b11416e4c5d040d3823e`.

All ten explicitly invoked real-browser qualifications executed successfully,
including `real_chromium_ag5_profile`,
`real_chromium_combined_cli_is_stable_and_selects_exactly`,
`real_chromium_ag7_scoped_relationships` and
`ag8_real_chromium_suites_are_stable`. The baseline assertions cover two whole-DOM,
twelve style, eighteen layout and two canvas fields: 34 equal fields with six
independent PASS results. Suite assertions establish CI exit 0 with seven cases /
eight documents and extended exit 1 with eleven cases / twelve documents,
exactly four known RGB differences and no additional differences. Both suite
reports were byte-identical across independent processes in that pinned environment.
All thirteen composition scenarios passed and final cleanup inventories were empty.

Evidence consists of retained untruncated execution logs, captured artifacts and
strict assertions in the unchanged tests. Individual suite JSON was captured by
the test, not separately exported; the retained `comparison.json` is canvas-only.
All twelve applicable workflow jobs and GitGuardian passed; the branch-specific
AG4 proxy-timing job was inapplicable. This distributed CI is not a literal
execution of local `make ci`.

AG9's implementation host is macOS arm64. Planned V7–V12 native Linux Chromium
commands were **not freshly executed** in AG9. No native Linux executor was
available through this workspace without a new publication or provisioning.
The retained exact-implementation qualification is sufficient for the existing
local capture/comparison exit criteria because this diff changes no executable,
fixture, pin or CI behavior. It is not a claim that V7–V12 ran on the AG9 tree.
The Linux setup text is derived from that existing CI mechanism, not a fresh
installation trial. A later executable or environment change requires its own
applicable validation; macOS support remains withdrawn.

### Retained optional AWS evidence

AG4 completion remains governed by its [existing contract](../../docs/conformance/ag4-disposable-aws-chromium-runner.md#completion-status).
AG9 inspected the sealed broader-acceptance record of 2026-10-19 for head
`ea7d79bec6e521ddd7bc8e73152614e064b06987`, tree
`7c64e864123d99758fff332106abbc476c51e450`, PR #1433. Ten selected sealed files
were checked against `SHA256SUMS`, including the test executable, summary,
stdout/stderr and six per-operation disposal verifications.
`aws::acceptance::real_capture_and_interrupted_create_recovery` executed once and
passed. All six scenarios confirmed cleanup: first/repeated cascade, both original
runnable fixtures, accepted-create interruption, cancellation and accepted-delete
interruption. Expected controller exits were respectively 0, 0, 0, 86, 2 and 87;
cleanup-only recovery preserved the original operation identities.

This was historical canvas-only live acceptance, not live qualification of the
later AG5–AG8 full profile. Raw comparison reports were not retained by that test;
its exact comparison assertions and logs are the evidence. A supplementary EC2
read failed once and its read-only retry succeeded; the harness was not retried.
AG4's completed lifecycle evidence plus later local/guest integration coverage
supports the existing optional backend. AG9 neither provisions resources nor
claims fresh AWS execution, and does not reopen completed AG4 or historical
macOS incident attribution.

## Local Chromium capture (AG2)

**Supported execution: native Linux x86-64**, also AG7's authoritative final
qualification backend. macOS Chromium execution is temporarily unsupported
because ownership cleanup completeness is not proven. Runnable local capture
and comparison reject with `chromium.platform`/exit 2 before native setup,
allocation/fork or Borrowser workers; there is no automatic fallback.
Historical macOS results and test-only bundle/ownership regressions remain
evidence, not current backend support. See the
[platform decision](../../docs/conformance/ag7-focused-external-tests-and-references.md#supported-platform-decision).

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

On Linux the executable is `chrome-linux64/chrome`. Qualification requires a
fresh complete checksum-verified archive extraction; main-executable identity
alone is insufficient. The historical macOS app/framework/helper structure
validator is retained only in tests for future support restoration.
See the [supported Linux setup](../../docs/conformance/ag2-local-chromium-capture.md#supported-linux-setup)
and [historical attribution limits](../../docs/conformance/ag7-focused-external-tests-and-references.md#macos-distribution-integrity-correction).

A missing or incompatible browser is an error, including when an opt-in test is explicitly
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
platform qualification record. AG2 is integrated; historical macOS arm64 and
native Linux x86-64 production-v2 qualification remains recorded. Current macOS
Chromium execution is withdrawn. Compilation on Linux
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
accepted, optionally within an explicit AG8 `--suite ci|extended` selection.
Unknown/duplicate selections, extra options and missing required
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
its `guarded-egui-canvas` observation method and observed color for canvas cases.
Other families retain their native inspection and production-frame provenance
as described by AG5. Egui shapes,
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

`--compare` emits pretty JSON, schema `borrowser.cross-engine-phases.v3`, followed
by one newline. Member order is fixed by typed serialization; case order is
fixture-ID order; engine order is Borrowser then Chromium. Each canvas case has
one `canvas_color_rgb8` field with both observed arrays, its `equal`/`different`
status, and red/green/blue channel statuses in that order. For `canvas/cascade`,
both arrays should be `[52, 86, 120]` and every channel should be `equal`.

DOM, style and layout cases expose their common values under `projections`,
with native Chromium data and Browser frame provenance under `captures`.
Case status is `equal_for_requested_scope`, `different`, `relationship_failed` or `incomplete`; family
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

AG8 adds `borrowser_status` and `summary.borrowser_cases` from the existing
aggregate evaluator, plus selection and reproduction context. These case totals
do not replace comparison, relationship or independent-expectation facts. Fatal
comparison construction still publishes no JSON; invocation errors have separate
stderr summaries and never fabricate unexecuted case outcomes.

`borrowser_expectation` separately reports the independently authored expected
projection or RGB8, expectation metadata, optional known-failure reason and
independent status. It reuses the same successful Borrowser observation, without a
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
cargo test -p borrowser-conformance --locked --bin borrowser-conformance comparison::tests::real_chromium_comparison_composition -- --exact --ignored --test-threads=1 --nocapture
cargo test -p borrowser-conformance --locked --test comparison_cli real_chromium_combined_cli_is_stable_and_selects_exactly -- --exact --ignored --test-threads=1 --nocapture
cargo test -p borrowser-conformance --locked --test comparison_cli ag8_real_chromium_suites_are_stable -- --exact --ignored --test-threads=1 --nocapture
```

Use these explicit outer tests on supported Linux; do not blanket-enable ignored
tests or invoke private helper entries. Verify each named test actually executes.

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
# These require supported native Linux x86-64 and the pinned browser.
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
The historical combined-CLI shutdown and composition attribution gaps remain
unresolved. The owner rejected a risk waiver and instead withdrew macOS Chromium
execution pending separate platform-runtime work. The shutdown correction uses
the existing absolute capture budget for mandatory EOF validation; it does not
reconstruct missing historical causes. AG7 publication required verified support
rejection and corrective-source native Linux/distributed CI qualification, which
completed as recorded in the linked AG7 completion record. See
the linked platform decision; no new live AWS qualification is claimed.

## Historical AG4 qualification summary

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
