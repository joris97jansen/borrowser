# Borrowser HTML Parser Fixture Format v2

## Identity and dispatch

Native AE13 canonical fixtures use:

```toml
format = "borrowser-html-parser-fixture-v2"
```

The loader first deserializes only a minimal `format` envelope. It then
deserializes the original complete TOML into the selected strict schema.
`FixtureFileV1` and `FixtureFileV2` both deny unknown fields. The envelope is a
selector, not a permissive common schema, and the loader never probes schemas
until one happens to deserialize. Unknown identifiers are
`UnsupportedFixtureFormat`.

Fixture IDs are obtained through an exhaustive accessor over the typed v1/v2
declaration, so duplicate and ASCII case-collision checks are version-neutral.
Fixture v1 remains governed by `parser-fixture-format-v1.md`; none of its scalar
failure identities acquire v2 meanings.

## Difference from fixture v1

V2 activates the typed AE13b1-b4 canonical observation surfaces, strict
versioned snapshot codecs, document execution, delivery-specific transitions,
and structured expected failures. V1 retains its scalar expected-failure field
and legacy token-driver compatibility path. The small native AE13 corpus uses
v2; this does not migrate WPT, html5lib, or broad legacy golden corpora.

All shared fixture fields retain their v1 syntax: source, exact input and hash,
target, declared deliveries, expectation paths, metadata, extensions, and
disposition status. Complete v2 structures remain `deny_unknown_fields`.

## Structured expected failures

V2 uses a real TOML subtable:

```toml
[disposition]
status = "expected-failure"
reason = "Tracked parser invariant."
reference = { kind = "tracking-issue", value = "#123" }

[disposition.failure]
kind = "parser-observation"
identity = "tokenizer-invariant"
code = "pending-text-range-invalid"
```

Failure kinds and fields are exact:

- `snapshot-read`: requires `surface` only.
- `snapshot-format`: requires `surface` only.
- `parser-observation`: requires `identity` plus exactly the `code` or `site`
  owned by that identity.
- `validated-runner-invariant`: requires `code` only.
- `expectation-mismatch`: requires `surface` only.
- `final-invariant`: requires `code` only. Execution remains unsupported until
  AE13c.

V1 scalar values such as `token-snapshot-read`, `token-snapshot-format`, and
`tokenizer-driver` are rejected by the v2 schema rather than reinterpreted.

## Declared and planned deliveries

```text
declared delivery = required fixture semantics
planned delivery  = requires production execution
```

Every declared delivery is capability-checked in fixture declaration order,
including unused deliveries. The first unsupported declaration makes the
fixture unsupported. This check does not execute the parser.

Only these deliveries are planned:

- the reference delivery when an ordinary surface is expected;
- each delivery named by a transition expectation;
- one unioned execution when those roles select the same delivery.

Unused supported deliveries do not execute. Planned deliveries execute in
validated declaration order and at most once. Each production request contains
the union of the surfaces required from that delivery.

AE13b5 supports whole Unicode-scalar delivery. Raw bytes, byte delivery,
Unicode-scalar boundaries, fragment parsing, and scripting-enabled parsing are
typed unsupported semantics. This is not whole/chunked parity.

## Skipped disposition

After declaration, path, input, hash, disposition, and cross-field validation,
a sealed `skipped` fixture short-circuits:

```text
validated fixture -> skipped-disposition check -> ordinary precedence
```

The result is `NotExecuted` with the exact validated skip classification. No
sidecar content is read, no capability plan is built, no parser runs, and no
canonical result is exposed. Disposition evaluation still compares the exact
declared classification. Expected-failure and expected-unsupported fixtures do
not use this short-circuit; they execute so the expected outcome can be
verified.

Declaration validation checks each sidecar using metadata only: normalized
bundle-relative containment, every path component's symlink status, existence,
regular-file type, case/path collisions, declaration consistency, and orphan
rules. It does not open the sidecar, inspect its length, or read and discard its
bytes. Complete sidecar content is read only in phase 5 below, after the skipped
short-circuit and unsupported-semantics precedence. The injected private file
access boundary distinguishes metadata inspection from content reads so tests
prove skipped fixtures perform zero sidecar-content reads and active fixtures
read each declared expectation once in fixed order.

## Authoritative precedence

For non-skipped v2 fixtures:

1. Start from the completely validated fixture.
2. Reject the first required unknown extension in ASCII lexicographic ID order.
3. Reject the first unsupported expectation surface; final invariants are
   unsupported in AE13b5.
4. Reject unsupported target, input, scripting, and declared-delivery
   semantics. Delivery selection follows declaration order.
5. Read and strictly parse all supported sidecars in surface order.
6. Build the deterministic execution plan.
7. Execute planned deliveries in declaration order.
8. Validate every requested `ObservationState` after every execution succeeds.
9. Reject `Incomplete`, requested `NotRequested`, unexpected `NotApplicable`,
   and prohibited unrequested capture before serialization.
10. Serialize all requested typed canonical surfaces.
11. Compare in the order below.
12. Return a completed report only after every delivery and expectation passes.

Earlier phases win. Thus unsupported input masks malformed sidecars, parser
failure masks mismatch, and incomplete capture masks every mismatch.

The following simultaneous failures therefore resolve exactly as follows:

- unknown required extension plus malformed snapshot: unknown required
  extension;
- final-invariant expectation plus malformed unrelated snapshot: unsupported
  final-invariant expectation;
- unsupported raw-byte input plus malformed snapshot: raw-byte input;
- parser execution failure plus snapshot mismatch: parser-observation failure;
- incomplete reference delivery plus transition-delivery mismatch: incomplete
  observation;
- multiple mismatching ordinary surfaces: the first surface in comparison
  order;
- multiple mismatching transition expectations: the first transition in
  fixture declaration order.

Required unknown extensions originate from a map and are sorted by ASCII ID;
TOML source ordering is not semantic.

## Comparison order and results

The first mismatch is returned in this exact order:

1. tokens;
2. parse errors;
3. implementation diagnostics;
4. document mode;
5. canonical tree;
6. canonical patches;
7. transitions in transition-expectation declaration order;
8. unsupported features.

Transitions precede unsupported features because they describe the dispatch
attempt leading to an unsupported-feature fallback.

Completed reports are all-or-nothing and contain every planned delivery result
in declaration order. A v2 report owns each `CanonicalParserResult` exactly
once, inside its delivery report; the compatibility `result()` accessor borrows
the reference-delivery entry. A failure can retain small private context for
diagnostics, but it cannot expose a completed partial report or usable partial
canonical result. Parser failures name the failing delivery but do not retain
earlier successful delivery results or a canonical result for the failing
delivery. Incomplete states name their delivery and surface. A mismatch retains
the exact failing delivery, surface, and textual difference, not a cloned
canonical result; in particular a transition mismatch never attaches the
reference-delivery result.

## Observation guardrails

The runner passes one private `FixtureObservationGuardrails` policy through
request construction. Production fixture runs use these fixed defensive
capacities:

| Surface | Capacity | Policy reason |
|---|---:|---|
| tokens | 65,536 | bounded focused-fixture event capture |
| parse errors | 65,536 | bounded dense recovery capture |
| implementation diagnostics | 65,536 | bounded diagnostic capture |
| unsupported features | 65,536 | bounded unsupported event capture |
| canonical tree units | 131,072 | bounded structural projection |
| transitions | 262,144 | dispatch attempts may outnumber tokens |
| patch operations | 262,144 | construction operations may outnumber nodes |

Canonical tree units are parser-created structural units: document, document
type, element or HTML template host, text, comment, processing instruction, and
typed template-contents boundary. Attributes and the outer `ObservedTree`
wrapper do not consume units.

These are defensive harness policy, not production parser limits, fixture input
size limits, or byte budgets. They cannot be configured by TOML or sidecars and
never derive from expected record counts or content. Derived arithmetic is
checked. Tests inject smaller private policies through the same request builder
to prove exact-capacity capture and capacity-plus-one incompleteness for every
retained collection. Capacity overflow remains authoritative
`IncompleteObservation`; its retained prefix is never serialized or compared,
even if that prefix equals the expected sidecar.

An incomplete outcome retains the delivery, expectation surface, complete
typed `IncompleteObservationReason`, retained count, and dropped count. A
requested incomplete state is this typed outcome. Requested `NotRequested` or
unexpected `NotApplicable`, and unrequested `Captured` or `Incomplete`, are
distinct validated-runner invariants. Diagnostics spell the delivery, surface,
reason, retained count, and dropped count without Rust `Debug`.

## Stable failure identities

Canonical execution failures are one of:

- `SnapshotRead(surface)`;
- `SnapshotFormat(surface)`;
- `ParserObservation(closed identity)`;
- `ValidatedFixtureInvariant(stable code)`.

The HTML subsystem maps every typed production execution error to a closed,
feature-gated identity. One authoritative fixture-v2 spelling codec both
parses structured declarations and formats diagnostic/disposition identities;
there are no independent string-to-enum and enum-to-string tables. Test support
does not classify messages or Rust `Debug`. Stable runner-invariant codes name impossible post-validation states;
ordinary fixture mistakes remain validation, unsupported, snapshot-format, or
mismatch outcomes.

The exact parser-observation identity spellings are:

- `parser-fatal-engine-invariant` (no `code` or `site`);
- `parser-fatal-resource-exhaustion` (`site` required);
- `parser-invariant` (no `code` or `site`);
- `tokenizer-invariant` (`code` required);
- `token-canonicalization-invariant` (no `code` or `site`);
- `tree-transition-token-canonicalization-invariant` (no `code` or `site`);
- `unsupported-feature-observation-invariant` (`code` required);
- `observation-recorder-missing` (no `code` or `site`);
- `patch-history-capture-missing` (no `code` or `site`);
- `observation-invariant` (`code` required);
- `observation-resource-exhaustion` (`site` required).

Parser-fatal sites are `known-tag-atom-storage`,
`known-tag-lookup-storage`, `template-child-storage`, and
`patch-history-observation-storage`. Observation sites are
`canonical-tree-projection`, `canonical-patch-projection`, and
`snapshot-label-storage`. Codes are closed exhaustive mappings of the nested
typed tokenizer, unsupported-feature-observation, and observation-invariant
enums; an identity with a missing, extra, or unknown owned field is invalid
fixture-v2 disposition syntax.

The exact validated-runner invariant codes are:

- `planned-reference-delivery-missing`;
- `planned-delivery-missing`;
- `duplicate-planned-delivery`;
- `requested-surface-unexpectedly-not-requested`;
- `requested-surface-unexpectedly-not-applicable`;
- `unrequested-surface-unexpectedly-captured`;
- `unrequested-surface-unexpectedly-incomplete`;
- `snapshot-variant-surface-contradiction`;
- `canonical-serializer-surface-contradiction`;
- `comparison-surface-contradiction`;
- `missing-executed-delivery-result`;
- `duplicate-executed-delivery-result`;
- `duplicate-expectation-identity`.

These codes are reserved for impossible runner states. They cannot be used to
reclassify an ordinary validation error, unsupported semantic, malformed
snapshot, incomplete capture, or mismatch.

Final-invariant execution, parity, fragment parsing, external adapters,
snapshot blessing, scripting, rendering, and public parser/DOM APIs are outside
fixture v2 AE13b5.
