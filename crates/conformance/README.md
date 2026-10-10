# Focused static conformance harness (AG1–AG2)

Run the explicit fixture set:

```sh
cargo run -p borrowser-conformance --locked
make conformance
cargo run -p borrowser-conformance --locked -- canvas/cascade
```

The only observation is opaque sRGB RGB8 canvas color at viewport pixel `(32, 32)`:
CSS pixel center `(32.5, 32.5)`. This establishes a small production execution and
exact comparison boundary. It is not general browser or rendering conformance.

## Production execution

Each case constructs a fresh `Tab`, calls `navigate_to_new`, and fulfils the
emitted document fetch with fixed `NetworkStart/Chunk/Done` events. Browser's
resulting parser commands run in the normal `runtime_parse` worker. Its genuine
mode-bearing publications go through `Tab::on_core_event`: Browser validates and
commits DOM, mode, and mutation facts, reconciles authored stylesheets, and queues
render work. The harness never constructs or accesses private document/style or
retained-rendering state.

After the parser terminal event and successful Browser completion validation,
`Tab::ui_content` performs the normal CSS preparation, Layout, GFX Paint, and
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

Capture inspects only the current `egui::FullOutput`. It requires exactly one
opaque, untextured, square-cornered, unstroked, unblurred rectangle covering the
viewport, whose clip also covers the viewport. No other effective paint may
intersect the guard rectangle `(30, 30)`–`(35, 35)`. The collector uses existing
epaint visual bounds and clips, after egui layer transforms, to exclude unrelated
shapes. It does not rasterize, blend, infer DOM identity, or receive an expected
color. Missing or ambiguous output, intersecting unsupported shapes, custom
callbacks, nonfinite geometry, and a different scale are capture errors.
A wrong opaque canvas color remains an observation and produces a mismatch.

The fixtures have zero-height empty content, transparent bodies, zero margins,
padding and borders, and no text, images, external resources, scripts, animation,
or transforms. UI decorations are acceptable only outside the sample guard.
Fresh egui passes drain their own shapes; neither a second frame nor channel
silence is used as a readiness heuristic.

The opt-in AG2 Chromium path independently samples that pixel from an sRGB
screenshot with the same viewport and device scale. Neither path makes a
`getBoundingClientRect()` equivalence claim.

## Fixtures and independent expectations

`src/fixtures.rs` registers the small fixture set in report order. HTML bytes are
embedded, so execution does not depend on the current directory.

- `canvas/root`: `html { background-color: #123456 }` yields RGB `(18, 52, 86)`.
- `canvas/cascade`: the `#canvas` rule sets `#345678`; a later `html` selector has
  lower specificity, so the expected RGB is `(52, 86, 120)`.

These expectations come from authored CSS and the
[CSS root/canvas background rule](https://www.w3.org/TR/css-backgrounds-3/#root-background),
not generated Borrowser output. Current production background selection prefers
a nontransparent body color; AG1 deliberately tests transparent bodies and does
not claim the full root/body propagation behavior is conformant.

To add a case, author a static fixture within these restrictions, register a
unique `TestId`, state the semantic reason for its exact expected color, and run
the targeted tests and harness. There is no discovery, automatic unsupported
feature detector, or expectation blessing command. A fixture outside the
supported observation must be explicitly classified or await a separately
reviewed extension; do not weaken capture to obtain a comparison.

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
Errors never satisfy known-failure metadata. The current two fixtures are runnable
and expected to match; classification regressions exercise the other outcomes
without adding artificial skipped or failing cases to the shipped registry.

Reports include relative fixture paths, fixed sample location, expected/actual
hex colors, differing channel values, and fixed-order summary counts. They omit
time, internal identities, absolute paths and orchestration traces. Exit 0 means
no FAIL/XPASS/ERROR; exit 1 means FAIL or XPASS; exit 2 means an execution,
configuration or output error and takes precedence. Unsupported/skipped cases
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
cargo test -p borrowser-conformance --locked real_chromium_ -- --ignored --nocapture --test-threads=1
```

On macOS, the executable is `Google Chrome for Testing.app/Contents/MacOS/Google
Chrome for Testing`; on Linux it is `chrome-linux64/chrome`. A missing or
incompatible browser is an error, including when an opt-in test is explicitly
requested. Ordinary AG1 execution and tests require no installed browser.

`--chromium` emits capture-only JSON: fixture identity, observed RGB8 color,
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
platform qualification record. Linux x86-64 qualification is still outstanding;
Linux compilation on ARM64 does not establish that acceptance criterion.

No integrated cross-engine comparison (AG3), AWS (AG4), WPT import, generalized
adapters/providers, broad fixture discovery, historical storage, dashboards,
flaky-test policy, or new engine semantics are included.
