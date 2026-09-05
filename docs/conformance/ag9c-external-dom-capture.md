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

## Validation and remaining scope

Run Rust codec/fixture, runner aggregate/advisory, provenance, feature-boundary,
and report compatibility tests. The independent inspector tests run locally:

```sh
node --test tools/conformance/web-observable-dom-tree-v1.test.mjs
```

Node is a local test prerequisite, not browser infrastructure. A missing Node
run is an explicit validation limitation, never an assumed pass.

Real-browser capture admission, trends, aggregate CLI/CI publication, broad WPT,
CSSOM, dynamic DOM, browser runtime/automation, and raster/pixel comparison remain
outside this issue. AG9c does not establish broad browser compatibility, WPT
compliance, or AG milestone completion.
