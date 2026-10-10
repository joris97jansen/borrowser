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

The screenshot is the sole color source. DOM inspection checks only the document
URL/mode; layout metrics check only the viewport. CSS declarations, computed
styles and DOM geometry are not alternative observations. No JavaScript is
evaluated by the harness. A wrong color remains an observation.

AG1 feeds its response in parser chunks; CDP fulfills one complete response.
These fixtures have no timing-dependent content. Chromium uses its real raster
pipeline and clock, whereas AG1 observes the production egui canvas output with
fixed time. No equivalence beyond this static pixel is claimed.

## Pipes and launch ownership

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

1. Verify the browser identity. Create an `about:blank` bootstrap target, create
   a hidden `about:blank` target, close the bootstrap, discover page/frame targets,
   and attach to the hidden target with a flattened session.
2. Enable Page lifecycle and Network events. Disable cache; bypass service
   workers. Disable page script execution **before navigation**, then set viewport,
   device scale and page scale. Enable Fetch interception for all request types
   at the request stage in this target session.
3. Obtain the main frame ID, initialize its navigation state, and navigate to
   the fixed fixture URL. Fulfill exactly one matching main-frame Document GET.
4. Correlate Fetch `networkId`, Network request ID/loader ID, `Page.navigate`
   frame/loader acknowledgement, committed frame URL/origin/loader, HTTP 200 HTML
   response and the **same loader's** load lifecycle event. An unrelated load,
   navigation acknowledgement alone or silence never establishes readiness.
5. Verify document mode/URL and viewport. Request an actual surface PNG screenshot
   without capture beyond the viewport. Decode/sample it, then query the frame
   tree again to verify the loaded document identity across capture.
6. Request `Browser.close` briefly; verified native cleanup is authoritative.

The hidden target is a supported pinned-CDP mechanism, chosen because ordinary
browser tabs autonomously request `/favicon.ico`. Its plain WebContents avoids
that UI helper without altering HTML or allowing extra requests. This pinned
build needs an existing remote-debugging page before hidden-target creation;
the bootstrap never loads fixture content. See Chromium's
[Chrome target handler](https://github.com/chromium/chromium/blob/155.0.8059.39/chrome/browser/devtools/protocol/target_handler.cc)
and [content target handler](https://github.com/chromium/chromium/blob/155.0.8059.39/content/browser/devtools/protocol/target_handler.cc).

Startup is bounded by 10 seconds; navigation and capture by 5 seconds each;
explicit and emergency cleanup by 5 seconds. Browser.close gets at most 250 ms.
Deadlines bound waiting and never establish success. There are no readiness
sleeps or success-after-N-retries rules.

PNG data is bounded to 2 MiB, decoding to 8 MiB. Require 640 × 480 RGB8/RGBA8,
valid PNG completion/CRC and an opaque sampled alpha. Launch forces sRGB; tagged
PNG color metadata must agree with sRGB. Untagged output uses that pinned launch
contract. Unsupported profiles, conflicting/duplicate color tags, animation,
unknown chunks, malformed base64 and unsupported image types fail explicitly.
No color conversion, rounding, alpha compositing or expectation normalization
is performed.

## Resource policy and network limits

Fetch covers requests intercepted in the attached fixture page session, at the
request stage, for all resource types. The exact frame, URL, GET method and
Document type identify the one permitted request. Duplicate requests, redirects,
external stylesheets/images, extra frames, new page targets, same-document
navigation and a changed committed document fail capture. Unexpected Fetch
requests are aborted where the current deadline permits; failure stays latched.

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

### GitHub-hosted Linux qualification lane (execution pending)

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

| Platform | Evidence / outstanding acceptance |
| --- | --- |
| macOS 27.0 arm64, build 26A428 | Passed real fixture/script/resource tests, lifecycle/topology tests and independent CLI serialization tests on 2026-10-09, outside Codex's restrictive sandbox. Normal helpers remain in the root session; two detached Crashpad handlers use the private database. Token permission/generation checks, cancellation, timeout and forced root exit passed; a final process scan found no processes from the test extraction. Other OS builds require requalification. |
| Linux x86-64 | The `ag2_linux_chromium` job provides a native GitHub-hosted `ubuntu-24.04` qualification path. Actual AG2 execution is pending a committed PR; native lifecycle, sandboxed Chrome startup, screenshots and cleanup remain **unverified acceptance criteria**. |
| Linux ARM64 Docker host | Earlier offline `cargo check --all-targets` and `cargo clippy -p borrowser-conformance --all-targets --locked --offline -- -D warnings` passed. This historical build evidence was not rerun after the macOS argument-copy correction; capture explicitly rejects this architecture and it does not qualify Linux x86-64. |

AG2 remains one issue. It is not closeable across both intended platforms until
the Linux x86-64 row is qualified. No AG3/AG4 functionality or new GitHub issues
are introduced by the implementation phases or these acceptance checks.
