# Borrowser-owned rust-url and ureq subtrees

Issue [#1457](https://github.com/joris97jansen/borrowser/issues/1457), Milestone AI
(#95). These are ordinary source files maintained and committed in Borrowser.
Future fixes do not require a permanent separate fork. CSS, Layout, Paint,
Addressing, Fetch and Browser retain their existing runtime ownership contracts.
This source-management migration does not change URL or HTTP behavior.

## Original source and initial migration

| Input | Exact identity |
| --- | --- |
| Official Servo baseline | `00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a` |
| Approved custom rust-url source | `92f7fb395f9ffb2f706e7c664283781fd3b294a2` |
| Approved rust-url tree | `5b1003bbe544b4ee9c25299fbe785289c0831ee7` |
| Published ureq 2.12.1 Git release | `ba4555d54fd72e16fc737f7e8fcc812efb131260` |
| Equivalent ureq maintenance release | `3b0b313db1ab4a08898a904dc7fb04bf4c0b99ae` |
| Shared ureq release tree | `dae21bb12bee845505f16bcdfd34b96033ffef69` |
| Original qualified Borrowser snapshot | `4a997e93cafeb9d6223b3eb034300e7f950a6bd6` |

The complete rust-url workspace has 80 tracked entries: 72 regular files and
eight original license links. Import its custom commit directly to preserve all
eleven custom commits, their original merge topology and official Servo ancestry.
The historical [closed, unmerged PR #1460](https://github.com/joris97jansen/borrowser/pull/1460)
remains evidence; its frozen-copy representation was superseded.

Ureq's published archive SHA-256 is
`02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d`.
Git contains 59 files; the archive contains 54. Git-only files are `cargo_deny.sh`,
`deny.toml`, `test.sh` and the four files under `examples/mbedtls/`. Cargo adds
`.cargo_vcs_info.json` and `Cargo.toml.orig`; among shared paths only `Cargo.toml`
differs. Archive `Cargo.toml.orig` equals Git's handwritten manifest.

The qualified vendor snapshot retained 50 package files and four Borrowser records.
All 54 survive byte-for-byte. Restore the four previously omitted package files
(`.github/dependabot.yml`, `.github/workflows/test.yml`, `.gitignore`, `Cargo.lock`)
and the seven Git-only files, producing 65 entries. The normalized selected
manifest and all AI3/AI5 source corrections remain intact. The imported standalone
ureq lockfile is historical upstream input, not the Borrowser consumer lockfile.

Initial commands, from a freshly verified clean master-based integration branch:

```sh
git fetch --no-tags https://github.com/servo/rust-url.git 00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a
git fetch --no-tags https://github.com/joris97jansen/rust-url.git 92f7fb395f9ffb2f706e7c664283781fd3b294a2
git subtree add --prefix=vendor/rust-url 92f7fb395f9ffb2f706e7c664283781fd3b294a2

git fetch --no-tags https://github.com/algesten/ureq.git ba4555d54fd72e16fc737f7e8fcc812efb131260
git fetch --no-tags https://github.com/algesten/ureq.git 3b0b313db1ab4a08898a904dc7fb04bf4c0b99ae
git rm -r vendor/ureq
git commit -m 'Prepare occupied ureq prefix for history import'
git subtree add --prefix=vendor/ureq ba4555d54fd72e16fc737f7e8fcc812efb131260
git subtree merge --prefix=vendor/ureq 3b0b313db1ab4a08898a904dc7fb04bf4c0b99ae
```

Before the bridge, assert both release trees equal the shared tree above. After
it, assert the complete host tree equals its first parent's tree. Bridge before
restoring the normalized manifest: bridging after restoration can conflict in
`Cargo.toml` despite the equal release trees.

Set `BORROWSER_BASE` to the verified pre-migration Borrowser commit, then restore
only its existing file paths, retaining the eleven added upstream files:

```sh
git ls-tree -r --name-only -z "$BORROWSER_BASE" -- vendor/ureq |
  git --literal-pathspecs restore --source="$BORROWSER_BASE" \
    --staged --worktree --pathspec-from-file=- --pathspec-file-nul
git commit -m 'Restore qualified Borrowser ureq source'
```

The removal and pristine-import commits are structural intermediate states, not
independently qualified releases. Do not run the browser with uncorrected ureq or
introduce runtime workarounds to make those states build. Qualify the complete
restored candidate. For #1457 only, the owner authorized exactly these five local
structural/restoration commits; final tooling/configuration changes stop for
review before their actual-repository commit. A disposable committed preview is
separate evidence and does not qualify a later actual commit.

## Current qualification and local development

`tools/ci/dependency-subtrees.json` describes exactly these two dependencies,
original imports, adopted official revisions, current paths/modes/content hashes
and URL Cargo graphs. Its digest is independently pinned in
`check_dependency_subtrees.py`. It contains no self-authorizing approval flag.
No editable exclusion list can hide an added source file. All vendor files,
including historical records, are accounted for; current policy lives outside
the prefixes and has no self-hashing cycle.

Edit dependency source and tests directly in Borrowser. Proposed source changes
must fail the existing baseline until deliberately qualified. For an intentional
change, review the source diff, update the proposed inventory and independent
policy digest, preserve or explicitly requalify Cargo graphs, and run applicable
behavior/security tests. Keep dependency fixes and unrelated host edits separate
where practical so later contributions can be focused.

The five boundaries are distinct:

1. Source integrity verifies admitted bytes, types, executable modes and links.
2. History verifies real import parents, bridge trees and reachable upstream commits.
3. Cargo verifies actual selected manifests, registry origins, versions, edges,
   features and locks, not only apparent package identities.
4. Rust, networking, browser, conformance, fuzz and supported native qualification
   establish the applicable behavioral/security evidence.
5. The project owner explicitly accepts the exact final candidate and merge.

Changing source, acceptance records, verifier and workflow together can make CI
pass. CI cannot establish the owner's authorization or prove the new policy is
adequate. The owner reviews those changes and results; later relevant changes
require renewed acceptance. No additional human reviewer or external service is
required. A harmless-comment fixture proves only the advancement mechanism, not
that behavior-changing modifications are safe.

The preserved ureq records and patches describe the original published/AI3/AI5
history. Their immutable snapshot is reconstructed separately, so legitimate
future current-source changes need not continue matching a permanently frozen
patch. Routine verification does not download the archive; optional independent
archive verification is:

```sh
python3 -I -B tools/ci/check_ureq_patch.py --archive /path/to/ureq-2.12.1.crate
```

## Verification and Cargo routing

Main patches `url` and `percent-encoding` to the local rust-url workspace; fuzz
patches only `url`. Original sibling paths select `idna` and `form_urlencoded`.
The nested workspace is excluded from the main workspace. Main keeps its local
ureq patch; fuzz does not gain an unused ureq dependency.

Preserve url 2.5.8, idna 1.1.0, form_urlencoded 1.2.2, percent-encoding 2.3.2 and
ureq 2.12.1. Both URL runtime closures remain 35 packages with 31 registry packages;
main and fuzz retain their distinct graph edges and feature baselines. Ureq keeps
`tls,json,charset,native-certs`; automatic decoding stays off, rustls 0.23.32 uses
`ring,std,tls12` without logging, and shared flate2 remains 1.1.2.

Prepare caches before offline gates:

```sh
cargo fetch --locked
cargo fetch --manifest-path fuzz/Cargo.toml --locked
make check-url-dependencies check-network-dependencies
```

History runs before source/Cargo qualification. The committed-candidate checker
requires complete history and checks committed blobs, staged changes and physical
source separately. Unmerged index stages, staged bad bytes with a restored good
worktree, invalid physical links and ignored extra vendor files are rejected.
Pristine imports must add exactly the complete upstream path inventory, with no
host changes; their exact prefix tree IDs bind every imported blob and mode.
Adopted official revisions must descend from the original Servo or ureq
maintenance-release baseline, in addition to being reachable through the required
integration history. Ordinary local commits and forward upstream updates remain
permitted.

Required qualification inputs must match the tested commit. This includes every
committed file under `tools/ci/tests/`: test implementations, helpers and fixtures.
Staged differences, physical byte/mode changes, missing files and untracked files
are rejected, including ignored source files. Both vendor prefixes retain their
stricter complete physical inventories.

Python qualification entry points require `-I -B`. Isolation prevents the tooling
directory or working directory from shadowing standard-library imports before
preflight. `-B` prevents cache writes; it does **not** prevent cache reads. Before
importing repository helpers or discovering tests, the source-executed checker
rejects every `__pycache__` entry and `.pyc`/`.pyo` path anywhere under `tools/ci/`,
including nested test directories, ignored files, symlinks, nonregular entries,
case aliases on insensitive filesystems, and tracked/staged caches whose physical
files were removed. Untracked Python
helpers are rejected too; helpers outside the test directory must be declared in
`COMMITTED_INPUTS`. Python helper/test bytes and Git/index state are checked
before those modules can be imported. The other source and Git checks remain mandatory.

The same preflight rejects native Python extension modules throughout `tools/ci/`
before that directory enters the import path. Extensions take precedence over `.py`
helpers during normal Python lookup. Rejection uses the interpreter's
`importlib.machinery.EXTENSION_SUFFIXES`, plus portable `.so` (Linux/macOS) and
`.pyd` (Windows) endings, including ABI-qualified filenames and case aliases.
Physical, ignored, untracked, tracked and staged entries are checked, including
symlinks, nonregular paths and Git entries whose physical files were removed.
No extension is loaded to validate it. Move a reported extension outside the
tooling directory and remove any tracked/staged copy from the reviewed candidate;
the guard never performs cleanup. Ordinary unrelated shell scripts remain allowed.

No cache is deleted or overwritten. If preflight reports a cache, inspect and
manually move the reported cache file/directory outside `tools/ci/`, then retry.
Do not regenerate caches in the qualified directories. Generated artifacts under
`target/` and unrelated private files outside these execution inputs are unaffected.
Use the guarded test entry point, not raw `python -B -m unittest` discovery:

```sh
python3 -I -B tools/ci/check_dependency_subtrees.py --tests
```

It checks the committed candidate before discovery and again after the tests.
Direct URL, ureq and monitoring entry points execute the checker source preflight
before any repository import. Monitoring stops with exit 2 if this preflight
fails; it does not report a successful or complete upstream observation.
Disposable qualification fixtures copy the whole test directory and stage its
deletions as well as additions, so a partial copy cannot stand in for the test
implementations being executed. Source-only and
Cargo checks can inspect a proposed working tree, but cannot claim qualification
of a final commit that does not yet exist.

Reproduce actual `cargo metadata --locked --offline --format-version 1` for main
default, html5, hardening (`html5 html5-fuzzing parser-conformance parser_invariants
parser-failure-injection`), conformance-observation and all-features; also fuzz
default and all-features. Migration comparison normalizes only intended URL Git
to local identities and checkout roots, preserving complete graphs and locks.
Actual source paths and registry cache origins are verified independently.
Registry-origin checks trust Cargo, the approved cache and host filesystem; they
do not cryptographically authenticate every extracted registry source file.

Pristine import evidence includes these assertions, with import/bridge IDs from
the qualification record:

```sh
git diff --exit-code "$URL_SOURCE^{tree}" "$URL_IMPORT:vendor/rust-url"
git diff --exit-code "$UREQ_RELEASE^{tree}" "$UREQ_IMPORT:vendor/ureq"
git show -s --format='%H %P %T' "$URL_IMPORT" "$UREQ_IMPORT" "$UREQ_BRIDGE"
git diff --exit-code "$UREQ_BRIDGE^1^{tree}" "$UREQ_BRIDGE^{tree}"
git merge-base --is-ancestor "$URL_SOURCE" HEAD
git merge-base --is-ancestor "$UREQ_MAINT_RELEASE" HEAD
```

A copied source tree or Git trailers are not history evidence. Tests also prove
that an equal-content squash can lose the required ancestry.

## Official upstream monitoring and updates

`check_rust_url_ureq_sync.py` observes only `servo/rust-url:main` and
`algesten/ureq:2.12.x-msrv1.71`. The existing workflow file is retained, with a
display name covering both. Both observations run even when one fails.

| Condition | Exit |
| --- | --- |
| Official tip equals qualified official baseline | 0: Current |
| Official tip descends from baseline with new commits | 1: Review available |
| Divergence/rewind, missing refs, fetch errors, incomplete or unqualified configuration | 2: Error/incomplete/unqualified |

Compare official tips to adopted official baselines, not local custom commits.
A stateless monitor cannot detect every rewrite between observations if the new
tip still descends from the recorded baseline. A review warning is a real
nonzero result; it does not mean source has been adopted or qualification failed.
Monitoring performs no source writes, PR creation or master updates.

For a deliberate update:

1. Read the exact observed revision and recheck current master.
2. Fetch that exact SHA from the official repository and verify it descends from
   the last qualified official baseline. Stop on unexpected history.
3. Create a dedicated Borrowser update branch from verified master.
4. Run `git subtree merge --prefix=vendor/rust-url "$REVISION"` or the ureq
   equivalent, without squash. Preserve local changes or expose conflicts.
5. Inspect every source difference. Resolve conflicts explicitly; abort with
   `git merge --abort` to restore the original clean commit when abandoning it.
6. Record the upstream revision/integration commit and requalify source, Cargo
   and applicable behavior/security lanes. Do not upgrade ureq to 3.x implicitly.
7. Prepare a PR with exact identities, local differences, conflicts/resolutions,
   qualification and failures. Obtain owner authorization for its final revision.
8. Integrate with a merge commit, preserving the history. Verify the approved PR
   head and upstream commits are reachable on remote master, source identities
   match, master-push qualification passes, and monitoring is operational.

The original migration adopts no newer Servo or ureq maintenance changes.

## Split output and optional upstream contributions

Borrowser's primary history preserves imported and integrated upstream commits.
`git subtree split` reconstructs prefix history; it need not preserve every
primary-history merge parent. With the actual ureq releases, splitting the
content-neutral bridge returned the original published release while omitting
the divergent maintenance-release ancestry. Later upstream merges can retain
correct split contents without retaining their upstream tip in the split graph.

Run splits with Bash explicitly. The installed `git-subtree` shell script
recursively visits uncached parents in these real imported histories; Ubuntu's
`/bin/sh` (dash) can stop with `Maximum function recursion depth (1000) reached`.
The history tests use the same installed, unmodified script under Bash, without
changing revisions, excluding history or relaxing split assertions. From the
repository root, for example:

```sh
subtree_exec="$(git --exec-path)"
GIT_EXEC_PATH="$subtree_exec" PATH="$subtree_exec:$PATH" \
  bash "$subtree_exec/git-subtree" split --prefix=vendor/ureq
```

Use `--prefix=vendor/rust-url` for rust-url. Bash is required for this split
qualification profile on Linux/macOS; native Windows execution remains
unqualified. This does not change the non-squashed import or merge procedure.

A successful split is not automatically suitable for pushing to an official
branch. Inspect its tree against the source prefix, ancestry against the exact
upstream target, complete commit range and full diff. Never directly push to the
official maintenance branch solely because splitting succeeded.

Prefer a contribution branch based on the intended official upstream branch.
Cherry-pick only inspected prefix-free split commits, or apply a focused patch
containing only the fix and tests. A Borrowser diff under either vendor prefix
can be checked in an upstream checkout with `git apply --check -p3`; inspect the
result before applying. Exclude normalized packaging manifests, policy records
and unrelated Borrowser changes. Run upstream qualification and propose the
contribution separately; no permanent fork is needed for ordinary development.

## Platforms and closeout

The original eight rust-url links have Git mode `120000` and target exactly
`../LICENSE-MIT` or `../LICENSE-APACHE` inside data-url, idna, percent_encoding and
url. Root license targets must remain real files inside the prefix. Preserve the
upstream ureq scripts' `100755` modes. Exact-byte checkouts must avoid EOL/filter
transformations. Do not silently materialize links.

With `core.symlinks=false`, Git can report clean while checking out plain link
text; that representation fails source qualification. Native Windows requires
working symlink support and suitable Git/filesystem permissions. Current hosted
Borrowser qualification is Ubuntu; commented matrix entries and nested upstream
workflows do not establish other platform support.

The authoritative Rust job uses the event SHA with full history and no persisted
credentials. PRs test synthetic integration commits; master pushes test actual
merges. Passing PR CI does not prove the later GitHub merge preserved ancestry.
This migration and future upstream integrations require owner-authorized merge
commits, followed by remote-master history/source and master-push verification.
No repository settings change is part of this issue. Hosted-only tests, historical
IDNA/setter findings and native baseline failures must retain their real status.
