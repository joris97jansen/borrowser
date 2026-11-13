# CI critical-path evidence and qualification — #1458

Status: local, uncommitted evidence-collection candidate. No optimized A/B
architecture is selected. No workflow candidate has been pushed or executed on
GitHub, and no repository ruleset has been changed. Issue #1458 remains open.

Baseline master was freshly fetched and verified clean before creating
`chore/1458-optimize-ci-critical-path`:
`144ef3bc887ac5688ac7f6b123a6e98f1e1485f2`.

## Qualification boundaries

The confirmed requirement is actual execution of the streaming guard on both
the exact PR head and the synthetic integration commit. Remaining existing
qualification runs on the integration commit. Master pushes qualify the actual
pushed commit. A recorded head SHA alone does not qualify its source.

The candidate preserves both original workspace test commands, including their
existing native-wrapper exclusion. Neither streaming-guard execution has been
removed, ignored, filtered, moved to nightly, or replaced by another profile.
The additional head execution makes this an expanded-qualification control,
not the optimized serial A or parallel B result.
It runs after all existing Rust qualification, retaining the original relative
execution order and allowing its original step timings to be compared directly.

All normal jobs explicitly check out `github.sha` and verify the executed commit.
Rust additionally verifies synthetic merge parents against the event's base and
head SHAs and rejects a dirty source checkout. AG2 retains its existing native
identity/environment verification.

For head execution, Rust creates a detached worktree from the event's exact head
SHA, verifies its commit and clean contents before and after execution, and
records its tree. It verifies that the head commit object is available before
creating the worktree, and removes the verified clean worktree only after
successful execution. A separate temporary target directory prevents mutable build
directory sharing with integration qualification. The existing registry cache
can supply dependencies; head build artifacts are not transferred from another
job or cached by a newly introduced mechanism.

The existing `fetch-depth: 0` integration checkout fetches the synthetic merge's
ancestry, including a fork head that is its second parent. No fork remote or fork
write credential is needed by the head step. A real Git fixture verifies the
fork head is absent after fetching ordinary base branches, then available after
a targeted merge-SHA fetch, with only the base origin configured. This matches
[checkout@v4's all-history and targeted-fetch behavior](https://github.com/actions/checkout/blob/v4/src/git-source-provider.ts).
Actual GitHub fork-event permissions and checkout remain a hosted verification
requirement. Missing objects, mismatched parents and source mismatch fail; the
candidate does not substitute a different revision or fall back to integration.

The head command keeps workspace package/target selection and feature
unification. Existing Cargo/libtest functionality is sufficient:

```sh
cargo test --workspace --lib --bins --tests --examples --locked \
  --no-run --message-format=json
cargo test --workspace --lib --bins --tests --examples --locked \
  runtime_parse_linear_streaming_guard -- --exact --list --format terse
```

The workspace listing must contain exactly one expected ordinary test record.
Cargo's build artifacts additionally bind the executable to package
`runtime_parse`, its exact manifest, integration-test kind `test`, target
`perf_regression`, source `tests/perf_regression.rs`, and test profile. The
package manifest must also retain the standard libtest harness for this target:
the implicit default and explicit `harness = true` are accepted; `harness = false`
is rejected. Custom harnesses on unrelated targets are unaffected. The executable
must be inside the isolated head target directory. That identified
artifact is listed again and executed directly, from `crates/runtime_parse`,
with `runtime_parse_linear_streaming_guard --exact --nocapture --format pretty
--color never`. It must report the expected test passing with zero ignored
tests; process success alone is insufficient. `pipefail` preserves failure
through `tee`. Unrelated targets, diagnostics and summary lines cannot substitute
for execution of the bound integration-test artifact.
Unrelated tests in the bound target may appear as filtered-out tests; they are
not additional selected tests and do not invalidate the one successful guard.

Zero selection, ignored or multiple matching tests, compilation failure, runtime
failure, source changes, timeout and cleanup failure cannot publish completed
head qualification. The output is emitted only after execution, source checks,
bounded worktree cleanup and confirmation that integration remains unchanged.
Build outputs stay isolated in runner-temporary storage; failure retains the
worktree and logs for diagnosis. All other tests remain in their existing
integration qualification lanes.

No new Python qualification helper, Makefile target, Rust API, production data
structure or dependency is introduced. The new tests execute the literal
workflow programs, real Git worktrees and real Cargo/libtest fixtures. Their
deadline-command fixture injects expiry without waiting minutes. A separate
Linux arm64 probe also exercises real GNU timeout 9.4 against the actual workflow
program with only the fixture's execution ceiling shortened to one second: exit
124, no head output, retained head worktree and unchanged integration checkout.
Actual GitHub cancellation/deadlines and native cold-cache capacity remain pending.
Tests are admitted through the existing committed `tools/ci/tests` provenance
contract. No verifier policy or source-inventory baseline is relaxed.

### Review reproduction and confirmed defects

With repository-pinned Rust 1.92.0, the exact original production listing command
ran 84 Cargo test targets on macOS arm64. Its stdout was exactly
`runtime_parse_linear_streaming_guard: test` plus a newline. There were no
per-target count summaries in this `--list --format terse` output. The concern
that this particular pinned command inherently breaks the whole-stdout check
was not reproduced. The expanded real-Cargo fixtures confirm the same listing
semantics on Linux arm64 and separately demonstrate the many zero-test summaries
produced by normal multi-target execution without `--list`.

A different selection defect was reproduced: with the guard name present only
in a package's unit-test target and absent from `perf_regression`, the previous
step returned zero and published `head_sha`. Checking stdout alone did not bind
the package or integration target. Artifact binding and direct execution correct
that false positive. Tests also cover a sole same-named test in another package
or integration target and diagnostics impersonating the listing.

A subsequent review reproduced a custom-harness substitution on pinned Rust
1.92.0. A disposable workspace declared `runtime_parse`'s `perf_regression`
integration target with `harness = false`. Its `main` contained no guard `#[test]`
and printed fabricated listing and success records. The actual preparation and
execution programs accepted it and published `head_sha` before the correction.
Cargo still reported `profile.test = true` and the expected package, target and
source path; these artifact properties alone do not establish a libtest harness.
The correction checks the matching manifest target's `harness` property before
binding the executable. The negative regression failed before this correction
and passes afterward. Positive regressions retain implicit and explicit standard
harnesses, unrelated filtered tests and an unrelated custom-harness target.
Reproduction details are recorded under `custom_harness_review` in the local
evidence JSON. This is a qualification fix, not a performance optimization.

A false rejection was also reproduced: a correct guard passed with one unrelated
test filtered out in its integration target, but the previous summary check
required zero filtered tests. The corrected check still requires exactly one
pass, zero failures and zero ignored tests in the bound artifact, while allowing
the irrelevant filtered-test count. No guard assertion is changed.

The previous 15-minute step ceiling was shorter than its sequential ten-minute
and eight-minute inner ceilings. The explicit 20-minute correction and its
provisional end-to-end limits are detailed below. Neither correction is a CI
performance result; A and B remain undecided. Structured reproduction data is
recorded under `review_reproduction` in
[1458-local-evidence.json](1458-local-evidence.json).

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

## Gate, cache and timeout contracts

`Rust (ubuntu-latest)` keeps its existing identity and 45-minute deadline.
The native equivalence collection is bounded by ten minutes. The head step
ceiling is explicitly increased from 15 to 20 minutes to fund sequential phase
limits rather than silently clipping their sum:

| Head phase | Included work | Ceiling |
| --- | --- | --- |
| Preparation | Commit availability, worktree creation, source verification, full workspace compilation, workspace listing, artifact binding/hash | 10m |
| Execution | Bound target listing, guard execution, successful-result and head-source verification | 8m |
| Cleanup | Remove the clean temporary worktree; no forced removal | 30s |
| Kill grace | Ten seconds for each of the three bounded phases | 30s total |
| Remaining margin | Initial/final integration checks, evidence writes, output publication and Actions overhead | 1m |
| Outer head step | All of the above | 20m |

Completed subphase timings are recorded in `pr-head-timings.txt` for worktree and
identity, compilation, workspace/target selection, artifact binding, execution
and worktree cleanup. Incomplete phases are not fabricated as successful timing
records. GitHub's step duration still records failed or interrupted work.

The known Linux guard durations are approximately 210–218s. The 244s warm-cache
debug build is not a measurement of the isolated head's cold compilation.
Native cold/fallback/exact-cache head build and cleanup costs remain unmeasured;
these ceilings are provisional control limits, not qualified capacity estimates.
The original 30-second inter-event test timeout is unchanged.

The effective head limit is also constrained by the remaining 45-minute Rust
job budget: the 20-minute step ceiling does not grant additional job time. The
historical 26m49s and 35m36s Rust runs leave only 18m11s and 9m24s respectively,
before accounting for new collection overhead. Serial head qualification may
therefore time out at the job boundary even when a phase is within its ceiling.
Hosted measurements must establish viability; no Rust-job deadline increase or
successful serial-control qualification is claimed here. Additional head work
can increase PR latency materially and is not presented as an improvement.

`CI qualification` depends explicitly on all twelve normal qualification jobs.
It executes with `always()` for PR/master qualification, has a two-minute
deadline and accepts only `success`. Missing/unexpected prerequisites, absent
results, failure, skipped, cancelled, timed-out and neutral results fail. Rust
must also publish matching integration/head completion identities. The separate
AG4 proxy-timing push experiment is intentionally outside normal PR gating.

The active master ruleset currently requires only `Rust (ubuntu-latest)`.
This candidate cannot claim the final gate is mandatory until an authorized
administrator adds **CI qualification** with the GitHub Actions integration and
reads back enforcement. Keep Rust required. If B is selected later, independently
require its guard check as well, verify failure/cancellation/skipping/missing-check
behavior, and only then transfer its replacement-covered execution. Do not use
`workflow_dispatch` results as substitutes for eligible PR status checks.

No cache configuration or AG2 job split is introduced. Existing debug/release,
panic, feature, provenance and fuzz contracts are retained.

## Remaining controlled experiments and selection

Hosted experiments require separate authorization to commit and push a
reviewable candidate. No hosted candidate results exist yet.

1. Execute this expanded control on a PR. Obtain native lane artifact records,
   hashes, assertions/selection inventory and actual head/integration identities.
2. Establish cold Cargo runs with fresh job-local Cargo/target directories and
   no restored cache, then seed explicit per-job/per-configuration namespaces
   and re-run warm. Separate exact hits from fallback restores. Record action
   revisions, runner image/CPU, locks and complete step/cache overhead.
3. Retain at least three warm and two cold samples for each viable candidate.
   Compare identical engine/test sources, profile, instrumentation and source
   contexts. Interleave samples; report individual values, median and range.
4. Evaluate A: one equivalent guard execution per source context inside Rust.
   For each lane prove its filtered inventory loses exactly the intended guard
   and no other test. A repeated binary name or local arm64 result is insufficient.
5. Evaluate B concurrently with Rust, including its own compilation/cache cost
   and both source contexts. Until execution and independent mandatory gating
   are proven, existing mandatory executions remain. An additive rollout can
   precede enforcement; all phases stay within issue #1458.
6. Compare C only for concrete measured compilation/test overhead. Retain all
   executions if equivalence cannot be established. No profile/dependency upgrade,
   generic orchestration layer, cross-job binary transfer or mutable shared
   target directory is an acceptable substitute for proof.

The approved decision thresholds are a median latency reduction beyond observed
variation, at least 60s additional median benefit for B over the qualified serial
alternative, and runner consumption within 10% of both refreshed baseline and
qualified serial alternative. Disclose the added head qualification separately
and compare A/B with identical coverage. Include the final gate, setup, cache
post-steps and uploads in total cost. A failed threshold is not a successful
optimization. At present neither A nor B has an accepted performance result.

## Local validation and closeability

```sh
python3 -I -B tools/ci/tests/test_ci_qualification.py
```

The suite exercises actual workflow blocks, real Cargo/libtest and isolated
fixture-only Git commits. It does not commit the Borrowser working tree. It
checks source isolation, ordinary/ignored/zero/duplicate selection, build/test
failure, injected timeout, unequal compiler evidence and the complete gate
result matrix. No repository tests gain ignore/skip annotations.
The previous 33-test suite includes multiple real Cargo packages/targets, fork
merge reachability, exact merge parents, mutable-head rejection and cleanup
failure, in addition to the original gate and deadline cases. All 33 tests are
retained; three focused harness-property regressions bring the suite to 36 tests.
Local and offline Linux arm64 container runs both pass all 36 tests. They validate
workflow mechanics only, not native Linux x86-64 performance. The earlier actual
lane-collection program also passed against this workspace locally.

Actionlint 1.7.12, with its published archive checksum verified, validates the
workflow with `-shellcheck=`; Bash program syntax and execution fixtures supply
additional local coverage. No new lint dependency is added to the repository.
`make fmt-check` and `git diff --check` pass. An original-command inventory
comparison confirms every original CI step remains in order, including unchanged
workspace test, conformance, release and fuzz commands.

`make ci` is required before close and final hosted qualification is mandatory.
The uncommitted candidate intentionally cannot pass the existing committed-input
provenance preflight: the workflow and new test source differ from HEAD. Do not
weaken that preflight or invent a qualified hosted result. Browser visual smoke
is unnecessary for these tooling-only changes; native AG2 and release-abort
runtime qualifications remain required on their supported environments.

The review rerun of `make ci` exits 2 with `staged qualification tests differ from
committed candidate`. The pre-existing staged candidate was left intact; review
corrections are working-tree changes. Review `git diff HEAD` to include both the
staged candidate and these corrections. No commit, push, PR or ruleset change
has been made.

Issue #1458 is not closeable: controlled cold/warm A/B measurements, native
equivalence/selection proof, hosted failure handling, mandatory-gate activation,
final full CI and a measurable accepted critical-path reduction are outstanding.
