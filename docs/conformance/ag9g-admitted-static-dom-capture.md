# AG9g Stage 0: controlled Chromium mechanism qualification

Status: Stage 0 implementation; real mechanism qualification **not established**.
The implementation environment is Darwin arm64, not the required Linux host.
No explicit pinned Chromium distribution has been supplied. No browser was
discovered, downloaded, or executed. AG9g, parent AG9, and Milestone AG remain open.

This document freezes Stage 0's collection configuration, distribution manifest,
packaging, Linux isolation, protocol subset, and mechanism-qualification contracts.
The [future evidence/admission contract](ag9g-collection-evidence-admission-v1.md)
is documentation only: its loader, publication, and advisory integration are not
implemented in Stage 0.

## Ownership and stages

`external-browser-capture` owns external process control, distribution snapshots,
configuration/source verification, and the Chromium adapter. It has no AG `TestId`,
aggregate variant, HTML parser, or Borrowser DOM dependency. `conformance-runner`
owns the single `conformance-capture` executable behind `external-capture`.
Ordinary aggregate execution does not depend on this crate or feature.

The internal `transaction::capture_static_dom_workload` owns the complete production
capture workload, including one retained verified distribution and checked outer
disposal. Its private per-input transaction owns fresh launch through checked
termination/reaping and watchdog completion; `PreparedSession` is only its private
CDP component. Qualification calls this workload inside the dedicated collector.
Future stages add byte handoff across that process boundary, not an in-process
aggregate call to a fail-stop-capable library service.

The unchanged `tools/conformance/web-observable-dom-tree-v1.mjs` is the single
authored external serializer. `html-test-support` remains the independent
Borrowser comparable-DOM producer. Production parsing is not rerun or modified.
`external-test-provenance` retains the sole artifact grammar and capture identity
authority. Stage 0 validates observation grammar but constructs no capture ID.

Stage 0 establishes mechanism feasibility. Stage 1 adds neutral fixture handoff
and orchestration. Stage 2 finishes candidate publication, followed by review,
**user commit**, a clean build, admission-grade qualification, and two genuine
fresh-process collections. Stage 3 adds offline reviewed admission and AG9
integration. Stage 0 cannot admit a capture, write registry records, or create
tracks, notes, baselines, trends, or two-attempt collection evidence.

Admission-grade qualification must use the exact final collector executable,
configuration, manifests, source revision, browser distribution, and environment
that produce attempts A and B. Any trust-bearing source, executable, packaging,
configuration, isolation, protocol, delivery, inspection, qualification, or
evidence-generation change invalidates that qualification, regardless of version
labels. The source must be clean, fully contained in the declared immutable Git
commit, and agree with the source manifest. Stage 0 development runs may be dirty;
they are never admission-grade historical evidence. The agent stops before commits.

## Canonical input rules

All new TOML inputs use bounded same-opened-object confined reads, typed Serde
deserialization with unknown-field rejection, explicit validation, deterministic
canonical serialization, and exact comparison with the original bytes. There is
no new TOML parser. Canonical output has UTF-8 without BOM, LF lines, one final LF,
no comments/blank lines, fixed field order, decimal unsigned integers, and
double-quoted strings. Strings use only `\"` and `\\` escapes. Arrays are compact
comma-separated sequences, without extra spaces. Arrays of tables use one
`[[name]]` line preceding each record. Empty collections use `name = []`.
Optional fields are omitted. Reject wrong types, unknown/missing/duplicate fields,
noncanonical input, duplicate records, and unsupported versions; never normalize
an input silently. Parser errors are local typed failures, not conformance fails.

Identity/version strings are 1–128 UTF-8 bytes, trimmed and without control
characters. Digests are 64 lowercase hexadecimal digits. Repository/distribution
paths are relative, at most 256 ASCII bytes, with components at most 64 bytes
using letters/digits/`._-`; no empty, `.` or `..` components, backslashes, or NUL.
The explicit distribution root is a runtime path, not a manifest-relative path.
Every trust-bearing repository file is bounded/read/hashed as the same opened
object. There is no check-path-then-reopen consumption path.

Typed parsing has bounded raw input and retained populations. These limits do
not claim an exact bound on every temporary allocation inside third-party TOML
or JSON parsing. Allocation failure cannot create an observation or GO result.

## `borrowser-static-dom-capture-chromium-linux-v1`

Exactly one configuration, at most 65,536 bytes. All fields below are required
in this order except `browser_build_revision`, which is omitted only when the
actual build does not expose it. A build that exposes a revision cannot omit it.
No actual qualified configuration is checked in: concrete browser pins must be
provided and verified before a GO verdict. Synthetic test values are not pins.

| Field | Exact value or meaning |
| --- | --- |
| format | `borrowser-static-dom-capture-chromium-linux-v1` |
| capture_mechanism | `borrowser-chromium-cdp-static-dom` |
| capture_mechanism_version | `17` |
| browser_product | Actual exact product, never inferred branding |
| browser_version | Exact actual version |
| browser_build_revision | Exact exposed revision, optional only if unavailable |
| browser_executable_path | Distribution-relative regular executable |
| browser_executable_sha256 | Exact executable content digest |
| browser_distribution_manifest_path | Confined repository-relative manifest |
| browser_distribution_sha256 | Raw canonical manifest digest |
| platform_os_family | `linux` |
| platform_os_version | Exact Linux `/etc/os-release` PRETTY_NAME value |
| platform_architecture | Exact Rust host architecture identity |
| kernel_release | Exact kernel release |
| cdp_protocol_version | Exact Browser.getVersion protocol version |
| cdp_contract | `ag9g-chromium-cdp-static-dom-v5` |
| capture_algorithm | `web-observable-dom-tree-v1-inspector` |
| capture_algorithm_version | `1` |
| capture_algorithm_path | `tools/conformance/web-observable-dom-tree-v1.mjs` |
| capture_algorithm_source_sha256 | Raw unchanged inspector bytes |
| packaging | `web-observable-dom-tree-v1-isolated-expression` |
| packaging_version | `1` |
| packaging_source_path | `crates/external_browser_capture/src/packaging.rs` |
| packaging_source_sha256 | Raw packaging implementation bytes |
| executed_expression_sha256 | Exact expression sent to the isolated world |
| collector_source_manifest_path | Exact collector-source manifest |
| collector_source_manifest_sha256 | Raw canonical manifest digest |
| qualification_suite | `ag9g-static-dom-chromium-qualification-v15` |
| qualification_manifest_path | Exact qualification source/input manifest |
| qualification_manifest_sha256 | Raw canonical manifest digest |
| isolation_profile | `linux-rootless-user-net-pid-mount-quiescent-proc-v12` |
| target_url | `http://ag9g.invalid/fixture.html` |
| response_status | `200` |
| content_type_header | `text/html; charset=utf-8` |
| decoding_policy | `utf8-no-bom-v1` |
| target_parser_input_context | `static-text-html-utf8-scripting-disabled-v1` |
| resource_network_policy | Existing `offline` policy |
| collection_policy | `ag9g-reviewed-pinned-static-dom` |
| collection_policy_version | `1` |
| invocation_arguments | Exact ordered browser argv, max 16 strings, max 1,024 bytes each |
| fixture_max_bytes | `1048576` |
| artifact_max_bytes | `8388608` |
| protocol_message_max_bytes | `16777216` |
| protocol_total_max_bytes | `67108864` per session |
| outstanding_requests_max | `4` |
| processed_events_max | `4096` |
| diagnostic_max_bytes | `262144` |
| command_timeout_ms | `30000` |
| attempt_timeout_ms | `120000` |

The configuration digest will occupy the existing provenance configuration field
in later stages. It does not reinterpret the historical JSON configuration,
whose bytes and `realCaptureSupport = unsupported` meaning remain unchanged.
Future `VerifiedCaptureSourcesV1` integration must resolve exactly one reviewed
mechanism/version/source/configuration set; arbitrary manifest membership cannot
make a digest recognized. That integration is not Stage 0 work.

V1's launch vocabulary requires `--headless=new`, `--remote-debugging-pipe`,
`--user-data-dir=profile`, `--no-first-run`, `--no-default-browser-check`,
`--disable-extensions`, `--enable-automation`, and
`--enable-features=NetworkServiceSandbox`, followed eventually by the
sole initial URL `about:blank` as the final argument. Optional reviewed flags are
`--disable-background-networking`, `--disable-component-update`, `--disable-sync`,
`--disable-default-apps`, `--metrics-recording-only`, and `--password-store=basic`.
Unknown or duplicated launch flags fail; the supplied order is retained. There
is no generic argument forwarding or sandbox-disabling flag. These flags are
not the network security boundary.

## Source manifests

Format `borrowser-capture-source-manifest-v1`, followed by ordered `[[entries]]`
records with exactly `path`, `sha256`. Maximum 128 entries, 64 KiB manifest,
1 MiB per source, 4 MiB source bytes per manifest validation. Membership and raw
source bytes must match the exact build-reviewed source set in `source_set.rs`,
as well as the source digest. Omitting a capture-core file fails. Paths sort by
ASCII bytes. Manifests exclude themselves, generated evidence, and configuration
bytes independently bound by the configuration digest. No self-referential
digest is introduced. Stage 0 establishes source/build agreement, not clean
committed-source admission authority.

The checked-in source manifests are
`tools/conformance/static-dom-capture-source-manifest-v1.toml` and
`tools/conformance/static-dom-qualification-source-manifest-v1.toml`.
After reviewed source changes, regenerate their per-file raw SHA-256 values,
then update the explicit configuration's manifest digests before requalification.
Membership must match `source_set.rs` exactly.

## `borrowser-chromium-distribution-manifest-v1`

Top-level field order: `format`, `root_mode`, `directories`, `entries`.
Maximum 1 MiB, 1,024 directories and 4,096 file/link entries. Directories exclude
the root and contain exactly `path`, `mode`. File/link records sort by path;
duplicate paths across collections fail. Every parent directory must be listed.

Regular entries have exactly, in order: `path`, `kind = "regular"`, `mode`,
`executable`, `byte_length`, `sha256`, `file_capabilities = "absent"`.
Symlink entries have exactly: `path`, `kind = "symlink"`, `mode`,
`executable = false`, `target`, `target_sha256`, `resolved_path`.

Mode is decimal permission/special bits. Reject setuid/setgid and group/other
write bits on directories/regular files; executable must agree with execute
bits. Require root/current-user ownership and absence of file capabilities.
Symlink mode is exactly 511 (0777); it does not establish target permissions.
The target is the exact ASCII relative link target, at most 1,024 bytes; its
digest covers target bytes, not resolved content. Reject absolute/escaping
targets, cycles, directory links, and chains longer than 16. The resolved target
must be the declared manifested regular file.

Maximum regular file 2 GiB; maximum distribution content 8 GiB. Files stream
through SHA-256, never retained wholesale. Enumerate every supplied directory
through its opened descriptor and reject unmanifested objects. Open each source
with descriptor-relative no-follow traversal. Read symlink targets from the
opened link object. Validate/copy regular files from the same opened object,
check content identity and metadata stability, and verify staged bytes through
the same opened output. Launch only from the private verified snapshot; never
reopen supplied executable paths after verification. Required helper executable
objects come from that snapshot. No root-owned setuid sandbox helper is admitted
under this rootless profile; Chromium's user-namespace sandbox must function.

Snapshot and profile teardown are checked before GO. The distribution manifest
root is recreated inside a separate mode-0700 private directory; its manifested
root permissions never make the private enclosing directory public. Validated
source directory descriptors remain open through copying, including across
pathname replacement. The distribution manifest
does not inventory host dynamic libraries, the loader, or every host policy.
Those remain part of the qualified host environment. No browser installation,
discovery, or downloading occurs in this tool or normal CI.

## `web-observable-dom-tree-v1-isolated-expression`, version 1

The raw inspector must equal the build-reviewed unchanged `.mjs`. Recognize
exactly these five line-start declaration prefixes:

```text
export const algorithm =
export const MAX_BYTES =
export class InspectionFailure
export function inspectWebObservableDomTreeV1(
export function captureWebObservableDomTreeV1(
```

Remove **7 ASCII bytes `export ` at each of 5 sites: 35 removed source bytes**.
Preserve every other byte, including comments/newlines. Prepend exactly these
LF-terminated lines:

```javascript
(() => {
"use strict";
```

Append one leading LF followed by these lines, each LF-terminated:

```javascript
if (document.readyState !== "complete" ||
    document.contentType !== "text/html" ||
    document.characterSet !== "UTF-8") {
  throw new InspectionFailure("capture-document-context");
}
return new TextDecoder("utf-8", { fatal: true })
  .decode(inspectWebObservableDomTreeV1(document));
})()
```

The current exact expression SHA-256 is
`316a83bad2374261833aa9890399f8fae6417742b0350406f452c52cd3936f0b`.
Raw inspector-source hash, packaging-source hash/version, and expression hash
remain distinct. JSON transport encoding must decode back to those expression
bytes. No further stripping, rewriting, minifying, source serialization, or
ordinary-page evaluation is allowed. Inspector semantic changes require an
algorithm-version change; packaging semantic changes require packaging/mechanism
version changes and requalification. Neither changes DOM V1 implicitly.

## `linux-rootless-user-net-pid-mount-quiescent-proc-v12`

Require a non-root, single-threaded collector and supported unprivileged user
namespaces. The fork supervisor creates a user namespace, maps only the caller's
UID/GID preserving nonzero IDs, denies setgroups, and creates network/PID
namespaces. Its PID-namespace init supervises Chromium. Verify the new network
namespace's owner, differing parent identity, UID/GID mappings, PID 1, browser
namespace inheritance, pidfd/start identity, and launched snapshot executable.

The network namespace has only down loopback and no usable external route.
Kernel reject routes are not usable routes. The kernel namespace boundary is the
network boundary, not browser flags or the absence of observed requests.
Chromium gets only standard streams and private CDP descriptors 3/4 across exec;
all other descriptors are CLOEXEC. There is no debugging listener or host socket
forwarding. Profiles are fresh private tmpfs roots created before browser fork. The environment is cleared except the
fixed locale and `TMPDIR=/tmp/ag9g/scratch`. No existing session/profile is accepted.

Post-observation process checks require known snapshot executables, zero main
process effective capabilities, and renderer seccomp/no-new-privileges plus a
distinct sandbox PID namespace. Nested network namespaces must be owned by the
isolated user namespace or its descendants. Unexpected targets fail immediately;
unverifiable live process relationships fail. Observations of the process tree
are checks of the pinned trusted browser, not an exhaustive adversarial syscall
history. Namespace inheritance and lack of ancestor capability enforce closure.

Normal completion requires namespace-init termination (which performs kernel PID-namespace teardown/reaping), outer supervisor wait/reap, and private workspace/snapshot cleanup. Error paths kill the
namespace init through its pidfd, tearing down descendants. No fallback to host
networking, root operation, a listening port, or disabled Chromium sandbox exists.
If this arrangement cannot run the chosen browser, qualification fails.

## `ag9g-chromium-cdp-static-dom-v5`

Use NUL-framed JSON over private pipes, bounded before parsing. Request IDs are
monotonic and session-bound; at most four requests are outstanding. Unknown,
duplicate/stale, or wrong-session acknowledgements fail. Legitimate responses
may arrive out of order only for outstanding requests. Navigation remains
pending while Fetch fulfillment is handled; there is no fulfillment deadlock.

Verify Browser.getVersion product/version/revision/protocol. Create a private
browser context and blank target, enable discovery and paused child auto-attach,
Page/Runtime/Network/lifecycle events, disabled cache, bypassed service workers,
script execution disabled, and Request-stage wildcard Fetch interception. Every
control acknowledgement precedes fixture navigation. The new document must
demonstrate scripting-disabled parser behavior in qualification.

Fulfill only the one expected main-document GET with exact retained UTF-8/no-BOM
bytes, status 200, exact Content-Type, Content-Length, and Cache-Control no-store.
Deny every non-document request. Reject redirects, replacement navigation,
unexpected frames/targets, wrong request/frame/loader IDs, cache/SW responses,
document errors, or missing completion. Require response completion and matching
DOMContentLoaded/load lifecycle events, plus Page.getFrameTree barriers. Never
use sleeps or networkidle as parser evidence.

Create `ag9g-read-only-v1` in the exact completed frame without universal access.
Bind its nondefault execution-context ID and unique ID before evaluating the
exact reviewed expression. Never evaluate in the ordinary world or re-enable
page scripting. Validate returned bytes with the existing external artifact
grammar. Reassert disabled scripting, recheck the exact document, reject realm
replacement, and perform process/sandbox/cleanup checks before success.

## `ag9g-static-dom-chromium-qualification-v15`

Inputs live outside AG discovery in
`tests/contract-vectors/static-dom-capture-qualification-v1/`. Every vector
uses the same core in a fresh isolated browser/profile. Parser-sensitive
noscript assertions distinguish actual parsed nodes from source text. Authored
inline/external scripts, custom-element/event/timer mutation attempts must leave
the sentinel intact and produce no mutation nodes; an image request exercises
resource denial. UTF-8 is tested against a conflicting meta declaration.
Redirect/child-frame inputs must reject; infrastructure failure is not an
expected rejection success. Deterministic input tests reject BOM/invalid UTF-8.

Only completing every vector and teardown produces a mechanism GO diagnostic.
No observation is admitted or stored as registry evidence. A script-transport
unit test, cross-compilation, or unsupported-host rejection is not a real GO.

## Explicit local commands and current limits

```sh
cargo test -p external-browser-capture --features chromium-cdp --locked
node --test tools/conformance/web-observable-dom-tree-v1.test.mjs
cargo run -p conformance-runner --no-default-features --features external-capture --bin conformance-capture --locked -- qualify --purpose mechanism --configuration tools/conformance/static-dom-capture-chromium-linux-v1.config.toml --browser-distribution /explicit/pinned/distribution --output /explicit/outside-repository/new-directory
```

The final command requires a real canonical configuration and distribution;
neither is fabricated by this repository. Admission/capture modes reject as
unavailable. The optional output is a plain mechanism diagnostic, not a new
conformance report or capture-evidence format.

`cargo test --all-features` never runs the ignored real-browser test. Explicitly
running it requires `AG9G_CONFIGURATION`, `AG9G_BROWSER_DISTRIBUTION`, and
`AG9G_COLLECTOR` (the explicit built `conformance-capture` executable), plus a
supported Linux host. It spawns that collector; the multithreaded test harness is
never the collector or qualification identity. Ordinary CI remains browser/network independent. Full CI
and actual Linux/browser qualification remain closeout gates; Stage 0 does not
assert parent/milestone completion or broad browser compatibility.


## Stage 0 review repairs: object consumption and deadline enforcement

The isolation profile is now `linux-rootless-user-net-pid-mount-quiescent-proc-v12`, and the
Chromium capture mechanism version is now `16`. Isolation versions V1–V11 and
capture mechanism versions V1–V12 are rejected. No real V1 qualification or admitted evidence existed; this is an
explicit implementation/identity change, not retroactive reuse of qualification.
The configuration wire format remains V1 with the same ordered fields. The
unchanged inspector and packaging semantics retain their versions; collector
source manifests and the configuration/source digests change.

Distribution preparation retains the verified regular-file objects, including
main/helper executables, as opened read-only descriptors. Supplied or preliminary
staged pathname replacement cannot change those inputs. Before any browser exec,
the rootless supervisor creates a private mount namespace, makes propagation
private, mounts a fresh tmpfs at `/tmp`, and copies/revalidates the retained
objects into `/tmp/ag9g/distribution`. This final tree has no writable host-backed
alias. It is remounted read-only before launch. The enclosing `/tmp` is also
read-only, protecting path ancestors; only explicit profile/scratch submounts
remain writable. There is no mount-policy fallback.

The final main executable remains an opened verified object. The supervisor
uses `execveat(fd, "", ..., AT_EMPTY_PATH)`; argv[0] is only a label, never the
exec authority. The parent receives expected main/helper identities from these
retained final objects before launch, and compares the actual process against
that authority. It never derives the expected executable with `metadata(path)`.
Helper pathnames resolve through the immutable final tree. Browser authority over the collector-owned mount namespace must remain absent;
the V6 zygote exception grants authority only in a descendant user namespace.
Chromium's own sandbox must remain intact.
These mechanisms prevent unreviewed replacement execution rather than accepting
it based on a later mismatch check. Linux behavior still requires real testing.

`FreshProfile` owns a fresh private tmpfs root, created inside the private mount
namespace before any browser process exists. Its parent workspace is retained as
a directory FD. `mkdirat` creates only a mount-point placeholder; a new empty
tmpfs is mounted there before the profile is opened with constrained `openat2`
(`BENEATH`, `NO_SYMLINKS`, `NO_MAGICLINKS`). Substitution of the placeholder cannot
supply preexisting profile contents. The workspace parent is remounted read-only
before acquiring the profile handle; the separate profile tmpfs remains writable.
The supervisor retains that profile object for the attempt. The parent receives
its device/inode and opens the protected browser-visible object, requiring the
same identity. There is no host-backed mutable profile-authority path. The
qualified host/collector is trusted; no browser child exists during mount setup.
Stage 2 can later use this exact identity; no evidence is produced now.

`attempt_timeout_ms = 120000` is the **whole per-browser attempt**, starting
before fresh profile/isolation setup and ending after termination, reaping and
profile/workspace cleanup. CDP receives the same `AttemptDeadline`; it does not
create a new budget. Every command deadline is the minimum of the remaining
attempt and 30 seconds. Handshakes, protocol polls and reap waits use that shared
budget; cleanup receives no new window. Checks surround synchronous work and
copy chunks. A separate pidfd-bound fail-stop watchdog also covers filesystem
or kernel calls that cannot accept a userspace timeout. Expiry can terminate the
collector, never produce GO; unsuccessful emergency termination may leave local
scratch paths requiring removal and is not reported as completed cleanup.
Distribution verification/preliminary staging is one-time pre-attempt work,
separately bounded by the manifest/file/population ceilings. Final namespace
copying/freezing is per-attempt work and consumes the same 120 seconds.

The correlated main response requires status 200, MIME `text/html`, exact
Content-Type, exact retained input Content-Length and `Cache-Control: no-store`.
Missing disk-cache/service-worker booleans are rejected, as are true values.
Exposed prefetch/early-hints flags must be false, and contradictory service-worker
or cache-storage provenance is rejected. Cache-served/early-hints events fail.

`Network.responseReceived` is insufficient by itself for duplicate-header proof.
Completion additionally requires correlated `Network.responseReceivedExtraInfo`
with status 200 and exact required headers. Its documented newline-concatenated
duplicate values and case-variant keys fail exact-value/uniqueness checks. When
`headersText` is present it is checked independently for status, required values,
repeated fields and folded/malformed lines. Missing extra-info support fails
closed: a fulfilled response on a Chromium build that cannot expose this evidence
cannot qualify. No map is claimed to reconstruct discarded duplicate headers.
See the [CDP extra-info contract](https://chromedevtools.github.io/devtools-protocol/tot/Network/#event-responseReceivedExtraInfo)
and [Linux execveat semantics](https://man7.org/linux/man-pages/man2/execveat.2.html).

ASCII symlink targets remain a deliberate V1 distribution restriction and now
have an explicit non-ASCII rejection test. Replacement tests cover supplied main,
preliminary staged main/helper and in-place helper mutation. An explicitly ignored,
browser-free Linux mount regression checks final-tree write/unlink/rename denial;
it requires `/usr/bin/unshare` and is not normal CI or real browser qualification.

## Second Stage 0 repair: quiescent population and attributed rejections

Mechanism version **16**, isolation profile
`linux-rootless-user-net-pid-mount-quiescent-proc-v12`, and qualification suite
`ag9g-static-dom-chromium-qualification-v15` replace their previous identities.
The configuration TOML wire schema remains V1. Qualification corpus filenames
remain in the existing V1 corpus directory; V3 changes the evaluator/lifecycle,
not a comparable DOM format. No previous real qualification exists to preserve.

Namespace PID 1 mounts a private proc view after entering the PID namespace and
before Chromium launch. After observation it enumerates this namespace's process
population, acquires pidfds plus proc-directory/start identities, issues SIGSTOP,
and waits for all threads of every retained live process to be stopped. A second
namespace enumeration must agree with the stopped population. Forks racing the
first enumeration are included in subsequent bounded rounds. Exited pidfds are
never rebound to replacement PIDs. No task-children walk or host-wide discovery is
used. Bounds: 256 processes, 256 stabilization rounds, 4096 threads per process,
1024 proc directory entries, 64 KiB per proc text input, 32 namespace ancestors;
all consume the existing attempt deadline. Failure to stabilize rejects. This is
a trusted-host snapshot, not protection against an external privileged actor
resuming stopped tasks. Namespace PID 1 creates no children during this boundary.

The retained population remains stopped while every live executable, process
start, PID/user/network namespace relationship, capability state and renderer
sandbox is verified. At least one renderer is required. Members and proc handles
remain retained until namespace teardown. The parent verifies its retained
browser/profile bindings after the supervisor reports VERIFIED.

Terminal cleanup deliberately uses SIGKILL on namespace PID 1: resuming stopped
processes for graceful shutdown would invalidate the snapshot. Kernel namespace
teardown kills/reaps the population; the outer supervisor waits for PID 1, and
the collector polls/waits/reaps that owned outer. Early launch errors close the
control channel so the supervisor unwinds while its outer still reaps it. Normal
errors do not rely on destructors. Cleanup failure takes precedence over the
original capture error. If process cleanup cannot be proven, a Cleanup diagnostic
is emitted and the armed watchdog remains until terminal attempt expiry; no
successful error return pretends cleanup completed. Watchdog shutdown itself is
explicitly waited/reaped, with no fresh deadline. Drop is emergency fallback only.

`CaptureOutcome` separates observations from bounded typed policy rejections.
Meta-refresh rejection requires the committed main frame/loader, CDP reason
`metaTagRefresh`, and a bounded destination. Child-frame rejection requires a
committed document and an attachment beneath the exact main frame. The core does
not know fixture filenames. Qualification requires the redirect vector's exact
`http://ag9g.invalid/redirected.html` destination or the child vector's attributed
attachment, respectively. Generic errors, startup failure, stale identities,
wrong sessions and unrelated targets never count as expected negative evidence.

New Linux runtime tests are explicitly separate from macOS deterministic tests:
`isolation::population::runtime_tests::quiescent_namespace_population_runtime`,
`profile::private_runtime_tests::private_profile_old_creation_race`, and
`isolation::linux::cleanup_runtime_tests::watchdog_success_failure_and_terminal_expiry`
are opt-in ignored tests (`cargo test -p external-browser-capture --features
chromium-cdp <name> -- --ignored --test-threads=1`). The first two explicitly use
`/usr/bin/unshare` only as test isolation scaffolding, not collector transport.
Other Linux cleanup/reap tests run in the Linux target's ordinary deterministic
suite. No Linux runtime or real Chromium GO is claimed from cross-compilation.

The additional ignored Linux test
`isolation::linux::attempt_failure_runtime_tests::actual_attempt_failure_paths_reap_owned_children`
drives the actual namespace/FD-launch core with an inert test ELF and verifies
supervisor setup, handshake, protocol-error cleanup and failed post-verification
cleanup. It never implements CDP/DOM output or qualifies a browser. The population
runtime test forces PID reuse through the **test namespace's** `ns_last_pid`;
that test requires permission to write this namespaced sysctl. This is a test-only
prerequisite, not a collector host prerequisite. All Linux runtime tests remain
unexecuted on the current macOS host.


## Browser-visible procfs repair (mechanism 12)

Mechanism version 16 and isolation profile
`linux-rootless-user-net-pid-mount-quiescent-proc-v12` supersede version 6;
qualification suite `ag9g-static-dom-chromium-qualification-v15` must be rerun.
The configuration wire grammar remains V1. No historical real qualification
exists. These changes confer no admission or Stage 1 capability.

Before Chromium exists, namespace PID 1 retains the original `/proc` directory
as a CLOEXEC capability. Bounded mountinfo validation rejects additional procfs
mounts, including subtree bind aliases; hosts with such aliases are unsupported.
The sole inherited `/proc` is covered with a fresh procfs mounted directly at
`/proc` writable with NOSUID, NODEV and NOEXEC. Its effective flags, private `self`
identity, PID 1, distinct filesystem identity and initially singleton numeric
population are checked. There is no `/tmp/ag9g/proc` or other host-proc alias.
The opened private root used for population verification is checked against the
browser-visible root's device/inode. The inherited mount remains covered, with
no reachable pathname in the browser mount namespace.

Host-visible READY/BROWSER identities are read descriptor-relatively from the
retained host procfs. After fork, both supervisor and browser child close their
copies before a bounded exec-release handshake permits execution. This also
removes access through `/proc/1/fd`, beyond merely relying on CLOEXEC. Existing
exec descriptor filtering remains in force. The host collector outside the
private mount namespace retains its existing pidfd/start-identity verification.

The private procfs uses ordinary kernel permission semantics; it is not
remounted read-only. This permits Chromium's own unprivileged user-namespace
sandbox setup without exposing host procfs or delegating ancestor capabilities.
No per-file virtual procfs, UID/GID mapping controller, setuid helper, sandbox
bypass, or writable host-proc alias is introduced.

## V5 private procfs and sandbox qualification

Review of upstream Chromium `sandbox/linux/services/credentials.cc`,
`namespace_utils.cc`, and `namespace_sandbox.cc` confirms setgroups denial and
single-ID UID/GID mapping, and procfs use beyond those writes: thread/capability
inspection, namespace probes and `/proc/self/fdinfo` for the safe-empty-directory
chroot. References: [credentials](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/linux/services/credentials.cc),
[namespace utilities](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/linux/services/namespace_utils.cc),
[namespace sandbox](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/linux/services/namespace_sandbox.cc).
This is an upstream architecture review, not a review or qualification of an
unsupplied pinned binary. Before GO, review the supplied build's exact matching
source revision for additional procfs operations and execute that distribution.

The mount's writable flag confers no additional process, mapping or sysctl
permission. Only the attempt PID namespace is visible. Kernel user-namespace
ownership, mapping write-once rules and capability checks govern writes.
Capabilities in a new descendant user namespace do not grant authority in its
ancestors. Global proc/sys controls are not made namespace-local by this mount;
operations requiring initial/ancestor-user-namespace authority remain forbidden.
Legitimate setup is limited by those kernel rules; a pinned build requiring
ancestor/host authority cannot qualify. User-namespace restrictions imposed by
host policy fail qualification rather than triggering a fallback.

Chromium starts with effective/permitted/inheritable/ambient capabilities cleared.
Post-observation checks require all four sets zero for the main process and
renderers. Other processes follow the closed V7 role policy below. A renderer must additionally have strictly nested PID and
user namespaces (with checked ancestry), NoNewPrivs=1 and seccomp mode 2. Existing
network ownership, executable-object, mode/capability-file rejection and no-setuid
requirements remain in force. Reaching the fixture never substitutes for these
checks. These are final-state invariants, not an exhaustive syscall trace.

Explicit Linux tests cover the private mount and a real child starting with
cleared capabilities, creating a user namespace, denying setgroups and mapping
only its own UID/GID. They check mapping readback, rejected rewrite/ancestor and
global-control writes, absence of host processes and unchanged proc mount identity.
They are kernel mechanism tests, never synthetic Chromium evidence. Runtime
Linux tests cannot be replaced with compilation. Stage 0 remains unqualified
until a supplied pinned Chromium passes the complete mechanism suite.


## V6 namespace-local zygote capability policy

Mechanism 12 / isolation `linux-rootless-user-net-pid-mount-quiescent-proc-v12` /
qualification `ag9g-static-dom-chromium-qualification-v15` replace V6. The V1 wire
schema is unchanged; no historical real evidence exists to preserve.

[Chromium EngageNamespaceSandboxInternal](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/policy/linux/sandbox_linux.cc)
checks that the namespace zygote is PID 1 in a nested PID namespace, enters a new
user namespace, drops filesystem access, then retains CAP_SYS_ADMIN to create
child PID namespaces. [NamespaceSandbox::ForkInNewPidNamespace](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/linux/services/namespace_sandbox.cc)
performs those forks and drops child capabilities when requested. This authority
belongs to Chromium's descendant user namespace, never the collector's namespace
or the host. Upstream HEAD is architectural evidence only: before GO the supplied
build's exact revision must match this frozen rule and pass real qualification.

The bounded stopped-population verifier first validates retained process/start
identity, distribution executable membership and equality to the retained main
Chromium executable object, namespace ancestry and network closure. It then
classifies the bounded flattened process-title evidence defined by V12 below
(64 KiB, 256 tokens after the display label, each at most 4096 bytes; exactly
one `--type=value` for child roles). This is not original invocation argv.
The owned main PID must be present; its title is not read or parsed. Duplicate, unknown,
missing or alternate type encodings fail. Role text alone confers no authority.

Closed roles:

- Main: the owned launch PID, outer user/PID namespaces, all four masks zero.
- Renderer: all masks zero, strictly descendant user/PID namespaces,
  NoNewPrivs=1 and seccomp mode 2.
- Namespace zygote: strictly descendant user AND PID namespaces, namespace-local
  PID 1 verified through NSpid, CapEff=CapPrm=0x0000000000200000 (only
  CAP_SYS_ADMIN), CapInh=CapAmb=0. Zero or additional bits fail this exact role.
- GPU and utility handling is refined by V7 below. No additional capability
  exception is introduced.

Unknown processes, standalone helper objects (including unreviewed crash handlers),
or additional process types cannot qualify under V7. They require an explicit
reviewed/versioned role contract if needed by a future supplied build; a generic
`--type` allowlist or "Chromium may have capabilities" rule is prohibited.
All four capability fields require one exact 16-digit lowercase hexadecimal
value. Missing, malformed or duplicate fields fail. Existing procfs semantics,
quiescence, renderer requirement, sandbox-preserving launch, no-setuid policy,
cleanup and Stage 0 non-claims remain unchanged.


V6 tests cover strict mask parsing, closed roles, extra bits, retained inheritable
or ambient capabilities, outer-namespace zygote spoofing, non-init zygotes and
renderer/unrelated-process SYS_ADMIN rejection. The opt-in Linux test
`sys_admin_is_local_to_descendant_user_namespace` starts capability-free, maps
only its own IDs in a new user namespace, retains exactly SYS_ADMIN, verifies
that entering the ancestor user namespace fails, and successfully creates/reaps
a child in a new PID namespace. It is kernel mechanism evidence only when run,
not real Chromium qualification. macOS cross-checks cannot execute this test.


## V7 dual zygotes and specialized GPU sandbox

Mechanism 12, isolation `linux-rootless-user-net-pid-mount-quiescent-proc-v12`, and
qualification `ag9g-static-dom-chromium-qualification-v15` retain the V1 wire schema.
No real V6 qualification or admitted evidence exists.

Upstream architecture references (HEAD, not qualification of an unsupplied build):

- [Browser initialization](https://raw.githubusercontent.com/chromium/chromium/main/content/app/content_main_runner_impl.cc)
  creates the unsandboxed zygote unless `--no-unsandboxed-zygote` is supplied,
  and also creates the generic zygote.
- [Zygote launch](https://raw.githubusercontent.com/chromium/chromium/main/content/browser/zygote_host/zygote_host_impl_linux.cc)
  uses ordinary process launch for `--no-zygote-sandbox`, inheriting the browser's
  user/PID/network namespaces; the generic zygote uses namespace-sandbox launch.
- [GPU launch](https://raw.githubusercontent.com/chromium/chromium/main/content/browser/gpu/gpu_process_host.cc)
  selects the unsandboxed zygote so GPU-specific sandboxing can occur after fork.
- [GPU initialization](https://raw.githubusercontent.com/chromium/chromium/main/content/gpu/gpu_main.cc),
  [sandbox options](https://chromium.googlesource.com/chromium/src/+/HEAD/sandbox/policy/linux/sandbox_linux.h),
  and the [GPU hook](https://raw.githubusercontent.com/chromium/chromium/main/content/common/gpu_pre_sandbox_hook_linux.cc)
  use specialized seccomp sandboxing. Namespace engagement defaults false and
  the GPU hook explicitly leaves it disabled. GPU therefore retains outer
  user/PID namespaces; renderer namespace requirements cannot be copied to it.

The namespace zygote retains its exact V6 authority, and must NOT carry
`--no-zygote-sandbox`. The separate UnsandboxedZygote role requires exactly one
bare `--no-zygote-sandbox`, exactly one `--type=zygote`, the main executable
object, outer user/PID/network namespaces, all four capability sets zero and
PPid equal to the retained main process. This is a Chromium bootstrap role only.
The host procfs remains inaccessible despite the role's Chromium name.

Every live GPU must have the same verified executable, outer user/PID/network
namespaces, zero capability sets, unique NoNewPrivs=1 and Seccomp=2 fields, and
a live verified UnsandboxedZygote parent in the stopped population. The selected
headless profile must demonstrate at least one such bootstrap and GPU, as well
as the existing required renderer. Parent relations use the same retained pidfd,
start-identity and proc-object population; disappearance/reparenting fails closed.
The bounded per-role vectors cannot exceed the existing 256-process ceiling.
These checks establish stable necessary sandbox evidence, not the contents of a
seccomp filter or an exhaustive syscall history. GO still requires review and
execution of the exact supplied Chromium source/build revision against this rule.

Historical V7 policy (superseded by the Network Service and V12 broker sections below):
Utility was recognized but had no admitted sandbox classes.
No pinned fixture run has supplied reviewed utility-class evidence. All utility
processes therefore fail closed until actually observed, reviewed classes receive
a separate versioned policy. Unknown types, helpers and broker types remain
unreviewed; V7 does not silently broaden their permissions or invent renderer
requirements for utility classes that intentionally use different sandboxes.

Role parsing rejects duplicate/value-bearing bootstrap switches and any bootstrap
switch on a non-zygote role. Presence (including `=false`) of these switches fails:
`--no-sandbox`, `--disable-namespace-sandbox`, `--disable-seccomp-filter-sandbox`,
`--disable-gpu-sandbox`, `--disable-setuid-sandbox`, `--no-zygote`,
`--no-unsandboxed-zygote`, `--single-process`, `--in-process-gpu`,
`--allow-sandbox-debugging`, and `--gpu-sandbox-failures-fatal`. No launch arguments
are added to suppress either zygote. Executable/namespace/capability identity is
required in addition to all command-line checks.

Tests cover dual-role attribution, conflicting switches, forbidden flags,
namespace/capability mismatches, separate GPU/renderer predicates, utility
rejection and bounded unique PPid parsing. An opt-in Linux test observes a real
capability-cleared shell fixture through executable/proc/pidfd objects and checks
bootstrap metadata in inherited namespaces; this is process-policy mechanism
testing, never a Chromium capture or qualification. On macOS it is only compiled.
All prior Stage 0 isolation, cleanup and capture invariants remain in force.


## V9 Network Service utility and controlled transaction

Current configuration identity is mechanism **12**, isolation
`linux-rootless-user-net-pid-mount-quiescent-proc-v12`, qualification suite
`ag9g-static-dom-chromium-qualification-v15`. The V1 wire grammar is unchanged.
V9 preserves NetworkServiceUtility as the sole reviewed utility and explicitly
selects its kNetwork sandbox.
No real V8 qualification exists. V7's other roles, especially GPU, are unchanged.

Reviewed upstream architecture (HEAD is not a qualification of a supplied build):

- [Network Service architecture](https://raw.githubusercontent.com/chromium/chromium/main/services/network/README.md)
  prefers an out-of-process dedicated utility on desktop and documents restarting
  after a crash. `GetNetworkService()` maintains one observed service remote.
- [NetworkService interface](https://raw.githubusercontent.com/chromium/chromium/main/services/network/public/mojom/network_service.mojom)
  declares `ServiceSandbox=sandbox.mojom.Sandbox.kNetwork`.
- [Service launch](https://raw.githubusercontent.com/chromium/chromium/main/content/browser/service_host/service_process_host_impl.cc)
  supplies the interface name as metrics/subtype identity. The utility launcher
  emits `--type=utility` and `--utility-sub-type`; the
  [sandbox mapping](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/policy/sandbox_type.cc)
  emits the exact `--service-sandbox-type=network` value for kNetwork. A build
  selecting `none` is not normalized into this reviewed role.
- [Utility launcher delegate](https://raw.githubusercontent.com/chromium/chromium/main/content/browser/service_host/utility_sandbox_delegate.cc)
  explicitly returns no zygote for kNetwork: its parent is the main browser.
- [Utility initialization](https://raw.githubusercontent.com/chromium/chromium/main/content/utility/utility_main.cc)
  uses default sandbox options and the
  [network pre-sandbox hook](https://raw.githubusercontent.com/chromium/chromium/main/services/network/network_sandbox_hook_linux.cc).
  This hook does not engage a new user/PID namespace. It starts a syscall broker;
  V12 below adds only this broker and the required GPU broker, with relational verification.

Exact closed role: one `--type=utility`, one
`--utility-sub-type=network.mojom.NetworkService`, one
`--service-sandbox-type=network`. Duplicate, empty, missing, separated or conflicting
subtype/sandbox switches fail. These switches on other roles fail. Existing global
sandbox-disable rejection remains. No generic Utility success path exists.

The stopped-population verifier first requires the retained main executable object
and existing pidfd/start/proc-object binding. NetworkServiceUtility then requires
main-browser PPid, identical outer user/PID/network namespace objects, four zero
capability masks, unique NoNewPrivs=1 and Seccomp=2. It never acquires zygote
CAP_SYS_ADMIN or renderer namespace authority. The independent kNetwork predicate
is deliberately separate from GPU even though their scalar seccomp/NNP evidence
currently agrees. This is necessary evidence, not proof of the seccomp filter's
contents. Outer AG9g network closure and private procfs remain mandatory.


The V9 launch vocabulary requires exactly one literal
`--enable-features=NetworkServiceSandbox` argument. It is serialized in the existing
ordered invocation arguments and covered by capture_configuration_sha256. Missing,
duplicate, combined/arbitrary feature lists, trial-qualified values, conflicting
`--disable-features`, and in-process selectors fail canonical configuration
validation. No new configuration field or capture identity grammar is introduced.
If the supplied pinned revision does not support this exact feature or falls back
to `--service-sandbox-type=none`, qualification fails. Role text is checked alongside
retained executable, pidfd, parent, namespace, capability, NoNewPrivs and seccomp
state; the feature flag alone is never sandbox proof.

The final quiescent population requires exactly one live reviewed Network Service.
Zero, simultaneous duplicates, or any unreviewed utility/sandbox state fails.
This establishes current population validity, not historical process continuity.
Chromium may internally replace a Network Service process outside the controlled
transaction. That alone is not a DOM-identity change and no historical service PID
is recorded in the comparable observation or external capture identity. The
supplied pinned revision/profile must still confirm these exact assumptions.

The required continuity is one controlled document transaction: the synthetic URL,
main frame, one requestWillBeSent, one fulfilled document request with exact retained
bytes, correlated network/loader identities, one validated response and ExtraInfo,
status 200, exact MIME/headers, no cache/service-worker/redirect contradictions,
loadingFinished, committed main frame, DOMContentLoaded/load, frame-tree barriers,
and unchanged isolated-world document through post-observation verification.
Duplicate request/response/ExtraInfo events fail even if IDs match. Replacement IDs,
loaders, navigation, missing ExtraInfo, crashes and controlled loading failures
fail through typed capture errors. A controlled loading failure or repeated finish
during/after isolated-world inspection also fails. Denied ancillary-resource
failures do not become controlled-document failures merely by occurring late.

A restart that breaks any of these facts is not treated as recovery. A transparent
internal restart can remain irrelevant only while the transaction and final live
sandbox population remain valid. Restart/fallback into kNoSandbox always fails.
There is no uninterrupted-service-PID claim and no kernel-wide lifecycle monitor.
The V8 unconditional lifecycle error/gate is removed; successful mechanism results
still require actual browser execution, all vectors, process verification and
bounded cleanup. Stage 2 A/B repeatability remains unavailable and is not replaced
by these scripted transaction tests.

Tests cover exact feature selection, configuration digest participation, closed
subtype/sandbox attribution, live singleton cardinality, wrong executable,
namespace/capability mismatch, duplicate or replaced transaction identities,
response/ExtraInfo replacement, missing completion, crash/failure during capture
and inspection, and absence of an unconditional lifetime rejection. Identical
scripted transactions demonstrate only controller semantics, never a genuine
browser restart or A/B collection. Linux population tests are cross-compiled on
macOS, not executed. No registry, admission, baseline, trend or Stage 1 behavior
is introduced. Real mechanism qualification remains NOT ESTABLISHED.


## V12 owned Main and child process-title attribution and required syscall brokers

V12 established isolation `linux-rootless-user-net-pid-mount-quiescent-proc-v12`
and the owned-Main rules below. V13 preserves these rules while superseding the
capture mechanism and qualification identities, as specified in the V13 section.
The configuration wire grammar stays V1. No real qualification or admitted
capture exists for a superseded mechanism. V9 feature selection and controlled
transaction continuity, V10 broker relationships, and all existing isolation,
object identity, cleanup and sandbox checks remain required.

### Observable and invocation are different types of evidence

The identity-bearing ordered `invocation_arguments` are Borrowser-controlled
exec inputs. They are not reconstructed from procfs. `ChromiumProcessTitle`
borrows bounded bytes read from the retained process's `cmdline` proc object.
That observable is mutable attribution metadata, never an executable authority
or a proof of original argv. It is used only for child attribution. Main is
selected directly by equality to the retained launch PID, after the existing
pidfd/start/stopped-object checks. `verify_main` additionally requires the retained
main executable object, exact outer user/PID/network namespace objects and zero
CapEff/CapPrm/CapInh/CapAmb. Its title is neither read nor parsed. No child title
can produce Main; `chromium_child_role` has no Main success branch.

The frozen invocation still includes final positional `about:blank`. That is an
identity-bearing exec input supplied by Borrowser, intentionally outside the
child-title grammar. There is no Main-title projection or inferred original argv.
A deferred child-title read makes the absence of a Main title dependency testable.

The frozen parser accepts one nonempty ASCII display label followed by zero or
more switch-shaped tokens, separated by exactly one ASCII space, terminated by
one or more NUL bytes. Remaining bytes after the first NUL must all be NUL
padding. Maximum observable size is 65,536 bytes; maximum label/token size is
4,096 bytes; at most 256 tokens follow the label. Empty tokens, interior NULs,
non-ASCII/control bytes, quotes, backslashes, positional tails and the bare `--`
separator fail. No shell splitting, escaping or argv reconstruction occurs.
Only exact complete role-affecting tokens are interpreted; a role-looking
substring within an unrelated key/value does not match. Duplicates/conflicts
and prohibited sandbox switches fail as before. The display label is never
used as executable identity.

Flattening cannot recover arbitrary original argument boundaries. V12 preserves the V11 child grammar and supports
only the unambiguous token vocabulary needed by the frozen profile; it rejects
space-containing/quoted positional forms rather than guessing. Even a valid
title can be forged by a process and is only one input to the independent
object, namespace, capability, sandbox and relationship verification. The pinned
build must confirm that its generated titles fit this restricted contract.

### Two-phase closed population

Phase one validates every retained stopped process's pidfd/start identity,
executable object, namespace objects and sandbox state. Main uses launch ownership
as above; child-title attribution yields `PreliminaryRole::Process` for the five
reviewed non-broker child roles, or
`BrokerCandidate` for exactly `<display-label> --type=broker` with NUL termination.
No utility subtype/service-sandbox metadata or other tokens are permitted on the
broker candidate. Neither `utility-broker` nor `gpu-process-broker` is supported.

Candidates require the retained main executable, exact outer user/PID/network
namespaces, all four capability sets zero, unique `NoNewPrivs=1` and unique
`Seccomp=2`. Those checks confer no client or zygote authority. They establish
necessary observable sandbox state, not the contents of BrokerProcessPolicy BPF.

Phase two resolves each candidate using measured PPid in the same retained
stopped population, only after all individual checks have succeeded:

| Completely validated live parent | Resolved semantic role |
| --- | --- |
| NetworkServiceUtility | NetworkServiceBroker |
| Gpu | GpuBroker |

Each such client requires exactly one matching broker. Main, zygote, renderer,
broker, missing or invalid parents fail; missing/duplicate brokers fail. Semantic
broker roles cannot be injected as preliminary roles. The resolved map contains
no candidates. The deterministic PID ordering and 256-process bound remain.
All other utility classes, helpers and unknown processes fail. This is not
support for arbitrary generic brokers: generic title text alone confers nothing.

### Reviewed Chromium source behavior

Chromium's internal `base::CommandLine` is structured. Its
[SetProcessTitleFromCommandLine](https://chromium.googlesource.com/chromium/src/+/HEAD/base/process/set_process_title.cc)
joins arguments with spaces, using the executable symlink as a display label,
and invokes [setproctitle](https://chromium.googlesource.com/chromium/src/+/HEAD/base/process/set_process_title_linux.cc)
to rewrite the original argv-memory region exposed by Linux proc cmdline. The
zygote also updates that title after installing child CommandLine state.

The [common broker callback](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/policy/linux/sandbox_linux.cc)
uses `GetArgs()`, resets CommandLine, initializes from that result, sets the
broker type and updates the title. [GetArgs](https://raw.githubusercontent.com/chromium/chromium/main/base/command_line.cc)
returns non-switch arguments. In the reviewed common path, parent process-type,
utility-subtype and service-sandbox switches are therefore discarded; the
resulting process type is generic `broker`. V10's client-specific metadata rule
was incorrect for this path, not merely unproven by HEAD.

The [Network Service hook](https://raw.githubusercontent.com/chromium/chromium/main/services/network/network_sandbox_hook_linux.cc)
and [GPU hook](https://raw.githubusercontent.com/chromium/chromium/main/content/common/gpu_pre_sandbox_hook_linux.cc)
each start a dedicated broker. [BrokerProcess::Fork](https://raw.githubusercontent.com/chromium/chromium/main/sandbox/linux/syscall_broker/broker_process.cc)
creates one broker with inherited namespace objects per initialized client;
BrokerProcessPolicy protects the broker while it services filesystem requests.
That supports the existing one-to-one relationship and distinct client policies.
HEAD is architecture evidence only. The supplied immutable Chromium build/source
revision must confirm these exact title, fork and sandbox behaviors before GO.

Deterministic tests cover flattened titles, malformed/ambiguous observables,
duplicate/prohibited tokens, substring non-matches, generic candidates, closed
parent resolution and sandbox failures that title text cannot override. The
opt-in Linux population regression rejects an actual shell's original exec argv
layout instead of interpreting it as a Chromium title. An additional opt-in test
rewrites a fork child’s kernel-reported argv-memory region and checks that procfs
then exposes the flattened title, leaving the parent unchanged. These are kernel tests,
not browser qualification. Linux cross-compilation is not runtime evidence.
Real mechanism qualification remains **NOT ESTABLISHED**; Stage 1 cannot begin.


V12 regression coverage uses the complete configuration specimen with final
`about:blank`, proves the Main path never calls its deferred child-title reader,
and rejects Main PID/executable/namespace/capability mismatches. A Main-looking
child and every child positional tail still fail. Existing broker graph and
child sandbox rules are unchanged. No real V11 evidence exists to preserve;
mechanism 12 and qualification suite V11 require genuine pinned-Linux execution
before GO. These changes implement no Stage 1/2/3 functionality.


## Shared CDP event scope and startup population (V14)

Capture mechanism 16 and qualification suite
`ag9g-static-dom-chromium-qualification-v15` supersede mechanism 15. The CDP
semantic adapter contract is now `ag9g-chromium-cdp-static-dom-v5`: it identifies
reviewed event validation semantics, not just a protocol family. Configuration
wire grammar remains V1. Linux isolation remains
`linux-rootless-user-net-pid-mount-quiescent-proc-v12`; no namespace, process-role,
capability, distribution or profile semantics change.

After acknowledging Target discovery and before creating its private context/target,
the collector calls Target.getTargets
and requires exactly one unattached page at about:blank. It retains that exact
TargetId and its optional context identity. Only it and the distinct target
returned by Target.createTarget belong to startup. Root Target creation/info and
explicit attachment events must agree with those IDs and contexts; additional
blank pages are not equivalent. Destruction/detachment and unknown Target methods
fail. After startup no new attachment or target creation is permitted.

One EventBoundary owns event-envelope scope throughout preparation, navigation,
inspection and final drains. Page/Runtime/Network/Fetch/Inspector events require
a non-null string sessionId equal to the retained flattened attached session.
Root Target events have their own method-specific identity checks and cannot
carry a session envelope. Command response binding remains Protocol's separate
responsibility. Startup admits only the selected blank frame/loader's bounded
lifecycle/default-context notifications; unexpected events fail.

Inspector.enable is acknowledged before navigation. Inspector.detached,
Inspector.targetCrashed and Inspector.targetReloadedAfterCrash always fail;
Root Target.targetCrashed also fails, independently of Inspector signals. References:
[Inspector protocol](https://chromedevtools.github.io/devtools-protocol/tot/Inspector/)
and [Target protocol](https://chromedevtools.github.io/devtools-protocol/tot/Target/).
Pinned protocol/build qualification remains mandatory.

Controlled request collisions are checked before resource-type dispatch. A
retained network/request ID cannot turn into XHR/Image or change frame/loader;
a Fetch networkId collision cannot be denied as an unrelated resource. Existing
exact response/ExtraInfo/header/freshness and duplicate-completion checks remain.
Page lifecycle loader/frame contradictions fail for every lifecycle name.
Genuinely distinct ancillary requests remain denied. Preparation errors remain
errors and never become successful negative-vector policy rejections.

The attached session/transport now remains owned through final stopped-population
verification. Before termination, the collector drains already-emitted events
nonblocking through the same boundary, under the existing deadline/budgets.
Incomplete buffered framing, EOF, fatal lifecycle or contradictory document
notifications fail. No command is sent to stopped Chromium. Cleanup still runs
and cleanup failure retains precedence. This is event accounting through the
existing quiescent boundary, not a change to Linux isolation. It does not claim
a protocol can report an internal event that Chromium never emitted.

Mechanism qualification remains NOT ESTABLISHED. No browser was supplied, no
Linux runtime qualification is inferred, and Stages 1–3 remain unavailable.

## V14 discovery and navigation correlation

V14 introduced discovery reconciliation and controlled-navigation correlation.
V15 below preserves those rules and supersedes the startup loader equality rule.
The current contract is CDP V5; configuration wire V1 and Linux isolation
`linux-rootless-user-net-pid-mount-quiescent-proc-v12` remain unchanged.

Target discovery is acknowledged before the first authoritative inventory.
The bounded Protocol queue retains every notification during inventory,
context creation and attachment. The root EventBoundary ledger reconciles these
against exactly the initial command-line page ID and the private page ID/context,
plus the exact attached session. Creation and attachment notifications may arrive
on either side of command responses. They must all be observed before the second
inventory and final frame-tree barrier. Inventories cannot erase transient unknown
targets, destruction, crashes, detachment or contradictory metadata. Both pages
must remain present; the private page must be attached. No quiet-period heuristic
is used. Discovery remains active through the final quiescent event drain.

Attached Page.frameStartedNavigating events use the same session boundary.
Startup accepts only the retained private frame, about:blank and differentDocument;
the pending token is separate from the committed frame-tree loader (V15 below). Controlled starts require the synthetic fixture URL,
main frame and differentDocument. DocumentState owns one loader identity shared
by start events, Page.navigate acknowledgement, network request and frame commit.
Either response/event ordering is accepted. At most 64 identical semantic starts
are permitted per phase; inconsistent starts fail. A controlled start is required
before completion. Inspection and post-observation starts fail, including same-URL
starts. Start events never supply meta-refresh or child-frame rejection authority.

These are scripted mechanism contracts, not real browser qualification.
Real mechanism qualification remains NOT ESTABLISHED; Stage 1 must not begin.

Protocol references: [navigation starts](https://chromedevtools.github.io/devtools-protocol/tot/Page/#event-frameStartedNavigating) and [Target discovery](https://chromedevtools.github.io/devtools-protocol/tot/Target/#method-setDiscoverTargets). These describe CDP semantics, not qualification of a supplied build.

## V15 startup committed and pending navigation identities

V15 introduced the committed/pending separation below. V16 preserves it while
superseding startup lifecycle sequencing and the mechanism/qualification/CDP
identities, as specified in the V16 section.
Linux isolation remains V12 and the configuration wire grammar remains V1.

The first private frame tree establishes the committed blank-document loader.
Startup state separately retains at most one pending navigation token. Identical
bounded starts are accepted before commit; a different token or a start after
commit fails. An exact main-frame, parentless, about:blank frameNavigated event
must commit that pending token. Duplicate or unsolicited commits fail.

Before commit, lifecycle notifications may reference the initial committed loader
or the one pending loader; after commit only the new committed loader is accepted.
Neither lifecycle notifications nor unscoped load notifications become fixture
completion evidence. Default Runtime contexts must still belong to the exact
main frame. Unknown startup events and every child frame remain rejected.

Queued events are replayed independently of frame-tree response arrival. Thus
start B / first tree A / commit B and start B / commit B / first tree B converge
to the same state. The final tree must match the resulting committed blank loader,
with no unresolved pending token or child frame. A third loader or an old tree
after a proven commit fails. No sleep or idle heuristic is introduced. Only then
can the independent controlled fixture DocumentState begin.

Chromium's [PageHandler::Enable](https://github.com/chromium/chromium/blob/main/content/browser/devtools/protocol/page_handler.cc)
reports an already-pending NavigationRequest using its devtools_navigation_token.
That pending token need not equal the currently committed frame-tree loader.
[Page.frameNavigated](https://chromedevtools.github.io/devtools-protocol/tot/Page/#event-frameNavigated)
instead describes the frame after navigation completion and its committed loader.
The supplied pinned revision must still confirm this V15 behavior during real
qualification. No genuine mechanism qualification or admitted evidence exists.

## V16 lifecycle replay after the committed baseline

Mechanism 16, qualification suite `ag9g-static-dom-chromium-qualification-v15`
and CDP semantic contract `ag9g-chromium-cdp-static-dom-v5` supersede V15.
Linux isolation V12 and configuration wire V1 remain unchanged.

Page.enable remains early so an already-pending navigation is observed.
Inspector, Page, Runtime and Network enablement and the existing cache,
service-worker, scripting and Fetch controls precede the first frame tree.
That tree must establish a parentless private main frame, about:blank, no children
and a committed loader baseline. Only then is lifecycle reporting enabled and
acknowledged, before startup event reconciliation, the second Target inventory
and final frame-tree barrier. Controlled Page.navigate follows convergence.

Blink's
[InspectorPageAgent::setLifecycleEventsEnabled](https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/inspector/inspector_page_agent.cc)
immediately emits already-observed phases for the current DocumentLoader,
including applicable commit, DOMContentLoaded, load, networkAlmostIdle and
networkIdle. Replay can arrive before the command acknowledgement. Establishing
the baseline first prevents old replay from being mistaken for an unknown loader
after a newer first snapshot.

StartupDocument remains unchanged. Stable A/replay A, pending B with first tree
A/replay A/commit B, early commit B/first tree B/replay B, and first tree A/commit
B/replay B all converge. Third-loader replay, old A after B commits, unresolved B
and lifecycle-enable failure abort preparation. No historical-loader allowlist
is introduced. Replay is startup consistency evidence only; none of its phases
contributes to controlled fixture completion.

The supplied pinned Chromium revision must still confirm the complete V16
startup behavior. Real mechanism qualification remains NOT ESTABLISHED.


## Mechanism 17: dedicated collector, retained environment, bounded workload

Mechanism 17 supersedes mechanism 16 for the production transaction/lifetime
contract. Qualification suite `ag9g-static-dom-chromium-qualification-v15` remains
unchanged: its vector assertions, correlated negative outcomes and GO acceptance
requirements have not changed. CDP V5, isolation V12, inspector/packaging V1 and
configuration/source-manifest wire V1 are unchanged. Every changed trust-bearing
source byte nevertheless changes reviewed source digests. Previous source or
executable qualification cannot apply to this revision.

### Process boundary and ownership

Real capture executes only inside the dedicated single-threaded
`conformance-capture` collector. Linux prerequisites reject root or multithreaded
collectors before fork-based setup. Deadline watchdog and unprovable terminal
cleanup may fail-stop the collector. This process boundary is part of the capture
mechanism, not a replaceable CLI packaging choice. The public
`qualification::mechanism` entry exists solely for that binary; it must not be
called from an aggregate process or the multithreaded Rust test harness. The
ignored real-browser test continues to execute the dedicated binary.

Only qualification and CLI error/result types are externally exposed.
Configuration/source loading, packaging, distribution, transaction, deadlines,
profiles, isolation and Chromium/CDP/session internals are private modules.
Completed workloads have private construction. No public resource-owning collector,
caller-managed finish, capture service, callback or browser handle is provided.

`transaction::capture_static_dom_workload` accepts a configuration and exact
immutable byte slices. It preflights the entire input population, validates host
and collector sources, loads the reviewed inspector expression, and prepares ONE
`VerifiedDistribution`. This retained environment survives the complete workload.
Each input borrows its already-verified distribution objects; supplied distribution
files are not independently re-read, re-hashed or re-snapshotted per input.
The existing per-launch private freeze/mount of retained objects is preserved.

Each `capture_attempt` creates a fresh deadline, watchdog, isolated process tree,
writable profile, browser context, target, transport and session. No mutable
browser state survives an attempt. Exact byte delivery, scripting-disabled parsing,
resource denial, correlated completion/realm checks, late-event verification,
stopped-population/sandbox verification and termination/reaping remain mandatory.
The 120-second attempt budget begins before per-browser resources and ends after
attempt cleanup/watchdog completion; no phase renews it. Shared distribution
preparation/disposal remain outside that per-browser budget, with existing
manifest/file/population resource bounds. This is not a new whole-workload
wall-clock deadline.

Qualification alone owns its corpus verification, qualification source membership,
vector expectations, negative-vector assertions, collector executable identity,
and final GO formatting. It submits the five HTML inputs as one workload and
checks outcomes only after shared disposal. The reusable transaction knows no
qualification filenames, expectations, AG identities or publication identity.

### Workload resources

These mechanism constants are independent of corpus membership; no configuration
wire fields were added:

| Resource | Maximum |
| --- | --- |
| Inputs per workload | 16, nonempty |
| Fixture bytes per input | 1,048,576 (existing bound) |
| Total fixture bytes | 16,777,216 |
| Observation artifact per input | 8,388,608 (existing bound) |
| Retained outcome payload across workload | 33,554,432 |

Sixteen inputs caps sequential process launches and cumulative attempt work.
Sixteen MiB bounds input population. The separate 32 MiB retained-output ceiling
prevents accumulating sixteen maximum-sized DOM artifacts. These are mechanism
resource choices, not counts derived from the current five qualification vectors.
Observation document/realm strings and all policy-rejection string data also count
against retained output payload. Fixed outcome metadata is bounded by input count.
The current provisional outcome still has the existing per-attempt limits before
retention accounting; these ceilings are not an exact total-process RSS bound.

All cumulative accounting uses checked arithmetic. Input count, size, total and
encoding errors reject before environment acquisition. Result slots use fallible
reservation (`CaptureError::Allocation`). Per-artifact and accumulated payload
limits are checked before appending a provisional outcome. Failure discards all
accumulated outcomes and performs applicable checked cleanup. There is no truncation,
splitting, retry, omission or partial publication.

### Terminal outcomes and cleanup

A correlated policy rejection remains distinct from infrastructure failure and
must complete the same terminal verification path as an observation. Infrastructure
failure stops further attempts. No provisional outcome escapes until all attempt
checks and final shared distribution disposal succeed.

Returned-error precedence, highest first: outer distribution disposal, watchdog
completion, browser terminal verification/cleanup, capture/preparation/resource
failure. Within browser finalization, cleanup still overrides verification failure.
All applicable terminal operations execute before choosing the returned error.
Explicit outer disposal now also runs after returned workload errors; it cannot be
skipped by an early `?` from the attempt loop.

Checked cleanup begins at partial acquisition, not only after environment or
browser preparation succeeds. Snapshot staging owns its private directory until
ownership transfers exactly once into `VerifiedDistribution`. On returned errors
it restores owner access through retained handles for created directories before
explicitly removing the staging tree, including after manifest modes were applied.
Pre-process launch preparation explicitly removes its workspace on socket,
namespace or fork failure. Once fork succeeds, the existing supervisor/pidfd
cleanup and fail-stop lifecycle remains authoritative. In either partial scope,
cleanup failure returns `CaptureError::Cleanup` over the original operation error;
successful cleanup preserves that original error. Destructor cleanup is emergency
fallback only. These corrections implement mechanism 17's existing semantics;
qualification-suite and other semantic identities remain unchanged. Changed
source bytes require refreshed source identities and a new real qualification.

An internal unwind boundary retains the shared environment long enough to attempt
explicit outer disposal and then resumes the panic. It does not convert panics
into ordinary capture errors. Attempt `Drop` guards remain emergency fallback;
panic, abort, fail-stop or abnormal exit never proves checked finalization and
never creates a completed workload or GO. Fail-stop may preclude later disposal
and leave scratch resources; this remains unsuccessful collector termination.

### Qualification and subsequent stages

Deterministic workload tests, scripted protocol peers, private API compile-fail
checks and Linux object-retention tests are implementation evidence only. They
cannot establish mechanism GO. Freeze/review this corrected source and manifest
set, bind the actual configuration/distribution/executable, then run every vector
through the dedicated collector on supported Linux with explicitly pinned Chromium.
Any trust-bearing repair requires updated identities and a complete rerun.

Future AG9g1 must transmit the retained validated AG fixture bytes over a bounded
controlled pipe/IPC to this dedicated process, without fixture-path rediscovery or
reopen. The future collector mode will call this same internal workload/attempt.
The aggregate parent must classify collector crash, timeout, fail-stop or abnormal
exit as advisory failure and continue ordinary reporting. No IPC, candidate mode,
AG handoff, publication or admission is implemented here. Real mechanism
qualification remains **NOT ESTABLISHED**; AG9g0 remains open and AG9g1 blocked.
