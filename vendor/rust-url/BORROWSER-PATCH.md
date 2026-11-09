# Approved Borrowser rust-url snapshot (#1457)

Upstream is `https://github.com/servo/rust-url.git` at
`00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a` (tree
`e14f83138396ca6f64702f263516ba5c39c9ffa9`). The maintained fork is
`https://github.com/joris97jansen/rust-url.git` at
`92f7fb395f9ffb2f706e7c664283781fd3b294a2` (original tree
`5b1003bbe544b4ee9c25299fbe785289c0831ee7`). The external fork remains the
development and upstream-contribution repository.

## Retained source and transformations

All 80 tracked entries are retained. No Rust file, Cargo manifest, fixture,
test, or contribution is corrected by this migration. The original six-member
workspace and independent `url/fuzz` workspace remain intact. `data-url` and
`url_debug_tests` preserve that workspace structure; neither joins Borrowser's
workspace or consumer graph. Parent-relative README, sibling-crate and debugger
metadata paths remain intact. The nested upstream workflows are retained source
evidence, not additional Borrowser Actions workflows.

The eight original `LICENSE-MIT`/`LICENSE-APACHE` symlinks in `data-url`, `idna`,
`percent_encoding`, and `url` are materialized as regular files containing the
exact root license bytes. Their only approved targets are `../LICENSE-MIT` and
`../LICENSE-APACHE`. `UPSTREAM.json` records original modes, blob identities, link
bytes, target names and final content hashes. Other files are byte-identical.
This materialized directory therefore does **not** have the original Git tree
identity. No `.git`, generated Cargo lockfile, or build output belongs here.

## Reproduction

Use a complete Git checkout containing both exact revisions. Enumerate tracked
entries with `git ls-tree -rz REVISION`, and read each blob with `git cat-file
blob OBJECT`. Preserve regular-file bytes and materialize only the eight recorded
license links after verifying their exact target text and root-license bytes.
The complete original and materialized inventories are in `UPSTREAM.json`.
Inventory digests use UTF-8 `json.dumps(value, sort_keys=True,
separators=(",", ":"))`, then SHA-256.

Generate the cumulative patch from that checkout with this exact command:

```sh
git -c core.quotePath=false diff --no-ext-diff --no-textconv --no-renames \
  --binary --full-index --no-color --diff-algorithm=myers --no-indent-heuristic \
  --unified=3 --src-prefix=a/ --dst-prefix=b/ \
  00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a \
  92f7fb395f9ffb2f706e7c664283781fd3b294a2 > fork.patch
```

The 84,285-byte patch SHA-256 is
`4c2d43bd7bcf39438bc975087c20e087341aa470393a1a3147a53fdef1813d47`.
It records the ten changed paths, including the added `url/tests/domain.rs`.
It is a cumulative content diff, not a linearized contribution history.
Eleven original raw commit objects in `UPSTREAM.json` preserve authorship,
messages, trees and ordered merge parents. The original upstream commit is the
common baseline; the merged contribution topology is retained without embedding
Git history in this directory.

## Offline verification and Cargo selection

From Borrowser's root, after preparing locked registry caches:

```sh
python3 -B tools/ci/check_url_dependencies.py --source-only
PYTHONDONTWRITEBYTECODE=1 make check-url-dependencies
```

The existing URL checker independently freezes the approved source identities,
layout, records and patch. Inventory/type admission precedes content reads. It
checks every retained file, reconstructs all 79 materialized upstream entries by
reverse patching in disposable storage, and replays the patch to all 80 approved
entries. It never changes the checkout, index, manifests, locks or approval
records. Routine verification has no network access; temporary reconstruction is
the only source-verification write.

Main patches `url` and `percent-encoding`; fuzz patches only `url`. Original
sibling paths select `idna` and `form_urlencoded`. Actual locked offline Cargo
metadata must identify the four exact local manifest paths, one implementation
per package, and the existing versions and 35-package URL runtime closure.
`QUALIFIED-GRAPHS.json` records seven configurations and their two distinct
edge/feature baselines. Main has 86 named closure edges; fuzz has 85. Features
are compared within each configuration, not required to match across workspaces.
The existing independent 31-package registry version/checksum policy remains in
the checker. No other dependency migration is authorized.

## Maintenance and evidence limits

Local byte verification establishes agreement with this approved snapshot. The
recorded Git objects and original qualification establish historical identity;
weekly network monitoring separately observes live upstream/fork ancestry and
verifies the approved commit's tree. An ancestry relationship is not proof of
semantic equivalence or complete WHATWG conformance. Existing AH limitations and
expectations remain unchanged.

The weekly monitor never adopts new source or rewrites records, hashes, manifests
or locks. A new source revision requires explicit source, graph and behavior
requalification before changing frozen approval anchors. Recalculating editable
record hashes cannot authorize changed source. The checker itself is reviewable
policy: changing it together with source is not independent approval.

The master ruleset inspected on 2026-11-09 requires `Rust (ubuntu-latest)` but zero
approving reviews. Minimum external policy recommendation: require an independent
approving review after the latest relevant changes and invalidate stale approvals.
No repository settings change is part of this candidate. Passing CI alone does
not approve a new source revision. Hosted PR CI and a candidate monitor run remain
separate qualification requirements; confirm the scheduled default-branch monitor
after merge. See the dated #1457 amendments in the AH contracts for results.
