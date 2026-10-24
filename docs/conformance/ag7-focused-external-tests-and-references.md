# AG7 focused external tests and direct references

AG7 remains one issue within the nine-issue AG milestone. It adds a curated,
embedded WPT subset and one local WPT-style mismatch case to the AG6 catalogue.
It reuses AG5 production observations and exact comparisons. No parser, CSS,
layout, paint or browser behavior is changed to make these cases pass.

## Completion record (2026-10-23)

AG7 completed through [PR 1436](https://github.com/joris97jansen/borrowser/pull/1436),
merged as `4ebae1ebd9b18d12ecbf8a7470f7cfbe58678f1b`. Native Linux qualification
[run 39925036784, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/39925036784)
passed all nine explicit Chromium qualifications and applicable CI for integrated
tree `822c20cd0a9ad06eae4511ab5524d773f02e6c0d`, corrective head
`e9b1caaf93ff1ff78eabef0069eadb517654cc66`. The
[issue completion record](https://github.com/joris97jansen/borrowser/issues/1109#issuecomment-6361546562)
supersedes historical pending qualification/closure language below. The supported
platform decision is unchanged: native Linux x86-64; macOS Chromium remains
withdrawn. Historical observations and evidence gaps are preserved.

AG8 builds on that completed baseline. Its comparison report is
`borrowser.cross-engine-phases.v3`; historical AG7 captures/reports and their
qualification identities are not rewritten. See the
[AG8 contract](ag8-deterministic-reporting-and-suites.md) for suites and reporting.

## Supported-platform decision

Native Linux x86-64 is the supported and authoritative Chromium qualification
backend for the AG7 publication. Native macOS Chromium execution is temporarily
unsupported pending generation-safe, creation-complete process supervision.
The shared local capture boundary rejects macOS before standalone native setup,
executable access, Chromium allocation/fork or any Borrowser worker. It returns
`chromium.platform`, exit 2 and empty stdout, with no automatic Linux/AWS fallback.
Excluded-only comparison performs no capture and retains its existing report;
Borrowser-only execution and the AG4 controller/remote Linux path are unchanged.

This is a correctness boundary, not a host-inspectability condition or a waiver.
Whole-UID BSD inspection can fail on an unclassified ambient same-UID process;
controlled unrelated-candidate denial establishes that availability dependency.
Separately, the controlled Crashpad late-spawn counterexample invalidates the
final-snapshot cleanup proof: an unretained detached intermediate can spawn a
handler after the PID snapshot and exit before inspection. Retained audit-token
signalling remains generation-safe, but does not make discovery creation-complete.
Plain session/path narrowing, ambient Mach task access, private generation SPI
and public kqueue negative exclusion did not establish an acceptable correction.
The tested Crashpad switches do not disable this topology in pinned macOS CfT.
No private SPI, kqueue or replacement supervisor experiment is shipped.

The accepted absolute capture-deadline and typed CDP termination corrections
remain. The unchanged macOS owner and controlled regressions are retained as
architecture evidence. The bundle validator is test-only; investigation-only
Crashpad/minidump parsing remains removed. All historical evidence is preserved.
Correct macOS supervision belongs to the separately proposed **Platform Runtime
Hardening** work; completing that work restores support. No issue is created by
this document, and AG keeps its existing nine issues.

Pre-withdrawal source inventory
`1ab466cc50e72a9270e0917b1da9375c2e26a3cd18f47518e9cf4aa5e67cca25`
passed all nine explicitly invoked macOS browser qualifications and expanded
incremental/source-free execution against the fresh checksum-verified official
archive. Its full local `make ci` failed in `native_capture_failure_diagnostics`,
`capture-send-timeout`, before the intended assertion, with BSD inspection EPERM
for unclassified PID 67513. That PID's ownership is unknown. These results remain
separate; the successful runs do not prove cleanup completeness or qualify the
support-withdrawal source. Historical A/B, EPERM and incomplete-bundle crash
attribution gaps are not reconstructed or waived by withdrawing execution.

The existing Ubuntu lane must qualify the complete corrective publication tree,
including native ownership regressions, ordinary conformance, all nine explicit
pinned-browser qualifications and applicable workspace CI. Its Rust job also
compiles CSS benches via the existing `make compile-css-benches` target. Together
with the existing jobs this preserves `make ci` component coverage; distributed
CI is not a literal execution of that command. Earlier Linux results below
qualify only their recorded trees. Mac-only tests, separate source-free evidence,
and unexecuted native EICE/OpenSSH qualifications retain their platform limits.
No new live AWS qualification is required or claimed. Corrective-source native
Linux CI and AG7 completion are recorded above; historical records below retain
their original source identities and limitations.

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

This dated record preserves the earlier results and incident merge gates. The
[supported-platform decision](#supported-platform-decision) supersedes the
requirement to restore macOS execution within AG7: support is withdrawn, not
waived. Its restoration belongs to separate platform-runtime work; AG7 still
requires corrective-source Linux qualification and owner merge authorization.

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

- **Combined-CLI shutdown incident: unresolved; merge blocker.**
  `real_chromium_combined_cli_is_stable_and_selects_exactly` exited 101; the
  harness exited 2 instead of 1 with `Shutdown deadline expired`. The retained
  record has no cleanup diagnostic line. The binding deadline, cause and verified
  historical cleanup outcome remain unknown.
- **Composition incident: unresolved; merge blocker.**
  `comparison::tests::real_chromium_comparison_composition` and its isolated child
  exited 101 in `borrowser-failure`, after the second capture started and before
  expected cleanup/worker events. Historical harness exit and primary/cleanup
  details were omitted; the operation label could be stale. The cause and final
  second-capture cleanup outcome remain unknown.

Missing historical evidence establishes neither successful cleanup nor a proven
leak. Later macOS/Linux passes and completed CI do not resolve either incident.
The owner explicitly rejected waiving either incident. Each requires an
evidence-based technical resolution before merge; later passes are not a waiver.
Recording completed qualification does not authorize readiness, merge or
issue/milestone closure.

### Shutdown deadline correction

The uncommitted continuation starts from published documentation commit
`9d137cafe36ac0d0376a9a2d0815598eb2e6a975`. Earlier qualification remains bound to
its recorded source; it does not qualify this correction. Complete earlier
investigation sources, failed runs and diagnostic transcripts remain preserved
separately. The original checkout and index are unchanged.

Three fresh native macOS failures established acknowledged close with no EOF at
the 250 ms cutoff despite substantial capture budget remaining. They occurred
in composition `multiple`, whole-catalogue scoped relationships, and the expanded
combined CLI. Subsequent owned termination, root reaping and profile removal
were verified, with no prematurely started Borrowser workers. The failed
incremental invocation did not reach copied-executable execution. None of those
facts recovers the missing primary/cleanup evidence of historical incident B.

History identifies the policy mismatch: `db2c8d0` used 250 ms for an explicitly
best-effort close call, ignored its result, and relied on process cleanup.
`93eb3c3` correctly made validated response EOF mandatory but carried that short
allowance into the evidence-validation path. A controlled native peer exposes
the resulting rejection of valid completion: it acknowledges close, keeps the
command direction open, and emits valid completion after the old cutoff but
before the supplied absolute capture deadline. The old policy fails this test;
the corrected owner accepts it. This is a controlled counterexample, not a
reconstruction of a historical Chromium run.

A later native expanded-corpus execution of the preserved observer supplied the
missing discriminating observation: after the original timeout at 250.889 ms,
the same stream reached validated EOF at 336.668 ms, within its original
4.987651083-second remaining capture budget, without command-writer closure,
another close request or a process signal. Subsequent cleanup verified owned
termination, root reaping and artifact removal. The original failure remained
exit 2. This directly establishes naturally delayed valid completion for that
occurrence; it does not reconstruct the earlier incidents. The run accidentally
used a cached integration executable whose embedded source path pointed to the
observer checkout, so it is diagnostic evidence, not corrective-source
qualification. Qualification must use artifacts bound to the publication
checkout; the failed attempt and its source-path audit are retained.

`Cdp::close_browser` now uses the unchanged absolute capture deadline for delivery
and all incoming validation. It does not restart the clock on acknowledgement,
events or progress. EOF, framing, resource/document policy and cancellation checks
remain mandatory. The command writer stays open until the existing subsequent
cleanup; neither forced termination, root exit nor acknowledgement establishes
capture success. Cleanup still independently verifies ownership, termination,
reaping and artifact removal under its separate five-second bound. See the
[AG2 lifecycle policy](ag2-local-chromium-capture.md).

A bounded native macOS probe established that anonymous pipe endpoints are FIFOs:
`shutdown(SHUT_RDWR)` returns `ENOTSOCK` and does not release a blocked reader;
closing the last writer does. Pinned Chromium revision
`3ff7ac5a9224be9156d7f8703a06e22890aafd34` uses that operation in its pipe handlers,
but schedules thread destruction/join as `CONTINUE_ON_SHUTDOWN`. Such running
tasks are not joined by thread-pool shutdown. Reader/writer descriptors are raw
integers, and process exit releases remaining descriptors. Thus this syscall
result does not establish a mandatory browser-root exit dependency on command
closure. A controlled second-capture read pause with the pinned browser also observed
root exit and response-pipe hangup before any command closure or cleanup. It
therefore disproves command closure as a universal prerequisite for browser-root
exit; hangup alone is still not validated EOF. No transport replacement or earlier
command closure is justified or implemented. The exact cause of each historical
delay remains unknown.

Native regressions cover completion beyond the former cutoff, missing completion,
late malformed/truncated/policy-invalid evidence, cancellation after acknowledgement
and real absolute-budget expiry. They verify owned termination, root reaping,
profile removal and unrelated-process survival. A second-capture pause past the
former cutoff fails under the old policy and succeeds under the shared capture
budget, without changing incoming bytes. Composition retains the original
`borrowser-failure` case and exercises cancellation after the SECOND capture's
close acknowledgement, actual absolute-budget expiry during the SECOND capture,
and cleanup failure after the SECOND capture. The expiry scenario consumes the
original deadline after acknowledgement without cancelling, closing either
protocol direction or changing the deadline. It requires `Timeout(Shutdown)`,
verified owned termination/reaping/profile removal, no result JSON and no worker
startup. Its delayed
variant of `borrowser-failure` must reach the intended overlap error only after
both captures/cleanups and worker startup. No Chromium capture/cleanup failure
may start Borrowser workers or publish a completed comparison.

The incident-B assessment distinguishes these current invariants from unavailable
historical facts. The original `borrowser-failure` scenario arms neither local
cancellation nor a fault. Each composition scenario executes in a fresh process;
standalone entry clears the signal flag and creates a fresh cancellation token.
Local cancellation is test-only and requires an explicitly armed scenario.
Production cancellation can follow received SIGINT/SIGTERM; SIGPIPE instead
becomes an I/O error. The original scenario does not itself generate cancellation,
and no retained evidence establishes that it received a signal. At the recorded
native `Budget` check, the possible primary failures were cancellation or expiry
of the caller's deadline. The stale operation label and omitted primary/cleanup
details cannot distinguish them. Current failures retain the actual phase,
primary error and separate cleanup outcome. Every required-capture failure
aborts the batch before worker startup, including after an earlier successful
capture. Cleanup has its own finite, cancellation-independent budget; a cleanup
failure also prevents a completed comparison. These are verified current
failure-handling requirements, not a reconstruction of incident B.

The subsequent macOS qualification completed all 13 composition scenarios and
the expanded incremental/source-free execution, including the copied executable.
Eight of the nine named pinned-browser qualifications passed. The independent
CLI qualification instead exited 101 after an ownership scan reported incomplete
BSD process information (`rc=0`, reported `EPERM`) during the
`wpt/rgb-alias-p6` reference capture, before shutdown. Its retained diagnostics
verify owned termination, root reaping and artifact removal, with no cleanup
errors or Borrowser workers. The candidate process's state and the reason for
the unavailable metadata remain unknown. This is a distinct current
qualification blocker, not evidence of another short-drain timeout or an
explanation of historical incident B. The failed invocation was preserved and
not retried; successful other qualifications do not supersede it.

A subsequent bounded native ownership probe exercised the actual registry with
pipe-synchronized live, waitable-exit and acknowledged-exec helpers. Live BSD
inspection returned the complete 136-byte structure; independently confirmed
exit returned zero with immediate and post-budget errno both ESRCH. Exec retained
the PID but changed the audit generation: the old token could not signal the new
generation, which required fresh discovery. Immediate and post-budget errno
agreed in these native observations. Native EPERM reproduction was unavailable;
explicitly synthetic EPERM checks rejected both unadmitted and retained
candidates, preserved retained identities, left scans incomplete and prohibited
termination success. Each case verified owned termination, direct-child reaping,
artifact removal and unrelated-sibling survival. A failed initial probe setup
assumption (the launcher had already populated its registry) remains recorded;
it was corrected in the test, without changing ownership policy.

These checks support the existing fail-closed ownership rule; they do not
reconstruct PID 62390's historical state or identify its permission failure.
Failure diagnostics now retain immediate libproc errno separately from errno
after the ownership-budget check and record whether that candidate was already
retained. Classification remains unchanged. A synthetic diagnostic control
checks that distinct immediate/post-check errors remain distinct. No permission
failure is converted into exit, ignored or retried to obtain qualification.

The owner accepted the ownership investigation and retained the fail-closed
policy. The historical PID 62390 inspection failure remains unexplained; it does
not justify a speculative ownership-policy change.

### Startup termination-event classification

The next independent-CLI invocation failed during startup `Page.enable`, with
`BrowserExited`, a complete response frame still buffered, and one pending
command. No native root exit was observed in the failure snapshot. Subsequent
owned termination, root reaping and artifact removal were verified, with no
cleanup errors or Borrowser workers. The exact event and the retained 432 bytes
of Chromium stderr were not published in that historical record and remain
unavailable. This occurrence is therefore unattributed; it is not evidence of
another shutdown timeout or ownership-inspection failure.

Investigation then demonstrated a deterministic dispatch defect through the
actual decoder: malformed `Target.targetCrashed` parameters, a crash naming
another target, and an Inspector crash on another session all became
`BrowserExited`. The preserved eight-case native regression failed with those
three classification violations after verified cleanup. This controlled result
does not identify the historical startup event.

The correction validates the pinned
[Inspector definitions](https://chromium.googlesource.com/chromium/src/+/3ff7ac5a9224be9156d7f8703a06e22890aafd34/third_party/blink/public/devtools_protocol/domains/Inspector.pdl)
and [Target definitions](https://chromium.googlesource.com/chromium/src/+/3ff7ac5a9224be9156d7f8703a06e22890aafd34/third_party/blink/public/devtools_protocol/domains/Target.pdl)
before capture identity checks. Detach requires a string reason; Inspector crash
has no parameter fields; Target crash requires string targetId/status and integer
errorCode. Flattened sessionId belongs to the transport envelope. Valid foreign
notifications continue through existing non-resource routing to the pending
response. Malformed notifications remain protocol failures even when foreign.
Only a validated captured-session detach is tolerated during shutdown, without
establishing completion or changing the absolute deadline.

Bounded failure-only diagnostics now distinguish native root termination and
each termination method, with session/target correspondence and relevant scalar
reason/status/code. The capture snapshot is retained before cleanup and emitted
afterward, along with bounded escaped Chromium stderr. No arbitrary event payload
or successful result is changed. The controlled regression also exercises an
actual comparison startup failure, verifying cleanup, error exit 2, no completed
report and no worker startup. Qualification results for this correction must be
attributed to its recorded source inventory; earlier successful qualifications do
not qualify the new dispatch. Final corrective-source Linux qualification remains
outstanding.

Focused macOS 27.0.1 ARM64 validation of source inventory
`60782f1eb44a53c35c24f905008170ce8de0c1800395eb5ba9169b3b393dc6e7`
passed five method-specific/diagnostic tests and one explicit invocation of
`native_startup_exit_event_classification`. All eight original cases passed,
including successful pending responses after foreign events. Its separate native
root-exit and comparison-failure helper scenarios verified diagnostic source,
owned termination, root reaping, profile removal and unrelated-sibling survival.
The comparison failure retained exit 2 and no worker startup.

The subsequent, single explicitly invoked
`real_chromium_cli_has_stable_independent_observations` failed (test exit 101,
first internal CLI iteration exit 2, empty stdout). For `canvas/cascade`, role
Test, `Page.enable` command 5 received `Inspector.targetCrashed` with a matching
capture session. The snapshot retained one complete buffered frame and one
pending command; native check was Complete, root was NoExitObserved and response
pipe OpenEmpty. Chromium stderr retained 432 bytes: network service crashed or
was terminated; GPU process exited unexpectedly with code 5; connection
terminated while reading from the pipe. These messages do not establish which
condition caused the target crash, or whether the pipe message preceded cleanup.
Cleanup verified owned termination, root reaping and artifact removal, with an
empty error vector and no Borrowser workers. This is a correctly classified
current-target crash, not the malformed/foreign-event regression. Its cause is
unresolved and AG7 remains blocked. The historical startup event is still
unavailable and cannot be retrospectively attributed to either condition.

No retry, broader subsystem run, all-nine qualification, full CI or Linux
qualification followed this failed invocation. The result is bound to the source
inventory above; this subsequent documentation update does not imply another
execution. The shutdown-policy correction, incomplete historical composition
record and historical ownership EPERM retain their separate evidence/status.

Pinned-source investigation of the captured-session crash found two relevant
page-target producers: `Inspector.enable` can synchronously report a non-live
frame, and the render-frame agent host reports renderer termination/launch
failure. This startup sends no `Inspector.enable`. Its first four completed
commands are browser-side version/create/discover/attach operations; the attach
response does not prove that a renderer handled a command. `Page.enable` is
command 5 and the first renderer-backed operation. Its being pending does not
establish that it caused the crash. Chromium's renderer-exit callback emits the
Inspector notification before the browser `Target.targetCrashed` status/code.

A failure-only diagnostic now records completed startup commands and inspects
at most four complete already-buffered frames within 4096 bytes for that
companion notification. It uses the existing method/identity validation, emits
only bounded status/code/correspondence, and labels the evidence not dispatched.
It performs no further read, changes no pending state or verdict, and cannot
establish stream completion. Snapshot/output retain their before/after-cleanup
boundary. Controlled tests cover a modeled non-live target at Inspector enable,
crash before/after a Page response, normal startup, foreign-session crash,
bounded/malformed input and unchanged buffered/pending state. These tests model
protocol order; they do not reproduce the native renderer's cause.

Source inventory
`950948e8ef45cb816f395853c7316fd93d19e765c2782592b8ede0119be445f4`
passed the new diagnostic unit test and one explicit native classification test
(all eight classifier cases, five lifecycle cases, root-exit and comparison
cleanup checks). One subsequent invocation of the same independent CLI
qualification failed on macOS 27.0.1 ARM64: test exit 101, internal iteration 0
exit 2, `canvas/cascade` test input, Startup `Page.enable` command 5. The captured
session's Inspector crash was followed in the already-buffered input by a valid
`Target.targetCrashed` naming the captured target, status `crashed`, error code 5.
No renderer-backed command had completed. Root state remained NoExitObserved;
the complete ownership scan retained 16 identities without a fresh descendant
liveness sample. Cleanup verified owned termination, root reaping and artifact
removal with an empty error vector and no Borrowser worker startup. The retained
432 stderr bytes again contained GPU exit code 5, network-service restart and
pipe-read termination. Their presence does not establish a causal connection;
stderr includes cleanup time. No further native or broader qualification ran.

The captured target's reported termination status is now known; its initiating
cause and pre-command renderer liveness remain unknown. No Borrowser startup
defect is demonstrated. At that checkpoint the proposed next evidence was an attributable native
renderer crash record. The later distribution-integrity finding below changes
the investigation boundary; it does not reconstruct the crash cause. Earlier startup,
shutdown/composition and ownership records are not retrospectively explained.
This documentation addition was made after the recorded diagnostic execution;
it does not qualify a later tree or replace outstanding corrective-source Linux
qualification.

The subsequent pinned crash-reporting audit established that macOS Crashpad is
initialized under the current launch without an additional switch. Its database
uses finalized UUID.dmp files and filesystem extended attributes, not a required
JSON sidecar. Process annotations can identify renderer reports; exception
metadata can supply Mach exception information. This supplies no audit-token
generation or captured-target-to-native-PID mapping. `Target.targetCrashed` takes
its code from `WebContents::GetCrashedErrorCode`, populated through render-view
termination from `ChildProcessTerminationInfo.exit_code`. The ordinary macOS path
retains raw wait status; Chromium can override it with a requested shutdown code.
Raw wait status 5 would encode SIGTRAP, but the protocol number alone does not
prove an unoverridden wait status or a specific exception/cause. Matching GPU
and renderer numbers do not establish shared causation.

A bounded failure-only private Crashpad snapshot was temporarily added during
this investigation. It is removed by the distribution-integrity correction below;
the following record describes the historical diagnostic source.
Source inventory
`fc4a195001813c32668ab9773abc9e9c159846bf475d5bdf72d9669bdf836776`
passed three metadata tests (bounds, escape/malformed rejection, attribution
and artifact removal) and one explicit native startup-classification invocation.
Its controlled comparison failure verified snapshot publication after cleanup,
normal removal despite a malformed report, error exit 2 and no worker startup.

Before the single permitted real diagnostic invocation, inspection found no
`Info.plist` files anywhere in the configured Chrome app, including its main
bundle. Its executable still matched SHA-256
`316795971c05d253320fcc574168aa8ae7782eae9beb540e0595ebbd654ab965`.
The bundle inventory retained 55 entries (17 regular files and five symlinks).
That is a concrete browser-installation integrity limitation; its origin/time
and causal relationship to the crash were not established. The bundle was not
repaired or changed. Executable identity alone does not establish integrity of
the complete pinned distribution.

The one real invocation failed again on macOS 27.0.1 ARM64 (test exit 101,
internal iteration 0 exit 2, `canvas/cascade`, Startup `Page.enable` command 5).
It retained the captured-session Inspector crash and matching buffered Target
crash with status `crashed`, code 5. Seven finalized reports were observed in
the private `pending` directory before cleanup; all failed the snapshot's
process-annotation/PID correspondence check. That diagnostic combined
absent and conflicting annotation data under `process annotation/PID mismatch`;
it did not retain enough data to distinguish these cases or identify a renderer
exception. It must not be interpreted as proof of conflicting native PIDs.
Only 1,960 metadata bytes and 63 reads were used. Cleanup verified owned
termination, root reaping and artifact removal, with empty cleanup errors and
no Borrowser worker start. The 432 retained stderr bytes again reported GPU exit
5, network-service restart and pipe-read termination. No dump binary was retained.

No further native run, broader qualification or bundle repair followed. This
failed execution is diagnostic evidence for the recorded source and observed
incomplete bundle, not qualification of an intact pinned distribution. The
snapshot proves local artifacts are produced without new flags, but does not
yet provide renderer-attributable exception evidence. At that checkpoint AG7 remained blocked on browser-installation integrity
and crash attribution; no lifecycle policy was weakened. Earlier incidents remain separate. These documentation additions
postdate the execution and do not qualify a later tree.

The owner's prohibition on waiving either incident remains in force. This
correction addresses the demonstrated deadline-policy defect; it does not recover
missing historical traces or automatically attribute incident B to that defect.
Final-source native Linux qualification and review of the technical evidence are
required before merge. No readiness, merge, closure or live AWS work is implied.

## macOS distribution integrity correction

The official 155.0.8059.39 mac-arm64 archive was downloaded separately and its
SHA-256 verified **before extraction**:
`529a71bd61aaa2ef266a4d4bd300ae9572ba6a3468a8d55c3023ffeffb5b6b4e`.
All 673 archive entries matched the fresh extraction's contents/types/modes and
symlink targets. The previously configured app lacked 323 files and 290
directories, including all ten Info.plist files, ordinary/renderer/GPU helper
executables, framework metadata and resources. All surviving files matched;
the main executable digest alone had therefore failed to establish distribution
integrity. The suspect installation and every failed-run record were preserved.
No specific missing file is established as the cause of any earlier crash.

One explicitly permitted independent-CLI qualification passed against the fresh
complete extraction on native macOS 27.0.1 ARM64: one Rust test (three built-in
CLI executions), exit 0, no ignored test counted as an execution. Its working
source inventory was
`ca1942b30453a9abc11b9bc573341b19570d1730bf91e52cc27520c7c11a119f`;
its executable SHA-256 was
`2484ed775ebadd3bbb5cf7ca466fd82b0b5cf88315d561e840c7f3dfdf7574ce`.
That pass predates the bundle preflight and does not qualify the later correction.

The corrected boundary combines archive-authoritative qualification setup with
[bounded macOS runtime preflight](ag2-local-chromium-capture.md#macos-distribution-integrity)
before private-directory allocation/fork. Its deterministic negative tests cover
missing/malformed/contradictory plists, missing helpers/executables, wrong entry
types, oversized metadata and symlink escapes. Test-owned minimal bundles retain
the actual framework alias shapes without pretending to contain the full browser.
Normal-CLI and isolated comparison tests assert preflight errors, no child/private
owner allocation and no Borrowser workers. The affected subsystem check also
exposed two stale delayed-shutdown helper events lacking the pinned protocol's
required `Inspector.detached.reason`. Their test-owned payloads now include that
field; the accepted production classifier, delays, EOF validation and timeout
assertions are unchanged. The failed subsystem execution and verified cleanup
remain in the correction evidence. Renderer/GPU/ordinary helpers and their
Aperitif variants are required; arbitrary resource files are not hashed at launch.

Local CI exposed two test-orchestration assumptions during this correction.
The preflight test now inspects a private temporary parent rather than the shared
directory used by parallel tests. The existing ownership probe incorrectly
expected a waitable root exit to imply a complete whole-user discovery scan.
It now uses the existing one-shot unavailable-arguments fault to prove that even
with all retained identities dead, an incomplete scan forbids termination success;
bounded cleanup must subsequently verify a complete scan, reaping and removal.
The failed execution did not retain which candidate made its scan incomplete;
the controlled counterexample proves the assertion defect, not that historical
trigger. Production ownership policy is unchanged. Both failures and the
counterexample remain recorded separately from final-source qualifications.

The investigation-only `crash_evidence.rs` minidump parser, snapshot wiring and
synthetic-dump tests were removed from the deliverable. They had no capture,
comparison, identity or cleanup authority and no ongoing AG7 responsibility.
Their retained external evidence is unchanged. The seven historical report
summaries conflated missing and contradictory PID annotations; their raw dumps
were removed by verified cleanup, so this distinction remains unavailable.
Generic typed CDP termination diagnostics, bounded retained Chromium stderr and
native process/cleanup evidence remain useful and are retained.

Incident boundaries remain distinct:

- The demonstrated shutdown-policy defect is corrected by using the original
  absolute capture deadline for mandatory EOF validation. No deadline is reset.
- The demonstrated CDP classification defect is corrected by method-specific
  validation and captured-session/target correspondence.
- Controlled live/exited/exec/denial ownership tests support the unchanged
  fail-closed ownership policy. The historical PID 62390 EPERM remains unexplained.
- Recent renderer/Crashpad executions used the structurally incomplete local
  installation. They remain diagnostic evidence, not qualification of the complete
  official distribution. The fresh-browser pass does not prove historical cause.
- Earlier successful macOS qualifications and historical A/B/EPERM records used
  the same recorded installation path where retained. Their contemporaneous full
  bundle state is unknown; neither incompleteness nor completeness can be inferred
  retrospectively from a path or main-executable hash. Their original results and
  attribution limitations remain recorded, without a waiver.

At the pre-withdrawal checkpoint the correction required focused, nine named
macOS browser, expanded incremental/source-free and full-local-CI evidence. The
[supported-platform decision](#supported-platform-decision) records the actual
outcomes and supersedes further macOS execution requirements. Earlier Linux qualification remains attributed to
its original source and separately verified linux64 archive; it does not qualify
this corrective source. Native Linux qualification of the final published
correction remains required before merge. No live AWS, readiness, merge, closure
or owner risk acceptance is implied.

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
The [completion record](#completion-record-2026-10-23) is authoritative for AG7's
completed status and native Linux qualification. The dated
[qualification and incident records](#qualification-record-2026-10-21) preserve
historical observations and evidence limitations; they do not describe AG7's
current completion state or outstanding closure requirements.

Multiple references, chains/cycles, fuzzy/print/readiness support, external
resources/fonts, page JS, testharness.js execution, WebDriver, screenshots and
broader selectors remain excluded. There is no generic importer, document graph,
provider interface, cache or metadata-version framework.
