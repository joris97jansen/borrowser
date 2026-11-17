# Independent HTML allocation qualification — #1471

Status: implementation under architectural review; no committed candidate,
published branch, hosted manual execution or post-merge scheduled qualification
is claimed by this record. The hosted acceptance gates below remain outstanding.

## Scope and frozen identities

- Issue: [#1471](https://github.com/joris97jansen/borrowser/issues/1471).
- Implementation branch: `fix/1471-run-html-allocation-guards-independently`.
- Verified master/base: `5d0fd878a8e954f4fd6047d8594b8ea113e32ea9`.
- Base tree: `979aa65cf7cbd7bb946c04169329c5bbd8dc2943`.
- Independent [PR #1470](https://github.com/joris97jansen/borrowser/pull/1470)
  source: `2b2076fa1a142ab54d26e824f4ed208bcb902868`.
- That source's tree: `bfc75a35f7153a1988a5847bd2933f53892823c3`.

The implementation branch originates from the verified base, not #1331's branch.
No parser code, allocation instrumentation, fixture, threshold, assertion,
manifest or lockfile is changed. #1331's allocation correction, #1469's CPU work
and the separate heavy-performance parse-error expectations remain independent.
No engine feature-gap or browser-engine ownership contract changes here.

## Existing hosted defect evidence

[Scheduled run 43848648702, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/43848648702)
tested the base above. [Job 133235204844](https://github.com/joris97jansen/borrowser/actions/runs/43848648702/job/133235204844)
reported 934 passed and three failed heavy guards, exiting 101. The existing
zero-parse-error expectations failed for whole HTML, large HTML and adversarial
rawtext. Allocation execution, Criterion and benchmark upload were then skipped
under the default successful-prior-steps condition. Tokenizer fuzzing succeeded
in its separate job. This run demonstrates the original defect, not successful
allocation qualification.

## Workflow graph and source selection

`Perf` retains manual dispatch and daily `0 3 * * *` scheduled execution. It keeps
`perf-${{ github.workflow }}-${{ github.ref }}` and `cancel-in-progress: true`.
The three jobs are independent; none depends on another:

- `html_bench`: existing heavy guards, Criterion command and benchmark upload,
  with only the former allocation step removed;
- `html_alloc_guards`: complete serial allocation target on Ubuntu;
- `html5_tokenizer_fuzz_long`: unchanged.

Only allocation checkout can use the single optional dispatch input
`allocation_source_sha`. Empty input uses the immutable event SHA; schedules
always use that SHA. A supplied input is accepted only for manual dispatch and
must be exactly 40 lowercase hexadecimal characters before checkout. Unavailable
commits fail checkout/verification. Repository selection is fixed to the workflow's
repository. Heavy guards and tokenizer fuzzing still use the event revision.

Required allocation command, with `RUST_TEST_THREADS=1`:

```sh
cargo test -p html --test alloc_guards --locked \
  --features html5,count-alloc -- --test-threads=1 --nocapture --format pretty --color never
```

No test-name filter or ignored-test selection is used. `--nocapture` exposes
measurements already emitted by the tested source; #1470 contains aggregate and
phase allocation measurements. The workflow does not retrofit instrumentation
into older commits. Neither earlier macOS measurements nor ordinary green PR CI
constitute native Linux execution of this allocation target.

Before execution, the same Cargo target/features are invoked with
`-- --list --format terse --color never`. Listing compiles but does not run tests.
The six existing allocation guard names are mandatory, including
`streaming_parser_chunk_sizes_do_not_cause_pathological_alloc_growth`. The sorted
listing must exactly match the test names in the native execution records. The
single final libtest summary must report that discovered number passed, with
zero failed, ignored, measured or filtered tests. Pretty/color settings stabilize
the pinned Rust 1.92.0 output; `--nocapture` and serial execution are preserved,
including measurements interleaved between a test's name and its final `ok`.
Execution-name extraction accepts both ordinary prefixes and the native
`test NAME - should panic ...` prefix, retaining only the exact test identity.
Expected-panic diagnostics/backtraces can appear before `ok`. Rust 1.92's built-in
`#[test]` expansion sets `compile_fail` and `no_run` to false for this standard
Cargo integration target, so rustdoc's `compile fail` and `compile` mode
annotations are not supported or needed here. See the versioned
[test expansion](https://github.com/rust-lang/rust/blob/1.92.0/compiler/rustc_builtin_macros/src/test.rs#L331)
and [test modes](https://github.com/rust-lang/rust/blob/1.92.0/library/test/src/types.rs#L228).

This is a target-specific inline check of native libtest output, not a new test
runner or general report parser. The required names protect existing guards;
the total count is not fixed at six. Future additions to the target must also
appear in execution records and pass. Missing/empty listings, zero-test runs,
ignored/filtered guards, missing execution records or summaries, and mismatched
counts fail the allocation step even when Cargo itself returns zero.

## Identity, build output and evidence lifecycle

The job initializes unique build and evidence directories under `$RUNNER_TEMP`
before any Cargo/cache operation. `CARGO_TARGET_DIR` points to that fresh build
directory. Test logs and receipts are outside the checkout as well. No ignore
rules, checkout resets, or deletion of unexpected source changes are used to
manufacture cleanliness. The hosted runner owns temporary-directory cleanup;
the uploaded evidence is separate from those ephemeral local paths.

Before execution, the job checks the workspace root, selected commit, actual
HEAD, matching commit/tree identity, and clean tracked/index/untracked state.
It records the workflow definition SHA/ref, event name/ref/SHA, requested and
selected source, actual checkout SHA/tree, build/evidence paths, and active
Rust/Cargo. The installer explicitly selects 1.92.0; verification also checks
the checked-out toolchain pin and actual Rust/Cargo versions.

Cargo first compiles/lists `html/alloc_guards`, then executes the complete target
from that same verified checkout with identical features and the same fresh
target directory. No qualification workload executes during listing. Capture
compilation and the native `Running` executable path; no previously built
executable is promoted or downloaded. Default-source runs may use registry-only
Cargo caching, without target or Cargo-bin caching. Explicit source overrides skip the cache action
entirely, restoring and saving no Cargo caches.

The final identity check uses
`!cancelled() && steps.source_before.outcome == 'success'`, so allocation failure
does not suppress it. It rechecks HEAD, the original tree and source cleanliness.
`source-after.txt` records the allocation step's outcome separately from source
integrity. The allocation step succeeds only after Cargo and the complete native
inventory/summary checks succeed. Only that outcome plus successful final identity
checks can produce `qualification-success.txt`. A failed, skipped, cancelled or
timed-out execution is not successful qualification. If cancellation or timeout
prevents final verification, the evidence is incomplete.

Available evidence is uploaded even after failure, provided temporary-path
initialization succeeded. Its artifact name identifies selected source, run ID
and attempt. `set -euo pipefail` preserves Cargo's failure through `tee`; neither
final source verification nor upload clears a failed allocation step. A heavy
failure still fails the overall workflow even when allocation qualification passes.

## Trust boundary

Manual overrides are for explicitly reviewed source commits, including the frozen
#1470 commit above. SHA syntax validation prevents ref ambiguity and shell-input
injection; it does not certify source as trustworthy. Cargo manifests, build
scripts and tests execute source code. This is not an arbitrary-code sandbox.

The new job has only `contents: read`, does not persist checkout credentials,
does not map secrets or use a deployment environment, and exposes no configurable
repository, branch, command or executable. Inputs are passed through environment
variables and quoted shell references. An override changes allocation source
only, not the workflow definition or other jobs' source.

## Four-level validation and CI enforcement

Targeted development command:

```sh
python3 -I -B -m unittest discover \
  -s tools/ci/tests -p test_perf_qualification.py -v
actionlint .github/workflows/perf.yml
git diff --check
```

The regression tests extract and execute the actual inline workflow programs in
disposable Git repositories. Cargo fault fixtures verify shell failure propagation
and receipts; they are explicitly not native allocation-guard qualification.

Required committed hosted CI discovers the new tests through:

```text
Source and provenance (source_provenance)
  make check-url-dependencies
    python3 -I -B tools/ci/check_dependency_subtrees.py --tests
      unittest.defaultTestLoader.discover("tools/ci/tests")
```

Failure propagates through `CI qualification` to `Rust (ubuntu-latest)`.
Local targeted execution covers working-tree inputs; it does not establish
committed-source CI qualification. The existing preflight intentionally rejects
uncommitted tooling tests. It is not weakened for development. `make ci` remains
the local complete-CI equivalent; hosted CI of the eventual committed candidate
must show these named tests actually running.

Subsystem review must verify job independence, unchanged triggers/concurrency,
heavy/Criterion/fuzz preservation, active compiler, cache behavior and receipts.
Browser startup and visual snapshots are unnecessary for this workflow-only change.

## Local implementation evidence — 2026-11-17

These are precommit working-tree observations on macOS 27.0.1 ARM64, not hosted
Linux evidence or qualification of a committed #1471 workflow definition.

| Check | Observed result |
| --- | --- |
| Updated focused Python suite | 43 passed; the original 39 remain, with positive expected-panic/interleaving coverage and rejection of missing/ignored expected-panic guards and dirty source after expected-panic success |
| actionlint v1.7.12 | Passed for `perf.yml`; installed outside the repository |
| Workflow preservation review | Heavy/Criterion/upload job equals the base with only its allocation step removed; tokenizer job and concurrency are unchanged; exactly one job/input added |
| Bash syntax and diff checks | All multiline workflow programs passed `bash -n`; tracked and new files passed whitespace checks |
| Earlier broader tooling discovery attempt | 182 executed tests passed; `DependencySubtrees.setUpClass` was blocked by its existing rejection of the then-untracked new test file; the overall run exited 1 and is not a passing complete suite |
| Earlier `make ci` | Exited 2 at the first committed-source preflight: `untracked qualification test/helper input: tools/ci/tests/test_perf_qualification.py`; no preflight was weakened |
| Unchanged base allocation target | Compiled in an external fresh target directory; five passed, one failed, zero ignored/filtered; the original streaming assertion reported 743,350,368 versus 16,249,548 bytes; exit 101 |
| Corrected local exact-#1470 execution | All six passed, zero ignored/filtered; native listed/executed inventories and summary matched; actual inline before/after checks passed for its recorded SHA/tree, using a fresh external target directory |
| Native expected-panic formatter fixture | Rust 1.92.0 ran seven small tests successfully with an expected panic and `--nocapture` backtrace; replay through the actual inline workflow programs matched all seven identities and gated the receipt on final source verification; this is formatter evidence, not HTML allocation qualification |

The positive local execution used a disposable shared clone detached at the
frozen #1470 commit. Its source remained clean. It ran the actual working-tree
workflow's path initialization, selection, before check, toolchain check,
allocation command and after check. The local driver supplied synthetic event
metadata and explicitly labelled the workflow definition
`uncommitted-working-tree-definition`. It performed no GitHub dispatch, cache
action, upload or PR check. Rust/Cargo were 1.92.0 on
`aarch64-apple-darwin`, LLVM 21.1.7.

| Chunk bytes | Allocation events | Cumulative allocated bytes | Reallocations |
| --- | ---: | ---: | ---: |
| 1 | 70,276 | 10,879,176 | 64 |
| 1,024 | 64,332 | 10,013,408 | 365 |

Both schedules emitted 12,007 patches; the original assertions passed unchanged.
Aggregate/phase measurements and Cargo's executable path were retained outside
the source. Initial local raw evidence is under
`/private/tmp/borrowser-1471-validation.GB2QOc/`, including
`local-exact-source-integration.log` and `local-job-runtime/`. Those paths are
ephemeral; the observations above do not replace the mandatory hosted records.

### Required inventory correction

The actual #1470 libtest executable returned exit 0 for an unmatched exact filter:
`0 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out`. With that successful
status and clean source, the original final-check program created a success
receipt despite no tests executing. This demonstrated the architectural finding
without changing the allocation test source.

The corrected allocation-step program rejects that successful-but-incomplete
output before its outcome can permit a qualification receipt. Focused fixtures
also reject an ignored streaming guard, a missing streaming execution record
even with a superficially successful summary, an absent required listing entry,
filtered execution, missing/duplicate summaries, incorrect counts and listing
failure. A complete seven-test fixture passes, demonstrating that legitimate
additions remain supported. Cargo failure and dirty-source cases remain enforced.

Correction evidence is under `/private/tmp/borrowser-1471-completeness.35Iprb/`,
including `native-listing.txt`, `native-zero-tests.txt`,
`corrected-exact-source-integration.log` and `corrected-runtime/`. The local driver
still labels its workflow definition uncommitted and uses synthetic event
metadata. No hosted input acceptance, upload, workflow conclusion or PR check is
claimed by these local observations.

### Expected-panic formatter correction

A genuine Rust 1.92.0 fixture with six empty guard-name stubs and one additional
`#[should_panic(expected = "fixture allocation unwind")]` test passed all seven
tests. It emitted `test NAME - should panic ...` followed by panic diagnostics,
a backtrace and `ok`. The old extraction expression found only six execution
names. The correction adds only the optional literal ` - should panic` segment;
the name capture, inventory comparison, dynamic count and summary predicate are
unchanged. There is no additional test runner or production executable.

The focused positive fixture follows that native transcript shape, normalizing
the PID/path and trimming backtrace frames. The complete genuine transcript was
also replayed through the actual allocation-step program in the existing Git and
Cargo fixtures: seven discovered/executed identities matched, allocation succeeded,
and no success receipt existed until final clean SHA/tree verification passed.
Missing expected-panic execution, ignored expected-panic tests and dirty source
after otherwise successful execution are explicitly rejected. All earlier
negative cases remain enforced.

Native source, compiler identity, listing, execution transcript and fixture-replay
evidence are under `/private/tmp/borrowser-1471-expected-panic.yZn2TS/`. These small
Rust tests establish native formatter behavior only; their empty name stubs do
not execute or qualify production HTML allocation guards. Transcript replay is
fixture-based validation, not hosted execution or allocation performance evidence.

## Mandatory hosted manual acceptance — outstanding

After separate publication and execution authorization, run the reviewed #1471
definition with empty input and then with the frozen #1470 source override.
Run them sequentially: a later run on the same event ref can cancel an earlier
one under the preserved concurrency policy. Record actual default-source results;
the uncorrected base is expected to fail its allocation regression.

Exact-source invocation, not executed as part of implementation:

```sh
gh workflow run perf.yml --repo joris97jansen/borrowser \
  --ref fix/1471-run-html-allocation-guards-independently \
  -f allocation_source_sha=2b2076fa1a142ab54d26e824f4ed208bcb902868
```

Acceptance requires all of the following in actual GitHub-hosted Linux evidence:

1. GitHub accepts the optional input for the intended reviewed definition.
2. `github.workflow_sha` equals that definition's reviewed revision; workflow ref
   and event ref/SHA are recorded independently.
3. Allocation HEAD/tree match the frozen #1470 identities above, and compilation
   and execution use that clean source and a fresh external target directory.
4. The complete target reports six passed, zero failed/ignored/filtered tests,
   with both chunk schedules' allocation events, bytes, reallocations and phases.
5. Final source verification succeeds and evidence is retained with run/attempt,
   job URLs, runner/compiler identities, logs and artifact references.
6. Heavy performance remains independently failed and visible, and the overall
   workflow reports failure. Allocation success must not be called overall success.

The run belongs to its actual event revision. It must not be described as a check
on PR #1470 unless GitHub actually reports that association. Exact-source evidence
can qualify its allocation behavior without changing its approved commit or PR.

## Post-merge scheduled acceptance — outstanding

Inspect an actual `schedule` run of the merged definition. Verify the allocation
job checked out that run's immutable default-branch event SHA and reported its own
conclusion independently of heavy tests. Record actual results; master may retain
the allocation regression until #1470 is integrated. Manual dispatch cannot prove
scheduled execution. Cancelled, timed-out or incomplete runs cannot satisfy this
gate. #1471 is not closeable until required hosted manual and scheduled evidence
is recorded and reviewed.
