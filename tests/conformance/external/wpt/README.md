# Bounded AG8 WPT source population

`sources.toml` is the human-authored authority for one immutable WPT revision,
seven accounted source records, six closure files, exact paths and SHA-256
digests, licence/attribution, and one derived-fixture lineage. Files below
`raw/<revision>/` are exact committed Git blobs from that revision. They are not a WPT
checkout and must not be expanded by directory or filename allowlisting.

All seven original WPT assertions remain visible and are not selected for
direct execution. A separate derived-adaptation decision selects one exact-copy
Paint-semantic adaptation; it is not a WPT raster result. The proof covers
JavaScript/testharness, WebDriver,
multi-reference raster semantics, pinned static resources without raster
support, WPT substitution/server requirements, and dynamic `reftest-wait`.
Supporting reference/resource files are closure material, not additional
accounted records.

`source-metadata.toml` owns evidence-backed interpretation annotations that are
not safely inferable from bounded WPT/HTML metadata. Its `NoJs` declarations
are positive reviewed facts; absence of a detected executable script leaves
no-JS compatibility unresolved and the no-JS filter reports
`not-yet-established`. `selection-policy.toml`
independently owns the seven filter axes and derived-adaptation eligibility.
Neither artifact defines source population membership, and changing policy
cannot change the immutable interpreted requirements.
`../assessment-profile.toml` owns generic repository-stable production,
harness, environment/resource, and representation assessments. Neither file
contains current-host availability.

`accounting-summary.toml` is deterministic generated review evidence. It is
not editable metadata and never overrides `sources.toml` or pinned bytes. Use:

```text
make check-conformance-wpt
make update-conformance-wpt-summary
```

Materialization is explicit and accepts a local WPT Git checkout containing
the exact declared revision; checkout `HEAD` may differ. The importer reads only committed Git objects for
declared paths, rejects symlink/executable/non-blob modes, verifies every hash
and preflights committed object sizes against individual and aggregate bounds
before reading blob bodies. It atomically publishes one immutable
`raw/<revision>/` closure
without replacing earlier valid revisions. It never trusts
working-tree bytes, disables Git lazy fetching, and never fetches or executes
the suite. Missing promised objects are a materialization failure.

Trusted record-local reference limitations remain in accounting: graph
depth/node/edge bounds, cycles, unsupported reference paths, and incomplete
declared reference closure do not erase sibling records. Closure totals are
computed from file roles. Fuzzy declarations retain the path of the graph node
that authored each opaque value, but remain non-executable pixel metadata. The
exact-copy adaptation accepts only a lineage reference node that is the actual
target of the source's single upstream match edge.
