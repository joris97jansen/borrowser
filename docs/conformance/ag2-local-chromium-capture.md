# AG2 — local Chromium canvas capture

AG2 belongs entirely to `crates/conformance`. It independently renders the AG1
fixture bytes and returns `CanvasColor` plus Chromium provenance. It does not
execute Borrowser, compare engines, change expectations, or alter production
Browser/HTML/CSS/Layout/Paint behavior. AG3 owns integrated comparison.

## Reference and invocation

Use the full **Chrome for Testing 155.0.8059.39** distribution, revision
`@3ff7ac5a9224be9156d7f8703a06e22890aafd34`, CDP protocol `1.3`.
[`chromium-reference.json`](../../crates/conformance/chromium-reference.json)
records versioned archive URLs and SHA-256 hashes for mac-arm64 and linux64.
The current capture profile is **`ag2-canvas-srgb-v2`**: one ordinary headless
page and the parsed-document resource policy below. The browser pin, archive
checksums, fixture expectations and `borrowser.chromium-canvas.v1` JSON schema
are unchanged. Earlier hidden-target and A/B evidence retains its historical v1
identity; it is not retroactively v2 qualification.
Download and verify the selected archive before extracting it to a trusted local
directory. `--chromium-executable PATH` takes precedence over
`BORROWSER_CHROMIUM_EXECUTABLE`. There is no implicit executable discovery,
download, version fallback or use of an existing profile.

The harness checks `Browser.getVersion` through the actual pipe connection. A
different product/version, revision or protocol is an execution error. This
check is provenance validation for a trusted, checksum-verified installation;
it is not executable attestation against a malicious program impersonating CDP.
Pin changes require both fixture and native lifecycle requalification on each
intended OS. Other Chrome builds and `chrome-headless-shell` are not qualified.

```sh
cargo run -p borrowser-conformance --locked -- --chromium --chromium-executable '/absolute/path/to/chrome'
cargo run -p borrowser-conformance --locked -- --chromium canvas/cascade
```

The second form requires the environment variable. Neither needs manual browser
interaction. Explicit capture without a usable browser fails with exit 2 and
empty stdout. Default invocation retains AG1's execution/report behavior.

## Fixture and observation

| Input | Contract |
| --- | --- |
| Bytes | Unmodified embedded AG1 fixture bytes |
| URL/origin | `https://borrowser.invalid/ag1/fixture.html` / `https://borrowser.invalid` |
| Response | HTTP 200, `Content-Type: text/html; charset=utf-8` |
| Document | Standards doctype; Chromium must report `NoQuirksMode` |
| Viewport | 640 × 480 CSS pixels, device scale 1, page scale 1, scroll (0, 0) |
| Observation | Physical screenshot pixel (32, 32), CSS center (32.5, 32.5) |
| Color | Opaque sRGB RGB8, copied into the existing `CanvasColor` |

The screenshot is the sole color source. Read-only DOM inspection checks the
document identity/URL/mode and absence of live-document `link` elements for resource
eligibility; it is not a DOM observation inventory. Layout metrics check only the viewport. CSS declarations, computed
styles and DOM geometry are not alternative observations. No JavaScript is
evaluated by the harness. A wrong color remains an observation.

AG1 feeds its response in parser chunks; CDP fulfills one complete response.
These fixtures have no timing-dependent content. Chromium uses its real raster
pipeline and clock, whereas AG1 observes the production egui canvas output with
fixed time. No equivalence beyond this static pixel is claimed.

## Pipes and launch ownership

Each launch owns one private `TempDir` beneath the parent process's temporary
directory. Its canonical root contains three separate locations: `profile`
for `--user-data-dir`, `crashes` for `BREAKPAD_DUMP_LOCATION`, and `tmp` for
Chromium-created temporary files, including POSIX singleton sockets. Before
fork, the launcher builds the complete child environment, removes inherited
`TMPDIR` and `BREAKPAD_DUMP_LOCATION` entries, and installs exactly one of each
with these private paths. Chromium and its descendants inherit them. The Rust
parent's environment is not changed; Cargo/build scripts and test/CLI target
runners keep their existing build/runtime separation.

On Linux, the private root uses the short `ag2-` prefix instead of
`borrowser-chromium-`. The pinned
[ProcessSingleton implementation](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/chrome/browser/process_singleton_posix.cc)
creates `<TMPDIR>/org.chromium.Chromium.XXXXXX/SingletonSocket`: six generated
ASCII suffix bytes and 45 total pathname bytes after TMPDIR. Its Linux socket
address must include a NUL within the native 108-byte `sockaddr_un.sun_path`.
The launcher checks the canonical absolute path's **byte** length before fork:
the complete socket pathname must be at most 107 bytes, so child TMPDIR can be
at most 62 bytes. Relative/NUL-containing or excessive paths produce an explicit
configuration error; no shared-directory fallback is attempted. The runner's
`/home/runner/work/_temp/ag2-profiles/ag2-XXXXXX/tmp` produces a 96-byte socket
pathname. Developer paths are checked independently, including multibyte names
and symlink-expanded canonical parents. The fixed suffix is tied to the pinned
browser and must be revisited when that pin changes.

Before fork, a local `Option<TempDir>` owns the root independently of fallible
initialization. A returned initialization error explicitly closes that directory;
successful removal leaves `Failure.cleanup` empty, while removal failure adds
`CleanupError::Artifacts` containing the private path. The original error remains
`Failure.primary` in either case. Removal is not atomic: an error can leave a
partially removed subtree, which must not be reported as removed.

Immediately after successful fork in the parent, the directory is transferred
exactly once into `OwnedChromium`. A failed fork leaves it with the pre-fork owner;
the exec child never runs this Rust cleanup path. After transfer, only the process
owner handles partial-launch failure and subsequent cleanup.

The process owner recursively removes the entire subtree only after existing native
identity, termination, direct-child/adopted-child reaping and deadline checks
succeed. Incomplete verification retains the tree and reports cleanup failure;
filesystem removal errors remain explicit. No name-based exception, sibling
scan/removal or pre-verification deletion is used.

The private Unix launcher uses `fork`/`execve`, not a shell or a reusable
transport framework. Chromium's `--remote-debugging-pipe` contract is:

| Pipe | Parent | Child after exec |
| --- | --- | --- |
| Commands | write | FD 3, read |
| Responses/events | read | FD 4, write |
| Exec status | read | separate close-on-exec writer |
| Diagnostics | bounded stderr reader | FD 2, write |

All pipe sources are duplicated to descriptors ≥5 with `F_DUPFD_CLOEXEC` before
fork, avoiding collisions with FD 3/4 and closed standard streams. `dup2` installs
FD 3/4 without close-on-exec. The parent drops child ends immediately. The child
closes every enumerated ambient descriptor above 4 except the exec-status writer.
FD 0/1 use `/dev/null`; stderr cannot corrupt CDP.

Before fork, the parent command writer, response reader, stderr reader and
exec-status reader are all made nonblocking. Opposite pipe ends have distinct
open-file descriptions: the child's FD 3/4 and exec-status writer remain blocking.
A descriptor-configuration failure therefore occurs before any child exists.
The owner is still installed immediately after successful fork; mask-restoration,
launch-status and cancellation failures, including unwind/Drop cleanup, cannot
reach a blocking parent stderr read.

Before fork, Rust allocates argv/envp, descriptor lists and ownership data. The
child branch only uses async-signal-safe `setsid`, `dup2`, `close`, signal-mask/
disposition operations, `execve`, `write` and `_exit`. It never allocates, logs,
unwinds or runs Rust destructors. Exec errors use a fixed atomic stage/errno
record. Interrupted writes are retried; truncated status is a launch error.
INT/TERM are blocked across fork until the parent installs its owner. Any
fallible operation after fork retains that owner and runs cleanup on error.

The standalone path has no concurrent launcher, unrelated child or competing
reaper. Default SIGCHLD disposition is required to retain a waitable root.
SIGINT/TERM set a cancellation atomic; SIGPIPE becomes an I/O error. Parent pipe
ends are nonblocking. Partial reads/writes and EINTR retain their deadline;
polling checks cancellation at most every 20 ms between native operations.
NUL-delimited JSON has monotonic command IDs, matching session IDs, a 4 MiB frame
limit, and explicit EOF, truncated-frame, malformed-response and remote errors.

## CDP ordering and readiness

1. Verify browser identity. Create exactly one ordinary `about:blank` target
   with `Target.createTarget({"url":"about:blank"})`, discover page/frame targets,
   and attach with a flattened session. There is no bootstrap or hidden fallback.
2. Enable Page lifecycle and Network events. Disable cache; bypass service
   workers. Disable page script execution **before navigation**, then set viewport,
   device scale and page scale. Enable Fetch interception for all request types
   at Request stage in this target session.
3. Obtain the main frame ID, initialize navigation state, and navigate to the
   fixed fixture URL. Fulfill exactly one matching main-frame Document GET.
4. Correlate Fetch `networkId`, Network request ID/loader ID, `Page.navigate`
   frame/loader acknowledgment, committed frame URL/origin/loader, HTTP 200 HTML
   response and the **same loader's** load event. An unrelated load, navigation
   acknowledgment alone or silence never establishes readiness.
5. Obtain a valid document node with `DOM.getDocument(depth:0)`. Check URL/mode,
   query `DOM.querySelector(nodeId, "link")`, require integer node ID zero, and
   verify the same frame/loader. No page JavaScript is evaluated. Document
   replacement invalidates this evidence; a second scan cannot erase the failure.
6. Verify the viewport. Request the unchanged surface PNG screenshot without
   capture beyond the viewport. Decode/sample it and verify the frame tree again.
7. Finish every observed favicon candidate and document-fulfillment acknowledgment
   within the same absolute capture deadline; check the frame tree and resource
   state again. No expected color is available to this execution path.
8. Send browser-scoped `Browser.close` while retaining fixture-session routing.
   Continue processing events after its acknowledgment until verified response-pipe
   EOF, after all complete frames have been dispatched. Root exit or a broken
   command pipe still requires this incoming-stream inspection. Expiry before
   EOF, truncated framing, late policy violations and incomplete resource evidence
   remain failures. Native termination/reaping/artifact verification is independently
   mandatory before an observation can be returned.

The ordinary target uses Chrome's presentation-capable tab/window path. The
pinned [Chrome target handler](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/chrome/browser/devtools/protocol/target_handler.cc)
can create the first window itself. The old requirement for an existing debugging
page belongs only to hidden-target creation. The historical Linux A/B evidence
below established the target-dependent screenshot behavior, not a measured
compositor callback failure.

Startup is bounded by 10 seconds; navigation and capture by 5 seconds each;
explicit and emergency cleanup by 5 seconds. Browser.close gets at most 250 ms, capped by the remaining capture budget.
Deadlines bound waiting and never establish success. There are no readiness
sleeps or success-after-N-retries rules.

Qualification builds (`cfg(test)` only) retain the active capture operation and
command ID, sending/receiving/response-wait stage, native check stage, and the
last 16 completed capture-operation labels. The fixture test logs its fixed
fixture identity and iteration before each capture and includes them on failure.
Before cleanup, a failed capture prints one progress summary, with no CDP
payloads, HTML, screenshot bytes, event history or elapsed times. PNG sampling
and document/viewport/frame verification have explicit local-operation labels.
Typed primary and cleanup errors are unchanged; successful CLI JSON and stderr
are unchanged. This is qualification instrumentation, not a production tracing
interface.

The failure summary samples only the retained direct child with one
`waitid(WNOHANG | WNOWAIT)` and the response pipe with one zero-time `poll`.
These diagnostic operations do not reap, discover, signal, retry or reset a
deadline. Root `NoExitObserved` and pipe state describe the instant failure is
observed, not historical state at the exact deadline or renderer health. Errors
(including EINTR) are unavailable evidence, not proof of liveness. Pipe hangup
can coexist with buffered data. The same kernel non-interruptibility limitation
as other native calls applies; the snapshot grants no signaling authority and
never changes the failed result or cleanup verification.

PNG data is bounded to 2 MiB, decoding to 8 MiB. Require 640 × 480 RGB8/RGBA8,
valid PNG completion/CRC and an opaque sampled alpha. Launch forces sRGB; tagged
PNG color metadata must agree with sRGB. Untagged output uses that pinned launch
contract. Unsupported profiles, conflicting/duplicate color tags, animation,
unknown chunks, malformed base64 and unsupported image types fail explicitly.
No color conversion, rounding, alpha compositing or expectation normalization
is performed.

## Resource policy and network limits

Fetch covers requests intercepted in the attached fixture page session, at
Request stage, for all resource types. The exact frame, URL, GET method and
Document type identify the sole fulfilled request. Its actual fulfillment
acknowledgment is required. Request identities cannot be shared between the
document and favicon. Unexpected Fetch requests are aborted where the current
deadline permits; their failure stays latched.

An exact main-frame GET for `https://borrowser.invalid/favicon.ico`, Network
`Other`/initiator `other` and Fetch `Other`, is only a provisional candidate.
URL/type/initiator alone cannot distinguish authored icons from browser-default
activity. The candidate is aborted immediately at Fetch Request stage, even if
the parsed-document check is pending. It is never continued or fulfilled.

For the current inline-only fixture profile, `DOM.querySelector(document, "link")`
must find no live-document link element. This deliberately rejects even inert
links. Chromium owns HTML parsing, character-reference decoding, case handling,
relation tokens and malformed-markup repair; the harness performs no raw text
scan or second parse. Comments and raw text are not elements. Inert template
content is not an active icon declaration, and disabled page scripts cannot
activate it. This is a restriction of AG2's static fixture scope, not a universal
safety claim for arbitrary HTML. The pinned
[document icon selection](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/third_party/blink/renderer/core/dom/document.cc)
uses authored HTML link candidates or synthesizes the default icon.

The no-link result becomes authoritative only after the intended loader's load,
valid document URL/mode/node identity, query result and unchanged frame/loader
verification. Production scripts remain disabled. The private script-enabled
positive control accepts only the exact existing color-changing input, checked
before launch. Arbitrary scripts could create and remove an icon before a DOM
snapshot, so altered inputs are configuration errors even if their final DOM
would have no links. The same original control remains blue with scripts disabled
and red with scripts enabled. This exception exists only in test builds.

| Activity/evidence | Decision |
| --- | --- |
| One exact fixture Document GET | Fulfill original bytes; require successful command and navigation correlation |
| First favicon-shaped request, policy pending | Abort at Request stage; retain one bounded candidate, not an exemption |
| No-link policy plus matching Network/Fetch IDs, abort acknowledgment and failed-load terminal | Accept as intercepted default activity |
| Authored link, including an icon using `/favicon.ico` | Fail, even if its request was aborted |
| Duplicate, redirected, conflicting, malformed or foreign-session request | Fail |
| Candidate response or successful transfer | Fail |
| Missing request pairing, abort acknowledgment or terminal failure | Never succeed; deadline or incomplete-evidence failure |
| Other resource, frame/target or navigation | Fail |
| No favicon observed | No exemption needed; still require document eligibility |

The candidate's Fetch interception ID, Network ID and abort command ID are kept
separately, with bounded nonempty request IDs (128 bytes). Both Network/Fetch
orders and both acknowledgment/terminal orders are supported. Completion requires
`Fetch.failRequest(BlockedByClient)` to succeed and the same request's unique
`Network.loadingFailed`, type `Other`, with the pinned Inspector abort reason
`net::ERR_BLOCKED_BY_CLIENT.Inspector`. This exact reason was observed in the
macOS v2 run; generic network failures do not satisfy the contract. A browser
favicon download is correlated through its actual frame/session and request IDs,
not an invented document-loader identity. Optional auxiliary events confer no
classification authority. There is no unbounded event collection.

Document readiness is separate from favicon completion, avoiding a paused-request
readiness cycle. All policy work uses the existing capture deadline. During
shutdown, close-command delivery is separate from response-stream validation.
Root exit or command-side disconnection does not skip incoming event processing
and grants no signaling or reaping authority. Success requires actual response-pipe
EOF with no partial frame, all preceding complete frames dispatched, complete
resource evidence, and unexpired shutdown and capture budgets. A close
acknowledgment, empty local buffer, zero-time poll or root exit is not a substitute
for EOF. Shutdown expiry remains `Timeout(Shutdown)`; EOF inside a frame remains
`Protocol(Truncated)`. Policy and other protocol errors propagate unchanged.
The existing 250 ms limit remains capped by the remaining capture deadline;
there is no timeout extension or fallback to native cleanup as protocol evidence.
No favicon observed is not a promise of future silence:
interception remains installed until teardown, and no network-idle heuristic or
fixed delay authorizes capture.

A fresh profile prevents prior cache/service-worker state. Network cache is also
disabled and service workers bypassed. Page scripts remain disabled through
capture. Browser background networking, extensions, component extension background
pages, sync and component updates are suppressed with launch flags. Browser-owned
background worker targets are outside the page-target discovery filter.

This guarantees a **self-contained fixture with no permitted external page
resource dependencies**. It does **not** guarantee zero outbound OS connections:
browser services, DNS/speculation, crash reporting and other targets are outside
this Fetch session. Flags are not a firewall. No proxy, listening HTTP server,
network namespace or OS isolation platform is introduced. The fixture's response
comes entirely from the harness, so it needs no external DNS, TLS or HTTP server.
Complete egress prevention is not a present AG2 requirement.

## Native lifecycle guarantees

`OwnedChromium` owns the root, pipes, identity registry and private temporary
directory. Paths are canonicalized before launch, including macOS `/var` aliases.
The browser receives a unique `--user-data-dir`; `BREAKPAD_DUMP_LOCATION` points
to its unique private crash database. Chromium's
[crash reporter client](https://github.com/chromium/chromium/blob/155.0.8059.39/chrome/app/chrome_crash_reporter_client.cc)
honors that override before handler launch.

Cleanup closes the command pipe, allows graceful exit, sends TERM after one fifth
of the cleanup budget and KILL after one half. It discovers late descendants,
retains process identities, verifies non-execution, and only then reaps/removes
artifacts. The unreaped root reserves its PID/session ID throughout discovery.
Signal success alone never proves termination. Incomplete discovery, denied
inspection/signaling, failed reaping or artifact removal prevents success.

One absolute deadline is passed through each native operation: process-list
iteration, ownership/argument inspection, retained-identity acquisition and
verification, signaling, final discovery and reaping. Checks run before and
after native calls and during user-space loops. Execution discovery also checks
cancellation; cleanup deliberately ignores cancellation to finish its termination
attempt. An interrupted or expired scan never becomes a complete observation.
The caller checks the deadline again after termination verification, after
reaping and after artifact removal. A repeated call cannot turn failed cleanup
into success.

These are enforced user-space budgets, not a promise to preempt the kernel.
A native call or filesystem removal may return after the deadline; its late
result is rejected. Once time expires, further discovery/signaling/reaping stops
and cleanup reports a timeout, preserving other errors. When termination or
reaping cannot be verified, the profile is retained with diagnostics. If artifact
removal itself returns late, already removed files cannot be restored, but no
successful observation is reported. No incomplete scan substitutes for full
ownership verification.

### Linux x86-64 (runtime qualification outstanding)

The standalone capture process sets `PR_SET_CHILD_SUBREAPER`; the ordinary AG1
path does not. `pidfd_open`/`pidfd_send_signal` availability is probed. `/proc`
supplies session, parent and birth-time evidence, rechecked after opening each
pidfd. The root, anchored-session members and children adopted by this otherwise
childless harness are owned. Detached helpers become adopted when their ancestors
exit. Retained pidfds are signaling authority for exactly those identities; PID
reuse cannot redirect a signal. No raw `kill(pid)`/`killpg` fallback is used.

After known creators terminate, a fresh complete scan finds newly adopted
descendants. The direct child is reaped with `waitpid`; adopted children are
reaped until `ECHILD`. An incomplete scan, live adopted child or unknown wait
failure is an error. This is specific to the standalone, one-browser capture
path; it is not a library-wide subreaper or process supervisor.

### macOS arm64

The implementation dynamically resolves `proc_signal_with_audittoken` and
`proc_pidpath_audittoken`, obtains real `TASK_AUDIT_TOKEN` values through retained
`task_name_for_pid` ports, and checks that a token with an incorrect generation
is rejected with ESRCH. Permission failures are explicit. These libproc APIs
are present in the installed SDK/runtime; their availability is not assumed
across every macOS release or execution sandbox.

Full same-UID enumeration plus anchored session membership identifies ordinary
helpers. Detached Crashpad candidates require the pinned app's bundled handler
path **and** the exact private `--database` argument, revalidated under the token.
The exact launched executable plus private profile identifies a transient detached
fork before exec. Paths/arguments/session IDs are ownership evidence, never a
license to signal a subsequently reused numeric PID. This assumes the trusted
pinned topology and no malicious same-user process impersonating the private
capture; arbitrary process behavior is not supported.

All signals use the retained audit token. Apple's
[kernel implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/proc_info.c)
resolves that identity, checks signaling privileges, reacquires the matching
process reference and signals that referenced process. A user-space generation
check followed by ordinary `kill(pid)` would not provide this guarantee and is
never used. Tokens identify targets; they do not bypass the caller's permissions.

A final scan after known creators terminate checks for remaining owned helpers.
Token-aware path queries plus zombie state distinguish exited/non-executing
identities; failed or truncated metadata never establishes success. The harness
reaps its direct child; macOS launchd reaps reparented helpers. The harness can
verify that those helpers no longer execute, but cannot `waitpid` launchd's
children or promise when launchd removes their zombie entries.

Darwin's [`KERN_PROCARGS2` implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sysctl.c)
can return EIO while copying argument memory during
process exit/exec, before other metadata reports a zombie. That result leaves
the entire scan incomplete and does not admit the candidate for signaling.
Discovery retries through the existing cleanup loop and absolute deadline;
only a later complete scan can establish termination. Persistent unavailability
times out and retains artifacts. Permission errors and malformed/truncated
argument data remain explicit errors. Isolated regressions exercise both a
transient argument-copy failure and persistent unavailability during cleanup.

### Failure boundaries

Normal errors, INT/TERM cancellation, early returns and Rust panic unwinding run
bounded cleanup; Drop supplies an emergency attempt with visible diagnostics.
Primary and cleanup errors are retained separately. Cleanup failures retain the
temporary directory and print its path only in failure diagnostics.

SIGKILL of the harness, abort without unwinding, OS failure and power loss cannot
run Rust destructors. No automatic cleanup or artifact removal is guaranteed in
those cases, and no success report is emitted. An uninterruptible kernel task or
revoked native permissions can also prevent confirmed cleanup within the deadline.
There is no durable journal, external supervisor or cleanup of other users' jobs.

## Results and acceptance evidence

`ChromiumCapture` contains `CanvasColor` and `BrowserIdentity` (product, version,
revision, protocol, OS/architecture and fixed capture-profile ID).
`Error`, `ProtocolError`, `Phase`, `CleanupError` and `Failure` distinguish
configuration/identity, launch, protocol, navigation, capture, timeout,
cancellation, premature exit and cleanup failure. They do not refactor AG1's
execution/comparison model.

Only fully cleaned-up observations enter the JSON report, in fixture registry
order. Reports exclude timestamps, PIDs, temporary paths, frame/session IDs,
durations and stderr. Failure diagnostics may include execution details and are
not reference artifacts. No partial success report is emitted if a later fixture
fails.

Qualification commands:

```sh
cargo test -p borrowser-conformance --locked
BORROWSER_CHROMIUM_EXECUTABLE='/path/to/pinned/browser' \
  cargo test -p borrowser-conformance --locked real_chromium_ -- --ignored --nocapture --test-threads=1
```

Tests cover malformed/bounded PNGs and CDP frames, document identity/order,
redirect/duplicate/subresource rejection, missing/mismatched browser, FD3/4
collisions and direction/close-on-exec, exec failure, EOF, stubborn/detached and
prematurely exiting helpers, cancellation, Drop cleanup and unrelated sibling
survival. Real tests capture both shipped fixtures three times in independent
processes, check script suppression against a test-only script-enabled red-pixel
control, reject external requests/navigation, and compare serialized reports
across independent CLI processes. Real lifecycle tests inspect Crashpad topology
and exercise cancellation, timeout and forced browser exit.

Isolated lifecycle regressions inject otherwise inaccessible descriptor,
post-fork mask-restoration, launch-status, cancellation and unwind failures.
The helper holds stderr open without writing and ignores TERM, requiring bounded
KILL/verification/reaping. Native-operation fault delays exercise deadline
expiry during discovery, final termination verification and reaping; they assert
timeout errors, retained artifacts and unrelated-process survival. Linux also
tests expiry while reaping an adopted descendant. Fault injection and rescue of
deliberately interrupted test children exist only under `cfg(test)`.

The 2026-10-09 macOS acceptance rerun exposed an intermittent argument-copy EIO
in the default parallel native tests. After the incomplete-scan correction above,
`cargo test -p borrowser-conformance --locked native_ -- --nocapture` passed both
native test groups, including transient/persistent argument-copy regressions.
The complete conformance run, with the pinned executable configured, passed all
27 tests (23 unit, two Chromium CLI, two AG1 CLI):

```sh
cargo test -p borrowser-conformance --locked -- --include-ignored \
  --skip chromium::tests::native_case --skip chromium::tests::browser_helper \
  --test-threads=1
```

Full `make ci` then completed with exit 0 on macOS 27.0 arm64, build 26A428.
This includes workspace/HTML5 tests, all three Clippy configurations, formatting,
parser/golden/WPT and fuzz lanes, debug/release builds, benchmark compilation and
the generated-entity check. Real Chromium tests are opt-in and were covered by
the separate complete conformance run above. Neither result qualifies Linux.

### GitHub-hosted Linux qualification lane

The existing `.github/workflows/ci.yml` has a dedicated
`ag2_linux_chromium` job on `ubuntu-24.04`, separate from the existing Rust and
fuzz jobs. This fixes the Ubuntu release and x64 architecture, not an immutable
VM image: the job records the actual image version, kernel, OS, CPU information,
virtualization, run URL/attempt, PR head SHA and checked-out SHA. It requires a
non-root x86-64 VM and rejects containers. It does not use Docker, CPU emulation,
SSH, AWS or the runner's preinstalled browser.

The job downloads only the Linux archive named in `chromium-reference.json`,
verifies its SHA-256 before extraction, and installs it root-owned at
`/opt/borrowser-ag2/chrome-linux64/chrome`. Runtime libraries correspond to the
pinned executable's ELF dependencies. The repository-pinned Rust toolchain builds
and lints the conformance crate; the existing workspace jobs retain their full
validation responsibilities.

Temporary storage has three separate purposes: Cargo, rustc and build scripts
inherit `$RUNNER_TEMP/ag2-build-tmp`; Chromium profiles and crash databases live
under `$RUNNER_TEMP/ag2-profiles`; records live in `ag2-evidence` and the bounded
upload directory. Only the two execution steps configure Cargo's
[target runner](https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner)
as `env TMPDIR=.../ag2-profiles`. Cargo applies it to test/CLI executable launches,
including `cargo run`; it does not apply it to compilation or build scripts.
Consequently a test target rebuild still uses build temporary storage. Isolated
test helpers and CLI subprocesses inherit the runtime value. The strict cleanup
gate explicitly inspects `ag2-profiles`, independently of the shell's build
`TMPDIR`. No compiler-file exception or pre-verification deletion is permitted.

Ubuntu 24.04 restricts unprivileged user namespaces. Following Chromium's
[documented per-executable AppArmor approach](https://chromium.googlesource.com/chromium/src/+/main/docs/security/apparmor-userns-restrictions.md),
the job loads a profile permitting `userns` only for that exact root-owned
executable path. It does not change global namespace/security sysctls, install
a privileged sandbox helper, run the browser as root, or add sandbox-disabling
flags. Loading the profile is required, but is not itself sandbox qualification.
During each Linux real-browser lifecycle scenario, a renderer CDP command
establishes readiness before read-only inspection of renderer IDs from that
browser's `SystemInfo.getProcessInfo`. Its integer `id` is decoded without
floating-point coercion and checked for positive native-PID range. Missing,
malformed, non-integer, zero, negative and out-of-range IDs fail qualification.
Bounded `/proc/<pid>/status` reads must show `NoNewPrivs: 1`, `Seccomp: 2` and
nested PID namespace membership. Missing
or incompatible evidence fails the test. These diagnostic IDs never authorize
signaling and never enter stable capture reports. This verifies those sandbox
mechanisms at runtime, not every detail of Chromium's security policy.

Native tests exercise actual pidfd opening/signaling, child subreaping,
detached/early-exiting children, adopted-child reaping through `ECHILD`, deadline
regressions and unrelated-process survival. Each isolated scenario is named in
the log. The complete conformance suite explicitly includes ignored real-browser
tests while excluding the two private helper entry points. It covers both AG1
fixtures, independent-process repeatability, script suppression and its positive
control, resource/navigation rejection, cancellation, timeout and forced exit.
An additional ordinary CLI capture preserves the exact color/provenance JSON.

The native owner still supplies authoritative cleanup verification; the job
additionally requires its dedicated temporary directory to be empty before
writing a success marker. Only explicitly named qualification logs,
manifest/checksum, environment, capture records and diagnostic summaries are
copied into the upload directory, with 14-day retention. Each exported record
is limited to its final 1 MiB; any truncation is recorded in `diagnostics.log`.
Raw Chromium profiles, databases, crash dumps and hidden browser-state files
are never uploaded. Retained profiles remain on the disposable runner; their
existence still fails the required cleanup check.

Optional process summaries (PID, parent, session, state and command name, without
arguments), kernel journal excerpts and retained-artifact inventories have
10-second timeouts with a one-second kill grace and 64-KiB output bounds.
The inventory contains only relative names and sizes, does not follow directory
symlinks, and stops at depth three. Command errors are
preserved in the corresponding summary; nonzero exits, timeouts and truncation
are recorded in `diagnostics.log`; reaching the byte limit is marked as possible
truncation even when the command exits successfully. Unavailable optional
diagnostics do not fail qualification, and cannot change a required step's
failure into success. The
workflow neither signals processes by discovery heuristics nor substitutes
these partial summaries for the owner's complete verification.
Job timeout/cancellation is a failed or incomplete qualification, never a pass;
artifact collection cannot be guaranteed after abrupt runner loss.

This workflow currently runs for pull requests and pushes to `master`. Running
the new job therefore requires committing/pushing the complete AG2 change and
opening a (possibly draft) PR. A PR run normally checks out the synthetic merge
commit, so both that SHA and the source head are recorded. The existing passing
AG1 runs only establish runner availability. **No AG2 Linux run has passed yet.**
Before acceptance, record the successful AG2 run URL, exact tested commit and
image, and its native/browser/sandbox/cleanup results below; all required CI jobs
must pass on the final PR tree. The existing macOS evidence remains independent.

The first AG2 run, [38025161958, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/38025161958),
tested source `db2c8d0995fd0e07e81354a7fc9d509ec15751d8` through merge
`334fe351bcf7b5be1bf298a10b0983d6a3bb1309`. Native regressions and real-browser
lifecycle/sandbox assertions passed, as did the other workspace CI jobs. The
fixture loop failed with `Timeout(Capture)` and no cleanup errors (23 unit tests
passed, one failed). Its log did not identify the operation or iteration. Linux
fixture repeatability, script/resource checks, CLI records and the final
directory gate were not completed; these remain acceptance requirements. No
capture behavior or timeout is changed on the basis of that incomplete evidence.
The diagnostic correction required the next run to distinguish a pending
CDP operation from time consumed by native inspection, before choosing a fix.

That run also retained `rustix_test_can_compile` in the runtime directory because
the earlier workflow set `TMPDIR` before Cargo ran. The target-runner separation
above corrects that demonstrated workflow defect without weakening cleanup.

The diagnostic/temp-directory correction was validated locally on macOS 27.0
arm64 (26A428) on 2026-10-10: the complete pinned-browser conformance invocation
above passed 31 tests (27 unit, two Chromium CLI, two AG1 CLI), using separate
build/runtime directories through the macOS Cargo target runner. The actual
runtime directory was empty afterward. New isolated cases distinguish response
timeout, blocked send, expiry inside ownership discovery, disconnected pipe and
exited root; each preserves typed failures, command ID/method and completed
operation evidence, and verifies cleanup and unrelated-process survival. A unit
test bounds the completed-operation history. Formatting, conformance Clippy,
actionlint and AG2 shell syntax checks passed.

A temporary local Cargo probe also forced a build-script rerun during `cargo
test`, then executed `cargo run`: compiler markers remained in build storage,
test/CLI execution used runtime storage, and the empty-directory gate passed.
A retained-runtime-file negative control failed the literal workflow gate and
produced no success marker. This is macOS validation of Cargo's execution
boundary, not Linux acceptance.

### Hidden-target screenshot investigation

The second Linux run, [38048172848, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/38048172848),
tested source `b5f594b17d5a2a35c40dcedb028ef55bb2e45700` through merge
`06602e4ee83e01e90748d3f7d57226a12bd9941f`. The runner was Ubuntu 24.04.5,
image `20261004.327.1`, kernel `6.17.0-1022-azure`, native x86-64 on a Microsoft
VM with AMD EPYC 7763 CPUs. Native lifecycle and renderer sandbox tests passed,
as did the other ten CI jobs. The first `canvas/root` iteration completed
document readiness, `DOM.getDocument`, URL/mode checks, `Page.getLayoutMetrics`
and viewport checks. Command 21, `Page.captureScreenshot`, was sent and remained
pending until `Timeout(Capture)`. Failure observation found `NoExitObserved`
and an `OpenEmpty` response pipe; cleanup returned no errors. This establishes
a missing screenshot response, not a measured compositor failure. The bounded
artifact inventory contained no runtime files, including compiler artifacts;
the required final empty-directory gate was skipped after the test failure.

Source inspection uses the exact pinned revision
`3ff7ac5a9224be9156d7f8703a06e22890aafd34` (its `chrome/VERSION` matches
155.0.8059.39):

- The [Chrome target handler](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/chrome/browser/devtools/protocol/target_handler.cc)
  delegates hidden targets to content. The
  [hidden target manager](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/devtools/protocol/hidden_target_manager.cc)
  creates plain WebContents, without the normal Chrome tab/window attachment.
  CDP `hidden: true` does not itself prove that WebContents currently reports
  `PageVisibilityState::kHidden`.
- [PageHandler::CaptureScreenshot](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/devtools/protocol/page_handler.cc)
  holds a capturer with `stay_hidden=true`. WebContents can consequently paint
  while hidden; this does not establish a presentation-capable native surface.
  The explicit hidden-page assertion and stall warning in that handler are
  inside the `kCDPScreenshotNewSurface` feature-enabled branch.
- That feature is [disabled by default](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/common/features.cc).
  Without it, [GetSnapshotFromBrowser](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/renderer_host/render_widget_host_impl.cc)
  requests ForceRedraw. Blink's [WidgetBase::ForceRedraw](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/third_party/blink/renderer/platform/widget/widget_base.cc)
  waits for next-frame presentation feedback before invoking the callback that
  leads to CopyFromSurface and the screenshot response. With the feature enabled,
  Chromium instead requests a new surface and queues its copy immediately.
  AG2 supplies no override; the effective runtime feature state has not been
  measured. Source defaults are not proof of that state.
- [Aura's compositor lookup](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/renderer_host/render_widget_host_view_aura.cc)
  needs a window host. [WebContentsViewAura](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/web_contents/web_contents_view_aura.cc)
  can initially create an unattached window. In contrast,
  [BrowserCompositorMac::UpdateState](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/content/browser/renderer_host/browser_compositor_view_mac.mm)
  can acquire its own compositor when its host is not hidden and no parent
  compositor exists. These paths explain a plausible platform difference;
  they do not establish which callback stalled on the Linux runner. The Mac
  window-snapshot delay is in the non-surface branch and does not explain AG2.

The historical ignored, test-only
`chromium::target_probe::real_chromium_target_presentation_experiment` compared
hidden and ordinary headless page targets for both original fixtures, each in
an independent process. Both arms retained the pinned browser, startup bootstrap,
flags/sandbox, fixture bytes/URL, script disabling, readiness checks, five-second
capture deadline, PNG decoder/sampler and native cleanup. The experiment logged
each target/fixture and typed failure or validated dimensions/pixel/provenance.
It attempted all four captures after cleanly handled failures, but failed the test
if any capture failed; a reproduced hidden-target timeout is not acceptance.
Unverified cleanup stopped the experiment immediately.

The probe's only resource exception was test-only, guarded by exact equality to
the embedded original fixture bytes. Those bytes contain no authored resource
or icon. It recorded the single browser-default favicon request separately,
required the main frame, exact `/favicon.ico` URL, GET, Other type and other
initiator, correlated Network and Fetch IDs, and aborted it at Fetch Request
stage. Duplicates, redirects, ID mismatches, incomplete interception or an HTTP
response failed. Other requests went through the unchanged strict fixture policy.
Unit tests rejected altered fixtures, authored-resource signatures and incomplete
evidence. This is not a production favicon allowlist: an authored icon can use
the [same favicon helper](https://github.com/chromium/chromium/blob/3ff7ac5a9224be9156d7f8703a06e22890aafd34/components/favicon/content/content_favicon_driver.cc),
so URL/type/initiator alone cannot justify a production exemption.

The probe was executed by the existing Linux job in
[run 38075849398](https://github.com/joris97jansen/borrowser/actions/runs/38075849398),
source `3d8eb34027d014b504b8e1ab98cbf9a8153c1811`, tested merge
`084e6b40eded27303a3f012d5b742a8502168e58`. Both hidden arms completed document
readiness and timed out awaiting `Page.captureScreenshot`. Both ordinary arms
returned valid 640 × 480 PNGs, sampled `[18,52,86]` and `[52,86,120]`, and verified
native cleanup. Each ordinary arm correlated and aborted one default favicon.
Native lifecycle and renderer sandbox assertions passed. The experiment correctly
failed on the hidden arms; production hidden capture also failed, leaving the
final strict runtime-directory gate skipped. These results established the
behavioral difference and justified the approved ordinary-target correction;
they did not constitute complete Linux acceptance or identify the precise
stalled compositor callback.

The v2 implementation deletes the temporary module, target-selection hooks and
probe policy after transferring correlation coverage into permanent tests. There
is no intentionally failing hidden-target test or expected-timeout success in
the final suite. The existing `--include-ignored` job, build/runtime directory
separation, diagnostic retention and strict cleanup gate remain unchanged.

Local validation on 2026-10-10, macOS 27.0 arm64 (26A428), passed all four probe
captures: both targets returned `[18, 52, 86]` for `canvas/root` and
`[52, 86, 120]` for `canvas/cascade`, through the ordinary validated 640 × 480
PNG path and verified cleanup. Each ordinary page produced one correlated,
intercepted and aborted default-favicon request; hidden targets produced none.
The final complete conformance invocation passed 34 tests (30 unit, two Chromium
CLI, two AG1 CLI), including the two probe-policy unit tests, the A/B experiment,
native lifecycle regressions, original three-process fixture repeatability,
script suppression/positive control, strict resource rejection and independent
CLI serialization. The dedicated runtime directory was empty afterward.
Formatting, conformance Clippy, actionlint and all 22 workflow shell blocks'
syntax checks passed. Full `make ci` was not rerun for this test-only experiment.
These results validate the probe on macOS and do not establish the Linux cause
or qualify Linux capture.

### Production v2 local validation

On 2026-10-10, macOS 27.0 arm64 (build 26A428, Darwin 27.0.0), the corrected
production path passed the complete applicable conformance suite: **40 passed**
(36 unit/native/real-browser tests, two Chromium CLI tests, two AG1 CLI tests),
with only the two private subprocess entry points excluded from direct selection.
Those helpers were exercised by their registered native parent tests. The focused
CDP policy group passed all ten tests. Registration was checked with `--list`.
The mac-arm64 archive SHA-256 was rechecked against the unchanged manifest:
`529a71bd61aaa2ef266a4d4bd300ae9572ba6a3468a8d55c3023ffeffb5b6b4e`.
Every real capture checked the browser product/version/revision/protocol through
its actual CDP connection.

Each original fixture passed three independent 640 × 480 surface-PNG captures:
`canvas/root` `[18,52,86]`, `canvas/cascade` `[52,86,120]`. Each asserted a
correlated favicon, actual abort acknowledgment, matching failed-load terminal,
profile `ag2-canvas-srgb-v2` and verified native cleanup. Independent CLI runs
produced identical complete capture JSON with the unchanged report schema.
The original script suppression and red positive control passed. Authored icons,
encoded/case-varied/token-list relations, malformed placement and inert links
failed; comment/raw-text lookalikes passed. Stylesheet/image/iframe and immediate/
delayed meta-refresh rejection remained intact.

Seven new isolated native resource scenarios passed: rejected abort, missing
abort acknowledgment, missing terminal event, valid shutdown interception,
incomplete shutdown interception, an authored resource after the close
acknowledgment, and document invalidation during shutdown. Each verified profile
removal, complete child reaping and unrelated-sibling survival. Existing launch,
partial initialization, cancellation, deadline, EIO and native identity tests
also passed; macOS real-browser topology/cancellation/timeout/forced-exit checks
passed with the same native ownership mechanism and sandbox-enabled flags.

The first development run exposed the pinned terminal spelling
`net::ERR_BLOCKED_BY_CLIENT.Inspector`; a bare `net::ERR_BLOCKED_BY_CLIENT`
check correctly failed closed with successful cleanup. The permanent check and
regressions require the observed Inspector reason rather than accepting generic
network failure.

Validation commands used the pinned executable through
`BORROWSER_CHROMIUM_EXECUTABLE`:

```sh
cargo test -p borrowser-conformance --locked -- --list
cargo test -p borrowser-conformance --locked chromium::cdp::tests -- --nocapture
cargo build -p borrowser-conformance --locked
cargo fmt --all -- --check
cargo clippy -p borrowser-conformance --all-targets --locked -- -D warnings
cargo test -p borrowser-conformance --locked -- --include-ignored \
  --skip chromium::tests::native_case --skip chromium::tests::browser_helper \
  --test-threads=1 --nocapture
git diff --check
```

These commands passed. Native/real-browser tests ran outside the restrictive
execution sandbox so the required macOS inspection and audit-token permissions
were available; Chromium's own sandbox was not disabled. Cargo used
`/private/tmp/borrowser-ag2-v2-build-tmp`; its macOS target runner set test/CLI
`TMPDIR=/private/tmp/borrowser-ag2-v2-profiles`. The runtime directory was empty
after the suite, with no deletion or filename exclusions before verification.
Logs are local execution evidence, not stable report content. This macOS v2
result does not qualify Linux.

Full local `TMPDIR=/private/tmp/borrowser-ag2-v2-build-tmp make ci` completed with
**exit 0** on the corrected implementation. This covered workspace lint/test
lanes, parser/fuzz/golden checks, debug/release builds, benchmark compilation and
the generated-entity check. Existing HTML/CSS release-build warnings remained;
no production-engine files were changed to suppress them. The evidence-record
update is documentation-only. This passing local CI is macOS evidence, not a
GitHub-hosted Linux qualification result.

### Shutdown stream-boundary correction

The subsequent independent review found two shutdown paths that could accept
undispatched input: failed close-command delivery after root exit skipped the
read loop, and the brief shutdown timeout could excuse a partial or buffered
frame. `close_browser` now continues incoming inspection after expected
command-side disconnection and accepts only verified response-stream EOF. Native
cleanup still runs on every outcome and cannot convert a protocol failure into
an observation.

On the same macOS 27.0 arm64 environment, the corrected tree passed all ten
focused CDP tests, the registered resource/shutdown parent with **14 isolated
scenarios**, and the complete **40-test** conformance suite. The full suite
exercised **43 isolated native scenarios**, each checking unrelated-sibling
survival. The seven additional resource scenarios cover root exit with a queued
document invalidation or clean stream, native discovery expiry with a complete
buffered invalidation, partial framing held open through shutdown expiry,
truncated EOF, clean acknowledged EOF, and invalidation after the close
acknowledgment. Existing late authored-resource and incomplete-favicon cases
remain required. Resource scenarios verify profile removal and reaping through
`ECHILD` after both successful and failed protocol inspection.

The buffered-deadline scenario forces the next real native discovery through the
existing test-only deadline injection; it checks that the fault fired, discovery
was the active native operation, and the complete event remained undispatched
while previously verified resource state stayed complete. The partial-timeout
case similarly requires actual nonempty, unterminated incoming bytes. These
conditions prevent an unrelated timeout or a zero-test selection from counting
as the intended regression. No fault hook is present in production builds.

Both original fixtures again passed three independent captures with the same
pixels and `ag2-canvas-srgb-v2` provenance. Favicon acknowledgments and terminal
events, script suppression/positive control, authored-resource rejection,
stable CLI serialization and real native cleanup passed. Each successful
capture now also requires actual response-stream EOF within the unchanged
shutdown budget. The dedicated `/private/tmp/borrowser-ag2-shutdown-profiles`
directory was empty after execution; no files were removed to obtain that result.
The unchanged mac-arm64 archive checksum was reverified. Formatting and
warnings-denied conformance Clippy passed. Logs are retained locally as
`/private/tmp/borrowser-ag2-shutdown-{registration,native,policy,conformance,clippy}.log`.
The focused native invocation was:

```sh
cargo test -p borrowser-conformance --locked \
  chromium::tests::native_resource_completion_and_shutdown \
  -- --exact --nocapture --test-threads=1
```

Full `TMPDIR=/private/tmp/borrowser-ag2-v2-build-tmp make ci` also completed with
**exit 0** on the corrected Rust tree; the execution log is
`/private/tmp/borrowser-ag2-shutdown-ci.log`. This includes workspace builds,
feature/lint/test lanes, parser/fuzz/golden checks, release and benchmark builds,
and generated-entity verification. Only the documentation evidence record was
updated afterward. This is macOS evidence only; the later Linux production-v2
execution is recorded separately below.

### Linux production-v2 artifact-ownership finding

[Run 38110745639, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/38110745639)
tested source `93eb3c3904effaa169eed880a65cc52cc4515ee8`, merge
`a77adafa0de4fa4d31152cb9b78496016c5bb4e4`, on Ubuntu 24.04.5 LTS,
image `20261004.327.1`, kernel `6.17.0-1022-azure`, native x86-64 AMD EPYC
7763 in a Microsoft full-virtualization VM. The pinned Linux archive checksum
matched `55672d1f392fd3e7b7a08621b6e804e6bcb39d40cf155504abb74b3a021ea8ea`.

All 39 applicable conformance tests passed, including both original fixtures
three times each with exact 640 × 480 PNG pixels, profile v2, favicon abort
acknowledgment/terminal evidence, DOM/resource rejection, script controls,
independent CLI serialization and EOF-only shutdown. Each native invocation
exercised 42 isolated scenarios, including 14 resource/shutdown scenarios;
unrelated-process survival, pidfd/subreaper behavior and adopted reaping passed.
Renderer checks reported NoNewPrivs=1, Seccomp=2 and nested PID namespaces.
The Rust workspace job and all nine fuzz/regression jobs passed.

The strict final runtime-directory gate nevertheless failed on
`ag2-profiles/org.chromium.Chromium.uSidyz`. The final process snapshot contained
no Chromium processes. Its bounded inventory included only regular files and
was empty, so the directory's contents and exact creator were not established.
The pinned singleton implementation is a supported explanation, not proof of
that specific artifact's origin. The demonstrated ownership gap was inherited
shared TMPDIR versus removal of only the private profile/crash subtree.

The per-launch child TMPDIR correction above places those temporary artifacts
within the existing owner. It does not alter process signaling/reaping, browser
flags, capture profile, screenshot/resource policy, deadlines or CI's strict
gate. The corrected launcher still requires a new native Linux run with an
empty runtime directory and all required jobs passing. This historical run is
partial v2 evidence, not complete Linux qualification.

### Temporary-containment correction: local validation

On 2026-10-11, macOS 27.0 arm64 (26A428, Darwin 27.0.0), the registered
socket-path boundary test passed, and the new native parent passed five isolated
scenarios: `temp-clean`, `temp-exit`, `temp-cancel`, `temp-incomplete`, and
`temp-remove-failure`. A real exec child reported exactly one private TMPDIR;
its exec descendant inherited it and created evidence in the same subtree.
Files, nested directories and a symlink were removed after verified cleanup;
the parent environment and unrelated sibling directory/process survived.
Injected discovery expiry retained artifacts and reported timeout; a real
directory-permission denial reported artifact-removal failure after reaping.
Test-only recovery removed deliberately retained fixtures after these assertions.

The native group passed six registered tests (48 isolated scenarios); the full
applicable suite passed **42 tests** (38 unit/native/real-browser, two Chromium
CLI, two AG1 CLI). Both original fixtures again passed three independent captures
with exact pixels, v2 provenance, favicon evidence and EOF shutdown. Script/
resource/DOM controls and independent CLI serialization passed unchanged.
The unchanged mac-arm64 archive checksum was reverified against the manifest.
The dedicated runtime directory was empty afterward without any cleanup-gate
exclusion or pre-verification removal. An isolated control of the exact CI gate
accepted an empty directory and rejected a retained hidden directory with exit 1.

Commands used the pinned executable and separated build/target-runner TMPDIR:

```sh
cargo test -p borrowser-conformance --locked -- --list
cargo test -p borrowser-conformance --locked \
  chromium::tests::linux_temporary_socket_path_uses_bytes_and_reserves_nul \
  -- --exact --nocapture
cargo test -p borrowser-conformance --locked \
  chromium::tests::native_temporary_artifact_containment \
  -- --exact --nocapture --test-threads=1
cargo test -p borrowser-conformance --locked native_ -- --nocapture
cargo test -p borrowser-conformance --locked -- --include-ignored \
  --skip chromium::tests::native_case --skip chromium::tests::browser_helper \
  --test-threads=1 --nocapture
cargo fmt --all -- --check
cargo clippy -p borrowser-conformance --all-targets --locked -- -D warnings
cargo build -p borrowser-conformance --locked
```

All passed. Cargo used `/private/tmp/ag2-temp-build`; the macOS target runner was
`env TMPDIR=/private/tmp/ag2-temp-runtime`. Native execution used the required
host inspection permissions, without disabling Chromium's sandbox. Logs are
`/private/tmp/borrowser-ag2-temp-{registration,path,targeted,native,conformance,clippy,build}.log`.
The byte-boundary unit test on macOS does not qualify Linux. The Linux-only
excessive-path launch case and actual singleton-socket containment assertion
remain pending hosted execution, as does the unchanged final runtime gate.

Full `TMPDIR=/private/tmp/ag2-temp-build make ci` completed with **exit 0**;
the execution log is `/private/tmp/borrowser-ag2-temp-ci.log`. Workspace
feature/lint/test lanes, parser performance/fuzz/golden checks, debug/release
builds, benchmark compilation and generated-entity checks passed. Existing
release warnings outside conformance were not changed. This remains macOS
evidence, not acceptance of the pending native Linux correction.

### Explicit pre-fork cleanup: local validation

The subsequent review found that pre-fork initialization still relied on
`TempDir::drop`, which discards removal errors. The explicit pre-fork ownership
described above now preserves those errors independently of the primary failure.
On the same macOS 27.0 arm64 host, `native_prefork_artifact_cleanup` was registered
and executed both `prefork-clean` and `prefork-remove-failure`. Each injected
failure occurs after real directory/file creation and must be consumed. Both
retain the identical launch stage and EIO primary error. The first proves the
private directory was removed; the second uses actual directory permissions to
deny removal and asserts `CleanupError::Artifacts`, its private path, and the
retained file before test-only recovery. Both assert ECHILD, unchanged parent
TMPDIR and unrelated sibling-file/process survival.

The targeted parent passed one test/two isolated scenarios. The native group
passed seven tests with **50 isolated scenarios**. The complete pinned-Chromium
suite passed **43 tests** (39 unit/native/browser, two Chromium CLI and two AG1
CLI), including both fixtures three times with unchanged pixels, profile v2,
favicon/DOM/script controls, EOF shutdown and independent CLI serialization.
The dedicated runtime directory was empty after execution. Formatting,
warnings-denied conformance Clippy, normal production build and the socket-path
boundary test passed. The mac-arm64 archive checksum still matched the manifest.

The commands above were rerun using Cargo TMPDIR `/private/tmp/ag2-prefork-build`
and target runner `env TMPDIR=/private/tmp/ag2-prefork-runtime`; the additional
targeted invocation was:

```sh
cargo test -p borrowser-conformance --locked \
  chromium::tests::native_prefork_artifact_cleanup \
  -- --exact --nocapture --test-threads=1
```

Execution logs are
`/private/tmp/borrowser-ag2-prefork-{registration,targeted,path,native,conformance,clippy,build}.log`.
Full `TMPDIR=/private/tmp/ag2-prefork-build make ci` then completed with **exit 0**;
its log is `/private/tmp/borrowser-ag2-prefork-ci.log`. Workspace lint/test,
feature, parser/fuzz/golden, WPT-style fixture, debug/release, benchmark and
generated-entity checks passed. Only documentation evidence was updated afterward.
The Linux-only cases and final hosted artifact gate remain unqualified by this
macOS evidence.

| Platform | Evidence / outstanding acceptance |
| --- | --- |
| macOS 27.0 arm64, build 26A428 | Production v2: explicit pre-fork cleanup passed the 43-test suite, 50 isolated native scenarios, empty runtime check and full local CI on 2026-10-11; prior 42-test containment and 40-test shutdown evidence is retained above. Historical v1: passed real fixture/script/resource tests, lifecycle/topology tests and independent CLI serialization tests on 2026-10-09, outside Codex's restrictive sandbox. Normal helpers remain in the root session; two detached Crashpad handlers use the private database. Token permission/generation checks, cancellation, timeout and forced root exit passed; a final process scan found no processes from the test extraction. Other OS builds require requalification. |
| Linux x86-64 | Historical v1/A/B evidence remains separate. Production-v2 run 38110745639 passed capture/repeatability, resource/DOM/script/CLI/EOF, native lifecycle and sandbox tests, but failed the strict artifact gate. The child-TMPDIR correction, new Linux launch/socket regressions, empty runtime storage and complete final-tree CI require a separately authorized hosted run. |
| Linux ARM64 Docker host | Earlier offline `cargo check --all-targets` and `cargo clippy -p borrowser-conformance --all-targets --locked --offline -- -D warnings` passed. This historical build evidence was not rerun after the macOS argument-copy correction; capture explicitly rejects this architecture and it does not qualify Linux x86-64. |

AG2 remains one issue. It is not closeable across both intended platforms until
the Linux x86-64 row is qualified. No AG3/AG4 functionality or new GitHub issues
are introduced by the implementation phases or these acceptance checks.
