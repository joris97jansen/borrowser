# Independent HTML allocation qualification — #1471

Status: implementation PR #1472 is merged. Hosted manual qualification and actual
post-merge scheduled qualification are complete; technical exit criteria are
satisfied. Three independent heavy-performance failures remain unresolved, and
the complete Perf workflows retained failure conclusions. See the GitHub issue
linked below for its administrative closure state.

## Scope and frozen identities

- Issue: [#1471](https://github.com/joris97jansen/borrowser/issues/1471).
- Milestone context: [AU (#107)](https://github.com/joris97jansen/borrowser/milestone/107).
- Implementation branch: `fix/1471-run-html-allocation-guards-independently`.
- Original verified planning base: `5d0fd878a8e954f4fd6047d8594b8ea113e32ea9`.
- Planning-base tree: `979aa65cf7cbd7bb946c04169329c5bbd8dc2943`.
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
the local complete-CI equivalent. The completed post-merge hosted CI evidence
below shows these named tests actually running against committed source.

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

## Completed implementation and ordinary CI — 2026-11-18

[PR #1472](https://github.com/joris97jansen/borrowser/pull/1472) merged at
08:46:14 UTC, after the two manual runs recorded below. GitHub commit metadata
and the merged history verify these immutable identities:

| Revision | Commit | Tree |
| --- | --- | --- |
| Original implementation | `00c49654a7f71e374365f341aed14bc9c7b90f80` | `af87ea4a9766dda98017d559914195774252ccbc` |
| Final feature merge | `b7783dc17dd95ea3ad56f9bfdac55ba339107e0d` | `cbfbacd73f0270480f880cf0b38490ea7825d205` |
| Actual master merge | `8c17949b291275444eca82ff41b71a6f421711ce` | `cbfbacd73f0270480f880cf0b38490ea7825d205` |

The original implementation remains an unchanged ancestor. The final feature
merge's ordered parents are the original implementation and
`8c6c3008bacdbaffb3facd5ed65dc677221e94f5`; the actual master merge's ordered
parents are that updated master and the final feature merge. The effective
approved implementation scope was exactly four files, +1,003/−7:

- `.github/workflows/perf.yml`;
- `tools/ci/tests/test_perf_qualification.py`;
- `crates/html/README.md`;
- `docs/ci/1471-html-allocation-qualification.md`.

[Post-merge ordinary CI 44269850761, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/44269850761)
passed on the actual master merge above (`push` event), completing at 08:54:17 UTC.
[Source and provenance](https://github.com/joris97jansen/borrowser/actions/runs/44269850761/job/134683424849)
passed all 245 tooling tests, including the 43 #1471 regression tests.
[Rust correctness](https://github.com/joris97jansen/borrowser/actions/runs/44269850761/job/134683424788),
[CI qualification](https://github.com/joris97jansen/borrowser/actions/runs/44269850761/job/134687186120)
and the required [Rust status](https://github.com/joris97jansen/borrowser/actions/runs/44269850761/job/134687234803)
passed. All 18 applicable jobs succeeded; the existing PR-only exact-head job and
separately gated native proxy-timing job were skipped as designed. Ordinary CI
success is distinct from the allocation-target and Perf conclusions below.

## Hosted manual acceptance — verified

The separately authorized executions followed the original acceptance procedure:
run the reviewed #1471 definition with empty input, then with the frozen #1470
source override. Run A completed at 08:21:12 UTC before Run B started at
08:23:54 UTC on 2026-11-18. Neither was cancelled. Sequential execution preserved
the existing concurrency policy, under which a later run on the same event ref
can cancel an earlier one. The default-source allocation regression was observed,
not merely assumed.

Exact-source invocation used for Run B:

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

### Verified workflow and allocation identities

Both runs were `workflow_dispatch`, attempt 1, with event ref
`refs/heads/fix/1471-run-html-allocation-guards-independently` and event SHA
`b7783dc17dd95ea3ad56f9bfdac55ba339107e0d`. Their recorded workflow ref was
`joris97jansen/borrowser/.github/workflows/perf.yml@refs/heads/fix/1471-run-html-allocation-guards-independently`,
with that same workflow-definition SHA. GitHub accepted the branch-specific
optional input; no fallback to master occurred. Heavy guards and tokenizer fuzz
continued to use the event source in both runs.

| Run | Requested allocation override | Selected and checked-out allocation SHA | Allocation tree |
| --- | --- | --- | --- |
| [A: 44265933344](https://github.com/joris97jansen/borrowser/actions/runs/44265933344) | Empty; default event source | `b7783dc17dd95ea3ad56f9bfdac55ba339107e0d` | `cbfbacd73f0270480f880cf0b38490ea7825d205` |
| [B: 44266622127](https://github.com/joris97jansen/borrowser/actions/runs/44266622127) | `2b2076fa1a142ab54d26e824f4ed208bcb902868` | `2b2076fa1a142ab54d26e824f4ed208bcb902868` | `bfc75a35f7153a1988a5847bd2933f53892823c3` |

Both allocation jobs ran natively on GitHub-hosted Ubuntu 26.04.1 x86-64 VMs,
image `ubuntu-26.04` version `20261111.196.1`. The recorded toolchain was
`rustc 1.92.0 (ded5c06cf 2025-12-08)` and
`cargo 1.92.0 (344c4567c 2025-10-21)`, host `x86_64-unknown-linux-gnu`.
Each used a fresh external Cargo target directory. Run B skipped the Cargo cache
action entirely. Before/after receipts in both runs verified the listed HEAD/tree
and `source_clean=true`; final verification and evidence upload succeeded even
when allocation execution failed.

### Native test execution and independent conclusions

The complete compiled inventory contained these six guards, all of which executed:

```text
atom_intern_reuse_does_not_allocate
atom_lookup_path_does_not_allocate
parse_document_attribute_values_avoid_unnecessary_allocs
parse_document_plain_text_allocation_is_bounded
parse_document_rawtext_allocation_is_bounded
streaming_parser_chunk_sizes_do_not_cause_pathological_alloc_growth
```

| Run | Allocation job | Heavy-performance job | Tokenizer fuzz job | Overall Perf |
| --- | --- | --- | --- | --- |
| A | [Failed: 5 passed, 1 failed](https://github.com/joris97jansen/borrowser/actions/runs/44265933344/job/134670342405) | [Failed: 934 passed, 3 failed](https://github.com/joris97jansen/borrowser/actions/runs/44265933344/job/134670342628) | [Passed](https://github.com/joris97jansen/borrowser/actions/runs/44265933344/job/134670342522) | Failure |
| B | [Passed: all 6](https://github.com/joris97jansen/borrowser/actions/runs/44266622127/job/134672645951) | [Failed: 934 passed, 3 failed](https://github.com/joris97jansen/borrowser/actions/runs/44266622127/job/134672646353) | [Passed](https://github.com/joris97jansen/borrowser/actions/runs/44266622127/job/134672646355) | Failure |

Both allocation summaries reported zero ignored, measured or filtered tests.
Run A retained the streaming allocation failure on the uncorrected event source;
its `allocation_outcome=failure` receipt did not produce
`qualification-success.txt`. Cargo exited 101 before the post-success inventory
extraction, so its artifact has no `alloc_guards-executed.txt`; all six native test
prefixes are present in `alloc_guards.log` and match the compiled listing.

Run B's compiled and executed inventory files match exactly. All six guards
passed, final source verification passed, and `qualification-success.txt` binds
that successful allocation qualification to the frozen #1470 SHA/tree above.
Its retained native measurements include both 1-byte and 1,024-byte schedules:

| Chunk bytes | Allocation events | Cumulative allocated bytes | Reallocations | Patches |
| --- | ---: | ---: | ---: | ---: |
| 1 | 70,276 | 10,975,568 | 64 | 12,007 |
| 1,024 | 64,332 | 10,109,840 | 365 | 12,007 |

The full log retains phase measurements and elapsed times; phase allocation
events, bytes and reallocations reconcile with their aggregates. These are hosted
Linux measurements of the frozen source, separate from the earlier local macOS
observations. No fixture, instrumentation or allocation threshold was changed.
Both runs retained the same three heavy parse-error failures; Criterion remained
skipped after those failures. Allocation success in Run B did not clear the heavy
job failure or make the overall workflow successful.

### Retained manual evidence

The allocation artifacts contain source selection, before/after identity receipts,
toolchain records, compilation/listing output and complete native execution logs.
Run B additionally contains the matching executed inventory and success receipt.
Downloaded ZIP checksums matched GitHub's artifact digests:

- Run A: [artifact 14409987612](https://github.com/joris97jansen/borrowser/actions/runs/44265933344/artifacts/14409987612),
  SHA-256 `97ef4bcce75b2e98c8fc412c260b61eb4fe21ca2e1b2e8247a98102462b8c201`.
- Run B: [artifact 14410098300](https://github.com/joris97jansen/borrowser/actions/runs/44266622127/artifacts/14410098300),
  SHA-256 `66809453c48821b6f9126d4c1d5687a08521873a79cd8eac4738ef04bcc3160e`.

This is exact-source manual allocation evidence under #1472's reviewed workflow
definition, not an ordinary PR #1470 check or qualification of its later merge.

## Post-merge scheduled acceptance — verified

Scheduled acceptance requires an actual `schedule` run of the merged definition.
The allocation job must check out that run's immutable default-branch event SHA
and report its conclusion independently of heavy tests. Manual dispatch cannot
prove scheduled execution; cancelled, timed-out or incomplete runs cannot satisfy
this gate. An allocation regression on the selected master source must remain
visible until the independent #1470 correction is integrated.

[Scheduled run 44289879147, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/44289879147)
completed with event `schedule` on 2026-11-18. Its recorded identities were:

- Event ref: `refs/heads/master`.
- Event SHA and workflow-definition SHA: `8c17949b291275444eca82ff41b71a6f421711ce`.
- Workflow ref: `joris97jansen/borrowser/.github/workflows/perf.yml@refs/heads/master`.
- Requested allocation override: empty.
- Selected source and checked-out HEAD: `8c17949b291275444eca82ff41b71a6f421711ce`.
- Source tree: `cbfbacd73f0270480f880cf0b38490ea7825d205`.

The workflow blob was `878cea3de001c47c004f682d1ae24c39dad70e1e`, identical to
the reviewed #1472 manual definition and the version on verified master
`f28e142a17a2053c5a16ec608895765ea2b7199b`. Allocation ran on the same native
Ubuntu 26.04.1 x86-64 image and Rust/Cargo 1.92.0 versions recorded above, with a
fresh external target directory and successful before/after clean-source checks.

| Job | Actual conclusion |
| --- | --- |
| [Allocation 134752019177](https://github.com/joris97jansen/borrowser/actions/runs/44289879147/job/134752019177) | Failed: all six compiled guards executed, five passed and the existing streaming allocation guard failed; zero ignored, measured or filtered |
| [Heavy performance 134752026838](https://github.com/joris97jansen/borrowser/actions/runs/44289879147/job/134752026838) | Failed independently: 934 passed, the same three parse-error assertions failed |
| [Tokenizer fuzz 134752020293](https://github.com/joris97jansen/borrowser/actions/runs/44289879147/job/134752020293) | Passed |
| Overall Perf | Failure, correctly retaining both failed jobs |

The event was created at 10:55:14 UTC, before PR #1470 merged at 10:56:27 UTC as
`f28e142a17a2053c5a16ec608895765ea2b7199b`. The allocation job completed at
10:56:26 UTC, while the entire workflow completed at 10:56:40 UTC. Its immutable
event source therefore predates the allocation correction, even though master
moved before the last job completed. The observed allocation failure is valid
evidence of independent scheduled execution and honest reporting, not successful
allocation qualification of that earlier master tree.

Final source verification and upload succeeded despite Cargo's exit 101.
`source-after.txt` retains the exact source SHA/tree, `source_clean=true` and
`allocation_outcome=failure`; no false success receipt was generated. As in Run A,
the failed Cargo command prevented post-success inventory extraction. The compiled
listing and all six execution prefixes in the retained log establish the complete
executed inventory without claiming a hosted `alloc_guards-executed.txt` exists.
[Evidence artifact 14420144178](https://github.com/joris97jansen/borrowser/actions/runs/44289879147/artifacts/14420144178)
retains the receipts, toolchain, compilation/listing and native test logs. Its
downloaded ZIP matched GitHub's SHA-256 digest:
`ee9836eff34cc5c209651d5176ff0052e40e1861621fdc30cf9c942f46007f50`.

## Final qualification conclusion

Issue #1471's technical exit criteria are satisfied: actual manual and scheduled
Linux runs execute the unchanged complete allocation target independently of
heavy-performance outcomes, retain exact source and job evidence, and distinguish
allocation failure from successful exact-source allocation qualification. The
locked dependencies, required features, serial execution, assertions, triggers,
concurrency and source-integrity boundaries remain enforced. The original defect
and failed development/hosted observations above remain part of the record.

The three independent heavy-performance failures remain unresolved, and all three
Perf runs retained overall failure conclusions. No parser or unrelated performance
repair belongs to #1471; no technical acceptance gate remains. Refer to the linked
GitHub issue for its administrative closure state.
