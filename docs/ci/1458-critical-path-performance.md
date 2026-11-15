# CI critical path and streaming-guard replacement — #1458

Status at the **2026-11-15 final acceptance review**: published replacement
`786849bc8cca3697cb21092e3dd6cf05f9237e31` passed native hosted qualification
and the complete sixteen-observation controlled campaign. The five prospective
replacement warm observations satisfy the owner-approved latency and Option B
runner-cost limits. This is completed performance evidence, not an uncommitted
implementation or a claim of statistical significance.

[PR #1463](https://github.com/joris97jansen/borrowser/pull/1463) remains draft.
This documentation update, its final hosted checks, history-preserving merge,
post-merge verification, cleanup and issue closure require their respective
separate authorizations. The [final acceptance section](#final-acceptance-on-2026-11-15) supersedes earlier
readiness statements; dated historical results and failed observations are retained.

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

## Published replacement contract

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

## Historical costs and pre-publication replacement projection

The following is the **2026-11-13 pre-publication** accounting snapshot. Its
counterfactual estimates and then-pending policy decision are preserved as history.
The controlled results and subsequent explicit Option B decision below supersede
its readiness statements; none of these estimates is an acceptance measurement.

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

## Historical local validation and pre-publication readiness

This records the **pre-publication replacement review on 2026-11-13**. In
particular, its uncommitted-input rejections and macOS native-ownership failures
remain failures; they are not rewritten as passing qualification. The published
replacement and subsequent controlled campaign have their own native hosted
qualification, recorded in the final acceptance section.

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

**Pre-publication verdict: Not closeable yet.** Hosted integration-failure enforcement is verified.
The candidate may proceed to separately authorized controlled publication after
local review and validation; it is not completed replacement qualification.
A positive hosted replacement run, controlled timings and runner-cost acceptance
remain outstanding.

No browser-engine behavior or feature-gap status changes; the feature-gap tracker
is unchanged. The independent parser scalability finding below remains separate.

## Final acceptance on 2026-11-15

The controlled campaign is complete: **two cold and six exact-cache observations
per candidate**, sixteen complete successful workflows. Performance acceptance
under the explicitly approved Option B policy is demonstrated for this fixed
campaign. It is separate from documentation publication, merge authorization,
post-merge verification, cleanup and issue closure.

### Frozen source and qualification identities

Master remained `144ef3bc887ac5688ac7f6b123a6e98f1e1485f2`. Control C is
`bf8572c5fb0169c6106e482cce439da0d62bb577`; replacement R is published
`786849bc8cca3697cb21092e3dd6cf05f9237e31`. These are **different Git
commits** with equal engine sources, manifests/locks, toolchain, compiler
profiles and guard workload/assertion/instrumentation blobs. Their five-path
difference is CI, qualification tests and documentation. C has 58 qualification
regressions; R has 63, adding replacement/inventory safeguards rather than
removing engine coverage.

The first published R qualification, [run 43454245674](https://github.com/joris97jansen/borrowser/actions/runs/43454245674),
passed all seventeen ordinary prerequisites and both gates on exact head
`786849bc8cca3697cb21092e3dd6cf05f9237e31` and integration
`14d62899adbfa7fd6e8bd8efc7570c21eb8ab9fd`, tree
`dc02569fe2ea6f1e44bcf0fbe2cc56c0dcb4bd3d`. Its parents were master then
that head. Its 10m44s required-check latency, 10m45s elapsed and 52m28s runner
execution are **observational**, not part of the controlled sample matrix.

The four isolated cohort patches changed only the workflow: pinned reviewed
rust-cache action, step-scoped cache namespace and verified empty dependency/
target baseline. All sixteen expanded cache actions use the cohort marker;
it affects primary/fallback keys and never reaches compiler/test commands.
Each cohort contains its initial workflow-only commit and three tree-identical
warm trigger commits. No retry, replacement sample, reordering or unfavorable
observation exclusion occurred.

| Cohort / draft PR | Initial workflow head | Frozen tree |
| --- | --- | --- |
| [C1 / #1465](https://github.com/joris97jansen/borrowser/pull/1465) | `5c67f95fcbdcd9d067a1af3824fba42579e1838d` | `656510dc0a468cbdbd449bf1f8b824ecb55ba640` |
| [R1 / #1466](https://github.com/joris97jansen/borrowser/pull/1466) | `ebcd254db0126199d1dbb45bd77905341faa14ba` | `698f0d2f6c1dbdf8a3f9758c76422c62a9779061` |
| [C2 / #1468](https://github.com/joris97jansen/borrowser/pull/1468) | `4bbaf39cf7eb5be68d4aed90132e7eb2fd0824a0` | `3ce2b9839a5639ea9296a053d1c6407c0c701bfe` |
| [R2 / #1467](https://github.com/joris97jansen/borrowser/pull/1467) | `e142d284a783e187943c48a6cf3c8a621ddbbdef` | `07f12434d1b88d75c63a9fa1dc25445c554285b6` |

Every run's full head, integration SHA, ordered parents, tree, timestamps and job
IDs are durable in [the structured record](1458-parallel-evidence.json), under
`final_acceptance.observations`. Actual checkout receipts cover all seventeen
ordinary jobs; the detached head worktree has its own source/artifact/execution
receipts. Recording a head SHA alone is not treated as head qualification.

### Complete cold and warm dataset

Primary latency is **`workflow.created_at` → completion of required
`Rust (ubuntu-latest)`**, including queues. Workflow elapsed is reported
separately. Runner execution sums all nineteen applicable jobs, including setup,
cache work, uploads, post-steps and both gates; queue and billing rounding are
excluded. The queue-free clock is longest ordinary job plus both gate execution
durations. Exact push webhook receipt times were unavailable: cold PR-creation
timestamps and warm push-acceptance bounds/proxies are kept separately in JSON.

All rows passed seventeen ordinary qualifications and both gates. Each cold row
has sixteen genuine misses and successful saves; each warm row has sixteen exact
primary-key restores with the same cohort cache IDs, versions and sizes. There
were no fallback observations or cache-service errors.

| Stage | Observation / immutable run | Cache | Required latency | Workflow elapsed | Runner execution |
| --- | --- | --- | ---: | ---: | ---: |
| 1 | [C1-cold](https://github.com/joris97jansen/borrowser/actions/runs/43573905149) (43573905149) | Cold | 18m35s | 18m36s | 76m08s |
| 1 | [R1-cold](https://github.com/joris97jansen/borrowser/actions/runs/43575885767) (43575885767) | Cold | 12m39s | 12m39s | 66m12s |
| 1 | [C1-warm](https://github.com/joris97jansen/borrowser/actions/runs/43636915894) (43636915894) | Exact | 14m37s | 14m38s | 59m28s |
| 1 | [R1-warm](https://github.com/joris97jansen/borrowser/actions/runs/43638762436) (43638762436) | Exact | 11m02s | 11m03s | 55m19s |
| 2 | [R2-cold](https://github.com/joris97jansen/borrowser/actions/runs/43691601576) (43691601576) | Cold | 12m41s | 12m41s | 68m20s |
| 2 | [C2-cold](https://github.com/joris97jansen/borrowser/actions/runs/43699239820) (43699239820) | Cold | 22m44s | 22m44s | 79m28s |
| 2 | [R1-warm2](https://github.com/joris97jansen/borrowser/actions/runs/43701262931) (43701262931) | Exact | 10m24s | 10m24s | 50m21s |
| 2 | [C1-warm2](https://github.com/joris97jansen/borrowser/actions/runs/43702366735) (43702366735) | Exact | 14m45s | 14m46s | 60m31s |
| 2 | [R2-warm1](https://github.com/joris97jansen/borrowser/actions/runs/43703735602) (43703735602) | Exact | 12m08s | 12m09s | 55m02s |
| 2 | [C2-warm1](https://github.com/joris97jansen/borrowser/actions/runs/43704982617) (43704982617) | Exact | 15m48s | 15m49s | 59m12s |
| 2 | [C1-warm3](https://github.com/joris97jansen/borrowser/actions/runs/43706373168) (43706373168) | Exact | 14m45s | 14m46s | 60m18s |
| 2 | [R1-warm3](https://github.com/joris97jansen/borrowser/actions/runs/43707683987) (43707683987) | Exact | 11m53s | 11m54s | 58m13s |
| 2 | [C2-warm2](https://github.com/joris97jansen/borrowser/actions/runs/43708755916) (43708755916) | Exact | 17m49s | 17m50s | 61m21s |
| 2 | [R2-warm2](https://github.com/joris97jansen/borrowser/actions/runs/43710332141) (43710332141) | Exact | 9m27s | 9m28s | 53m42s |
| 2 | [R2-warm3](https://github.com/joris97jansen/borrowser/actions/runs/43711325621) (43711325621) | Exact | 10m03s | 10m03s | 53m44s |
| 2 | [C2-warm3](https://github.com/joris97jansen/borrowser/actions/runs/43712326996) (43712326996) | Exact | 19m00s | 19m01s | 61m35s |

### Independent calculations and prospective policy

The final review recalculated these values from raw run/job/check timestamps,
rather than accepting the campaign summary's medians. Candidate median and
observed range are:

| Condition / measurement | C median (range) | R median (range) |
| --- | ---: | ---: |
| cold / Required-check latency | 20m39.5s (18m35s–22m44s) | 12m40s (12m39s–12m41s) |
| cold / Runner execution | 77m48s (76m08s–79m28s) | 67m16s (66m12s–68m20s) |
| cold / Queue-free critical path | 20m32s (18m28s–22m36s) | 12m33s (12m32s–12m34s) |
| exact / Required-check latency | 15m16.5s (14m37s–19m00s) | 10m43s (9m27s–12m08s) |
| exact / Runner execution | 60m24.5s (59m12s–61m35s) | 54m23s (50m21s–58m13s) |
| exact / Queue-free critical path | 15m10s (14m29s–18m53s) | 10m36.5s (9m22s–12m02s) |

Pair by cache-cohort number and warm repetition; values below are **R minus C**,
not differences between unpaired job samples:

| Cache condition / cohort pair | C run | R run | Required latency difference | Runner difference |
| --- | ---: | ---: | ---: | ---: |
| cold / 1 | 43573905149 | 43575885767 | −5m56s | −9m56s |
| cold / 2 | 43699239820 | 43691601576 | −10m03s | −11m08s |
| exact / 1 | 43636915894 | 43638762436 | −3m35s | −4m09s |
| exact / 1 | 43702366735 | 43701262931 | −4m21s | −10m10s |
| exact / 1 | 43706373168 | 43707683987 | −2m52s | −2m05s |
| exact / 2 | 43704982617 | 43703735602 | −3m40s | −4m10s |
| exact / 2 | 43708755916 | 43710332141 | −8m22s | −7m39s |
| exact / 2 | 43712326996 | 43711325621 | −8m57s | −7m51s |

Warm **difference of candidate medians** is −4m33.5s required latency (−29.84%)
and −6m01.5s runner execution (−9.97%). The **median of paired differences**
is −4m00.5s and −5m54.5s respectively. All six paired warm comparisons favor R:
latency effects range −8m57s to −2m52s; runner effects −10m10s to −2m05s.
Cold median differences are −7m59.5s latency and −10m32s runner execution.

The owner adopted **Option B after Stage 1**: successful prospective exact-cache
replacement workflows must complete the primary clock in **strictly less than
900 seconds** and use **at most 3,600 runner-seconds**. The exact policy-message
timestamp is unavailable; approval preceded the Stage 2 authorization recorded
at 2026-11-15 07:36:30 UTC and was explicitly reaffirmed for this review.

All five prospective R observations passed: **43701262931, 43703735602,
43707683987, 43710332141 and 43711325621**. Their latency range was
9m27s–12m08s and runner range 50m21s–58m13s. Historical Stage 1 R warm also
passed, but is not prospective evidence for the later policy.

Historical master remains **42m03s** runner execution; its original nominal +10%
ceiling was **46m15.3s**. That policy was **not achieved**. R's warm median
54m23s remains 29.33% above this weaker-source-qualification benchmark. Option B
is an explicit acceptance-policy revision, not baseline redefinition or proof
that the old ceiling passed. It is not a GitHub-enforced aggregate billing cap.

The experimental unit is the whole workflow. There are only **two independent
cache-cohort pairs**; three warm repetitions share each cohort's state. Ordinary
image/CPU variation, time effects and the action-review pause are confounders.
Paired/cohort results are descriptive: no statistical significance, independent
six-pair inference, population reliability or future fleet guarantee is claimed.

### Qualification, compiler evidence and remaining cost owners

All **272 ordinary results and 32 final gates** passed, including both independent
guard executions in every observation. All four cold runs completed within
unchanged declared job deadlines. The active master ruleset still requires only
`Rust (ubuntu-latest)` from GitHub Actions app **15368**, with no bypass actors.
`CI qualification` is mandatory through that compatibility barrier; it is not
claimed to be independently required by a changed ruleset.

The six Rust work jobs remain independent:

| Job ownership | Job deadline | Mutable build/cache boundary |
| --- | ---: | --- |
| Source and provenance | 10m | Registry-only cache; committed source/URL/network checks |
| Rust correctness and parser regressions | 30m | Own workspace-debug target |
| Release, networking abort and benchmarks | 20m | Own release/abort/benchmark builds |
| Conformance execution | 10m | Own conformance target |
| Integration streaming guard | 25m | Own full-workspace guard target and mandatory equivalence/inventory audit |
| Exact-head streaming guard | 25m | Registry-only cache; detached verified head and separate temporary target |

Existing AG2, AG4 and all nine fuzz jobs remain independent gate prerequisites.
No production cache, compiler profile, timeout, permission, dependency graph or
qualification requirement was changed by the measurement campaign. The head
step keeps its 20-minute outer bound and sequential 10-minute compilation /
8-minute execution bounds, with remaining step/job headroom for source setup,
selection, evidence and cleanup. Native observations fit these bounds. On master
pushes only the head job may be skipped, without fabricated head outputs; all
remaining prerequisites qualify the actual pushed commit.

The previously demonstrated [negative integration experiment](#completed-hosted-negative-enforcement-experiment)
remains the hosted failure-propagation evidence. Head failure, cancellation,
timeout, unexpected skip, missing result and receipt mismatch are covered by
the existing real regression fixtures, not individually reproduced hosted.

All sixteen native observations have **410 equal default/HTML5 compiler
configurations and identical guard bytes within their lane comparison**.
Actual Rust 1.92.0 commit
`ded5c06cf21d2b93bffd5d884aa6e96934ee4234`, Cargo 1.92.0 commit
`344c4567c634a25837e3c3476aac08af84cf9203`, x86-64 Linux target, LLVM 21.1.3,
flags/features/profiles and installation inputs were checked across 256 compiler
jobs. Guard artifact identity remains runtime_parse / perf_regression / the
expected source and standard harness, with exactly one non-ignored test passing.
The two R workspace lanes remove exactly one guard each (2,964 → 2,963 records);
all other records, ignored-test policy and native-wrapper exclusions are intact.
Full-workspace guard compilation remains; no narrower or package-only recipe
was substituted.

The matched C1/R1 cold correctness jobs used image 20261111.127.1, Xeon 6975P-C,
four CPUs. Correctness fell 18m22s → 12m25s; workspace lanes explain 353s of
the 357s difference. Other jobs were not all hardware-matched. In the warm
dataset C lanes took 233–303s and R lanes 59–75s.

All five new R warm workflows were exact-head-job bound. Each head build still
compiled **453 units, including 346 dependency units**, despite exact cache
restoration; R warm head compilation took 336–449s. This isolated compilation is
a measured remaining cost owner, not a reason to change qualification or add
infrastructure before accepting the present result. Unit-duration sums overlap
and are not runner/CPU consumption. All 144 Cargo timing reports and job/step
timings remain archived. Warm cache restore steps summed 67–81s plus 0–4s post
steps; cold lookup/post steps summed 20–31s / 102–118s. No caching or scheduling
change is required by this review.

### Retrospective amendments and retained stops

1. **Fleet stop:** both Stage 1 cold runs passed qualification, then the original
   auditor stopped on runner image/CPU differences. On **2026-11-14 at
   20:18:02 UTC**, the owner retrospectively accepted ordinary hosted image/CPU
   revisions as recorded covariates. Source, platform class, compiler/flags/
   profiles/features, cache state and qualification stayed strict. Both cold
   observations and the original stop remain; neither was replaced.
2. **Action stop:** R2 cold 43691601576 passed qualification, then correctly
   stopped under the then-current action-SHA rule. On **2026-11-15**, approval
   recorded at **09:13:51 UTC**, the owner permitted only the proven
   `dtolnay/rust-toolchain@stable` pair
   `b725eda525362e7cf34af9a7243cac1e3f01a7ec` /
   `39008827e6b4319a349e9a42dfc90579dd25f1ca`.
   Complete upstream endpoint trees differ only in `.github/workflows/ci.yml`.
   Both execute `action.yml` blob
   `62252504fab44d94e1bf3a73df1c976b3dec35c4`, content SHA-256
   `0b52547c145ae18d8631c9ab711e4d917e9e489cea800368f338c25a56e0aee2`.
   Commands are inline and executable dependencies are unchanged. The source
   proof SHA-256 is
   `9c885e1dec2e1a4301186ae10190edc78e7663489d81891888866ffec9a5c32b`;
   auditor-diff SHA-256 is
   `591fd8b48509fa344bc868863a5abfed364e9abdb6841a835dbb1e99b70984ec`.
   Unknown actions, changed executable content/dependencies/modes, missing proof,
   altered inputs or compiler identities still fail. Thirty-one focused cases
   passed before continuation and again with fresh replay fixtures.

The action exception changed **only the temporary measurement auditor**. It
changed neither production nor experimental workflows and added no production
action-revision allowlist. Its fail-closed claims describe that analysis protocol.

R2's original failed audit, code/manifest/stop snapshots and **12m41s latency /
1h08m20s runner execution** are preserved separately from its successful
retrospective re-audit. No R2 rerun occurred. Full stop records, timestamps,
endpoint tree hashes and preserved-file fingerprints are in the structured
record and archive. Local packaging/replay setup errors are also retained:
the archived original auditor path and fresh exclusive fixture setup were
corrected without changing auditor/test logic or launching hosted retries.

### Resource closeout, archive integrity and reproduction

Stage 1 consumed **4h17m07s**, Stage 2 **12h01m47s** of its separate 18-hour
allowance, and the complete campaign **16h18m54s** of 22h17m07s. Unused Stage 2
allowance is **5h58m13s**; no unused Stage 1 allowance carried over. Each new
launch passed a **284 runner-minute** reservation.

At the closeout snapshot **2026-11-15 12:37:27 UTC**, storage was
23.575997 GiB / 32 GiB, leaving 8.424003 GiB; retention was seven days.
There are 64 experimental entries (13.178372 GiB) and 55 protected production
entries (10.397626 GiB). Estimated cumulative experimental storage spending was
**$0.021295**, leaving **$0.178705** of the $0.20 allowance; the conservative
additional estimate through the deadline was $0.041083. These are operational
estimates using compressed GiB-hours, not an invoice or enforced spending cap.
Later independent read-only review still found all 119 identities and the same
32 GiB/seven-day settings; this does not extend the storage window.

A pre-existing weekly master HTML5 Conformance run, 43710988949, created cache
10552787326 (145,023,492 bytes). Its uploaded bytes/key were verified, it matches
no experimental fallback prefix, and original protected/experimental entries
remain intact. This is an existing production schedule, not an additional
experimental trigger; that entry is protected from cleanup.

The preserved archive is **stage2-complete-evidence.tar.gz**, **56,320,403 bytes**,
SHA-256:

```text
882f9e88b22443b736284a189c2d7cd1a3e6cc362e5ae2422c644e2d7200321c
```

The final review reread all **2,706 manifest-listed files**, compared every
artifact's actual ZIP bytes to its SHA-256 and called ZIP CRC verification on all
**96 archives**. All passed; a metadata digest alone was not treated as CRC proof.
The live API corroborated all sixteen runs/checks and all 96 artifact digests.
Forty-four packaged-data cache/invariant/receipt audit replays passed. The
existing main qualification regressions contain 63 tests; control's historical
58 and replacement's 63 both passed always/never color before experiment
publication. Existing committed provenance was enforced normally.

The archive is locally retained under
`target/ci-1458-stage2-continuation-20261115/`, outside versioned source.
**Preserve a checksum-verified independent copy before cleaning targets or
worktrees.** No large raw archive is added to Git. Source/run/job IDs, cache
identities, native proofs, artifact hashes, raw timing inputs and statistics are
curated in the existing JSON so later review does not depend only on a prose
summary. GitHub logs/artifacts/caches have retention limits.

Verify the archive and actual ZIP contents without extracting executable files:

```sh
CI_1458_ARCHIVE=/absolute/path/to/stage2-complete-evidence.tar.gz
shasum -a 256 "$CI_1458_ARCHIVE"
python3 -I -B - "$CI_1458_ARCHIVE" <<'PY'
import hashlib, io, json, sys, tarfile, zipfile
from pathlib import Path
p = Path(sys.argv[1])
assert hashlib.sha256(p.read_bytes()).hexdigest() == "882f9e88b22443b736284a189c2d7cd1a3e6cc362e5ae2422c644e2d7200321c"
with tarfile.open(p, "r:gz") as t:
    actual = {}
    for member in t:
        assert member.isfile()
        raw = t.extractfile(member).read()
        actual[member.name] = hashlib.sha256(raw).hexdigest()
        if member.name == "SHA256SUMS":
            expected = {line[66:]: line[:64] for line in raw.decode().splitlines()}
    assert {k: v for k, v in actual.items() if k != "SHA256SUMS"} == expected
    records = json.load(t.extractfile("analysis/artifact-integrity.json"))
    assert len(records) == 96
    for record in sorted(records, key=lambda r: r["path"]):
        raw = t.extractfile(record["path"]).read()
        assert "sha256:" + hashlib.sha256(raw).hexdigest() == record["digest"]
        with zipfile.ZipFile(io.BytesIO(raw)) as z:
            assert z.testzip() is None
print("Manifest, 96 artifact digests and ZIP CRCs verified")
PY
```

For independent arithmetic, use each observation's `run_created_at`,
`required_check_completed_at` and per-job `started_at` /
`completed_at` in the JSON. Subtract timestamps, sum all nineteen jobs, group
by candidate/cache condition and apply `statistics.median`, `min`, `max`.
Pair by cohort number and warm repetition, not by matching favorable hardware.
The archive also retains the raw API payloads and logs under
`continuation/<observation>/`; fetch later metadata with:

```sh
gh api repos/joris97jansen/borrowser/actions/runs/RUN_ID
gh api 'repos/joris97jansen/borrowser/actions/runs/RUN_ID/jobs?per_page=100'
gh api 'repos/joris97jansen/borrowser/actions/runs/RUN_ID/artifacts?per_page=100'
gh api 'repos/joris97jansen/borrowser/commits/HEAD_SHA/check-runs?per_page=100'
```

Replace the recorded RUN_ID/HEAD_SHA tokens; GitHub APIs are read-only here.

From the repository root, replay the durable timestamp arithmetic independently
of the precomputed statistics (this is not full CI qualification):

```sh
python3 -I -B - <<'PY'
import datetime as dt, json, statistics
from pathlib import Path
record = json.loads(Path("docs/ci/1458-parallel-evidence.json").read_text())["final_acceptance"]
stamp = lambda value: dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
groups = {}
for row in record["observations"]:
    latency = (stamp(row["required_check_completed_at"]) - stamp(row["run_created_at"])).total_seconds()
    runner = sum((stamp(j["completed_at"]) - stamp(j["started_at"])).total_seconds() for j in row["jobs"].values())
    assert latency == row["required_seconds"] and runner == row["runner_seconds"]
    groups.setdefault((row["condition"], row["candidate"]), []).append((latency, runner))
for key, values in sorted(groups.items()):
    print(key, len(values), [
        (statistics.median(v[i] for v in values), min(v[i] for v in values), max(v[i] for v in values))
        for i in (0, 1)
    ])
PY
```

Archived one-off auditors retain original historical-path references and their
31-case runner creates exclusive fixtures. Replaying those scripts requires
the recorded baselines/path mapping and a fresh fixture directory; they are
analysis artifacts, not a permanent CI framework or a portable production tool.

### Documentation-candidate validation during final review

The 2026-11-15 review independently checked the archive bytes, all 96 ZIP CRCs,
raw timestamp arithmetic, sixteen source/cache/artifact reference sets and
finite/unique-key JSON. The repository has no separate evidence JSON Schema;
schema-1 and final-record shape/type/reference contracts were checked directly.
The documented archive-verification and timestamp-recalculation programs were
executed successfully. Markdown fences/tables, new/changed local references and
all eleven documented Bash blocks passed structural/syntax checks. One unrelated
cutover link to `../crates/html/src/parser.rs` was already broken in HEAD and
remains unchanged; no new evidence reference is broken.

With the repository's Rust/Cargo 1.92.0 on macOS arm64:

```sh
CARGO_TERM_COLOR=always python3 -I -B tools/ci/tests/test_ci_qualification.py
CARGO_TERM_COLOR=never python3 -I -B tools/ci/tests/test_ci_qualification.py
python3 -I -B tools/ci/check_dependency_subtrees.py
make check-network-dependencies
make fmt-check
git diff --check HEAD
```

Both qualification suites passed **63 tests** (44.178s / 41.466s); committed
source/history, network provenance, formatting and diff checks passed. Existing
critical qualification inputs remain committed; no preflight was bypassed.
These are working-tree documentation checks and regressions against unchanged
workflow programs, not full qualification of an unpublished documentation
commit. No new local `make ci`, runtime smoke or hosted workflow was launched.
The sixteen native full-CI observations qualify published R; a new documentation
commit still needs its own hosted checks before merge.

### Exit criteria and remaining authorization

| Original criterion | Final architectural/evidence assessment |
| --- | --- |
| 1. Reproducible baseline and optimized timings | Satisfied for published implementation: historical baseline plus fixed controlled cold/warm data, raw timestamps, immutable IDs and checksummed archive. |
| 2. Largest latency sources identified | Satisfied: duplicate guard execution explained correctness latency; isolated head compilation is now the warm critical-path owner in all five prospective samples. |
| 3. Eliminated duplication proven equivalent | Satisfied: native full-lane 410-configuration/hash equality, exact guard identity and complete per-lane inventory proofs remain fatal before qualification succeeds. |
| 4. Correctness/conformance coverage intact | Satisfied: all other workspace tests, profiles, assertions, source qualification, native/browser, release, provenance and fuzz qualifications remain. |
| 5. Applicable Actions checks pass | Published R and all sixteen controlled contexts passed. A separately authorized documentation commit must receive its own complete hosted qualification before merge; it is not covered by old green checks. |
| 6. Measurable latency reduction and accepted cost | Satisfied for the fixed campaign under explicit Option B; original +10% remains unmet. Finite cohort and future-fleet limitations remain explicit. |
| 7. Boundaries/caching/timeouts documented | Satisfied by the contracts above and structured job/deadline/source records; no unsafe shared target or binary promotion. |
| 8. Independent engine defect reported separately | Satisfied by the concrete unchanged-guard profile/proposed parser finding below. Native Linux attribution remains separate; it is neither hidden nor made a CI-engine-change prerequisite. No new issue is created here. |

The implementation is technically acceptable without another architecture,
dependency, engine or caching change. No ownership boundary, weakened workload,
unexpected omission, permissive failure path or speculative infrastructure
blocker was found. `docs/engine-feature-gap-tracker.md` needs no update for this
CI evidence; no browser feature status changed.

The remaining decisions are distinct:

1. Review and separately authorize the documentation commit and PR-description
   publication; run committed provenance and full hosted checks on its actual
   head and synthetic integration source. This uncommitted documentation review
   is not that qualification.
2. Preserve the raw archive independently. Review exact cleanup of the **64
   experimental cache IDs** recorded in JSON; preserve all protected entries.
   Experimental PR/branch cleanup and settings restoration need separate
   authorization. No cleanup has occurred.
3. The absolute storage deadline remains **2026-11-16 07:49:55 UTC**. No
   extension or automatic rollback is authorized. Even after experimental cache
   deletion, recorded production usage **10.397626 GiB exceeds 10 GiB**; restoring
   that limit may evict production caches and requires an explicit decision
   after refreshing usage.
4. Separately authorize a merge commit preserving the published PR history only
   after final review and required checks pass; do not squash, rebase or rewrite
   those commits. Verify the actual pushed master commit and receipts,
   all sixteen non-head ordinary qualifications, the intentionally skipped
   head-only job and both final gates after merge.
5. Decide closure of #1458 after the published documentation, final merge and
   post-merge evidence are verified. The independent parser finding remains
   separately documented; optional head-compilation efficiency work is not an
   architectural blocker or a new implementation-phase issue.

**Review verdict: Not closeable yet.** Performance acceptance is complete under
the approved policy; documentation publication, final hosted qualification,
merge/post-merge verification and closure remain separately authorized work.


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
