# AG6 fixture discovery, metadata and expectations

AG6 owns fixture authoring and selection in `crates/conformance`. HTML parsing,
CSS computation, Layout, Paint and Browser observation authority stay in their
existing production subsystems. The [AG5 contract](ag5-phase-comparison-projections.md)
continues to define the supported observations and exact comparisons.

## Authoring a fixture

Add UTF-8 HTML and a `*.fixture.json` declaration beneath
`crates/conformance/fixtures/`. Add an independently authored `*.expected.json`
when using a projection expectation. No Rust registration or test-list edit is
required. Rebuild the harness before running the new data:

```sh
cargo run -p borrowser-conformance --locked -- canvas/root
cargo test -p borrowser-conformance --locked --test fixture_build
cargo test -p borrowser-conformance --locked
```

For example, `root-canvas-color.fixture.json` declares:

```json
{
  "id": "canvas/root",
  "input": "root-canvas-color.html",
  "requires_javascript": false,
  "classification": { "kind": "runnable" },
  "observations": { "canvas": { "kind": "compare" } },
  "expectation": { "kind": "match" },
  "expected": { "kind": "canvas", "rgb8": [18, 52, 86] }
}
```

The original six HTML inputs and independently authored expected values are
unchanged. Their targeted acceptance assertions remain a named baseline;
general corpus tests discover and execute additional fixtures automatically.

## Small strict declaration

All objects reject unknown and duplicate fields. There is no metadata version,
feature negotiation, automatic unsupported-feature detector, or blessing command.

| Field | Meaning |
| --- | --- |
| `id` | Required unique identity. Slash-separated nonempty lowercase ASCII letter/digit/underscore/hyphen components, each starting with a letter or digit. |
| `input` | Required path to UTF-8 `.html`, relative to the declaration's directory. |
| `requires_javascript` | Required boolean describing eligibility. It never enables page scripts. |
| `classification` | `{"kind":"runnable"}`, or `unsupported`/`skipped` with a nonblank `reason`. |
| `observations` | Required family selections; runnable cases request at least one observation. Excluded cases may use `{}`. |
| `expectation` | For runnable fixtures only: `{"kind":"match"}` or `{"kind":"known_failure","reason":"..."}`. |
| `expected` | For runnable fixtures only: inline canvas RGB8 as above, or `{"kind":"projection","path":"case.expected.json"}`. |

Runnable fixtures require both expectation fields unless they declare an AG7
relationship; relationship cases may omit both. They cannot require JavaScript.
AG7 adds optional `relationship`, paired `provenance`/`upstream_assertion` fields
and exact reference provenance; see the [AG7 contract](ag7-focused-external-tests-and-references.md).
Unsupported/skipped fixtures must omit expectation data; optional JSON nulls
are equivalent to omission. They may declare a JavaScript requirement. Reasons
must contain non-whitespace text. Accepted strings, including Unicode, quotes,
backslashes and controls in descriptive text, retain their exact values; they
are encoded as escaped Rust string literals, never interpreted as Rust syntax.

An unsupported JavaScript-dependent fixture can therefore declare:

```json
{
  "id": "unsupported/scripted-dom",
  "input": "scripted-dom.html",
  "requires_javascript": true,
  "classification": { "kind": "unsupported", "reason": "Requires page JavaScript" },
  "observations": { "dom": { "kind": "compare" } }
}
```

Classification is explicit authored metadata. Merely containing a script element
is not an automatic classification algorithm. Page scripts stay disabled in
Chromium; trusted read-only harness inspection retains its existing boundary.

## Observation selection

Metadata constructs the existing AG5 `ObservationRequest` directly:

- `dom` and `canvas`: `{"kind":"compare"}` or
  `{"kind":"unsupported","reason":"..."}`.
- `styles`: an ordered array of `{"target":"parent","property":"color",
  "selection":{"kind":"compare"}}`. Compared properties are `color`,
  `background-color`, `font-size`, and `display`. An unsupported property uses
  `selection: {"kind":"unsupported","reason":"..."}`; it is not silently
  converted to a comparison.
- `layout`: `{"kind":"compare","targets":["container","child"]}` or
  `{"kind":"unsupported","reason":"..."}`.
- Omitted families are `NotRequested`. An omitted/empty styles array selects no
  style fields. Mixed-family requests retain AG5 semantics.

Existing target limits and duplicate checks apply. Empty layout comparison
lists are invalid. Expected projections must contain exactly the requested
family states, style target/property records and layout target records, in
request order. Missing, extra, duplicate or misidentified records are errors,
including under known-failure metadata. A complete missing-node observation,
zero-box result, different DOM child count or different rectangle count remains
comparable content, not a malformed expectation record.

## Discovery, embedding and validation

`build.rs` recursively discovers declarations under its fixed crate fixture
root. Missing roots and empty catalogues fail the build. Paths are sorted before
validation so discovery diagnostics do not depend on filesystem enumeration.
Duplicate IDs fail; the final catalogue is sorted lexicographically by ID.
Report family order remains DOM/style/layout/canvas, followed by the declared
property/target order and existing field ordering.

References must remain within the declaration directory or its descendants.
Absolute paths, empty/dot/parent components, backslashes, colons, control
characters, non-UTF-8 filenames, symlinks and nonregular entries are rejected.
Missing references and orphan `.html`/`.expected.json` files fail explicitly.
Metadata syntax, eligibility, required reasons and declared-reference errors
identify the metadata path; JSON parse errors also include line and column.

The generator writes only `OUT_DIR/fixtures_generated.rs`. It produces the
existing `Fixture` structures and compile-time includes for exact HTML and
expectation bytes; it does not reserialize those inputs. Generated source is
independent of checkout location. Cargo tracks the entire fixture directory,
including file additions/removals, plus the build helper. Each successful
regeneration replaces the whole generated catalogue. The existing AG4
`BORROWSER_AG4_SOURCE_TREE` rebuild tracking remains in effect.

The generated array is private. `fixtures::catalogue()` validates the complete
embedded corpus with AG5 request and expected-projection validators before
returning any fixtures. `fixtures::select()` uses that same boundary for all
or one exact ID. Typed expectation validation happens at startup rather than
being duplicated in the build helper. Thus invalid expectation data can compile
but returns exit 2 before an engine starts, even when another fixture was selected.
Artifact identity publication and preparation also fail on invalid expectations.
Failures propagate explicitly; no empty catalogue or panic substitutes for an error.

No runtime input, metadata, expectation or checkout file access occurs. A copied
executable runs from an unrelated working directory after its build source tree
has been removed. Reports retain relative descriptive source paths.

## Execution and reporting

| Mode | Runnable and excluded cases together | Excluded-only selection |
| --- | --- | --- |
| Default | Execute runnable fixtures; report unsupported/skipped fixtures. | Report exclusions with existing local exit behavior (0). |
| `--compare` | Capture runnable fixtures only; include exclusions in comparison reports. | Incomplete report, exit 3; no Chromium configuration or launch. |
| `--chromium` | Capture runnable fixtures only; capture-only output has no classification records. | Configuration error, exit 2; no launch or invented observation. |
| `--compare-aws` | Existing remote capture/cleanup for runnable fixtures, then report the full selection. | Incomplete report, exit 3; no cloud configuration, credentials or activity. |

A nonempty catalogue with no runnable cases is valid; accidental empty discovery
is not. Unknown exact IDs remain errors. Excluded fixtures never enter either
engine. All Chromium captures and cleanup finish before Borrowser parser workers
start. Existing comparison exit precedence and incomplete-coverage behavior remain.

Execution classification, independent expectations, observation coverage and
cross-engine truth stay separate. Known failure produces XFAIL only for a valid
Borrowser expectation mismatch; an unexpected match is XPASS. Configuration,
execution and capture errors never satisfy known-failure metadata. XFAIL never
suppresses a Borrowser/Chromium difference. No status is added by AG6. AG7 retains the same exit precedence (2, 1, 3, 0),
adds separate scoped relationship outcomes and upstream assertion coverage, and
never counts reference dependencies as independent passes.

## AG4 identity and bounded integration

AG4 #1387 is completed and PR #1433 is merged. AG6 does not reopen its cloud
qualification or authorize live AWS work.

Artifact identities still enumerate only runnable fixtures, in catalogue order,
with AG7 `CaptureDescriptor` records: case ID, test/reference role, SHA-256 of
the exact embedded HTML, and the exact typed observation request. References
follow their test immediately; they are not separately registered test cases. Existing source-tree
identity binds metadata/expectations and executable SHA-256 binds the produced
artifact. Native and Linux builds must agree. New fixture metadata/expectation
JSON is admitted by source preparation only beneath the fixed fixture root;
unrelated new JSON is not admitted by this rule.

Capture ingestion checks the exact ordered descriptor vector, count, uniqueness,
parsed link facts and native observation compatibility. The 64-KiB capture bound, artifact/identity size bounds, process
budgets, transport and cleanup contracts remain unchanged. A larger corpus is
not permission to exceed them; exact-ID selection remains available. Historical
`--aws-cleanup` is independent of catalogue validation.

## Regression validation

The build-helper tests exercise real discovery and strict declaration parsing.
The explicitly invoked isolated production rebuild test proves incremental
add/edit/remove behavior, Rust-literal encoding and exact accepted text,
pre-execution failures across consumers, excluded-only handling, artifact
identity and a copied executable with no source checkout:

```sh
cargo test -p borrowser-conformance --locked --test fixture_build isolated_cargo_rebuild_and_standalone_execution -- --exact --ignored --nocapture
```

The test is ignored by default because it compiles production dependencies in a
separate target directory. It uses cached dependencies (`--offline`) and creates
no cloud resources. With `BORROWSER_CHROMIUM_EXECUTABLE` set to the existing
pinned browser, it also runs actual mixed-catalogue capture and comparison; without
that variable it explicitly reports that additional native check as not executed.
Normal native lifecycle tests and opt-in pinned Chromium checks retain their
existing permission and standalone-process requirements.
