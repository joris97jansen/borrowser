# AG9c selected DOM advisory operations

AG9c implements `web-observable-dom-tree-v1` in two independent producers and
selected local advisory comparison infrastructure. It does **not** currently
support collecting a real browser capture: no admitted mechanism establishes
the frozen parser/input context. The checked-in capture registry remains empty.

## Ownership and canonical handoff

`html` owns production DOM semantics and `html::conformance` observations.
`html-test-support::web_observable_dom` owns the V1 codec, with no AG identities,
tracks, registry attachments, expectations, or verdicts.
`FixtureEvaluation::serialize_web_observable_dom_tree_v1()` borrows the actually
produced reference result selected by the existing private `reference_result`.
It never reads expected snapshot bytes or invokes parsing. In the AG adapter,
`evaluate_and_normalize_once` returns the SAME evaluation used by
`apply_evaluation`; the selected observer borrows it before it is dropped.
The existing baseline, declared, and generated AE parity deliveries all remain
unchanged. One AG evaluation is not necessarily one parser invocation.

No canonical tree is retained in `NormalizedCaseResult` or `AggregateRun`.
No debug `html5-dom-v3`, `ObservationArtifact`, or report bytes are read back.

## Explicit operation scope

The library entry point `run_repository_aggregate_for_selected_dom_operation`
accepts the ordinary named-lane request plus `SelectedDomOperationRequest`,
which contains an exact `AggregateVariantKey`. It returns
`SelectedDomOperationRun`: the unchanged sealed `AggregateRun`, exact selection,
and a separate comparable-observation success/failure. Selection never makes an
otherwise excluded or unavailable case run. Unknown or unsupported selections
are separate observation failures, not changed aggregate outcomes.

Call `compare_external(repository_root)` on that operation only after execution.
It loads/reconciles AG9b evidence against its immutable run, verifies the source
files, and processes attachments matching the exact selected variant and surface.
`SelectedDomAdvisoryOperation` explicitly reports `SelectedVariantOnly` scope,
selected variant, comparable version, total attachment count, in-scope count,
outside-scope count, and evaluated attachment/result pairs. Its retained immutable
registry evidence exposes all outside-scope attachments without assigning them
any verdict, failure, or unsupported classification. Results follow AG9b's stable
typed attachment ordering and remain keyed by attachment plus track identity.
Even a registry containing only matching attachments does not change the API's
selected-operation scope into a complete-population claim.

The one retained 8 MiB Borrowser artifact is an operation/lifecycle memory bound,
not a global limit on AG9 logical cases or advisory populations. Future callers
may explicitly request independent selected operations sequentially. AG9c has
no hidden rerun loop, recovery evaluation, multi-observation retention budget,
or mechanism to recover observations already discarded from the same run.

## Independent written-contract vectors

Both Rust and JavaScript tests read the hand-authored, reviewed files in
`tests/contract-vectors/web-observable-dom-tree-v1/`. Inputs are independently
constructed by each producer's tests. Neither producer generates expected bytes
or the other implementation. Synthetic capture tests reference those same files;
copying one into a temporary confined repository is test setup, not another
source of golden truth. The corpus is outside the AG2 discovery root
`tests/conformance/fixtures` and does not change inventory or manifest membership.

## Codec and resources

The frozen AG9 grammar remains authoritative: all six node kinds, separate
ordinary/template-content children, namespace/name/prefix relationships,
UTF-8 tuple attribute ordering, duplicate rejection, and V1 escapes are retained.
Element prefixes are structurally absent in current production expanded names;
the external inspector explicitly rejects non-null `Element.prefix`.
Actual processing instructions remain distinct from HTML-created comments.

Every Rust append checks arithmetic and exact encoded length, checks the limit,
fallibly reserves, and appends. Attribute and iterative traversal workspaces are
also fallible. The output ceiling is exactly 8,388,608 UTF-8 bytes including
headers and final LF. Escaping expansion counts. No partial artifact escapes on
excess, overflow, invalid structure, or allocation failure. The JavaScript
inspector likewise bounds encoded construction, rejects unpaired surrogates,
and sorts by UTF-8 bytes rather than JavaScript UTF-16 ordering.

## Inspector and real-capture admission

Algorithm: `tools/conformance/web-observable-dom-tree-v1.mjs`.
Configuration: `tools/conformance/web-observable-dom-tree-v1.config.json`.
Identity/version: `web-observable-dom-tree-v1-inspector`, `1`.

`inspectWebObservableDomTreeV1(document)` is a read-only DOM inspector, not a
capture mechanism. It uses standardized node/doctype/attribute/data properties
and HTML template `content`. It does not use inner/outer HTML, XMLSerializer,
DOMParser, source serialization, or uncanonicalized NamedNodeMap ordering.
It rejects unsupported nodes/namespaces/prefixes, malformed Unicode and attribute
relationships, repeated tree/fragment associations, and detectable shadow state.
It reads `shadowRoot` only on Element nodes, never infers a shadow interface from
an unrelated `.host` property, and rejects free DocumentFragment/ShadowRoot nodes.
Template content must be a DocumentFragment whose standardized
`getRootNode({ composed: true })` is itself. A referenced ShadowRoot resolves
through its host instead, even when detached or closed, and is rejected without
cross-realm constructor checks or generic property-name probing. These checks do
not discover closed roots that are not exposed to inspection. The inspector
cannot prove the absence of undetectable shadow state or prior mutation.

`captureWebObservableDomTreeV1()` always reports unsupported capture mechanism.
The public Rust advisory operation likewise rejects unproven capture context.
Private synthetic-test admission exercises equivalent/different behavior; no
public flag can bless synthetic or unproven real evidence as a valid capture.

Before a future mechanism can be admitted, it must establish ALL of:

- delivered body equals the declared raw fixture bytes and digest;
- MIME `text/html` and fixed UTF-8 decoding, with the exact delivery configuration;
- scripting disabled in the target before parsing;
- parser completion before inspection;
- no target scripts, mutation, custom-element reactions, events, timers, CSSOM,
  or post-load effects influencing the observation;
- controlled resource/network policy;
- out-of-band inspection without enabling or mutating the target.

A completed DOM or a provenance declaration cannot prove this history. A normal
JavaScript-enabled page load followed by a DevTools snippet is invalid. No
browser is downloaded, discovered, launched, or automated by AG9c. Do not add a
capture to the real registry merely to demonstrate the workflow.

## Current contributor workflow

The supported operations are inspection/codec validation, declaration review,
and explicit local baseline publication. There is no admitted operational
real-browser collection path. Keep these three facts separate:

- the Rust/JavaScript comparable-DOM and advisory machinery is implemented;
- the repository currently declares zero captures, tracks, attachments, and notes;
- a future real-browser mechanism must establish every admission condition above.

Validate the independent inspector with
`node --test tools/conformance/web-observable-dom-tree-v1.test.mjs` and Rust
support with `cargo test -p html-test-support --features parser-fixtures --locked`.
The shared vectors are independently authored machinery tests, not observations
collected from named browser versions. Do not add them to the real registry.

Before any future capture is proposed, review original fixture bytes and digest,
source/revision, engine/build/platform identity, algorithm/configuration hashes,
invocation and resource policy, and proof of the frozen parser/input context.
The [provenance fields](ag9-cross-engine-comparison-reporting.md#external-capture-provenance)
and [registry validation](ag9-cross-engine-comparison-reporting.md#deterministic-registry-validation)
define byte/identity validation. Passing those checks does not prove delivery,
scripting state, parser completion, or non-mutation. A DevTools snippet or
ordinary JavaScript-enabled browser load is not an admitted mechanism.

Reviewer-authored notes may describe limitations with an explicit existing DOM
attachment and no capture reference. Tracks preserve their invariant series
tuple; attachments cannot invent a capture or broaden selected scope. See the
[contributor track/note workflow](../../tests/conformance/README.md#external-sources-provenance-tracks-and-notes).
Baseline publication validates all declarations even without evaluation. With an
empty registry a selected baseline can publish without a comparison point;
publication success is not real-browser equivalence.

[AE13e external parser expectations](../html5/ae13e-external-fixture-and-snapshot-workflow.md)
and [AG8 source adaptation](ag8-wpt-import-filtering-classification.md) are existing
provenance-backed external-output workflows. They do not admit browser captures
under this contract. Their sufficiency for parent #1108's selected comparison and
external-browser collection goal remains explicitly unresolved in the
[AG9f audit](ag9f-requirement-evidence-closeout.md#parent-comparison-finding-and-follow-up-draft-f1).
This records a gap, not an accepted deferral or a reason to loosen admission.

## Exact source bytes and existing capture authority

`VerifiedCaptureSourcesV1::load` reads both source files using same-opened-object
confined reads with a 65,536-byte ceiling each. It hashes raw bytes without
newline normalization or JSON reserialization. Source identity/version resolves
to the reviewed source bytes compiled into this tooling build; a different
source requires review/rebuild rather than silently retaining the same version.
Comparison checks the actual source digests against provenance. Hash agreement
proves only source identity, not browser context.

`external-test-provenance` remains the sole capture-ID owner.
`ValidatedExternalCaptureV1::verify` is the sole public constructor and checks
artifact length, digest, and equality of the supplied claim with the canonical
recomputed capture ID. Grammar validation and verified artifacts alone cannot
construct this final authority. Synthetic tests use fixed reviewed claims through
this same verification path; no production convenience constructor exists.
The separate public V1 grammar validator returns only validation success/error,
not trusted artifact or capture authority.

After AG9b validation, comparison only borrows `capture.artifact().bytes()`.
It never reopens `artifact_path`, even if the file was deleted or replaced.

## Verdicts, failures, and evidence

Only valid, compatible produced observations can yield `Equivalent` or
`Different`. Missing/incomplete/failed observations, source/fixture mismatch,
unproven context, invalid artifacts, invariant/resource/allocation failures,
and operation preparation failures are typed errors. Registry/source preparation
failures do not fabricate per-attachment results. Selection exclusion is scope,
never a comparison verdict. Real capture currently remains unsupported.

A `Different` includes deterministic first-byte and one-based-line coordinates,
complete observation lengths, missing/present line state, original line lengths,
and UTF-8-safe excerpts. Each side retains at most 1,024 source bytes, with an
explicit `excerpt_omitted` flag when shortened. Full artifacts are never trimmed.
Serialized evidence is bounded to 16 KiB each. Retained evidence accounting
includes both serialized bytes and decoded excerpts within the 16 KiB slot and
4 MiB operation pool. An evidence
failure yields a comparison failure, not a verdict missing required evidence.

Evidence format `borrowser-advisory-dom-first-difference-v1` uses the existing
AG report scalar escaping (NOT the comparable DOM escaping). Field order:
`format`, `first-differing-byte` (zero-based), `one-based-line`,
`borrowser-byte-length`, `external-byte-length`, then Borrowser and external
side records. Each side has `side`, `line-state`; present sides additionally have
`original-line-bytes`, `excerpt`, `excerpt-omitted`. Lengths exclude the line LF
for line records and include all bytes for artifacts. Physical lines are LF.
This is a distinct versioned DOM evidence grammar, not AG7 rendering evidence.
AG9 freezes AG7's ceilings and UTF-8 safety, not its line-record bytes. Excluding
the delimiter describes field content; validated V1 observations require LF-only
framing and a final LF, so differing line-ending conventions cannot produce a
valid advisory pair. CR/LF in DOM strings are escaped field data.

Preparation errors belong to `html-test-support::parser_fixture::ComparableDomPreparationError`:
unavailable, execution failure, resource exhaustion, incomplete, invariant, or
unsupported context. Only its `Serialization` variant contains codec errors.
The runner preserves this boundary as `DomObservationFailure::Preparation`;
projection structure/attribute/size/allocation failures remain codec-owned.

External evidence cannot modify AG3, execution/selection/outcomes/policy,
aggregate identity/accounting, or existing parser/CSS/rendering/aggregate report
bytes. Normal CI uses ordinary entry points and has no external dependency.

AG9d may passively seal a completed selected operation into a historical
baseline. The baseline records its actual `selected-variant-only/completed`
scope and complete separately reconciled declaration membership. It never
promotes that operation to `all-declared`, even when all declarations happen to
match the selection. An operation-wide AG9c failure produces no completed value
and therefore cannot be sealed as evaluated evidence.

## Validation and remaining scope

Run Rust codec/fixture, runner aggregate/advisory, provenance, feature-boundary,
and report compatibility tests. The independent inspector tests run locally:

```sh
node --test tools/conformance/web-observable-dom-tree-v1.test.mjs
```

Node is a local test prerequisite, not browser infrastructure. A missing Node
run is an explicit validation limitation, never an assumed pass.

Real-browser capture admission, broad WPT,
CSSOM, dynamic DOM, browser runtime/automation, and raster/pixel comparison remain
outside this issue. AG9c does not establish broad browser compatibility, WPT
compliance, or AG milestone completion.

## AG9e selected baseline publication

`aggregate baseline --lane L --external-evidence repository --compare-dom-test
TEST_ID [--check]` publishes existing Baseline V1 with a completed selected
operation. The ID uses `TestId::parse`. The CLI constructs only `DomTree` plus
`Singleton(ExecutionVariantId::new(SingletonExecutionVariant::Singleton))`;
AG9c fixes the comparable surface to `WebObservableDomTreeV1`. There is no
composite-key syntax, wildcard, rendering selection, or first-match behavior.

The selected observation comes from the existing observer during that exact
aggregate execution. Typed unknown/unsupported identity failures return exit 3.
A valid selection that was not attempted due to eligibility/lane state remains
AG9c observation evidence. Other reviewed per-attachment failures remain evidence,
not process failure or Borrowser policy. Operation-wide registry/source failures
produce no baseline. No observation is rerun or recovered.

The CLI seals through `seal_baseline_from_selected_operation`; outside-scope
membership remains unevaluated, including when there are zero matching points.
It never promotes selected scope to all-declared. Real capture admission remains
unsupported. Baseline publication success means artifact success, not proof of
browser equivalence. Separate external-detail publication would require a new
reviewed format outside AG9e.
