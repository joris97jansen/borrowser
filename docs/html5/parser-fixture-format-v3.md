# Parser fixture declaration v3

Fixture-v3 is a serialization contract for the canonical parser fixture
boundary. It is not an external-fixture execution model.

The load path is:

    fixture-v1/v2/v3 declaration
      -> strict deserialization
      -> version-specific validation
      -> normalized ValidatedFixtureSpec
      -> canonical runner
      -> production CanonicalParserResult observations

Execution plans are semantic and version-independent. Existing v1 single
delivery and v2 parity behavior remains unchanged; fixture-v3 uses the same
normalized parity execution plan when its declaration requests parity.

## External source contract

source.kind = "external" is accepted only by fixture-v3 and requires both:

    [source]
    kind = "external"
    provenance_record = "provenance.toml"
    provenance_sha256 = "<sha256 of that record>"

The validator reads the referenced record, verifies its hash, rejects missing
or malformed required fields, and rejects it as an orphan sidecar if it is not
part of the declaration. A provenance record is versioned and contains the
upstream project, stable source revision identifier, source path, one-based
record ordinal, record SHA-256, source-file SHA-256, licence identifier,
required licence notice, attribution, and representation-only adaptation
description. The generic fixture-v3 boundary does not require a Git commit
shape; source-specific adapters may impose stronger revision rules.

The record's case identity is derived as:

    <revision>:<source path>:<record ordinal>:<record SHA-256>

An importer or documentation cannot make an incomplete external source valid.
Native v1/v2 declarations remain stable, but their legacy free-form external
source form is no longer accepted as a canonical external fixture.

## Expectation strength

Native exact typed parse-error snapshots remain unchanged:

    parse_errors = "parse-errors.txt"

Fixture-v3 can also preserve a weaker upstream count expectation:

    [expectations.parse_errors]
    kind = "count"
    count = 1

The production CanonicalParserResult still captures Borrowser's typed parse
errors. Count comparison compares only the number of captured observations and
never fabricates typed identities from upstream diagnostic text. Failure
diagnostics spell count mismatches as parse-error-count; exact snapshot
mismatches retain the existing parse-errors spelling.

The existing html5-dom-v3 tree snapshot is reused for namespaces, templates,
doctypes, comments, text, attributes, and processing instructions. Fixture-v3
does not imply a new DOM snapshot version.
