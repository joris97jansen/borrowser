# AH9: Addressing conformance fixtures and exact synthetic observations

Issue: #1119. Milestone AH; milestone closeout remains AH10 / #1120.
Baseline: master `1cef5bceafd79c872fe6edc9750384e8281a338a` (AH7 and AH8 merged).

## Completion and qualification update (2026-11-04)

[PR #1450](https://github.com/joris97jansen/borrowser/pull/1450) is merged and
[issue #1119](https://github.com/joris97jansen/borrowser/issues/1119) is closed as
completed. [Hosted CI run 41864987854, attempt 1](https://github.com/joris97jansen/borrowser/actions/runs/41864987854)
passed for PR head `d26f44e732f2413422f57d791f31770fe1b19eaf` against the baseline
above. The actual tested synthetic merge was
`48322eafaa59f8f534bb59fa6eba03d36b9ded08`, tree
`96995c3ff5b10e9ca3960d7a65f481f57ae94f12`. Published merge
`b0e617eda1c6142c91cff3a3cc89ea538f4aa958` has that same tree.

The Ubuntu Rust job passed the required addressing suite (22 PASS / 2 UNSUPPORTED),
AG8 suite, dependency/static gates and default/HTML5 workspace lanes. The separate
native Linux x86-64 job passed ownership and pinned real Chromium/composition/suite
qualification, including explicit execution of required ignored browser tests.
The Rust lanes filter duplicate native_procfs wrappers; native qualification
runs in the required Linux job. Other fuzz/regression and AG4 synthesis jobs
passed; the special-branch proxy timing job was skipped. This is not Chromium
addressing capture/comparison support or a successful macOS `make ci` run.

This later evidence supersedes the pending supported-host qualification and
not-closeable conclusion at the historical local checkpoint below. The nine
native macOS failures and ignores remain failures/ignores, not retrospective
passes. [AH10](ah10-url-origin-resource-addressing-foundation-closeout.md#hosted-qualification-verified-on-2026-11-04)
records exact job links, integration boundaries and milestone-wide limitations.
AH9 qualification does not establish AH10 revision qualification or GitHub lifecycle
state.

## Implemented scope

AH9 integrates existing addressing behavior with AG. It introduces no new URL,
origin, discovery, loading, or security-policy semantics. The local addressing
suite has 22 runnable fixtures (65 rows) and two explicitly unsupported creation-context
fixtures. Each runnable fixture contains a nonempty homogeneous table and an
independently authored structured expected snapshot.

## Authority and execution

| Observation | Production authority |
| --- | --- |
| URL parsing/resolution, components, serialization, ordered diagnostics/failure | `addressing::BrowserUrl` and its existing result/accessor types |
| URL-derived origin and same-origin result | `BrowserUrl::url_origin`, `Origin::clone`, `Origin::is_same_origin` |
| Complete resource address, capabilities, retrieval key | `ResourceAddress` and `ResourceIdentityKey` |
| Committed document URL, fallback/active/frozen base, retained origin | Browser's `DocumentAddressState` and accepted publication path |
| HTML occurrences and resolution-base source | Existing Browser HTML inventory; its producer records the selected base source without changing discovery behavior |
| CSS values, decoding, spans, contexts, diagnostics, completeness | Existing CSS discovery reports |
| CSS resolution outcome and external base provenance | Existing Browser `CssSourceReferences::outcome` and retained admitted response base |

Pure URL/origin/key cases call only addressing APIs. They do not parse a document,
produce a frame, or start a runtime. Document cases traverse navigation, response
admission, the production parser runtime, and accepted Browser publication. They
observe committed state without requiring settled loads or a rendered frame.
AG5's DOM/style/layout/canvas observations still require their accepted production
frame and retain their existing common projections and Chromium formats.

`Tab::observe_document_addressing` exists only under the existing
`conformance-observation` feature. It borrows the committed DOM/address state and
owns temporary production inventory records. It neither changes the page nor
retains an observation cache. Missing document state is `None`; source identity or
stylesheet input failures are errors. Base source keys pass through the existing
DOM identity bridge; consumers do not cast parser keys into materialized IDs.
Feature-gated accessors expose existing records, not a second discovery model.
`observe_addressing_request` correlates an observed issued command with retained
pending/completed state; it cannot create or settle a request.

## Closed fixture representation

Existing phase declarations remain unchanged. Addressing declarations use the
same discovery root, safe reference rules, sorted identities, eligibility and
expectation metadata. For example:

```json
{
  "id": "addressing/example",
  "requires_javascript": false,
  "classification": { "kind": "runnable" },
  "addressing": {
    "kind": "url",
    "rows": [
      { "reference": "../icon.svg", "base": "https://example.test/css/main.css", "purpose": "image" }
    ]
  },
  "expectation": { "kind": "match" },
  "expected": { "kind": "addressing", "path": "example.expected.json" }
}
```

The four kinds are `url`, `origins`, `keys`, and `documents`. Their schema is
`crates/conformance/src/addressing/input.rs`, shared with the build reader.
There is no generic action language, variable binding, registry, or plugin API.
Origin operands explicitly request independent determination or `clone_left`.
Document rows contain HTML asset names, requested/final response URLs, required
resource commands, optionally supplied CSS responses, and an optional explicit
same-document destination. They do not express arbitrary lifecycle event streams.
Existing Browser lifecycle tests continue to cover mutations, retirement,
rejection, stale publications and ordered frozen-base transitions.

Addressing tables must be nonempty. Addressing declarations cannot also contain
phase inputs/observations or reference relationships. Assets are embedded at build
time; HTML/CSS references must have the proper extension, remain within the
metadata directory/descendants, and refer to regular UTF-8 files. Orphan HTML,
CSS and expectation files fail discovery. Full catalogue validation precedes any
selection or artifact identity publication, including excluded selections.
Expected and actual families/table cardinalities must agree with the input.
Origin/key comparisons require both operands' observations. Document snapshots,
request cardinality and retained-origin observations follow the declared case.
Missing fields, including absent optional URL components omitted instead of
written as JSON null, are malformed evidence.

## Offline resource accounting

The existing phase fixture path still rejects subresource commands. The shared
HTML publication helper supplies no permission to serve resources.

The addressing driver starts no network/file runtime. Each emitted resource
command must match an unused declaration by production kind and exact URL. It
checks actual request and stylesheet-slot identities, and every required declared
command must occur. Two stylesheet attachments at the same retrieval URL remain
two issued requests. Two image occurrences may share one production request;
occurrence inventory and request accounting are independent.

Declared pending requests must first be observed and remain pending. They receive
no fabricated success/error events. Supplied CSS starts from a real issued slot
and traverses Browser's NetworkStart/Chunk/Done admission with immutable response
metadata. Actual CssChunk/Done commands go through the existing CSS runtime; only
its real decoded/completion events are applied. Missing, excess, mismatched or
misordered commands/events, uninstalled CSS, and timeout are errors. The driver
also verifies the final production pending/completed states.

Supplied CSS uses the admitted final response URL as its base. Missing and invalid
final URLs retain their respective provenance and existing resolution outcomes.
The request URL and document base are never substitutes. Distinct final response
metadata exercises the existing response handoff, not redirect processing.
`supplied_final_url` records the exact fixture transport input; the CSS base
observation records what the retained production base can actually expose.

AI3 migrates supplied responses to the immutable `net` contract and retains the
actual issued `FetchRequest`. The historical empty final-URL sentinel denotes
absent provenance at fixture ingestion. For HTML, the `document-default` missing
provenance row now exercises production admission rejection: it must emit the
matching cancellation, commit no addressing snapshot, and remain rejected after
completion. Its expected snapshot list is empty rather than inventing the request
URL as a selected response. The two valid-URL rows retain their original complete
snapshots. CSS missing/invalid response-base observations remain unchanged.

## Exact snapshots and privacy

The harness's structured observations contain exact controlled synthetic values.
They do not enable a raw production Debug mode. Existing URL/origin/key/Browser
Debug redaction remains unchanged. No human-readable Debug text is parsed.

Snapshots retain complete normalized URL components/serialization, explicit
base source, diagnostics/failures, URL-derived versus retained-document origin,
comparison operands/creation/result, resource address/purpose/key/capabilities,
source nodes/attributes and CSS slices/decoded values/spans, blocking contexts,
completeness, notice truncation and unevaluated participation.

`present`, `unavailable` with a reason, and `inapplicable` are separate states.
HTML discovery exposes stored attribute values, not original attribute spelling
or HTML byte offsets. CSS source slices and decoded URL values remain separate;
CSS offsets/line/column refer to the existing decoded source buffer, not the HTML
file. Missing HTML byte positions are explicitly unavailable.

Element and stylesheet labels are local to an observation. Element labels follow
ordinary DOM preorder; stylesheet labels correlate actual issued attachments and
observed sources. Underlying IDs are used only for correlation. Snapshots contain
no parser/runtime allocation IDs, CSS input IDs, memory addresses, timestamps, persistent
hash IDs, or checkout-dependent paths. Occurrences, duplicate references, source
order, repeated diagnostics and overlapping contexts are never deduplicated.
Opaque-origin equality uses real production identities, never labels or `null`.

`*.expected.json` files are the structured debug snapshots. Reports print actual
snapshots and exact differing field paths. They are internal regression contracts,
not public APIs, capture wire formats or a complete Fetch/cache model.

## Coverage and expectation review

| Fixtures | Independent basis and protected boundaries |
| --- | --- |
| url-normalization/diagnostics/failures | AH2/AH3 contract and native parsing/diagnostic tests; normalization, typed failures, ordered recoverable events |
| relative-resolution/opaque-base | AH3 explicit-base and opaque-path rules, unchanged base and query/fragment distinctions |
| origins-tuples/opaque/blob-fallback | AH5 tuple normalization, clone versus fresh opaque identity, file policy, no-entry blob fallback |
| resource-keys/scheme-capabilities | AH6 fragmentless keys, meaningful query/credentials, purpose independence, operation-aware capability matrix |
| document-default/base-selection/base-rejected/base-frozen/document-file-origin | AH4/AH5 accepted document state, first applicable base, rejected first candidate, frozen value retention, retained origin |
| html-resources/html-boundaries | AH7 finite matrix, namespace/template boundaries, empty/whitespace values, source occurrences and production request counts |
| css-inline/css-contexts | AH8 decoding/spans, inline bases, unsupported contexts, local/empty/invalid shapes, authored-before-cascade inventory |
| css-external/css-missing-base/css-invalid-base | AH8 actual admitted final response base, unavailable-base independence and typed failures |
| unsupported-about-inheritance/unsupported-blob-entry | Explicit missing creation contexts; these exclusions do not replace runnable URL-only assertions |

Expected transformations, origins, keys, booleans and discovery records are
authored from these contracts and source-bound fixture reasoning independently
of captured output. The initial 22 runnable expectations matched on their first
execution. Additional invalid-base/diagnostic-isolation rows, unknown CSS contexts
and supplied-response input fields were independently authored before their own
execution. No expectation was regenerated to silence a mismatch. Existing pinned WPT qualification remains separate broader evidence;
AH9 neither revendors WPT nor claims complete URL or browser conformance.

A parsed unsupported scheme can pass an independently asserted capability result.
A blocked CSS context can pass an inventory assertion without implementing that
feature. Fixture `unsupported` metadata excludes unavailable requested behavior;
nonblank reasons are required and expectations are omitted. Use `known_failure`
only for a demonstrated semantic mismatch: it yields XFAIL on mismatch and XPASS
on unexpected match. Malformed data, missing observations and execution errors
remain ERROR even under known-failure metadata. No artificial XFAIL is checked in.

## Adding and running a case

1. Add a declaration under `crates/conformance/fixtures/addressing/` with a
   nonempty table of an existing kind. Reuse existing assets where suitable.
2. Write the expectation independently. Review each transformation, failure,
   origin relationship, key, source label/span and capability against the AH
   contract. Running a formatter can help review; output generation is not proof.
3. Classify unsupported behavior explicitly. Do not use exclusions or XFAIL to
   hide malformed input, unexpected commands or required AH9 coverage.
4. Rebuild and run the exact identity, then the suite. Discovery does not change
   suite membership: add an explicitly reviewed ID and corresponding coverage/cost
   assertions when a new case belongs in the fixed required suite.

```sh
cargo run -q -p borrowser-conformance --locked -- addressing/css-external
make conformance-addressing
cargo test -p borrowser-conformance --locked ah9_
cargo test -p browser --features conformance-observation --locked addressing_observation
```

Inspect the actual snapshot and `difference:` field paths on FAIL. Exit 1 means
FAIL/XPASS, exit 2 means configuration/execution/publication error, and existing
incomplete precedence remains unchanged. A matching expected parse failure is a
PASS. The addressing suite expects 22 PASS and two UNSUPPORTED, exit 0; mandatory
coverage tests ensure required runnable cases cannot be replaced by exclusions.
Independent-process CLI and source-free rebuild tests protect determinism and
embedding, including CSS-only edits that must produce a real expectation failure.

## AG modes, validation and remaining scope

`--suite addressing` is a fixed local suite in the existing catalogue. `make ci`
and the existing Rust CI job run it. Original `ci`/`extended` members and their
qualified verdicts are unchanged. No-argument local runs include the new cases.

Chromium/AWS comparisons explicitly report addressing observation unsupported
for that operation, with no capture descriptor, invented sample, independent
local PASS, or common phase projection. The independent local suite remains
required. Addressing-only comparison returns 3 without backend configuration or
activity; capture-only selection returns 2. Mixed phase/addressing selections
retain phase capture and explicit incomplete addressing coverage. Chromium wire,
comparison schema identity, and AG4 manifest format remain unchanged; AG4 capture
descriptors enumerate only capturable phase documents while source identity binds
the complete source tree.

Validation uses targeted tests, subsystem/feature lanes, executable smoke and
full CI. Fresh clean-baseline conformance testing reproduced nine native macOS
ownership failures (177 passed, 13 ignored); one host-permission baseline probe
also failed. These are baseline evidence, not permission to ignore new failures.
Final implementation validation is recorded separately with the review packet.
Required supported-native qualification cannot be replaced by a different host's
results or marked passed without execution.

No new fetching, redirect processing, HTTP caching, cookies/storage, navigation
history, service workers, CORS/CSP/mixed-content enforcement, JavaScript URL API,
full security policy, parser source-position infrastructure, computed URL-valued
CSS property, resource loader, or Chromium addressing capture is added. Inherited
and override base contexts, associated blob entries, broader discovery/loading,
and dependency conformance limitations remain. AH10 owns milestone-wide closeout.

## Local implementation validation checkpoint (2026-11-04)

| Gate | Result |
| --- | --- |
| AH9 targeted conformance regressions | Seven unit tests plus declaration/comparison/capture CLI regressions pass; mandatory 22 fixtures / 65 rows match their snapshots. |
| Browser | 363 unit tests plus five integrations pass with `conformance-observation`; both new observer regressions pass. Standalone feature-off check passes. |
| Addressing / CSS runtime | All 48 addressing tests and three CSS runtime tests pass, preserving pinned qualification and existing origin/key/debug coverage. |
| Explicit Browser/HTML5 consumer lane | Pass, including Browser and 934 HTML unit tests plus integrations. |
| CLI / fixture build | Four ordinary CLI tests, four capture tests, eleven comparison tests and eleven nonignored build tests pass. Existing real-browser ignores remain explicit. |
| Isolated embedding | Pass: CSS-only edit causes FAIL with unchanged expectation, restoration matches, copied executable runs after source removal. Optional real Chromium check not executed. |
| Dependency policy / static gates | Both approved 35-package closures verified; formatting and workspace default/HTML5/hardening lint pass. Parser revision, policy and graph/checksum guards unchanged. |
| CI continuation | Previously unreached workspace crates and AWS CLI tests pass; parser/external/fatal/golden/real-page/script/WPT lanes, six fuzz smokes, debug/release builds in both modes, benches and entity check pass. |
| Source preparation | Seven tests pass, existing opt-in real Docker test skipped. Initial sandbox run denied local `ps`; permitted host rerun passes. |
| Aggregate conformance | Final current run: 184 pass, nine native macOS ownership failures, 13 existing ignores. Clean baseline: 177 pass, same nine named failures, 13 ignores; a baseline host-permission probe also fails. |
| Full CI / HTML5 aggregate | Both fail at those native ownership tests. Separately passing continuation targets do not turn either aggregate green. |
| Supported native Chromium compatibility | Pending: required Linux x86-64 pinned-Chromium qualification cannot run on this Darwin arm64 host; no commit/push/hosted run was performed to obtain it. |

Architecture review finds no remaining implementation change required by AH9.
AH9 is **not closeable yet**: aggregate/native qualification evidence remains
outstanding. Neither AH9 nor Milestone AH is claimed complete; milestone-wide
closeout remains AH10 / #1120.
