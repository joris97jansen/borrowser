# CI critical path and streaming-guard replacement — #1458

Status: **local replacement candidate, not published or qualified on GitHub**,
based on published parallel head `bf8572c5fb0169c6106e482cce439da0d62bb577`.
[PR #1463](https://github.com/joris97jansen/borrowser/pull/1463) remains draft.
The replacement remains uncommitted and unpublished. Hosted integration-job
failure propagation is verified below; a positive replacement run, controlled
performance measurements and runner-cost acceptance remain pending. The published
control retains both workspace guards. Publication requires separate authorization.

## Verified successful parallel control

[Run 43402060645, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/43402060645)
qualified exact head `bf8572c5fb0169c6106e482cce439da0d62bb577` and synthetic
integration `2598e93245977f25b1cc9e93d11aa97071f88e5e`.
Its ordered parents were master `144ef3bc887ac5688ac7f6b123a6e98f1e1485f2`
and that head. Both source trees were `7e385d78b60b79eaba51c5ab38a5b55a6f694c06`;
they nevertheless received independent execution.

All seventeen ordinary jobs and both final gates succeeded. The push-only AG4
proxy-timing experiment was correctly skipped. All six work jobs began between
09:31:46 and 09:31:48 UTC; queues were 2–4 seconds.

| Job | Actual execution | Existing deadline |
| --- | ---: | ---: |
| Source and provenance | 4m29s; 197 tests passed in 236.914s | 10m |
| Rust correctness and parser regressions | 18m40s | 30m |
| Release, networking abort and benches | 5m17s | 20m |
| Conformance execution | 4m56s | 10m |
| Streaming integration | 11m59s | 25m |
| Streaming exact head | 10m29s | 25m |
| AG2 native Chromium | 7m39s | 30m |
| AG4 synthesis/snapshots | 2m21s | 10m |

The remaining nine fuzz jobs all passed in 36–51 seconds. The last ordinary job
ended at 09:50:27; `CI qualification` ran at 09:50:29–31 and the existing required
`Rust (ubuntu-latest)` context at 09:50:33–36. Both checks reported success for
the exact PR head under GitHub Actions app 15368, matching the live master ruleset.
The gate's seventeen actual result records and all six Rust source receipts were
checked. This run proves the hosted success path; the separate negative-enforcement
experiment below verifies integration-job failure propagation.

Run creation to final required check was **18m53s**; API finalization made workflow
elapsed **18m54s**. The execution-only critical path was **18m45s** (longest work
job plus both gates, excluding queue/dispatch/finalization). Total runner execution
was **72m26s**, excluding queue and billing rounding. Rust correctness owned the
critical path. The earlier serial control was cancelled after 45m17s with head
execution incomplete; it remains evidence of the serial architecture's failure,
not a successful equal-coverage baseline.

[1458-parallel-evidence.json](1458-parallel-evidence.json) records the successful
run's source identities, job/cache observations, artifact digests and native proof.
Earlier design/profiling observations remain explicitly historical.

## Native coverage and compiler equivalence

The guard used Rust/Cargo 1.92.0 natively on Linux x86-64 with opt-level 1, debuginfo 2,
debug assertions and overflow checks enabled. `CARGO_INCREMENTAL=0`;
`RUSTFLAGS`, encoded Rust flags and build-target overrides were absent.

| Guard execution | Result | Actual guard time |
| --- | --- | ---: |
| Original default workspace lane | Passed | 188.24s |
| Original explicit HTML5 workspace lane | Passed | 187.07s |
| Independent integration artifact | Exactly one passed; no failures/ignored tests | 219.41s |
| Independent exact-head artifact | Exactly one passed; no failures/ignored tests | 192.77s |

Head compilation took 352s; source verification, selection and bounded cleanup
completed before success. The head executable lived in its separate temporary
target directory. The strict binding verified package `runtime_parse`, target
`perf_regression`, expected source path, standard libtest manifest properties
and successful non-ignored execution. Integration used its own full-workspace
artifact. Recording the head object alone was not treated as head qualification.

The default and HTML5 recipes emitted **410 identical package/target/profile/
feature configurations** and identical guard bytes:
`ab03f23b53a2be100647c775ec69621effe576cebb84590081105434a095768c`.
All 410 default artifacts were rebuilt; the HTML5 lane reused all 410. The
narrower workspace probe emitted an 86-configuration subset and reused the same
guard bytes. It remains diagnostic: neither guard execution recipe is narrowed,
and package-scoped `-p runtime_parse` compilation is not substituted.

Each native lane's baseline listing contained **2,964 records**. Adding the
proposed guard exclusion yielded **2,963**; multiset comparison removed exactly
`runtime_parse_linear_streaming_guard: test` and changed nothing else. Existing
ignored inventory records and native-wrapper exclusions were preserved.

Integration archive 14026292028 (18 files), SHA-256
`6fe1f3421c7b892551f6eb1321d043ada87e261bc89ea039a4713bcc42a5b5e6`,
contains all three compiler JSON streams, inventory pairs and guard evidence.
Head archive 14025469348 (10 files), SHA-256
`0f1b78a0101c410201fb6340631b77db15c9d9f4fff6d373d383f03d370eb0d0`,
contains source, selection, binding, execution and phase timings. All six run
archives passed digest and ZIP-integrity verification, including nine compilation
timing reports and AG2 native/browser/cleanup evidence.

The unchanged guard constructs 5,000/20,000-fragment in-memory strings and feeds
64-byte chunks through the runtime. Workloads, counter/patch/token/batch assertions,
instrumentation and the 30-second inter-event timeout remain unchanged. Its source
SHA-256 remains `d94155270a625f8441fc4500b534825b8648f979389eb4c20b49e626c669487a`.

## Local replacement contract

Only the two correctness invocations gain a guard exclusion:

```sh
cargo test --workspace --lib --bins --tests --examples --locked -- --skip aws::command::tests::native_procfs_ --skip runtime_parse_linear_streaming_guard
cargo test --workspace --lib --bins --tests --examples --features html5 --locked -- --skip aws::command::tests::native_procfs_ --skip runtime_parse_linear_streaming_guard
```

Every other argument and qualification command stays intact. Cargo still compiles
the full workspace guard; this removes duplicate execution, not compilation.

The two literal commands have YAML scalar anchors, passed as the integration
audit's environment values. The audit parses those exact commands with `shlex`,
requires the explicit terminal guard exclusion and removes only that pair to
construct each baseline listing. The candidate and baseline listings must differ
by exactly one expected guard record. This prevents the audit and executed
recipes from silently drifting apart. Substring collisions, duplicate/absent
guards and missing command inputs fail closed.

The full default/HTML5 compiler-configuration and guard-byte comparison is now
fatal on every run. The collector first retains diagnostics, then fails on either
mismatch. Narrow-recipe comparisons remain informational because no narrowing is
activated. Equivalence and inventory validation run before integration guard
execution and successful source receipts. Their failure makes the integration
job fail; the existing final gate rejects it, and the required Rust barrier
rejects the gate. No permissive fallback or rollout switch is introduced.

The current qualification-command ownership regression replaces the historical
`git show 144ef3bc` command-freezing test. Its explicit current contracts cover
Rust/provenance/release/conformance commands, AG2's native and standalone ignored
qualifications, AG4 and every fuzz command. Real Cargo fixtures validate actual
production workspace recipes, complete inventory differences and continued
failure of unrelated tests. The successful published control's original-command
conservation audit is retained as immutable migration evidence, not a permanent
requirement to execute obsolete commands.

## Mandatory graph and source boundaries

The successful graph is unchanged: six independent Rust work jobs, existing AG2,
AG4 and nine fuzz jobs feed `CI qualification`, then the required
`Rust (ubuntu-latest)` compatibility barrier. There are no new heavy dependencies,
job identities, cache namespaces, permissions or deadlines.

Each Rust work job verifies the integration SHA, ordered PR parents, exact head
object and fast committed-input preflight before Cargo work. Full provenance runs
in its independent mandatory job. Completion rechecks source identity/cleanliness.
Fork heads come through the synthetic merge ancestry without fork write access.

On PRs every ordinary job must succeed. Only the head job may be skipped on master
pushes, without head outputs; the actual pushed SHA is qualified there. Missing,
unexpected, failed, cancelled, timed-out or incorrectly skipped results and
missing/mismatched source receipts are rejected. Local fixtures exercise the
actual gate and compatibility programs, including both guard jobs independently.

### Completed hosted negative-enforcement experiment

[Experimental draft PR #1464](https://github.com/joris97jansen/borrowser/pull/1464)
was published separately from #1463, then closed **without merging** at
2026-11-13 13:47:21 UTC after preserving the evidence. Its sole experimental commit
`6141fe4f6cd45b50b3e1499d90d85452576fb822` has successful control `bf8572c5` as
its parent. The seven-line workflow addition wrote a diagnostic marker and exited
1 in `streaming_integration` after source/preflight verification and before
compilation. Both original workspace guards and all gates remained unchanged.
The binary/full-index diff against the control has SHA-256
`1c80460d032127bd49868fc3ef26126331c04e743ea27bde82b88fa0d6e0780f`.
No experimental failure step is present in the replacement candidate.

[Run 43433009156, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/43433009156)
executed exact head `6141fe4f6cd45b50b3e1499d90d85452576fb822` and synthetic
integration `bb290833509066ca43d3d14bb3fbbd814b9ef3a0`. The integration parents,
in order, were master `144ef3bc887ac5688ac7f6b123a6e98f1e1485f2` and that head.
Both trees were `07072facc675638f7a5606a166021bee85d07a7b`; their separate SHAs
were verified during execution and in the source receipts.

| Check | Actual result | Hosted evidence |
| --- | --- | --- |
| Streaming guard (integration) | Failure; deliberate exit 1, no successful source receipt | [Job 131910976120](https://github.com/joris97jansen/borrowser/actions/runs/43433009156/job/131910976120) |
| CI qualification | Failure; `qualification did not succeed: streaming_integration` | [Job 131922428041](https://github.com/joris97jansen/borrowser/actions/runs/43433009156/job/131922428041) |
| Rust (ubuntu-latest) | Failure; received `QUALIFICATION_RESULT: failure`, exited 1 | [Job 131922486390](https://github.com/joris97jansen/borrowser/actions/runs/43433009156/job/131922486390) |

All **sixteen other ordinary jobs succeeded**. The actual final-gate input held
exactly seventeen prerequisite results, with only `streaming_integration` failed
and its outputs empty. Both original workspace guards passed; the independent
exact-head guard passed exactly one test with zero failures or ignored tests
(126.08s). Source/provenance passed all 197 tests. Only the unrelated push-only
AG4 proxy-timing job was intentionally skipped; no ordinary job was cancelled.

Read-only `gh pr checks 1464 --repo joris97jansen/borrowser --required --json
name,state,link,bucket` selected exactly `Rust (ubuntu-latest)`, with state
`FAILURE` and bucket `fail`. Active master ruleset **11427025** required that
exact context from GitHub Actions app **15368**, with no bypass actors. All three
failed checks identify the experimental head and app 15368. The ruleset was not
changed and no merge was attempted. Draft status alone was not used as proof.

The failing integration job's marker artifact **14041945334** uploaded successfully:
305-byte ZIP, SHA-256
`e9ca6b8d19813c3dcc10738b8849d0f7aba82c317c4f3b35bc8cf482c2265a3a`.
Its contents record experiment `1458-negative-streaming-integration`, the exact
integration SHA and expected exit 1. The PR records the conclusions and source
identities; structured evidence records receipts, check identities and artifact
checksums. The local logs/API snapshots and verified archive are retained under
`/private/tmp/borrowser-1458-negative-enforcement`.

This demonstrates hosted **integration-job failure propagation** through both
gates into the existing required check. Head-job failure, cancellation, timeout,
unexpected skips and missing/mismatched outputs were tested locally, not
individually reproduced hosted. Whole-workflow cancellation was not exercised.
The deliberately early-failing run took 23m14s elapsed and 64m24s of summed job
execution; it is not a successful performance baseline or replacement qualification.
GitHub treats skipped jobs as successful required checks, making both `always()`
barriers and explicit result validation material; see
[GitHub's job-condition documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-jobs-with-conditions).

## Caching, duplication and runner cost

The cache architecture is unchanged. All six new namespaces missed in the
successful parallel control; AG2 and all nine fuzz jobs had exact hits. There
were no fallback hits in that run.

| New namespace/job | Miss lookup | Post-cache step | Compressed upload bytes |
| --- | ---: | ---: | ---: |
| provenance registry | 2s | 5s | 91,491,557 |
| workspace correctness | 1s | 11s | 615,973,341 |
| release/abort/benches | 2s | 7s | 451,493,982 |
| package conformance | 2s | 10s | 428,418,316 |
| integration streaming | 3s | 11s | 473,111,029 |
| head registry | 2s | 46s | 91,492,219 |

Uploads totalled 2,151,980,444 bytes, approximately 2.00 GiB; retained storage after
eviction was not measured. AG2 restored 510,652,035 bytes in an 8s cache step.
Fuzz restore sizes were approximately 87 MiB, or 157 MiB for CSS regressions.

The source/head caches remain registry-only. Other jobs own separate targets;
workspace-crate caching, Cargo-home `cache-bin` and cache-on-failure remain disabled. There
is no shared mutable target, cross-job binary transfer, sccache or privileged
cross-PR cache promotion. The installer explicitly requests the pinned 1.92.0;
unused stable was not upgraded in these six jobs. Other installed compilers and
runner-image changes can still invalidate keys.

The first integration and head compilations each compiled 453 timing units,
including 346 dependency units; conformance compiled 317 units, including 298
dependency units. Release's initial build compiled 334 units, including 309
dependency units. This demonstrates substantial repeated dependency compilation.
Unit elapsed durations overlap; they are not CPU seconds or runner consumption.
Narrowing's independent cold cost and cache-policy improvements remain unmeasured.

## Measured costs and replacement projection

The latest successful master baseline had 26m53s workflow elapsed and 42m03s
runner execution. The parallel control achieved 18m54s and 72m26s: observed
latency fell 29.7%, while runner execution increased 72.3%. These differ in source
qualification, temporary extra guard execution, caches and runner variability;
they are not a controlled equal-coverage A/B comparison.

Removing the two measured workspace executions subtracts 375.31s from correctness.
Holding all other costs constant gives a **counterfactual** correctness duration
of about 12m25s, final workflow duration about 12m34s and runner execution about
66m11s (8.6% below the parallel control). Integration's observed 11m59s would be
close to the projected critical path. These are arithmetic estimates, not measured
replacement results, warm-cache evidence or a guarantee of the 15-minute target.

Even this estimate remains about 57% above the old master runner total. The
original 10% cost criterion has not been relaxed or satisfied. Equal-coverage
serial/reference accounting, repeated measurements and an explicit cost decision
remain necessary. Preserve at least three exact-cache and two controlled cold
samples, classify fallback separately, and report medians/ranges plus queue,
setup, post-cache and artifact costs. An incidental cache miss is not a controlled
fresh-Cargo-home experiment. No failed/cancelled run can establish an improvement.

## Four-level validation and readiness

1. **Targeted:** actual Cargo/Git fixtures exercise same-run equivalence failures,
   binary-only/configuration-only mismatch, inventory collisions, replacement
   execution of other targets and both gates' guard failure/source-result matrix.
2. **Subsystem:** run all qualification regressions in both color modes on macOS
   and offline Linux arm64; run the actual workspace recipes, feature/inventory
   programs, Actionlint with ShellCheck, YAML/Bash, formatting and diff checks.
   Developer runs of an uncommitted candidate are not committed CI qualification.
3. **Runtime smoke:** no new visible/runtime behavior changes. Existing native AG2
   and networking release-abort qualification passed in the published control.
4. **Full CI:** retain `make check-url-dependencies` and `make ci` committed-input
   preflight. The local candidate cannot pass it while qualification inputs are
   uncommitted; do not bypass it. Require committed provenance and a new successful
   hosted replacement run; the separate hosted integration-failure experiment is
   complete and does not qualify this unpublished replacement.

Earlier local replacement validation results are recorded in the structured
evidence.
All 63 qualification regressions passed in both color modes on macOS arm64
(55.700s each) and offline Linux arm64 (14.795s / 14.027s). The eight focused
replacement cases passed first. The actual local compiler/inventory programs
passed: 392 matching full/HTML5 configurations, a 92-configuration narrower subset,
identical guard bytes and 2,957 → 2,956 inventory records in each lane. These
existing-cache developer probes are not native hosted replacement qualification.

An additional full default workspace execution exited 101 at the existing
conformance native process-ownership tests: 186 passed, nine failed, 13 ignored.
The failures report macOS Mach inspection errors; the guard target was not reached.
An outside-sandbox ownership probe also exited 101, reporting unavailable live
process-executable inspection and unsupported native sandbox-denial reproduction.
The full HTML5 execution was not attempted after that failure. No test was skipped
to turn this into a passing result; the exact replacement recipes also ran in
the disposable real-Cargo fixtures, and both full inventory/feature probes passed.

Evidence-synchronization revalidation used macOS arm64 with Rust/Cargo 1.92.0.
The two focused replacement/gate regressions passed first (0.565s); all 63
qualification regressions then passed with `CARGO_TERM_COLOR=always` (44.631s)
and `never` (47.843s). Actionlint 1.7.12 with ShellCheck 0.11.0, expanded YAML,
all 96 Bash run blocks, Rust formatting, JSON and combined-diff checks passed.
The actual workspace commands differ from the published control only by the
terminal guard exclusion. Their shared audit values, mandatory equivalence and
inventory steps, independent guard execution and both final gates were verified.
No executable CI logic or test content changed during this evidence update.

`make check-url-dependencies`, `make check-network-dependencies` and `make ci`
each exited 2 at `staged qualification tests differ from committed candidate`.
This expected preflight rejection is not full committed qualification; no
workaround was used. A separately authorized commit must pass that qualification
before publication.

**Verdict: Not closeable yet.** Hosted integration-failure enforcement is verified.
The candidate may proceed to separately authorized controlled publication after
local review and validation; it is not completed replacement qualification.
A positive hosted replacement run, controlled timings and runner-cost acceptance
remain outstanding.

No browser-engine behavior or feature-gap status changes; the feature-gap tracker
is unchanged. The independent parser scalability finding below remains separate.

## Retained historical measurements and independent engine finding

The following observations preserve the baseline and engine profile. They do
not describe the new job graph or substitute for its hosted qualification.

## Refreshed observational baseline

Collected on 2026-11-12 using GitHub's run/job APIs and decoded job logs.
[1458-baseline.json](1458-baseline.json) preserves all returned job and step
timestamps, durations, available cache excerpts and guard excerpts. Its
`head_sha` is the run API's field, not independent proof of a PR-head execution.

| Run | Revision | Rust | Workflow elapsed | Sum of job execution | Rust cache |
| --- | --- | --- | --- | --- | --- |
| [42522258228](https://github.com/joris97jansen/borrowser/actions/runs/42522258228) | `4012cb42` PR run API head | 24m01s | 24m05s | 38m30s | fallback, 968 MiB |
| [42568144045](https://github.com/joris97jansen/borrowser/actions/runs/42568144045) | `4a997e93` master push | 24m17s | 24m21s | 40m39s | fallback, 968 MiB |
| [43188478801](https://github.com/joris97jansen/borrowser/actions/runs/43188478801) | `a382f604` master push | 35m36s | 35m40s | 53m18s | fallback, 1088 MiB |
| [43239794456](https://github.com/joris97jansen/borrowser/actions/runs/43239794456) | `144ef3bc` master push | 26m49s | 26m53s | 42m03s | exact hit, 1081 MiB |

These are different revisions with different qualification/setup behavior. They
are not a controlled cold/warm comparison, repeated runs at one revision, or
evidence that #1458 has improved CI. No cold-cache baseline is available yet.
Workflow elapsed includes queue/startup/completion overhead; summed job time
excludes queue delay and billing rounding. Both are merge/push-only baselines
without independently executed PR heads.

Latest master Rust step breakdown:

| Step | Elapsed |
| --- | --- |
| Cargo cache restore | 28s |
| URL sources, graphs and provenance regression qualification | 225s |
| Debug all-target build | 244s |
| Package-only conformance build | 61s |
| Default workspace tests | 282s, including 210.90s guard |
| Explicit HTML5 workspace tests | 280s, including 209.97s guard |
| Release workspace build | 104s |
| Feature-qualified release-abort qualification | 59s |

The URL step's actual graph checks took about two seconds; the regression suite
accounts for most of the remaining time. Those provenance/history tests remain
required. No cache or scheduling change is justified merely by their duration.

Latest master AG2 took 6m48s: runtime packages 5s, reference installation 4s,
Cargo cache 12s, build/lint 63s, native qualification 67s, complete conformance
242s, artifact upload 1s. It is not the observed critical path. Keep its pinned
browser, native ownership, sandbox, cleanup and complete real-browser coverage.

The original comparison workflow remains available at the immutable source
revision referenced by the original run; the refreshed comparison workflow is
[ci.yml at 144ef3bc](https://github.com/joris97jansen/borrowser/blob/144ef3bc887ac5688ac7f6b123a6e98f1e1485f2/.github/workflows/ci.yml).

### Re-fetching the raw timing evidence

```sh
gh api repos/joris97jansen/borrowser/actions/runs/43239794456
gh api 'repos/joris97jansen/borrowser/actions/runs/43239794456/jobs?per_page=100'
gh api repos/joris97jansen/borrowser/actions/jobs/131256790892/logs > rust.log
```

For each job or step, subtract `started_at` from `completed_at`. Sum job
execution, including cache post-steps, separately from event-to-final-check
latency. Retain run attempt and all failures/cancellations; do not select only
fast successful runs.

## Unchanged guard profiling

Supporting local evidence is macOS arm64 with Rust/Cargo 1.92.0 and the existing
development profile (`opt-level=1`, debug assertions and overflow checks enabled).
It does not establish native Linux x86-64 equivalence or hosted performance.
Structured local results are in [1458-local-evidence.json](1458-local-evidence.json);
the [profile excerpt](1458-profile-sample.txt) preserves the observed worker path.

The guard source SHA-256 is
`d94155270a625f8441fc4500b534825b8648f979389eb4c20b49e626c669487a`.
Its workload, assertions, instrumentation and behavior are unchanged.

The full default workspace test build completed in 65.222s against an existing
local dependency/build cache; it was not a cold build. The explicit HTML5
no-run invocation took 1.353s and reported the guard fresh. Both contained 392
identical compiler-artifact configuration records and the same guard bytes:
`eb942f8885bf88a6d805088c48b0a535cc8c5b08505dc95bf2a347297438edfa`.

`cargo test --workspace --test perf_regression --locked --offline --no-run
--message-format=json` also reused those guard bytes locally (3.708s invocation).
This is supporting evidence for a narrower target recipe, not proof of its cold
compilation cost or native Linux equivalence. A plain `-p runtime_parse` remains
unqualified: its feature graph differs, including `html/parser-conformance`.

The unchanged guard passed in 133.71s in the sampled run. A ten-second sample
contained 8,463 worker-thread observations, with 7,623 (90.1% of that window)
under `assert_invariants`' parent child-list `filter().count()` loop. The test
thread was predominantly waiting for events. This is a sample-window finding,
not a claim that 90.1% of the whole run or Linux runtime has been attributed.

The concrete path is:

```text
runtime_parse::driver::handle_runtime_chunk
  RuntimeState::drain_patches
    HtmlParser::take_patches
      HtmlParser::apply_patches
        PatchValidationArena::apply_batch_trusted
          PatchValidationArena::assert_invariants
            parent_node.children.iter().filter(...).count()
```

`validate.rs:141–145` scans a parent's whole child list for each child. A wide
parent therefore introduces quadratic sibling-reference counting per audit;
auditing the growing arena after fixed-size drains compounds that work. This
is production parser validation, not fixture-string construction or a sleep.
The trusted runtime path does not clone the arena transactionally. Full phase
attribution, including allocation, finalization and Linux profiling, remains
pending. No validation is bypassed or reduced to conceal this finding.

An additional unprofiled test passed in 134.81s, but `/usr/bin/time -l` returned
1 because the sandbox denied `sysctl kern.clockrate`. That wrapper result is
not recorded as a successful measurement command or a parser-test failure.
An independent invocation using Python's child-process resource accounting
passed: 131.82s reported by libtest, 131.860s wall, 131.424s user and 0.196s system.
These uncontrolled local observations do not satisfy the hosted comparison criteria.
Local raw artifacts are under `/private/tmp/borrowser-1458-evidence`; the sample
SHA-256 is `dabc14bfe89f0b1e3bccb69b451ecbf4b62e54c8c61ec17094e69a050216d91d`.

### Reproduction

Run the exact workspace compilation configurations; do not substitute release,
alter compiler profiles, disable instrumentation or reduce inputs:

```sh
CARGO_INCREMENTAL=0 cargo test --workspace --lib --bins --tests --examples \
  --locked --offline --no-run --message-format=json --timings -vv > default.jsonl
CARGO_INCREMENTAL=0 cargo test --workspace --lib --bins --tests --examples \
  --features html5 --locked --offline --no-run --message-format=json --timings -vv > html5.jsonl
```

Cargo's verbose build-script diagnostics can appear alongside JSON; identify
`compiler-artifact` JSON records. Obtain the `runtime_parse` `perf_regression`
test executable from its artifact, hash it immediately after each build, then
run it from `crates/runtime_parse` with
`runtime_parse_linear_streaming_guard --exact --nocapture`. Sample that process,
including the runtime worker, with macOS `sample PID 10 1 -mayDie -file profile.txt`
or an available native Linux CPU profiler. Sampling is diagnostic; compare
unmodified, unprofiled runs for performance decisions.

## Proposed independent parser defect

No GitHub issue has been created and no milestone setting changed.

This evidence and proposal belong here rather than in
`docs/engine-feature-gap-tracker.md`, whose documented purpose is a small
checklist of visible browser/CSS feature gaps. Profiling an existing validation
path does not change feature support or conformance status. The review removes
the candidate's tracker entry while preserving the complete finding below.

Milestone proposal: existing **Milestone AU — Real-World Static Page
Compatibility, Quirks, Resilience, and Production Hardening** (#107). The original
HTML/parser foundation milestones are closed; this is an unresolved production
scalability finding, not a reason to claim their conformance work is undone.

Title: **Bound repeated sibling-reference scans in streaming HTML patch validation**.

Description: the unchanged streaming guard at `144ef3bc` spends most of the
recorded macOS worker sample in the full parent-child reference count in
`PatchValidationArena::assert_invariants`. Repeated audits of a wide, growing
arena impose superlinear validation work even though emitted token/patch counts
meet the current guard. Include the exact source/binary identities, workload,
sample counts and reproduction above; obtain native Linux attribution as well.
Investigate preserving duplicate-reference detection, parent consistency,
template ownership, reachability, acyclicity, transactional public application
and terminal trusted-session failure while bounding validation work. Add stable
work-count regression coverage and retain semantic/failure tests. Do not fix
this defect by reducing fixture size, dropping audits or changing CI profiles.
