# AI9: Fetch/network fixtures, observations, and AG integration

> Current-status qualification (2026-11-21): AI9 is merged through PR #1483 as
> master `f77a6834`; #1129 is closed. The uncommitted and pending-qualification
> statements below remain historical checkpoints. The PR contains later
> committed-source evidence, and [AI10](ai10-fetch-http-networking-response-handling-foundation-closeout.md)
> records successful exact-source merged-baseline aggregate CI separately from
> retained local failures. This does not qualify AI10's new documentation bytes,
> establish Chromium networking equivalence or formally close Milestone AI.

Issue: [#1129](https://github.com/joris97jansen/borrowser/issues/1129).
Milestone AI — Fetch, HTTP Networking, Redirects, and Response Handling.
Implementation base: `b9df9172c58c52fc384a4553cf8222d6132f6fcb`.
Status: uncommitted implementation. Working-tree checks are distinct from
committed-source, supported-native, hosted, issue-closure and milestone evidence.

## Authority and scope

[AI1](ai1-fetch-http-networking-response-contract.md) owns subsystem boundaries;
AI2–AI8 own request admission, immutable responses, fake/real acquisition,
redirects, errors, bodies and metadata. Addressing alone parses/resolves URLs and
constructs retrieval identities. Conformance ingests data, executes those APIs,
projects their facts and compares independently authored expectations. It does
not resolve Location, follow redirects, extract MIME/length, classify network
errors or implement consumer acceptance. Browser, CSS, Layout and Paint retain
their existing responsibilities. Document/resource lifecycle work remains AJ/AK.

AI9 adds 16 required runnable declarations / 36 rows and two explicit unsupported
declarations under `crates/conformance/fixtures/networking/fetch/`. The required
suite executes memory-only fake networking. Focused net tests use controlled
loopback HTTP. New tests require no public endpoints or public DNS; installed
dependencies and optional existing Chromium qualification are separate concerns.

## Actual acquisition observation

`net::fake::FakeNetwork::fetch_stream_observed` accepts the ordinary request,
execution route, cooperative token and NetEvent callback plus a concrete
`FakeAcquisitionEvent` callback. The ordinary facade remains available. A private
adapter implements the existing private backend interface; no backend trait or
reader is exposed. Frozen route tables remain immutable and shared.

Entered records capture the actual request and execution route immediately on
acquisition entry. Returned records distinguish acquired metadata, a configured
network error, missing route, and response-construction rejection. The fake
lookup has one implementation. Observers belong to individual executions and do
not create shared history, response queues or call-order dependencies.

Acquired metadata precedes shared Fetch validation/history attachment. Retained
redirect chains come from actual response events, never reconstructed route
registrations. A pending target is not an entered acquisition. Entry is not
proof of transmission, server receipt, body completion or consumer acceptance.
Debug for the new records delegates to existing redacted production types.

## Authoring a networking fixture

Add a `*.fixture.json` declaration with exactly one networking input family and,
for a runnable case, a separate `*.expected.json` snapshot:

```json
{
  "id": "networking/fetch/example",
  "requires_javascript": false,
  "classification": {"kind": "runnable"},
  "networking": {
    "kind": "fetch",
    "rows": [{
      "request": {
        "url": "https://fixture.invalid/data",
        "purpose": "document", "route": "html", "method": "GET",
        "headers": [], "initiator_url": null, "requesting_origin_url": null
      },
      "routes": [{
        "url": "https://fixture.invalid/data", "method": "GET",
        "outcome": {
          "kind": "response", "status": 200, "headers": [],
          "body": {"kind": "bytes", "bytes": [0, 255]},
          "reported_response_url": "https://fixture.invalid/data",
          "read_failure_after": null
        }
      }]
    }]
  },
  "expectation": {"kind": "match"},
  "expected": {"kind": "networking", "path": "example.expected.json"}
}
```

Each row executes one request against a finite registration table. Header pairs
contain a name and exact value bytes; order and duplicates matter. Response
bodies are bytes or `{"kind":"repeat","byte":128,"len":2097153}` for bounded
limit evidence. A route may instead declare `network_error` with an existing
supported acquisition category. Optional read failure uses the existing fake
reader offset. No action language, script, scheduler, registry or plugin exists.

The build reader rejects mixed families, unknown/duplicate fields, invalid
paths, missing references, orphan expectations and duplicate IDs. Runtime
preflight also validates typed route/header construction and duplicate normalized
routes without materializing repeated bodies. Complete catalogue validation
precedes filtering and artifact identity publication. No runtime fixture files
are opened; additions/edits/removals trigger rebuilds. Input/bounds modules are
explicit rebuild dependencies. Fixture discovery still rejects symlinks.

Author expectations from the AI contracts and explicit scenario reasoning before
inspecting actual observations. There is no blessing command. The initial corpus
was independently authored and matched on first execution. Hashes of repeated
synthetic bodies are derived from their explicit byte patterns, not captured
engine output. A changed input must fail an unchanged expectation.

Use UNSUPPORTED with a concrete reason for genuinely deferred behavior. The two
checked-in examples describe JavaScript Fetch and HTTP cache; they execute
nothing and contain no expectations. An expected rejection of supported negative
input is a PASS. `known_failure` is reserved for demonstrated semantic mismatches:
XFAIL on mismatch, XPASS on match. Malformed/missing evidence remains ERROR.
There is no artificial checked-in known failure.

## Completion and failure protocol

Request-construction rejection never dispatches. Fake HTTP dispatch rejection
returns before spawning. Both yield typed rejection with zero acquisitions and
zero NetEvents; neither requires a terminal event. FTP construction, file dispatch
and redirect-to-file fixtures exercise their respective production boundaries.

Accepted execution requires valid correlation/order, all acquisition returns,
exactly one final Done/Error, complete data-channel draining through disconnection,
producer-lifetime closure and a clear delivery-failure latch. Accepted pre-cancelled
execution has zero acquisitions and one terminal Cancelled event. Receiving a
terminal event or lifetime closure alone never establishes completed collection.

The same concrete lifecycle validation applies to independently authored
expectations and collected actual snapshots, before match/known-failure
classification. These are required evidence relationships, not a second Fetch
implementation:

| Terminal boundary | Required evidence |
| --- | --- |
| URL/request construction rejection | Unavailable admitted request, no acquisitions or Start, empty body, final URL inapplicable |
| Dispatch rejection | Present admitted request, no acquisitions or Start, empty body, final URL inapplicable |
| Done | Present admitted request, acquired response preceding a present Start, immutable Start/terminal response, progress matching delivered bytes and request route |
| Error after Start | The same response and progress evidence, including present progress for **zero** delivered bytes |
| Error before Start | Present admitted request, no delivered body or selected final URL, retained response availability consistent with acquisition evidence; progress may be unavailable before admission |

Only cancellation can settle an accepted execution without any acquisition.
Acquisition failures cannot be followed by another acquisition. A parsed selected
Start URL requires a present selected-final-URL observation; unknown Start state
is incomplete evidence. Validation does not derive a redirect chain, interpret
headers/MIME, resolve URLs, recalculate production limits or classify a failure.
Complete but incorrect semantic values remain ordinary FAIL/XFAIL comparisons.
For example, consistently observed wrong HTTP status is a semantic mismatch;
Done without Start or an Error that loses post-Start progress is harness ERROR.
`known_failure` cannot conceal either malformed expected or malformed actual
evidence. Runtime catalogue validation checks every expectation before selection,
including non-core fixtures that would otherwise be excluded by a named suite.

Two invocation-owned callback endpoints send through a synchronous 64-message
queue; sending may block. Only the endpoints own senders on an independent
`Infallible` lifetime channel. No lifetime messages are sent. Dispatch consumes
both callback owners; no runner-owned sender or callback clone remains.

The absolute ten-second deadline is checked before receiving, after dequeueing,
after processing and before success, including continuously ready data. Callback
projection/send failure is latched and requests cooperative cancellation. Sends
are attempted once; disconnection drops the unsent value without panic or retry.

On collection failure, preserve the primary error, request cancellation and
immediately destroy the data receiver. This releases blocked sends and discards
queued data rather than draining after failure. Observe producer closure for at
most one additional second. Closed cleanup leaves a local case ERROR. Unresolved
closure is fatal: no later row/fixture executes, exit is 2, and no completed report
is published. Diagnostics preserve the primary cause and cleanup status.

Producer closure proves that these callback owners cannot publish further
records. It does not prove a worker join, completion of every destructor, immediate
acquisition abort or thread termination. The existing Fetch API exposes no join.
Unresolved CLI execution stops rather than assuming stronger cleanup.

## Observations and resource bounds

Snapshots include admitted request facts, actual acquisition results, selected
Start, terminal response/error/progress, entered redirect history, selected final
URL and delivered-body evidence. Missing/unavailable/inapplicable facts remain
explicit. A failed target's attempted URL never replaces retained source-response
provenance. A selected final URL is unavailable without selected-response evidence.

404/500 remain responses. Missing Location selects its response; empty Location
resolves normally; repeated/fatally invalid Location fails without another
acquisition. Cycles use the twenty-follow budget: 21 actual acquisitions and 20
entered hops, with no acquisition 22. Exactly twenty follows can still complete.
Recoverable URL diagnostics do not become fatal failures. Diagnostic projections
only expose diagnostics already available from typed production values.

Declared encoded Content-Length, delivered decoded bytes, acquired metadata,
body completion and consumer acceptance are independent. The gzip fixture checks
that separation. Actual CSS bodies at 2 MiB and 2 MiB + 1 exercise the unchanged
production limit. Existing tests retain every-route decoded/encoded boundaries.

| Harness resource | Limit |
| --- | ---: |
| Declaration bytes before parse (all AG declarations) | 256 KiB |
| Networking expectation file before parse/embedding | 1 MiB |
| Queue messages | 64 |
| Owned projection record before enqueue | 64 KiB |
| Combined actual/expected row projection storage | 1 MiB |
| Combined fixture projection storage | 4 MiB |
| Retained networking projection storage per invocation | 16 MiB |
| Complete compact JSON row observation | 256 KiB |
| Differences / diagnostic bytes per row | 32 / 8 KiB |
| Staged report, including formatting/newline | 32 MiB |
| Materialized registered body data per active row | 8 MiB |
| Individual repeated body | 2 MiB + 1 |

Sequence bounds are enforced during deserialization. Projection charging includes
owned string/byte/vector capacities and record storage before excess retention.
Expected row storage is reserved during execution. Queue storage and encoded
output are separate budgets. These are observation-storage contracts, not an
allocator-exact RSS claim. Static embedded data, allocator/channel bookkeeping
and transient production values are distinct; container cardinalities and typed
production/input limits also bound them.

All repeated acquired/Start/terminal facts and JSON byte-array expansion count.
Overflow is harness ERROR; required evidence is never truncated into a match.
Serialization uses capped buffers, not an unlimited serialization checked later.
Diagnostics compare complete typed observations first and remain independently
bounded. Body length/SHA-256/32-byte prefix/suffix are updated incrementally as
chunks arrive. Large bodies are never retained merely to summarize afterward.

Materialize one row's fake bodies, execute it through producer closure, release
its network/body storage and retain only bounded observations before the next row.
Exact synthetic snapshot contents/hashes do not create a raw production Debug
mode or change default redaction. No human-readable Debug output is parsed.

## AG reporting, comparison and required execution

```sh
make conformance-networking
cargo run -q -p borrowser-conformance --locked -- networking/fetch/response-200
cargo run -q -p borrowser-conformance --locked -- --compare --suite networking
```

`--suite networking` is fixed reviewed membership: 16 PASS / 2 UNSUPPORTED,
exit 0. Required members must remain runnable with match expectations. Discovery
alone never expands a named suite. Required execution is wired into `make ci`
and the existing required `rust_conformance` workflow job after package build.

Local reports add a derived `GROUP networking/fetch` line before the unchanged
aggregate SUMMARY. Mixed runs count the networking subset separately; every
case contributes exactly once to aggregate local counts. Existing verdict and
exit precedence remain 2 (ERROR), 1 (FAIL/XPASS), 3 (INCOMPLETE), 0.
Local networking XFAIL and XPASS reports also print the declared known-failure
reason, consistently with the existing AG fixture families.

Comparison output is `borrowser.cross-engine-phases.v4`. Runnable networking
cases execute local assertions after all capture cleanup. `borrowser_status`,
`borrowser_expectation` and typed `local_observations` expose actual local facts;
`coverage.chromium` explicitly says unsupported and `fields` is empty. A local
PASS creates no equal field; a local FAIL increments expectation failures, not
cross-engine differences. Networking never increments executed phase documents.
`summary.groups["networking/fetch"]` derives local counts. Comparison incompleteness
is separate from local case success.

Phase facts now live under `environment.phases`: present/value with document URL,
viewport, scale, sample pixel and colour space when phase documents executed;
otherwise inapplicable. These facts apply only to phase cases. Existing phase
capture/provenance and field comparison meanings remain intact. Chromium wire v2,
AG4 manifest v2 and capture descriptors are unchanged. Networking JSON participates
in source identity but creates no capture descriptor.

Networking-only comparison returns 3 without Chromium/AWS configuration or
activity; a local mismatch returns 1. Capture-only networking returns 2 without
launch. Fatal harness errors return 2 without publishing a partial comparison.
Reports are staged within the output budget. Physical write failures can leave
accepted bytes; exit 2/publication_failed requires consumers to reject them.
Existing structural Fetch diagnostics remain on stderr. CLI orchestration must
not hold a stderr lock while workers need it to log and settle.

The following v4 excerpts show selected fields; the wire report includes complete
typed local observations and expected snapshots. A single response-200 comparison
has exit status 3:

```json
{
  "schema": "borrowser.cross-engine-phases.v4",
  "environment": {"phases": {"state": "inapplicable"}},
  "cases": [{
    "test_id": "networking/fetch/response-200", "family": "networking/fetch",
    "borrowser_status": "PASS", "status": "incomplete",
    "local_observations": {"rows": [{"body": {"kind": "bytes", "bytes": [0, 255]}}]},
    "borrowser_expectation": {"metadata": "match", "status": "PASS", "differences": []},
    "coverage": {"chromium": {"state": "unsupported", "reason": "Networking observations are local-only; Chromium networking capture is unavailable"}},
    "fields": []
  }],
  "summary": {
    "borrowser_cases": {"passed": 1, "failed": 0},
    "groups": {"networking/fetch": {"passed": 1, "failed": 0}},
    "equal_fields": 0, "different_fields": 0, "expectation_failures": 0,
    "executed_documents": 0, "incomplete_cases": 1
  }
}
```

A mixed selection containing canvas/root and response-200, with an equal canvas
capture, also exits 3. Its phase environment applies only to canvas/root:

```json
{
  "schema": "borrowser.cross-engine-phases.v4",
  "environment": {"phases": {"state": "present", "value": {
    "document_url": "https://borrowser.invalid/ag1/fixture.html",
    "viewport": [640, 480], "device_scale_factor": 1,
    "sample_pixel": [32, 32], "color_space": "srgb"
  }}},
  "cases": [
    {"test_id": "canvas/root", "family": "phases", "borrowser_status": "PASS",
     "status": "equal_for_requested_scope", "fields": [{"id": "canvas_color_rgb8", "status": "equal"}]},
    {"test_id": "networking/fetch/response-200", "family": "networking/fetch",
     "borrowser_status": "PASS", "status": "incomplete", "fields": []}
  ],
  "summary": {
    "borrowser_cases": {"passed": 2, "failed": 0},
    "groups": {"networking/fetch": {"passed": 1, "failed": 0}},
    "equal_fields": 1, "different_fields": 0, "expectation_failures": 0,
    "executed_documents": 1, "incomplete_cases": 1
  }
}
```

A networking mismatch changes local status/counts and expectation failures, and
exits 1; it contributes no different cross-engine field. A fatal collection,
validation or shutdown failure exits 2 and publishes neither example report.

## Validation and remaining qualification

Direct regressions cover actual/isolation observations, rejection/cancellation,
queue pressure/disconnection, deadlines with ready records, closure and missing
records, resource/encoding limits, interior-byte digest changes, independent
mismatches, discovery, embedding and v4 reporting. Controlled test producers use
channels and joined test-owned threads; no sleep-based correctness assertions.

Loopback selected-response tests independently assert 200/404/500, duplicate
headers, metadata and bytes. The malformed-target case independently expects
real BadStatus -> MalformedResponse, and an existing fake ConnectionFailure.
Only common lifecycle/provenance invariants are compared across those failures.
No fake fault capability is added to force category equality.

### Working-tree checkpoint, 2026-11-21

These checks were launched from the original checkout against the combined AI9
and authorized middleware syntax cleanup, based on `b9df9172`. Earlier isolated
implementation results are separate and are not substituted for these checks.

| Check | Observed result |
| --- | --- |
| `cargo test -p borrowser-conformance ai9 --locked` | 25 unit and 5 integration tests passed; the subsequent full unit run also passed the final retained-producer regression |
| `cargo test -p net ai9 --locked` | Both observer tests and both focused loopback tests passed, including bounded observer closure |
| `cargo test -p net -p runtime_net --locked` | 284 tests passed |
| `cargo test -p browser --features conformance-observation --locked` | 395 tests passed |
| `cargo test -p borrowser-conformance --bin borrowser-conformance --locked` | 214 passed, 9 failed, 13 ignored; native ownership failures below |
| `cargo test -p borrowser-conformance --locked --test cli --test fixture_build --test chromium_cli --test comparison_cli --test aws_cli` | 37 passed, 5 opt-in tests ignored |
| `cargo test -p borrowser-conformance --locked --test fixture_build isolated_cargo_rebuild_and_standalone_execution -- --ignored --nocapture` | Passed: discovery/rebuild mutations, embedded expectations, contained vendor licence links, and source-free copied executable; 263.43 seconds |
| `make conformance-networking conformance-ci conformance-addressing` | Networking 16 PASS / 2 UNSUPPORTED; CI 7 PASS; addressing 22 PASS / 2 UNSUPPORTED |
| `make fmt-check lint` | Passed; workspace all-target Clippy denies warnings |
| Standalone Python unit checks: `test_ci_qualification.py`; `test_prepare_ag4_artifact.py` | 64 passed; 20 passed / 1 opt-in Docker test skipped, respectively |
| Local CLI smokes | Networking exits 0; comparison and AWS comparison exit 3 without configured backends; capture-only networking exits 2 without launch |

The nine failing conformance tests are `native_capture_failure_diagnostics`,
`native_cleanup_diagnostic_failures`, `native_lifecycle_regressions`,
`native_macos_bsd_ownership_states`, `native_process_and_pipe_contract`,
`native_resource_completion_and_shutdown`, `native_shutdown_uses_capture_deadline`,
`native_startup_exit_event_classification`, and `native_temporary_artifact_containment`.
On this macOS host they report unavailable live-process executable ownership
evidence, with incomplete cleanup diagnostics. They are failures, not passes or
fresh supported Linux qualification. The aggregate conformance run is not green.

Python unit results above are standalone working-tree checks, not execution of
the guarded committed-source qualification entry point. The vendor inventory and
its policy digest describe proposed bytes only; historical acceptance and patch
records are unchanged. The full `make ci`, exact committed-source checks,
supported native/pinned Chromium and hosted qualification were not completed.

### Focused evidence correction, 2026-11-21

The checkpoint above precedes this correction. Four new regression tests exercise
well-formed JSON with missing/contradictory lifecycle evidence under both `match`
and `known_failure`, through expectation parsing, execution intake and actual
report validation. Controlled closed acquisition/Start/Error sequences distinguish
missing zero-byte progress (ERROR/exit 2) from present zero-byte progress (valid
evidence, ordinary comparison). Production pre-Start metadata rejection, earlier
acquisition failures, construction/dispatch rejection and pre-cancellation remain
valid. Complete HTTP status mismatches retain FAIL/XFAIL behavior; local XFAIL and
XPASS explanations are checked explicitly. The original tests reproduced the
three reported defects before the correction.

The isolated rebuild regression uses an added non-core fixture, leaving every
core suite member runnable with a matching expectation. Missing Start, request
or acquisitions must fail CLI catalogue preflight with ERROR/exit 2 under both
metadata dispositions, even when selecting a different valid core fixture. Valid
controlled mismatches and stale known-failure metadata exercise FAIL/exit 1,
XFAIL/exit 0 and XPASS/exit 1 through the rebuilt CLI. No malformed evidence is
injected as an already-classified `Execution::Error` to prove detection.

| Correction check | Observed result |
| --- | --- |
| `cargo test -p borrowser-conformance ai9 --locked` | 29 unit and 5 integration tests passed |
| `cargo test -p borrowser-conformance --bin borrowser-conformance --locked` | 218 passed, the same 9 native ownership tests failed, 13 ignored |
| Affected `cli`, `fixture_build`, `comparison_cli`, `chromium_cli`, `aws_cli` integration targets | 37 passed, 5 opt-in tests ignored |
| Extended isolated rebuild / CLI / source-free regression | Passed in 349.85 seconds, including non-core malformed match/known-failure preflight, FAIL/XFAIL/XPASS exits/explanations, unchanged required suite and source-free execution |
| `make conformance-networking fmt-check lint` | 16 PASS / 2 UNSUPPORTED; formatting and workspace all-target Clippy passed |

The native failures listed above remain unresolved failures. No committed-source
or historical acceptance record was rewritten; committed-candidate, supported
Linux/pinned Chromium, hosted and full-CI qualification remain pending. The
authorized middleware cleanup and its current-source inventory are preserved.

Working-tree checks do not qualify a committed candidate. No temporary commit,
source-verifier bypass or acceptance-hash update is permitted for this checkpoint.
Committed-source checks, supported-native Linux/Chromium and hosted CI remain
separate required evidence. The issue is not closeable until that evidence exists.

Deferred: JS Fetch/XHR, service workers, cache, cookies/storage, full CORS/CSP/
mixed-content/security policy, authentication UI, HTTP/2, HTTP/3, WebSockets,
navigation history and document/resource lifecycles. AI9 claims no Chromium
networking agreement, broad standards compliance or milestone completion.
