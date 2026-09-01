# AG6 Layout and Paint structural conformance

AG6 runs static, no-JavaScript rendering packages through production HTML,
CSS, Layout, and Paint. It compares subsystem-owned canonical structural
snapshots with authored expected files. It does not compare pixels or execute
reference documents.

The stable AG2 `TestId` identifies one logical test. Each available width under
`synthetic-text-metrics-v1` is an independently executed variant. AG3 V1
metadata remains authoritative for the whole logical test and cannot express
different policy by width or observation profile.

Rendering packages list all stylesheets explicitly. AG6 does not discover
`<style>` or `<link>` elements and injects no Browser default UA stylesheet.
Inline `style=""` declarations remain CSS-owned element input. The parser uses
the public production whole-document parser facade, and CSS uses the
parser-selected document mode. The controlled host does not enable HTML's
internal DOM-construction API.

The exact HTML profile emits EOF, uses the production tokenizer and tree-builder
limits, does not coalesce text, and tracks errors and counters with at most 128
stored errors (`debug_only = false`). A degraded parse is an attempted resource
failure, and the typed production degradation reasons remain available as
ordered report evidence. Every explicit stylesheet is bounded by
`SyntaxLimits::default().max_stylesheet_input_bytes`, parsed with the production
stylesheet parser, and rejected when `stats.hit_limit` is set. Complete sheets
are mapped into the production cascade coordinates: one optional UA sheet uses
source `0` plus a required namespace; user and author sheets use their unique
in-memory generation source and forbid namespaces. Authored order is unique and
strictly increasing. Rendering V1 has no media or condition field.

`synthetic-text-metrics-v1` reports width as Unicode scalar count multiplied by
computed font size in CSS px and `0.5`; line height is computed font size
multiplied by `1.2`. Whitespace, combining marks, NBSP, and zero-width scalars
receive no special treatment. This is artificial Layout input, not evidence
for shaping, graphemes, fallback, bidi, kerning, ligatures, rasterization, or
real typography. Height, replaced-resource loading, networking, Browser
runtime retention/repaint state, and backend/GPU output are not available.
Because the controlled metric is established by the rendering adapter itself,
it is variant identity rather than an external AG3 environment requirement;
the ready seed records therefore have an empty AG3 environment-requirement
list. This does not create variant-specific AG3 policy.

Layout profiles are `layout-phase-output`, `layout-sizing`,
`layout-advanced-flow`, and `layout-flex`. Paint profiles are
`paint-semantic-artifact`, `paint-order`, `paint-stacking-contexts`,
`paint-layering`, and `paint-operations`. A fixture may select only profiles
owned by its AG2 primary observation. Every selected profile must match for a
variant to pass. Layout and Paint canonical snapshot headers remain the
serializer-version authority.

## Rendering fixture V1

The nested descriptor uses `format = "borrowser-rendering-fixture-v1"` and
rejects unknown fields at every level. Its shape is:

```toml
format = "borrowser-rendering-fixture-v1"
id = "logical-ag2-test-id"
profiles = ["layout-phase-output"]

[input]
html = "document.html"
stylesheets = [
  { path = "author.css", origin = "author", order = 0, source = 0 },
]

[[variants]]
environment = "synthetic-text-metrics-v1"
available_width_css_px = 320
expectations = [
  { profile = "layout-phase-output", snapshot = "expected-01.txt" },
]
```

Width is an integer from 1 through 16,777,216 CSS px. There is no height or
device-scale input. Paths are opaque package-local file references and carry no
test, variant, or profile identity. Profiles, variants, and expectations are
unique; each variant has exactly one expected snapshot for every selected
profile. The loader orders variants and profiles by their typed values rather
than authored TOML order.

The V1 resource limits are AG2's 64 KiB transport limit for the descriptor,
4 MiB for HTML, the production CSS syntax maximum (currently 4 MiB) per
stylesheet, 16 MiB for all stylesheet source, 64 stylesheets, the AG report's
8 MiB ceiling per expected or actual observation,
40 MiB derived maximum actual owner bytes across five maximum-sized
observations, 32 MiB cumulative expected snapshot source, 16 variants, 5 selected profiles,
and 80 `(variant, profile)` expectation pairs. All cumulative arithmetic is
checked. The maximum 64 stylesheet and 80 expectation support files total 144,
below AG2's 256 support-path maximum. Expected snapshot bodies are loaded per
variant; the report's separate 8 MiB observation ceiling is not presented as an
authored-input memory limit.

For each stylesheet, the loader validates the path, performs the individually
bounded read, decodes UTF-8, and only then applies cumulative stylesheet-byte
accounting. This ordering is part of AG6 fixture failure compatibility: an
individual path, file-size, or UTF-8 failure is not replaced by a later
aggregate-limit diagnosis.

The runner derives the descriptor ceiling from AG2 and the individual expected
snapshot ceiling from the AG report contract, then validates that those values
are compatible with rendering fixture V1; rendering test support is not a
second authority for them. The HTML and per-sheet ceilings match the current
production parser input scale; the 16 MiB
sheet aggregate permits several maximum-sized controlled sheets without making
64 such sheets resident. Five profiles is the largest closed owner vocabulary,
16 variants is sufficient for a compact width matrix, and their product is the
80-pair ceiling. Eight MiB permits existing canonical structural artifacts while
remaining bounded during owner serialization. The derived 40 MiB actual
aggregate preserves AG6's original five-times-eight-MiB capture behavior; the
32 MiB expected aggregate keeps authored package input bounded independently
of AG's separately enforced run-wide report budget.

## Attempt and report contract

Package loading and AG2/AG3 reconciliation happen before an attempt. A malformed
harness-ready package aborts the rendering run as a runner-level fixture error.
A harness-not-ready logical case remains visible with zero variants. A valid
but ineligible package retains every variant as `not-attempted`; a valid,
eligible attempt starts exactly at `html::parse_document`.

For an attempted variant, every selected owner profile must be produced and
match. Any difference produces the aggregate semantic mismatch and ordered,
typed profile evidence containing the first mismatching one-based line and the
expected/actual byte lengths without retaining an unbounded textual diff.
Resource/execution failures, unretainable observations,
and final writer/profile invariants remain distinct terminal outcomes. Expected
failure is derived only from the aggregate semantic result: a mismatch can be
XFAIL and a pass XPASS, while execution, incomplete, and invariant failures are
unexpected outcomes.

`borrowser-conformance-rendering-report-v1` has execution-variant granularity
and reports both `logical-case-count` and `execution-variant-count` at its root.
Incomplete observations explicitly retain
`phase = "observation-serialization"`.
It retains logical AG metadata separately, identifies the typed synthetic
environment and available width, reports typed profiles and terminal state,
and embeds exact successfully retained owner-produced canonical bytes. It adds
no AG serializer-version field: each Layout/Paint snapshot header is the
authoritative canonical-format version.

The basic Paint-operation seed asserts exactly one 40×20 red background for an
empty `div`; it deliberately contains no text node. The AB-derived layering
seed uses two 30×20 relatively positioned normal-flow boxes. Their authored
integer z-index values create negative and positive Paint stacking contexts,
while their supported Layout geometry places the blue box at y=0 and the red
box at y=20. These expected artifacts assert authored size, normal-flow
placement, and Paint ordering rather than preserving incomplete absolute-
positioned geometry.

AG7 owns test-document/reference-document comparison. AC Browser/runtime
epochs, invalidation, artifact reuse, retained work planning, and repaint
surfaces remain outside AG6 even when they consume Paint artifacts. The AC7
seed observes only the freshly constructed Paint-owned semantic artifact; it
contains no retained key, lifecycle action, reuse counter, epoch, or repaint
assertion.

## AG7 shared capture boundary and report evolution

AG7 extracts AG6's production execution and owner-observation capture into one
internal primitive. AG6 still loads `borrowser-rendering-fixture-v1`, invokes
that primitive once, and applies the existing authored-snapshot oracle. It does
not change stylesheet coordinates, execution variants, owner profiles,
canonical bytes, mismatch classification, or AG3 policy semantics.

AG7 invokes the same primitive for test and reference documents. The primitive
continues to own strict host inputs and bounded capture only; HTML, CSS, Layout,
and Paint remain production owners of their semantics. Rendering report V1 is
retained as an exact snapshot-only compatibility serializer. The repository
CLI now emits `borrowser-conformance-rendering-report-v2`, which represents
both authored-snapshot and document-reference oracles without changing V1
bytes.
