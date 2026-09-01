# AG7 static structural reference comparison

AG7 defines Borrowser's deterministic, no-JavaScript reftest-style facility.
It compares two in-repository documents through the production HTML, CSS,
Layout, and Paint pipeline. It is inspired by the WPT match/mismatch relation
model, but it is not a WPT reftest runner and it does not compare pixels.

## Versioned inventory relation

`borrowser-conformance-fixture-v3` is the first AG fixture format that authors
reference polarity. Its mandatory outer `[reference]` table contains exactly
one `kind`, one `relation`, and one `path`. `kind` is `semantic` or
`structural`; `relation` is `match` or `mismatch`. The outer declaration is the
only relation authority. V1 and V2 remain unchanged and their existing
references imply `match` when represented in manifest V3.

Reference kind is an orthogonal inventory claim, not a comparator selector.
It classifies whether the authored assertion is semantic or structural; the
outer observation surface and validated single-owner profile set select the
actual Layout- or Paint-owned bytes. The runner carries both axes into report
V2 and rejects owner/profile contradictions, but it does not reinterpret
owner output or use `kind` to choose an alternate representation.

Manifest `borrowser-conformance-manifest-v3` publishes `reference_kind`,
`reference_relation`, and `reference_path` in stable field and test-id order.

## Execution-package containment

The parent of `execution_package.entry_path` is the package root. A V3 test
path, reference path, and every execution support path must be below that root.
Entry, test, reference, and support roles are distinct; test and reference are
not support paths. The nested paired descriptor resolves all inputs relative
to its package root and rejects absolute paths, parent traversal, symlinks,
non-regular files, duplicate paths, and non-portable components.

The outer payload set—test, reference, and support paths—must exactly equal the
nested descriptor's resolved HTML and stylesheet set. AG2's generic
`MAX_EXECUTION_SUPPORT_PATHS_V2` transport ceiling is 256. Paired rendering V1
adds a stricter 64-support-path sublimit; it does not redefine AG2's ceiling.

## Paired rendering fixture V1

`borrowser-paired-rendering-fixture-v1` declares one test document, one
reference document, a single-owner profile set, and shared execution variants.
Each side authors the complete AG6 stylesheet coordinates: path, origin,
order, source, and the optional UA namespace. UA, user, and author origins use
the existing production mapping. UA source is zero and requires a namespace;
user and author sheets forbid namespaces. Source IDs and authored order are
unique, and authored order is strictly increasing independently on each side.

Duplicate profiles, variants, stylesheet coordinates, or path roles are
invalid. After validation, profiles and variants use their existing typed
orders. Layout cases select only `layout-phase-output`, `layout-sizing`,
`layout-advanced-flow`, or `layout-flex`. Paint cases select only
`paint-semantic-artifact`, `paint-order`, `paint-stacking-contexts`,
`paint-layering`, or `paint-operations`. Owner serializer headers remain the
canonical artifact-version authority.

## Capture, oracle, relation, and policy

AG6 and AG7 share one production execution/capture primitive. It parses HTML
and stylesheets, constructs production cascade inputs, computes style, runs
Layout and Paint, and asks the selected owner serializers for canonical bytes.
AG6 compares those bytes with authored snapshots. AG7 invokes the same
primitive for both documents under the identical
`RenderingExecutionVariantId`, then compares the complete owner bytes exactly.

Both sides are attempted for every runnable variant, including when the test
side terminates unsuccessfully. Relation evaluation occurs only after both
sides completely capture every selected observation. Failures, resource
limits, incomplete serialization, allocation outcomes, and invariant failures
remain terminal outcomes and cannot satisfy `mismatch`.

The relation truth table is:

| Oracle | Relation | Semantic result |
| --- | --- | --- |
| equivalent | match | pass |
| different | match | mismatch |
| equivalent | mismatch | mismatch |
| different | mismatch | pass |

Expected-result policy is applied afterward. Capability-unavailable cases are
not runnable, not attempted, and reported `not-run`. Only a runnable semantic
mismatch may become XFAIL; a runnable semantic pass under expected-fail is
XPASS. Other terminal outcomes are unexpected outcomes.

## Evidence and limits

Complete exact observations decide the oracle. Report evidence is diagnostic
only. Exact comparison produces `equivalent` or `different` plus an
allocation-free first-difference locator before any excerpt is materialized.
Failure to allocate or encode bounded evidence is a tooling/reporting failure;
it is not a comparison invariant and cannot be projected through AG3 as a
semantic pass, mismatch, XFAIL, or XPASS. AG7 retains the first differing typed profile and one-based line, full
observation lengths, explicit missing-line state, original line lengths, and
UTF-8-safe excerpts of at most 1,024 source bytes per side. Original line
lengths and excerpts include a line terminator when the owner bytes contain
one, so a line-ending-only difference remains visible. It retains no
surrounding context or unbounded diff. A successful mismatch retains its
difference; an equal unsuccessful mismatch reports that no differing profile
was found.

Report V2 uses its actual canonical escaping implementation to calculate the
serialized evidence size exactly. Each item must fit the AG7 per-variant
`REFERENCE_DIFFERENCE_SERIALIZED_BYTES_V1` ceiling of 16 KiB and the existing
1 MiB individual mismatch, 32 MiB run-wide retained-evidence, and 32 MiB
bounded report limits. There is no per-package evidence pool.

Authored inputs retain the AG2 64 KiB descriptor ceiling and AG6 limits of 4
MiB HTML per side, 4 MiB per stylesheet, 16 MiB stylesheets per side, 64
stylesheets per side, five selected profiles, and 16 variants. AG7 additionally
bounds the pair to 8 MiB combined HTML, 16 MiB combined stylesheet bytes, and
64 combined stylesheet support paths. Owner capture is bounded to 8 MiB per
observation and 8 MiB cumulatively per side, or at most 16 MiB of successfully
capturable owner observations for one paired variant. Across the maximum 16
variants, 256 MiB is therefore the theoretical cumulative successfully
capturable owner-observation volume, not a simultaneously retained pool and
not a comprehensive bound on parser, CSS, Layout, Paint, CPU, or total memory
work.

## Controlled environment and non-claims

AG7 controls available width under `synthetic-text-metrics-v1`. It provides no
viewport height, DPR, platform fonts, shaping/raster metrics, scrolling,
surface state, screenshots, or pixels. It does not parse WPT relation links,
reference graphs, fuzzy metadata, `reftest-wait`, external resources,
JavaScript, browser automation, Browser/runtime lifecycle behavior, or
cross-engine execution. A future raster facility requires a complete viewport
and font environment, backend capture, image artifacts, tolerance semantics,
and bounded pixel diagnostics while preserving the capture/oracle/relation/
policy separation established here.
