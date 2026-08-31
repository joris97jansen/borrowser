# AG2 conformance fixture inventory and manifest contract

## Status and ownership

This document is the normative AG2 contract for repository-owned conformance
fixture discovery, validation, stable identity, and manifest generation. It
implements the inventory foundation described by the AG1 architecture contract;
AG1 remains authoritative where this document is silent.

The test/tooling-only `conformance-test-support` crate owns generic inventory
bookkeeping. It has no dependency on HTML, CSS, Layout, GFX/Paint,
Browser/runtime, or `html-test-support`, and production crates do not depend on
it. Subsystems continue to own browser semantics and their canonical
observations. `html-test-support` remains the canonical parser-fixture runner.
AG2 discovers declarations; it neither executes fixtures nor duplicates a
subsystem implementation.

## Root and bundle layout

The inventory root is `tests/conformance/fixtures/`. Grouping directories may
organize bundles for reviewers, but their names and file extensions never
define identity, scope, observation, provenance, capability, or executability.

A directory containing `fixture.toml` is one logical fixture bundle and one
discovery leaf. Discovery does not register descendant bundles. It separately
integrity-scans every descendant so nested descriptors and unsafe content cannot
hide below a discovered bundle.

V1 bundle contents are default-deny:

- `fixture.toml` is required and authoritative;
- `test_path` names one required regular payload file;
- optional `[reference].path` names one required regular reference file;
- subdirectories are permitted only to contain those declared files;
- every other regular file is rejected as undeclared;
- nested `fixture.toml` files, symlinks, non-regular entries, and names outside
  the V1 portable component grammar are rejected.

Assets or resources beyond these declarations require a future versioned schema
change. Files outside a bundle are invalid rather than implicit fixtures.

AG4 adds `borrowser-conformance-fixture-v2` for one explicitly bounded opaque
subsystem execution package. V2 keeps `fixture.toml`, `test_path`, and optional
`reference.path` unchanged and adds:

```toml
[execution_package]
entry_path = "parser/fixture.toml"
support_paths = ["parser/parse-errors.txt", "parser/tokens.txt"]
```

`entry_path` must be nested and establishes the package root. `test_path` and
every support path must be beneath that root. The entry, test payload,
reference when present, and each support file are separate declarations;
duplicates are errors. V2 allows only the exact nested `fixture.toml` named by
`entry_path`; every other nested descriptor remains invalid. Every contained
regular file is still default-deny, symlinks and non-regular entries remain
invalid, and support paths are limited to 256. The schema deliberately has no
package-kind field and no unrestricted assets or resources directory. Generic
AG validation treats package bytes as opaque and never interprets parser,
CSS, Layout, Paint, or runtime semantics.

AG5 uses the same unchanged V2 envelope for CSS. A CSS-ready package declares
one nested `borrowser-css-fixture-v1` descriptor and every authored input and
expected snapshot as explicit support paths. The CSS adapter—not AG2—requires
exact outer/nested IDs, maps the nested execution profile to AG2's authoritative
CSS observation surface, reconciles `test_path` with that profile's primary
stylesheet or selector-list input, and enforces required/optional/forbidden
fields before production execution. It infers no semantics from extensions or
directories. The default-deny file-set, containment, portability, symlink,
regular-file, duplicate, missing-file, and 256-support-path rules remain AG2
authority.

Fixture payload bytes are opaque to AG2. Discovery verifies filesystem shape but
does not read, decode, normalize, parse, or hash payloads. In particular,
extensions do not establish a text contract; CRLF, lone CR, invalid UTF-8, and
other exact byte sequences remain unchanged. Only infrastructure-owned
`fixture.toml`, the generated manifest, and documentation have an LF policy.

## Versioned descriptor

Every bundle uses this strict V1 shape:

```toml
format = "borrowser-conformance-fixture-v1"
id = "stable-logical-id"
scope = "static-html-css-no-js"
observation = "dom-tree"
test_path = "test.html"

[source]
kind = "native"

[reference]
kind = "semantic"
path = "reference.html"

[metadata]
description = "A concise inventory description."
```

`[reference]` is optional; every other shown field is required. Unknown fields
are errors at every table level. Schema evolution uses a new explicit `format`
value and parser, never permissive field acceptance.

The validation pipeline is:

```text
versioned serialized descriptor
  -> strict TOML deserialization
  -> typed semantic and path validation
  -> ValidatedInventory
  -> ConformanceManifest
  -> canonical TOML bytes
```

No manifest can be built from an unvalidated serialized descriptor.

V1 remains strict and unchanged. V2 has the same required common fields and
requires the strict `[execution_package]` table shown above. Unknown root or
nested fields are errors in both versions; V2 is not a permissive extension of
V1.

## Stable logical identity

`id` is an explicit `TestId`, not a filesystem identity. Its V1 grammar is 1 to
128 ASCII bytes of lowercase kebab case: it begins with `a-z`; subsequent
characters are `a-z`, `0-9`, or single separating hyphens; it cannot end with a
hyphen or contain consecutive hyphens. Examples are
`html-tokenizer-basic-document` and `css-cascade-basic-author-rule`.

IDs remain stable when bundles or payloads are reorganized. They are never
derived from paths, checkout locations, content hashes, timestamps, random
values, filesystem metadata, or provenance identity. Exact duplicate IDs and
ASCII case-folding collisions are repository errors. A case-unsafe ID is also
invalid independently, which prevents aliases on case-insensitive hosts.

## Independent inventory axes

`scope = "static-html-css-no-js"` says only that the fixture is authored for
Borrowser's named static HTML/CSS, no-JavaScript conformance domain. It does not
state capability availability, harness readiness, execution eligibility,
expectation, lane selection, stability, execution-attempt state, or outcome.
Those remain independent AG concepts and are not implemented in AG2.

`observation` declares the subsystem-owned surface a later adapter may observe:

- `html-tokenizer`
- `html-tree-construction`
- `dom-tree`
- `css-parsing`
- `css-selectors`
- `css-cascade`
- `computed-style`
- `layout-geometry`
- `paint-operations`
- `browser-runtime-semantic`

`source.kind` independently records provenance. V1 supports `native` for an
in-repository purpose-built fixture and `controlled-static-page` for an
in-repository controlled real-page-style source. A controlled static page is
not an observation category. V1 has no `SourceForm`: native provenance alone
does not define an independent authored test form, and speculative WPT or
external formats do not belong in AG2.

An optional `[reference]` records only a declared relation. `semantic` denotes
equivalence at a later subsystem-owned semantic observation; `structural`
denotes equivalence at a later subsystem-owned structural observation. The
relation combines independently with `observation`. It does not implement or
claim comparison behavior, rendered-output/WPT reftests, screenshots, raster or
pixel equality, or fuzzy-image support.

## Deterministic discovery and diagnostics

Discovery is iterative. At each directory it materializes entries, converts
names to UTF-8, and sorts them before processing. A directory with
`fixture.toml` registers one logical bundle and begins a separate iterative
integrity scan of that bundle's full descendant tree. A directory without a
descriptor is only grouping; regular files there produce a missing-descriptor
diagnostic. The descriptor and every declared path must be regular files.

One portable component grammar applies to every organizational directory, bundle
directory, descendant directory, payload filename, reference filename, and
`fixture.toml` beneath the fixture root. A component is 1 to 128 ASCII bytes,
begins and ends with lowercase `a-z` or `0-9`, and otherwise contains only
lowercase `a-z`, `0-9`, `-`, `_`, or `.`. Consecutive dots are forbidden. The
basename before the first dot cannot be a Windows device name: `con`, `prn`,
`aux`, `nul`, `com1` through `com9`, or `lpt1` through `lpt9`.

The grammar excludes uppercase aliases, Unicode normalization and case-folding
ambiguity, control characters, backslashes, colons, platform-reserved
punctuation, leading or trailing dots/spaces, and `.`/`..` semantics by
construction. Borrowser does not emulate host filesystem case folding. Both
discovered names and descriptor-declared relative paths use the same component
validator. Serialized repository-relative paths join validated components with
`/`; traversal outside the fixture bundle and symlinks are rejected.

Discovery never follows a symlink. Every `fixture.toml` read is limited to 64
KiB plus one sentinel byte; observing the sentinel produces
`DescriptorTooLarge`, never a TOML diagnostic. The reader cannot allocate or
consume materially beyond that bound. Fixture payload bytes are not loaded.

Validation collects independent errors where practical. Diagnostics carry a
stable repository-relative path and typed `InventoryDiagnosticKind`; final
ordering is path, diagnostic-kind rank, then stable typed detail. Filesystem
enumeration order therefore cannot select the visible error. Root failures that
make safe traversal impossible stop discovery before bundle validation.

## Canonical checked-in manifest

`tests/conformance/manifest.toml` is a generated review artifact. Its only
source of truth is the set of validated bundle-local `fixture.toml` files.
AG4 evolves the generated review artifact to Manifest V2 because reviewers
must be able to see which descriptor version and explicitly declared package
files were validated. Manifest V2 begins with:

```toml
format = "borrowser-conformance-manifest-v2"
```

It then contains one `[[tests]]` record per logical ID, ordered bytewise by
`TestId`. Required fields occur in this exact order: `id`, `fixture_format`, `fixture_path`,
`test_path`, `metadata_path`, `scope`, `observation`, `source_kind`. Optional
`reference_kind` and `reference_path` follow in that order. V2 fixture records
then contain `execution_entry_path` and the sorted
`execution_support_paths` array. V1 records omit those two package fields. The
descriptor path is the metadata path because the outer `fixture.toml` remains
the single authoritative AG descriptor/metadata source; a nested subsystem
descriptor is only an opaque declared package entry.

The generator fixes record order, field order, one blank line before each
record, UTF-8 encoding, LF newlines, and one final newline. TOML string lexical
encoding is delegated to the pinned `toml` crate's typed scalar serializer; AG2
does not implement a quoting or escaping language. Golden tests cover quotes,
backslashes, Unicode, tabs, newlines, layout, and exact bytes.

The manifest deliberately contains no capabilities, readiness, eligibility,
expectations, lanes, stability, attempt state, results, timestamps, mtimes,
hashes, hostnames, checkout paths, locale data, environment data, execution
adapters, or external-source identities. Repository paths always use `/`.

Generation is all-or-nothing. Complete discovery and validation precede any
output mutation. `--check` reads only and fails for a missing, stale, or invalid
manifest. `--update` serializes complete bytes to an established same-directory
temporary file, flushes and synchronizes it, revalidates that no output path
component or target became a symlink, and atomically persists over the target
using `tempfile`'s platform replacement implementation. A pre-persist failure
leaves the prior valid manifest intact. This is a narrow checked-in-artifact
replacement boundary, not a general filesystem transaction or directory-sync
durability guarantee.

The CLI accepts exactly no argument or `--check`, and `--update`. Unknown
operations, additional arguments, and combinations such as `--check extra` are
usage errors and do not mutate the repository.

Use:

```sh
make update-conformance-manifest
make check-conformance-manifest
```

CI can run the check target for byte-for-byte freshness without executing a
fixture or selecting an AG execution lane.

## Non-claims and deferred work

The inventory layer proves only layout, descriptor parsing, discovery,
validation, identity, reference/package declaration, and manifest generation.
Fixture registration does not imply executability, pass status, standards
conformance, WPT coverage, or browser compatibility. Generic AG2 inventory does
not execute fixtures. AG4 separately executes classified HTML tokenizer,
tree-construction, and parser-created DOM V2 packages through
`conformance-runner`; AG5 separately executes classified CSS property/value,
selector, cascade, inheritance, and computed-style V2 packages through the CSS
adapter. Layout, Paint/GFX, and Browser/runtime adapters, broader lane
selection, broad WPT/source imports, cross-engine capture, rendered/raster
comparison, and browser automation remain later Milestone AG work.

## Relationship to AG3 classification metadata

AG3 adds the separate human-authored
`tests/conformance/expected-results.toml` registry described by
[`ag3-expected-results-classification-contract.md`](ag3-expected-results-classification-contract.md).
It reconciles one record per validated AG2 `TestId`, but it does not place AG3
fields in either fixture descriptor version or Manifest V2. V1 remains
unchanged; V2 and Manifest V2 add only generic package review truth and continue
to reject unknown AG3 fields.

AG2 inventory truth supplies stable identity and the sole authoritative
`ObservationSurface`. AG3 supplies independent classification, capability,
harness, expectation, stability, environment-requirement, and inert lane-policy
truth. Neither format is generated from the other. The AG3 primary subsystem
owner is derived from AG2's observation, and expected-failure metadata does not
duplicate that observation.

# AG6 rendering packages

AG6 rendering packages use AG2 fixture V2 transport. Execution-package paths
declare files only: width and profile identity live in the strict nested
rendering descriptor and never in `TestId`, filesystem paths, or snapshot file
names. A package can declare at most 64 stylesheets and 80 expectation files,
which totals 144 paths and remains below AG2's 256 support-path ceiling.
