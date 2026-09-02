# AG8 WPT import, filtering, and unsupported-test classification

This contract defines Borrowser's bounded WPT-source intake path. It does not
claim broad WPT execution or compliance. The normative source population is
`tests/conformance/external/wpt/sources.toml`; its immutable WPT revision,
declared paths, hashes, licence notice, attribution, source records, closure
files, and adaptation lineage are semantic authority. The checked-in AG8 proof
has exactly seven records and six closure files, but those counts and its pinned
revision are repository integration assertions rather than schema invariants.
Materialized source bytes are pinned inputs. `accounting-summary.toml` is
generated review evidence, never an independent metadata authority.

WPT-authored form interpretation belongs to `wpt-test-support`. Immutable WPT
bytes and the strict, evidence-backed
`tests/conformance/external/wpt/source-metadata.toml` contract are its only
semantic inputs. The resulting `InterpretedWptRecord` projects only
source-neutral `SourceRequirements` into `conformance-test-support`.
Generic AG accounting independently assesses production capabilities, harness
readiness, repository selection-environment support, resource support, and
representation/adaptation support. Selection is derived policy and does not
erase any blocker or unresolved fact.

`tests/conformance/external/assessment-profile.toml` is the strict,
source-neutral `borrowser-external-assessment-profile-v1` authority for those
repository-stable assessments. Supported assertions require independent,
path-confined repository evidence. Missing assertions reconcile as
`not-yet-established`; the file contains no current-host or transient runtime
state. `wpt-test-support` consumes this validated profile and does not hard-code
Borrowser capability or harness availability.

`source-metadata.toml` records human-reviewed facts that cannot safely be
derived from bounded HTML metadata alone, including feature areas, browser
capability prerequisites, positive no-JS compatibility, and WPT
server/controlled-HTTP prerequisites. Every annotation must select evidence
produced from the immutable source record or an authoritative WPT convention.
`NoJs` is positive, reviewed evidence: the absence of a detected executable
`script` never establishes it. Positive executable-script evidence establishes
`RequiresJs`; absence of both tags leaves no-JS compatibility unresolved.
Contradictory positive declarations are rejected. Changing assessment or
selection policy cannot change an interpreted record.

`tests/conformance/external/wpt/selection-policy.toml` is the strict
`borrowser-wpt-selection-policy-v1` authority for test-form, path/category,
feature-area, no-JS, resource/network, pixel, and platform filters. All source
records enter interpretation and accounting before this independent policy is
applied. Feature filtering consumes the evidence-backed feature areas already
present on interpreted records; unknown help links remain evidence and do not
require a Rust feature switch.

The type and dependency flow is:

```text
immutable WPT bytes + validated source interpretation metadata
  -> wpt-test-support::InterpretedWptRecord
       WPT facts stay WPT-owned
       SourceRequirements projects source-neutral requirements
  -> conformance-test-support repository assessments
  -> independent WPT selection-policy evaluation
  -> AccountedExternalSource and original-WPT decision
  -> independent derived-adaptation requirements, assessment, and decision
  -> wpt-test-support combines both immutable views
  -> borrowser-wpt-import-summary-v1
```

Changing a future Borrowser production profile or selection policy can therefore
change accounting without reparsing or changing upstream WPT requirements. A missing engine
capability is never inferred from a harness or environment limitation; a
harness limitation is never invented because an engine capability is absent;
and an environment requirement is not current-host availability.

The accounting invariant is:

```text
validated declared source records == deterministic source decisions
```

Integrity failures—including missing provenance, hash mismatch, path escape,
symlinked path components in any authoritative AG8 input, an undeclared
replacement, or an unsupported schema—are fatal.
They are not recorded as unsupported tests. Record-local bounded interpretation
conditions—including reference depth/node/edge ceilings, cycles, unsupported
reference paths, and an incomplete declared reference closure—remain accounted
with a typed stable limitation; successfully established orthogonal source facts
are retained. They do not abort unaffected sibling records. Rejected and
unresolved sources remain present in the deterministic summary.

## WPT authored forms and references

AG8 implements only the forms required by its proof population: HTML reftests,
`testharness.js` tests, and Python wdspec/WebDriver tests. Manual, visual,
crashtest, print-reftest, and parser `.dat` forms remain deferred. AE13e keeps
ownership of its parser `.dat` record generation and execution.

HTML-authored metadata is interpreted with test-tooling-only `html5ever`
0.39.0 using `parse_document` and a bounded custom tree sink implementing the
complete required `TreeSink` contract for the bounded document domain. Limits
are 1 MiB input, 131,072 nodes, 131,072 total attributes, 4,096 attributes per
element, 1 MiB retained metadata, and 1,024 parse errors. Script/raw-text state,
malformed markup, attribute case, and quoted/unquoted attributes therefore use
HTML semantics; markup-looking text inside script data cannot create a false
dependency. A private typed limit abort is caught only at this parser boundary;
unrelated panics resume unwinding. There is no regex, substring, handwritten
HTML parser, or Borrowser production-parser fallback.

Reference edges preserve match versus mismatch relations, multiple references,
`reftest-wait`, and the resource closure. Each fuzzy declaration retains both
its opaque value and the root/reference graph node that authored it; AG8 does
not parse or execute fuzzy pixel semantics. Missing/cyclic references and graph
bounds produce accounted record-local limitations rather than corrupting the
trusted population. A WPT reftest is a raster assertion. AG7 is only a structural
or semantic comparison harness. All seven original proof assertions are
therefore currently not selected for direct execution. A separate typed derived
adaptation decision selects one exact-copy Paint-semantic fixture only after its
own requirements, filter, lineage, adapter, and representation support validate.
The derived result never bypasses original blockers and never becomes a WPT
raster pass.

## Resources and mutation

Resources are distinguished as self-contained, pinned local static closure,
controlled/server-dependent, live-network-dependent, or platform-service
dependent. Relative URLs are not automatically networking. All materialized
local files are explicitly declared, path-confined, regular non-symlink Git
blobs, bounded by registry/record/file/byte and reference node/edge/depth
ceilings, and SHA-256 verified. Closure-file accounting is based on declared
file roles, not a record-count subtraction. There are no implicit filesystem
reads or network requests during normal checks/tests.

Normal operation is read-only:

```text
cargo run -p wpt-test-support --bin conformance-wpt-import -- --check
```

Explicit source materialization reads committed objects—not working-tree
bytes—from a checkout containing the exact pinned revision:

```text
cargo run -p wpt-test-support --bin conformance-wpt-import -- \
  --materialize --from-wpt-checkout /path/to/wpt
```

The checkout's `HEAD` need not equal the declared revision. Dirty or newer
working trees are permitted because the declared commit must exist locally and
only exact revision-addressed declared Git objects are read. Git lazy fetching
is disabled; a partial clone that does not already contain a declared object fails
rather than accessing the network. Every object mode and object size is
preflighted for both individual and aggregate byte limits before any blob body
is captured; post-read size and digest checks remain defense in depth. Validated
bytes are atomically published under `raw/<revision>/`. A failed publication cannot
remove or replace a previously valid immutable revision. Summary refresh is a
separate explicit operation:

```text
cargo run -p wpt-test-support --bin conformance-wpt-import -- --update
```

Both canonical stdout and the checked-in summary come from the same validated
model. Serialization uses UTF-8, LF newlines, sorted records and fields, and no
timestamps, host paths, locale, filesystem order, current-host availability,
or transient execution state.

## Adding a future subset

Pin one immutable revision, declare the complete bounded file closure and
hashes before selection, add both accepted and rejected proof records where
architecturally useful, extend WPT interpretation only for evidenced upstream
forms, and add subsystem-owned adaptation only when an existing Borrowser
observation truthfully represents the derived assertion. Do not expand volume
until registry validation, accounting conservation, summary checks, and
subsystem execution tests pass.

Every AG2 V4 `external-derived` fixture must reconcile exactly once through
`tests/conformance/external/registries.toml` to authoritative source-record,
derived `TestId`, adapter, and adapter-version lineage. The AG8 exact-copy-v1
adapter additionally requires the lineage reference to be a declared reference
node and exactly the target of the source record's single upstream match edge.
It then checks the derived test/reference bytes against those pinned
source/reference bytes and declared SHA-256 values, so fixture drift is fatal.
