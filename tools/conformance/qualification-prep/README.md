# AG9g0 qualification preparation

**qualification-prep is a producer, not an authority.** Its outputs are candidate
inputs. It cannot produce mechanism GO, capture a DOM, run qualification vectors,
or admit evidence. The independent frozen collector verifies every real input.

Collector source remains `04d22c360c34c891a66400f31afcf3c4a4146973`.
This package is a separate Cargo workspace with its own lockfile. Do not add it
to the root workspace or depend on Borrowser crates. Build in a separate target
directory, using Rust 1.92.0 and pre-provisioned dependencies:

```sh
CARGO_TARGET_DIR=/absolute/local/prep-build cargo +1.92.0 build --offline --locked --manifest-path tools/conformance/qualification-prep/Cargo.toml
CARGO_TARGET_DIR=/absolute/local/prep-build cargo +1.92.0 test --offline --locked --manifest-path tools/conformance/qualification-prep/Cargo.toml
CARGO_TARGET_DIR=/absolute/local/prep-build cargo +1.92.0 clippy --offline --locked --all-targets --manifest-path tools/conformance/qualification-prep/Cargo.toml -- -D warnings
cargo +1.92.0 fmt --manifest-path tools/conformance/qualification-prep/Cargo.toml -- --check
```

Use `rustup run 1.92.0 cargo` instead of `cargo +1.92.0` if invoking a Cargo binary
that is not the rustup proxy; ensure its invoked rustc is also 1.92.0. Record the
helper source revision/diff, independent lockfile digest, compiler/target/build
command and helper executable SHA-256. The helper's build is never the collector
build. Root `make ci` does not discover this independent workspace.

## Commands

All paths shown are placeholders. Output paths must be explicit absolute paths
whose parent already exists. Commands never overwrite an existing output.

```sh
qualification-prep manifest --distribution-root /explicit/frozen/tree --output /explicit/new/distribution.toml
qualification-prep identity --distribution-root /explicit/frozen/tree --distribution-manifest /explicit/distribution.toml --executable chrome --unshare /usr/bin/unshare --unshare-sha256 ACTUAL_SHA256 --unshare-version 'ACTUAL COMPLETE VERSION LINE' --output /explicit/new/browser.json
qualification-prep record --kind host --input /explicit/authored-host.json --output /explicit/new/host.json
qualification-prep record --kind pin --input /explicit/authored-pin.json --output /explicit/new/pin.json
qualification-prep configuration --collector-source-root /explicit/clean-04d22c36 --reviewed-pin /explicit/pin.json --distribution-manifest /explicit/distribution.toml --browser-identity /explicit/browser.json --host-record /explicit/host.json --output /explicit/new/config.toml
```

`record` only validates/serializes explicitly authored preparation metadata. Host
records additionally must agree with the executing Linux host. It does not select
an image, release, publisher or optional argument. Authored record JSON can have
ordinary whitespace/order; persisted records are compact ordered JSON with one
final LF. Record readers require that canonical representation. Browser records
are produced only by the bounded identity probe, not the `record` command.

`manifest` runs only on Linux because capability absence and same-object symlink
reads require Linux filesystem interfaces. The identity command additionally
requires native x86-64, a non-root single-threaded process and the documented
namespace host. Pure format tests run on other hosts. Linux filesystem/process
unit tests on ARM are not ARM Chromium support.

`__probe-worker` is the implementation's private re-exec entry. It requires actual
namespace PID 1, exact maps, offline networking, private procfs, zero effective
capabilities and frozen read-only storage. It is not a public operation or
alternative to the outer probe and its checked cleanup. Its stdout is provisional internal tuple transport, not an accepted browser record; callers must use `identity` for publication.

## Preparation-only records

All three schemas reject unknown or duplicate fields. Raw/persisted input is at
most 65,536 bytes. There are no unbounded record collections. Strings identifying
versions/platforms/provenance are 1–128 UTF-8 bytes, trimmed without controls;
SHA-256 values are 64 lowercase hex digits. The upstream artifact locator alone
allows 1–1,024 bytes without controls. Relative executable paths obey the existing
V1 path grammar. No schema field confers review authority just by being present.

Every record contains `authority = "candidate-only-not-qualification"` (JSON
syntax in actual records). Fields below are in canonical order.

### `borrowser-preparation-pin-v1`

`format`, `authority`, `publisher`, `release`, `architecture`,
`upstream_artifact`, `upstream_sha256`, `provenance_record_sha256`,
`source_build_provenance`, `vendor_patches`, `extraction_record_sha256`,
`frozen_tree_identity`, `executable_path`, `browser_identity_sha256`,
`host_record_sha256`, `distribution_manifest_sha256`, `review_record_sha256`.

Architecture is exactly `x86_64`. Record exact publisher/release, immutable
upstream artifact and checksum, source/build provenance and vendor changes.
Long provenance, extraction and human review records remain external and are
referenced by raw SHA-256. A statement that source provenance is unknown is not
fabricated provenance; the reviewer must decide whether that candidate is usable.
The final review record binds inputs independently and excludes the pin record
itself to avoid a circular digest.

### `borrowser-preparation-host-v1`

`format`, `authority`, `image_identity`, `image_sha256`, `snapshot_identity`,
`pretty_name`, `architecture`, `kernel_release`, `uid`, `gid`,
`security_policy_record_sha256`, `libraries_record_sha256`,
`resources_record_sha256`.

Image and provisioned snapshot identities are explicit reviewed choices. Other
platform fields must match the actual host when validating/generating config:
PRETTY_NAME as parsed by the frozen collector, `x86_64`, kernel release, real and
effective nonzero UID and the nonzero GID. Detailed policy, loader/library and
resource inventories stay external, referenced by raw digest. Nothing invokes a
shell to source `/etc/os-release`.

### `borrowser-preparation-browser-identity-v1`

`format`, `authority`, `product_raw`, `browser_product`, `browser_version`,
`revision`, `protocol_version`, `executable_sha256`,
`distribution_manifest_sha256`, `unshare_sha256`, `unshare_version`.

The raw product is preserved and must have exactly one slash separating two
valid nonempty identity strings. `revision: null` means the response omitted the
field; `revision: ""` preserves an actually returned empty string. A number,
object, null JSON response value, control characters or invalid nonempty revision
is an error. Only missing/empty revision omits the configuration build-revision
field. Protocol version is the exact observed string, never a version-table guess.

## Manifest and configuration

The manifest producer reads the supplied tree without changing it. Directory
and regular-file ownership, modes and absent capabilities must be provable.
Capability query errors, including unsupported xattrs, are failures. It retains
directory/object handles, rejects unsupported types before potentially blocking
opens, streams hashes, checks mutations and resolves symlinks lexically. Every
object is inventoried; unsupported content is never silently excluded. At the final
inventory barrier every child entry is reopened with no-follow semantics through
its retained parent directory descriptor. Its device/inode and metadata must
match the retained original object. This covers files, symlinks themselves and
child directories, alongside the retained-object stability and directory
population checks. The opened root is the authority and has no parent binding.
These sequential checks do not replace the requirement for frozen storage.

Limits remain 1 MiB manifest, 1,024 directories, 4,096 file/link entries, 2 GiB per
file, 8 GiB total regular content, 256-byte relative paths and 64-byte components.
Links have at most 1,024 ASCII target bytes and 16 links per resolution; no escapes,
absolute targets, cycles, directory targets or empty components. Ordering and
canonical TOML come from the frozen V1 contract, independently implemented here.

Configuration generation verifies a clean exact `04d22c36` source checkout, both
pinned source manifest hashes and all members, plus inspector, packaging and the
independently reconstructed expression digest. It verifies actual host values and
cross-binds the pin, browser record, host record and distribution manifest digests.
It does not regenerate source manifests. No optional browser flags are selected.
The full canonical configuration golden is synthetic test data, never a real pin.

## Probe lifetime and differences from qualification

The probe inventories the exact explicitly supplied read-only distribution before
and after execution. Require read-only storage, with no writable mount alias of
that device in the host/probe mount inventory. Payload path ancestors must also be protected from non-root rename/write access.
Review backing storage outside the
VM as part of the freeze; mount flags do not prove that an external actor cannot
change the backing disk. Do not chmod/normalize payloads to satisfy these checks.

The explicitly identified util-linux executable is opened, hashed and executed
through its retained `/proc/self/fd` object; its bounded `--version` output must
match the supplied version line. It creates current-user mappings and new user,
network, PID and mount namespaces with a private procfs. The worker checks actual
maps, namespace identities and network namespace ownership, procfs identity and
population, mount flags/aliases, loopback flags and IPv4/IPv6 routes before launch.
Unexpected/unverifiable state fails. Browser capabilities are not retained from
namespace setup, and Chromium's own sandbox remains enabled.

The outer helper opens the kernel `/proc/self/exe` object, validates its regular
executable type, ownership/mode and absent capabilities, and retains its identity.
Exactly one deliberate helper executable descriptor survives the util-linux exec
boundary; other non-stdio descriptors are marked close-on-exec. The worker executes
through that inherited object, verifies its own `/proc/self/exe` against it and the
expected device/inode and executable metadata, then marks the inherited descriptor
close-on-exec. UID/GID presentation can change inside the user namespace; ownership
policy is checked before namespace entry. Rename-related ctime/link-count changes
cannot select a different executable object. Chromium receives only its deliberate
CDP descriptors beyond standard streams.

The probe uses a fresh private workspace as CWD and a fresh `profile` directory.
It uses exactly the nine minimal reviewed arguments, including relative
`--user-data-dir=profile`, `--remote-debugging-pipe` and final `about:blank`.
Descriptors 3/4 are private pipes. No TCP endpoint or arbitrary CDP forwarding
exists. The only commands are Browser.getVersion and Browser.close. Shutdown
without a close acknowledgement is allowed only when the browser exits normally
and the complete process/pipe cleanup checks succeed; shutdown error/hang fails.

Unlike qualification, the probe does not construct the collector's private tmpfs
snapshot/mount tree or run stopped-population sandbox qualification. It runs the
already frozen read-only supplied executable, and its profile/scratch directory
is a fresh private host temporary workspace. LANG/LC_ALL are C and TMPDIR is that
workspace. These storage paths differ; the headless mode and identity-bearing
browser arguments are unchanged. Browser.getVersion returns the running build's
identity, not a document/profile result. Regardless, the frozen collector must
independently check the tuple under its own full launch semantics.

A 120-second watchdog process bounds the probe, including filesystem preparation.
Ten seconds are reserved for terminal cleanup. Protocol frames are at most 1 MiB,
aggregate protocol (including both requests) at most 4 MiB, retained diagnostics at most 256 KiB. JSON has
bounded object/array populations and rejects duplicate keys. Requests are fixed,
small and bounded; no command renews the deadline. PIDfds bind termination targets.
Parent death kills unshare, whose child-death setup kills namespace PID 1 and its
descendants. Normal success additionally requires orderly browser exit, descendant
reaping, unshare reaping, explicit workspace disposal and watchdog completion.

A panic, watchdog kill, abnormal exit or cleanup error never publishes a candidate.
Abnormal termination can leave scratch resources; preserve failure diagnostics and
inspect/remove them through a separate reviewed cleanup procedure. Never interpret
absence of a returned cleanup error after a crash as proof of checked cleanup.

## Publication

Candidates are fully validated in memory. Publication opens and retains the
explicit destination-parent directory before exclusively creating an unpredictable
temporary name through that descriptor. It writes/syncs the complete file and uses
an atomic exclusive rename through the same parent descriptor. There is no
pathname-based rename/unlink or link/unlink fallback. Unsupported primitives fail.

The supplied parent pathname must identify that retained directory at both the
pre-publication and post-publication barriers. Rebinding fails; an already-published
candidate is removed through retained parent authority after checking its identity.
Temporary cleanup likewise uses retained authority. Cleanup failure takes precedence
over the operation error. Third-party hard links are not owned cleanup entries.
These checks cover the bounded transaction, not arbitrary post-return renames or
hostile concurrent manipulation of the output directory. Use a private controlled
output directory. Atomic visibility does not claim crash-durable directory metadata.

## Candidate reads and source-verification processes

Ordinary and confined candidate/source reads capture metadata before and after
reading from the same descriptor. Device/inode, type/mode, UID/GID, size, mtime,
ctime and link count must remain stable. Length, canonical-byte and expected digest
checks remain independent. These are bounded sequential checks, not snapshots.

Source verification has a closed command vocabulary: a bounded `git --version`
prerequisite, fixed filter-driver discovery, `rev-parse HEAD`, cached index equality, and hardened
`status --porcelain --untracked-files=all`. No caller supplies arbitrary commands,
config keys or regular expressions.
Require Git >= 2.45.0. Version output must be exactly `git version MAJOR.MINOR.PATCH`
with one final LF and canonical decimal components; vendor suffixes, prereleases,
old or ambiguous versions fail rather than being interpreted approximately. Retain
the exact version printed to preparation diagnostics in the build/environment record.
The Git executable and environment must remain fixed throughout preparation.

Verification uses `--no-optional-locks --no-pager --no-lazy-fetch -c core.fsmonitor=false -C <root>`.
The version prerequisite runs before relying on the Boolean FSMonitor setting:
older Git releases can interpret `false` as a hook pathname. Command-line `-c`
overrides repository/user configuration and inherited `GIT_CONFIG_COUNT` pairs
and `GIT_CONFIG_PARAMETERS`; no configuration files are changed. These environment
variables are retained, including unrelated settings such as reviewed safe-directory
configuration. The three existing repository-location environment overrides remain
removed. Tests cover an external hook, conflicting environment values and configured
Boolean `true`, without starting a built-in monitor daemon.

FSMonitor and discovered content-filter programs are disabled at command scope.
Before and after status, a typed discovery operation runs only:

```text
git --no-optional-locks --no-pager --no-lazy-fetch -c core.fsmonitor=false -C <root> config --includes --null --name-only --get-regexp '^filter\..*\.(clean|smudge|process|required)$'
```

Git's no-match exit status 1 is accepted only with empty stdout/stderr. Query bytes
are capped at 65,536, keys at 1,024, distinct drivers at 128, key length at 144 bytes,
and driver length at 128 bytes. Supported driver names contain ASCII letters,
digits, dot, underscore or hyphen. Unsupported names/framing fail closed. The parser
strips the exact prefix and recognized final suffix, preserving the entire
case-sensitive driver name (including dots), then deduplicates and sorts it.

For each driver, status receives direct argv `-c` settings with empty `clean`,
`smudge` and `process` values and `required=false`. Empty commands do not execute;
real clean/process invocation-witness tests verify this behavior. A repository that
needs a content filter to appear clean is rejected based on its unfiltered status;
the helper never executes filters or repairs/normalizes the checkout to compensate.

Shared inherited configuration, including includes, system/global files,
`GIT_CONFIG_COUNT` pairs and `GIT_CONFIG_PARAMETERS`, remains visible to discovery.
Only discovery removes `GIT_CONFIG`: that legacy selector redirects `git config`
but has no effect on status, so retaining it would query different sources.
Command-line overrides take precedence. Tests cover both injection mechanisms and
a conflicting `GIT_CONFIG` selector hiding a local driver.

The sorted pre/post driver sets must match. Changes to values for existing drivers
remain overridden; adding/removing a driver fails the final barrier. These are
sequential checks under a frozen preparation environment, not an atomic config
snapshot or protection against a driver added and removed between the checks.

The frozen `04d22c36` HEAD tree contains no gitlinks. Source verification first
checks the Git version and exact HEAD, then independently requires HEAD/index
equality using `diff-index --cached --quiet --no-ext-diff --ignore-submodules=none
HEAD --`. Exit 1 is a typed dirty-index failure; other nonzero exits are Git
failures. The explicit `none` prevents configuration from hiding staged gitlinks;
cached mode does not inspect their worktrees or invoke external diff programs.
After filter discovery, status uses `--porcelain --untracked-files=all
--ignore-submodules=all` with the FSMonitor/filter overrides. It cannot recurse
into submodule repositories, including gitlinks introduced in a changing index.
Filter discovery is repeated, then HEAD/index equality is checked again before
empty status output is accepted and frozen source digests are verified. Failed
operations stop verification immediately with checked cleanup. These sequential
barriers require the frozen preparation environment, not an atomic index snapshot.
This supports only the specified frozen revision, not arbitrary submodule-bearing
repositories. Under these constraints no further documented configuration-driven
external-helper path is identified for the five closed verification operations.

Every repository-reading command also forces child environment
`GIT_NO_LAZY_FETCH=1`, overriding inherited values without changing the parent
environment. Git 2.45 introduced the global `--no-lazy-fetch` option; there is no
older-Git fallback. Partial/promisor repositories are acceptable only when all
objects needed for verification are already local. Missing objects fail verification;
preparation never hydrates, fetches, repairs or retries them. Source verification
requires no network access. A local Trace2 regression witnesses fetch/upload-pack
in the unprotected positive control and no children in hardened verification,
including an inherited `GIT_NO_LAZY_FETCH=0`.

Ordinary probe children acquire a partial owner immediately after successful spawn.
Before pidfd transfer, their unreaped direct-child PIDs cannot be recycled. Failed
pidfd acquisition checks for prior exit, attempts termination if needed, and reaps
independently of signal success within a monotonic cleanup deadline. Interrupted
polls retry within the bound; lost ownership or timeout is cleanup failure. Cleanup
failure overrides acquisition failure. After transfer, signaling uses the pidfd.
Drop remains nonblocking emergency fallback, never checked completion.

The Git runner streams stdout up
to 65,536 bytes and stderr up to 16,384 bytes with a 30-second monotonic deadline.
Excess output, timeout or failure triggers explicit termination and bounded reaping
(up to two seconds); cleanup failure takes precedence. Git remains part of the
recorded development environment, and source bytes remain independently hash-bound.

Watchdog ownership begins immediately after fork. A partial owner handles pidfd
acquisition failure with checked termination and bounded, EINTR-aware reaping;
full ownership transfers once on success. Completion and failure cleanup are
explicit; destructors only attempt nonblocking emergency cleanup. No ordinary
watchdog error path uses blocking waitpid. Watchdog expiry remains fail-stop.

See [the preparation procedure](../../../docs/conformance/ag9g0-qualification-preparation.md)
for selection, independent review, frozen collector build and genuine execution.

## AG9g0a host-only prerequisite evidence

The execution baseline for AG9g0a is the clean reviewed helper source at
`763159c513e5a0d2c68ceff927b702ca4aa97351`, with its original independent lockfile.
See [the host-readiness contract](../../../docs/conformance/ag9g0a-linux-host-readiness.md).
The retained-worker deterministic test proves this helper's executable/FD handling
across test exec boundaries. The ignored `namespace_prerequisites_runtime` executes
actual util-linux with `/bin/true` and proves only basic namespace setup. The separate
host-readiness fixture proves external util-linux descriptor/namespace prerequisites;
it is not another WorkerExecutable implementation or proof of the full `identity`
path. Frozen collector runtime tests retain their separate authority.

No native host has been qualified by the repository-side support. Dirty/uncommitted
host-probe development output cannot become qualification evidence. Host readiness
requires the reviewed source-freeze handoff and genuine native x86-64 execution;
Chromium pinning, probing and mechanism qualification remain downstream.
